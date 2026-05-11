//! CSV parser with structured row output (P2-011)

use super::{DocumentParser, ParsedDocument};
use crate::error::GBrainError;
use std::collections::HashMap;

pub struct CsvParser {
    delimiter: u8,
}

impl Default for CsvParser {
    fn default() -> Self {
        Self::new()
    }
}

impl CsvParser {
    pub fn new() -> Self {
        Self { delimiter: b',' }
    }
}

impl DocumentParser for CsvParser {
    fn parse(&self, data: &[u8]) -> Result<ParsedDocument, GBrainError> {
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(self.delimiter)
            .flexible(true)
            .from_reader(data);

        let mut rows = Vec::new();
        let mut headers: Vec<String> = Vec::new();
        let mut row_records: Vec<String> = Vec::new();

        for result in reader.records() {
            match result {
                Ok(record) => {
                    let fields: Vec<String> = record.iter().map(|s| s.to_string()).collect();
                    if headers.is_empty() && !fields.is_empty() {
                        headers = fields.clone();
                    }
                    let row_json = serde_json::to_string(
                        &headers
                            .iter()
                            .enumerate()
                            .map(|(i, h)| (h.clone(), fields.get(i).cloned().unwrap_or_default()))
                            .collect::<HashMap<_, _>>(),
                    )
                    .unwrap_or_default();
                    rows.push(fields.join("\t"));
                    row_records.push(row_json);
                }
                Err(_) => continue,
            }
        }

        let content = rows.join("\n");
        let mut metadata = HashMap::new();
        metadata.insert("row_count".to_string(), rows.len().to_string());
        metadata.insert("column_count".to_string(), headers.len().to_string());
        if !headers.is_empty() {
            metadata.insert("headers".to_string(), headers.join(", "));
            metadata.insert(
                "row_json_list".to_string(),
                serde_json::to_string(&row_records).unwrap_or_default(),
            );
        }

        Ok(ParsedDocument {
            content,
            metadata,
            blocks: None,
        })
    }

    fn extensions(&self) -> &[&str] {
        &["csv"]
    }
}
