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
        // CSV 行数上限，防止超大文件耗尽内存
        const MAX_CSV_ROWS: usize = 100_000;
        // C4 fix: 存储 serde_json::Value 而非预序列化的 JSON 字符串，
        // 避免最终 serde_json::to_string(&row_records) 产生双重转义。
        let mut row_records: Vec<serde_json::Value> = Vec::new();

        for result in reader.records() {
            match result {
                Ok(record) => {
                    if rows.len() >= MAX_CSV_ROWS {
                        return Err(GBrainError::InvalidInput(format!(
                            "CSV 行数超过上限 {}，请拆分文件后重试",
                            MAX_CSV_ROWS
                        )));
                    }
                    let fields: Vec<String> = record.iter().map(|s| s.to_string()).collect();
                    if headers.is_empty() && !fields.is_empty() {
                        headers = fields.clone();
                    }
                    let row_map: HashMap<String, String> = headers
                        .iter()
                        .enumerate()
                        .map(|(i, h)| (h.clone(), fields.get(i).cloned().unwrap_or_default()))
                        .collect();
                    // 直接存为 Value，不再提前序列化
                    row_records.push(serde_json::to_value(&row_map).expect("JSON 序列化不应失败: HashMap<String, String> 始终可序列化"));
                    rows.push(fields.join("\t"));
                }
                Err(e) => {
                    // M50: 畸形 CSV 行跳过时记录 debug 日志，便于排查数据质量问题
                    tracing::debug!(error = %e, "CSV 行解析错误，跳过");
                    continue;
                }
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
