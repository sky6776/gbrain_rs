//! XLSX parser using calamine with structured sheet/row output (P2-012)

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
        let mut sheet_metas: Vec<HashMap<String, serde_json::Value>> = Vec::new();

        for sheet_name in &sheets {
            parts.push(format!("=== {} ===", sheet_name));

            let mut headers: Vec<String> = Vec::new();
            let mut row_data: Vec<String> = Vec::new();
            let mut row_count = 0;

            if let Ok(range) = workbook.worksheet_range(sheet_name) {
                for row in range.rows() {
                    let cells: Vec<String> = row.iter().map(|cell| match cell {
                        Data::String(s) => s.clone(),
                        Data::Int(i) => i.to_string(),
                        Data::Float(f) => f.to_string(),
                        Data::Bool(b) => b.to_string(),
                        Data::DateTime(dt) => dt.to_string(),
                        _ => String::new(),
                    }).collect();

                    // 第一行作为表头（仅在 headers 为空时）
                    if headers.is_empty() && !cells.is_empty() && row_count == 0 {
                        headers = cells.clone();
                    }

                    let row_json = serde_json::to_string(
                        &headers.iter().enumerate().map(|(i, h)| {
                            (h.clone(), cells.get(i).cloned().unwrap_or_default())
                        }).collect::<HashMap<_, _>>()
                    ).unwrap_or_default();
                    row_data.push(row_json);
                    parts.push(cells.join("\t"));
                    row_count += 1;
                }
            }

            sheet_metas.push({
                let mut m = HashMap::new();
                m.insert("name".to_string(), serde_json::Value::String(sheet_name.clone()));
                m.insert("headers".to_string(), serde_json::Value::Array(
                    headers.iter().map(|h| serde_json::Value::String(h.clone())).collect()
                ));
                m.insert("row_count".to_string(), serde_json::Value::Number((row_count - 1).into()));
                m.insert("rows".to_string(), serde_json::to_value(&row_data).unwrap_or_default());
                m
            });
        }

        let content = parts.join("\n");
        let mut metadata = HashMap::new();
        metadata.insert("sheet_count".to_string(), sheets.len().to_string());
        metadata.insert("sheet_data".to_string(),
            serde_json::to_string(&sheet_metas).unwrap_or_default());

        Ok(ParsedDocument { content, metadata })
    }

    fn extensions(&self) -> &[&str] {
        &["xlsx"]
    }
}
