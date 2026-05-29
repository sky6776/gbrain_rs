//! 统一查询引擎 — BrainFirst / EvidenceFirst / Provenance / TimelineFirst 四种策略
//!
//! 负责：
//! 1. 根据查询意图自动选择策略
//! 2. BrainFirst: 先查 gbrain，再查 KB 补充
//! 3. EvidenceFirst: 先查 KB 证据，再查 gbrain 上下文
//! 4. Provenance: 仅追溯来源链
//! 5. TimelineFirst: 先查时间线事件，再查 gbrain 上下文（§11.1/§11.2）

use rusqlite::{params, Connection};
use std::collections::{HashMap, HashSet};
use tracing::{debug, info};

use crate::config::Config;
use crate::error::{GBrainError, Result};
use crate::search::hybrid::{hybrid_search, HybridOpts};
use crate::search::intent::{classify_intent, Intent};

use super::projection;
use super::provenance;
use super::store;
use super::types::*;

/// 根据查询文本自动推断策略
pub fn infer_query_strategy(query: &str) -> QueryStrategy {
    // 使用现有意图分类器推断
    let intent = classify_intent(query);

    // 根据意图映射到策略
    let strategy = match intent.intent {
        // "是谁/是什么" 类问题 → BrainFirst
        Intent::Entity => QueryStrategy::BrainFirst,
        // "时间线/最近发生了什么" 类问题 → TimelineFirst（§11.2）
        Intent::Temporal => QueryStrategy::TimelineFirst,
        // "事件" 类问题 → TimelineFirst（§11.2: 事件与时间线紧密关联）
        Intent::Event => QueryStrategy::TimelineFirst,
        // 默认 → BrainFirst + KB fallback
        Intent::General => QueryStrategy::BrainFirst,
    };
    debug!(
        "infer_query_strategy: query={}, strategy={}",
        query, strategy
    );
    strategy
}

/// 执行统一查询
pub fn unified_query(
    conn: &Connection,
    input: &UnifiedQueryInput,
    engine: &crate::sqlite_engine::SqliteEngine,
    config: &crate::config::Config,
) -> Result<UnifiedQueryResult> {
    let strategy = if input.strategy == QueryStrategy::BrainFirst {
        // 尝试自动推断
        infer_query_strategy(&input.query)
    } else {
        input.strategy.clone()
    };

    info!("统一查询: query={}, strategy={}", input.query, strategy);

    // 修复：限制 limit 在 1..=100 范围内，防止负数导致 usize 溢出产生无限 SQL LIMIT
    let limit = input.limit.unwrap_or(10).clamp(1, 100);

    let mut brain_hits = Vec::new();
    let mut evidence_hits = Vec::new();
    let mut provenance_records = Vec::new();
    let mut timeline_hits = Vec::new();

    match strategy {
        QueryStrategy::BrainFirst => {
            // 1. 先查 gbrain
            // 修复：把 filter_slug 下推到 query_brain，在搜索阶段就限定 slug，
            // 避免先全局 LIMIT 再后置 retain 导致目标 slug 的命中被挤掉
            brain_hits = query_brain(
                conn,
                &input.query,
                limit,
                engine,
                config,
                input.filter_slug.as_deref(),
            )?;

            // 2. 如果 gbrain 结果不足，查 KB 补充
            if brain_hits.len() < limit as usize && input.include_evidence {
                evidence_hits = query_kb_evidence(
                    conn,
                    &input.query,
                    limit - brain_hits.len() as i64,
                    input.filter_slug.as_deref(),
                    config,
                )?;
            }

            // 3. 给 brain hit 附加 provenance
            // 修复：只收集 filter_slug 匹配的 brain_hits 的 provenance，
            // 避免提前收集非目标 slug 的来源记录
            if input.include_provenance {
                for hit in &brain_hits {
                    let prov = provenance::find_provenance_by_brain_slug(conn, &hit.slug)?;
                    provenance_records.extend(prov);
                }
            }
        }
        QueryStrategy::EvidenceFirst => {
            // 1. 先查 KB
            // 修复：把 filter_slug 下推到 query_kb_evidence
            evidence_hits = query_kb_evidence(
                conn,
                &input.query,
                limit,
                input.filter_slug.as_deref(),
                config,
            )?;

            // 2. 给 KB hit 附加 artifact 和 shadow page 信息
            // 修复：query_kb_evidence 创建 EvidenceHit 时 projections 恒为空，
            // 从 hit.projections 找 kb_document_id 永远找不到。
            // 应直接用 hit.kb_document_id 查 artifact_projections
            // 修复：当 filter_slug 存在且 shadow_page_slug 已由 SQL JOIN 正确填入时，
            // 跳过无约束的 projection 查找，避免同一 kb_document 被多个 artifact 复用时
            // 拿到错误 artifact 的投影并覆盖正确的 shadow_page_slug。
            for hit in &mut evidence_hits {
                if hit.kb_document_id > 0 && hit.shadow_page_slug.is_none() {
                    let kb_doc_id = hit.kb_document_id;
                    // 查找关联的 artifact 投影
                    let proj = store::find_projection_by_ref(
                        conn,
                        "kb_document",
                        &format!("kb_document:{}", kb_doc_id),
                    )
                    .map_err(|e| GBrainError::Database(format!("查找投影失败: {}", e)))?;
                    if let Some(proj) = proj {
                        let artifact =
                            store::find_artifact_by_id(conn, proj.artifact_id).map_err(|e| {
                                GBrainError::Database(format!("查找 artifact 失败: {}", e))
                            })?;
                        hit.artifact = artifact;

                        // 查找影子页面
                        let shadow_slug =
                            projection::find_shadow_page_slug(conn, proj.artifact_id)?;
                        hit.shadow_page_slug = shadow_slug;
                    }
                }
            }

            // 3. 查 gbrain 上下文
            if brain_hits.len() < limit as usize {
                // 修复：把 filter_slug 下推到 query_brain
                brain_hits = query_brain(
                    conn,
                    &input.query,
                    limit / 2,
                    engine,
                    config,
                    input.filter_slug.as_deref(),
                )?;
            }

            // 4. 给 brain hit 附加 provenance
            // 修复：只收集 filter_slug 匹配的 brain_hits 的 provenance
            if input.include_provenance {
                for hit in &brain_hits {
                    let prov = provenance::find_provenance_by_brain_slug(conn, &hit.slug)?;
                    provenance_records.extend(prov);
                }
            }
        }
        QueryStrategy::Provenance => {
            // 仅追溯来源链
            if let Some(slug) = &input.filter_slug {
                provenance_records = provenance::find_provenance_by_brain_slug(conn, slug)?;
            } else {
                // 尝试从查询中推断 slug
                let brain_hits_tmp = query_brain(conn, &input.query, 1, engine, config, None)?;
                if let Some(hit) = brain_hits_tmp.first() {
                    provenance_records =
                        provenance::find_provenance_by_brain_slug(conn, &hit.slug)?;
                }
            }
        }
        QueryStrategy::TimelineFirst => {
            // §11.2: "最近发生了什么 / 时间线" → 先查时间线事件，再查 gbrain 上下文
            // 1. 查询已接受的 timeline_event 候选
            // 修复：把 filter_slug 下推到 query_timeline_events
            timeline_hits =
                query_timeline_events(conn, &input.query, limit, input.filter_slug.as_deref())?;

            // 2. 给时间线命中附加 shadow page 信息
            for hit in &mut timeline_hits {
                let shadow_slug = projection::find_shadow_page_slug(conn, hit.artifact_id)?;
                hit.shadow_page_slug = shadow_slug;
            }

            // 3. 查 gbrain 上下文补充
            if timeline_hits.len() < limit as usize {
                let remaining = limit - timeline_hits.len() as i64;
                // 修复：把 filter_slug 下推到 query_brain
                brain_hits = query_brain(
                    conn,
                    &input.query,
                    remaining,
                    engine,
                    config,
                    input.filter_slug.as_deref(),
                )?;
            }

            // 4. 给 brain hit 附加 provenance
            // 修复：只收集 filter_slug 匹配的 brain_hits 的 provenance
            if input.include_provenance {
                for hit in &brain_hits {
                    let prov = provenance::find_provenance_by_brain_slug(conn, &hit.slug)?;
                    provenance_records.extend(prov);
                }
            }
        }
    }

    // 修复：filter_slug 已下推到各查询函数，不再需要后置 retain。
    // 保留此块作为防御性兜底，确保即使下推逻辑有遗漏也不会泄漏非目标 slug 的结果
    if let Some(slug) = &input.filter_slug {
        brain_hits.retain(|hit| hit.slug == *slug);
        evidence_hits.retain(|hit| hit.shadow_page_slug.as_deref() == Some(slug.as_str()));
        timeline_hits.retain(|hit| hit.shadow_page_slug.as_deref() == Some(slug.as_str()));
        // 修复：同步过滤 provenance_records，只保留目标 slug 的来源记录
        provenance_records.retain(|rec| rec.brain_slug == *slug);
    }

    let total_hits =
        brain_hits.len() as i64 + evidence_hits.len() as i64 + timeline_hits.len() as i64;

    Ok(UnifiedQueryResult {
        strategy: strategy.to_string(),
        brain_hits,
        evidence_hits,
        timeline_hits,
        provenance_records,
        total_hits,
    })
}

/// 查询 gbrain
/// 修复：当提供 filter_slug 时，通过 SearchOpts.include_slugs 下推精确 slug 过滤到搜索层，
/// 让搜索引擎只返回目标 slug 的页面，避免先全局 LIMIT 再后置 retain 导致假阴性。
fn query_brain(
    _conn: &Connection,
    query: &str,
    limit: i64,
    engine: &crate::sqlite_engine::SqliteEngine,
    _config: &crate::config::Config,
    filter_slug: Option<&str>,
) -> Result<Vec<BrainHit>> {
    // 修复：使用 include_slugs 精确限定搜索范围，不再扩大 limit * 3 后后置过滤
    let search_opts = crate::types::SearchOpts {
        limit: Some(limit as usize),
        include_slugs: filter_slug.map(|s| vec![s.to_string()]),
        ..Default::default()
    };
    let hybrid_opts = HybridOpts::default();

    let search_result = hybrid_search(engine, query, None, search_opts, hybrid_opts)
        .map_err(|e| GBrainError::Search(format!("gbrain 搜索失败: {}", e)))?;

    let hits: Vec<BrainHit> = search_result
        .results
        .into_iter()
        .map(|r| BrainHit {
            slug: r.slug,
            title: r.title,
            snippet: r.chunk_text,
            relevance: r.score,
            provenance: Vec::new(),
        })
        .collect();

    debug!(
        "query_brain: query={}, filter_slug={:?}, count={}",
        query,
        filter_slug,
        hits.len()
    );
    Ok(hits)
}

/// 查询 KB 证据
/// 修复：增加 filter_slug 参数，下推 slug 过滤到 SQL 查询阶段，
/// 通过 JOIN artifact_projections 限定只返回目标 slug 的 KB 文档，
/// 避免先全局 LIMIT 再后置 retain 导致目标 slug 的命中被挤掉
fn query_kb_evidence(
    conn: &Connection,
    query: &str,
    limit: i64,
    filter_slug: Option<&str>,
    config: &Config,
) -> Result<Vec<EvidenceHit>> {
    let plan = EvidenceQueryPlan::from_query(query);
    if plan.relaxed_match.is_empty() && plan.strict_match.is_none() {
        return Ok(Vec::new());
    }

    let limit_usize = limit.clamp(1, 100) as usize;
    let fetch_k = (limit_usize * 8).max(50).min(500);

    let mut routes: Vec<Vec<EvidenceCandidate>> = Vec::new();

    if let Some(strict_match) = &plan.strict_match {
        let strict_nodes = query_node_candidates(conn, strict_match, fetch_k, filter_slug, 1.3)?;
        ensure_passages_for_candidates(conn, &strict_nodes)?;
        routes.push(strict_nodes);
        routes.push(query_passage_candidates(
            conn,
            strict_match,
            fetch_k,
            filter_slug,
            1.6,
        )?);
    }

    let relaxed_nodes =
        query_node_candidates(conn, &plan.relaxed_match, fetch_k, filter_slug, 0.8)?;
    ensure_passages_for_candidates(conn, &relaxed_nodes)?;
    routes.push(relaxed_nodes);
    routes.push(query_passage_candidates(
        conn,
        &plan.relaxed_match,
        fetch_k,
        filter_slug,
        1.0,
    )?);

    let mut candidates = merge_evidence_routes(routes);
    if candidates.is_empty() {
        return Ok(Vec::new());
    }

    for candidate in &mut candidates {
        focus_evidence_candidate_content(candidate, &plan);
        candidate.local_score = score_evidence_candidate(candidate, &plan);
    }

    candidates = rerank_evidence_candidates(conn, query, candidates, &plan, config)?;
    candidates = dedup_evidence_by_document(candidates);
    candidates.truncate(limit_usize);

    debug!(
        "query_kb_evidence: query={}, filter_slug={:?}, count={}, core_terms={:?}",
        query,
        filter_slug,
        candidates.len(),
        plan.core_terms
    );

    candidates
        .into_iter()
        .map(|candidate| candidate.into_hit(conn, query, filter_slug))
        .collect()
}

#[derive(Debug, Clone)]
struct EvidenceQueryPlan {
    core_terms: Vec<String>,
    weak_terms: Vec<String>,
    relaxed_match: String,
    strict_match: Option<String>,
}

impl EvidenceQueryPlan {
    fn from_query(query: &str) -> Self {
        let relaxed_match = crate::nlp::chinese::build_fts_match_query(query);
        let original_lower = query.to_lowercase();
        let mut seen = HashSet::new();
        let mut core_terms = Vec::new();
        let mut weak_terms = Vec::new();

        for token in crate::nlp::chinese::tokenize_content(query).split_whitespace() {
            let token = crate::nlp::chinese::normalize_token(token);
            if token.is_empty() || !seen.insert(token.clone()) {
                continue;
            }
            if is_weak_query_token(&token) {
                weak_terms.push(token);
                continue;
            }
            let has_chinese = crate::nlp::chinese::has_chinese(&token);
            let appears_as_ascii = !has_chinese && original_lower.contains(&token);
            if has_chinese || appears_as_ascii || is_domain_abbreviation(&token) {
                let char_len = token.chars().count();
                if char_len >= 2 || is_domain_abbreviation(&token) {
                    core_terms.push(token);
                }
            }
        }

        let strict_match = if core_terms.len() >= 2 {
            Some(build_and_match_query(&core_terms))
        } else {
            None
        };

        Self {
            core_terms,
            weak_terms,
            relaxed_match,
            strict_match,
        }
    }
}

#[derive(Debug, Clone)]
struct EvidenceCandidate {
    candidate_id: i64,
    passage_id: Option<i64>,
    kb_document_id: i64,
    library_id: i64,
    title: String,
    content: String,
    level: i64,
    artifact_id: i64,
    view_type: String,
    route_score: f64,
    local_score: f64,
    final_score: f64,
}

impl EvidenceCandidate {
    fn into_hit(
        self,
        conn: &Connection,
        query: &str,
        filter_slug: Option<&str>,
    ) -> Result<EvidenceHit> {
        let artifact = if self.artifact_id > 0 {
            super::store::find_artifact_by_id(conn, self.artifact_id)
                .ok()
                .flatten()
        } else {
            None
        };

        Ok(EvidenceHit {
            kb_document_id: self.kb_document_id,
            title: self.title,
            snippet: query_centered_snippet(&self.content, query),
            relevance: self.final_score.max(self.local_score).max(self.route_score),
            artifact,
            shadow_page_slug: filter_slug.map(|s| s.to_string()),
            projections: Vec::new(),
        })
    }
}

fn query_node_candidates(
    conn: &Connection,
    fts_query: &str,
    fetch_k: usize,
    filter_slug: Option<&str>,
    route_weight: f64,
) -> Result<Vec<EvidenceCandidate>> {
    if fts_query.is_empty() {
        return Ok(Vec::new());
    }

    let sql = if filter_slug.is_some() {
        "SELECT dn.id, dn.document_id, dn.library_id, dn.content, dn.level,
            d.original_name, d.title, ap_kb.artifact_id, bm25(kb_doc_fts) AS bm25_score
         FROM kb_doc_fts fts
         JOIN kb_document_nodes dn ON dn.id = fts.rowid
         JOIN kb_documents d ON d.id = dn.document_id
         JOIN artifact_projections ap_kb ON ap_kb.projection_type = 'kb_document'
              AND ap_kb.projection_ref = 'kb_document:' || dn.document_id
              AND ap_kb.status = 'active'
         JOIN artifact_projections ap_sp ON ap_sp.artifact_id = ap_kb.artifact_id
              AND ap_sp.projection_type = 'brain_shadow_page'
              AND ap_sp.projection_ref = ?3
              AND ap_sp.status = 'active'
         WHERE kb_doc_fts MATCH ?1
           AND d.document_status != 'deleted'
           AND d.deleted_at IS NULL
         ORDER BY bm25_score ASC
         LIMIT ?2"
    } else {
        "SELECT dn.id, dn.document_id, dn.library_id, dn.content, dn.level,
            d.original_name, d.title, 0 AS artifact_id, bm25(kb_doc_fts) AS bm25_score
         FROM kb_doc_fts fts
         JOIN kb_document_nodes dn ON dn.id = fts.rowid
         JOIN kb_documents d ON d.id = dn.document_id
         WHERE kb_doc_fts MATCH ?1
           AND d.document_status != 'deleted'
           AND d.deleted_at IS NULL
         ORDER BY bm25_score ASC
         LIMIT ?2"
    };

    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| GBrainError::Database(format!("准备 KB node 搜索失败: {}", e)))?;

    let map_row = |rank: usize, row: &rusqlite::Row<'_>| -> rusqlite::Result<EvidenceCandidate> {
        let node_id: i64 = row.get(0)?;
        let kb_document_id: i64 = row.get(1)?;
        let library_id: i64 = row.get(2)?;
        let content: String = row.get(3)?;
        let level: i64 = row.get(4)?;
        let original_name: String = row.get(5)?;
        let doc_title: String = row.get(6)?;
        let artifact_id: i64 = row.get(7)?;
        let title = if doc_title.is_empty() {
            original_name
        } else {
            doc_title
        };
        Ok(EvidenceCandidate {
            candidate_id: -node_id,
            passage_id: None,
            kb_document_id,
            library_id,
            title,
            content,
            level,
            artifact_id,
            view_type: "node".to_string(),
            route_score: route_weight / (60.0 + rank as f64 + 1.0),
            local_score: 0.0,
            final_score: 0.0,
        })
    };

    let mut out = Vec::new();
    if let Some(slug) = filter_slug {
        let slug_ref = format!("slug:{}", slug);
        let mut rows = stmt
            .query(params![fts_query, fetch_k as i64, slug_ref])
            .map_err(|e| GBrainError::Database(format!("KB node 搜索失败: {}", e)))?;
        let mut rank = 0;
        while let Some(row) = rows.next()? {
            out.push(map_row(rank, row)?);
            rank += 1;
        }
    } else {
        let mut rows = stmt
            .query(params![fts_query, fetch_k as i64])
            .map_err(|e| GBrainError::Database(format!("KB node 搜索失败: {}", e)))?;
        let mut rank = 0;
        while let Some(row) = rows.next()? {
            out.push(map_row(rank, row)?);
            rank += 1;
        }
    }
    Ok(out)
}

fn query_passage_candidates(
    conn: &Connection,
    fts_query: &str,
    fetch_k: usize,
    filter_slug: Option<&str>,
    route_weight: f64,
) -> Result<Vec<EvidenceCandidate>> {
    if fts_query.is_empty() {
        return Ok(Vec::new());
    }

    let sql = if filter_slug.is_some() {
        "SELECT ps.id, ps.node_id, ps.document_id, ps.library_id, ps.content,
            ps.view_type, dn.level, d.original_name, d.title,
            ap_kb.artifact_id, bm25(kb_passage_fts) AS bm25_score
         FROM kb_passage_fts fts
         JOIN kb_passage_spans ps ON ps.id = fts.rowid
         JOIN kb_document_nodes dn ON dn.id = ps.node_id
         JOIN kb_documents d ON d.id = ps.document_id
         JOIN artifact_projections ap_kb ON ap_kb.projection_type = 'kb_document'
              AND ap_kb.projection_ref = 'kb_document:' || ps.document_id
              AND ap_kb.status = 'active'
         JOIN artifact_projections ap_sp ON ap_sp.artifact_id = ap_kb.artifact_id
              AND ap_sp.projection_type = 'brain_shadow_page'
              AND ap_sp.projection_ref = ?3
              AND ap_sp.status = 'active'
         WHERE kb_passage_fts MATCH ?1
           AND d.document_status != 'deleted'
           AND d.deleted_at IS NULL
         ORDER BY bm25_score ASC
         LIMIT ?2"
    } else {
        "SELECT ps.id, ps.node_id, ps.document_id, ps.library_id, ps.content,
            ps.view_type, dn.level, d.original_name, d.title,
            0 AS artifact_id, bm25(kb_passage_fts) AS bm25_score
         FROM kb_passage_fts fts
         JOIN kb_passage_spans ps ON ps.id = fts.rowid
         JOIN kb_document_nodes dn ON dn.id = ps.node_id
         JOIN kb_documents d ON d.id = ps.document_id
         WHERE kb_passage_fts MATCH ?1
           AND d.document_status != 'deleted'
           AND d.deleted_at IS NULL
         ORDER BY bm25_score ASC
         LIMIT ?2"
    };

    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| GBrainError::Database(format!("准备 KB passage 搜索失败: {}", e)))?;

    let map_row = |rank: usize, row: &rusqlite::Row<'_>| -> rusqlite::Result<EvidenceCandidate> {
        let passage_id: i64 = row.get(0)?;
        let _node_id: i64 = row.get(1)?;
        let kb_document_id: i64 = row.get(2)?;
        let library_id: i64 = row.get(3)?;
        let content: String = row.get(4)?;
        let view_type: String = row.get(5)?;
        let level: i64 = row.get(6)?;
        let original_name: String = row.get(7)?;
        let doc_title: String = row.get(8)?;
        let artifact_id: i64 = row.get(9)?;
        let title = if doc_title.is_empty() {
            original_name
        } else {
            doc_title
        };
        Ok(EvidenceCandidate {
            candidate_id: passage_id,
            passage_id: Some(passage_id),
            kb_document_id,
            library_id,
            title,
            content,
            level,
            artifact_id,
            view_type,
            route_score: route_weight / (60.0 + rank as f64 + 1.0),
            local_score: 0.0,
            final_score: 0.0,
        })
    };

    let mut out = Vec::new();
    if let Some(slug) = filter_slug {
        let slug_ref = format!("slug:{}", slug);
        let mut rows = stmt
            .query(params![fts_query, fetch_k as i64, slug_ref])
            .map_err(|e| GBrainError::Database(format!("KB passage 搜索失败: {}", e)))?;
        let mut rank = 0;
        while let Some(row) = rows.next()? {
            out.push(map_row(rank, row)?);
            rank += 1;
        }
    } else {
        let mut rows = stmt
            .query(params![fts_query, fetch_k as i64])
            .map_err(|e| GBrainError::Database(format!("KB passage 搜索失败: {}", e)))?;
        let mut rank = 0;
        while let Some(row) = rows.next()? {
            out.push(map_row(rank, row)?);
            rank += 1;
        }
    }
    Ok(out)
}

fn ensure_passages_for_candidates(
    conn: &Connection,
    candidates: &[EvidenceCandidate],
) -> Result<()> {
    let mut docs = HashSet::new();
    for candidate in candidates {
        docs.insert(candidate.kb_document_id);
    }
    for doc_id in docs {
        crate::kb::passage::ensure_document_passages(conn, doc_id)?;
    }
    Ok(())
}

fn merge_evidence_routes(routes: Vec<Vec<EvidenceCandidate>>) -> Vec<EvidenceCandidate> {
    let mut merged: HashMap<i64, EvidenceCandidate> = HashMap::new();
    for route in routes {
        for candidate in route {
            merged
                .entry(candidate.candidate_id)
                .and_modify(|existing| {
                    existing.route_score += candidate.route_score;
                    if candidate.content.chars().count() < existing.content.chars().count()
                        && candidate.passage_id.is_some()
                    {
                        existing.content = candidate.content.clone();
                        existing.view_type = candidate.view_type.clone();
                        existing.passage_id = candidate.passage_id;
                    }
                })
                .or_insert(candidate);
        }
    }
    let mut out: Vec<EvidenceCandidate> = merged.into_values().collect();
    out.sort_by(|a, b| {
        b.route_score
            .partial_cmp(&a.route_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

fn dedup_evidence_by_document(candidates: Vec<EvidenceCandidate>) -> Vec<EvidenceCandidate> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for candidate in candidates {
        if seen.insert(candidate.kb_document_id) {
            out.push(candidate);
        }
    }
    out
}

fn focus_evidence_candidate_content(candidate: &mut EvidenceCandidate, plan: &EvidenceQueryPlan) {
    if candidate.content.chars().count() <= RERANK_EXCERPT_CHARS {
        return;
    }
    if let Some(excerpt) =
        query_focused_excerpt(&candidate.content, &plan.core_terms, RERANK_EXCERPT_CHARS)
    {
        candidate.content = excerpt;
    }
}

fn rerank_evidence_candidates(
    conn: &Connection,
    query: &str,
    mut candidates: Vec<EvidenceCandidate>,
    plan: &EvidenceQueryPlan,
    config: &Config,
) -> Result<Vec<EvidenceCandidate>> {
    candidates.sort_by(|a, b| {
        let b_score = b.local_score + b.route_score * 5.0;
        let a_score = a.local_score + a.route_score * 5.0;
        b_score
            .partial_cmp(&a_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // P1: 读取库级 rerank_enabled / rerank_provider，与 kb_search 策略保持一致
    let (external_allowed, redaction_enabled, lib_rerank_enabled, rerank_provider_str) =
        resolve_evidence_rerank_policy(conn, &candidates)?;
    let rerank_cfg = crate::kb::rerank::RerankConfig {
        model_rerank_enabled: lib_rerank_enabled,
        rerank_provider: if rerank_provider_str.is_empty() {
            "chat_completions".to_string()
        } else {
            rerank_provider_str
        },
        rerank_model: config.expansion_model.clone(),
        rerank_timeout_ms: 5000,
        rerank_max_candidates: 50,
        external_rerank_allowed: external_allowed,
    };

    let local_candidates: Vec<(i64, crate::kb::rerank::LocalRankSignals)> = candidates
        .iter()
        .map(|c| {
            (
                c.candidate_id,
                crate::kb::rerank::LocalRankSignals {
                    fts_score: c.route_score,
                    exact_match_score: c.local_score,
                    granularity_score: view_type_score(&c.view_type),
                    ..Default::default()
                },
            )
        })
        .collect();

    let candidate_texts: Vec<crate::kb::rerank::RerankCandidate> = candidates
        .iter()
        .take(rerank_cfg.rerank_max_candidates)
        .map(|c| {
            let text = if redaction_enabled && external_allowed {
                crate::kb::privacy::redact_content(&c.content)
            } else {
                c.content.clone()
            };
            crate::kb::rerank::RerankCandidate {
                doc_id: c.candidate_id,
                text,
            }
        })
        .collect();

    let api_key = config.expansion_api_key_resolved().unwrap_or("");
    let base_url = config
        .expansion_base_url_resolved()
        .filter(|s| !s.is_empty())
        .unwrap_or("https://api.openai.com/v1");
    let weights = vec![0.25, 0.0, 0.0, 0.65, 0.0, 0.0];
    let (scored, rerank_info) = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt.block_on(crate::kb::rerank::try_model_rerank_simple(
            &rerank_cfg,
            query,
            &local_candidates,
            &candidate_texts,
            &weights,
            None,
            base_url,
            api_key,
        )),
        Err(_) => (
            crate::kb::rerank::local_rerank(&local_candidates, &weights),
            crate::kb::rerank::RerankResult {
                model_rerank_attempted: false,
                model_rerank_succeeded: false,
                fallback_used: true,
                fallback_reason: Some(crate::kb::rerank::FallbackReason::ApiError),
                provider: "local".to_string(),
                candidates_reranked: candidates.len(),
            },
        ),
    };

    // P2+P3: 在调用完成后按实际结果写审计，每个涉及的库独立记录一条
    let should_audit =
        external_allowed && rerank_cfg.model_rerank_enabled && !api_key.is_empty();
    if should_audit {
        // 仅收集实际发给外部模型的候选库（与 candidate_texts 截断一致）
        let distinct_library_ids: HashSet<i64> = candidates
            .iter()
            .take(rerank_cfg.rerank_max_candidates)
            .map(|c| c.library_id)
            .collect();
        let succeeded = rerank_info.model_rerank_succeeded;
        let error_msg = if succeeded {
            ""
        } else {
            rerank_info
                .fallback_reason
                .as_ref()
                .map(|r| r.as_str())
                .unwrap_or("unknown")
        };
        for lib_id in distinct_library_ids {
            let _ = crate::kb::privacy::log_external_model_call(
                conn,
                Some(lib_id),
                None,
                "rerank",
                &rerank_cfg.rerank_provider,
                &rerank_cfg.rerank_model,
                query.len() as i32,
                candidate_texts.len() as i32,
                0,
                0.0,
                succeeded,
                error_msg,
            );
        }
    }

    let model_succeeded = rerank_info.model_rerank_succeeded;
    let score_by_id: HashMap<i64, f64> = scored.into_iter().collect();
    let (local_min, local_max) = score_bounds(candidates.iter().map(|c| c.local_score));
    let (route_min, route_max) = score_bounds(candidates.iter().map(|c| c.route_score));
    for candidate in &mut candidates {
        candidate.final_score = if model_succeeded {
            if let Some(model_score) = score_by_id.get(&candidate.candidate_id).copied() {
                let local = normalize_score(candidate.local_score, local_min, local_max);
                let route = normalize_score(candidate.route_score, route_min, route_max);
                model_score.clamp(0.0, 1.0) * 0.75 + local * 0.20 + route * 0.05
            } else {
                // 模型未返回该候选分数（如部分 batch 失败），保留本地排序分
                candidate.local_score * 0.80 + candidate.route_score * 0.20
            }
        } else {
            candidate.local_score * 0.80 + candidate.route_score * 0.20
        };
    }

    candidates.sort_by(|a, b| {
        b.final_score
            .partial_cmp(&a.final_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    apply_quality_guard(&mut candidates, plan);
    Ok(candidates)
}

fn score_bounds(scores: impl Iterator<Item = f64>) -> (f64, f64) {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for score in scores {
        if score < min {
            min = score;
        }
        if score > max {
            max = score;
        }
    }
    if min.is_finite() && max.is_finite() {
        (min, max)
    } else {
        (0.0, 0.0)
    }
}

fn normalize_score(score: f64, min: f64, max: f64) -> f64 {
    let width = max - min;
    if width.abs() < f64::EPSILON {
        return if score > 0.0 { 1.0 } else { 0.0 };
    }
    ((score - min) / width).clamp(0.0, 1.0)
}

fn resolve_evidence_rerank_policy(
    conn: &Connection,
    candidates: &[EvidenceCandidate],
) -> Result<(bool, bool, bool, String)> {
    // 返回: (external_rerank_allowed, redaction_enabled, rerank_enabled, rerank_provider)
    let mut library_ids = HashSet::new();
    for candidate in candidates {
        library_ids.insert(candidate.library_id);
    }

    let mut external_allowed = true;
    let mut redaction_enabled = false;
    let mut rerank_enabled = true;
    let mut rerank_provider = String::new();
    for library_id in library_ids {
        let policy = conn.query_row(
            "SELECT external_rerank_allowed, redaction_enabled, rerank_enabled, rerank_provider \
             FROM kb_libraries WHERE id = ?1",
            params![library_id],
            |row| {
                Ok((
                    row.get::<_, i32>(0)?,
                    row.get::<_, i32>(1)?,
                    row.get::<_, i32>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        );
        if let Ok((allowed, redact, re_enabled, re_provider)) = policy {
            if allowed == 0 {
                external_allowed = false;
            }
            if redact != 0 {
                redaction_enabled = true;
            }
            // 任一库禁用 rerank 则全局禁用，取最保守策略
            if re_enabled == 0 {
                rerank_enabled = false;
            }
            if rerank_provider.is_empty() && !re_provider.is_empty() {
                rerank_provider = re_provider;
            }
        }
    }
    Ok((external_allowed, redaction_enabled, rerank_enabled, rerank_provider))
}

const RERANK_EXCERPT_CHARS: usize = 1200;

fn query_focused_excerpt(content: &str, terms: &[String], limit: usize) -> Option<String> {
    if terms.is_empty() {
        return None;
    }

    let folded = content.to_ascii_lowercase();
    let chars: Vec<char> = content.chars().collect();
    let content_len = chars.len();
    if content_len <= limit {
        return Some(content.to_string());
    }

    let mut positions = Vec::new();
    for term in terms {
        let term = term.to_ascii_lowercase();
        if term.is_empty() {
            continue;
        }

        let mut search_from = 0;
        let mut hits_for_term = 0;
        while search_from < folded.len() && hits_for_term < 32 {
            let Some(relative) = folded[search_from..].find(&term) else {
                break;
            };
            let byte_pos = search_from + relative;
            let char_pos = content[..byte_pos].chars().count();
            positions.push(char_pos);
            search_from = byte_pos + term.len();
            hits_for_term += 1;
        }
    }
    if positions.is_empty() {
        return None;
    }

    let mut best_start = 0usize;
    let mut best_score = f64::MIN;
    for pos in positions {
        let mut start = pos.saturating_sub(limit / 2);
        if start + limit > content_len {
            start = content_len.saturating_sub(limit);
        }
        let end = (start + limit).min(content_len);
        let excerpt: String = chars[start..end].iter().collect();
        let folded_excerpt = excerpt.to_ascii_lowercase();
        let coverage = terms
            .iter()
            .filter(|term| folded_excerpt.contains(&term.to_ascii_lowercase()))
            .count();
        let center = start + (end - start) / 2;
        let distance = center.abs_diff(pos) as f64;
        let score = coverage as f64 * 1000.0 - distance;
        if score > best_score {
            best_score = score;
            best_start = start;
        }
    }

    let end = (best_start + limit).min(content_len);
    let excerpt: String = chars[best_start..end].iter().collect();
    Some(format!(
        "{}{}{}",
        if best_start > 0 { "..." } else { "" },
        excerpt,
        if end < content_len { "..." } else { "" }
    ))
}

fn score_evidence_candidate(candidate: &EvidenceCandidate, plan: &EvidenceQueryPlan) -> f64 {
    let content = candidate.content.to_lowercase();
    let title = candidate.title.to_lowercase();
    let mut score = candidate.route_score * 5.0;

    let mut covered = 0usize;
    let mut positions = Vec::new();
    for term in &plan.core_terms {
        let term_lower = term.to_lowercase();
        let content_pos = content.find(&term_lower);
        let title_hit = title.contains(&term_lower);
        if content_pos.is_some() || title_hit {
            covered += 1;
            score += 2.5 + (term.chars().count() as f64 * 0.15).min(0.8);
            if title_hit {
                score += 1.0;
            }
            if let Some(pos) = content_pos {
                positions.push(pos);
            }
        }
    }

    if !plan.core_terms.is_empty() {
        score += 4.0 * (covered as f64 / plan.core_terms.len() as f64);
    }
    if covered >= 2 {
        score += 2.5;
    }
    if covered == plan.core_terms.len() && covered > 0 {
        score += 2.0;
    }
    if let Some(span) = position_span(&positions) {
        if span <= 120 {
            score += 3.0;
        } else if span <= 300 {
            score += 1.5;
        }
    }

    for term in &plan.weak_terms {
        if content.contains(term) || title.contains(term) {
            score += 0.15;
        }
    }

    score += view_type_score(&candidate.view_type);
    score += 1.0 / (candidate.level as f64 + 1.0);

    let len = candidate.content.chars().count();
    if len > 2500 {
        score -= 1.5;
    } else if len <= 900 {
        score += 0.8;
    }

    score.max(0.0)
}

fn apply_quality_guard(candidates: &mut [EvidenceCandidate], plan: &EvidenceQueryPlan) {
    if plan.core_terms.len() < 2 || candidates.len() < 2 {
        return;
    }
    let required = plan.core_terms.len().min(2);
    let top_coverage = core_coverage(&candidates[0].content, &candidates[0].title, plan);
    if top_coverage >= required {
        return;
    }
    if let Some((idx, _)) = candidates
        .iter()
        .enumerate()
        .skip(1)
        .filter(|(_, c)| core_coverage(&c.content, &c.title, plan) >= required)
        .max_by(|(_, a), (_, b)| {
            a.local_score
                .partial_cmp(&b.local_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    {
        candidates.swap(0, idx);
    }
}

fn core_coverage(content: &str, title: &str, plan: &EvidenceQueryPlan) -> usize {
    let content = content.to_lowercase();
    let title = title.to_lowercase();
    plan.core_terms
        .iter()
        .filter(|term| {
            let term = term.to_lowercase();
            content.contains(&term) || title.contains(&term)
        })
        .count()
}

fn position_span(positions: &[usize]) -> Option<usize> {
    if positions.len() < 2 {
        return None;
    }
    let min = positions.iter().min()?;
    let max = positions.iter().max()?;
    Some(max.saturating_sub(*min))
}

fn view_type_score(view_type: &str) -> f64 {
    match view_type {
        "atomic" => 1.4,
        "window" => 1.0,
        "clean" => 0.8,
        "node" => 0.2,
        _ => 0.5,
    }
}

fn build_and_match_query(terms: &[String]) -> String {
    terms
        .iter()
        .take(5)
        .filter_map(|term| {
            let escaped = crate::nlp::chinese::escape_fts5_token(term);
            (!escaped.is_empty()).then(|| format!("{}*", escaped))
        })
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn is_domain_abbreviation(token: &str) -> bool {
    matches!(token, "lp" | "gp" | "vi" | "api" | "sdk")
}

fn is_weak_query_token(token: &str) -> bool {
    matches!(
        token,
        "请" | "告诉"
            | "告诉我"
            | "我"
            | "你"
            | "一下"
            | "相关"
            | "逻辑"
            | "相关逻辑"
            | "请问"
            | "什么"
            | "怎么"
            | "怎样"
            | "如何"
            | "是否"
            | "有关"
            | "内容"
            | "信息"
            | "的"
            | "是"
            | "和"
            | "与"
            | "及"
    )
}

const EVIDENCE_SNIPPET_CHARS: usize = 200;

/// Return evidence context near the text that caused the KB FTS hit.
///
/// KB nodes can hold an entire imported document. Returning the first characters
/// of such a node hides a match later in the document and makes a valid hit look
/// unrelated to callers.
fn query_centered_snippet(content: &str, query: &str) -> String {
    let content_len = content.chars().count();
    if content_len <= EVIDENCE_SNIPPET_CHARS {
        return content.to_string();
    }

    let folded_content = content.to_ascii_lowercase();
    let folded_query = query.trim().to_ascii_lowercase();
    let exact_match = (!folded_query.is_empty())
        .then(|| {
            folded_content
                .find(&folded_query)
                .map(|start| (start, folded_query.len()))
        })
        .flatten();

    let token_match = || {
        crate::nlp::chinese::tokenize_content(query)
            .split_whitespace()
            .filter_map(|term| {
                let term = term.to_ascii_lowercase();
                folded_content
                    .find(&term)
                    .map(|start| (start, term.len(), term.chars().count()))
            })
            .max_by_key(|(_, _, chars)| *chars)
            .map(|(start, len, _)| (start, len))
    };

    let Some((match_byte_start, match_byte_len)) = exact_match.or_else(token_match) else {
        return format!(
            "{}...",
            content
                .chars()
                .take(EVIDENCE_SNIPPET_CHARS)
                .collect::<String>()
        );
    };

    // to_ascii_lowercase preserves byte offsets, so these byte boundaries are
    // also valid in the original UTF-8 content.
    let match_char_start = content[..match_byte_start].chars().count();
    let match_char_len = content[match_byte_start..match_byte_start + match_byte_len]
        .chars()
        .count()
        .min(EVIDENCE_SNIPPET_CHARS);
    let before_match = (EVIDENCE_SNIPPET_CHARS - match_char_len) / 2;
    let mut start = match_char_start.saturating_sub(before_match);
    if start + EVIDENCE_SNIPPET_CHARS > content_len {
        start = content_len.saturating_sub(EVIDENCE_SNIPPET_CHARS);
    }
    let end = (start + EVIDENCE_SNIPPET_CHARS).min(content_len);
    let excerpt: String = content.chars().skip(start).take(end - start).collect();

    format!(
        "{}{}{}",
        if start > 0 { "..." } else { "" },
        excerpt,
        if end < content_len { "..." } else { "" }
    )
}

/// 查询时间线事件（§11.1 TimelineFirst 策略）
///
/// 从已接受或已应用的 timeline_event 候选中提取时间线事件，
/// 按事件日期降序排列，并关联 artifact 和 KB 文档信息。
///
/// 修复：之前只查 status='accepted'，但 apply_candidate 会把候选状态改成 'applied'，
/// 导致时间线候选刚应用到 gbrain 后反而从 TimelineFirst 结果里消失。
/// 现在查 status IN ('accepted', 'applied')：
/// - accepted: 已批准但尚未写入 gbrain
/// - applied: 已写入 gbrain，仍应出现在时间线查询中
///   修复：增加 filter_slug 参数，下推 slug 过滤到 SQL 查询阶段，
///   通过 JOIN artifact_projections 限定只返回目标 slug 的时间线事件，
///   避免先全局 LIMIT 再后置 retain 导致目标 slug 的命中被挤掉
fn query_timeline_events(
    conn: &Connection,
    query: &str,
    limit: i64,
    filter_slug: Option<&str>,
) -> Result<Vec<TimelineHit>> {
    // 修复：转义 LIKE 通配符 % 和 _，避免用户输入中的这些字符
    // 被解释为 SQL LIKE 通配符导致匹配意外结果
    let escaped_query = query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");

    // 修复：当提供 filter_slug 时，JOIN artifact_projections 匹配 brain_shadow_page 投影
    let sql = if filter_slug.is_some() {
        "SELECT pc.id, pc.proposed_payload, pc.artifact_id, pc.kb_document_id,
                d.original_name, d.title
         FROM promotion_candidates pc
         LEFT JOIN kb_documents d ON d.id = pc.kb_document_id
         JOIN artifact_projections ap ON ap.artifact_id = pc.artifact_id
              AND ap.projection_type = 'brain_shadow_page'
              AND ap.projection_ref = ?3
              AND ap.status = 'active'
         WHERE pc.candidate_type = 'timeline_event'
           AND pc.status IN ('accepted', 'applied')
           AND (pc.proposed_payload LIKE '%' || ?1 || '%' ESCAPE '\\'
                OR d.original_name LIKE '%' || ?1 || '%' ESCAPE '\\'
                OR d.title LIKE '%' || ?1 || '%' ESCAPE '\\')
         ORDER BY pc.created_at DESC
         LIMIT ?2"
    } else {
        "SELECT pc.id, pc.proposed_payload, pc.artifact_id, pc.kb_document_id,
                d.original_name, d.title
         FROM promotion_candidates pc
         LEFT JOIN kb_documents d ON d.id = pc.kb_document_id
         WHERE pc.candidate_type = 'timeline_event'
           AND pc.status IN ('accepted', 'applied')
           AND (pc.proposed_payload LIKE '%' || ?1 || '%' ESCAPE '\\'
                OR d.original_name LIKE '%' || ?1 || '%' ESCAPE '\\'
                OR d.title LIKE '%' || ?1 || '%' ESCAPE '\\')
         ORDER BY pc.created_at DESC
         LIMIT ?2"
    };

    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| GBrainError::Database(format!("准备时间线查询失败: {}", e)))?;

    // 修复：filter_slug 参数格式为 'slug:{slug}'，与 artifact_projections.projection_ref 匹配
    // 使用统一闭包避免 Rust 闭包类型不匹配问题
    #[allow(clippy::type_complexity)]
    let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<(i64, String, i64, Option<i64>, String, Option<String>)> {
        let candidate_id: i64 = row.get(0)?;
        let payload: String = row.get(1)?;
        let artifact_id: i64 = row.get(2)?;
        let kb_document_id: Option<i64> = row.get(3)?;
        let original_name: String = row.get(4)?;
        let doc_title: Option<String> = row.get(5)?;
        Ok((candidate_id, payload, artifact_id, kb_document_id, original_name, doc_title))
    };

    let rows = if let Some(slug) = filter_slug {
        let slug_ref = format!("slug:{}", slug);
        stmt.query_map(params![escaped_query, limit, slug_ref], map_row)
            .map_err(|e| GBrainError::Database(format!("时间线查询失败: {}", e)))?
    } else {
        stmt.query_map(params![escaped_query, limit], map_row)
            .map_err(|e| GBrainError::Database(format!("时间线查询失败: {}", e)))?
    };

    let mut hits = Vec::new();
    for row in rows {
        let (candidate_id, payload, artifact_id, kb_document_id, original_name, doc_title) =
            row.map_err(|e| GBrainError::Database(format!("映射时间线行失败: {}", e)))?;

        // 从 payload JSON 中提取 event_date 和 description
        let (event_date, description) = parse_timeline_payload(&payload);

        let source_title = doc_title.unwrap_or(original_name);

        hits.push(TimelineHit {
            candidate_id,
            event_date,
            description,
            artifact_id,
            kb_document_id,
            shadow_page_slug: None, // 后续填充
            source_title,
        });
    }

    Ok(hits)
}

/// 从 timeline_event 候选的 payload JSON 中提取 event_date 和 description
fn parse_timeline_payload(payload: &str) -> (String, String) {
    let mut event_date = String::new();
    let mut description = String::new();

    if let Ok(val) = serde_json::from_str::<serde_json::Value>(payload) {
        if let Some(date) = val.get("event_date").and_then(|v| v.as_str()) {
            event_date = date.to_string();
        } else if let Some(date) = val.get("date").and_then(|v| v.as_str()) {
            event_date = date.to_string();
        }
        if let Some(desc) = val.get("description").and_then(|v| v.as_str()) {
            description = desc.to_string();
        } else if let Some(desc) = val.get("text").and_then(|v| v.as_str()) {
            description = desc.to_string();
        } else if let Some(desc) = val.get("content").and_then(|v| v.as_str()) {
            description = desc.to_string();
        }
    }

    // 如果 JSON 解析失败，使用原始 payload 作为描述
    if description.is_empty() && event_date.is_empty() {
        // 修复：按字符截断而非字节，避免中文等多字节字符在 UTF-8 边界 panic
        description = if payload.chars().count() > 200 {
            format!("{}...", payload.chars().take(200).collect::<String>())
        } else {
            payload.to_string()
        };
    }

    (event_date, description)
}

/// 健康检查 — 检查 artifact 投影一致性
pub fn check_artifact_health(conn: &Connection) -> Result<ArtifactHealthReport> {
    let total_artifacts = store::count_total_artifacts(conn)
        .map_err(|e| GBrainError::Database(format!("统计 artifact 总数失败: {}", e)))?;
    let active_artifacts = store::count_active_artifacts(conn)
        .map_err(|e| GBrainError::Database(format!("统计活跃 artifact 失败: {}", e)))?;

    let orphan_projections = store::find_orphan_projections(conn)
        .map_err(|e| GBrainError::Database(format!("查找孤立投影失败: {}", e)))?
        .len() as i64;
    let stale_projections = store::count_stale_projections(conn)
        .map_err(|e| GBrainError::Database(format!("统计过期投影失败: {}", e)))?;
    let pending_candidates = crate::artifact::promotion::count_pending_candidates(conn)?;
    let active_provenance = provenance::count_active_provenance(conn)?;
    let stale_provenance = provenance::count_stale_provenance(conn)?;

    let mut issues = Vec::new();

    // 检查孤立投影
    if orphan_projections > 0 {
        issues.push(HealthIssue {
            severity: "warning".to_string(),
            issue_type: "orphan_projection".to_string(),
            description: format!(
                "{} 个投影的 artifact 已删除但投影仍标记为 active",
                orphan_projections
            ),
            suggestion: "运行 doctor --fix-artifacts 修复孤立投影".to_string(),
        });
    }

    // 检查过期投影
    if stale_projections > 0 {
        issues.push(HealthIssue {
            severity: "info".to_string(),
            issue_type: "stale_projection".to_string(),
            description: format!("{} 个投影已标记为 stale", stale_projections),
            suggestion: "检查 stale 投影是否需要重新处理".to_string(),
        });
    }

    // 检查待审核候选
    if pending_candidates > 100 {
        issues.push(HealthIssue {
            severity: "warning".to_string(),
            issue_type: "pending_candidates".to_string(),
            description: format!("{} 个候选变更待审核", pending_candidates),
            suggestion: "运行 artifact review list --status pending 查看并审核".to_string(),
        });
    }

    // 检查 artifact 文件完整性（分页全量检查）
    let page_size = 500;
    let mut offset = 0;
    loop {
        let artifacts = store::list_active_artifacts(conn, page_size, offset)
            .map_err(|e| GBrainError::Database(format!("列出 artifact 失败: {}", e)))?;
        if artifacts.is_empty() {
            break;
        }
        for artifact in &artifacts {
            let path = std::path::PathBuf::from(&artifact.storage_path);
            if !path.exists() {
                issues.push(HealthIssue {
                    severity: "error".to_string(),
                    issue_type: "missing_artifact_file".to_string(),
                    description: format!(
                        "Artifact {} 文件不存在: {}",
                        artifact.artifact_uid, artifact.storage_path
                    ),
                    suggestion: "检查 artifact store 目录是否完整".to_string(),
                });
            }
        }
        offset += page_size;
    }

    // 检查 KB 作业卡住（超过 24 小时仍在 queued/processing）
    let stale_jobs: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM kb_documents WHERE document_status IN ('queued', 'processing')
             AND updated_at < datetime('now', '-24 hours') AND deleted_at IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if stale_jobs > 0 {
        issues.push(HealthIssue {
            severity: "warning".to_string(),
            issue_type: "stale_kb_jobs".to_string(),
            description: format!("{} KB 文档处理作业超过 24 小时未完成", stale_jobs),
            suggestion: "检查 KB worker 是否正常运行".to_string(),
        });
    }

    let report = ArtifactHealthReport {
        total_artifacts,
        active_artifacts,
        orphan_projections,
        stale_projections,
        pending_candidates,
        active_provenance,
        stale_provenance,
        issues,
    };
    info!("check_artifact_health: total={}, active={}, orphans={}, stale_projections={}, pending={}, issues={}",
        report.total_artifacts, report.active_artifacts, report.orphan_projections,
        report.stale_projections, report.pending_candidates, report.issues.len());
    Ok(report)
}
