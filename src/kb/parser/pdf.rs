//! PDF parser with page-level extraction, header/footer cleaning, and OCR tagging
//!
//! P2-004: Outputs page_number metadata per block
//! P2-005: Heuristic header/footer removal
//! P2-006: Text density detection → needs_ocr flag

use super::{DocumentParser, ParsedDocument};
use crate::error::GBrainError;
use std::collections::HashMap;

pub struct PdfParser;

impl Default for PdfParser {
    fn default() -> Self {
        Self::new()
    }
}

impl PdfParser {
    pub fn new() -> Self {
        Self
    }
}

impl DocumentParser for PdfParser {
    fn parse(&self, data: &[u8]) -> Result<ParsedDocument, GBrainError> {
        let pdf = lopdf::Document::load_mem(data)
            .map_err(|e| GBrainError::FileError(format!("PDF load failed: {}", e)))?;

        let pages = pdf.get_pages();
        let total_pages = pages.len();
        let page_numbers: Vec<u32> = pages.keys().copied().collect();

        // FIX9-04: 在拼接全文时累计字符偏移，为每页 block 写入 source span
        let mut page_texts: Vec<String> = Vec::new();
        let mut all_text = Vec::new();
        let mut low_density_pages = 0u32;
        // 记录每页 block 在全文中的起止偏移
        let mut page_spans: Vec<(usize, usize)> = Vec::new();
        let mut global_offset: usize = 0;

        for page_num in &page_numbers {
            match pdf.extract_text(&[*page_num]) {
                Ok(text) => {
                    let cleaned = clean_text(&text);
                    let deduped = remove_header_footer(&cleaned, &page_texts);

                    let density = deduped.chars().count();
                    if density < 50 {
                        low_density_pages += 1;
                    }

                    let page_block = format!("[PAGE:{}]\n{}", page_num, deduped);
                    // FIX9-04: 记录此页在全文中的起止偏移
                    let start = global_offset;
                    // FIX10-08: 统一使用字符偏移（chars().count()），禁止混用 byte 长度
                    let block_len = page_block.chars().count();
                    global_offset += block_len + 2; // 加上 "\n\n" 分隔符长度
                    let end = start + block_len;
                    page_spans.push((start, end));
                    page_texts.push(deduped.clone());
                    if !deduped.is_empty() {
                        all_text.push(page_block);
                    }
                }
                Err(_) => {
                    // 空页也要记录 span 占位
                    page_spans.push((global_offset, global_offset));
                    continue;
                }
            }
        }

        let content = all_text.join("\n\n");

        let mut metadata = HashMap::new();
        metadata.insert("total_pages".to_string(), total_pages.to_string());
        // P2-004: 每页文本以 JSON 数组记录（含 page_number）
        metadata.insert(
            "page_texts".to_string(),
            serde_json::to_string(&page_texts).unwrap_or_default(),
        );
        // P2-006: 文本密度标记
        let needs_ocr = low_density_pages as f64 / total_pages.max(1) as f64 > 0.5;
        metadata.insert("needs_ocr".to_string(), needs_ocr.to_string());
        metadata.insert(
            "low_density_pages".to_string(),
            low_density_pages.to_string(),
        );

        // FIX9-04: 为每页 block 写入真实的 source span（基于 page_spans）
        let blocks: Vec<crate::kb::types::ParsedBlock> = page_texts
            .iter()
            .enumerate()
            .map(|(i, text)| {
                let (start, end) = page_spans.get(i).copied().unwrap_or((0, 0));
                crate::kb::types::ParsedBlock {
                    text: text.clone(),
                    title_path: String::new(),
                    page_number: Some((i + 1) as i32),
                    source_start: Some(start as i32),
                    source_end: Some(end as i32),
                    block_type: "page".to_string(),
                    metadata: String::new(),
                }
            })
            .collect();

        Ok(ParsedDocument {
            content,
            metadata,
            blocks: Some(blocks),
        })
    }

    fn extensions(&self) -> &[&str] {
        &["pdf"]
    }
}

fn clean_text(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut result = Vec::new();
    let mut prev_empty = false;

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !prev_empty {
                result.push(String::new());
                prev_empty = true;
            }
        } else {
            let normalized: String = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
            result.push(normalized);
            prev_empty = false;
        }
    }

    result.join("\n")
}

/// P2-005: 启发式去除页眉页脚 — 检测与前页重复的首/尾行
fn remove_header_footer(text: &str, previous_pages: &[String]) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= 2 || previous_pages.is_empty() {
        return text.to_string();
    }

    let first = lines[0].trim();
    let last = lines.last().map(|l| l.trim()).unwrap_or("");

    let mut remove_first = false;
    let mut remove_last = false;

    // 检查与先前页面的重复
    for prev in previous_pages.iter().rev().take(3) {
        let prev_lines: Vec<&str> = prev.lines().collect();
        if !prev_lines.is_empty() && prev_lines[0].trim() == first {
            remove_first = true;
        }
        if let Some(prev_last) = prev_lines.last() {
            if prev_last.trim() == last && last.chars().count() < 50 {
                remove_last = true;
            }
        }
    }

    // 检测纯数字行（页码）
    if first.chars().all(|c| c.is_ascii_digit()) {
        remove_first = true;
    }
    if last.chars().all(|c| c.is_ascii_digit()) {
        remove_last = true;
    }

    let range = if remove_first { 1 } else { 0 }..if remove_last {
        lines.len().saturating_sub(1)
    } else {
        lines.len()
    };

    lines[range].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_header_footer_repeat() {
        let page1 = "Chapter 1\nSome content\nPage 1";
        let page2 = "Chapter 1\nMore content\nPage 2";
        let previous = vec![page1.to_string()];
        let result = remove_header_footer(page2, &previous);
        assert!(!result.contains("Chapter 1"));
        assert!(result.contains("More content"));
        // 页码被移除（footer 变了所以不会被移除，因为不匹配 previous）
    }

    #[test]
    fn test_clean_text() {
        let text = "Hello   world\n\n   \nFoo bar";
        let result = clean_text(text);
        assert!(result.contains("Hello world"));
        assert!(result.contains("Foo bar"));
    }
}
