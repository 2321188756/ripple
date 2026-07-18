//! 日志管理命令：前端写入日志、读取有界的结构化日志快照。

use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::UNIX_EPOCH;

/// 单次读取的硬上限。即使前端传入极大的行数，也不会把整个日志载入内存。
const LOG_TAIL_BYTE_CAP: u64 = 512 * 1024;
const DEFAULT_LOG_LINES: usize = 500;
const MAX_LOG_LINES: usize = 2_000;

/// 全局日志文件路径（由 setup 时设置）
static LOG_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn set_log_dir(path: PathBuf) {
    let _ = LOG_DIR.set(path);
}

pub fn get_log_dir() -> Option<&'static PathBuf> {
    LOG_DIR.get()
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub timestamp: Option<String>,
    pub level: String,
    pub target: Option<String>,
    pub message: String,
    pub raw: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LogSnapshot {
    pub path: String,
    pub file_size: u64,
    pub modified_at_ms: Option<u64>,
    pub byte_cap: u64,
    pub requested_lines: usize,
    pub returned_lines: usize,
    /// 文件头或更早的日志因行数/字节硬上限未包含在本快照中。
    pub truncated: bool,
    pub entries: Vec<LogEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ClientLogEvent {
    SendIgnored,
    ConversationCreating,
    ConversationCreateFailed,
    SendStarted {
        conversation_id: String,
        content_chars: usize,
    },
    SendSucceeded {
        message_id: String,
    },
    SendFailed,
}

/// 记录 allowlist 中的前端诊断事件。事件不接受自由文本，避免 renderer 将正文或凭据写入持久日志。
#[tauri::command]
pub fn log_event(event: ClientLogEvent) -> Result<(), String> {
    match &event {
        ClientLogEvent::SendStarted {
            conversation_id,
            content_chars,
        } => {
            uuid::Uuid::parse_str(conversation_id)
                .map_err(|_| "invalid conversation id".to_string())?;
            if *content_chars > 10 * 1024 * 1024 {
                return Err("content length exceeds diagnostic limit".into());
            }
        }
        ClientLogEvent::SendSucceeded { message_id } => {
            uuid::Uuid::parse_str(message_id).map_err(|_| "invalid message id".to_string())?;
        }
        _ => {}
    }
    match event {
        ClientLogEvent::SendIgnored => {
            tracing::warn!(target: "frontend", event = "chat_send_ignored")
        }
        ClientLogEvent::ConversationCreating => {
            tracing::info!(target: "frontend", event = "chat_conversation_creating")
        }
        ClientLogEvent::ConversationCreateFailed => {
            tracing::error!(target: "frontend", event = "chat_conversation_create_failed")
        }
        ClientLogEvent::SendStarted {
            conversation_id,
            content_chars,
        } => tracing::info!(
            target: "frontend",
            event = "chat_send_started",
            %conversation_id,
            content_chars,
        ),
        ClientLogEvent::SendSucceeded { message_id } => tracing::info!(
            target: "frontend",
            event = "chat_send_succeeded",
            %message_id,
        ),
        ClientLogEvent::SendFailed => {
            tracing::error!(target: "frontend", event = "chat_send_failed")
        }
    }
    Ok(())
}

fn latest_log_path(log_dir: &Path) -> Result<PathBuf, String> {
    let entries = std::fs::read_dir(log_dir).map_err(|e| format!("read log dir: {e}"))?;
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains("ripple.log"))
        })
        .max_by_key(|path| {
            std::fs::metadata(path)
                .and_then(|metadata| metadata.modified())
                .ok()
        })
        .ok_or_else(|| "no log file found".to_string())
}

/// 获取日志文件路径（前端可直接定位）
#[tauri::command]
pub fn get_log_path() -> Result<String, String> {
    let log_dir = get_log_dir().ok_or("log dir not initialized")?;
    Ok(latest_log_path(log_dir)?.to_string_lossy().to_string())
}

fn parse_log_entry(line: &str) -> LogEntry {
    let raw = line.to_string();
    let trimmed = line.trim_start();
    let (first, after_timestamp) = trimmed
        .split_once(char::is_whitespace)
        .map(|(timestamp, rest)| (Some(timestamp), rest.trim_start()))
        .unwrap_or((Some(trimmed), ""));
    let (second, rest) = after_timestamp
        .split_once(char::is_whitespace)
        .map(|(level, rest)| (Some(level), rest.trim_start()))
        .unwrap_or((Some(after_timestamp), ""));

    let looks_like_timestamp = first.is_some_and(|value| value.contains('T'));
    let normalized_level = second
        .map(|value| value.trim())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let known_level = matches!(
        normalized_level.as_str(),
        "trace" | "debug" | "info" | "warn" | "error"
    );

    if looks_like_timestamp && known_level {
        let remainder = rest;
        let (target, message) = match remainder.split_once(": ") {
            Some((target, message)) if !target.contains(char::is_whitespace) => {
                (Some(target.to_string()), message.to_string())
            }
            _ => (None, remainder.to_string()),
        };
        LogEntry {
            timestamp: first.map(str::to_string),
            level: normalized_level,
            target,
            message,
            raw,
        }
    } else {
        LogEntry {
            timestamp: None,
            level: "unknown".to_string(),
            target: None,
            message: line.to_string(),
            raw,
        }
    }
}

fn read_log_snapshot(path: &Path, requested_lines: usize) -> Result<LogSnapshot, String> {
    let requested_lines = requested_lines.clamp(1, MAX_LOG_LINES);
    let mut file = File::open(path).map_err(|e| format!("open log: {e}"))?;
    let metadata = file
        .metadata()
        .map_err(|e| format!("read log metadata: {e}"))?;
    let file_size = metadata.len();
    let start = file_size.saturating_sub(LOG_TAIL_BYTE_CAP);
    file.seek(SeekFrom::Start(start))
        .map_err(|e| format!("seek log: {e}"))?;

    let mut bytes = Vec::with_capacity((file_size - start) as usize);
    file.take(LOG_TAIL_BYTE_CAP)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("read log tail: {e}"))?;

    let text = String::from_utf8_lossy(&bytes);
    let skipped_partial_line = start > 0 && !text.starts_with('\n');
    let bounded_text = if skipped_partial_line {
        text.split_once('\n')
            .map(|(_, rest)| rest)
            .unwrap_or_default()
    } else {
        text.as_ref()
    };
    let all_lines: Vec<&str> = bounded_text.lines().collect();
    let line_start = all_lines.len().saturating_sub(requested_lines);
    let entries: Vec<LogEntry> = all_lines[line_start..]
        .iter()
        .map(|line| parse_log_entry(line))
        .collect();
    let truncated = start > 0 || line_start > 0 || skipped_partial_line;

    Ok(LogSnapshot {
        path: path.to_string_lossy().to_string(),
        file_size,
        modified_at_ms: metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64),
        byte_cap: LOG_TAIL_BYTE_CAP,
        requested_lines,
        returned_lines: entries.len(),
        truncated,
        entries,
    })
}

/// 读取最新日志文件末尾的结构化快照。
#[tauri::command]
pub fn get_logs(lines: Option<usize>) -> Result<LogSnapshot, String> {
    let log_dir = get_log_dir().ok_or("log dir not initialized")?;
    let log_path = latest_log_path(log_dir)?;
    read_log_snapshot(&log_path, lines.unwrap_or(DEFAULT_LOG_LINES))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_log(name: &str, content: &[u8]) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "ripple-log-test-{name}-{}-{}.log",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut file = File::create(&path).unwrap();
        file.write_all(content).unwrap();
        path
    }

    #[test]
    fn parses_structured_tracing_lines_and_unknown_lines() {
        let parsed = parse_log_entry(
            "2026-07-13T14:43:49.203148Z  INFO ripple_app_lib::commands::memory: database ready",
        );
        assert_eq!(
            parsed.timestamp.as_deref(),
            Some("2026-07-13T14:43:49.203148Z")
        );
        assert_eq!(parsed.level, "info");
        assert_eq!(
            parsed.target.as_deref(),
            Some("ripple_app_lib::commands::memory")
        );
        assert_eq!(parsed.message, "database ready");

        let unknown = parse_log_entry("continuation line");
        assert_eq!(unknown.level, "unknown");
        assert_eq!(unknown.message, "continuation line");
    }

    #[test]
    fn returns_only_requested_tail_lines_with_metadata() {
        let path = temp_log(
            "tail",
            b"2026-07-13T00:00:00Z INFO app: one\n2026-07-13T00:00:01Z WARN app: two\n2026-07-13T00:00:02Z ERROR app: three\n",
        );
        let snapshot = read_log_snapshot(&path, 2).unwrap();
        std::fs::remove_file(&path).unwrap();

        assert_eq!(snapshot.requested_lines, 2);
        assert_eq!(snapshot.returned_lines, 2);
        assert!(snapshot.truncated);
        assert_eq!(snapshot.entries[0].level, "warn");
        assert_eq!(snapshot.entries[1].message, "three");
    }

    #[test]
    fn enforces_byte_and_line_hard_caps() {
        let mut content = Vec::new();
        for index in 0..20_000 {
            writeln!(
                content,
                "2026-07-13T00:00:00Z INFO app: line-{index:05}-{}",
                "x".repeat(32)
            )
            .unwrap();
        }
        assert!(content.len() as u64 > LOG_TAIL_BYTE_CAP);
        let path = temp_log("cap", &content);
        let snapshot = read_log_snapshot(&path, usize::MAX).unwrap();
        std::fs::remove_file(&path).unwrap();

        assert_eq!(snapshot.byte_cap, LOG_TAIL_BYTE_CAP);
        assert_eq!(snapshot.requested_lines, MAX_LOG_LINES);
        assert!(snapshot.returned_lines <= MAX_LOG_LINES);
        assert!(snapshot.truncated);
        assert!(snapshot
            .entries
            .last()
            .unwrap()
            .message
            .contains("line-19999"));
    }
}
