//! Artifact 应用服务 — 统一编排入口（设计文档 §3.2）
//!
//! 所有知识操作（写入、查询、来源管理、审核变更）通过此服务编排。
//! KB、gbrain、file attachment 等内部模块不直接对外暴露。

use crate::artifact::projection;
use crate::artifact::provenance;
use crate::artifact::query;
use crate::artifact::store;
use crate::artifact::types::{
    ArtifactHealthReport, ArtifactListItem, ArtifactQueryInput, ArtifactQueryOutput, ArtifactReviewActionOutput,
    DeleteImpactPreview, ReviewCandidateInput, SourceArtifact,
    UnifiedQueryInput, UnifiedQueryResult, UploadSourceInput, UploadSourceOutput,
};
use crate::config::Config;
use crate::error::{GBrainError, Result};
use crate::operations::OpContext;
use crate::sqlite_engine::SqliteEngine;

/// Artifact 应用服务 — 统一知识操作编排入口
pub struct ArtifactService<'a> {
    engine: &'a SqliteEngine,
    config: &'a Config,
    ctx: OpContext,
}

impl<'a> ArtifactService<'a> {
    pub fn new(engine: &'a SqliteEngine, ctx: OpContext, config: &'a Config) -> Self {
        Self {
            engine,
            config,
            ctx,
        }
    }

    /// 手动写入长期记忆（设计文档 §4.1.2）
    ///
    /// 创建 text/manual artifact 并投影到 gbrain 页面。
    /// 步骤：
    /// 1. 创建 artifact store 记录和 projection（在事务内）
    /// 2. 调用原 gbrain put_page 写入页面内容（设计文档 §8.6 步骤 6）
    pub fn put_memory(
        &self,
        slug: &str,
        content: &str,
        title: Option<&str>,
        intent: Option<&str>,
        dry_run: bool,
        force: bool,
    ) -> Result<serde_json::Value> {
        // 安全校验：内容长度限制
        let max_content_size = 1024 * 1024; // 1MB
        if content.len() > max_content_size {
            return Err(GBrainError::InvalidInput(format!(
                "内容长度 {} 超过上限 {} 字节",
                content.len(),
                max_content_size
            )));
        }

        // 安全校验：slug 格式
        crate::security::validate_page_slug(slug)?;

        // 解析意图：用户指定 > 配置默认 > "memory"
        let resolved_intent = intent.unwrap_or(&self.config.artifact_default_intent);

        // 根据意图推断路由计划（manual=true 表示手动 put）
        let route_plan = crate::artifact::routing::infer_route_plan_from_artifact_intent(
            "md", "text/markdown", resolved_intent, true,
        );

        // P1-1 修复：幂等/冲突检测必须在 dry_run 判断之前完成，
        // 但所有写入操作必须在 dry_run 判断之后、且在同一个事务内执行。
        // 之前的代码在 dry_run 判断前就执行了 touch_artifact 和 mark_projection_stale，
        // 导致 dry-run 有副作用。
        // 现在改为：先计算 resolution（只读），dry_run 立即返回，
        // 非 dry_run 时在事务内完成所有写入。
        let page_title = title.unwrap_or(slug);
        let md_content = if content.starts_with("---\n") || content.starts_with("---\r\n") {
            content.to_string()
        } else {
            format!("# {}\n\n{}", page_title, content)
        };
        let content_sha256 = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(md_content.as_bytes());
            format!("{:x}", hasher.finalize())
        };

        // 只读阶段：查询现有 artifact，计算 resolution
        let conn = self.engine.connection()?;
        let existing = store::find_artifact_by_slug(conn, slug)
            .map_err(|e| GBrainError::Database(e.to_string()))?;

        // P1 修复：人工修改冲突检测（设计文档 §5.6）
        // 当 route_plan.to_brain=true 且页面已存在时，
        // 读取当前页面的 content_hash，与上次 artifact 写入时记录的
        // brain_page_update 投影的 version_hash 比较。
        // 如果不同，说明页面被人工修改过，不应无提示覆盖。
        // force=true 时跳过冲突检测，允许强制覆盖。
        let page_conflict_detected = if route_plan.to_brain && !force {
            // 查询当前页面的 content_hash
            let page_hash: Option<String> = conn
                .query_row(
                    "SELECT content_hash FROM pages WHERE slug = ?1 AND deleted_at IS NULL",
                    rusqlite::params![slug],
                    |row| row.get(0),
                )
                .ok()
                .flatten();

            // 查询上次 artifact 写入时记录的 page hash（存储在 brain_page_update 投影的 version_hash）
            // version_hash 存储的是上次写入时的 page content_hash（非 artifact sha256），
            // 如果 version_hash 为空，说明是旧数据（修复前），无法判断冲突。
            let last_artifact_page_hash: Option<String> = existing
                .as_ref()
                .and_then(|a| {
                    let projections = store::find_projections_by_artifact(conn, a.id).ok()?;
                    projections
                        .into_iter()
                        .filter(|p| p.status == "active" && p.projection_type == "brain_page_update")
                        .filter_map(|p| {
                            if p.version_hash.is_empty() {
                                None
                            } else {
                                Some(p.version_hash)
                            }
                        })
                        .next()
                });

            match (page_hash, last_artifact_page_hash) {
                (Some(current), Some(last)) => current != last,
                // 页面存在但无上次记录 → 无法判断是否被人工修改，不冲突
                (Some(_), None) => false,
                // 页面不存在 → 新建，不冲突
                (None, _) => false,
            }
        } else {
            false
        };

        let resolution = match &existing {
            Some(a) if a.sha256 == content_sha256 && a.status == "active" => "no_op",
            Some(a) if a.status == "active" && page_conflict_detected => "conflict",
            Some(a) if a.status == "active" => "update",
            _ => "create",
        };

        // P1-1 修复：dry_run 在任何写入之前返回，零副作用
        if dry_run {
            return Ok(serde_json::json!({
                "dry_run": true,
                "slug": slug,
                "intent": resolved_intent,
                "content_length": content.len(),
                "resolution": resolution,
                "route_plan": {
                    "to_kb": route_plan.to_kb,
                    "to_brain": route_plan.to_brain,
                    "to_shadow": route_plan.to_shadow,
                    "to_file": route_plan.to_file,
                    "promotion": route_plan.promotion.to_string(),
                },
            }));
        }

        // 非 dry_run：所有写入在同一个事务内完成
        // 幂等 no-op：touch last_seen_at
        // 更新：标记旧 brain_page_update 为 stale，然后继续创建新 artifact
        // 创建：正常创建新 artifact
        let artifact_dir = self.config.artifact_dir();
        std::fs::create_dir_all(&artifact_dir)
            .map_err(|e| GBrainError::FileError(format!("创建 artifact 目录失败: {}", e)))?;

        // 修复：将 put_page 和 artifact/projection 写入放进同一事务，
        // 避免 put_page 成功但后续 artifact/projection 失败时留下半写入状态。
        // 使用 put_page_in_transaction 确保 page 和 artifact/projection
        // 要么全部成功要么全部回滚，保持"Artifact 统一入口"的一致性。
        let output = self.engine.transaction_with_engine(|engine| {
            let conn = engine.connection()?;

            // 在事务内重新查询，确保数据一致性
            let existing_in_txn = store::find_artifact_by_slug(conn, slug)
                .map_err(|e| GBrainError::Database(e.to_string()))?;

            // P1-1 修复 + P1 冲突检测：在事务内完成幂等/更新/冲突/创建写入
            match existing_in_txn {
                Some(ref a) if a.sha256 == content_sha256 && a.status == "active" => {
                    // 相同内容 → 幂等 no-op：touch last_seen_at，返回现有信息
                    store::touch_artifact(conn, a.id)
                        .map_err(|e| GBrainError::Database(e.to_string()))?;
                    return Ok(serde_json::json!({
                        "resolution": "no_op",
                        "artifact_id": a.id,
                        "artifact_uid": a.artifact_uid,
                        "slug": slug,
                        "detail": "内容完全相同，幂等返回已有 artifact",
                    }));
                }
                Some(ref a) if a.status == "active" && page_conflict_detected => {
                    // P1 修复：人工修改冲突 — 页面被人工修改过，不应无提示覆盖。
                    // 设计文档 §5.6 要求：默认生成 review change，而不是直接覆盖。
                    // 当前实现：返回冲突信息，不覆盖页面，由调用方决定如何处理
                    // （可通过 --force 参数强制覆盖，或通过 artifact_review_* 创建 suggested change）。
                    let current_page_hash: Option<String> = conn
                        .query_row(
                            "SELECT content_hash FROM pages WHERE slug = ?1 AND deleted_at IS NULL",
                            rusqlite::params![slug],
                            |row| row.get(0),
                        )
                        .ok()
                        .flatten();
                    return Ok(serde_json::json!({
                        "resolution": "conflict",
                        "artifact_id": a.id,
                        "artifact_uid": a.artifact_uid,
                        "slug": slug,
                        "detail": "页面已被人工修改，无法无提示覆盖。请使用 --force 强制覆盖，或通过 artifact_review_* 创建 suggested change。",
                        "current_page_hash": current_page_hash,
                        "last_artifact_hash": existing.as_ref().and_then(|a| {
                            let projections = store::find_projections_by_artifact(conn, a.id).ok()?;
                            projections.into_iter()
                                .filter(|p| p.status == "active" && p.projection_type == "brain_page_update")
                                .filter_map(|p| if p.version_hash.is_empty() { None } else { Some(p.version_hash) })
                                .next()
                        }),
                    }));
                }
                Some(ref a) if a.status == "active" => {
                    // 不同内容 → 标记旧投影为 superseded/stale，继续创建新 artifact
                    // P1 修复：同 slug 不同内容更新时，不仅 stale brain_page_update，
                    // 也 stale brain_shadow_page，避免新 artifact 的 active shadow projection
                    // 指向旧 page 内容（设计文档 §5.6 版本策略）
                    let old_projections = store::find_projections_by_artifact(conn, a.id)
                        .map_err(|e| GBrainError::Database(e.to_string()))?;
                    for p in old_projections {
                        if p.status == "active"
                            && (p.projection_type == "brain_page_update"
                                || p.projection_type == "brain_shadow_page")
                        {
                            store::mark_projection_stale(conn, p.id, "content_updated")
                                .map_err(|e| GBrainError::Database(e.to_string()))?;
                        }
                    }
                }
                _ => {} // 无已有 artifact 或已删除 → 正常创建
            }

            let manual_output = crate::artifact::upload::put_manual_memory(
                conn,
                slug,
                content,
                title,
                Some(resolved_intent),
                &artifact_dir,
                &self.engine.gbrain_dir(),
                self.config.default_kb_library_id,
                &self.config.upload_default_promotion_policy,
                self.config.artifact_manual_memory_to_kb,
                self.config.artifact_auto_create_inbox_library,
                &route_plan,
            )?;

            // P1-2 修复：只在 route_plan.to_brain=true 时写入 gbrain page，
            // intent=evidence 不应写 gbrain page
            if route_plan.to_brain {
                let page_title = title.unwrap_or(slug);
                let ops = crate::operations::Operations::with_config_in_transaction(
                    engine,
                    self.ctx.clone(),
                    self.config.clone(),
                );
                let page = ops.put_page(slug, page_title, content, None, None)?;

                // P1 修复：写入页面后，将页面的 content_hash 存储到 brain_page_update 投影的
                // version_hash 中，以便下次 artifact_put 时能检测页面是否被人工修改。
                // 之前 version_hash 存的是 artifact 的 sha256，无法与 page content_hash 比较。
                // 现在改为存储 page 的 content_hash，使冲突检测能精确判断：
                // 如果当前 page_hash != 上次写入时的 page_hash → 页面被人工修改过。
                if let Some(ref page_hash) = page.content_hash {
                    // 查找刚创建的 brain_page_update 投影并更新 version_hash
                    let new_projections = store::find_projections_by_artifact(conn, manual_output.artifact_id)
                        .map_err(|e| GBrainError::Database(e.to_string()))?;
                    for p in new_projections {
                        if p.status == "active" && p.projection_type == "brain_page_update" {
                            store::update_projection_version_hash(conn, p.id, page_hash)
                                .map_err(|e| GBrainError::Database(e.to_string()))?;
                        }
                    }
                }
            }

            Ok(serde_json::to_value(manual_output)
                .unwrap_or_else(|_| serde_json::json!({"status": "ok"})))
        })?;

        Ok(serde_json::to_value(output).unwrap_or_else(|_| serde_json::json!({"status": "ok"})))
    }

    /// 上传文件作为知识源（设计文档 §4.1.1）
    /// 委托给现有 upload_source 逻辑
    pub fn upload_file(&self, input: UploadSourceInput) -> Result<UploadSourceOutput> {
        let artifact_dir = self.config.artifact_dir();
        std::fs::create_dir_all(&artifact_dir)
            .map_err(|e| GBrainError::FileError(format!("创建 artifact 目录失败: {}", e)))?;

        self.engine.transaction_with_engine(|engine| {
            let conn = engine.connection()?;
            crate::artifact::upload::upload_source(
                conn,
                &input,
                &artifact_dir,
                &self.engine.gbrain_dir(),
                self.config.default_kb_library_id,
                &self.config.upload_default_promotion_policy,
                self.config.artifact_auto_create_inbox_library,
            )
        })
    }

    /// 统一知识查询（设计文档 §4.1.3）
    pub fn query(&self, input: UnifiedQueryInput) -> Result<UnifiedQueryResult> {
        let conn = self.engine.connection()?;
        query::unified_query(conn, &input, self.engine, self.config)
    }

    /// 统一查询 — 用户友好接口（设计文档 §7）
    /// 返回 ArtifactQueryOutput，隐藏内部 ID
    pub fn query_facade(&self, input: &ArtifactQueryInput) -> Result<ArtifactQueryOutput> {
        let start = std::time::Instant::now();

        // 将用户友好的 mode 映射到内部 QueryStrategy
        let strategy = match input.mode.as_deref().unwrap_or("auto") {
            "memory" | "auto" => crate::artifact::types::QueryStrategy::BrainFirst,
            "evidence" => crate::artifact::types::QueryStrategy::EvidenceFirst,
            "timeline" => crate::artifact::types::QueryStrategy::TimelineFirst,
            "graph" => crate::artifact::types::QueryStrategy::BrainFirst,
            _ => crate::artifact::types::QueryStrategy::BrainFirst,
        };

        let include_sources = input.include_sources.unwrap_or(false);

        let internal_input = UnifiedQueryInput {
            query: input.query.clone(),
            strategy,
            limit: input.limit.map(|l| l as i64),
            filter_slug: input.filter_slug.clone(),
            include_evidence: true,
            include_provenance: include_sources,
        };

        let internal_result = self.query(internal_input)?;
        let elapsed = start.elapsed();

        // 将 provenance_records 转换为用户友好的 SourceRef 列表
        let conn = self.engine.connection()?;
        let all_sources: Vec<crate::artifact::types::SourceRef> = if include_sources {
            internal_result
                .provenance_records
                .iter()
                .filter(|p| p.status == "active")
                .map(|p| {
                    let artifact_info = p
                        .artifact_id
                        .and_then(|aid| store::find_artifact_by_id(conn, aid).ok().flatten());
                    crate::artifact::types::SourceRef {
                        artifact_uid: artifact_info
                            .as_ref()
                            .map(|a| a.artifact_uid.clone())
                            .unwrap_or_default(),
                        original_name: artifact_info
                            .as_ref()
                            .map(|a| Some(a.original_name.clone()))
                            .unwrap_or(None),
                        quote_text: Some(p.quote_text.clone()),
                        confidence: p.confidence,
                        brain_slug: Some(p.brain_slug.clone()),
                        brain_field: Some(p.brain_field.clone()),
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        // P2-2: used_vector 反映实际搜索策略
        let used_vector = !internal_result.brain_hits.is_empty()
            || !internal_result.evidence_hits.is_empty();

        // 转换为用户友好的输出格式
        Ok(ArtifactQueryOutput {
            query: input.query.clone(),
            mode: input.mode.clone().unwrap_or_else(|| "auto".to_string()),
            memories: internal_result
                .brain_hits
                .into_iter()
                .map(|h| {
                    let hit_sources: Vec<crate::artifact::types::SourceRef> = all_sources
                        .iter()
                        .filter(|s| s.brain_slug.as_deref() == Some(&h.slug))
                        .cloned()
                        .collect();
                    crate::artifact::types::MemoryResult {
                        slug: h.slug,
                        title: h.title,
                        summary: h.snippet,
                        score: h.relevance,
                        sources: hit_sources,
                    }
                })
                .collect(),
            evidence: internal_result
                .evidence_hits
                .into_iter()
                .map(|e| {
                    // P2-4 修复：evidence 的 fallback source 也受 include_sources 控制，
                    // 当 include_sources=false 时不应返回任何 source 信息
                    let hit_sources: Vec<crate::artifact::types::SourceRef> = if include_sources {
                        e
                            .artifact
                            .as_ref()
                            .map(|a| {
                                let from_provenance: Vec<crate::artifact::types::SourceRef> = all_sources
                                    .iter()
                                    .filter(|s| s.artifact_uid == a.artifact_uid)
                                    .cloned()
                                    .collect();
                                // 如果 provenance 已有此 artifact 的来源，直接用
                                if !from_provenance.is_empty() {
                                    from_provenance
                                } else {
                                    // 否则从 artifact 直接构造基础 SourceRef
                                    vec![crate::artifact::types::SourceRef {
                                        artifact_uid: a.artifact_uid.clone(),
                                        original_name: Some(a.original_name.clone()),
                                        quote_text: None,
                                        confidence: 0.5,
                                        brain_slug: None,
                                        brain_field: None,
                                    }]
                                }
                            })
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };
                    crate::artifact::types::EvidenceResult {
                        title: e.title,
                        snippet: e.snippet,
                        score: e.relevance,
                        sources: hit_sources,
                    }
                })
                .collect(),
            timeline: internal_result
                .timeline_hits
                .into_iter()
                .map(|t| crate::artifact::types::TimelineEvent {
                    timestamp: t.event_date,
                    description: t.description,
                    slug: t.shadow_page_slug,
                })
                .collect(),
            graph: Vec::new(),
            meta: crate::artifact::types::QueryMeta {
                total: internal_result.total_hits as usize,
                elapsed_ms: elapsed.as_millis() as u64,
                used_vector,
                used_keyword: true,
            },
            sources: all_sources,
        })
    }

    /// 列出建议变更（设计文档 §4.1.5）
    pub fn list_suggested_changes(
        &self,
        status: Option<&str>,
        target_slug: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<crate::artifact::types::ArtifactReviewItem>> {
        let conn = self.engine.connection()?;
        crate::artifact::review::list_suggested_changes(conn, status, target_slug, limit, offset)
    }

    /// 获取建议变更详情
    pub fn get_suggested_change(
        &self,
        change_id: i64,
    ) -> Result<Option<crate::artifact::types::ArtifactReviewItem>> {
        let conn = self.engine.connection()?;
        crate::artifact::review::get_suggested_change(conn, change_id)
    }

    /// 应用建议变更
    pub fn apply_suggested_change(&self, change_id: i64) -> Result<ArtifactReviewActionOutput> {
        self.engine.transaction_with_engine(|engine| {
            let conn = engine.connection()?;
            let c = crate::artifact::review::apply_suggested_change(conn, change_id)?;
            Ok(candidate_to_review_action_output(c, "applied"))
        })
    }

    /// 拒绝建议变更
    pub fn reject_suggested_change(
        &self,
        input: ReviewCandidateInput,
    ) -> Result<ArtifactReviewActionOutput> {
        self.engine.transaction_with_engine(|engine| {
            let conn = engine.connection()?;
            let c = crate::artifact::review::reject_suggested_change(conn, &input)?;
            Ok(candidate_to_review_action_output(c, "rejected"))
        })
    }

    /// 回滚已应用的建议变更
    pub fn rollback_suggested_change(&self, change_id: i64) -> Result<ArtifactReviewActionOutput> {
        self.engine.transaction_with_engine(|engine| {
            let conn = engine.connection()?;
            let c = crate::artifact::review::rollback_suggested_change(conn, change_id)?;
            Ok(candidate_to_review_action_output(c, "rolled_back"))
        })
    }

    /// 健康检查
    pub fn health_check(&self) -> Result<ArtifactHealthReport> {
        let conn = self.engine.connection()?;
        query::check_artifact_health(conn)
    }

    /// 获取 Artifact 详情
    pub fn get_artifact(&self, artifact_id: i64) -> Result<Option<SourceArtifact>> {
        let conn = self.engine.connection()?;
        store::find_artifact_by_id(conn, artifact_id)
            .map_err(|e| GBrainError::Database(e.to_string()))
    }

    /// 获取 Artifact 详情（按 UID）
    pub fn get_artifact_by_uid(&self, uid: &str) -> Result<Option<SourceArtifact>> {
        let conn = self.engine.connection()?;
        store::find_artifact_by_uid(conn, uid).map_err(|e| GBrainError::Database(e.to_string()))
    }

    /// 获取 Artifact 详情（用户友好 DTO）
    ///
    /// 按 artifact_id 查询 provenance（不是 canonical_slug），
    /// 包含 occurrences，隐藏内部字段。
    pub fn get_artifact_detail(
        &self,
        id_or_uid: &str,
        include_projections: bool,
        include_sources: bool,
    ) -> Result<Option<crate::artifact::types::ArtifactDetailOutput>> {
        let artifact_id = self.resolve_artifact_id(id_or_uid)?;
        let conn = self.engine.connection()?;
        let artifact = store::find_artifact_by_id(conn, artifact_id)
            .map_err(|e| GBrainError::Database(e.to_string()))?;

        match artifact {
            Some(a) => {
                let projections = if include_projections {
                    let projs = store::find_projections_by_artifact(conn, artifact_id)
                        .map_err(|e| GBrainError::Database(e.to_string()))?;
                    Some(
                        projs
                            .into_iter()
                            .map(|p| crate::artifact::types::ArtifactProjectionSummary {
                                projection_type: p.projection_type,
                                projection_key: p.projection_key,
                                projection_ref: Some(p.projection_ref),
                                status: p.status,
                            })
                            .collect(),
                    )
                } else {
                    None
                };

                let sources = if include_sources {
                    let provs = provenance::find_provenance_by_artifact(conn, artifact_id)?;
                    Some(
                        provs
                            .into_iter()
                            .filter(|p| p.status == "active")
                            .map(|p| crate::artifact::types::SourceRef {
                                artifact_uid: a.artifact_uid.clone(),
                                original_name: Some(a.original_name.clone()),
                                quote_text: Some(p.quote_text),
                                confidence: p.confidence,
                                brain_slug: Some(p.brain_slug),
                                brain_field: Some(p.brain_field),
                            })
                            .collect(),
                    )
                } else {
                    None
                };

                let occurrences = Some(
                    store::find_occurrences_by_artifact(conn, artifact_id)
                        .map_err(|e| GBrainError::Database(e.to_string()))?
                        .into_iter()
                        .map(|o| crate::artifact::types::ArtifactOccurrenceSummary {
                            uid: o.occurrence_uid,
                            intent: Some(o.intent),
                            target_slug: Some(o.target_slug),
                            status: o.status,
                            created_at: o.created_at,
                        })
                        .collect(),
                );

                Ok(Some(crate::artifact::types::ArtifactDetailOutput {
                    uid: a.artifact_uid,
                    slug: a.canonical_slug,
                    original_name: Some(a.original_name),
                    mime_type: Some(a.mime_type),
                    size_bytes: Some(a.size_bytes),
                    created_at: a.created_at,
                    updated_at: a.updated_at,
                    status: a.status,
                    extension: Some(a.extension),
                    projections,
                    sources,
                    occurrences,
                }))
            }
            None => Ok(None),
        }
    }

    /// 列出 Artifacts
    /// P2-3 修复：返回外部 DTO ArtifactListItem，隐藏内部 id/storage_path/raw metadata
    pub fn list_artifacts(&self, limit: i64, offset: i64) -> Result<Vec<ArtifactListItem>> {
        let conn = self.engine.connection()?;
        store::list_active_artifacts(conn, limit, offset)
            .map_err(|e| GBrainError::Database(e.to_string()))
            .map(|artifacts| {
                artifacts.into_iter().map(|a| ArtifactListItem {
                    uid: a.artifact_uid,
                    slug: a.canonical_slug,
                    original_name: Some(a.original_name),
                    size_bytes: Some(a.size_bytes),
                    status: a.status,
                    updated_at: a.updated_at,
                }).collect()
            })
    }

    /// 软删除 Artifact
    pub fn delete_artifact(&self, artifact_id: i64) -> Result<()> {
        self.engine.transaction_with_engine(|engine| {
            let conn = engine.connection()?;
            // 标记所有投影为 stale
            projection::mark_all_projections_stale(conn, artifact_id, "artifact_deleted")?;

            // 软删除关联的 occurrences
            store::soft_delete_occurrences_by_artifact(conn, artifact_id)
                .map_err(|e| GBrainError::Database(e.to_string()))?;

            // 软删除关联的 kb_documents
            let kb_doc_ids = projection::find_all_kb_document_ids(conn, artifact_id)?;
            for kb_doc_id in kb_doc_ids {
                crate::kb::lifecycle::soft_delete_document(conn, kb_doc_id)?;
                provenance::mark_provenance_stale_by_kb_document(
                    conn,
                    kb_doc_id,
                    "kb_document_deleted",
                )?;
            }

            // 软删除 artifact
            store::soft_delete_artifact(conn, artifact_id)?;
            Ok(())
        })
    }

    /// 删除 dry-run — 返回影响预览
    pub fn delete_artifact_dry_run(&self, id_or_uid: &str) -> Result<DeleteImpactPreview> {
        let artifact_id = self.resolve_artifact_id(id_or_uid)?;
        let conn = self.engine.connection()?;
        let artifact = store::find_artifact_by_id(conn, artifact_id)
            .map_err(|e| GBrainError::Database(e.to_string()))?
            .ok_or_else(|| GBrainError::InvalidInput(format!("未找到 artifact {}", artifact_id)))?;
        let projection_count = store::find_projections_by_artifact(conn, artifact_id)
            .map_err(|e| GBrainError::Database(e.to_string()))?
            .len() as i64;
        let occurrence_count = store::find_occurrences_by_artifact(conn, artifact_id)
            .map_err(|e| GBrainError::Database(e.to_string()))?
            .len() as i64;
        let kb_document_count = projection::find_all_kb_document_ids(conn, artifact_id)?
            .len() as i64;
        let provenance_count = provenance::find_provenance_by_artifact(conn, artifact_id)?
            .len() as i64;
        Ok(DeleteImpactPreview {
            artifact_id,
            artifact_uid: artifact.artifact_uid,
            artifact_status: artifact.status,
            projection_count,
            occurrence_count,
            kb_document_count,
            provenance_count,
        })
    }

    /// 获取 Provenance（按 brain slug）
    pub fn get_provenance(
        &self,
        brain_slug: &str,
    ) -> Result<Vec<crate::artifact::types::ProvenanceRecord>> {
        let conn = self.engine.connection()?;
        provenance::find_provenance_by_brain_slug(conn, brain_slug)
    }

    /// 移除知识源与某次使用的关联（设计文档 §4.1.4）
    ///
    /// 只 stale 目标页面相关 occurrence/projection，不删除 source artifact。
    /// 与 delete 的区别：detach 只解除特定 occurrence 的关联，artifact 本身保留。
    pub fn detach(
        &self,
        id_or_uid: &str,
        from_slug: &str,
        dry_run: bool,
    ) -> Result<serde_json::Value> {
        // 解析 artifact ID 或 UID
        let artifact_id = self.resolve_artifact_id(id_or_uid)?;

        if dry_run {
            return Ok(serde_json::json!({
                "dry_run": true,
                "artifact_id": artifact_id,
                "from_slug": from_slug,
                "action": "detach",
                "description": format!("将 artifact {} 与 slug '{}' 的关联标记为 stale", artifact_id, from_slug),
            }));
        }

        self.engine.transaction_with_engine(|engine| {
            let conn = engine.connection()?;

            // 查找匹配的 occurrence（artifact_id + target_slug）
            let occurrences =
                store::find_active_occurrences_by_artifact_and_slug(conn, artifact_id, from_slug)?;

            if occurrences.is_empty() {
                return Err(GBrainError::InvalidInput(format!(
                    "未找到 artifact {} 与 slug '{}' 的关联",
                    artifact_id, from_slug
                )));
            }

            let mut detached_count = 0u64;
            for occ in &occurrences {
                // 将关联的 projection 标记为 stale
                projection::mark_projections_stale_by_occurrence(conn, occ.id, "detached_by_user")?;

                // P1-3 修复：将 occurrence 标记为 stale，写 stale_reason='detached_by_user'
                // 确保 restore 不会误恢复 detach 的 occurrence
                store::stale_occurrence(conn, occ.id, "detached_by_user")?;

                detached_count += 1;
            }

            Ok(serde_json::json!({
                "artifact_id": artifact_id,
                "from_slug": from_slug,
                "detached_occurrences": detached_count,
            }))
        })
    }

    /// 恢复已软删除的知识源（设计文档 §4.1.4）
    ///
    /// 重新激活 artifact 及其关联的 occurrence/projection。
    /// 同时恢复关联的 kb_documents（清除 deleted_at、重新入队处理）
    /// 和因 kb_document_deleted 而变 stale 的 provenance。
    pub fn restore(&self, id_or_uid: &str, dry_run: bool) -> Result<serde_json::Value> {
        let artifact_id = self.resolve_artifact_id(id_or_uid)?;

        if dry_run {
            return Ok(serde_json::json!({
                "dry_run": true,
                "artifact_id": artifact_id,
                "action": "restore",
                "description": format!("恢复 artifact {} 及其关联（含 KB 文档和 provenance）", artifact_id),
            }));
        }

        self.engine.transaction_with_engine(|engine| {
            let conn = engine.connection()?;

            // 检查 artifact 是否已删除
            let artifact = store::find_artifact_by_id(conn, artifact_id)?.ok_or_else(|| {
                GBrainError::InvalidInput(format!("未找到 artifact {}", artifact_id))
            })?;

            if artifact.status != "deleted" {
                return Err(GBrainError::InvalidInput(format!(
                    "artifact {} 状态为 '{}'，无需恢复",
                    artifact_id, artifact.status
                )));
            }

            // 恢复 artifact
            store::reactivate_artifact(conn, artifact_id)?;

            // 恢复关联的 occurrence
            let restored_occurrences =
                store::reactivate_occurrences_by_artifact(conn, artifact_id)?;

            // 恢复关联的 kb_documents（清除 deleted_at，重新入队处理）
            // 必须在 reactivate_projections_by_artifact 之前收集 KB doc IDs，
            // 因为 reactivate 会把 status='stale' AND stale_reason='artifact_deleted' 改成 active，
            // 之后 find_kb_document_ids_for_restore 就找不到匹配的 projection 了。
            let kb_doc_ids = projection::find_kb_document_ids_for_restore(conn, artifact_id)?;

            // 恢复关联的 projection
            let restored_projections =
                projection::reactivate_projections_by_artifact(conn, artifact_id)?;

            let mut restored_kb_documents = 0u64;
            let mut restored_provenance = 0u64;
            for kb_doc_id in kb_doc_ids {
                crate::kb::lifecycle::restore_document(conn, kb_doc_id)?;
                restored_kb_documents += 1;
                // 恢复因 kb_document_deleted 而变 stale 的 provenance
                restored_provenance +=
                    provenance::reactivate_provenance_by_kb_document(conn, kb_doc_id)?;
            }

            Ok(serde_json::json!({
                "artifact_id": artifact_id,
                "restored_occurrences": restored_occurrences,
                "restored_projections": restored_projections,
                "restored_kb_documents": restored_kb_documents,
                "restored_provenance": restored_provenance,
            }))
        })
    }

    /// 重新处理知识源（设计文档 §4.1.4）
    ///
    /// 将 artifact 的所有 projection 标记为 stale，然后根据旧投影类型
    /// 重新创建同类型投影（KB/brain/shadow/file）。
    ///
    /// 修复：之前用通用 intent 推断路由，manual memory 的 intent="memory"
    /// 映射到 to_brain=false, to_shadow=true，与原始写入
    /// brain_page_update(to_brain=true, to_shadow=false) 不一致。
    /// 现在基于旧投影类型重建，保证 reprocess 后投影类型与原始一致。
    pub fn reprocess(&self, id_or_uid: &str, dry_run: bool) -> Result<serde_json::Value> {
        let artifact_id = self.resolve_artifact_id(id_or_uid)?;

        if dry_run {
            return Ok(serde_json::json!({
                "dry_run": true,
                "artifact_id": artifact_id,
                "action": "reprocess",
                "description": format!("重新处理 artifact {} 的所有投影", artifact_id),
            }));
        }

        self.engine.transaction_with_engine(|engine| {
            let conn = engine.connection()?;

            // 检查 artifact 存在且活跃
            let artifact = store::find_artifact_by_id(conn, artifact_id)?.ok_or_else(|| {
                GBrainError::InvalidInput(format!("未找到 artifact {}", artifact_id))
            })?;

            if artifact.status != "active" {
                return Err(GBrainError::InvalidInput(format!(
                    "artifact {} 状态为 '{}'，无法重新处理",
                    artifact_id, artifact.status
                )));
            }

            // 步骤 1: 收集旧投影（在标 stale 之前），按 occurrence 分组
            let old_projections = store::find_projections_by_artifact(conn, artifact_id)
                .map_err(|e| GBrainError::Database(e.to_string()))?;
            // 按 (occurrence_id, projection_type) 分组，只保留 active 的
            let mut old_by_occ: std::collections::HashMap<(Option<i64>, String), Vec<&crate::artifact::types::ArtifactProjection>> =
                std::collections::HashMap::new();
            for p in &old_projections {
                if p.status == "active" {
                    old_by_occ
                        .entry((p.occurrence_id, p.projection_type.clone()))
                        .or_default()
                        .push(p);
                }
            }

            // 步骤 2: 将所有活跃 projection 标记为 stale
            let stale_count =
                projection::mark_all_projections_stale(conn, artifact_id, "reprocess_requested")?;

            // 步骤 3: 按旧 active 投影逐条精确重建，直接复用 projection_key/projection_ref
            //
            // 修复 P1: KB 投影从旧 projection 的 projection_key=library:{id} 解析 library_id，
            // 不再依赖 occ.library_id.or(config.default_kb_library_id)（默认安装下两者均为 None）。
            // 修复 P2: brain_page_update / brain_shadow_page 直接复用旧 key/ref，
            // 不再用 page_slug/target_slug 重新构造（上传路径用 slug:{target_slug}，与 page_slug 不同）。
            // 修复 P3: file_attachment 只遍历 old_by_occ 中已过滤为 active 的投影，
            // 不再遍历 old_projections 全量（会复活历史 stale/superseded 投影）。
            let mut rebuilt_projections = Vec::new();

            for ((occ_id_opt, proj_type), projs) in &old_by_occ {
                let occ_id = match occ_id_opt {
                    Some(id) => *id,
                    None => continue,
                };

                // 确认对应 occurrence 仍为 active
                if let Some(occ) = store::find_occurrence_by_id(conn, occ_id)? {
                    if occ.status != "active" {
                        continue;
                    }
                } else {
                    continue;
                }

                for old_proj in projs {
                    match proj_type.as_str() {
                        "kb_document" => {
                            // 从旧 projection_key=library:{id} 解析 library_id
                            if let Some(id_str) = old_proj.projection_key.strip_prefix("library:") {
                                if let Ok(library_id) = id_str.parse::<i64>() {
                                    let proj_ref =
                                        crate::artifact::projection::create_kb_projection(
                                            conn,
                                            artifact_id,
                                            occ_id,
                                            library_id,
                                        )?;
                                    rebuilt_projections.push(serde_json::json!({
                                        "type": "kb_document",
                                        "occurrence_id": occ_id,
                                        "projection_ref": proj_ref,
                                    }));
                                }
                            }
                        }
                        "brain_page_update" | "brain_shadow_page" | "file_attachment" => {
                            // 直接复用旧投影的 projection_key 和 projection_ref
                            let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
                            let proj = crate::artifact::types::ArtifactProjection {
                                id: 0,
                                created_at: now.clone(),
                                updated_at: now,
                                artifact_id,
                                occurrence_id: Some(occ_id),
                                projection_type: proj_type.clone(),
                                projection_key: old_proj.projection_key.clone(),
                                projection_ref: old_proj.projection_ref.clone(),
                                status: "active".to_string(),
                                version_hash: if proj_type == "brain_page_update" {
                                    artifact.sha256.clone()
                                } else {
                                    String::new()
                                },
                                stale_reason: String::new(),
                                metadata_json: "{}".to_string(),
                                superseded_by: None,
                            };
                            store::insert_projection(conn, &proj)?;
                            rebuilt_projections.push(serde_json::json!({
                                "type": proj_type,
                                "occurrence_id": occ_id,
                                "projection_ref": old_proj.projection_ref,
                            }));
                        }
                        _ => {}
                    }
                }
            }

            Ok(serde_json::json!({
                "artifact_id": artifact_id,
                "stale_projections": stale_count,
                "rebuilt_projections": rebuilt_projections,
                "status": "reprocessed",
            }))
        })
    }

    /// 解析 artifact ID 或 UID
    ///
    /// 支持数字 ID 或字符串 UID 两种格式
    pub fn resolve_artifact_id(&self, id_or_uid: &str) -> Result<i64> {
        if let Ok(id) = id_or_uid.parse::<i64>() {
            return Ok(id);
        }
        // 按 UID 查找
        let conn = self.engine.connection()?;
        let artifact = store::find_artifact_by_uid(conn, id_or_uid)?
            .ok_or_else(|| GBrainError::InvalidInput(format!("未找到 artifact '{}'", id_or_uid)))?;
        Ok(artifact.id)
    }
}

/// 将 PromotionCandidate 转换为用户友好的 ArtifactReviewActionOutput
fn candidate_to_review_action_output(
    c: crate::artifact::types::PromotionCandidate,
    action: &str,
) -> crate::artifact::types::ArtifactReviewActionOutput {
    let evidence = if c.evidence_json.is_empty() {
        None
    } else {
        serde_json::from_str::<serde_json::Value>(&c.evidence_json).ok()
    };
    let action_description = format!(
        "{} 变更 #{} ({}) 到页面 {}",
        action, c.id, c.candidate_type, c.target_slug
    );
    crate::artifact::types::ArtifactReviewActionOutput {
        change_id: c.id,
        target_slug: c.target_slug,
        candidate_type: c.candidate_type,
        status: c.status,
        action_description,
        evidence,
        risk_level: Some(c.risk_level),
    }
}
