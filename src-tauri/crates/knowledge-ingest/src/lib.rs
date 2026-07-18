//! Server-side immutable upload snapshot support.
//!
//! The upload HTTP adapter will stream into `LocalObjectStore`; this crate owns
//! only content-addressed object semantics and never returns host filesystem paths.

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use std::{io, path::PathBuf};
use tokio::{fs, io::AsyncWriteExt};

#[derive(Debug, Clone)]
pub struct StoredObject {
    pub key: String,
    pub sha256: [u8; 32],
    pub byte_size: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum ObjectStoreError {
    #[error("object input is too large")]
    TooLarge,
    #[error("object storage failure")]
    Storage,
}

#[async_trait]
pub trait ObjectStore: Send + Sync {
    async fn put_bytes(
        &self,
        organization_scope: &str,
        bytes: &[u8],
        max_bytes: u64,
    ) -> Result<StoredObject, ObjectStoreError>;
    async fn read_bytes(&self, key: &str, max_bytes: u64) -> Result<Vec<u8>, ObjectStoreError>;
}

#[derive(Clone)]
pub struct LocalObjectStore {
    root: PathBuf,
}

impl LocalObjectStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
    fn safe_scope(scope: &str) -> Option<&str> {
        (!scope.is_empty() && scope.chars().all(|c| c.is_ascii_hexdigit() || c == '-'))
            .then_some(scope)
    }
    fn path_for(&self, key: &str) -> Option<PathBuf> {
        let mut parts = key.split('/');
        let scope = parts.next()?;
        let directory = parts.next()?;
        let prefix = parts.next()?;
        let hash = parts.next()?;
        if parts.next().is_some()
            || directory != "sha256"
            || Self::safe_scope(scope).is_none()
            || prefix.len() != 2
            || hash.len() != 64
            || !prefix.chars().all(|c| c.is_ascii_hexdigit())
            || !hash.chars().all(|c| c.is_ascii_hexdigit())
        {
            return None;
        }
        Some(self.root.join(scope).join("sha256").join(prefix).join(hash))
    }
}

#[async_trait]
impl ObjectStore for LocalObjectStore {
    async fn put_bytes(
        &self,
        organization_scope: &str,
        bytes: &[u8],
        max_bytes: u64,
    ) -> Result<StoredObject, ObjectStoreError> {
        if bytes.len() as u64 > max_bytes || Self::safe_scope(organization_scope).is_none() {
            return Err(ObjectStoreError::TooLarge);
        }
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        let hash = hex::encode(digest);
        let key = format!("{organization_scope}/sha256/{}/{hash}", &hash[..2]);
        let path = self.path_for(&key).ok_or(ObjectStoreError::Storage)?;
        if fs::try_exists(&path)
            .await
            .map_err(|_| ObjectStoreError::Storage)?
        {
            return Ok(StoredObject {
                key,
                sha256: digest,
                byte_size: bytes.len() as u64,
            });
        }
        let parent = path.parent().ok_or(ObjectStoreError::Storage)?;
        fs::create_dir_all(parent)
            .await
            .map_err(|_| ObjectStoreError::Storage)?;
        let temporary = parent.join(format!(".{hash}.tmp-{}", uuid::Uuid::new_v4()));
        let mut file = fs::File::create(&temporary)
            .await
            .map_err(|_| ObjectStoreError::Storage)?;
        file.write_all(bytes)
            .await
            .map_err(|_| ObjectStoreError::Storage)?;
        file.sync_all()
            .await
            .map_err(|_| ObjectStoreError::Storage)?;
        drop(file);
        match fs::rename(&temporary, &path).await {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let _ = fs::remove_file(&temporary).await;
            }
            Err(_) => {
                let _ = fs::remove_file(&temporary).await;
                return Err(ObjectStoreError::Storage);
            }
        }
        Ok(StoredObject {
            key,
            sha256: digest,
            byte_size: bytes.len() as u64,
        })
    }
    async fn read_bytes(&self, key: &str, max_bytes: u64) -> Result<Vec<u8>, ObjectStoreError> {
        let path = self.path_for(key).ok_or(ObjectStoreError::Storage)?;
        let metadata = fs::metadata(&path)
            .await
            .map_err(|_| ObjectStoreError::Storage)?;
        if metadata.len() > max_bytes {
            return Err(ObjectStoreError::TooLarge);
        }
        fs::read(path).await.map_err(|_| ObjectStoreError::Storage)
    }
}

#[derive(Debug, Clone)]
pub struct ExtractedText {
    pub title: String,
    pub normalized_text: String,
    pub extractor_id: &'static str,
    pub extractor_version: &'static str,
    pub warnings: Vec<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ExtractionError {
    #[error("unsupported document type")]
    UnsupportedType,
    #[error("document encoding is invalid")]
    InvalidEncoding,
    #[error("document contains no searchable text")]
    Empty,
    #[error("document text is too large")]
    TooLarge,
}

pub fn supports_text_mime(mime_type: &str) -> bool {
    let mime_type = mime_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    mime_type.starts_with("text/")
        || matches!(
            mime_type.as_str(),
            "application/json"
                | "application/xml"
                | "application/javascript"
                | "application/x-javascript"
                | "application/toml"
                | "application/yaml"
                | "application/x-yaml"
        )
}

pub fn extract_text(
    bytes: &[u8],
    mime_type: &str,
    display_name: &str,
    max_text_bytes: usize,
) -> Result<ExtractedText, ExtractionError> {
    if !supports_text_mime(mime_type) {
        return Err(ExtractionError::UnsupportedType);
    }
    if bytes.len() > max_text_bytes {
        return Err(ExtractionError::TooLarge);
    }
    let decoded = std::str::from_utf8(bytes).map_err(|_| ExtractionError::InvalidEncoding)?;
    if decoded.contains('\0') {
        return Err(ExtractionError::InvalidEncoding);
    }
    let normalized_text = decoded.replace("\r\n", "\n").replace('\r', "\n");
    let normalized_text = normalized_text.trim().to_owned();
    if normalized_text.is_empty() {
        return Err(ExtractionError::Empty);
    }
    let title = display_name.trim();
    Ok(ExtractedText {
        title: if title.is_empty() { "Untitled" } else { title }.to_owned(),
        normalized_text,
        extractor_id: "ripple.plain-text",
        extractor_version: "1",
        warnings: Vec::new(),
    })
}

#[derive(Debug, Clone)]
pub struct TextChunk {
    pub ordinal: i32,
    pub content: String,
    pub token_count: i32,
    pub char_start: i32,
    pub char_end: i32,
}

/// Deterministic first-release chunker. It treats whitespace-delimited words as
/// tokens until a model-specific tokenizer profile is configured. It never edits
/// visible content or injects overlap markers into source text.
pub fn chunk_text(text: &str, target_tokens: usize, max_tokens: usize) -> Vec<TextChunk> {
    let target_tokens = target_tokens.clamp(64, max_tokens.max(64));
    let max_tokens = max_tokens.max(target_tokens);
    let mut chunks = Vec::new();
    let mut start = 0usize;
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    while start < chars.len() {
        let end_limit = (start + max_tokens).min(chars.len());
        let target_end = (start + target_tokens).min(end_limit);
        let mut end = target_end;
        for index in (start + 1..=end_limit).rev() {
            if index <= target_end
                && matches!(
                    chars[index - 1].1,
                    '.' | '。' | '!' | '！' | '?' | '？' | '\n'
                )
            {
                end = index;
                break;
            }
        }
        if end <= start {
            end = end_limit;
        }
        let char_start = chars[start].0;
        let char_end = if end < chars.len() {
            chars[end].0
        } else {
            text.len()
        };
        let content = text[char_start..char_end].trim().to_owned();
        if !content.is_empty() {
            let token_count = content.split_whitespace().count().max(1) as i32;
            chunks.push(TextChunk {
                ordinal: chunks.len() as i32,
                content,
                token_count,
                char_start: char_start as i32,
                char_end: char_end as i32,
            });
        }
        start = end;
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_and_normalizes_supported_text() {
        let extracted = extract_text(
            b"# Title\r\n\r\nBody\rLine",
            "text/markdown; charset=utf-8",
            "guide.md",
            1024,
        )
        .unwrap();
        assert_eq!(extracted.title, "guide.md");
        assert_eq!(extracted.normalized_text, "# Title\n\nBody\nLine");
        assert_eq!(extracted.extractor_id, "ripple.plain-text");
    }

    #[test]
    fn rejects_binary_and_invalid_utf8() {
        assert_eq!(
            extract_text(b"binary", "application/pdf", "file.pdf", 1024).unwrap_err(),
            ExtractionError::UnsupportedType
        );
        assert_eq!(
            extract_text(&[0xff, 0xfe], "text/plain", "bad.txt", 1024).unwrap_err(),
            ExtractionError::InvalidEncoding
        );
        assert_eq!(
            extract_text(b"a\0b", "text/plain", "bad.txt", 1024).unwrap_err(),
            ExtractionError::InvalidEncoding
        );
    }

    #[test]
    fn enforces_text_limit_and_non_empty_content() {
        assert_eq!(
            extract_text(b"too long", "text/plain", "large.txt", 3).unwrap_err(),
            ExtractionError::TooLarge
        );
        assert_eq!(
            extract_text(b" \r\n ", "text/plain", "empty.txt", 1024).unwrap_err(),
            ExtractionError::Empty
        );
    }
}
