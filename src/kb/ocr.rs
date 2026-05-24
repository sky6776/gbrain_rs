//! OCR 模块 — 状态管理、OCR 结果回写与页级/块级持久化
//!
//! Phase 1~4: 支持 GLM-OCR 版面识别、页级状态、块级融合、异步 job。

use crate::error::{GBrainError, Result};
use crate::kb::context;
use crate::kb::engine::KbEngine;
use crate::kb::pipeline::persist_nodes_and_vectors;
use crate::kb::types::*;
use crate::nlp::chinese;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// 检查 OCR 写入的 run guard：比较文档当前的 processing_run_id 与期望值。
/// 不匹配时返回 Err，调用方可据此跳过写入，避免 stale job 污染新 run 的数据。
pub fn check_ocr_run_guard(
    conn: &Connection,
    document_id: i64,
    expected_run_id: &str,
) -> Result<()> {
    let current_run: String = conn
        .query_row(
            "SELECT processing_run_id FROM kb_documents WHERE id = ?1",
            rusqlite::params![document_id],
            |row| row.get(0),
        )
        .unwrap_or_default();
    if current_run != expected_run_id {
        return Err(GBrainError::InvalidInput(format!(
            "OCR run guard 失败: 文档 {} 当前 run={}, 期望 run={}",
            document_id, current_run, expected_run_id
        )));
    }
    Ok(())
}

/// OCR 文档级处理状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OcrStatus {
    /// 不需要 OCR（纯文本 PDF）
    NotNeeded,
    /// 需要 OCR，但 OCR 或外部 OCR 被显式关闭，或等待排队
    Needed,
    /// OCR job 已入队
    Queued,
    /// OCR 正在执行
    Processing,
    /// OCR 完成并已进入 KB 索引
    Done,
    /// 部分页成功，部分页失败
    Partial,
    /// OCR 完全失败
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
            Self::Partial => "partial",
            Self::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "not_needed" => Self::NotNeeded,
            "needed" => Self::Needed,
            "queued" => Self::Queued,
            "processing" => Self::Processing,
            "done" => Self::Done,
            "partial" => Self::Partial,
            "failed" => Self::Failed,
            _ => Self::NotNeeded,
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
// Phase 3: 页级/块级 OCR 结果持久化
// ---------------------------------------------------------------------------

/// 将 OCR 页级结果写入 kb_document_ocr_pages 表
/// `run_id` 用于 run 隔离：UNIQUE 约束包含 (document_id, page_number, processing_run_id)，
/// 不同 run 的页级结果互不干扰。
pub fn persist_ocr_page_results(
    conn: &Connection,
    document_id: i64,
    run_id: &str,
    page_results: &[crate::kb::ocr_provider::OcrPageResult],
) -> Result<()> {
    for page in page_results {
        // 检查是否为携带错误信息的失败页（单页降级重试部分失败场景）
        let (status, error_msg) = if page.raw_response_json.get("_ocr_failed").is_some() {
            let err = page
                .raw_response_json
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("OCR 失败")
                .to_string();
            ("failed", Some(err))
        } else if page.text.trim().is_empty() && page.markdown.trim().is_empty() {
            ("empty_ocr", None)
        } else {
            ("done", None)
        };

        let layout_json = serde_json::to_string(&page.blocks).unwrap_or_default();

        conn.execute(
            "INSERT OR REPLACE INTO kb_document_ocr_pages \
             (document_id, page_number, processing_run_id, status, error, provider, model, text, markdown, \
              layout_json, layout_visualization_url, raw_response_json, request_id, \
              confidence, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, datetime('now'))",
            rusqlite::params![
                document_id,
                page.page_number,
                run_id,
                status,
                error_msg.unwrap_or_default(),
                page.provider,
                page.model,
                page.text,
                page.markdown,
                layout_json,
                page.layout_visualization_url.as_deref().unwrap_or(""),
                page.raw_response_json.to_string(),
                page.request_id.as_deref().unwrap_or(""),
                page.confidence,
            ],
        )?;
    }
    Ok(())
}

/// 将 OCR 版面块写入 kb_document_ocr_blocks 表
/// `run_id` 用于 run 隔离：仅删除和插入同一 run 的 blocks，不影响其他 run 的数据。
pub fn persist_ocr_blocks(
    conn: &Connection,
    document_id: i64,
    run_id: &str,
    page_results: &[crate::kb::ocr_provider::OcrPageResult],
) -> Result<()> {
    // 先删除每页同一 run 的旧 blocks，防止重试后残留高序号旧 block
    for page in page_results {
        conn.execute(
            "DELETE FROM kb_document_ocr_blocks WHERE document_id = ?1 AND page_number = ?2 AND processing_run_id = ?3",
            rusqlite::params![document_id, page.page_number, run_id],
        )?;
    }

    for page in page_results {
        for (block_idx, block) in page.blocks.iter().enumerate() {
            let bbox_json = block
                .bbox_2d
                .map(|b| serde_json::to_string(&b).unwrap_or_default())
                .unwrap_or_default();

            // 生成 plain_text：text/formula 直接用 content，table 去 HTML 标签
            let plain_text = match &block.label {
                crate::kb::ocr_provider::OcrBlockLabel::Table => strip_html_tags(&block.content),
                crate::kb::ocr_provider::OcrBlockLabel::Image => String::new(),
                _ => block.content.clone(),
            };

            let raw_json = serde_json::json!({
                "page_number": block.page_number,
                "index": block.index,
                "label": block.label.as_str(),
                "width": block.width,
                "height": block.height,
            });

            conn.execute(
                "INSERT OR REPLACE INTO kb_document_ocr_blocks \
                 (document_id, page_number, processing_run_id, block_index, label, bbox_json, \
                  content, plain_text, source, raw_json) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'glm_ocr', ?9)",
                rusqlite::params![
                    document_id,
                    block.page_number,
                    run_id,
                    block_idx as i32,
                    block.label.as_str(),
                    bbox_json,
                    block.content,
                    plain_text,
                    raw_json.to_string(),
                ],
            )?;
        }
    }
    Ok(())
}

/// 更新页级 OCR 状态（用于标记失败/跳过页）
///
/// 使用 INSERT OR REPLACE 确保即使页级记录尚不存在也能写入状态，
/// 防止 OCR 完全失败时无任何页级行，导致 update_document_ocr_status
/// 将 total_ocr_pages==0 误判为 NotNeeded。
///
/// `provider` / `model` 记录实际使用（或意图使用）的 OCR 提供者和模型，
/// 用于失败诊断和审计。API 错误、超时、拆分失败等场景均应传入。
pub fn update_ocr_page_status(
    conn: &Connection,
    document_id: i64,
    page_number: i32,
    status: &str,
    error: &str,
    provider: &str,
    model: &str,
    run_id: &str,
) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO kb_document_ocr_pages \
         (document_id, page_number, processing_run_id, status, error, provider, model, text, markdown, \
          layout_json, layout_visualization_url, raw_response_json, request_id, \
          confidence, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, '', '', '[]', '', '{}', '', NULL, datetime('now'))",
        rusqlite::params![document_id, page_number, run_id, status, error, provider, model],
    )?;
    Ok(())
}

pub fn update_ocr_pages_status(
    conn: &Connection,
    document_id: i64,
    pages: &[i32],
    status: &str,
    error: &str,
    provider: &str,
    model: &str,
    run_id: &str,
) -> Result<()> {
    for &page_number in pages {
        update_ocr_page_status(
            conn,
            document_id,
            page_number,
            status,
            error,
            provider,
            model,
            run_id,
        )?;
    }
    Ok(())
}

/// HTML 标签匹配正则（全局编译一次，避免热路径重复编译）
static HTML_TAG_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"<[^>]+>").unwrap());

/// 简单去掉 HTML 标签
fn strip_html_tags(html: &str) -> String {
    HTML_TAG_RE.replace_all(html, "").trim().to_string()
}

/// 计算并更新文档级 OCR 状态和文本覆盖率。
/// `run_id` 用于双重保障：(1) 聚合查询按 run 过滤，避免统计旧 run 的页记录；
/// (2) 文档状态更新使用 run guard，防止 stale job 覆盖新 run 的状态。
pub fn update_document_ocr_status(
    conn: &Connection,
    document_id: i64,
    total_pages: i32,
    run_id: Option<&str>,
) -> Result<OcrStatus> {
    // 辅助函数：按 run_id 过滤统计指定状态的页数
    let count_by_status = |status: &str| -> i32 {
        if let Some(rid) = run_id {
            conn.query_row(
                "SELECT COUNT(*) FROM kb_document_ocr_pages \
                 WHERE document_id = ?1 AND status = ?2 AND processing_run_id = ?3",
                rusqlite::params![document_id, status, rid],
                |row| row.get(0),
            )
            .unwrap_or(0)
        } else {
            conn.query_row(
                "SELECT COUNT(*) FROM kb_document_ocr_pages \
                 WHERE document_id = ?1 AND status = ?2",
                rusqlite::params![document_id, status],
                |row| row.get(0),
            )
            .unwrap_or(0)
        }
    };

    // 统计各状态页数
    let done_count = count_by_status("done");
    let failed_count = count_by_status("failed");
    // 统计 empty_ocr 页（OCR 已执行但内容为空，视为识别失败）
    let empty_ocr_count = count_by_status("empty_ocr");
    // 统计 skipped 页（超出上限被跳过，不应算为完成）
    let skipped_count = count_by_status("skipped");
    // 统计 needed 页（策略阻断：OCR 仍需要但被库隐私策略阻止）
    let needed_count = count_by_status("needed");

    let total_ocr_pages: i32 = if let Some(rid) = run_id {
        conn.query_row(
            "SELECT COUNT(*) FROM kb_document_ocr_pages WHERE document_id = ?1 AND processing_run_id = ?2",
            rusqlite::params![document_id, rid],
            |row| row.get(0),
        )
        .unwrap_or(0)
    } else {
        conn.query_row(
            "SELECT COUNT(*) FROM kb_document_ocr_pages WHERE document_id = ?1",
            rusqlite::params![document_id],
            |row| row.get(0),
        )
        .unwrap_or(0)
    };

    // 成功处理的页数：仅 done 页有实际内容，计入覆盖率
    let success_count = done_count;
    // empty_ocr 表示 OCR 已执行但无内容，页面可能仍有原生文本层，
    // 不应与真正的请求失败（failed）混为一谈。
    // 只有真正的 failed（429/timeout/API 错误）才计入失败。

    let terminal_count = done_count + failed_count + empty_ocr_count + skipped_count + needed_count;

    let (status, coverage) = if total_ocr_pages == 0 {
        (OcrStatus::NotNeeded, 0.0)
    } else if terminal_count < total_ocr_pages {
        // 仍有 pending/processing 页，不应误判为 done。
        let cov = if total_pages > 0 {
            success_count as f64 / total_pages as f64
        } else {
            0.0
        };
        (OcrStatus::Processing, cov)
    } else if needed_count == total_ocr_pages {
        // 策略阻断：所有页均因库隐私策略或全局开关被阻止，OCR 仍需要。
        (OcrStatus::Needed, 0.0)
    } else if failed_count == total_ocr_pages {
        // 全部页真正失败（仅有 failed，无 done 也无 empty_ocr）
        (OcrStatus::Failed, 0.0)
    } else if failed_count > 0 || empty_ocr_count > 0 || skipped_count > 0 || needed_count > 0 {
        // 有失败/空结果/跳过/策略阻断页 → partial
        let cov = if total_pages > 0 {
            success_count as f64 / total_pages as f64
        } else {
            0.0
        };
        (OcrStatus::Partial, cov)
    } else {
        // 全部成功（仅 done 页）
        let cov = if total_pages > 0 {
            success_count as f64 / total_pages as f64
        } else {
            0.0
        };
        (OcrStatus::Done, cov)
    };

    let kb = KbEngine::new(conn);
    kb.update_document_ocr_with_run_guard(document_id, status.as_str(), coverage, run_id)?;

    Ok(status)
}

/// 记录一次 GLM-OCR 外部调用审计。审计写入失败只记录 warn，不影响 OCR 主流程。
#[allow(clippy::too_many_arguments)]
pub fn log_ocr_external_model_call(
    conn: &Connection,
    library_id: i64,
    document_id: i64,
    provider: &str,
    model: &str,
    latency_ms: i32,
    success: bool,
    error_message: &str,
    page_results: &[crate::kb::ocr_provider::OcrPageResult],
) {
    let (input_tokens, output_tokens) = ocr_usage_tokens(page_results);
    let safe_error = sanitize_audit_error(error_message);
    if let Err(e) = crate::kb::privacy::log_external_model_call(
        conn,
        Some(library_id),
        Some(document_id),
        "ocr",
        provider,
        model,
        input_tokens,
        output_tokens,
        latency_ms,
        0.0,
        success,
        &safe_error,
    ) {
        tracing::warn!(
            document_id,
            provider,
            model,
            error = %e,
            "OCR 外部模型调用审计写入失败"
        );
    }
}

fn ocr_usage_tokens(page_results: &[crate::kb::ocr_provider::OcrPageResult]) -> (i32, i32) {
    let mut seen = std::collections::HashSet::new();
    let mut input_tokens = 0i32;
    let mut output_tokens = 0i32;

    for result in page_results {
        let key = result
            .request_id
            .clone()
            .or_else(|| {
                result
                    .raw_response_json
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| result.raw_response_json.to_string());
        if !seen.insert(key) {
            continue;
        }

        if let Some(usage) = result.raw_response_json.get("usage") {
            let prompt_tokens = usage
                .get("prompt_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(0)
                .clamp(0, i32::MAX as i64) as i32;
            let completion_tokens = usage
                .get("completion_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(0)
                .clamp(0, i32::MAX as i64) as i32;
            input_tokens = input_tokens.saturating_add(prompt_tokens);
            output_tokens = output_tokens.saturating_add(completion_tokens);
        }
    }

    (input_tokens, output_tokens)
}

fn sanitize_audit_error(message: &str) -> String {
    let one_line = message.split_whitespace().collect::<Vec<_>>().join(" ");
    one_line.chars().take(1000).collect()
}

// ---------------------------------------------------------------------------
// P2-019: OCR 结果回写
// ---------------------------------------------------------------------------

/// OCR 回写用的简化页级结果（仅含页码和文本）
///
/// 与 ocr_provider::OcrPageResult（含 markdown/blocks/provider 等完整字段）不同，
/// 此结构体仅用于 writeback 路径，聚焦于文本回写到 KB 索引。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrWritebackPage {
    /// 页码（1-based）
    pub page_number: i32,
    /// 该页 OCR 识别出的文本
    pub text: String,
}

/// P2-019: 将 OCR 结果转换为标准 ParsedBlock 列表。
///
/// 每个 `OcrWritebackPage` 生成一个 `ParsedBlock`，block_type 为 "ocr_text"，
/// 携带 page_number 和基于累计字符偏移的 source_start/source_end。
pub fn ocr_to_parsed_blocks(ocr_pages: &[OcrWritebackPage]) -> Vec<ParsedBlock> {
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
/// 接收 OCR 识别结果（或合并后的文本层+OCR 结果），将其转换为 ParsedBlock，
/// 再分割为 RaptorNode，持久化到数据库，并更新文档的 OCR 状态和文本覆盖率。
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
///
/// `run_id` 用于防止过期 OCR 覆盖新上传产生的节点，传 None 则不做守卫。
#[allow(clippy::too_many_arguments)]
pub fn writeback_ocr_results(
    conn: &Connection,
    doc_id: i64,
    lib_id: i64,
    ocr_pages: &[OcrWritebackPage],
    chunk_size: usize,
    chunk_overlap: usize,
    doc_title: &str,
    total_page_count: i32,
    run_id: Option<&str>,
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
        // 即使全文为空，也必须更新文档 OCR 状态，避免文档永远停在 processing
        update_document_ocr_status(conn, doc_id, total_page_count, run_id)?;
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

    // 5. 持久化节点，使用 run_id 守卫防止过期 OCR 覆盖新节点
    persist_nodes_and_vectors(conn, doc_id, lib_id, &nodes, run_id)?;

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

    // 7. 更新文档 OCR 状态（根据实际页级状态计算，避免无条件设为 Done 掩盖 failed/partial）
    update_document_ocr_status(conn, doc_id, total_page_count, run_id)?;

    // 8. 更新文档统计（词数/分块数）
    // 修复：embedding_status 传 STATUS_PENDING 而非 None。
    // None 会 unwrap_or(STATUS_COMPLETED) 导致 embedding 被误标为已完成，
    // 但此时节点均无向量，向量检索和 RAPTOR 全部丢失。
    // 正确做法：标记为 pending，随后入队 kb_reembed job 补齐 embedding。
    let kb = KbEngine::new(conn);
    let word_total: i32 = if chinese::has_chinese(&full_text) {
        let tokens = chinese::tokenize_content(&full_text);
        tokens.split_whitespace().count() as i32
    } else {
        full_text.split_whitespace().count() as i32
    };
    // 使用 run guard 版本，防止 stale OCR job 覆盖新 run 的文档状态
    kb.update_document_stats_with_run_guard(
        doc_id,
        word_total,
        nodes_created as i32,
        Some(crate::kb::types::STATUS_PENDING),
        run_id,
    )?;

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
        let pages = vec![OcrWritebackPage {
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
            OcrWritebackPage {
                page_number: 1,
                text: "Page one".to_string(),
            },
            OcrWritebackPage {
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
            OcrWritebackPage {
                page_number: 1,
                text: "第一页内容".to_string(),
            },
            OcrWritebackPage {
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
