//! PDF parser using lopdf

use super::{DocumentParser, ParsedDocument};
use crate::error::GBrainError;
use std::collections::HashMap;

pub struct PdfParser;

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
        let mut text_parts = Vec::new();
        let total_pages = pages.len();

        // extract_text takes page numbers (u32), not ObjectIds
        let page_numbers: Vec<u32> = pages.keys().copied().collect();

        for page_num in &page_numbers {
            match pdf.extract_text(&[*page_num]) {
                Ok(text) => {
                    let cleaned = clean_text(&text);
                    if !cleaned.is_empty() {
                        text_parts.push(cleaned);
                    }
                }
                Err(_) => continue,
            }
        }

        let content = text_parts.join("\n\n");
        let mut metadata = HashMap::new();
        metadata.insert("total_pages".to_string(), total_pages.to_string());

        Ok(ParsedDocument { content, metadata })
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
