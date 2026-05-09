//! CSV parser

use super::{DocumentParser, ParsedDocument};
use crate::error::GBrainError;
use std::collections::HashMap;

pub struct CsvParser {
    delimiter: u8,
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
        let mut col_count = 0;

        for result in reader.records() {
            match result {
                Ok(record) => {
                    if col_count == 0 {
                        col_count = record.len();
                    }
                    rows.push(record.iter().collect::<Vec<_>>().join("\t"));
                }
                Err(_) => continue,
            }
        }

        let content = rows.join("\n");
        let mut metadata = HashMap::new();
        metadata.insert("row_count".to_string(), rows.len().to_string());
        metadata.insert("column_count".to_string(), col_count.to_string());

        Ok(ParsedDocument { content, metadata })
    }

    fn extensions(&self) -> &[&str] {
        &["csv"]
    }
}
