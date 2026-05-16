//! Artifact 应用服务 — 统一编排入口（设计文档 §3.2）
//!
//! 所有知识操作（写入、查询、来源管理、审核变更）通过此服务编排。
//! KB、gbrain、file attachment 等内部模块不直接对外暴露。

use crate::artifact::projection;
use crate::artifact::provenance;
use crate::artifact::query;
use crate::artifact::store;
use crate::artifact::types::{
    ArtifactHealthReport, ArtifactQueryInput, ArtifactQueryOutput,
    PromotionCandidate, ReviewCandidateInput, SourceArtifact, UnifiedQueryInput,
    UnifiedQueryResult, UploadSourceInput, UploadSourceOutput,
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
        Self { engine, config, ctx }
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

        if dry_run {
            let route_plan = crate::artifact::routing::infer_route_plan_from_artifact_intent(
                "md", "text/markdown", intent.unwrap_or("memory"),
            );
            return Ok(serde_json::json!({
                "dry_run": true,
                "slug": slug,
                "intent": intent.unwrap_or("memory"),
                "content_length": content.len(),
                "route_plan": {
                    "to_kb": route_plan.to_kb,
                    "to_brain": route_plan.to_brain,
                    "to_shadow": route_plan.to_shadow,
                    "to_file": route_plan.to_file,
                    "promotion": route_plan.promotion.to_string(),
                },
            }));
        }

        let artifact_dir = self.config.artifact_dir();
        std::fs::create_dir_all(&artifact_dir).map_err(|e| {
            GBrainError::FileError(format!("创建 artifact 目录失败: {}", e))
        })?;

        // 步骤 1: 在事务内创建 artifact/projection 记录
        let output = self.engine.transaction_with_engine(|engine| {
            let conn = engine.connection()?;
            crate::artifact::upload::put_manual_memory(
                conn,
                slug,
                content,
                title,
                intent,
                &artifact_dir,
                &self.engine.gbrain_dir(),
                self.config.default_kb_library_id,
                &self.config.upload_default_promotion_policy,
                self.config.artifact_manual_memory_to_kb,
            )
        })?;

        // 步骤 2: 调用原 gbrain put_page 写入页面内容（设计文档 §8.6 步骤 6）
        // 必须在事务外调用，因为 Operations::put_page 有自己的事务逻辑
        let page_title = title.unwrap_or(slug);
        let ops = crate::operations::Operations::with_config(
            self.engine,
            self.ctx.clone(),
            self.config.clone(),
        );
        ops.put_page(slug, page_title, content, None, None)?;

        Ok(serde_json::to_value(output).unwrap_or_else(|_| serde_json::json!({"status": "ok"})))
    }

    /// 上传文件作为知识源（设计文档 §4.1.1）
    /// 委托给现有 upload_source 逻辑
    pub fn upload_file(
        &self,
        input: UploadSourceInput,
    ) -> Result<UploadSourceOutput> {
        let artifact_dir = self.config.artifact_dir();
        std::fs::create_dir_all(&artifact_dir).map_err(|e| {
            GBrainError::FileError(format!("创建 artifact 目录失败: {}", e))
        })?;

        self.engine.transaction_with_engine(|engine| {
            let conn = engine.connection()?;
            crate::artifact::upload::upload_source(
                conn,
                &input,
                &artifact_dir,
                &self.engine.gbrain_dir(),
                self.config.default_kb_library_id,
                &self.config.upload_default_promotion_policy,
            )
        })
    }

    /// 统一知识查询（设计文档 §4.1.3）
    pub fn query(
        &self,
        input: UnifiedQueryInput,
    ) -> Result<UnifiedQueryResult> {
        let conn = self.engine.connection()?;
        query::unified_query(conn, &input, self.engine, self.config)
    }

    /// 统一查询 — 用户友好接口（设计文档 §7）
    /// 返回 ArtifactQueryOutput，隐藏内部 ID
    pub fn query_facade(
        &self,
        input: &ArtifactQueryInput,
    ) -> Result<ArtifactQueryOutput> {
        let start = std::time::Instant::now();

        // 将用户友好的 mode 映射到内部 QueryStrategy
        let strategy = match input.mode.as_deref().unwrap_or("auto") {
            "memory" | "auto" => crate::artifact::types::QueryStrategy::BrainFirst,
            "evidence" => crate::artifact::types::QueryStrategy::EvidenceFirst,
            "timeline" => crate::artifact::types::QueryStrategy::TimelineFirst,
            "graph" => crate::artifact::types::QueryStrategy::BrainFirst,
            _ => crate::artifact::types::QueryStrategy::BrainFirst,
        };

        let internal_input = UnifiedQueryInput {
            query: input.query.clone(),
            strategy,
            limit: input.limit.map(|l| l as i64),
            filter_slug: input.filter_slug.clone(),
            include_evidence: true,
            include_provenance: input.include_sources.unwrap_or(false),
        };

        let internal_result = self.query(internal_input)?;
        let elapsed = start.elapsed();

        // 转换为用户友好的输出格式
        Ok(ArtifactQueryOutput {
            query: input.query.clone(),
            mode: input.mode.clone().unwrap_or_else(|| "auto".to_string()),
            memories: internal_result.brain_hits.into_iter().map(|h| {
                crate::artifact::types::MemoryResult {
                    slug: h.slug,
                    title: h.title,
                    summary: h.snippet,
                    score: h.relevance,
                    source_artifact_id: None,
                }
            }).collect(),
            evidence: internal_result.evidence_hits.into_iter().map(|e| {
                crate::artifact::types::EvidenceResult {
                    title: e.title,
                    snippet: e.snippet,
                    score: e.relevance,
                    source_artifact_id: e.artifact.as_ref().map(|a| a.id),
                }
            }).collect(),
            timeline: internal_result.timeline_hits.into_iter().map(|t| {
                crate::artifact::types::TimelineEvent {
                    timestamp: t.event_date,
                    description: t.description,
                    slug: t.shadow_page_slug,
                }
            }).collect(),
            graph: Vec::new(), // 图谱关系暂不转换
            meta: crate::artifact::types::QueryMeta {
                total: internal_result.total_hits as usize,
                elapsed_ms: elapsed.as_millis() as u64,
                used_vector: false,
                used_keyword: true,
            },
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
    pub fn apply_suggested_change(
        &self,
        change_id: i64,
    ) -> Result<PromotionCandidate> {
        self.engine.transaction_with_engine(|engine| {
            let conn = engine.connection()?;
            crate::artifact::review::apply_suggested_change(conn, change_id)
        })
    }

    /// 拒绝建议变更
    pub fn reject_suggested_change(
        &self,
        input: ReviewCandidateInput,
    ) -> Result<PromotionCandidate> {
        self.engine.transaction_with_engine(|engine| {
            let conn = engine.connection()?;
            crate::artifact::review::reject_suggested_change(conn, &input)
        })
    }

    /// 回滚已应用的建议变更
    pub fn rollback_suggested_change(
        &self,
        change_id: i64,
    ) -> Result<PromotionCandidate> {
        self.engine.transaction_with_engine(|engine| {
            let conn = engine.connection()?;
            crate::artifact::review::rollback_suggested_change(conn, change_id)
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
        store::find_artifact_by_uid(conn, uid)
            .map_err(|e| GBrainError::Database(e.to_string()))
    }

    /// 列出 Artifacts
    pub fn list_artifacts(&self, limit: i64, offset: i64) -> Result<Vec<SourceArtifact>> {
        let conn = self.engine.connection()?;
        store::list_active_artifacts(conn, limit, offset)
            .map_err(|e| GBrainError::Database(e.to_string()))
    }

    /// 软删除 Artifact
    pub fn delete_artifact(&self, artifact_id: i64) -> Result<()> {
        self.engine.transaction_with_engine(|engine| {
            let conn = engine.connection()?;
            // 标记所有投影为 stale
            projection::mark_all_projections_stale(conn, artifact_id, "artifact_deleted")?;

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
            let occurrences = store::find_active_occurrences_by_artifact_and_slug(
                conn, artifact_id, from_slug,
            )?;

            if occurrences.is_empty() {
                return Err(GBrainError::InvalidInput(format!(
                    "未找到 artifact {} 与 slug '{}' 的关联",
                    artifact_id, from_slug
                )));
            }

            let mut detached_count = 0u64;
            for occ in &occurrences {
                // 将关联的 projection 标记为 stale
                projection::mark_projections_stale_by_occurrence(
                    conn, occ.id, "detached_by_user",
                )?;

                // 将 occurrence 标记为 stale
                store::stale_occurrence(conn, occ.id)?;

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
    pub fn restore(
        &self,
        id_or_uid: &str,
        dry_run: bool,
    ) -> Result<serde_json::Value> {
        let artifact_id = self.resolve_artifact_id(id_or_uid)?;

        if dry_run {
            return Ok(serde_json::json!({
                "dry_run": true,
                "artifact_id": artifact_id,
                "action": "restore",
                "description": format!("恢复 artifact {} 及其关联", artifact_id),
            }));
        }

        self.engine.transaction_with_engine(|engine| {
            let conn = engine.connection()?;

            // 检查 artifact 是否已删除
            let artifact = store::find_artifact_by_id(conn, artifact_id)?
                .ok_or_else(|| GBrainError::InvalidInput(format!(
                    "未找到 artifact {}", artifact_id
                )))?;

            if artifact.status != "deleted" {
                return Err(GBrainError::InvalidInput(format!(
                    "artifact {} 状态为 '{}'，无需恢复", artifact_id, artifact.status
                )));
            }

            // 恢复 artifact
            store::reactivate_artifact(conn, artifact_id)?;

            // 恢复关联的 occurrence
            let restored_occurrences = store::reactivate_occurrences_by_artifact(conn, artifact_id)?;

            // 恢复关联的 projection
            let restored_projections = projection::reactivate_projections_by_artifact(
                conn, artifact_id,
            )?;

            Ok(serde_json::json!({
                "artifact_id": artifact_id,
                "restored_occurrences": restored_occurrences,
                "restored_projections": restored_projections,
            }))
        })
    }

    /// 重新处理知识源（设计文档 §4.1.4）
    ///
    /// 将 artifact 的所有 projection 标记为 stale 并重新触发处理。
    pub fn reprocess(
        &self,
        id_or_uid: &str,
        dry_run: bool,
    ) -> Result<serde_json::Value> {
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
            let artifact = store::find_artifact_by_id(conn, artifact_id)?
                .ok_or_else(|| GBrainError::InvalidInput(format!(
                    "未找到 artifact {}", artifact_id
                )))?;

            if artifact.status != "active" {
                return Err(GBrainError::InvalidInput(format!(
                    "artifact {} 状态为 '{}'，无法重新处理", artifact_id, artifact.status
                )));
            }

            // 将所有活跃 projection 标记为 stale（触发重新处理）
            let stale_count = projection::mark_all_projections_stale(
                conn, artifact_id, "reprocess_requested",
            )?;

            // 创建 reprocess 作业（如果有作业队列）
            // 当前阶段：仅标记 stale，由 worker 下次循环时重新处理

            Ok(serde_json::json!({
                "artifact_id": artifact_id,
                "stale_projections": stale_count,
                "status": "reprocess_requested",
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
            .ok_or_else(|| GBrainError::InvalidInput(format!(
                "未找到 artifact '{}'", id_or_uid
            )))?;
        Ok(artifact.id)
    }
}
