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
        let text_len = page.text.chars().count() as i32;
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
#[allow(clippy::too_many_arguments)]
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

    // FIX9-05: 为每个 chunk 通过 span overlap 匹配对应的 block 元数据，
    // 而非按 chunk 下标直接对应 page index。
    // 当一个 OCR 页被切成多个 chunk，或多个短页合并成一个 chunk 时，
    // 按 chunk 在全文中的真实位置与 block 的 source span 重叠度匹配。

    // FIX10-R1: 使用统一的 helper 定位 chunk 字符偏移，max_overlap 按 splitter 类型计算
    let max_overlap = crate::kb::pipeline::splitter_max_overlap(&splitter_config, false);
    let chunk_spans: Vec<(usize, usize)> =
        crate::kb::pipeline::locate_chunk_char_offsets(&full_text, &chunks, max_overlap);

    #[allow(clippy::type_complexity)]
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
            // FIX9-05: 用 span overlap 匹配最相关的 block，而非 block_meta_vec.get(i)
            let (chunk_start, chunk_end) = chunk_spans[i];
            let best_match = block_meta_vec
                .iter()
                .filter_map(|meta| {
                    let (title_path, page_num, src_start, src_end) = meta;
                    // 计算重叠长度
                    let bs = src_start.unwrap_or(0) as usize;
                    let be = src_end.unwrap_or(i32::MAX / 2) as usize;
                    let overlap_start = chunk_start.max(bs);
                    let overlap_end = chunk_end.min(be);
                    let overlap = overlap_end.saturating_sub(overlap_start);
                    if overlap > 0 {
                        Some((overlap, title_path, page_num, src_start, src_end))
                    } else {
                        None
                    }
                })
                .max_by_key(|(overlap, _, _, _, _)| *overlap);

            let (title_path, page_num, src_start, src_end) = best_match
                .map(|(_, tp, pn, ss, se)| (tp.clone(), *pn, *ss, *se))
                .unwrap_or_default();
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

    // 5. 持久化节点（OCR 路径无 run_id 守卫需求，传 None）
    persist_nodes_and_vectors(conn, doc_id, lib_id, &nodes, None)?;

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
    kb.update_document_stats(doc_id, word_total, nodes_created as i32, None)?;

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
        assert_eq!(blocks[1].source_start, Some(9)); // "Page one".chars().count() + 1(换行符) == 9
        assert_eq!(blocks[1].source_end, Some(17)); // 9 + "Page two".chars().count()
    }

    #[test]
    fn test_ocr_to_parsed_blocks_chinese() {
        let pages = vec![
            OcrPageResult {
                page_number: 1,
                text: "第一页内容".to_string(),
            },
            OcrPageResult {
                page_number: 2,
                text: "第二页内容".to_string(),
            },
        ];
        let blocks = ocr_to_parsed_blocks(&pages);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].source_start, Some(0));
        assert_eq!(blocks[0].source_end, Some(5)); // "第一页内容" = 5 chars
        assert_eq!(blocks[1].source_start, Some(6)); // 5 + 1(换行符)
        assert_eq!(blocks[1].source_end, Some(11)); // 6 + 5
    }

    #[test]
    fn test_ocr_status_as_str() {
        assert_eq!(OcrStatus::Done.as_str(), "done");
        assert_eq!(OcrStatus::Needed.as_str(), "needed");
        assert_eq!(OcrStatus::Failed.as_str(), "failed");
    }
}
