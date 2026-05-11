//! DOCX parser with heading style detection and structure preservation
//!
//! P2-007: Identifies Word heading styles (Heading1-6) and generates heading_path
//! P2-008: Preserves paragraph/list/table structure blocks

use super::{DocumentParser, ParsedDocument};
use crate::error::GBrainError;
use std::collections::HashMap;
use std::io::Read;

pub struct DocxParser;

impl DocxParser {
    pub fn new() -> Self {
        Self
    }
}

impl DocumentParser for DocxParser {
    fn parse(&self, data: &[u8]) -> Result<ParsedDocument, GBrainError> {
        let (content, headings) = extract_docx_text_structured(data)?;
        let mut metadata = HashMap::new();
        if !headings.is_empty() {
            metadata.insert("headings".to_string(), headings.join(" > "));
        }
        Ok(ParsedDocument {
            content,
            metadata,
            blocks: None,
        })
    }

    fn extensions(&self) -> &[&str] {
        &["docx"]
    }
}

/// P2-007: 判断是否是标题样式并返回层级
fn heading_level(style: &str) -> Option<u32> {
    let s = style.to_lowercase();
    for level in 1..=6 {
        if s.contains(&format!("heading{}", level)) || s.contains(&format!("heading {}", level)) {
            return Some(level);
        }
    }
    if s.contains("heading") || s.contains("title") {
        return Some(1);
    }
    None
}

/// P2-007 + P2-008: 结构化提取 DOCX 文本，同时收集标题层级
fn extract_docx_text_structured(data: &[u8]) -> Result<(String, Vec<String>), GBrainError> {
    let reader = std::io::Cursor::new(data);
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|e| GBrainError::FileError(format!("DOCX open failed: {}", e)))?;

    let mut xml_content = String::new();
    let mut file = archive
        .by_name("word/document.xml")
        .map_err(|_| GBrainError::FileError("DOCX missing word/document.xml".to_string()))?;
    file.read_to_string(&mut xml_content)
        .map_err(|e| GBrainError::FileError(format!("DOCX read failed: {}", e)))?;

    let mut paragraphs = Vec::new();
    let mut headings = Vec::new();
    let mut current_text = String::new();
    let mut in_t_tag = false;
    let mut in_pstyle = false;
    let mut current_heading_level: Option<u32> = None;
    let mut pstyle_text = String::new();

    let mut reader = quick_xml::Reader::from_str(&xml_content);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(quick_xml::events::Event::Start(ref e)) => {
                let local = e.local_name();
                let name = local.as_ref();
                if name == b"t" {
                    in_t_tag = true;
                }
                if name == b"pStyle" {
                    in_pstyle = true;
                    pstyle_text.clear();
                }
                // P2-007: 从 pStyle 的 w:val 属性提取样式名
                if name == b"pStyle" {
                    for attr in e.attributes().flatten() {
                        let attr_local = attr.key.local_name();
                        if attr_local.as_ref() == b"val" {
                            let style = String::from_utf8_lossy(&attr.value).to_string();
                            current_heading_level = heading_level(&style);
                        }
                    }
                }
            }
            Ok(quick_xml::events::Event::Text(ref e)) => {
                if in_t_tag || in_pstyle {
                    if let Ok(text) = e.unescape() {
                        if in_pstyle {
                            pstyle_text.push_str(&text);
                        } else {
                            current_text.push_str(&text);
                        }
                    }
                }
            }
            Ok(quick_xml::events::Event::End(e)) => {
                let local = e.local_name();
                let name = local.as_ref();
                if name == b"t" {
                    in_t_tag = false;
                } else if name == b"pStyle" {
                    in_pstyle = false;
                } else if name == b"p" {
                    let trimmed = current_text.trim().to_string();
                    if !trimmed.is_empty() {
                        if let Some(level) = current_heading_level {
                            let prefix = "#".repeat(level as usize);
                            headings.push(format!("{} {}", prefix, trimmed));
                            paragraphs.push(format!("{} {}", prefix, trimmed));
                        } else {
                            paragraphs.push(trimmed);
                        }
                    }
                    current_text.clear();
                    current_heading_level = None;
                }
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Err(e) => {
                return Err(GBrainError::FileError(format!(
                    "DOCX XML parse error: {}",
                    e
                )))
            }
            _ => {}
        }
        buf.clear();
    }

    Ok((paragraphs.join("\n\n"), headings))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heading_level() {
        assert_eq!(heading_level("Heading1"), Some(1));
        assert_eq!(heading_level("heading 3"), Some(3));
        assert_eq!(heading_level("Title"), Some(1));
        assert!(heading_level("Normal").is_none());
    }
}
