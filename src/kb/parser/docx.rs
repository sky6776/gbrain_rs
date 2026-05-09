//! DOCX parser using zip + quick-xml

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
        let content = extract_docx_text(data)?;
        Ok(ParsedDocument {
            content,
            metadata: HashMap::new(),
        })
    }

    fn extensions(&self) -> &[&str] {
        &["docx"]
    }
}

fn extract_docx_text(data: &[u8]) -> Result<String, GBrainError> {
    let reader = std::io::Cursor::new(data);
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|e| GBrainError::FileError(format!("DOCX open failed: {}", e)))?;

    let mut xml_content = String::new();
    match archive.by_name("word/document.xml") {
        Ok(mut file) => {
            file.read_to_string(&mut xml_content)
                .map_err(|e| GBrainError::FileError(format!("DOCX read failed: {}", e)))?;
        }
        Err(_) => {
            return Err(GBrainError::FileError(
                "DOCX missing word/document.xml".to_string(),
            ))
        }
    }

    // Parse XML and extract <w:t> text
    let mut paragraphs = Vec::new();
    let mut current_text = String::new();
    let mut in_t_tag = false;

    let mut reader = quick_xml::Reader::from_str(&xml_content);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(quick_xml::events::Event::Start(e)) => {
                let local = e.local_name();
                let name = local.as_ref();
                if name == b"t" {
                    in_t_tag = true;
                } else if name == b"p" {
                    if !current_text.trim().is_empty() {
                        paragraphs.push(current_text.trim().to_string());
                    }
                    current_text = String::new();
                }
            }
            Ok(quick_xml::events::Event::Empty(_e)) => {}
            Ok(quick_xml::events::Event::Text(e)) => {
                if in_t_tag {
                    if let Ok(text) = e.unescape() {
                        current_text.push_str(&text);
                    }
                }
            }
            Ok(quick_xml::events::Event::End(e)) => {
                let local = e.local_name();
                let name = local.as_ref();
                if name == b"t" {
                    in_t_tag = false;
                } else if name == b"p" {
                    if !current_text.trim().is_empty() {
                        paragraphs.push(current_text.trim().to_string());
                    }
                    current_text = String::new();
                }
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Err(e) => {
                return Err(GBrainError::FileError(format!(
                    "DOCX XML parse error: {}",
                    e
                )));
            }
            _ => {}
        }
        buf.clear();
    }

    if !current_text.trim().is_empty() {
        paragraphs.push(current_text.trim().to_string());
    }

    Ok(paragraphs.join("\n\n"))
}
