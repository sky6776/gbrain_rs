//! KB hybrid search: vector KNN + FTS5 BM25 + RRF fusion
//!
//! Implements a two-retriever search pipeline:
//! 1. FTS5 keyword search using `kb_doc_fts` with jieba-tokenized query
//! 2. Vector search using sqlite-vec (with BLOB fallback via `kb_node_embeddings`)
//! 3. RRF (Reciprocal Rank Fusion) merge with k=60
//! 4. Fetch node details with document and library names
//!
//! All functions accept `&Connection` so callers control the transaction scope.

use crate::error::Result;
use crate::kb::types::*;
use crate::nlp::chinese;
use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::Mutex;

/// P4-006: 查询 embedding 缓存（按 model/dimensions 隔离）
#[allow(dead_code)]
static EMBEDDING_CACHE: std::sync::LazyLock<Mutex<crate::kb::cache::SearchCache<Vec<f32>>>> =
    std::sync::LazyLock::new(|| Mutex::new(crate::kb::cache::SearchCache::new(1000, 3600)));
/// P4-007: 查询分词缓存
#[allow(dead_code)]
static TOKENS_CACHE: std::sync::LazyLock<Mutex<crate::kb::cache::SearchCache<String>>> =
    std::sync::LazyLock::new(|| Mutex::new(crate::kb::cache::SearchCache::new(5000, 14400)));
/// P4-008: 召回结果缓存（短 TTL）
static RETRIEVAL_CACHE: std::sync::LazyLock<
    Mutex<crate::kb::cache::SearchCache<Vec<RankedResult>>>,
> = std::sync::LazyLock::new(|| Mutex::new(crate::kb::cache::SearchCache::new(200, 30)));
/// P4-009: rerank 结果缓存（按 provider/model/profile 隔离）
#[allow(dead_code)]
static RERANK_CACHE: std::sync::LazyLock<Mutex<crate::kb::cache::SearchCache<Vec<RankedResult>>>> =
    std::sync::LazyLock::new(|| Mutex::new(crate::kb::cache::SearchCache::new(200, 60)));

/// RRF smoothing constant. Higher k dampens the effect of individual rank
/// positions, making the merge more robust to outlier rankings.
const RRF_K: usize = 60;

/// Perform KB hybrid search with full pipeline:
/// query normalization → planner → multi-retriever → RRF → rerank → context expansion
///
/// Returns results sorted by descending relevance score.
pub fn kb_search(
    conn: &Connection,
    input: &KbSearchInput,
    query_vector: Option<&[f32]>,
) -> Result<Vec<KbSearchResult>> {
    let fetch_k = (input.top_k * 3).max(30);

    // P3-001: query normalization
    let query_normalized = normalize_query(&input.query);

    // P3-006/P3-007: query planner
    let planner_type = if let Some(ref override_str) = input.planner_override {
        // 解析 override 字符串为 QueryType
        match override_str.to_lowercase().as_str() {
            "exact_lookup" | "exact" => crate::kb::planner::QueryType::ExactLookup,
            "how_to" | "howto" => crate::kb::planner::QueryType::HowTo,
            "fact_lookup" | "fact" => crate::kb::planner::QueryType::FactLookup,
            "conceptual" | "concept" => crate::kb::planner::QueryType::Conceptual,
            "table_lookup" | "table" => crate::kb::planner::QueryType::TableLookup,
            "recent_or_timebound" | "recent" | "timebound" => {
                crate::kb::planner::QueryType::RecentOrTimebound
            }
            "small_document" | "small" => crate::kb::planner::QueryType::SmallDocument,
            _ => crate::kb::planner::classify_query(&query_normalized),
        }
    } else {
        crate::kb::planner::classify_query(&query_normalized)
    };
    let plan = crate::kb::planner::plan(planner_type);

    // P3-008~013: multi-retriever execution — use plan to decide retrievers
    let mut all_candidates: Vec<Vec<RankedResult>> = Vec::new();

    let retriever_set: std::collections::HashSet<crate::kb::planner::RetrieverType> =
        plan.retrievers.iter().map(|(rt, _)| *rt).collect();

    // Title/name retriever
    if retriever_set.contains(&crate::kb::planner::RetrieverType::TitleName) {
        let title_results =
            title_name_retriever(conn, &query_normalized, &input.library_ids, fetch_k)?;
        if !title_results.is_empty() {
            all_candidates.push(title_results);
        }
    }

    // P4-001: profile routing (override by planner)
    let profile = input.profile.as_deref().unwrap_or("balanced");

    // Node FTS retriever (P3-009)
    if retriever_set.contains(&crate::kb::planner::RetrieverType::NodeFts) {
        let fts_results = kb_fts_search(
            conn,
            &query_normalized,
            &input.library_ids,
            input.level,
            fetch_k,
        )?;
        if !fts_results.is_empty() {
            all_candidates.push(fts_results);
        }
    }

    // Vector retriever (P3-010)
    if retriever_set.contains(&crate::kb::planner::RetrieverType::Vector) {
        if let Some(vec) = query_vector {
            let vec_results = kb_vector_search(
                conn,
                vec,
                &input.library_ids,
                input.level,
                fetch_k,
                input.embedding_index_id,
            )?;
            if !vec_results.is_empty() {
                all_candidates.push(vec_results);
            }
        }
    }

    // P3-011: Summary retriever
    if retriever_set.contains(&crate::kb::planner::RetrieverType::Summary) {
        if let Ok(sr) = summary_retriever(conn, &query_normalized, &input.library_ids, fetch_k) {
            if !sr.is_empty() {
                all_candidates.push(sr);
            }
        }
    }

    // P3-012: Table retriever
    if retriever_set.contains(&crate::kb::planner::RetrieverType::Table) {
        if let Ok(tr) = table_retriever(conn, &query_normalized, &input.library_ids, fetch_k) {
            if !tr.is_empty() {
                all_candidates.push(tr);
            }
        }
    }

    // P3-013: Metadata retriever
    if retriever_set.contains(&crate::kb::planner::RetrieverType::Metadata) {
        if let Ok(mr) = metadata_retriever(conn, &query_normalized, &input.library_ids, fetch_k) {
            if !mr.is_empty() {
                all_candidates.push(mr);
            }
        }
    }

    // P4-006~009: 缓存查询 — RRF merge 前检查 cache
    let index_version: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(index_version), 1) FROM kb_index_state WHERE index_type='vector'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(1);
    // 缓存 key 纳入所有影响召回的参数
    let level_str = input
        .level
        .map_or_else(|| "-".to_string(), |l| l.to_string());
    let folder_str = input
        .folder_id
        .map_or_else(|| "-".to_string(), |f| f.to_string());
    let eidx_str = input
        .embedding_index_id
        .map_or_else(|| "-".to_string(), |e| e.to_string());
    let merge_cache_key = format!(
        "merge:{}|libs:{}|v:{}|k:{}|lvl:{}|prof:{}|fid:{}|eidx:{}|vec:{}",
        query_normalized,
        input
            .library_ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(","),
        index_version,
        input.top_k,
        level_str,
        input.profile.as_deref().unwrap_or("-"),
        folder_str,
        eidx_str,
        query_vector.is_some(),
    );
    let cached_merged = RETRIEVAL_CACHE
        .lock()
        .ok()
        .and_then(|c| c.get(&merge_cache_key));
    let merged = if let Some(cached) = cached_merged {
        cached
    } else {
        let computed = compute_rrf_merge(all_candidates);
        // P4-008: 存储召回结果
        if let Ok(c) = RETRIEVAL_CACHE.lock() {
            c.set(merge_cache_key.clone(), computed.clone());
        }
        computed
    };

    // P1-004: 过滤已删除
    let mut merged = filter_deleted_docs(conn, merged);
    // P4-002: 7 级 fallback 链
    let mut fallbacks_used: Vec<&str> = Vec::new();
    if merged.is_empty() {
        // Level 1: strict → synonym + alias expand
        let mut variants = crate::nlp::chinese::expand_query_with_synonyms(&query_normalized);
        variants.extend(crate::nlp::chinese::expand_query_with_aliases(
            &query_normalized,
        ));
        for variant in variants.iter().skip(1).take(3) {
            if let Ok(fr) =
                kb_fts_search(conn, variant, &input.library_ids, input.level, fetch_k * 2)
            {
                if !fr.is_empty() {
                    merged.extend(fr);
                    fallbacks_used.push("synonym_alias_expand");
                    break;
                }
            }
        }
    }
    if merged.is_empty() {
        // Level 2: broaden_or — AND → OR
        let broad_query = query_normalized
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" OR ");
        if broad_query != query_normalized {
            if let Ok(fr) = kb_fts_search(
                conn,
                &broad_query,
                &input.library_ids,
                input.level,
                fetch_k * 2,
            ) {
                if !fr.is_empty() {
                    merged.extend(fr);
                    fallbacks_used.push("broaden_or");
                }
            }
        }
    }
    if merged.is_empty() && crate::nlp::chinese::detect_pinyin_query(&query_normalized) {
        // Level 3: pinyin — 对中文字段做拼音匹配
        if let Ok(fr) = kb_fts_search(
            conn,
            &input.query,
            &input.library_ids,
            input.level,
            fetch_k * 3,
        ) {
            if !fr.is_empty() {
                merged.extend(fr);
                fallbacks_used.push("pinyin");
            }
        }
    }
    if merged.is_empty() {
        // Level 4: title_name_expand — 扩展到文件名/标题检索
        if let Ok(fr) =
            title_name_retriever(conn, &query_normalized, &input.library_ids, fetch_k * 3)
        {
            if !fr.is_empty() {
                merged.extend(fr);
                fallbacks_used.push("title_name_expand");
            }
        }
    }
    if merged.is_empty() {
        // Level 5: summary_search — 搜索摘要
        if let Ok(sr) = summary_retriever(conn, &query_normalized, &input.library_ids, fetch_k * 3)
        {
            if !sr.is_empty() {
                merged.extend(sr);
                fallbacks_used.push("summary_search");
            }
        }
    }
    if merged.is_empty() {
        // Level 6: low_threshold_vector — 降低向量阈值扩大召回
        // 保留用户指定的 library_ids 和 level 过滤，避免返回其它库或层级的结果
        if let Some(vec) = query_vector {
            if let Ok(fr) = kb_vector_search(
                conn,
                vec,
                &input.library_ids,
                input.level,
                fetch_k * 5,
                input.embedding_index_id,
            ) {
                if !fr.is_empty() {
                    merged.extend(fr);
                    fallbacks_used.push("low_threshold_vector");
                }
            }
        }
    }
    merged = dedup_by_node(merged);

    // P3-028: 按 folder_id 过滤
    if let Some(folder_id) = input.folder_id {
        merged = filter_by_folder(conn, merged, folder_id);
    }

    // P4-003: 基于实际候选 node_ids 解析隐私策略，取最严格约束
    // 候选为空时可默认允许（不会发送任何内容到外部模型）
    let (external_rerank_allowed, redaction_enabled) = if merged.is_empty() {
        (true, false)
    } else {
        resolve_rerank_policy_for_candidate_nodes(conn, &merged)?
    };

    // P3-016/P3-020/P4-003~005: 模型 rerank 优先，失败 fallback 本地 rerank
    let rerank_info = if !merged.is_empty() && merged.len() <= 50 {
        // 读取库级 rerank 配置
        let lib_rerank_config = input.library_ids.first().and_then(|&lib_id| {
            conn.query_row(
                "SELECT rerank_enabled, rerank_provider FROM kb_libraries WHERE id=?1",
                [lib_id],
                |row| Ok((row.get::<_, i32>(0)?, row.get::<_, String>(1)?)),
            )
            .ok()
        });
        let (rerank_enabled, rerank_provider) = lib_rerank_config.unwrap_or((1, String::new()));

        // 构建 RerankConfig
        let rerank_cfg = crate::kb::rerank::RerankConfig {
            model_rerank_enabled: rerank_enabled != 0,
            rerank_provider: rerank_provider.clone(),
            rerank_model: "gpt-4o-mini".into(),
            rerank_timeout_ms: 5000,
            rerank_max_candidates: 50,
            external_rerank_allowed,
        };

        // 构建 LocalRankSignals（使用 RRF 分数作为主信号）
        let candidates: Vec<(i64, crate::kb::rerank::LocalRankSignals)> = merged
            .iter()
            .map(|r| {
                (
                    r.node_id,
                    crate::kb::rerank::LocalRankSignals {
                        fts_score: r.score,
                        ..Default::default()
                    },
                )
            })
            .collect();

        // 获取候选节点内容用于模型 rerank（开启脱敏时先 redact）
        let candidate_texts: Vec<crate::kb::rerank::RerankCandidate> = merged
            .iter()
            .filter_map(|r| {
                conn.query_row(
                    "SELECT content FROM kb_document_nodes WHERE id=?1",
                    [r.node_id],
                    |row| row.get::<_, String>(0),
                )
                .ok()
                .map(|text| {
                    let safe_text = if redaction_enabled && external_rerank_allowed {
                        crate::kb::privacy::redact_content(&text)
                    } else {
                        text
                    };
                    crate::kb::rerank::RerankCandidate {
                        doc_id: r.node_id,
                        text: safe_text,
                    }
                })
            })
            .collect();

        let weights = vec![0.4, 0.3, 0.2, 0.1, 0.0, 0.0];

        // 尝试模型 rerank（通过 mini tokio runtime）
        // base_url 缺省为 OpenAI 默认端点，确保只配了 API key 的用户也能使用模型 rerank
        let has_api_key = input
            .rerank_api_key
            .as_deref()
            .is_some_and(|k| !k.is_empty());
        let (scored, rerank_result) = if external_rerank_allowed
            && rerank_cfg.model_rerank_enabled
            && has_api_key
        {
            let api_key = input.rerank_api_key.as_deref().unwrap_or("");
            let base_url = input
                .rerank_base_url
                .as_deref()
                .filter(|u| !u.is_empty())
                .unwrap_or("https://api.openai.com/v1");
            // P4-004: 审计外部模型调用
            let _ = crate::kb::privacy::log_external_model_call(
                conn,
                input.library_ids.first().copied(),
                None,
                "rerank",
                &rerank_provider,
                &rerank_cfg.rerank_model,
                query_normalized.len() as i32,
                merged.len() as i32,
                0,
                0.0,
                true,
                "",
            );
            match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt.block_on(crate::kb::rerank::try_model_rerank_simple(
                    &rerank_cfg,
                    &query_normalized,
                    &candidates,
                    &candidate_texts,
                    &weights,
                    None,
                    base_url,
                    api_key,
                )),
                Err(_) => {
                    // 无法创建 runtime，直接使用本地 rerank
                    let local = crate::kb::rerank::local_rerank(&candidates, &weights);
                    (
                        local,
                        crate::kb::rerank::RerankResult {
                            model_rerank_attempted: false,
                            model_rerank_succeeded: false,
                            fallback_used: true,
                            fallback_reason: Some(crate::kb::rerank::FallbackReason::NotConfigured),
                            provider: "local".into(),
                            candidates_reranked: merged.len(),
                        },
                    )
                }
            }
        } else {
            // 跳过模型 rerank，直接本地 rerank
            let local = crate::kb::rerank::local_rerank(&candidates, &weights);
            let reason = if !external_rerank_allowed {
                crate::kb::rerank::FallbackReason::PrivacyBlocked
            } else {
                crate::kb::rerank::FallbackReason::NotConfigured
            };
            (
                local,
                crate::kb::rerank::RerankResult {
                    model_rerank_attempted: false,
                    model_rerank_succeeded: false,
                    fallback_used: true,
                    fallback_reason: Some(reason),
                    provider: "local".into(),
                    candidates_reranked: merged.len(),
                },
            )
        };

        // 按 rerank 分数重排 merged
        let score_map: HashMap<i64, f64> = scored.iter().map(|(id, s)| (*id, *s)).collect();
        for r in &mut merged {
            if let Some(&new_score) = score_map.get(&r.node_id) {
                r.score = new_score;
            }
        }
        merged.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for (i, r) in merged.iter_mut().enumerate() {
            r.rank = i + 1;
        }

        rerank_result
    } else {
        crate::kb::rerank::RerankResult {
            model_rerank_attempted: false,
            model_rerank_succeeded: false,
            fallback_used: false,
            fallback_reason: None,
            provider: String::new(),
            candidates_reranked: 0,
        }
    };

    // Fetch full node details with context
    let mut results = fetch_node_details(conn, &merged, input.top_k)?;

    // P3-025: group_by_document
    if input.group_by_document {
        results = group_by_document(results);
    }

    // P3-023~027: enrich results with context/highlights/open_target
    if input.include_context || input.include_highlights || input.debug {
        enrich_results(&mut results, conn, input, &query_normalized);
    }

    // P4-020: search logging (异步，不阻塞返回)
    // FIX9-21: 写日志前按结果顺序去重 document_id，保留首次出现顺序，
    // 避免同一文档多个 node 命中导致重复 document_id 污染评测指标
    let mut seen_doc_ids = std::collections::HashSet::new();
    let result_doc_ids: Vec<i64> = results
        .iter()
        .filter_map(|r| {
            if seen_doc_ids.insert(r.document_id) {
                Some(r.document_id)
            } else {
                None
            }
        })
        .collect();
    let _ = crate::kb::eval::log_search(
        conn,
        &query_normalized,
        &input.library_ids,
        profile,
        planner_type.as_str(),
        results.len(),
        0,
        false,
        input.embedding_index_id,
        &result_doc_ids,
    );

    // P3-021: debug signals
    if input.debug {
        for r in &mut results {
            let debug_info = serde_json::json!({
                "planner_type": planner_type.as_str(),
                "rerank_provider": rerank_info.provider,
                "model_rerank_attempted": rerank_info.model_rerank_attempted,
                "model_rerank_succeeded": rerank_info.model_rerank_succeeded,
                "fallback_used": rerank_info.fallback_used,
                "fallback_reason": rerank_info.fallback_reason.map(|r| r.as_str()),
                "fallbacks_chain": fallbacks_used,
            });
            r.debug_signals = Some(debug_info);
        }
    }

    Ok(results)
}

/// P3-001~002: query normalization — trim, lowercase, punctuation, 繁→简
fn normalize_query(query: &str) -> String {
    let mut q = query.trim().to_lowercase();
    // P3-002: 繁体→简体
    q = crate::nlp::chinese::traditional_to_simplified(&q);
    // 全角→半角标点
    q = q
        .replace('，', ",")
        .replace('。', ".")
        .replace('！', "!")
        .replace('？', "?")
        .replace('：', ":")
        .replace('；', ";")
        .replace('（', "(")
        .replace('）', ")");
    // 多余空白清理
    let parts: Vec<&str> = q.split_whitespace().collect();
    parts.join(" ")
}

/// P3-008: title/name retriever — FTS on document names
///
/// Returns one node_id per matched document (first chunk by chunk_order).
/// Filters by library_ids and excludes soft-deleted documents.
fn title_name_retriever(
    conn: &Connection,
    query: &str,
    library_ids: &[i64],
    top_k: usize,
) -> Result<Vec<RankedResult>> {
    let token_query = chinese::build_fts_match_query(query);
    if token_query.is_empty() {
        return Ok(Vec::new());
    }

    let mut sql = String::from(
        "SELECT MIN(n.id) FROM kb_doc_name_fts f \
         JOIN kb_documents d ON d.id = f.rowid \
         JOIN kb_document_nodes n ON n.document_id = d.id \
         WHERE kb_doc_name_fts MATCH ?1 AND d.deleted_at IS NULL",
    );
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(token_query)];

    if !library_ids.is_empty() {
        let start = param_values.len() + 1;
        let placeholders: Vec<String> = library_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", start + i))
            .collect();
        sql.push_str(&format!(
            " AND d.library_id IN ({})",
            placeholders.join(",")
        ));
        for &id in library_ids {
            param_values.push(Box::new(id));
        }
    }

    let limit_idx = param_values.len() + 1;
    sql.push_str(&format!(" GROUP BY f.rowid LIMIT ?{}", limit_idx));
    param_values.push(Box::new(top_k as i64));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(RankedResult {
            node_id: row.get::<_, i64>(0)?,
            rank: 0,
            score: 0.0,
        })
    })?;
    let mut results: Vec<RankedResult> = rows.filter_map(|r| r.ok()).collect();
    for (i, r) in results.iter_mut().enumerate() {
        r.rank = i + 1;
    }
    Ok(results)
}

/// P3-022: 按 node_id 去重（回退链可能引入重复节点）
fn dedup_by_node(mut merged: Vec<RankedResult>) -> Vec<RankedResult> {
    let mut seen = HashMap::new();
    merged.retain(|r| seen.insert(r.node_id, ()).is_none());
    merged
}

/// P3-023~027: enrich results with context, highlights, open_target
fn enrich_results(
    results: &mut [KbSearchResult],
    conn: &Connection,
    input: &KbSearchInput,
    query_normalized: &str,
) {
    for r in results.iter_mut() {
        // P3-026: compute highlight ranges
        if input.include_highlights {
            r.highlight_ranges = Some(compute_highlights(&r.content, query_normalized));
        }
        // P3-023/P3-024: context expansion — 从相邻 nodes 获取前后文
        if input.include_context {
            if let Ok((before, after)) = get_node_context(
                conn,
                r.document_id,
                r.node_id,
                input.context_before,
                input.context_after,
            ) {
                r.context_before = before;
                r.context_after = after;
            }
        }
        // P3-027: open_target URI
        r.open_target =
            build_open_target(conn, r.document_id, r.page_number, r.title_path.as_deref());
    }
}

/// P3-023/P3-024: 从相邻 node 获取上下文
fn get_node_context(
    conn: &Connection,
    document_id: i64,
    node_id: i64,
    context_before_chars: usize,
    context_after_chars: usize,
) -> Result<(Option<String>, Option<String>)> {
    // 获取当前 node 的 chunk_order
    let chunk_order: i32 = conn.query_row(
        "SELECT chunk_order FROM kb_document_nodes WHERE id = ?1",
        rusqlite::params![node_id],
        |row| row.get(0),
    )?;
    // 前一个 node
    let before = if chunk_order > 0 {
        conn.query_row(
            "SELECT content FROM kb_document_nodes WHERE document_id = ?1 AND chunk_order = ?2",
            rusqlite::params![document_id, chunk_order - 1],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .map(|s| {
            let chars: Vec<char> = s.chars().collect();
            let start = chars.len().saturating_sub(context_before_chars);
            chars[start..].iter().collect()
        })
    } else {
        None
    };
    // 后一个 node
    let after = conn
        .query_row(
            "SELECT content FROM kb_document_nodes WHERE document_id = ?1 AND chunk_order = ?2",
            rusqlite::params![document_id, chunk_order + 1],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .map(|s| {
            let chars: Vec<char> = s.chars().collect();
            let end = context_after_chars.min(chars.len());
            chars[..end].iter().collect()
        });
    Ok((before, after))
}

/// P3-026: compute highlight character ranges
fn compute_highlights(content: &str, query: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let content_lower = content.to_lowercase();
    let query_terms: Vec<&str> = query.split_whitespace().collect();
    for term in query_terms {
        if term.len() < 2 {
            continue;
        }
        let mut start = 0;
        while let Some(pos) = content_lower[start..].find(&term.to_lowercase()) {
            ranges.push((start + pos, start + pos + term.len()));
            start += pos + term.len();
        }
    }
    ranges
}

/// P3-027: build open_target URI
fn build_open_target(
    _conn: &Connection,
    document_id: i64,
    page_number: Option<i32>,
    _title_path: Option<&str>,
) -> Option<String> {
    if let Some(pn) = page_number {
        if pn > 0 {
            return Some(format!("gbrain://kb/doc/{}#page={}", document_id, pn));
        }
    }
    Some(format!("gbrain://kb/doc/{}", document_id))
}

// ---------------------------------------------------------------------------
// P3-011/012/013: Summary, Table, Metadata retrievers
// ---------------------------------------------------------------------------

/// P3-011: 摘要检索器 — 在 kb_document_summaries 中搜索
///
/// Returns one node_id per matched document (first chunk by chunk_order).
/// Filters by library_ids and excludes soft-deleted documents.
fn summary_retriever(
    conn: &Connection,
    query: &str,
    library_ids: &[i64],
    top_k: usize,
) -> Result<Vec<RankedResult>> {
    let mut sql = String::from(
        "SELECT MIN(n.id) FROM kb_document_summaries s \
         JOIN kb_documents d ON d.id = s.document_id \
         JOIN kb_document_nodes n ON n.document_id = d.id \
         WHERE s.summary_tokens LIKE ?1 AND d.deleted_at IS NULL",
    );
    let like_query = format!("%{}%", query);
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(like_query)];

    if !library_ids.is_empty() {
        let start = param_values.len() + 1;
        let placeholders: Vec<String> = library_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", start + i))
            .collect();
        sql.push_str(&format!(
            " AND d.library_id IN ({})",
            placeholders.join(",")
        ));
        for &id in library_ids {
            param_values.push(Box::new(id));
        }
    }

    let limit_idx = param_values.len() + 1;
    sql.push_str(&format!(" GROUP BY s.document_id LIMIT ?{}", limit_idx));
    param_values.push(Box::new(top_k as i64));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(RankedResult {
            node_id: row.get::<_, i64>(0)?,
            rank: 0,
            score: 0.0,
        })
    })?;
    let mut results: Vec<RankedResult> = rows.filter_map(|r| r.ok()).collect();
    for (i, r) in results.iter_mut().enumerate() {
        r.rank = i + 1;
    }
    Ok(results)
}

/// P3-012: 表格检索器 — 在 kb_table_rows 中搜索
///
/// 每个匹配的 table 返回一个 node_id（取 chunk_order 最小的节点）。
/// 排除已删除文档，按 library_ids 过滤。
fn table_retriever(
    conn: &Connection,
    query: &str,
    library_ids: &[i64],
    top_k: usize,
) -> Result<Vec<RankedResult>> {
    let mut sql = String::from(
        "SELECT MIN(n.id) FROM kb_table_rows r \
         JOIN kb_tables t ON t.id = r.table_id \
         JOIN kb_documents d ON d.id = t.document_id \
         JOIN kb_document_nodes n ON n.document_id = d.id \
         WHERE r.row_text LIKE ?1 AND d.deleted_at IS NULL",
    );
    let like_query = format!("%{}%", query);
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(like_query)];

    if !library_ids.is_empty() {
        let start = param_values.len() + 1;
        let placeholders: Vec<String> = library_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", start + i))
            .collect();
        sql.push_str(&format!(
            " AND d.library_id IN ({})",
            placeholders.join(",")
        ));
        for &id in library_ids {
            param_values.push(Box::new(id));
        }
    }

    let limit_idx = param_values.len() + 1;
    sql.push_str(&format!(" GROUP BY r.table_id LIMIT ?{}", limit_idx));
    param_values.push(Box::new(top_k as i64));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(RankedResult {
            node_id: row.get::<_, i64>(0)?,
            rank: 0,
            score: 0.0,
        })
    })?;
    let mut results: Vec<RankedResult> = rows.filter_map(|r| r.ok()).collect();
    for (i, r) in results.iter_mut().enumerate() {
        r.rank = i + 1;
    }
    Ok(results)
}

/// P3-013: 元数据检索器 — 在 kb_documents 的 title/keywords/entity_names 中搜索
///
/// 每个匹配的文档返回一个 node_id（取 chunk_order 最小的节点）。
/// 按 library_ids 过滤，排除已删除文档。
fn metadata_retriever(
    conn: &Connection,
    query: &str,
    library_ids: &[i64],
    top_k: usize,
) -> Result<Vec<RankedResult>> {
    let mut sql = String::from(
        "SELECT MIN(n.id) FROM kb_documents d \
         JOIN kb_document_nodes n ON n.document_id = d.id \
         WHERE (d.title LIKE ?1 OR d.keywords LIKE ?1 \
         OR d.entity_names LIKE ?1) AND d.deleted_at IS NULL",
    );
    let like_query = format!("%{}%", query);
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(like_query)];

    if !library_ids.is_empty() {
        let start = param_values.len() + 1;
        let placeholders: Vec<String> = library_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", start + i))
            .collect();
        sql.push_str(&format!(
            " AND d.library_id IN ({})",
            placeholders.join(",")
        ));
        for &id in library_ids {
            param_values.push(Box::new(id));
        }
    }

    let limit_idx = param_values.len() + 1;
    sql.push_str(&format!(" GROUP BY d.id LIMIT ?{}", limit_idx));
    param_values.push(Box::new(top_k as i64));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(RankedResult {
            node_id: row.get::<_, i64>(0)?,
            rank: 0,
            score: 0.0,
        })
    })?;
    let mut results: Vec<RankedResult> = rows.filter_map(|r| r.ok()).collect();
    for (i, r) in results.iter_mut().enumerate() {
        r.rank = i + 1;
    }
    Ok(results)
}

/// P1-004: 过滤掉已删除文档的结果
fn filter_deleted_docs(conn: &Connection, merged: Vec<RankedResult>) -> Vec<RankedResult> {
    if merged.is_empty() {
        return merged;
    }
    let placeholders: Vec<String> = merged
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect();
    let sql = format!(
        "SELECT n.id FROM kb_document_nodes n \
         INNER JOIN kb_documents d ON d.id = n.document_id \
         WHERE n.id IN ({}) AND d.deleted_at IS NULL",
        placeholders.join(",")
    );
    let param_values: Vec<Box<dyn rusqlite::types::ToSql>> = merged
        .iter()
        .map(|r| Box::new(r.node_id) as Box<dyn rusqlite::types::ToSql>)
        .collect();
    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();
    if let Ok(mut stmt) = conn.prepare(&sql) {
        if let Ok(valid_ids) = stmt.query_map(param_refs.as_slice(), |row| row.get::<_, i64>(0)) {
            let valid_set: std::collections::HashSet<i64> =
                valid_ids.filter_map(|r| r.ok()).collect();
            return merged
                .into_iter()
                .filter(|r| valid_set.contains(&r.node_id))
                .collect();
        }
    }
    merged
}

/// P3-025: 按文档分组结果
///
/// Groups search results by document_id. Within each group, hits are sorted by
/// descending score. Between groups, groups are sorted by best_score descending.
/// Returns a flat list where the best hit of each group appears first (interleaved),
/// followed by remaining hits from each group in score order.
///
/// Each top-level result carries a `group_hits` field with the other hits from
/// the same document, and `matched_by` is set to "grouped_by_document".
fn group_by_document(results: Vec<KbSearchResult>) -> Vec<KbSearchResult> {
    if results.is_empty() {
        return results;
    }

    // 1. Group by document_id
    let mut groups: HashMap<i64, Vec<KbSearchResult>> = HashMap::new();
    for r in results {
        groups.entry(r.document_id).or_default().push(r);
    }

    // 2. Sort hits within each group by descending score
    for hits in groups.values_mut() {
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    // 3. Build DocumentGroup list sorted by best_score descending
    let mut doc_groups: Vec<DocumentGroup> = groups
        .into_iter()
        .map(|(doc_id, hits)| {
            // hits already sorted descending by score
            let best_score = hits.first().map(|h| h.score).unwrap_or(0.0);
            let document_title = hits
                .first()
                .map(|h| h.document_name.clone())
                .unwrap_or_default();
            DocumentGroup {
                document_id: doc_id,
                document_title,
                best_score,
                hits,
            }
        })
        .collect();
    doc_groups.sort_by(|a, b| {
        b.best_score
            .partial_cmp(&a.best_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // 4. Interleave: emit the best hit from each group first, then remaining hits.
    //    Each emitted hit gets group_hits populated with the other hits from its group.
    let mut output: Vec<KbSearchResult> = Vec::new();

    // First pass: emit the top hit from each group (best group first)
    let mut remaining: Vec<Vec<KbSearchResult>> = Vec::new();
    for group in doc_groups {
        let mut hits = group.hits;
        if let Some(mut best) = hits.first().cloned() {
            // Collect the rest as group_hits
            let rest: Vec<KbSearchResult> = hits.iter().skip(1).cloned().collect();
            best.matched_by = Some("grouped_by_document".into());
            if !rest.is_empty() {
                best.group_hits = Some(rest);
            }
            output.push(best);
            // Keep remaining hits (skip first) for second pass
            remaining.push(hits.split_off(1));
        } else {
            remaining.push(Vec::new());
        }
    }

    // Second pass: emit remaining hits from each group in order
    for group_remaining in remaining {
        for mut hit in group_remaining {
            hit.matched_by = Some("grouped_by_document".into());
            output.push(hit);
        }
    }

    output
}

/// Vector similarity search using sqlite-vec KNN.
///
/// When `embedding_index_id` is provided, tries the per-index vec table
/// (`vec_kb_{index_id}`) first. Falls back to the legacy `vec_kb_nodes` table.
/// If sqlite-vec is unavailable or returns no results, falls back to brute-force
/// cosine similarity over `kb_node_embeddings` BLOB storage.
///
/// Returns results ordered by rank (position in the result list).
pub fn kb_vector_search(
    conn: &Connection,
    embedding: &[f32],
    library_ids: &[i64],
    level: Option<i32>,
    top_k: usize,
    embedding_index_id: Option<i64>,
) -> Result<Vec<RankedResult>> {
    let query_blob = embedding_to_blob(embedding);

    // P5-012: 指定 index 时只查 per-index vec 表，失败则 fallback 到同 index 的 BLOB 表
    if let Some(index_id) = embedding_index_id {
        let per_index_table = crate::kb::embedding_index::vec_table_name_for_index(index_id);
        let result = try_vec_knn(
            conn,
            &query_blob,
            library_ids,
            level,
            top_k,
            &per_index_table,
        );
        match result {
            Ok(results) if !results.is_empty() => return Ok(results),
            _ => {
                // 不 fallback 到 legacy vec_kb_nodes，只 fallback 到同 index 的 BLOB 表
                return vector_search_fallback(
                    conn,
                    embedding,
                    library_ids,
                    level,
                    top_k,
                    embedding_index_id,
                );
            }
        }
    }

    // 未指定 index：legacy 路径，先查 legacy vec 表，再 fallback BLOB
    let result = try_vec_knn(conn, &query_blob, library_ids, level, top_k, "vec_kb_nodes");
    match result {
        Ok(results) if !results.is_empty() => Ok(results),
        _ => vector_search_fallback(
            conn,
            embedding,
            library_ids,
            level,
            top_k,
            embedding_index_id,
        ),
    }
}

/// FTS5 keyword search using `kb_doc_fts` with jieba tokenization.
///
/// The query string is tokenized via `chinese::build_fts_match_query` which
/// uses jieba segmentation for Chinese text and produces a prefix-match FTS5
/// query string. Results are ranked by BM25 score (ascending = more relevant).
pub fn kb_fts_search(
    conn: &Connection,
    query: &str,
    library_ids: &[i64],
    level: Option<i32>,
    top_k: usize,
) -> Result<Vec<RankedResult>> {
    let token_query = chinese::build_fts_match_query(query);

    if token_query.is_empty() {
        return Ok(Vec::new());
    }

    let mut sql = String::from(
        "SELECT f.rowid \
         FROM kb_doc_fts f \
         INNER JOIN kb_document_nodes n ON n.id = f.rowid \
         WHERE kb_doc_fts MATCH ?1",
    );

    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(token_query)];

    if !library_ids.is_empty() {
        let start = param_values.len() + 1;
        let placeholders: Vec<String> = library_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", start + i))
            .collect();
        sql.push_str(&format!(
            " AND n.library_id IN ({})",
            placeholders.join(",")
        ));
        for &id in library_ids {
            param_values.push(Box::new(id));
        }
    }

    if let Some(lvl) = level {
        let idx = param_values.len() + 1;
        sql.push_str(&format!(" AND n.level = ?{}", idx));
        param_values.push(Box::new(lvl));
    }

    let limit_idx = param_values.len() + 1;
    sql.push_str(&format!(
        " ORDER BY bm25(kb_doc_fts) ASC LIMIT ?{}",
        limit_idx
    ));
    param_values.push(Box::new(top_k as i64));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(param_refs.as_slice())?;

    let mut results = Vec::new();
    while let Some(row) = rows.next()? {
        let node_id: i64 = row.get(0)?;
        results.push(RankedResult {
            node_id,
            rank: results.len() + 1,
            score: 0.0,
        });
    }

    Ok(results)
}

/// RRF merge of multiple retriever outputs into a single ranked list
fn compute_rrf_merge(all_candidates: Vec<Vec<RankedResult>>) -> Vec<RankedResult> {
    if all_candidates.is_empty() {
        return Vec::new();
    }
    // 即使只有一个候选列表，也按 RRF 公式计算分数
    let mut scores: HashMap<i64, f64> = HashMap::new();
    for candidates in &all_candidates {
        for r in candidates {
            *scores.entry(r.node_id).or_insert(0.0) += 1.0 / (RRF_K + r.rank) as f64;
        }
    }
    let mut m: Vec<RankedResult> = scores
        .into_iter()
        .map(|(node_id, score)| RankedResult {
            node_id,
            rank: 0,
            score,
        })
        .collect();
    m.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for (i, r) in m.iter_mut().enumerate() {
        r.rank = i + 1;
    }
    m
}

/// Reciprocal Rank Fusion merge of two ranked result lists (legacy).
///
/// Each result's contribution is `1 / (k + rank)` where k is the smoothing
/// constant (60 by default). Results appearing in both lists get combined
/// scores, rewarding agreement between retrieval methods.
///
/// Returns results sorted by descending fused score, with rank reassigned.
pub fn rrf_merge(
    vec_results: Vec<RankedResult>,
    fts_results: Vec<RankedResult>,
) -> Vec<RankedResult> {
    let mut scores: HashMap<i64, f64> = HashMap::new();

    for r in &vec_results {
        *scores.entry(r.node_id).or_insert(0.0) += 1.0 / (RRF_K + r.rank) as f64;
    }

    for r in &fts_results {
        *scores.entry(r.node_id).or_insert(0.0) += 1.0 / (RRF_K + r.rank) as f64;
    }

    let mut merged: Vec<RankedResult> = scores
        .into_iter()
        .map(|(node_id, score)| RankedResult {
            node_id,
            rank: 0,
            score,
        })
        .collect();

    merged.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for (i, r) in merged.iter_mut().enumerate() {
        r.rank = i + 1;
    }

    merged
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Attempt sqlite-vec KNN search.
fn try_vec_knn(
    conn: &Connection,
    query_blob: &[u8],
    library_ids: &[i64],
    level: Option<i32>,
    top_k: usize,
    table_name: &str,
) -> Result<Vec<RankedResult>> {
    let mut sql = format!(
        "SELECT v.node_id \
         FROM {} v \
         INNER JOIN kb_document_nodes n ON n.id = v.node_id \
         WHERE v.embedding MATCH ?1 AND k = ?2",
        table_name,
    );

    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> =
        vec![Box::new(query_blob.to_vec()), Box::new(top_k as i64)];

    if !library_ids.is_empty() {
        let start = param_values.len() + 1;
        let placeholders: Vec<String> = library_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", start + i))
            .collect();
        sql.push_str(&format!(
            " AND n.library_id IN ({})",
            placeholders.join(",")
        ));
        for &id in library_ids {
            param_values.push(Box::new(id));
        }
    }

    if let Some(lvl) = level {
        let idx = param_values.len() + 1;
        sql.push_str(&format!(" AND n.level = ?{}", idx));
        param_values.push(Box::new(lvl));
    }

    sql.push_str(" ORDER BY v.distance ASC");

    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();

    let mut results = Vec::new();

    // sqlite-vec may not be available; wrap in best-effort block
    if let Ok(mut stmt) = conn.prepare(&sql) {
        if let Ok(mut rows) = stmt.query(param_refs.as_slice()) {
            while let Ok(Some(row)) = rows.next() {
                if let Ok(node_id) = row.get::<_, i64>(0) {
                    results.push(RankedResult {
                        node_id,
                        rank: results.len() + 1,
                        score: 0.0,
                    });
                }
            }
        }
    }

    Ok(results)
}

/// Brute-force cosine similarity search over kb_node_embeddings BLOB table.
///
/// This is the fallback when sqlite-vec is not available. It loads candidate
/// embeddings into memory (capped at 10000 rows) and computes cosine similarity.
const VECTOR_FALLBACK_MAX_ROWS: usize = 10000;

fn vector_search_fallback(
    conn: &Connection,
    embedding: &[f32],
    library_ids: &[i64],
    level: Option<i32>,
    top_k: usize,
    embedding_index_id: Option<i64>,
) -> Result<Vec<RankedResult>> {
    let mut sql = String::from(
        "SELECT ne.node_id, ne.embedding \
         FROM kb_node_embeddings ne \
         INNER JOIN kb_document_nodes n ON n.id = ne.node_id \
         WHERE 1=1",
    );

    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    // P5-011: Filter by embedding_index_id if specified
    if let Some(index_id) = embedding_index_id {
        let idx = param_values.len() + 1;
        sql.push_str(&format!(" AND ne.embedding_index_id = ?{}", idx));
        param_values.push(Box::new(index_id));
    }

    if !library_ids.is_empty() {
        let start = param_values.len() + 1;
        let placeholders: Vec<String> = library_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", start + i))
            .collect();
        sql.push_str(&format!(
            " AND n.library_id IN ({})",
            placeholders.join(",")
        ));
        for &id in library_ids {
            param_values.push(Box::new(id));
        }
    }

    if let Some(lvl) = level {
        let idx = param_values.len() + 1;
        sql.push_str(&format!(" AND n.level = ?{}", idx));
        param_values.push(Box::new(lvl));
    }

    // Cap candidate rows to prevent unbounded memory allocation
    let limit_idx = param_values.len() + 1;
    sql.push_str(&format!(" LIMIT ?{}", limit_idx));
    param_values.push(Box::new(VECTOR_FALLBACK_MAX_ROWS as i64));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        let node_id: i64 = row.get(0)?;
        let blob: Vec<u8> = row.get(1)?;
        Ok((node_id, blob))
    })?;

    let mut candidates: Vec<(i64, f64)> = Vec::new();
    for row in rows {
        let (node_id, blob) = row?;
        let node_embedding = blob_to_embedding(&blob);
        let sim = cosine_similarity_f64(embedding, &node_embedding);
        candidates.push((node_id, sim));
    }

    candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    candidates.truncate(top_k);

    Ok(candidates
        .iter()
        .enumerate()
        .map(|(i, (node_id, _))| RankedResult {
            node_id: *node_id,
            rank: i + 1,
            score: 0.0,
        })
        .collect())
}

/// P4-003: 基于候选 node_id 反查所属 library 的隐私策略，取最严格约束。
///
/// 不依赖 `input.library_ids`（可能为空，表示全库搜索），而是按实际命中的
/// 节点所属 library 逐条判断。任一库禁止外部 rerank 则全局禁用；任一库要求
/// 脱敏则全局启用。
fn resolve_rerank_policy_for_candidate_nodes(
    conn: &Connection,
    merged: &[RankedResult],
) -> Result<(bool, bool)> {
    let placeholders: Vec<String> = merged
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect();
    // 查询所有候选节点所属 library 的隐私策略（DISTINCT 去重）
    let sql = format!(
        "SELECT DISTINCT l.external_rerank_allowed, l.redaction_enabled \
         FROM kb_document_nodes n \
         JOIN kb_libraries l ON l.id = n.library_id \
         WHERE n.id IN ({})",
        placeholders.join(",")
    );
    let param_values: Vec<Box<dyn rusqlite::types::ToSql>> = merged
        .iter()
        .map(|r| Box::new(r.node_id) as Box<dyn rusqlite::types::ToSql>)
        .collect();
    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?))
    })?;

    let mut all_allowed = true;
    let mut any_redaction = false;
    for r in rows {
        let (allowed, redact) = r?;
        if allowed == 0 {
            all_allowed = false;
        }
        if redact != 0 {
            any_redaction = true;
        }
    }
    Ok((all_allowed, any_redaction))
}

/// P3-028: 按 folder_id 过滤结果（仅保留属于指定 folder 的文档的节点）
fn filter_by_folder(
    conn: &Connection,
    merged: Vec<RankedResult>,
    folder_id: i64,
) -> Vec<RankedResult> {
    if merged.is_empty() {
        return merged;
    }
    let placeholders: Vec<String> = merged
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect();
    let sql = format!(
        "SELECT DISTINCT n.id FROM kb_document_nodes n \
         JOIN kb_documents d ON d.id = n.document_id \
         WHERE n.id IN ({}) AND d.folder_id = ?{}",
        placeholders.join(","),
        placeholders.len() + 1,
    );
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = merged
        .iter()
        .map(|r| Box::new(r.node_id) as Box<dyn rusqlite::types::ToSql>)
        .collect();
    param_values.push(Box::new(folder_id));
    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();
    if let Ok(mut stmt) = conn.prepare(&sql) {
        if let Ok(valid_ids) = stmt.query_map(param_refs.as_slice(), |row| row.get::<_, i64>(0)) {
            let valid_set: std::collections::HashSet<i64> =
                valid_ids.filter_map(|r| r.ok()).collect();
            return merged
                .into_iter()
                .filter(|r| valid_set.contains(&r.node_id))
                .collect();
        }
    }
    merged
}

/// Fetch full details for merged results, joining with document and library
/// tables to populate `document_name` and `library_name`.
fn fetch_node_details(
    conn: &Connection,
    merged: &[RankedResult],
    top_k: usize,
) -> Result<Vec<KbSearchResult>> {
    if merged.is_empty() {
        return Ok(Vec::new());
    }

    let node_ids: Vec<i64> = merged.iter().take(top_k).map(|r| r.node_id).collect();
    let score_map: HashMap<i64, f64> = merged
        .iter()
        .take(top_k)
        .map(|r| (r.node_id, r.score))
        .collect();

    let placeholders: Vec<String> = node_ids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect();
    let sql = format!(
        "SELECT n.id, n.document_id, n.content, n.level, \
                d.original_name, n.library_id, l.name, \
                n.title_path, n.page_number \
         FROM kb_document_nodes n \
         INNER JOIN kb_documents d ON d.id = n.document_id \
         INNER JOIN kb_libraries l ON l.id = n.library_id \
         WHERE n.id IN ({})",
        placeholders.join(",")
    );

    let param_values: Vec<Box<dyn rusqlite::types::ToSql>> = node_ids
        .iter()
        .map(|&id| Box::new(id) as Box<dyn rusqlite::types::ToSql>)
        .collect();
    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(KbSearchResult {
            node_id: row.get(0)?,
            document_id: row.get(1)?,
            content: row.get(2)?,
            level: row.get(3)?,
            document_name: row.get(4)?,
            library_id: row.get(5)?,
            library_name: row.get(6)?,
            score: 0.0,
            title_path: row.get::<_, Option<String>>(7)?,
            page_number: row.get(8)?,
            context_before: None,
            context_after: None,
            highlight_ranges: None,
            open_target: None,
            matched_by: None,
            debug_signals: None,
            group_hits: None,
        })
    })?;

    let mut results: Vec<KbSearchResult> = rows.filter_map(|r| r.ok()).collect();

    // Assign RRF scores from the merge step
    for result in &mut results {
        if let Some(&score) = score_map.get(&result.node_id) {
            result.score = score;
        }
    }

    // Sort by descending score
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(results)
}

/// Convert f32 vector to BLOB (little-endian).
///
/// Each f32 is encoded as 4 bytes in little-endian byte order.
/// Compatible with both `kb_node_embeddings.embedding` and `vec_kb_nodes.embedding`.
fn embedding_to_blob(vector: &[f32]) -> Vec<u8> {
    vector.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Decode a BLOB back into an f32 vector.
/// Returns an error if the blob length is not a multiple of 4.
fn blob_to_embedding(blob: &[u8]) -> Vec<f32> {
    if !blob.len().is_multiple_of(4) && !blob.is_empty() {
        tracing::warn!(
            "embedding blob has {} bytes (not multiple of 4); trailing bytes dropped",
            blob.len() % 4
        );
    }
    blob.chunks_exact(4)
        .filter_map(|chunk| {
            let bytes: [u8; 4] = chunk.try_into().ok()?;
            Some(f32::from_le_bytes(bytes))
        })
        .collect()
}

/// Compute cosine similarity between two f32 vectors, returned as f64.
fn cosine_similarity_f64(a: &[f32], b: &[f32]) -> f64 {
    let len = a.len().min(b.len());
    if len == 0 {
        return 0.0;
    }
    let dot: f64 = a[..len]
        .iter()
        .zip(b[..len].iter())
        .map(|(x, y)| (*x as f64) * (*y as f64))
        .sum();
    let norm_a: f64 = a[..len]
        .iter()
        .map(|x| (*x as f64).powi(2))
        .sum::<f64>()
        .sqrt();
    let norm_b: f64 = b[..len]
        .iter()
        .map(|x| (*x as f64).powi(2))
        .sum::<f64>()
        .sqrt();
    if !norm_a.is_finite() || !norm_b.is_finite() || norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    let result = dot / (norm_a * norm_b);
    if result.is_finite() {
        result
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rrf_merge_single_list() {
        let vec_results = vec![
            RankedResult {
                node_id: 1,
                rank: 1,
                score: 0.0,
            },
            RankedResult {
                node_id: 2,
                rank: 2,
                score: 0.0,
            },
        ];
        let fts_results = vec![];
        let merged = rrf_merge(vec_results, fts_results);

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].node_id, 1); // rank 1 should score higher
        assert!(merged[0].score > merged[1].score);
    }

    #[test]
    fn test_rrf_merge_combined() {
        let vec_results = vec![
            RankedResult {
                node_id: 1,
                rank: 1,
                score: 0.0,
            },
            RankedResult {
                node_id: 2,
                rank: 2,
                score: 0.0,
            },
        ];
        let fts_results = vec![
            RankedResult {
                node_id: 2,
                rank: 1,
                score: 0.0,
            },
            RankedResult {
                node_id: 3,
                rank: 2,
                score: 0.0,
            },
        ];
        let merged = rrf_merge(vec_results, fts_results);

        // Node 2 appears in both lists, should rank highest
        assert_eq!(merged[0].node_id, 2);
        assert!(merged[0].score > merged[1].score);
    }

    #[test]
    fn test_embedding_blob_roundtrip() {
        let original: Vec<f32> = vec![0.1, -0.2, 0.3, 0.0, 1.0];
        let blob = embedding_to_blob(&original);
        let decoded = blob_to_embedding(&blob);

        assert_eq!(decoded.len(), original.len());
        for (a, b) in original.iter().zip(decoded.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let v = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity_f64(&v, &v);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity_f64(&a, &b);
        assert!((sim - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 0.0];
        let sim = cosine_similarity_f64(&a, &b);
        assert!((sim - 0.0).abs() < 1e-6);
    }

    fn make_result(
        node_id: i64,
        document_id: i64,
        document_name: &str,
        score: f64,
    ) -> KbSearchResult {
        KbSearchResult {
            node_id,
            document_id,
            document_name: document_name.to_string(),
            content: String::new(),
            level: 0,
            score,
            library_id: 1,
            library_name: "test".to_string(),
            title_path: None,
            page_number: None,
            context_before: None,
            context_after: None,
            highlight_ranges: None,
            open_target: None,
            matched_by: None,
            debug_signals: None,
            group_hits: None,
        }
    }

    #[test]
    fn test_group_by_document_basic() {
        // Doc A: scores 0.9, 0.5 ; Doc B: scores 0.8, 0.3
        let results = vec![
            make_result(1, 100, "DocA", 0.9),
            make_result(2, 100, "DocA", 0.5),
            make_result(3, 200, "DocB", 0.8),
            make_result(4, 200, "DocB", 0.3),
        ];
        let grouped = group_by_document(results);

        // Should have 4 results total (all hits preserved)
        assert_eq!(grouped.len(), 4);

        // First two should be the best hit from each group, interleaved by best_score
        assert_eq!(grouped[0].document_id, 100); // DocA best=0.9
        assert_eq!(grouped[0].score, 0.9);
        assert_eq!(grouped[1].document_id, 200); // DocB best=0.8
        assert_eq!(grouped[1].score, 0.8);

        // Best hits carry group_hits with the remaining hits
        assert!(grouped[0].group_hits.is_some());
        assert_eq!(grouped[0].group_hits.as_ref().unwrap().len(), 1);
        assert_eq!(grouped[0].group_hits.as_ref().unwrap()[0].score, 0.5);

        assert!(grouped[1].group_hits.is_some());
        assert_eq!(grouped[1].group_hits.as_ref().unwrap().len(), 1);
        assert_eq!(grouped[1].group_hits.as_ref().unwrap()[0].score, 0.3);

        // matched_by should be set
        assert_eq!(
            grouped[0].matched_by.as_deref(),
            Some("grouped_by_document")
        );
    }

    #[test]
    fn test_group_by_document_single_doc() {
        let results = vec![
            make_result(1, 100, "DocA", 0.9),
            make_result(2, 100, "DocA", 0.5),
        ];
        let grouped = group_by_document(results);
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[0].score, 0.9);
        assert_eq!(grouped[1].score, 0.5);
    }

    #[test]
    fn test_group_by_document_empty() {
        let results: Vec<KbSearchResult> = vec![];
        let grouped = group_by_document(results);
        assert!(grouped.is_empty());
    }

    #[test]
    fn test_group_by_document_three_docs() {
        // Doc A: 0.95, 0.7 ; Doc B: 0.85 ; Doc C: 0.6, 0.4, 0.2
        let results = vec![
            make_result(1, 100, "DocA", 0.95),
            make_result(2, 100, "DocA", 0.7),
            make_result(3, 200, "DocB", 0.85),
            make_result(4, 300, "DocC", 0.6),
            make_result(5, 300, "DocC", 0.4),
            make_result(6, 300, "DocC", 0.2),
        ];
        let grouped = group_by_document(results);

        assert_eq!(grouped.len(), 6);

        // First pass: best of each group, ordered by best_score desc
        assert_eq!(grouped[0].document_id, 100); // 0.95
        assert_eq!(grouped[1].document_id, 200); // 0.85
        assert_eq!(grouped[2].document_id, 300); // 0.6

        // DocC has 3 hits, so group_hits has 2 entries
        assert_eq!(grouped[2].group_hits.as_ref().unwrap().len(), 2);
    }
}
