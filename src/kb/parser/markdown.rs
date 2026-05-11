//! Markdown parser (passthrough — structure handled by splitter)

use super::{DocumentParser, ParsedDocument};
use crate::error::GBrainError;
use std::collections::HashMap;

pub struct MarkdownParser;

impl Default for MarkdownParser {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkdownParser {
    pub fn new() -> Self {
        Self
    }
}

impl DocumentParser for MarkdownParser {
    fn parse(&self, data: &[u8]) -> Result<ParsedDocument, GBrainError> {
        let content = std::str::from_utf8(data)
            .map_err(|e| GBrainError::InvalidInput(format!("UTF-8 decode failed: {}", e)))?
            .to_string();

        Ok(ParsedDocument {
            content,
            metadata: HashMap::new(),
            blocks: None,
        })
    }

    fn extensions(&self) -> &[&str] {
        &["md"]
    }
}
