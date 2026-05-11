//! XLSX parser using calamine with structured sheet/row output (P2-012)

use super::{DocumentParser, ParsedDocument};
use crate::error::GBrainError;
use calamine::{open_workbook_from_rs, Data, Reader, Xlsx};
use std::collections::HashMap;

pub struct XlsxParser;

impl Default for XlsxParser {
    fn default() -> Self {
        Self::new()
    }
}

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
        let mut blocks: Vec<crate::kb::types::ParsedBlock> = Vec::new();

        for sheet_name in &sheets {
            let sheet_header = format!("=== {} ===", sheet_name);
            parts.push(sheet_header.clone());

            let mut headers: Vec<String> = Vec::new();
            let mut row_data: Vec<serde_json::Value> = Vec::new();
            let mut row_count: usize = 0;

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

                    // 第一行作为表头（仅在 headers 为空时）
                    if headers.is_empty() && !cells.is_empty() && row_count == 0 {
                        headers = cells.clone();
                    }

                    // 存储为 JSON object 而非 JSON string
                    let row_obj: serde_json::Value = serde_json::to_value(
                        headers
                            .iter()
                            .enumerate()
                            .map(|(i, h)| (h.clone(), cells.get(i).cloned().unwrap_or_default()))
                            .collect::<HashMap<_, _>>(),
                    )
                    .unwrap_or(serde_json::Value::Object(Default::default()));
                    row_data.push(row_obj);
                    parts.push(cells.join("\t"));
                    row_count += 1;
                }
            }

            // 每个 sheet 一个 ParsedBlock，metadata 包含完整 sheet 数据
            let sheet_meta = serde_json::json!({
                "name": sheet_name,
                "headers": headers,
                "row_count": row_count.saturating_sub(1) as i32,
                "rows": row_data,
            });
            blocks.push(crate::kb::types::ParsedBlock {
                text: parts[parts.len() - row_count - 1..].join("\n"),
                title_path: sheet_name.clone(),
                page_number: None,
                source_start: None,
                source_end: None,
                block_type: "table".to_string(),
                metadata: serde_json::to_string(&sheet_meta).unwrap_or_default(),
            });
        }

        let content = parts.join("\n");
        let mut metadata = HashMap::new();
        metadata.insert("sheet_count".to_string(), sheets.len().to_string());

        Ok(ParsedDocument {
            content,
            metadata,
            blocks: Some(blocks),
        })
    }

    fn extensions(&self) -> &[&str] {
        &["xlsx"]
    }
}
