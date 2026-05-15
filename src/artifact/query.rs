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
            brain_hits = query_brain(conn, &input.query, limit, engine, config)?;

            // 2. 如果 gbrain 结果不足，查 KB 补充
            if brain_hits.len() < limit as usize && input.include_evidence {
                evidence_hits =
                    query_kb_evidence(conn, &input.query, limit - brain_hits.len() as i64)?;
            }

            // 3. 给 brain hit 附加 provenance
            if input.include_provenance {
                for hit in &brain_hits {
                    let prov = provenance::find_provenance_by_brain_slug(conn, &hit.slug)?;
                    provenance_records.extend(prov);
                }
            }
        }
        QueryStrategy::EvidenceFirst => {
            // 1. 先查 KB
            evidence_hits = query_kb_evidence(conn, &input.query, limit)?;

            // 2. 给 KB hit 附加 artifact 和 shadow page 信息
            // 修复：query_kb_evidence 创建 EvidenceHit 时 projections 恒为空，
            // 从 hit.projections 找 kb_document_id 永远找不到。
            // 应直接用 hit.kb_document_id 查 artifact_projections
            for hit in &mut evidence_hits {
                if hit.kb_document_id > 0 {
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
                brain_hits = query_brain(conn, &input.query, limit / 2, engine, config)?;
            }

            // 4. 给 brain hit 附加 provenance
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
                let brain_hits_tmp = query_brain(conn, &input.query, 1, engine, config)?;
                if let Some(hit) = brain_hits_tmp.first() {
                    provenance_records =
                        provenance::find_provenance_by_brain_slug(conn, &hit.slug)?;
                }
            }
        }
        QueryStrategy::TimelineFirst => {
            // §11.2: "最近发生了什么 / 时间线" → 先查时间线事件，再查 gbrain 上下文
            // 1. 查询已接受的 timeline_event 候选
            timeline_hits = query_timeline_events(conn, &input.query, limit)?;

            // 2. 给时间线命中附加 shadow page 信息
            for hit in &mut timeline_hits {
                let shadow_slug = projection::find_shadow_page_slug(conn, hit.artifact_id)?;
                hit.shadow_page_slug = shadow_slug;
            }

            // 3. 查 gbrain 上下文补充
            if timeline_hits.len() < limit as usize {
                let remaining = limit - timeline_hits.len() as i64;
                brain_hits = query_brain(conn, &input.query, remaining, engine, config)?;
            }

            // 4. 给 brain hit 附加 provenance
            if input.include_provenance {
                for hit in &brain_hits {
                    let prov = provenance::find_provenance_by_brain_slug(conn, &hit.slug)?;
                    provenance_records.extend(prov);
                }
            }
        }
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
fn query_brain(
    _conn: &Connection,
    query: &str,
    limit: i64,
    engine: &crate::sqlite_engine::SqliteEngine,
    _config: &crate::config::Config,
) -> Result<Vec<BrainHit>> {
    // 使用现有 hybrid search
    let search_opts = crate::types::SearchOpts {
        limit: Some(limit as usize),
        ..Default::default()
    };
    let hybrid_opts = HybridOpts::default();

    let search_result = hybrid_search(engine, query, None, search_opts, hybrid_opts)
        .map_err(|e| GBrainError::Search(format!("gbrain 搜索失败: {}", e)))?;

    let mut hits = Vec::new();
    for r in &search_result.results {
        hits.push(BrainHit {
            slug: r.slug.clone(),
            title: r.title.clone(),
            snippet: r.chunk_text.clone(),
            relevance: r.score,
            provenance: Vec::new(), // 后续填充
        });
    }

    Ok(hits)
}

/// 查询 KB 证据
fn query_kb_evidence(conn: &Connection, query: &str, limit: i64) -> Result<Vec<EvidenceHit>> {
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
    // 修复：过滤已删除的文档，避免 artifact GC 后已删除文档内容仍被检索到
    let mut stmt = conn
        .prepare(
            "SELECT dn.id, dn.document_id, dn.content, dn.level,
                d.original_name, d.title, d.summary
         FROM kb_doc_fts fts
         JOIN kb_document_nodes dn ON dn.id = fts.rowid
         JOIN kb_documents d ON d.id = dn.document_id
         WHERE kb_doc_fts MATCH ?1
           AND d.document_status != 'deleted'
           AND d.deleted_at IS NULL
         ORDER BY rank
         LIMIT ?2",
        )
        .map_err(|e| GBrainError::Database(format!("准备 KB 搜索失败: {}", e)))?;

    let rows = stmt
        .query_map(params![fts_query, limit], |row| {
            let node_id: i64 = row.get(0)?;
            let kb_document_id: i64 = row.get(1)?;
            let content: String = row.get(2)?;
            let level: i64 = row.get(3)?;
            let original_name: String = row.get(4)?;
            let doc_title: String = row.get(5)?;
            let doc_summary: String = row.get(6)?;

            Ok((
                node_id,
                kb_document_id,
                content,
                level,
                original_name,
                doc_title,
                doc_summary,
            ))
        })
        .map_err(|e| GBrainError::Database(format!("KB 搜索失败: {}", e)))?;

    let mut hits = Vec::new();
    for row in rows {
        let (_node_id, kb_document_id, content, level, original_name, doc_title, _doc_summary) =
            row.map_err(|e| GBrainError::Database(format!("映射 KB 行失败: {}", e)))?;

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

        hits.push(EvidenceHit {
            kb_document_id,
            title,
            snippet,
            relevance: 1.0 / (level as f64 + 1.0), // 简化评分
            artifact: None,
            shadow_page_slug: None,
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
fn query_timeline_events(conn: &Connection, query: &str, limit: i64) -> Result<Vec<TimelineHit>> {
    // 查询已接受和已应用的 timeline_event 候选，按事件日期降序
    let mut stmt = conn
        .prepare(
            "SELECT pc.id, pc.proposed_payload, pc.artifact_id, pc.kb_document_id,
                    d.original_name, d.title
             FROM promotion_candidates pc
             LEFT JOIN kb_documents d ON d.id = pc.kb_document_id
             WHERE pc.candidate_type = 'timeline_event'
               AND pc.status IN ('accepted', 'applied')
               AND (pc.proposed_payload LIKE '%' || ?1 || '%'
                    OR d.original_name LIKE '%' || ?1 || '%'
                    OR d.title LIKE '%' || ?1 || '%')
             ORDER BY pc.created_at DESC
             LIMIT ?2",
        )
        .map_err(|e| GBrainError::Database(format!("准备时间线查询失败: {}", e)))?;

    let rows = stmt
        .query_map(params![query, limit], |row| {
            let candidate_id: i64 = row.get(0)?;
            let payload: String = row.get(1)?;
            let artifact_id: i64 = row.get(2)?;
            let kb_document_id: Option<i64> = row.get(3)?;
            let original_name: String = row.get(4)?;
            let doc_title: Option<String> = row.get(5)?;
            Ok((
                candidate_id,
                payload,
                artifact_id,
                kb_document_id,
                original_name,
                doc_title,
            ))
        })
        .map_err(|e| GBrainError::Database(format!("时间线查询失败: {}", e)))?;

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
