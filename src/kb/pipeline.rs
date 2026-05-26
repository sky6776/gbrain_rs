//! KB 文档处理管道
//!
//! 5阶段异步管道: 解析 → 分割 → 嵌入 → RAPTOR → 持久化
//!
//! 每个阶段通过可选的 `on_progress` 回调报告进度。

use crate::embedding::Embedder;
use crate::error::{GBrainError, Result};
use crate::kb::engine::KbEngine;
use crate::kb::jobs::KbProcessPayload;
use crate::kb::parser::{ParsedDocument, ParserRegistry};
use crate::kb::raptor::{self, RaptorConfig};
use crate::kb::splitter::{create_async_splitter, create_splitter, SplitterConfig};
use crate::kb::types::*;
use crate::nlp::chinese;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Arc;

/// FIX10-R1: 在全文中定位每个 chunk 的字符偏移范围
///
/// 严格区分 byte offset 和 char offset：
/// - `String` 切片和 `find()` 返回的都是 byte offset
/// - 存储到 chunk_offsets 的是 char offset（字符偏移），与 source_start/source_end 语义一致
/// - 更新 byte_cursor 时确保落在 UTF-8 字符边界上
/// - fallback 路径用字符 offset 推进，再通过 char_indices 找对应 byte offset，始终 clamp 到 full_text.len()
///
/// 游标推进策略：匹配当前 chunk 后，游标推进到 end_char（无回退）。
/// 搜索下一 chunk 前，按相邻 chunk 的实际 suffix/prefix overlap 回退游标到重叠区，
/// 使 overlap chunk 能从重叠区域精确匹配，而非 fallback 到错误位置。
///
/// `max_overlap` 为 splitter 可能产生的最大重叠字符数，作为 overlap 上限：
/// - 防止偶然的 suffix/prefix 相同被误判为 splitter overlap
/// - 例：chunk_overlap=0 或 Markdown splitter 无 overlap 时传 0，
///   即使 chunks[i-1] 尾部和 chunks[i] 头部偶然相同也不回退
/// - Recursive splitter 传 config.chunk_overlap（splitter 内部已 cap 到 chunk_size/2）
///
/// 回退时机必须在搜索当前块之前，而非搜索之后：
/// - 搜索之后回退只能影响下一块，当前块已经从错误位置搜索过了
/// - 例：full_text="abcdef"、chunks ["abcd","cdef"]
///   若第一块后 cursor=4，第二块从 4 开始 find("cdef") 找不到，fallback 记录 4..8
///   正确做法：搜索第二块前回退 cursor 到 2，从 2 开始 find("cdef") 精确匹配 2..6
pub fn locate_chunk_char_offsets(
    full_text: &str,
    chunks: &[String],
    max_overlap: usize,
) -> Vec<(usize, usize)> {
    let mut offsets = Vec::with_capacity(chunks.len());
    let mut byte_cursor: usize = 0;
    let mut char_cursor: usize = 0;

    for (i, chunk) in chunks.iter().enumerate() {
        // 搜索当前块前，按与前一块的实际 overlap 回退游标到重叠区
        // 实际 overlap 取 suffix/prefix 匹配长度，但不超过 max_overlap 上限
        // max_overlap=0 时（Markdown splitter / chunk_overlap=0）不回退，避免偶然相同误判
        if i > 0 && max_overlap > 0 {
            let overlap_chars = actual_overlap_chars(&chunks[i - 1], chunk, max_overlap);
            // 允许 overlap_chars == char_cursor 回退到 0：
            // 例 full_text="abcdef"、chunks=["a","abcdef"]、max_overlap=1
            // 第一块后 char_cursor=1，overlap_chars=1，应回退到 0 才能精确匹配
            if overlap_chars > 0 && overlap_chars <= char_cursor {
                let back_char = char_cursor - overlap_chars;
                byte_cursor = full_text
                    .char_indices()
                    .nth(back_char)
                    .map(|(i, _)| i)
                    .unwrap_or(full_text.len());
                char_cursor = back_char;
            }
        }

        // FIX10-R1: 先检查 byte_cursor 不越界，再尝试 find
        // find 成功 → 精确匹配路径；find 失败或 byte_cursor 越界 → fallback 推算路径
        let found = if byte_cursor <= full_text.len() {
            full_text[byte_cursor..].find(chunk.as_str())
        } else {
            None
        };
        if let Some(pos) = found {
            let start_byte = byte_cursor + pos;

            // byte offset 转字符 offset
            let start_char = char_cursor + full_text[byte_cursor..start_byte].chars().count();
            let end_char = start_char + chunk.chars().count();
            offsets.push((start_char, end_char));

            // 匹配成功后，游标推进到 end_char（不回退）
            // 下一次迭代开始时会根据与当前块的 overlap 回退
            byte_cursor = full_text
                .char_indices()
                .nth(end_char)
                .map(|(i, _)| i)
                .unwrap_or(full_text.len());
            char_cursor = end_char;
        } else {
            // 无法找到精确位置，用推算偏移
            // fallback 路径：用字符 offset 推进，再通过 char_indices 找对应 byte offset
            let start_char = char_cursor;
            let chunk_char_len = chunk.chars().count();
            let end_char = start_char + chunk_char_len;
            offsets.push((start_char, end_char));

            // fallback 也推进到 end_char
            let skip_chars = end_char.saturating_sub(char_cursor);
            byte_cursor = full_text[byte_cursor..]
                .char_indices()
                .nth(skip_chars)
                .map(|(i, _)| byte_cursor + i)
                .unwrap_or(full_text.len());
            // 始终 clamp 到 full_text.len()，防止越界
            byte_cursor = byte_cursor.min(full_text.len());
            char_cursor = end_char;
        }
    }

    offsets
}

/// 计算两个相邻 chunk 的实际 suffix/prefix overlap 字符数
///
/// 取前一个 chunk 的尾部和当前 chunk 的头部，找到最大公共子串长度。
/// 结果不超过 `max_overlap` 上限，防止偶然的 suffix/prefix 相同被误判为 splitter overlap。
/// 例：max_overlap=0 时直接返回 0；max_overlap=2 时最多回退 2 个字符。
fn actual_overlap_chars(prev: &str, curr: &str, max_overlap: usize) -> usize {
    if max_overlap == 0 {
        return 0;
    }
    let prev_chars: Vec<char> = prev.chars().collect();
    let curr_chars: Vec<char> = curr.chars().collect();
    let max_possible = prev_chars.len().min(curr_chars.len()).min(max_overlap);

    // 从最大可能重叠开始递减，找到第一个匹配的长度
    for overlap in (1..=max_possible).rev() {
        let prev_suffix = &prev_chars[prev_chars.len() - overlap..];
        let curr_prefix = &curr_chars[..overlap];
        if prev_suffix == curr_prefix {
            return overlap;
        }
    }
    0
}

/// 根据 splitter 配置计算最大可能 overlap 字符数
///
/// 优先级与 `create_async_splitter` 一致：
/// 1. semantic_enabled + 有 embedder → SemanticSplitter 有 overlap → 返回 chunk_overlap
/// 2. semantic_enabled 但无 embedder → 回退到 Markdown/Recursive
/// 3. Markdown 扩展名 → MarkdownHeaderSplitter 无 overlap → 返回 0
/// 4. 其余 → RecursiveCharSplitter，内部 cap 到 chunk_size/2 → 返回 chunk_overlap.min(chunk_size/2)
///
/// `has_embedder` 参数指示调用方是否实际提供了 embedder，
/// 用于匹配 `create_async_splitter` 中 semantic_enabled=true 但无 embedder 的回退逻辑。
pub fn splitter_max_overlap(config: &SplitterConfig, has_embedder: bool) -> usize {
    if config.semantic_enabled && has_embedder {
        return config.chunk_overlap;
    }
    // semantic_enabled=true 但无 embedder 时，回退到与 create_splitter 相同的逻辑
    let ext = std::path::Path::new(&config.file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext == "md" || ext == "markdown" {
        0
    } else {
        config.chunk_overlap.min(config.chunk_size / 2)
    }
}

/// 进度回调类型: 接收阶段名称和进度消息
pub type ProgressCallback = Box<dyn Fn(&str, &str) + Send + Sync>;

/// 管道配置
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// 分割器配置
    pub splitter: SplitterConfig,
    /// RAPTOR 配置
    pub raptor: RaptorConfig,
    /// 是否在嵌入后构建 RAPTOR 树
    pub enable_raptor: bool,
    /// 是否生成嵌入向量
    pub enable_embedding: bool,
}

// ---------------------------------------------------------------------------
// 同步管道 (保留原有接口兼容性)
// ---------------------------------------------------------------------------

/// 处理 KB 文档的同步管道 (原有接口)
///
/// 阶段:
/// 1. 解析  - 使用 ParserRegistry 从文档文件提取文本
/// 2. 分割  - 使用 create_splitter 将文本分块
/// 3. 构建  - 构造 level-0 RaptorNode 对象
/// 4. RAPTOR - 构建抽象树 (如启用, 延迟到嵌入完成后)
/// 5. 持久化 - 在一个事务中写入节点、向量和 FTS5 条目
///
/// 成功时返回词数/分块数。失败时文档状态设为 STATUS_FAILED。
pub fn process_document(conn: &Connection, payload: &KbProcessPayload) -> Result<ProcessResult> {
    // PDF 必须走异步 OCR 流程（process_document_async），同步入口拒绝 PDF
    if payload.extension.to_lowercase() == "pdf" {
        return Err(GBrainError::InvalidInput(
            "PDF 文档必须使用异步 OCR 流程 (process_document_async)，不允许通过同步入口处理".to_string(),
        ));
    }

    let kb = KbEngine::new(conn);
    let doc_id = payload.document_id;
    let lib_id = payload.library_id;
    let run_id = &payload.processing_run_id;

    // 守卫: 确保此运行仍是当前的 (防止过期作业执行)
    kb.ensure_document_run_current(doc_id, run_id)?;

    // 加载库配置
    let library = kb.get_library(lib_id)?;

    // --- 阶段 1: 解析 ---
    // 修复：所有中间状态更新传入 run_id，防止旧 job 污染新 run 的文档状态
    kb.update_document_status_with_run_guard(
        doc_id,
        Some(STATUS_PROCESSING),
        Some(10),
        None,
        None,
        None,
        None,
        Some(run_id),
    )?;

    let registry = ParserRegistry::new();
    let ext = &payload.extension;
    let storage_path = &payload.storage_path;

    let file_data = std::fs::read(storage_path).map_err(|e| {
        let _ = kb.update_document_status_with_run_guard(
            doc_id,
            Some(STATUS_FAILED),
            None,
            Some(&format!("无法读取 {}: {}", storage_path, e)),
            None,
            None,
            None,
            Some(run_id),
        );
        GBrainError::FileError(format!("无法读取 {}: {}", storage_path, e))
    })?;

    let parsed = registry.parse(ext, &file_data).inspect_err(|e| {
        let _ = kb.update_document_status_with_run_guard(
            doc_id,
            Some(STATUS_FAILED),
            None,
            Some(&e.to_string()),
            None,
            None,
            None,
            Some(run_id),
        );
    })?;

    let word_total: i32 = count_words(&parsed.content) as i32;
    kb.update_document_status_with_run_guard(
        doc_id,
        Some(STATUS_PROCESSING),
        Some(30),
        None,
        None,
        None,
        None,
        Some(run_id),
    )?;

    // --- 阶段 2: 分割 ---
    let splitter_config = SplitterConfig {
        file_path: storage_path.to_string(),
        chunk_size: library.chunk_size,
        chunk_overlap: library.chunk_overlap,
        semantic_enabled: library.semantic_segmentation_enabled,
    };

    let splitter = create_splitter(&splitter_config);
    let chunks = splitter.split(&parsed.content).inspect_err(|e| {
        let _ = kb.update_document_status_with_run_guard(
            doc_id,
            Some(STATUS_FAILED),
            None,
            Some(&e.to_string()),
            None,
            None,
            None,
            Some(run_id),
        );
    })?;

    let split_total: i32 = chunks.len() as i32;
    kb.update_document_status_with_run_guard(
        doc_id,
        Some(STATUS_COMPLETED),
        Some(100),
        None,
        Some(STATUS_PROCESSING),
        Some(10),
        None,
        Some(run_id),
    )?;

    // --- 阶段 3: 构建 Level-0 节点 ---
    let raptor_nodes: Vec<RaptorNode> = chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| RaptorNode {
            id: -((i as i64) + 1), // 临时负 ID
            library_id: lib_id,
            document_id: doc_id,
            content: chunk.clone(),
            level: 0,
            parent_id: None,
            chunk_order: i as i32,
            vector: None,
            title_path: String::new(),
            page_number: None,
            source_start: None,
            source_end: None,
            node_metadata: String::new(),
            embedding_text: String::new(),
        })
        .collect();

    // --- 阶段 4: RAPTOR (可选, 延迟) ---
    if library.raptor_enabled && raptor_nodes.len() >= 3 {
        tracing::info!(
            doc_id,
            node_count = raptor_nodes.len(),
            "RAPTOR 已启用; 树构建延迟到嵌入完成后"
        );
    }

    // --- 阶段 5: 持久化 ---
    // 修复：持久化前再次校验 run_id，防止 stale job 在通过初始校验后
    // 继续跑完后续阶段（新上传可能已更新 run_id）
    kb.ensure_document_run_current(doc_id, run_id)?;
    // 修复：将 persist_nodes_and_vectors 和 update_document_stats_with_run_guard
    // 合入同一事务，消除两者之间的竞态窗口。旧代码先单独提交 nodes（事务 1），
    // 再单独更新 stats（事务 2）。若新上传在两者之间更新 processing_run_id，
    // stats 会失败（run_guard 拒绝），但旧 run 的 nodes 已经留下。
    // 合入同一事务后，如果 stats 的 run_guard 检查失败，整个事务回滚，
    // nodes 也不会留下。
    {
        let tx = conn.unchecked_transaction()?;
        let result = (|| -> Result<()> {
            persist_nodes_and_vectors_inner(conn, doc_id, lib_id, &raptor_nodes, Some(run_id))?;
            // 修复：外层已开事务，调用 _inner 避免嵌套 BEGIN（SQLite 不支持）
            kb.update_document_stats_with_run_guard_inner(
                doc_id,
                word_total,
                split_total,
                None,
                Some(run_id),
            )?;
            Ok(())
        })();
        match result {
            Ok(_) => tx.commit()?,
            Err(e) => {
                let _ = tx.rollback();
                return Err(e);
            }
        }
    }

    Ok(ProcessResult {
        word_total,
        split_total,
        deferred_ocr: false,
    })
}

// ---------------------------------------------------------------------------
// 异步管道 (5阶段: 解析 → 分割 → 嵌入 → RAPTOR → 持久化)
// ---------------------------------------------------------------------------

/// 异步处理 KB 文档的 5 阶段管道
///
/// 阶段:
/// 1. **解析** — 检测格式, 提取文本内容
/// 2. **分割** — 使用配置的分割器将文本分块
/// 3. **嵌入** — 为每个节点生成嵌入向量
/// 4. **RAPTOR** — 构建层次化摘要树 (可选)
/// 5. **持久化** — 将所有节点写入数据库
#[allow(clippy::too_many_arguments)]
pub async fn process_document_async(
    conn: &Connection,
    payload: &KbProcessPayload,
    embedder: Option<Arc<Embedder>>,
    raptor_config: Option<&RaptorConfig>,
    kb_raptor_secret_ref: Option<&str>,
    kb_raptor_base_url: Option<&str>,
    kb_raptor_model: Option<&str>,
    on_progress: Option<&ProgressCallback>,
) -> Result<ProcessResult> {
    let kb = KbEngine::new(conn);
    let doc_id = payload.document_id;
    let lib_id = payload.library_id;
    let run_id = &payload.processing_run_id;

    kb.ensure_document_run_current(doc_id, run_id)?;
    let library = kb.get_library(lib_id)?;

    // --- 阶段 1: 解析 ---
    report_progress(
        on_progress,
        "parsing",
        &format!("解析 {}", payload.storage_path),
    );
    // 修复：中间状态更新传入 run_id，防止旧 job 污染新 run 的文档状态
    kb.update_document_status_with_run_guard(
        doc_id,
        Some(STATUS_PROCESSING),
        Some(10),
        None,
        None,
        None,
        None,
        Some(run_id),
    )?;

    let registry = ParserRegistry::new();
    let ext = &payload.extension;
    let storage_path = &payload.storage_path;

    let file_data = std::fs::read(storage_path).map_err(|e| {
        let _ = kb.update_document_status_with_run_guard(
            doc_id,
            Some(STATUS_FAILED),
            None,
            Some(&format!("无法读取 {}: {}", storage_path, e)),
            None,
            None,
            None,
            Some(run_id),
        );
        GBrainError::FileError(format!("无法读取 {}: {}", storage_path, e))
    })?;

    let parsed = registry.parse(ext, &file_data).inspect_err(|e| {
        let _ = kb.update_document_status_with_run_guard(
            doc_id,
            Some(STATUS_FAILED),
            None,
            Some(&e.to_string()),
            None,
            None,
            None,
            Some(run_id),
        );
    })?;

    // Phase 1+2: PDF OCR 分支 — 解析后、分割前执行 OCR
    // 返回 None 表示异步 OCR 已入队，主 pipeline 应在此提前返回，不执行后续 split/embed/persist
    let parsed = if ext == "pdf" {
        // 加载 OCR 配置（单次加载，避免 maybe_apply_pdf_ocr 内部重复 Config::load）
        let ocr_config = crate::config::Config::load().unwrap_or_default();
        match maybe_apply_pdf_ocr(
            conn,
            doc_id,
            lib_id,
            run_id,
            storage_path,
            &parsed,
            &library,
            &file_data,
            &ocr_config,
        )? {
            Some(p) => p,
            None => {
                // 异步 OCR 已入队，且 maybe_apply_pdf_ocr 已在同一事务中
                // 写入 document_status=ocr_pending / ocr_status=queued。
                // 这里仅提前返回，避免后续 split/embed/persist 和 artifact 流程抢跑。
                tracing::info!(doc_id, "PDF OCR 已异步入队，主 pipeline 提前返回");
                return Ok(ProcessResult {
                    word_total: 0,
                    split_total: 0,
                    deferred_ocr: true, // P2 修复：显式标记 OCR 已延后
                });
            }
        }
    } else {
        parsed
    };

    let word_total: i32 = count_words(&parsed.content) as i32;

    // P1-013: 元数据抽取（文件系统 + 格式特定）
    let storage = std::path::Path::new(storage_path);
    let mut doc_meta = crate::kb::metadata::DocumentMetadata::from_file_path(storage);
    let format_meta = match ext.as_str() {
        "md" => crate::kb::metadata::extract_markdown_metadata(&parsed.content, &file_data),
        "pdf" => crate::kb::metadata::extract_pdf_metadata(&parsed.content, &file_data),
        "docx" => crate::kb::metadata::extract_docx_metadata(&parsed.content, &file_data),
        "html" | "htm" => crate::kb::metadata::extract_html_metadata(&parsed.content, &file_data),
        _ => crate::kb::metadata::DocumentMetadata::default(),
    };
    doc_meta.merge_with(&format_meta);
    // P1-019: 关键词和实体抽取
    let (keywords, entities) = crate::kb::metadata::extract_keywords_and_entities(
        &parsed.content,
        doc_meta.language.as_deref().unwrap_or("zh"),
    );
    // 落库元数据
    // FIX11-04: 元数据更新失败不应静默吞下，至少记录警告
    // 修复：传入 run_id，防止旧 job 污染新 run 的文档元数据
    if let Err(e) = kb.update_document_metadata_with_run_guard(
        doc_id,
        doc_meta.title.as_deref().unwrap_or(""),
        doc_meta.author.as_deref().unwrap_or(""),
        &keywords,
        &entities,
        doc_meta.source_uri.as_deref().unwrap_or(""),
        doc_meta.document_date.as_deref(),
        doc_meta.modified_at.as_deref(),
        Some(run_id),
    ) {
        tracing::warn!("文档 {} 元数据更新失败: {}", doc_id, e);
    }

    // P1-014: 文档粒度分类（解析完成后立即判定）
    let char_count = parsed.content.chars().count();
    let page_count: usize = parsed
        .metadata
        .get("total_pages")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let granularity = crate::kb::granularity::classify_granularity(ext, char_count, page_count);
    let chunk_strategy = crate::kb::granularity::chunk_strategy_for(granularity);
    // 修复：传入 run_id，防止旧 job 污染新 run 的 granularity
    kb.update_document_granularity_with_run_guard(
        doc_id,
        granularity.as_str(),
        chunk_strategy,
        char_count as i32,
        page_count as i32,
        Some(run_id),
    )?;

    // 修复：中间状态更新传入 run_id，防止旧 job 污染新 run 的文档状态
    kb.update_document_status_with_run_guard(
        doc_id,
        Some(STATUS_PROCESSING),
        Some(30),
        None,
        None,
        None,
        None,
        Some(run_id),
    )?;

    // --- 阶段 2: 分割 ---
    report_progress(on_progress, "splitting", "分割为节点");
    let splitter_config = SplitterConfig {
        file_path: storage_path.to_string(),
        chunk_size: library.chunk_size,
        chunk_overlap: library.chunk_overlap,
        semantic_enabled: library.semantic_segmentation_enabled,
    };

    let splitter = create_async_splitter(&splitter_config, embedder.clone()).inspect_err(|e| {
        let _ = kb.update_document_status_with_run_guard(
            doc_id,
            Some(STATUS_FAILED),
            None,
            Some(&e.to_string()),
            None,
            None,
            None,
            Some(run_id),
        );
    })?;
    let chunks = splitter
        .split_async(&parsed.content)
        .await
        .inspect_err(|e| {
            let _ = kb.update_document_status_with_run_guard(
                doc_id,
                Some(STATUS_FAILED),
                None,
                Some(&e.to_string()),
                None,
                None,
                None,
                Some(run_id),
            );
        })?;

    let split_total: i32 = chunks.len() as i32;

    // P1-010: 从 parser blocks 中提取每块的元数据，用 span 匹配而非下标硬匹配
    #[allow(clippy::type_complexity)]
    let block_spans: Vec<(String, Option<i32>, Option<i32>, Option<i32>)> = parsed
        .blocks
        .as_ref()
        .map(|blocks| {
            blocks
                .iter()
                .map(|b| {
                    (
                        b.title_path.clone(),
                        b.page_number,
                        b.source_start,
                        b.source_end,
                    )
                })
                .collect()
        })
        .unwrap_or_default();

    // FIX10-R1: 使用统一的 helper 定位 chunk 字符偏移，max_overlap 按 splitter 类型计算
    // semantic → chunk_overlap; Markdown → 0; Recursive → chunk_overlap.min(chunk_size/2)
    let full_text = &parsed.content;
    let max_overlap = splitter_max_overlap(&splitter_config, embedder.is_some());
    let chunk_offsets: Vec<(usize, usize)> =
        locate_chunk_char_offsets(full_text, &chunks, max_overlap);

    // 对每个 chunk，找与其 span 重叠最多的 block
    #[allow(clippy::type_complexity)]
    fn find_best_block(
        chunk_start: usize,
        chunk_end: usize,
        spans: &[(String, Option<i32>, Option<i32>, Option<i32>)],
    ) -> (String, Option<i32>, Option<i32>, Option<i32>) {
        let mut best: (usize, (String, Option<i32>, Option<i32>, Option<i32>)) =
            (0, spans.first().cloned().unwrap_or_default());
        for (idx, (_title, _page, s_start, s_end)) in spans.iter().enumerate() {
            let bs = s_start.unwrap_or(0) as usize;
            let be = s_end.unwrap_or(i32::MAX) as usize;
            let overlap = if chunk_start < be && chunk_end > bs {
                chunk_end.min(be) - chunk_start.max(bs)
            } else {
                0
            };
            if overlap > best.0 {
                best = (overlap, spans[idx].clone());
            }
        }
        best.1
    }

    // P1-006/P1-007: 根据粒度应用节点策略 + P1-011: 生成 contextual embedding 文本
    let doc_title = doc_meta.title.as_deref().unwrap_or("");
    let mut nodes: Vec<RaptorNode> = chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| {
            let (c_start, c_end) = chunk_offsets[i];
            let (title_path, page_num, src_start, src_end) = if block_spans.is_empty() {
                (String::new(), None, None, None)
            } else {
                find_best_block(c_start, c_end, &block_spans)
            };
            let embedding_text =
                crate::kb::context::build_embedding_text(doc_title, &title_path, page_num, chunk);
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

    // P2-013/P2-014: 表格文档写入 kb_tables / kb_table_rows
    if granularity == crate::kb::granularity::DocumentGranularity::Table {
        if let Some(ref blocks) = parsed.blocks {
            for block in blocks {
                if block.block_type == "table" && !block.metadata.is_empty() {
                    if let Ok(sheet_data) =
                        serde_json::from_str::<serde_json::Value>(&block.metadata)
                    {
                        if let Some(name) = sheet_data.get("name").and_then(|v| v.as_str()) {
                            let headers: Vec<String> = sheet_data
                                .get("headers")
                                .and_then(|v| v.as_array())
                                .map(|a| {
                                    a.iter()
                                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                        .collect()
                                })
                                .unwrap_or_default();
                            let row_count = sheet_data
                                .get("row_count")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0) as i32;
                            match crate::kb::table_index::insert_table(
                                conn, doc_id, name, &headers, row_count,
                            ) {
                                Ok(table_id) => {
                                    if let Some(rows) =
                                        sheet_data.get("rows").and_then(|v| v.as_array())
                                    {
                                        for (ri, row) in rows.iter().enumerate() {
                                            let row_text = headers
                                                .iter()
                                                .filter_map(|h| {
                                                    row.get(h)
                                                        .and_then(|v| v.as_str())
                                                        .map(|s| format!("{}: {}", h, s))
                                                })
                                                .collect::<Vec<_>>()
                                                .join(" ");
                                            let row_json =
                                                serde_json::to_string(row).unwrap_or_default();
                                            // FIX11-05: 表格行插入失败不应静默吞下
                                            if let Err(e) = crate::kb::table_index::insert_table_row(
                                                conn, table_id, ri as i32, &row_text, &row_json,
                                            ) {
                                                tracing::warn!(
                                                    "表格行插入失败 table_id={} row={}: {}",
                                                    table_id,
                                                    ri,
                                                    e
                                                );
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "表格元数据创建失败 doc_id={} table_name={}: {}",
                                        doc_id,
                                        name,
                                        e
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // P1-006: micro 文档策略 — 仅保留一个 whole-document node
    if granularity == crate::kb::granularity::DocumentGranularity::Micro && !chunks.is_empty() {
        let full_text = parsed.content.clone();
        let embedding_text = crate::kb::context::build_micro_embedding_text(doc_title, &full_text);
        nodes = vec![RaptorNode {
            id: -1,
            library_id: lib_id,
            document_id: doc_id,
            content: full_text,
            level: 0,
            parent_id: None,
            chunk_order: 0,
            vector: None,
            title_path: String::new(),
            page_number: None,
            source_start: None,
            source_end: None,
            node_metadata: "{\"node_type\":\"whole_document\"}".to_string(),
            embedding_text,
        }];
    }

    // --- 阶段 3: 嵌入 ---
    // FIX9-02: 区分 embedding_failed 和 embedding_skipped（因隐私策略跳过）
    let mut embedding_failed = false;
    let mut embedding_skipped = false;
    if let Some(emb) = embedder.as_ref() {
        report_progress(
            on_progress,
            "embedding",
            &format!("嵌入 {} 个节点", nodes.len()),
        );
        // 修复：中间状态更新传入 run_id，防止旧 job 污染新 run 的文档状态
        kb.update_document_status_with_run_guard(
            doc_id,
            None,
            None,
            None,
            Some(STATUS_PROCESSING),
            Some(10),
            None,
            Some(run_id),
        )?;

        // P0-016: 检查库级隐私策略 — 禁止外部 embedding 时跳过
        if !library.external_embedding_allowed {
            embedding_skipped = true;
            report_progress(
                on_progress,
                "embedding",
                "库级策略禁止外部 embedding，跳过嵌入阶段",
            );
        } else {
            // P1-012: embedding 使用 embedding_text，为空时 fallback 到 content
            let texts: Vec<&str> = nodes
                .iter()
                .map(|n| {
                    if n.embedding_text.is_empty() {
                        n.content.as_str()
                    } else {
                        n.embedding_text.as_str()
                    }
                })
                .collect();
            match emb.embed_batch(&texts).await {
                Ok(vectors) => {
                    for (i, node) in nodes.iter_mut().enumerate() {
                        if i < vectors.len() {
                            node.vector = Some(vectors[i].clone());
                        }
                    }
                    // 修复：中间状态更新传入 run_id，防止旧 job 污染新 run 的文档状态
                    kb.update_document_status_with_run_guard(
                        doc_id,
                        None,
                        None,
                        None,
                        Some(STATUS_PROCESSING),
                        Some(80),
                        None,
                        Some(run_id),
                    )?;
                }
                Err(e) => {
                    embedding_failed = true;
                    report_progress(
                        on_progress,
                        "embedding",
                        &format!("嵌入失败: {}, 标记为 embedding_failed", e),
                    );
                    // 修复：中间状态更新传入 run_id，防止旧 job 污染新 run 的文档状态
                    kb.update_document_status_with_run_guard(
                        doc_id,
                        None,
                        None,
                        None,
                        Some(STATUS_FAILED),
                        Some(80),
                        Some(&format!("embedding failed: {}", e)),
                        Some(run_id),
                    )?;
                }
            }
        }
    }

    // --- 阶段 4: RAPTOR ---
    // P0-016: 检查库级隐私策略 — 禁止外部摘要时跳过 RAPTOR
    if library.raptor_enabled && nodes.len() >= 3 {
        if !library.external_summary_allowed {
            report_progress(
                on_progress,
                "raptor",
                "库级策略禁止外部摘要，跳过 RAPTOR 阶段",
            );
        } else if let Some(rc) = raptor_config {
            report_progress(on_progress, "raptor", "构建 RAPTOR 树");

            match raptor::resolve_raptor_llm_config(
                Some(&library),
                kb_raptor_secret_ref,
                kb_raptor_base_url,
                kb_raptor_model,
            ) {
                Ok(llm_config) => {
                    let max_tokens = rc.max_tokens_per_summary;
                    let llm_cfg = llm_config.clone();

                    // 将节点内容克隆为拥有的数据, 避免闭包中的生命周期问题

                    let result = raptor::build_raptor_tree(rc, &mut nodes, |cluster| {
                        let cfg = llm_cfg.clone();
                        // 克隆集群节点内容用于 LLM 调用
                        let cluster_texts: Vec<String> =
                            cluster.iter().map(|n| n.content.clone()).collect();
                        async move {
                            // 构造临时 RaptorNode 列表用于 summarize_cluster
                            let temp_nodes: Vec<RaptorNode> = cluster_texts
                                .iter()
                                .enumerate()
                                .map(|(i, content)| RaptorNode {
                                    id: i as i64,
                                    library_id: 0,
                                    document_id: 0,
                                    content: content.clone(),
                                    level: 0,
                                    parent_id: None,
                                    chunk_order: i as i32,
                                    vector: None,
                                    title_path: String::new(),
                                    page_number: None,
                                    source_start: None,
                                    source_end: None,
                                    node_metadata: String::new(),
                                    embedding_text: String::new(),
                                })
                                .collect();
                            let refs: Vec<&RaptorNode> = temp_nodes.iter().collect();
                            raptor::summarize_cluster(&refs, &cfg, max_tokens).await
                        }
                    })
                    .await;

                    match result {
                        Ok(tree_nodes) => {
                            nodes = tree_nodes;
                            report_progress(
                                on_progress,
                                "raptor",
                                &format!("RAPTOR 树已构建: {} 个总节点", nodes.len()),
                            );
                        }
                        Err(e) => {
                            report_progress(
                                on_progress,
                                "raptor",
                                &format!("RAPTOR 失败: {}, 继续无摘要", e),
                            );
                        }
                    }
                }
                Err(e) => {
                    report_progress(
                        on_progress,
                        "raptor",
                        &format!("RAPTOR LLM 未配置: {}, 跳过", e),
                    );
                }
            }
        }
    }

    // --- 阶段 5: 持久化 ---
    // 修复：持久化前再次校验 run_id，防止 stale job 在通过初始校验后
    // 继续跑完后续阶段（新上传可能已更新 run_id）
    kb.ensure_document_run_current(doc_id, run_id)?;
    report_progress(
        on_progress,
        "persist",
        &format!("持久化 {} 个节点", nodes.len()),
    );
    // 修复：将 persist_nodes_and_vectors 和 update_document_stats_with_run_guard
    // 合入同一事务，消除两者之间的竞态窗口。旧代码先单独提交 nodes（事务 1），
    // 再单独更新 stats（事务 2）。若新上传在两者之间更新 processing_run_id，
    // stats 会失败（run_guard 拒绝），但旧 run 的 nodes 已经留下。
    // 合入同一事务后，如果 stats 的 run_guard 检查失败，整个事务回滚，
    // nodes 也不会留下。
    {
        let emb_status = if embedding_failed {
            Some(STATUS_FAILED)
        } else if embedding_skipped {
            Some(STATUS_SKIPPED)
        } else {
            None
        };
        let tx = conn.unchecked_transaction()?;
        let result = (|| -> Result<()> {
            persist_nodes_and_vectors_inner(conn, doc_id, lib_id, &nodes, Some(run_id))?;
            // 修复：外层已开事务，调用 _inner 避免嵌套 BEGIN（SQLite 不支持）
            kb.update_document_stats_with_run_guard_inner(
                doc_id,
                word_total,
                split_total,
                emb_status,
                Some(run_id),
            )?;
            Ok(())
        })();
        match result {
            Ok(_) => tx.commit()?,
            Err(e) => {
                let _ = tx.rollback();
                return Err(e);
            }
        }
    }

    // 同步内联 OCR 状态延迟更新：仅在节点持久化成功后才标记 OCR 完成，
    // 防止节点写入失败时残留 ocr_status=done。
    if ext == "pdf"
        && parsed
            .metadata
            .get("needs_ocr")
            .and_then(|v| v.parse::<bool>().ok())
            .unwrap_or(false)
    {
        let pdf_total: i32 = parsed
            .metadata
            .get("total_pages")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        crate::kb::ocr::update_document_ocr_status(conn, doc_id, pdf_total, Some(run_id))?;
    }

    report_progress(
        on_progress,
        "done",
        &format!("文档处理完成: {} 个节点", nodes.len()),
    );

    Ok(ProcessResult {
        word_total,
        split_total,
        deferred_ocr: false,
    })
}

// ---------------------------------------------------------------------------
// 目录批量导入
// ---------------------------------------------------------------------------

/// 批量导入目录中所有支持的文件
///
/// 递归遍历目录, 解析每个支持的文件, 并通过 5 阶段管道处理。
/// 返回成功处理的文档数量。
pub async fn ingest_directory(
    conn: &Connection,
    library_id: i64,
    folder_id: Option<i64>,
    dir_path: &Path,
    embedder: Option<Arc<Embedder>>,
    raptor_config: Option<&RaptorConfig>,
    on_progress: Option<&ProgressCallback>,
) -> Result<usize> {
    if !dir_path.is_dir() {
        return Err(GBrainError::FileError(format!(
            "不是目录: {}",
            dir_path.display()
        )));
    }

    let supported_extensions: &[&str] = &["pdf", "docx", "xlsx", "csv", "html", "htm", "txt", "md"];

    let mut files: Vec<std::path::PathBuf> = Vec::new();
    collect_supported_files(dir_path, supported_extensions, &mut files);

    if files.is_empty() {
        report_progress(on_progress, "done", "未找到支持的文件");
        return Ok(0);
    }

    report_progress(
        on_progress,
        "ingest",
        &format!("找到 {} 个文件待导入", files.len()),
    );

    let mut success_count = 0usize;
    for (i, file_path) in files.iter().enumerate() {
        let original_name = file_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("file_{}", i));

        report_progress(
            on_progress,
            "ingest",
            &format!("[{}/{}] {}", i + 1, files.len(), original_name),
        );

        // 为每个文件构造 KbProcessPayload
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("txt")
            .to_string();

        let run_id = crate::kb::jobs::new_run_id();

        // Create document record before calling the pipeline
        let kb = KbEngine::new(conn);
        let file_data = match std::fs::read(file_path) {
            Ok(data) => data,
            Err(e) => {
                report_progress(
                    on_progress,
                    "ingest",
                    &format!("无法读取 {}: {}", original_name, e),
                );
                continue;
            }
        };
        let content_hash = {
            use sha2::{Digest, Sha256};
            hex::encode(Sha256::digest(&file_data))
        };
        let name_tokens = crate::nlp::chinese::tokenize_name(&original_name);
        let doc = Document {
            library_id,
            folder_id,
            original_name: original_name.clone(),
            name_tokens,
            file_size: file_data.len() as i64,
            content_hash,
            extension: ext.clone(),
            mime_type: format!("text/{}", ext),
            source_type: "ingest".to_string(),
            storage_path: file_path.to_string_lossy().to_string(),
            original_path: file_path.to_string_lossy().to_string(),
            job_id: String::new(),
            processing_run_id: run_id.clone(),
            ..Default::default()
        };
        let doc_id = match kb.create_document(&doc) {
            Ok(id) => id,
            Err(e) => {
                report_progress(
                    on_progress,
                    "ingest",
                    &format!("无法创建文档记录 {}: {}", original_name, e),
                );
                continue;
            }
        };

        let payload = KbProcessPayload {
            kind: "kb_process_document".to_string(),
            document_id: doc_id,
            library_id,
            processing_run_id: run_id,
            storage_path: file_path.to_string_lossy().to_string(),
            extension: ext,
        };

        match process_document_async(
            conn,
            &payload,
            embedder.clone(),
            raptor_config,
            None,
            None,
            None,
            on_progress,
        )
        .await
        {
            Ok(_) => success_count += 1,
            Err(e) => {
                report_progress(
                    on_progress,
                    "ingest",
                    &format!("失败 {}: {}", original_name, e),
                );
            }
        }
    }

    report_progress(
        on_progress,
        "done",
        &format!("已导入 {}/{} 个文件", success_count, files.len()),
    );

    Ok(success_count)
}

/// 递归收集支持扩展名的文件
fn collect_supported_files(dir: &Path, extensions: &[&str], files: &mut Vec<std::path::PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_supported_files(&path, extensions, files);
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if extensions.contains(&ext.to_lowercase().as_str()) {
                    files.push(path);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 持久化与嵌入辅助函数
// ---------------------------------------------------------------------------

/// 在单个逻辑事务中持久化文档节点及其向量
pub fn persist_nodes_and_vectors(
    conn: &Connection,
    doc_id: i64,
    lib_id: i64,
    nodes: &[RaptorNode],
    run_id: Option<&str>,
) -> Result<()> {
    // 修复：如果提供了 run_id，在删除旧节点前校验 processing_run_id 仍匹配。
    // 这把 run 校验和写入放进同一事务，消除竞态窗口：
    // 旧 job 通过 ensure_document_run_current 后，新 run 可能立刻更新 processing_run_id，
    // 旧 job 的 persist 仍会删除新 run 的节点。条件删除确保 run_id 不匹配时操作被拒绝。
    let tx = conn.unchecked_transaction()?;
    let result = persist_nodes_and_vectors_inner(conn, doc_id, lib_id, nodes, run_id);
    match result {
        Ok(_) => tx.commit()?,
        Err(_) => {
            let _ = tx.rollback();
        }
    }
    result
}

/// persist_nodes_and_vectors 的内部实现，不自带事务，可在外层事务内调用。
/// 修复：将 persist 和 stats 合入同一事务时，需要此内部版本避免嵌套事务。
pub(crate) fn persist_nodes_and_vectors_inner(
    conn: &Connection,
    doc_id: i64,
    _lib_id: i64,
    nodes: &[RaptorNode],
    run_id: Option<&str>,
) -> Result<()> {
    // 修复：如果提供了 run_id，校验 processing_run_id 仍匹配，防止 stale job 覆盖新 run 的节点
    if let Some(rid) = run_id {
        let current_run_id: String = conn
            .query_row(
                "SELECT processing_run_id FROM kb_documents WHERE id = ?1",
                [doc_id],
                |row| row.get(0),
            )
            .map_err(|e| GBrainError::Database(format!("查询 processing_run_id 失败: {}", e)))?;
        if current_run_id != rid {
            return Err(GBrainError::InvalidInput(
                "stale KB processing job; document has a newer run (persist 阶段)".to_string(),
            ));
        }
    }

    // 删除此文档的旧节点/向量 (内联操作, 避免嵌套事务)
    {
        let node_ids: Vec<i64> = {
            let mut stmt =
                conn.prepare("SELECT id FROM kb_document_nodes WHERE document_id = ?1")?;
            let rows = stmt.query_map([doc_id], |row| row.get(0))?;
            rows.filter_map(|r| r.ok()).collect()
        };

        // FIX12-03: 复用 engine 的 cleanup_node_vectors，统一清理 vec 表 + kb_node_embeddings，
        // 之前的内联逻辑用 let _ 吞掉所有删除失败，文档重处理时可能留下孤立向量数据
        for &node_id in &node_ids {
            crate::kb::engine::cleanup_node_vectors(conn, node_id);
        }

        conn.execute(
            "DELETE FROM kb_document_nodes WHERE document_id = ?1",
            [doc_id],
        )?;
    }

    // 按 level ASC, chunk_order ASC 排序插入, 确保父节点先于子节点
    let mut sorted_nodes: Vec<&RaptorNode> = nodes.iter().collect();
    sorted_nodes.sort_by_key(|n| (n.level, n.chunk_order));

    // 临时内存 ID → 数据库行 ID 映射
    let mut id_map: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();

    for node in &sorted_nodes {
        let content_tokens = chinese::tokenize_content(&node.content);

        conn.execute(
            "INSERT INTO kb_document_nodes \
             (library_id, document_id, content, content_tokens, level, chunk_order, \
              title_path, page_number, source_start, source_end, node_metadata, embedding_text) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                node.library_id,
                doc_id,
                node.content,
                content_tokens,
                node.level,
                node.chunk_order,
                node.title_path,
                node.page_number,
                node.source_start,
                node.source_end,
                node.node_metadata,
                node.embedding_text,
            ],
        )?;

        let db_id = conn.last_insert_rowid();
        id_map.insert(node.id, db_id);
    }

    // 更新 parent_id 关系 (临时 ID → 数据库 ID)
    for node in &sorted_nodes {
        if let Some(parent_temp_id) = node.parent_id {
            if let Some(&parent_db_id) = id_map.get(&parent_temp_id) {
                let &db_id = id_map.get(&node.id).ok_or_else(|| {
                    GBrainError::Database(format!(
                        "节点临时 ID {} 在 id_map 中不存在 (插入后应存在)",
                        node.id
                    ))
                })?;
                conn.execute(
                    "UPDATE kb_document_nodes SET parent_id = ?1 WHERE id = ?2",
                    rusqlite::params![parent_db_id, db_id],
                )?;
            }
        }
    }

    // 将向量写入 BLOB 回退表和 per-index vec 虚表（使用统一函数）
    for node in &sorted_nodes {
        if let Some(ref vector) = node.vector {
            let &db_id = id_map.get(&node.id).ok_or_else(|| {
                GBrainError::Database(format!(
                    "节点临时 ID {} 在 id_map 中不存在 (插入后应存在)",
                    node.id
                ))
            })?;

            // 解析该节点所属 library 的 active embedding index
            let active_index_id: i64 = conn
                .query_row(
                    "SELECT ei.id FROM kb_embedding_indexes ei \
                 INNER JOIN kb_document_nodes dn ON dn.library_id = ei.library_id \
                 WHERE dn.id = ?1 AND ei.is_active = 1 LIMIT 1",
                    rusqlite::params![db_id],
                    |row| row.get(0),
                )
                .map_err(|_| {
                    GBrainError::InvalidInput(format!(
                        "节点 {} 所属 library 没有 active embedding index",
                        db_id
                    ))
                })?;

            // 统一写入（BLOB 表 + per-index vec 表）
            crate::kb::embedding_index::upsert_node_embedding_for_index(
                conn,
                db_id,
                active_index_id,
                vector,
                vector.len() as i32,
                "text-embedding-3-large",
            )?;

            // 向后兼容：同步写入 legacy vec_kb_nodes
            let blob = embedding_to_blob(vector);
            let _ = conn.execute(
                "DELETE FROM vec_kb_nodes WHERE node_id = ?1",
                rusqlite::params![db_id],
            );
            let _ = conn.execute(
                "INSERT INTO vec_kb_nodes (node_id, embedding) VALUES (?1, ?2)",
                rusqlite::params![db_id, &blob],
            );
        }
    }

    Ok(())
}

/// 将 f32 嵌入向量转换为小端序 BLOB 用于 SQLite 存储
pub fn embedding_to_blob(vector: &[f32]) -> Vec<u8> {
    vector.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Count words in text, using jieba tokenization for Chinese content
/// and whitespace splitting for other text.
fn count_words(text: &str) -> usize {
    if crate::nlp::chinese::has_chinese(text) {
        let tokens = crate::nlp::chinese::tokenize_content(text);
        tokens.split_whitespace().count()
    } else {
        text.split_whitespace().count()
    }
}

/// 报告进度 (如果提供了回调)
fn report_progress(on_progress: Option<&ProgressCallback>, phase: &str, message: &str) {
    if let Some(cb) = on_progress {
        cb(phase, message);
    }
}

// ---------------------------------------------------------------------------
// PDF OCR 分支（Phase 1+2+3）
// ---------------------------------------------------------------------------

/// PDF OCR 分支入口：检测 → 规划 → 执行 OCR → 合并文本
///
/// 在 parse 后、split 前调用。行为：
/// 1. 读取 parsed.metadata.page_analyses 并运行 OcrDetector
/// 2. 如果 ocr_scope=none，更新 ocr_status=not_needed，返回原 parsed
/// 3. 检查 OCR 是否启用、外部 OCR 是否允许
/// 4. OCR 或外部 OCR 被关闭时，更新 ocr_status=needed，返回原 parsed
/// 5. 同步内联模式：规划 → 调用 GLM-OCR → 规范化 → 合并 → 返回新 parsed
/// 6. 异步模式：入队 kb_ocr_document job，返回 None 阻断后续本地索引
#[allow(clippy::too_many_arguments)]
fn maybe_apply_pdf_ocr(
    conn: &Connection,
    doc_id: i64,
    lib_id: i64,
    run_id: &str,
    storage_path: &str,
    parsed: &ParsedDocument,
    library: &crate::kb::types::Library,
    file_data: &[u8],
    config: &crate::config::Config,
) -> Result<Option<ParsedDocument>> {
    use crate::kb::ocr::OcrStatus;
    use crate::kb::ocr_detector::{detect_ocr_pages, PdfImageRegion, PdfPageAnalysis};
    use crate::kb::ocr_provider::{OcrMode, OcrSubmitMode};

    let ocr_enabled = config.ocr_enabled;
    let external_allowed = library.external_ocr_allowed;

    // 从 metadata 中读取页级分析
    let page_analyses_raw = parsed
        .metadata
        .get("page_analyses")
        .and_then(|v| serde_json::from_str::<Vec<serde_json::Value>>(v).ok())
        .unwrap_or_default();

    let total_pages: usize = parsed
        .metadata
        .get("total_pages")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    if total_pages == 0 || page_analyses_raw.is_empty() {
        // 无法确认 PDF 页数或页分析结果为空，标记 failed 而非 not_needed
        // not_needed 意味着明确不需要 OCR，但页数为 0 可能是解析异常
        let kb = KbEngine::new(conn);
        kb.update_document_ocr_with_run_guard(
            doc_id,
            OcrStatus::Failed.as_str(),
            0.0,
            Some(run_id),
        )?;
        return Ok(Some(parsed.clone()));
    }

    // 转换为 PdfPageAnalysis
    let page_analyses: Vec<PdfPageAnalysis> = page_analyses_raw
        .iter()
        .map(|pa| {
            let page_number = pa.get("page_number").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let text = pa
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let char_count = pa.get("char_count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let image_area_ratio = pa
                .get("image_area_ratio")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let image_count = pa.get("image_count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let has_vector = pa
                .get("has_vector_or_unknown_objects")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let image_regions: Vec<PdfImageRegion> = if image_count > 0 {
                (0..image_count)
                    .map(|_| PdfImageRegion {
                        bbox: None,
                        area_ratio: image_area_ratio / image_count as f64,
                    })
                    .collect()
            } else if image_area_ratio > 0.0 {
                vec![PdfImageRegion {
                    bbox: None,
                    area_ratio: image_area_ratio,
                }]
            } else {
                vec![]
            };

            PdfPageAnalysis {
                page_number,
                text,
                text_blocks: vec![],
                char_count,
                image_regions,
                image_area_ratio,
                has_vector_or_unknown_objects: has_vector,
                width: pa.get("width").and_then(|v| v.as_u64()).map(|v| v as u32),
                height: pa.get("height").and_then(|v| v.as_u64()).map(|v| v as u32),
                content_parse_failed: pa
                    .get("content_parse_failed")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                has_vector_drawing_ops: pa
                    .get("has_vector_drawing_ops")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                has_invisible_text: pa
                    .get("has_invisible_text")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                font_encoding_suspected: pa
                    .get("font_encoding_suspected")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            }
        })
        .collect();

    // 运行 OCR 检测
    let ocr_mode = OcrMode::from_str(&config.ocr_mode);
    let mut detection = detect_ocr_pages(
        &page_analyses,
        config.ocr_text_density_threshold,
        config.ocr_image_area_threshold,
        config.ocr_image_count_threshold,
        config.ocr_min_low_density_ratio,
        &ocr_mode,
    );

    let kb = KbEngine::new(conn);

    let reasons_json: std::collections::BTreeMap<String, Vec<String>> = detection
        .reasons_by_page
        .iter()
        .map(|(page, reasons)| {
            let labels: Vec<String> = reasons
                .iter()
                .map(|r| {
                    serde_json::to_value(r)
                        .ok()
                        .and_then(|v| v.as_str().map(|s| s.to_string()))
                        .unwrap_or_default()
                })
                .collect();
            (page.to_string(), labels)
        })
        .collect();
    let needs_ocr_pages_str =
        serde_json::to_string(&detection.ocr_pages).unwrap_or_else(|_| "[]".to_string());
    let reasons_str =
        serde_json::to_string(&reasons_json).unwrap_or_else(|_| "{}".to_string());
    let ocr_scope = match detection.scope() {
        crate::kb::ocr_detector::OcrScope::None => "none",
        crate::kb::ocr_detector::OcrScope::Partial => "partial",
        crate::kb::ocr_detector::OcrScope::Full => "full",
    };

    // 在处理分支前统一持久化 OCR 检测元数据到文档 metadata_json，
    // 确保所有路径（not_needed、策略阻断、失败、inline OCR、异步 OCR）的状态接口
    // 均可查询 needs_ocr_pages、ocr_reasons_by_page、ocr_scope。
    conn.execute(
        "UPDATE kb_documents SET metadata_json = json_set(COALESCE(metadata_json, '{}'), \
         '$.needs_ocr_pages', json(?2), '$.ocr_reasons_by_page', json(?3), '$.ocr_scope', ?4), \
         updated_at = datetime('now') WHERE id = ?1 AND processing_run_id = ?5",
        rusqlite::params![doc_id, needs_ocr_pages_str, reasons_str, ocr_scope, run_id],
    )?;

    // 如果不需要 OCR
    if !detection.needs_ocr {
        tracing::info!(doc_id, "PDF OCR 检测: 不需要 OCR (ocr_scope=none)");
        kb.update_document_ocr_with_run_guard(
            doc_id,
            OcrStatus::NotNeeded.as_str(),
            0.0,
            Some(run_id),
        )?;
        return Ok(Some(parsed.clone()));
    }

    tracing::info!(
        doc_id,
        ocr_pages = ?detection.ocr_pages,
        "PDF OCR 检测: 需要 OCR"
    );

    // 限制 OCR 页数上限
    let max_pages = config.ocr_max_pages_per_document.max(1);
    if detection.ocr_pages.len() > max_pages {
        tracing::warn!(
            doc_id,
            ocr_page_count = detection.ocr_pages.len(),
            max_pages,
            "OCR 页数超过单文档上限，截断为前 {} 页",
            max_pages
        );
        // 超出上限的页标记为 skipped，确保有可见状态
        let skipped_pages: Vec<i32> = detection.ocr_pages[max_pages..].to_vec();
        crate::kb::ocr::update_ocr_pages_status(
            conn,
            doc_id,
            &skipped_pages,
            "skipped",
            &format!("超出单文档 OCR 页数上限 ({})", max_pages),
            "glm_ocr",
            &config.ocr_model,
            run_id,
        )?;
        tracing::info!(
            doc_id,
            skipped_count = skipped_pages.len(),
            "已将超出上限的 {} 页标记为 skipped",
            skipped_pages.len()
        );
        detection.ocr_pages.truncate(max_pages);
        detection.needs_ocr = !detection.ocr_pages.is_empty();
    }

    // 计算文档总页数，供后续分支的 OCR 状态聚合使用
    let doc_page_count: i32 = parsed
        .metadata
        .get("total_pages")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    // 检查 OCR 是否启用和外部 OCR 是否允许
    if !ocr_enabled || !external_allowed {
        tracing::warn!(
            doc_id,
            ocr_enabled,
            external_allowed,
            "PDF 需要 OCR 但 OCR 或外部 OCR 被关闭"
        );
        let reason = if !ocr_enabled && !external_allowed {
            "全局 OCR 和库外部 OCR 均已关闭"
        } else if !ocr_enabled {
            "全局 OCR 已关闭 (GBRAIN_OCR_ENABLED=false)"
        } else {
            "库策略已关闭外部 OCR"
        };
        crate::kb::ocr::update_ocr_pages_status(
            conn,
            doc_id,
            &detection.ocr_pages,
            "needed",
            reason,
            "glm_ocr",
            &config.ocr_model,
            run_id,
        )?;
        // 通过聚合函数更新文档 OCR 状态，确保 skipped + needed → partial
        crate::kb::ocr::update_document_ocr_status(conn, doc_id, doc_page_count, Some(run_id))?;
        return Ok(Some(parsed.clone()));
    }

    // 检查隐私策略：redaction_enabled 时禁止外部 OCR
    if library.redaction_enabled {
        tracing::warn!(doc_id, "PDF 需要 OCR 但 library 启用了脱敏，禁止外部 OCR");
        crate::kb::ocr::update_ocr_pages_status(
            conn,
            doc_id,
            &detection.ocr_pages,
            "needed",
            "库已启用脱敏，禁止外部 OCR",
            "glm_ocr",
            &config.ocr_model,
            run_id,
        )?;
        // 通过聚合函数更新文档 OCR 状态，确保 skipped + needed → partial
        crate::kb::ocr::update_document_ocr_status(conn, doc_id, doc_page_count, Some(run_id))?;
        return Ok(Some(parsed.clone()));
    }

    // 检查 API key
    if config.ocr_api_key.is_none() {
        tracing::error!(
            doc_id,
            "PDF 需要 OCR 但未配置 GBRAIN_OCR_API_KEY 或 ZHIPU_API_KEY"
        );
        crate::kb::ocr::update_ocr_pages_status(
            conn,
            doc_id,
            &detection.ocr_pages,
            "failed",
            "未配置 OCR API key (GBRAIN_OCR_API_KEY 或 ZHIPU_API_KEY)",
            "glm_ocr",
            &config.ocr_model,
            run_id,
        )?;
        // 通过聚合函数更新文档 OCR 状态，确保 skipped + failed → partial
        crate::kb::ocr::update_document_ocr_status(conn, doc_id, doc_page_count, Some(run_id))?;
        // 将错误同步写入文档级 parsing_error，确保状态接口可展示失败原因
        kb.update_document_status_with_run_guard(
            doc_id,
            None,
            None,
            Some("未配置 OCR API key (GBRAIN_OCR_API_KEY 或 ZHIPU_API_KEY)"),
            None,
            None,
            None,
            Some(run_id),
        )?;
        return Ok(Some(parsed.clone()));
    }

    // 异步模式：入队 kb_ocr_document job，返回 None 阻断后续 split/embed/persist
    if !config.ocr_sync_inline {
        tracing::info!(doc_id, "PDF OCR: 入队异步 OCR job");

        // 主 pipeline 会在异步 OCR 入队后提前返回，因此这里必须持久化
        // PDF 页数；否则 kb_ocr_retry 的严格页码校验拿不到总页数。
        // 这些文档状态、页状态和 job 入队必须同事务提交，否则 OCR worker
        // 可能在 job 可见后立刻跑完，又被外层/后续状态更新覆盖回 queued/ocr_pending。
        let char_count = parsed.content.chars().count();
        let granularity = crate::kb::granularity::classify_granularity(
            "pdf",
            char_count,
            total_pages,
        );
        let chunk_strategy = crate::kb::granularity::chunk_strategy_for(granularity);

        // 入队 OCR job
        let ocr_payload = crate::kb::jobs::KbOcrPayload {
            kind: "kb_ocr_document".to_string(),
            document_id: doc_id,
            library_id: lib_id,
            processing_run_id: run_id.to_string(),
            storage_path: storage_path.to_string(),
            pages: detection.ocr_pages.clone(),
            submit_mode: config.ocr_submit_mode.clone(),
            provider: "glm_ocr".to_string(),
            model: config.ocr_model.clone(),
            return_crop_images: config.ocr_return_crop_images,
            need_layout_visualization: config.ocr_need_layout_visualization,
        };

        let ocr_payload_json = serde_json::to_value(&ocr_payload)
            .map_err(|e| GBrainError::Serialization(e.to_string()))?;

        let tx = conn.unchecked_transaction()?;
        let enqueue_result = (|| -> Result<i64> {
            let rows = conn.execute(
                "UPDATE kb_documents SET document_granularity = ?1, chunk_strategy = ?2, \
                 content_char_count = ?3, page_count = ?4, parsing_status = ?5, \
                 parsing_progress = ?6, document_status = 'ocr_pending', \
                 ocr_status = ?7, ocr_text_coverage = 0.0, updated_at = datetime('now') \
                 WHERE id = ?8 AND processing_run_id = ?9",
                rusqlite::params![
                    granularity.as_str(),
                    chunk_strategy,
                    char_count as i32,
                    total_pages as i32,
                    STATUS_PROCESSING,
                    30,
                    OcrStatus::Queued.as_str(),
                    doc_id,
                    run_id
                ],
            )?;
            if rows == 0 {
                return Err(GBrainError::InvalidInput(
                    "文档 processing_run_id 已变化，跳过异步 OCR 入队".to_string(),
                ));
            }

            // 为每个 OCR 页创建 pending 状态记录，并持久化该页的 OCR 检测原因。
            // CLI 状态查询（kb ocr-status）读页表，写入原因后用户可看到"为什么该页需要 OCR"。
            for &page_num in &detection.ocr_pages {
                let page_reasons = detection
                    .reasons_by_page
                    .get(&page_num)
                    .map(|reasons| {
                        let labels: Vec<String> = reasons
                            .iter()
                            .filter_map(|r| serde_json::to_value(r).ok())
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                        labels.join(", ")
                    })
                    .unwrap_or_default();
                let reason_desc = if page_reasons.is_empty() {
                    "等待异步 OCR 处理".to_string()
                } else {
                    format!("OCR 原因: {} | 等待异步 OCR 处理", page_reasons)
                };
                crate::kb::ocr::update_ocr_page_status(
                    conn,
                    doc_id,
                    page_num,
                    "pending",
                    &reason_desc,
                    "glm_ocr",
                    &config.ocr_model,
                    run_id,
                )?;
            }

            let queue = crate::jobs::JobQueue::new(conn);
            queue.enqueue(crate::jobs::JobInput {
                job_type: "kb_ocr_document".to_string(),
                payload: ocr_payload_json,
                priority: Some(0),
                max_attempts: Some(3),
            })
        })();
        match enqueue_result {
            Ok(_) => tx.commit()?,
            Err(e) => {
                let _ = tx.rollback();
                return Err(e);
            }
        };

        // 异步 OCR 入队成功：返回 None 阻断后续 split/embed/persist
        // 文档状态保持 ocr_status=queued，document_status 不会被标为 ready
        // OCR 完成后由 OCR worker 调用 writeback_ocr_results 完成后续流程
        return Ok(None);
    }

    // 同步内联模式：直接执行 OCR
    tracing::info!(doc_id, "PDF OCR: 同步内联执行 OCR");
    kb.update_document_ocr_with_run_guard(
        doc_id,
        OcrStatus::Processing.as_str(),
        0.0,
        Some(run_id),
    )?;

    match execute_inline_ocr(
        conn,
        doc_id,
        lib_id,
        run_id,
        &page_analyses,
        &detection,
        file_data,
        total_pages as i32,
        &config,
    ) {
        Ok(merged_results) => {
            // 用合并后的内容替换原 parsed 的 content
            let merged_content = crate::kb::ocr_merge::merged_results_to_content(&merged_results);
            let mut new_parsed = parsed.clone();
            new_parsed.content = merged_content;

            // 修复：从合并结果重建 blocks，使后续 split 的页码/source span 匹配正确。
            // 原代码只更新 content 不重建 blocks，导致 split 用新 OCR 正文切块时
            // 仍用原 PDF 文本层 blocks 做页码/span 匹配，扫描页容易得到错误页码。
            new_parsed.blocks = Some(build_blocks_from_merged(&merged_results));

            // 更新 page_texts metadata（从 page_analyses 重新生成）
            let page_texts: Vec<String> = page_analyses.iter().map(|pa| pa.text.clone()).collect();
            new_parsed.metadata.insert(
                "page_texts".to_string(),
                serde_json::to_string(&page_texts).unwrap_or_default(),
            );
            new_parsed
                .metadata
                .insert("needs_ocr".to_string(), detection.needs_ocr.to_string());
            new_parsed.metadata.insert(
                "ocr_pages".to_string(),
                serde_json::to_string(&detection.ocr_pages).unwrap_or_default(),
            );
            new_parsed.metadata.insert(
                "ocr_reasons_by_page".to_string(),
                serde_json::to_string(&reasons_json).unwrap_or_default(),
            );

            // 聚合 OCR 页状态，判断是否有成功结果。
            // execute_inline_ocr 在所有 chunk 都失败时仍返回 Ok，
            // 因此必须在此处根据实际页状态决定是否清空文档级错误。
            // 只读聚合：仅在节点持久化前判断是否需要清空文档错误，
            // 不写入 ocr_status，防止后续 split/节点写入失败时残留错误终态。
            let (final_ocr_status, _coverage) = crate::kb::ocr::compute_ocr_status(
                conn,
                doc_id,
                total_pages as i32,
                Some(run_id),
            )?;
            if final_ocr_status == OcrStatus::Failed {
                // 全失败：将脱敏后的错误写入文档级 parsing_error
                let failed_error = "同步内联 OCR 全部页面处理失败";
                kb.update_document_status_with_run_guard(
                    doc_id,
                    None,
                    None,
                    Some(failed_error),
                    None,
                    None,
                    None,
                    Some(run_id),
                )?;
            } else {
                // 存在成功结果：清除可能由上一次失败写入的文档级错误
                kb.update_document_status_with_run_guard(
                    doc_id,
                    None,
                    None,
                    Some(""),
                    None,
                    None,
                    None,
                    Some(run_id),
                )?;
            }

            Ok(Some(new_parsed))
        }
        Err(e) => {
            let safe_error = crate::kb::ocr::sanitize_error_text_with_secret(
                &e.to_string(),
                config.ocr_api_key.as_deref(),
            );
            tracing::error!(doc_id, error = %safe_error, "PDF OCR 执行失败");
            kb.update_document_ocr_with_run_guard(
                doc_id,
                OcrStatus::Failed.as_str(),
                0.0,
                Some(run_id),
            )?;
            // 全失败时将脱敏后的错误写入文档级 parsing_error
            kb.update_document_status_with_run_guard(
                doc_id,
                None,
                None,
                Some(&safe_error),
                None,
                None,
                None,
                Some(run_id),
            )?;
            // OCR 失败时返回原始 parsed，不阻塞后续文本层索引
            Ok(Some(parsed.clone()))
        }
    }
}

/// 从 OCR 合并结果构建 ParsedBlock 列表，offset 与 merged_results_to_content 输出对齐
///
/// 每个非空页生成一个 ParsedBlock，source_start/source_end 基于
/// `[PAGE:N]\n{text}` 格式在全文中的字符偏移计算，确保后续 split
/// 的 span 匹配能正确定位页码。
fn build_blocks_from_merged(merged: &[crate::kb::ocr_merge::MergedPageResult]) -> Vec<ParsedBlock> {
    let mut blocks = Vec::new();
    let mut offset = 0i32;
    for page in merged {
        if page.text.trim().is_empty() {
            continue;
        }
        let marker = format!("[PAGE:{}]\n", page.page_number);
        let entry_chars = marker.chars().count() as i32 + page.text.chars().count() as i32;
        blocks.push(ParsedBlock {
            text: page.text.clone(),
            title_path: String::new(),
            page_number: Some(page.page_number),
            source_start: Some(offset),
            source_end: Some(offset + entry_chars),
            block_type: if page.used_ocr { "ocr_text" } else { "text" }.to_string(),
            metadata: String::new(),
        });
        offset += entry_chars + 2; // +2 for "\n\n" between pages
    }
    blocks
}

/// 同步内联执行 OCR：规划 → 调用 GLM-OCR → 合并文本
#[allow(clippy::too_many_arguments)]
fn execute_inline_ocr(
    conn: &Connection,
    doc_id: i64,
    lib_id: i64,
    run_id: &str,
    page_analyses: &[crate::kb::ocr_detector::PdfPageAnalysis],
    detection: &crate::kb::ocr_detector::OcrDetection,
    file_data: &[u8],
    total_pages: i32,
    config: &crate::config::Config,
) -> Result<Vec<crate::kb::ocr_merge::MergedPageResult>> {
    use crate::kb::ocr_glm::{build_ocr_options_from_config, pdf_to_base64, GlmOcrProvider};
    use crate::kb::ocr_merge::merge_text_and_ocr;
    use crate::kb::ocr_planner::plan_ocr_requests;
    use crate::kb::ocr_provider::{OcrFilePayload, OcrInput, OcrProvider, OcrSubmitMode};

    let options = build_ocr_options_from_config(config);
    let submit_mode = OcrSubmitMode::from_str(&config.ocr_submit_mode);

    // 创建临时目录：包含 run_id 避免同一文档的并发 job 相互覆盖/删除
    // 使用 RAII 守卫确保任何退出路径都自动清理
    let mut temp_guard = crate::kb::temp_guard::TempOcrDir::create(
        &format!("gbrain_ocr_{}_{}", doc_id, run_id),
        file_data.len() as u64,
        config.ocr_temp_dir_max_bytes,
    )?;

    // 规划 OCR 请求（内部按实际输出文件字节数申请临时目录预算）
    let plan = plan_ocr_requests(
        doc_id,
        "",
        file_data,
        total_pages,
        &detection.ocr_pages,
        options.max_pages_per_request,
        options.max_pdf_bytes_per_request,
        &submit_mode,
        config.ocr_temp_dir_max_bytes,
        &mut temp_guard,
    )?;

    // 执行每个请求块
    let api_key = config.ocr_api_key.as_deref().unwrap_or("");
    let provider = GlmOcrProvider::new(api_key);
    let mut all_ocr_results: Vec<crate::kb::ocr_provider::OcrPageResult> = Vec::new();

    for chunk in &plan.chunks {
        // 拆分失败的页段：跳过 OCR 请求，直接标记为 failed
        if chunk.split_failed {
            tracing::warn!(
                doc_id,
                start = chunk.source_start_page,
                end = chunk.source_end_page,
                "同步 OCR: 页段拆分失败，跳过 OCR 并标记为 failed"
            );
            // 持久化失败时向上返回错误，避免静默产生不可信状态
            for page_num in chunk.source_start_page..=chunk.source_end_page {
                crate::kb::ocr::update_ocr_page_status(
                    conn,
                    doc_id,
                    page_num,
                    "failed",
                    "PDF 页段拆分失败，无法提交 OCR",
                    "glm_ocr",
                    &config.ocr_model,
                    run_id,
                )?;
            }
            continue;
        }

        // 准备输入
        let file_payload = if let Some(ref split_path) = chunk.split_pdf_path {
            // 使用拆分后的 PDF 子文件
            let split_data = std::fs::read(split_path)?;
            OcrFilePayload::Base64(pdf_to_base64(&split_data))
        } else {
            // 使用原始 PDF
            OcrFilePayload::Base64(pdf_to_base64(file_data))
        };

        let input = OcrInput::PdfRange {
            file: file_payload,
            request_start_page_id: chunk.request_start_page_id,
            request_end_page_id: chunk.request_end_page_id,
            source_start_page: chunk.source_start_page,
            source_end_page: chunk.source_end_page,
            document_id: doc_id,
            run_id: run_id.to_string(),
        };

        // 指数退避重试：对可重试错误（429/503/timeout/网络）重试最多 3 次
        let mut retry_count = 0u32;
        loop {
            // 获取 OCR 并发许可（与异步 worker 共享同一信号量），
            // 防止同步内联路径绕过 ocr_max_concurrency 限制
            let _ocr_permit = match crate::kb::worker::try_acquire_ocr_permit(
                config.ocr_max_concurrency,
                std::time::Duration::from_secs(30),
            ) {
                Some(permit) => permit,
                None => {
                    retry_count += 1;
                    if retry_count < 3 {
                        let backoff_secs = 2u64 * 2u64.pow(retry_count - 1);
                        tracing::warn!(
                            doc_id,
                            retry_count,
                            backoff_secs,
                            "同步 OCR: 并发槽位已满，等待后重试"
                        );
                        std::thread::sleep(std::time::Duration::from_secs(backoff_secs));
                        continue;
                    }
                    return Err(GBrainError::Config(
                        "OCR 并发槽位已满，同步内联 OCR 超时放弃".to_string(),
                    ));
                }
            };

            let call_started = std::time::Instant::now();
            let recognition = provider.recognize(&input, &options);
            let latency_ms = call_started.elapsed().as_millis().min(i32::MAX as u128) as i32;

            match recognition {
                Ok(results) => {
                    let results =
                        crate::kb::ocr::sanitize_ocr_page_results(&results, Some(api_key));
                    crate::kb::ocr::log_ocr_external_model_call(
                        conn,
                        lib_id,
                        doc_id,
                        "glm_ocr",
                        &config.ocr_model,
                        latency_ms,
                        true,
                        "",
                        &results,
                        Some(api_key),
                    );
                    // 持久化页级和块级结果
                    // 持久化 OCR 结果失败时向上返回错误，避免页面结果丢失但文档显示完成
                    if !results.is_empty() {
                        crate::kb::ocr::persist_ocr_page_results(
                            conn,
                            doc_id,
                            run_id,
                            &results,
                            Some(api_key),
                        )?;
                        crate::kb::ocr::persist_ocr_blocks(
                            conn,
                            doc_id,
                            run_id,
                            &results,
                            Some(api_key),
                        )?;
                    }
                    // 标记未返回的页为 failed（处理多页请求部分失败的情况）
                    let returned_pages: std::collections::HashSet<i32> =
                        results.iter().map(|r| r.page_number).collect();
                    for page_num in chunk.source_start_page..=chunk.source_end_page {
                        if !returned_pages.contains(&page_num) {
                            crate::kb::ocr::update_ocr_page_status(
                                conn,
                                doc_id,
                                page_num,
                                "failed",
                                "OCR 请求未返回该页结果（部分失败）",
                                "glm_ocr",
                                &config.ocr_model,
                                run_id,
                            )?;
                        }
                    }
                    all_ocr_results.extend(results);
                    break;
                }
                Err(e) => {
                    let error_str = e.to_string();
                    let safe_error_str =
                        crate::kb::ocr::sanitize_error_text_with_secret(&error_str, Some(api_key));
                    crate::kb::ocr::log_ocr_external_model_call(
                        conn,
                        lib_id,
                        doc_id,
                        "glm_ocr",
                        &config.ocr_model,
                        latency_ms,
                        false,
                        &error_str,
                        &[],
                        Some(api_key),
                    );
                    let lower = error_str.to_lowercase();
                    let retryable = lower.contains("429")
                        || lower.contains("rate limit")
                        || lower.contains("503")
                        || lower.contains("timeout")
                        || lower.contains("timed out")
                        || lower.contains("connection")
                        || lower.contains("hyper");

                    if retryable && retry_count < 3 {
                        retry_count += 1;
                        let backoff_secs = 2u64 * 2u64.pow(retry_count - 1);
                        tracing::warn!(
                            doc_id,
                            start = chunk.source_start_page,
                            end = chunk.source_end_page,
                            retry = retry_count,
                            backoff_secs,
                            error = %safe_error_str,
                            "同步 OCR 请求遇到可重试错误，指数退避重试"
                        );
                        std::thread::sleep(std::time::Duration::from_secs(backoff_secs));
                        continue;
                    }

                    tracing::warn!(
                        doc_id,
                        start = chunk.source_start_page,
                        end = chunk.source_end_page,
                        error = %safe_error_str,
                        "OCR 请求失败"
                    );
                    // 标记失败页（脱敏错误信息，防止 API 响应正文泄露到数据库）
                    for page_num in chunk.source_start_page..=chunk.source_end_page {
                        crate::kb::ocr::update_ocr_page_status(
                            conn,
                            doc_id,
                            page_num,
                            "failed",
                            &safe_error_str,
                            "glm_ocr",
                            &config.ocr_model,
                            run_id,
                        )?;
                    }
                    break;
                }
            }
        }
    }

    // 合并文本层与 OCR 结果
    let merged = merge_text_and_ocr(page_analyses, &all_ocr_results, &detection.ocr_pages);

    // 注意：不在此处更新文档 OCR 状态，状态更新延迟到节点持久化完成后执行，
    // 防止 ocr_status=done 但节点写入失败导致的不一致。

    // temp_guard 在此处 drop，自动清理临时目录
    drop(temp_guard);

    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_to_blob_roundtrip() {
        let original: Vec<f32> = vec![0.1, -0.2, 0.3, 0.0, 1.0];
        let blob = embedding_to_blob(&original);

        assert_eq!(blob.len(), original.len() * 4);

        let decoded: Vec<f32> = blob
            .chunks_exact(4)
            .filter_map(|chunk| {
                let bytes: [u8; 4] = chunk.try_into().ok()?;
                Some(f32::from_le_bytes(bytes))
            })
            .collect();

        assert_eq!(decoded.len(), original.len());
        for (a, b) in original.iter().zip(decoded.iter()) {
            assert!(a - b < 1e-6);
        }
    }

    #[test]
    fn test_embedding_to_blob_empty() {
        let empty: Vec<f32> = vec![];
        let blob = embedding_to_blob(&empty);
        assert!(blob.is_empty());
    }

    #[test]
    fn test_collect_supported_files() {
        let dir = std::env::temp_dir();
        let sub = dir.join("gbrain_test_collect");
        let _ = std::fs::create_dir_all(&sub);
        let _ = std::fs::write(sub.join("test.md"), "hello");
        let _ = std::fs::write(sub.join("test.exe"), "binary");

        let mut files = Vec::new();
        collect_supported_files(&sub, &["md"], &mut files);
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("test.md"));

        let _ = std::fs::remove_dir_all(&sub);
    }

    #[test]
    fn test_locate_chunk_overlap_boundary() {
        // P2 修复：overlap_chars == char_cursor 时应允许回退到 0
        // full_text="abcdef"、chunks=["a","abcdef"]、max_overlap=1
        // 第一块 "a" 匹配后 char_cursor=1，第二块与第一块 overlap=1（'a'）
        // overlap_chars(1) == char_cursor(1)，应回退到 0，从 0 开始 find("abcdef")
        let full_text = "abcdef".to_string();
        let chunks = vec!["a".to_string(), "abcdef".to_string()];
        let offsets = locate_chunk_char_offsets(&full_text, &chunks, 1);
        assert_eq!(offsets[0], (0, 1)); // "a" at 0..1
        assert_eq!(offsets[1], (0, 6)); // "abcdef" at 0..6（回退到 0 后精确匹配）
    }

    #[test]
    fn test_locate_chunk_overlap_normal() {
        // 正常 overlap：full_text="abcdef"、chunks=["abcd","cdef"]、max_overlap=2
        let full_text = "abcdef".to_string();
        let chunks = vec!["abcd".to_string(), "cdef".to_string()];
        let offsets = locate_chunk_char_offsets(&full_text, &chunks, 2);
        assert_eq!(offsets[0], (0, 4)); // "abcd" at 0..4
        assert_eq!(offsets[1], (2, 6)); // "cdef" at 2..6（overlap=2 回退到 2）
    }

    #[test]
    fn test_locate_chunk_no_overlap() {
        // max_overlap=0 不回退
        let full_text = "abcdef".to_string();
        let chunks = vec!["abcd".to_string(), "ef".to_string()];
        let offsets = locate_chunk_char_offsets(&full_text, &chunks, 0);
        assert_eq!(offsets[0], (0, 4)); // "abcd" at 0..4
        assert_eq!(offsets[1], (4, 6)); // "ef" at 4..6（不回退）
    }
}
