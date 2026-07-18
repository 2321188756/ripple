//! 文本分块。支持 Markdown 感知分块 + 固定大小 + 滑动窗口重叠。

use crate::types::Chunk;

/// 分块配置
#[derive(Debug, Clone)]
pub struct ChunkConfig {
    pub chunk_size: usize,    // 每块最多字符数
    pub chunk_overlap: usize, // 相邻块重叠字符数
}

impl Default for ChunkConfig {
    fn default() -> Self {
        Self {
            chunk_size: 1000,
            chunk_overlap: 100,
        }
    }
}

/// 按固定大小分块（保持段落完整性，Markdown 感知）
pub fn chunk_text(text: &str, doc_id: &str, kb_id: &str, config: &ChunkConfig) -> Vec<Chunk> {
    let mut chunks = Vec::new();

    // 先按段落拆分（double newline）
    let paragraphs: Vec<&str> = text.split("\n\n").collect();
    let mut current = String::new();

    for para in paragraphs {
        let para = para.trim();
        if para.is_empty() {
            continue;
        }

        // 如果当前块 + 新段落 ≤ chunk_size，追加（按字符数比较，CJK 不被字节数高估）
        if current.chars().count() + para.chars().count() + 2 <= config.chunk_size {
            if !current.is_empty() {
                current.push_str("\n\n");
            }
            current.push_str(para);
        } else {
            // 当前块已满，存入
            if !current.is_empty() {
                chunks.push(make_chunk(&current, doc_id, kb_id, chunks.len()));
            }
            // 新段落作为新块（如果它本身就超长，按行再切）
            if para.chars().count() > config.chunk_size {
                for line_chunk in split_long_para(para, config) {
                    chunks.push(make_chunk(&line_chunk, doc_id, kb_id, chunks.len()));
                }
                current = String::new();
            } else {
                current = para.to_string();
            }
        }
    }
    // 最后一块
    if !current.is_empty() {
        chunks.push(make_chunk(&current, doc_id, kb_id, chunks.len()));
    }

    // 滑动窗口重叠
    if config.chunk_overlap > 0 && chunks.len() > 1 {
        add_overlap(&mut chunks, config);
    }

    chunks
}

/// 超长段落按行拆分（例：代码块、长文本）
fn split_long_para(para: &str, config: &ChunkConfig) -> Vec<String> {
    let lines: Vec<&str> = para.lines().collect();
    let mut result = Vec::new();
    let mut buf = String::new();
    for line in lines {
        if buf.chars().count() + line.chars().count() + 1 > config.chunk_size {
            result.push(buf.clone());
            buf = line.to_string();
        } else {
            if !buf.is_empty() {
                buf.push('\n');
            }
            buf.push_str(line);
        }
    }
    if !buf.is_empty() {
        result.push(buf);
    }
    result
}

/// 添加滑动窗口重叠（每块末尾追加下一块开头的内容）
fn add_overlap(chunks: &mut [Chunk], config: &ChunkConfig) {
    let overlap = config.chunk_overlap.min(200);
    for i in 0..chunks.len() - 1 {
        let next_start = chunks[i + 1]
            .content
            .chars()
            .take(overlap)
            .collect::<String>();
        if !next_start.is_empty() {
            chunks[i].content.push_str("\n\n[overlap]\n");
            chunks[i].content.push_str(&next_start);
        }
    }
}

fn make_chunk(content: &str, doc_id: &str, kb_id: &str, index: usize) -> Chunk {
    Chunk {
        id: uuid::Uuid::new_v4().to_string(),
        doc_id: doc_id.to_string(),
        kb_id: kb_id.to_string(),
        chunk_index: index,
        content: content.to_string(),
        metadata: serde_json::json!({}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_chunking() {
        let text = "Para one.\n\nPara two.\n\nPara three.";
        // chunk_size 15：每段 9~11 字符，第二段起 9+9+2=20 > 15 触发切分，故产生多块。
        // （早期 chunk_size 50 能装下整段 33 字符，只会产 1 块，与 >=2 断言矛盾。）
        let chunks = chunk_text(
            text,
            "doc1",
            "kb1",
            &ChunkConfig {
                chunk_size: 15,
                chunk_overlap: 0,
            },
        );
        assert!(chunks.len() >= 2);
        assert!(chunks[0].content.contains("Para one"));
    }

    #[test]
    fn overlap_added() {
        let text = "A\n\nB\n\nC\n\nD\n\nE";
        let chunks = chunk_text(
            text,
            "doc1",
            "kb1",
            &ChunkConfig {
                chunk_size: 10,
                chunk_overlap: 5,
            },
        );
        // Some chunks should have [overlap] marker
        let has_overlap = chunks.iter().any(|c| c.content.contains("[overlap]"));
        assert!(has_overlap);
    }
}
