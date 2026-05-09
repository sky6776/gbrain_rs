//! XLSX parser using calamine

use super::{DocumentParser, ParsedDocument};
use crate::error::GBrainError;
use calamine::{open_workbook_from_rs, Data, Reader, Xlsx};
use std::collections::HashMap;

pub struct XlsxParser;

impl XlsxParser {
    pub fn new() -> Self {
        Self
    }
}

impl DocumentParser for XlsxParser {
    fn parse(&self, data: &[u8]) -> Result<ParsedDocument, GBrainError> {
        let cursor = std::io::Cursor::new(data);
        let mut workbook: Xlsx<_> = open_workbook_from_rs(cursor)
            .map_err(|e| GBrainError::FileError(format!("XLSX open failed: {}", e)))?;

        let sheets = workbook.sheet_names().to_vec();
        let mut parts = Vec::new();

        for sheet_name in &sheets {
            parts.push(format!("=== {} ===", sheet_name));

            if let Ok(range) = workbook.worksheet_range(sheet_name) {
                for row in range.rows() {
                    let cells: Vec<String> = row
                        .iter()
                        .map(|cell| match cell {
                            Data::String(s) => s.clone(),
                            Data::Int(i) => i.to_string(),
                            Data::Float(f) => f.to_string(),
                            Data::Bool(b) => b.to_string(),
                            Data::DateTime(dt) => dt.to_string(),
                            _ => String::new(),
                        })
                        .collect();
                    parts.push(cells.join("\t"));
                }
            }
        }

        let content = parts.join("\n");
        let mut metadata = HashMap::new();
        metadata.insert("sheet_count".to_string(), sheets.len().to_string());

        Ok(ParsedDocument { content, metadata })
    }

    fn extensions(&self) -> &[&str] {
        &["xlsx"]
    }
}
