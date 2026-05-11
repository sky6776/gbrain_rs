//! OCR 可选模块 (P2-017~P2-019)
//!
//! 为扫描型 PDF 提供 OCR job schema 和状态管理。
//! P2-019: OCR 结果回写 — 将 OCR 识别文本转换为 ParsedBlock 并写入节点/段。

use crate::error::{GBrainError, Result};
use crate::kb::context;
use crate::kb::engine::KbEngine;
use crate::kb::pipeline::persist_nodes_and_vectors;
use crate::kb::types::*;
use crate::nlp::chinese;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// OCR processing status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OcrStatus {
    NotNeeded,
    Needed,
    Queued,
    Processing,
    Done,
    Failed,
}

impl OcrStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotNeeded => "not_needed",
            Self::Needed => "needed",
            Self::Queued => "queued",
            Self::Processing => "processing",
            Self::Done => "done",
            Self::Failed => "failed",
        }
    }
}

/// OCR 是否可通过配置启用
pub fn is_ocr_enabled(external_ocr_allowed: bool, global_ocr_enabled: bool) -> bool {
    external_ocr_allowed && global_ocr_enabled
}

/// 判断是否需要 OCR（基于文本密度）
pub fn needs_ocr(text_density: f64, threshold: f64) -> bool {
    text_density < threshold
}

// ---------------------------------------------------------------------------
// P2-019: OCR 结果回写
// ---------------------------------------------------------------------------

/// 单页 OCR 识别结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrPageResult {
    /// 页码（1-based）
    pub page_number: i32,
    /// 该页 OCR 识别出的文本
    pub text: String,
}

/// P2-019: 将 OCR 结果转换为标准 ParsedBlock 列表。
///
/// 每个 `OcrPageResult` 生成一个 `ParsedBlock`，block_type 为 "ocr_text"，
/// 携带 page_number 和基于累计字符偏移的 source_start/source_end。
pub fn ocr_to_parsed_blocks(ocr_pages: &[OcrPageResult]) -> Vec<ParsedBlock> {
    let mut blocks = Vec::with_capacity(ocr_pages.len());
    let mut offset = 0i32;

    for page in ocr_pages {
        let text_len = page.text.len() as i32;
        blocks.push(ParsedBlock {
            text: page.text.clone(),
            title_path: String::new(),
            page_number: Some(page.page_number),
            source_start: Some(offset),
            source_end: Some(offset + text_len),
            block_type: "ocr_text".to_string(),
            metadata: String::new(),
        });
        offset += text_len + 1; // +1 为页间换行符
    }

    blocks
}

/// P2-019: OCR 结果回写主函数。
///
/// 接收 OCR 识别结果，将其转换为 ParsedBlock，再分割为 RaptorNode，
/// 持久化到数据库，并更新文档的 OCR 状态和文本覆盖率。
///
/// 流程：
/// 1. 将 OCR 结果转为 ParsedBlock 列表
/// 2. 拼接全文，按配置分割为 chunks
/// 3. 为每个 chunk 构建 RaptorNode（含 page_number / source offsets）
/// 4. 持久化节点
/// 5. 更新文档 ocr_status = "done" 和 ocr_text_coverage
///
/// 注意：此函数不执行 OCR 识别本身，仅处理识别后的文本回写。
/// 调用方应在 OCR 完成后调用此函数。
pub fn writeback_ocr_results(
    conn: &Connection,
    doc_id: i64,
    lib_id: i64,
    ocr_pages: &[OcrPageResult],
    chunk_size: usize,
    chunk_overlap: usize,
    doc_title: &str,
    total_page_count: i32,
) -> Result<WritebackResult> {
    if ocr_pages.is_empty() {
        return Ok(WritebackResult {
            blocks_created: 0,
            nodes_created: 0,
            ocr_text_coverage: 0.0,
        });
    }

    // 1. 转换为 ParsedBlock
    let blocks = ocr_to_parsed_blocks(ocr_pages);

    // 2. 拼接全文并分割
    let full_text: String = blocks
        .iter()
        .map(|b| b.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    if full_text.trim().is_empty() {
        return Ok(WritebackResult {
            blocks_created: blocks.len(),
            nodes_created: 0,
            ocr_text_coverage: 0.0,
        });
    }

    let splitter_config = crate::kb::splitter::SplitterConfig {
        file_path: String::new(),
        chunk_size,
        chunk_overlap,
        semantic_enabled: false,
    };
    let splitter = crate::kb::splitter::create_splitter(&splitter_config);
    let chunks = splitter
        .split(&full_text)
        .map_err(|e| GBrainError::InvalidInput(format!("OCR 回写分割失败: {}", e)))?;

    // 3. 为每个 chunk 找到对应的 block 元数据（page_number, source offsets）
    let block_meta_vec: Vec<(String, Option<i32>, Option<i32>, Option<i32>)> = blocks
        .iter()
        .map(|b| {
            (
                b.title_path.clone(),
                b.page_number,
                b.source_start,
                b.source_end,
            )
        })
        .collect();

    // 4. 构建 RaptorNode 列表
    let nodes: Vec<RaptorNode> = chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| {
            let (title_path, page_num, src_start, src_end) =
                block_meta_vec.get(i).cloned().unwrap_or_default();
            let embedding_text =
                context::build_embedding_text(doc_title, &title_path, page_num, chunk);
            RaptorNode {
                id: -((i as i64) + 1),
                library_id: lib_id,
                document_id: doc_id,
                content: chunk.clone(),
                level: 0,
                parent_id: None,
                chunk_order: i as i32,
                vector: None,
                title_path,
                page_number: page_num,
                source_start: src_start,
                source_end: src_end,
                node_metadata: String::new(),
                embedding_text,
            }
        })
        .collect();

    let nodes_created = nodes.len();

    // 5. 持久化节点
    persist_nodes_and_vectors(conn, doc_id, lib_id, &nodes)?;

    // 6. 计算 ocr_text_coverage
    let ocr_pages_with_text = ocr_pages
        .iter()
        .filter(|p| !p.text.trim().is_empty())
        .count() as i32;
    let coverage = if total_page_count > 0 {
        ocr_pages_with_text as f64 / total_page_count as f64
    } else {
        0.0
    };

    // 7. 更新文档 OCR 状态
    let kb = KbEngine::new(conn);
    kb.update_document_ocr(doc_id, OcrStatus::Done.as_str(), coverage)?;

    // 8. 更新文档统计（词数/分块数）
    let word_total: i32 = if chinese::has_chinese(&full_text) {
        let tokens = chinese::tokenize_content(&full_text);
        tokens.split_whitespace().count() as i32
    } else {
        full_text.split_whitespace().count() as i32
    };
    kb.update_document_stats(doc_id, word_total, nodes_created as i32)?;

    Ok(WritebackResult {
        blocks_created: blocks.len(),
        nodes_created,
        ocr_text_coverage: coverage,
    })
}

/// OCR 回写结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WritebackResult {
    /// 创建的 ParsedBlock 数量
    pub blocks_created: usize,
    /// 创建的 RaptorNode 数量
    pub nodes_created: usize,
    /// OCR 文本覆盖率（0.0~1.0）
    pub ocr_text_coverage: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ocr_disabled() {
        assert!(!is_ocr_enabled(false, true));
        assert!(!is_ocr_enabled(true, false));
        assert!(is_ocr_enabled(true, true));
    }

    #[test]
    fn test_needs_ocr() {
        assert!(needs_ocr(0.01, 0.05));
        assert!(!needs_ocr(0.1, 0.05));
    }

    #[test]
    fn test_ocr_to_parsed_blocks_empty() {
        let blocks = ocr_to_parsed_blocks(&[]);
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_ocr_to_parsed_blocks_single_page() {
        let pages = vec![OcrPageResult {
            page_number: 1,
            text: "Hello world".to_string(),
        }];
        let blocks = ocr_to_parsed_blocks(&pages);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].text, "Hello world");
        assert_eq!(blocks[0].page_number, Some(1));
        assert_eq!(blocks[0].block_type, "ocr_text");
        assert_eq!(blocks[0].source_start, Some(0));
        assert_eq!(blocks[0].source_end, Some(11));
    }

    #[test]
    fn test_ocr_to_parsed_blocks_multi_page() {
        let pages = vec![
            OcrPageResult {
                page_number: 1,
                text: "Page one".to_string(),
            },
            OcrPageResult {
                page_number: 2,
                text: "Page two".to_string(),
            },
        ];
        let blocks = ocr_to_parsed_blocks(&pages);
        assert_eq!(blocks.len(), 2);
        // Second block offset starts after first page text
        assert_eq!(blocks[0].source_start, Some(0));
        assert_eq!(blocks[1].source_start, Some(10)); // "Page one".len() + 1(换行符) == 10
        assert_eq!(blocks[1].source_end, Some(19)); // 10 + "Page two".len()
    }

    #[test]
    fn test_ocr_status_as_str() {
        assert_eq!(OcrStatus::Done.as_str(), "done");
        assert_eq!(OcrStatus::Needed.as_str(), "needed");
        assert_eq!(OcrStatus::Failed.as_str(), "failed");
    }
}
