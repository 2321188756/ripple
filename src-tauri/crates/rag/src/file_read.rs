//! 文件读取：按扩展名分发。PDF 用 pdf-extract 提取文本，其他直接 read_to_string。

use std::path::Path;

/// 读取文件内容为纯文本。
/// - `.pdf`：用 `pdf-extract` 提取文本（仅支持文本型 PDF，扫描件不支持）
/// - 其他：直接 `read_to_string`（txt/md/代码等）
pub fn read_file_content(path: &str) -> Result<String, String> {
    let ext = Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext == "pdf" {
        pdf_extract::extract_text(path).map_err(|e| format!("pdf parse: {e}"))
    } else {
        std::fs::read_to_string(path).map_err(|e| format!("read file: {e}"))
    }
}
