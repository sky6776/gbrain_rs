//! HTML parser using html2text

use super::{DocumentParser, ParsedDocument};
use crate::error::GBrainError;
use std::collections::HashMap;

pub struct HtmlParser;

impl HtmlParser {
    pub fn new() -> Self {
        Self
    }
}

impl DocumentParser for HtmlParser {
    fn parse(&self, data: &[u8]) -> Result<ParsedDocument, GBrainError> {
        let text = html2text::from_read(data as &[u8], 80)
            .map_err(|e| GBrainError::FileError(format!("HTML parse failed: {}", e)))?;
        let content = clean_html_text(&text);
        Ok(ParsedDocument {
            content,
            metadata: HashMap::new(),
        })
    }

    fn extensions(&self) -> &[&str] {
        &["html", "htm"]
    }
}

fn clean_html_text(text: &str) -> String {
    text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}
