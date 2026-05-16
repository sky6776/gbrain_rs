//! 统一查询引擎 — BrainFirst / EvidenceFirst / Provenance / TimelineFirst 四种策略
//!
//! 负责：
//! 1. 根据查询意图自动选择策略
//! 2. BrainFirst: 先查 gbrain，再查 KB 补充
//! 3. EvidenceFirst: 先查 KB 证据，再查 gbrain 上下文
//! 4. Provenance: 仅追溯来源链
//! 5. TimelineFirst: 先查时间线事件，再查 gbrain 上下文（§11.1/§11.2）

use rusqlite::{params, Connection};
use tracing::info;

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
    match intent.intent {
        // "是谁/是什么" 类问题 → BrainFirst
        Intent::Entity => QueryStrategy::BrainFirst,
        // "时间线/最近发生了什么" 类问题 → TimelineFirst（§11.2）
        Intent::Temporal => QueryStrategy::TimelineFirst,
        // "事件" 类问题 → TimelineFirst（§11.2: 事件与时间线紧密关联）
        Intent::Event => QueryStrategy::TimelineFirst,
        // 默认 → BrainFirst + KB fallback
        Intent::General => QueryStrategy::BrainFirst,
    }
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
            evidence_hits =
                query_kb_evidence(conn, &input.query, limit, input.filter_slug.as_deref())?;

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

    let hits = search_result
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
) -> Result<Vec<EvidenceHit>> {
    // 使用 KB FTS5 搜索
    // kb_doc_fts 的 content_rowid 是 kb_document_nodes.id，
    // 而 document_id 列存储的是 kb_documents.id（父文档 ID）。
    // 因此 JOIN 应使用 FTS 的 rowid 与 kb_document_nodes.id 关联，
    // 而不是用 fts.document_id（那是 kb_documents.id）。
    // 修复：复用 build_fts_match_query 对自然语言分词+转义，
    // 避免原始 query 中的 ?、:、-、引号、中文等触发 FTS 语法错误或召回很差
    let fts_query = crate::nlp::chinese::build_fts_match_query(query);
    if fts_query.is_empty() {
        return Ok(Vec::new());
    }
    // 修复：当提供 filter_slug 时，通过 artifact_projections 双 JOIN 限定 slug，
    // 下推 slug 过滤到 SQL 查询阶段，避免先全局 LIMIT 再后置 retain 导致目标 slug 的命中被挤掉。
    // 第一个 JOIN：kb_document 投影 → 获取 artifact_id
    // 第二个 JOIN：brain_shadow_page 投影 → 匹配 projection_ref = 'slug:{filter_slug}'
    // 修复：带出 ap_kb.artifact_id，用于后续填充 EvidenceHit.artifact，
    // 避免 filter_slug 命中时 shadow_page_slug 已非空而跳过 artifact 补全分支
    let sql = if filter_slug.is_some() {
        "SELECT dn.id, dn.document_id, dn.content, dn.level,
            d.original_name, d.title, d.summary,
            ap_kb.artifact_id
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
         ORDER BY rank
         LIMIT ?2"
    } else {
        "SELECT dn.id, dn.document_id, dn.content, dn.level,
            d.original_name, d.title, d.summary,
            0 AS artifact_id
         FROM kb_doc_fts fts
         JOIN kb_document_nodes dn ON dn.id = fts.rowid
         JOIN kb_documents d ON d.id = dn.document_id
         WHERE kb_doc_fts MATCH ?1
           AND d.document_status != 'deleted'
           AND d.deleted_at IS NULL
         ORDER BY rank
         LIMIT ?2"
    };

    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| GBrainError::Database(format!("准备 KB 搜索失败: {}", e)))?;

    // 修复：filter_slug 参数格式为 'slug:{slug}'，与 artifact_projections.projection_ref 匹配
    // 使用统一闭包避免 Rust 闭包类型不匹配问题
    // 修复：增加 artifact_id 字段，用于填充 EvidenceHit.artifact
    #[allow(clippy::type_complexity)]
    let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<(i64, i64, String, i64, String, String, String, i64)> {
        let node_id: i64 = row.get(0)?;
        let kb_document_id: i64 = row.get(1)?;
        let content: String = row.get(2)?;
        let level: i64 = row.get(3)?;
        let original_name: String = row.get(4)?;
        let doc_title: String = row.get(5)?;
        let doc_summary: String = row.get(6)?;
        let artifact_id: i64 = row.get(7)?;
        Ok((node_id, kb_document_id, content, level, original_name, doc_title, doc_summary, artifact_id))
    };

    let rows = if let Some(slug) = filter_slug {
        let slug_ref = format!("slug:{}", slug);
        stmt.query_map(params![fts_query, limit, slug_ref], map_row)
            .map_err(|e| GBrainError::Database(format!("KB 搜索失败: {}", e)))?
    } else {
        stmt.query_map(params![fts_query, limit], map_row)
            .map_err(|e| GBrainError::Database(format!("KB 搜索失败: {}", e)))?
    };

    let mut hits = Vec::new();
    for row in rows {
        let (
            _node_id,
            kb_document_id,
            content,
            level,
            original_name,
            doc_title,
            _doc_summary,
            artifact_id,
        ) = row.map_err(|e| GBrainError::Database(format!("映射 KB 行失败: {}", e)))?;

        let title = if doc_title.is_empty() {
            original_name
        } else {
            doc_title
        };
        // 修复：按字符截断而非字节，避免中文等多字节字符在 UTF-8 边界 panic
        let snippet = if content.chars().count() > 200 {
            format!("{}...", content.chars().take(200).collect::<String>())
        } else {
            content.clone()
        };

        // 修复：当 filter_slug 存在时，SQL JOIN 已带出 artifact_id，
        // 直接查 artifact 填充，避免 shadow_page_slug 已非空时跳过补全分支
        let artifact = if artifact_id > 0 {
            super::store::find_artifact_by_id(conn, artifact_id)
                .ok()
                .flatten()
        } else {
            None
        };

        hits.push(EvidenceHit {
            kb_document_id,
            title,
            snippet,
            relevance: 1.0 / (level as f64 + 1.0), // 简化评分
            artifact,
            // 修复：当 filter_slug 已在 SQL 中通过 JOIN 过滤时，
            // 这些 hit 已确认属于该 slug，直接填入 shadow_page_slug。
            // 否则兜底 retain 要求 shadow_page_slug == filter_slug 会把所有 hit 清空。
            shadow_page_slug: filter_slug.map(|s| s.to_string()),
            projections: Vec::new(),
        });
    }

    Ok(hits)
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
    let total_artifacts = store::count_active_artifacts(conn)
        .map_err(|e| GBrainError::Database(format!("统计 artifact 失败: {}", e)))?;
    let active_artifacts = total_artifacts;

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
            suggestion: "运行 promotion list --status pending 查看并审核".to_string(),
        });
    }

    // 检查 artifact 文件完整性
    let artifacts = store::list_active_artifacts(conn, 100, 0)
        .map_err(|e| GBrainError::Database(format!("列出 artifact 失败: {}", e)))?;
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

    Ok(ArtifactHealthReport {
        total_artifacts,
        active_artifacts,
        orphan_projections,
        stale_projections,
        pending_candidates,
        active_provenance,
        stale_provenance,
        issues,
    })
}
