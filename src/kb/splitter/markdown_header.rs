//! Markdown header splitter using pulldown-cmark

use super::{Chunks, DocumentSplitter};
use crate::error::GBrainError;
use pulldown_cmark::{Event, Parser, Tag, TagEnd};

pub struct MarkdownHeaderSplitter;

impl MarkdownHeaderSplitter {
    pub fn new() -> Self {
        Self
    }
}

impl DocumentSplitter for MarkdownHeaderSplitter {
    fn split(&self, text: &str) -> Result<Chunks, GBrainError> {
        let mut chunks = Vec::new();
        let mut current_section = String::new();

        let parser = Parser::new(text);

        for event in parser {
            match event {
                Event::Start(Tag::Heading { level, .. }) => {
                    if !current_section.trim().is_empty() {
                        chunks.push(current_section.trim().to_string());
                    }
                    current_section = String::new();
                    // Add heading marker
                    let prefix = "#".repeat(level as usize);
                    current_section.push_str(&prefix);
                    current_section.push(' ');
                }
                Event::End(TagEnd::Heading(_)) => {
                    current_section.push('\n');
                }
                Event::Text(t) => {
                    current_section.push_str(&t);
                }
                Event::Code(code) => {
                    current_section.push_str("```\n");
                    current_section.push_str(&code);
                    current_section.push_str("\n```\n");
                }
                Event::SoftBreak | Event::HardBreak => {
                    current_section.push('\n');
                }
                _ => {}
            }
        }

        if !current_section.trim().is_empty() {
            chunks.push(current_section.trim().to_string());
        }

        if chunks.is_empty() && !text.trim().is_empty() {
            chunks.push(text.trim().to_string());
        }

        Ok(chunks)
    }
}
