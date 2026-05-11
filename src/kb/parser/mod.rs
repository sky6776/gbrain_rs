//! Document parser registry

pub mod csv;
pub mod docx;
pub mod html;
pub mod markdown;
pub mod pdf;
pub mod text;
pub mod xlsx;

use crate::error::GBrainError;
use std::collections::HashMap;
use std::sync::Arc;

/// Parsed document result
#[derive(Debug, Clone)]
pub struct ParsedDocument {
    pub content: String,
    pub metadata: HashMap<String, String>,
    /// P1-010: 结构化 block 列表（含 title_path/page/source offsets/block_type）
    /// 解析器应尽量填充此字段；为 None 时 pipeline 回退到 content 字符串
    pub blocks: Option<Vec<crate::kb::types::ParsedBlock>>,
}

/// Document parser trait
pub trait DocumentParser: Send + Sync {
    fn parse(&self, data: &[u8]) -> Result<ParsedDocument, GBrainError>;
    fn extensions(&self) -> &[&str];
}

/// Parser registry: dispatches to the correct parser by extension
pub struct ParserRegistry {
    parsers: HashMap<String, Arc<dyn DocumentParser>>,
    fallback: text::TextParser,
}

impl Default for ParserRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ParserRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            parsers: HashMap::new(),
            fallback: text::TextParser::new(),
        };

        let parsers: Vec<Arc<dyn DocumentParser>> = vec![
            Arc::new(pdf::PdfParser::new()),
            Arc::new(docx::DocxParser::new()),
            Arc::new(xlsx::XlsxParser::new()),
            Arc::new(csv::CsvParser::new()),
            Arc::new(html::HtmlParser::new()),
            Arc::new(markdown::MarkdownParser::new()),
        ];

        for parser in parsers {
            for ext in parser.extensions() {
                registry
                    .parsers
                    .insert(ext.to_string(), Arc::clone(&parser));
            }
        }

        registry
    }

    pub fn parse(&self, ext: &str, data: &[u8]) -> Result<ParsedDocument, GBrainError> {
        let key = ext.to_lowercase();
        match self.parsers.get(&key) {
            Some(p) => p.parse(data),
            None => self.fallback.parse(data),
        }
    }
}
