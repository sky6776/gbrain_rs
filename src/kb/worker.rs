//! KB 作业 worker — 后台轮询并处理 KB 文档作业
//!
//! 轮询 `claim_next_kb_job`，调用 `process_document_async`，
//! 然后标记作业完成或失败。可通过 CLI 子命令或 MCP 内置后台线程运行。
//! 支持多 worker 并发处理 (P5-023)。

use crate::config::Config;
use crate::embedding::Embedder;
use crate::engine::BrainEngine;
use crate::error::{GBrainError, Result};
use crate::kb::engine::KbEngine;
use crate::kb::jobs::{claim_next_kb_job, complete_kb_job, fail_kb_job};
use crate::kb::pipeline::process_document_async;
use crate::kb::raptor::RaptorConfig;
use crate::sqlite_engine::SqliteEngine;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// 运行一次 KB 作业处理循环：认领一个待处理作业并执行。
/// 返回是否处理了一个作业。
pub fn run_kb_worker_once(engine: &SqliteEngine, config: &Config) -> Result<bool> {
    let conn = engine.connection()?;

    // 认领下一个待处理作业
    let claimed = claim_next_kb_job(conn)?;
    let Some((job_db_id, payload)) = claimed else {
        debug!("KB worker: 无待处理作业");
        return Ok(false);
    };

    info!(
        job_db_id,
        document_id = payload.document_id,
        "KB worker: 认领作业"
    );

    // 创建 embedder（如果已配置 API key）
    let embedder: Option<Arc<Embedder>> = config.openai_api_key.as_deref().map(|api_key| {
        Arc::new(Embedder::new(
            api_key,
            config.openai_base_url.as_deref(),
            Some(&config.embedding_model),
            Some(config.embedding_dimensions),
        ))
    });

    // 构建 RaptorConfig（使用默认值）
    let raptor_config = RaptorConfig::default();

    // 创建 tokio 运行时执行异步管道
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| GBrainError::InvalidInput(format!("创建异步运行时失败: {}", e)))?;

    // 执行文档处理管道
    let result = rt.block_on(process_document_async(
        conn,
        &payload,
        embedder,
        Some(&raptor_config),
        config.kb_raptor_secret_ref.as_deref(),
        config.kb_raptor_base_url.as_deref(),
        if config.kb_raptor_model.is_empty() {
            None
        } else {
            Some(config.kb_raptor_model.as_str())
        },
        None,
    ));

    match result {
        Ok(process_result) => {
            info!(
                job_db_id,
                document_id = payload.document_id,
                word_total = process_result.word_total,
                split_total = process_result.split_total,
                "KB worker: 文档处理完成"
            );

            if let Err(e) = update_shadow_pages_after_kb_success(
                conn,
                payload.document_id,
                &payload.processing_run_id,
            ) {
                warn!(
                    document_id = payload.document_id,
                    error = %e,
                    "KB worker: 更新 artifact shadow page 状态失败"
                );
            }

            // 修复：先入队 promotion 再 complete job，避免 enqueue 失败时
            // job 已完成不会重试，导致 artifact_promote_extract 永久丢失。
            // 之前先 complete_kb_job 再 enqueue，一旦 INSERT/COMMIT/查询出现
            // 瞬时 DB 错误，KB job 已完成且不会重试，promotion 入队永久丢失。
            // 现在先尝试入队，入队成功后再 complete job；
            // 入队失败时 job 保持可认领状态，下次轮询可重试。
            // 注意：enqueue_artifact_promote_if_linked 内部已用
            // BEGIN IMMEDIATE 事务保护 run_id 校验和 INSERT，不存在竞态窗口。
            if let Err(e) = enqueue_artifact_promote_if_linked(
                conn,
                payload.document_id,
                &payload.processing_run_id,
            ) {
                warn!(
                    document_id = payload.document_id,
                    error = %e,
                    "KB worker: 触发 artifact promotion 失败，job 保持可重试状态"
                );
                // 入队失败时用 fail_kb_job 标记失败（而非 complete），
                // 让 job 可被人工重试或后续恢复逻辑处理，
                // 避免无限循环重试同一 job
                fail_kb_job(conn, job_db_id, &format!("promotion 入队失败: {}", e))?;
                return Ok(true);
            }

            complete_kb_job(conn, job_db_id)?;

            Ok(true)
        }
        Err(e) => {
            warn!(
                job_db_id,
                document_id = payload.document_id,
                error = %e,
                "KB worker: 文档处理失败"
            );
            // 修复：直接用 update_document_status_with_run_guard 做条件更新，
            // 消除 ensure_document_run_current + update_document_status 两步间的竞态。
            // 旧代码先检查 run_id 再无守卫更新，两步之间新 run 可能更新 processing_run_id，
            // 导致旧 job 把新 run 文档标为 failed。
            // 带守卫的 UPDATE 在 SQL 层面原子检查 processing_run_id，不存在竞态窗口。
            let kb = KbEngine::new(conn);
            let _ = kb.update_document_status_with_run_guard(
                payload.document_id,
                Some(crate::kb::types::STATUS_FAILED),
                None,
                Some(&e.to_string()),
                None,
                None,
                None,
                Some(&payload.processing_run_id),
            );
            if let Err(update_error) = update_shadow_pages_after_kb_failure(
                conn,
                payload.document_id,
                &payload.processing_run_id,
                &e.to_string(),
            ) {
                warn!(
                    document_id = payload.document_id,
                    error = %update_error,
                    "KB worker: 更新失败状态到 artifact shadow page 失败"
                );
            }
            fail_kb_job(conn, job_db_id, &e.to_string())?;
            Ok(true)
        }
    }
}

fn update_shadow_pages_after_kb_success(
    conn: &Connection,
    document_id: i64,
    run_id: &str,
) -> Result<usize> {
    with_current_document_run(conn, document_id, run_id, || {
        let (embedding_status, embedding_error, word_total, split_total): (i32, String, i64, i64) =
            conn.query_row(
                "SELECT embedding_status, embedding_error, word_total, split_total
             FROM kb_documents WHERE id = ?1",
                rusqlite::params![document_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .map_err(|e| GBrainError::Database(format!("查询 KB 文档状态失败: {}", e)))?;

        let summary = if embedding_status == crate::kb::types::STATUS_FAILED {
            format!(
                "KB text extracted and keyword indexed, but embedding failed: {}",
                truncate_shadow_status_detail(&embedding_error)
            )
        } else {
            format!(
                "KB processing completed. Text extracted and indexed. Chunks: {}. Words: {}.",
                split_total, word_total
            )
        };

        update_shadow_pages_for_kb_document(conn, document_id, &summary)
    })
}

fn update_shadow_pages_after_kb_failure(
    conn: &Connection,
    document_id: i64,
    run_id: &str,
    error: &str,
) -> Result<usize> {
    let summary = format!(
        "KB processing failed: {}",
        truncate_shadow_status_detail(error)
    );
    with_current_document_run(conn, document_id, run_id, || {
        update_shadow_pages_for_kb_document(conn, document_id, &summary)
    })
}

fn with_current_document_run<F>(
    conn: &Connection,
    document_id: i64,
    run_id: &str,
    update: F,
) -> Result<usize>
where
    F: FnOnce() -> Result<usize>,
{
    conn.execute("BEGIN IMMEDIATE", [])
        .map_err(|e| GBrainError::Database(format!("开启 shadow page 状态回写事务失败: {}", e)))?;

    let result = (|| -> Result<usize> {
        if !document_run_is_current(conn, document_id, run_id)? {
            debug!(
                document_id,
                expected_run_id = run_id,
                "KB worker: shadow page 状态回写时 run_id 已过期，跳过"
            );
            return Ok(0);
        }

        update()
    })();

    match result {
        Ok(updated) => {
            conn.execute("COMMIT", []).map_err(|e| {
                let _ = conn.execute("ROLLBACK", []);
                GBrainError::Database(format!("提交 shadow page 状态回写失败: {}", e))
            })?;
            Ok(updated)
        }
        Err(e) => {
            let _ = conn.execute("ROLLBACK", []);
            Err(e)
        }
    }
}

fn document_run_is_current(conn: &Connection, document_id: i64, run_id: &str) -> Result<bool> {
    let current_run_id: Option<String> = match conn.query_row(
        "SELECT processing_run_id FROM kb_documents WHERE id = ?1",
        rusqlite::params![document_id],
        |row| row.get::<_, String>(0),
    ) {
        Ok(current_run_id) => Some(current_run_id),
        Err(rusqlite::Error::QueryReturnedNoRows) => None,
        Err(e) => {
            return Err(GBrainError::Database(format!(
                "查询 KB 文档 run_id 失败: {}",
                e
            )))
        }
    };

    Ok(current_run_id.as_deref() == Some(run_id))
}

fn update_shadow_pages_for_kb_document(
    conn: &Connection,
    document_id: i64,
    summary: &str,
) -> Result<usize> {
    let projection_ref = format!("kb_document:{}", document_id);
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT sp.projection_ref
             FROM artifact_projections kb
             JOIN artifact_projections sp
               ON sp.artifact_id = kb.artifact_id
              AND COALESCE(sp.occurrence_id, -1) = COALESCE(kb.occurrence_id, -1)
             WHERE kb.projection_type = 'kb_document'
               AND kb.projection_ref = ?1
               AND kb.status = 'active'
               AND sp.projection_type = 'brain_shadow_page'
               AND sp.status = 'active'",
        )
        .map_err(|e| GBrainError::Database(format!("查询 shadow page 投影失败: {}", e)))?;

    let rows = stmt
        .query_map(rusqlite::params![projection_ref], |row| {
            row.get::<_, String>(0)
        })
        .map_err(|e| GBrainError::Database(format!("遍历 shadow page 投影失败: {}", e)))?;

    let mut updated = 0usize;
    for row in rows {
        let projection_ref = row.map_err(|e| GBrainError::Database(e.to_string()))?;
        let slug = projection_ref
            .strip_prefix("slug:")
            .unwrap_or(&projection_ref)
            .to_string();
        if update_shadow_page_summary(conn, &slug, summary)? {
            updated += 1;
        }
    }

    Ok(updated)
}

fn update_shadow_page_summary(conn: &Connection, slug: &str, summary: &str) -> Result<bool> {
    let page: Option<(i64, String, String)> = match conn.query_row(
        "SELECT id, title, compiled_truth FROM pages
         WHERE slug = ?1 AND deleted_at IS NULL",
        rusqlite::params![slug],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ) {
        Ok(page) => Some(page),
        Err(rusqlite::Error::QueryReturnedNoRows) => None,
        Err(e) => {
            return Err(GBrainError::Database(format!(
                "查询 shadow page 失败: {}",
                e
            )))
        }
    };

    let Some((page_id, title, body)) = page else {
        return Ok(false);
    };

    let updated_body = set_shadow_summary(&body, summary);
    if updated_body == body {
        return Ok(false);
    }

    let truth_tokens = crate::nlp::chinese::tokenize_content(&updated_body);
    let content_hash = page_content_hash(&title, &updated_body);
    conn.execute(
        "UPDATE pages
         SET compiled_truth = ?1,
             compiled_truth_tokens = ?2,
             content_hash = ?3,
             updated_at = datetime('now')
         WHERE id = ?4",
        rusqlite::params![updated_body, truth_tokens, content_hash, page_id],
    )
    .map_err(|e| GBrainError::Database(format!("更新 shadow page 状态失败: {}", e)))?;

    rebuild_shadow_page_chunks(conn, page_id, &updated_body)?;
    Ok(true)
}

fn set_shadow_summary(body: &str, summary: &str) -> String {
    const HEADING: &str = "## Summary";
    let summary = summary.trim();
    let Some(start) = body.find(HEADING) else {
        return format!("{}\n\n{}\n\n{}", body.trim_end(), HEADING, summary);
    };

    let content_start = start + HEADING.len();
    let suffix = body[content_start..]
        .find("\n\n## ")
        .map(|offset| &body[content_start + offset..])
        .unwrap_or("");
    format!("{}\n\n{}{}", &body[..content_start], summary, suffix)
}

fn rebuild_shadow_page_chunks(conn: &Connection, page_id: i64, body: &str) -> Result<()> {
    let _ = conn.execute(
        "DELETE FROM vec_chunks
         WHERE chunk_id IN (SELECT id FROM chunks WHERE page_id = ?1)",
        rusqlite::params![page_id],
    );
    let _ = conn.execute(
        "DELETE FROM chunk_embeddings
         WHERE chunk_id IN (SELECT id FROM chunks WHERE page_id = ?1)",
        rusqlite::params![page_id],
    );
    conn.execute(
        "DELETE FROM chunks WHERE page_id = ?1",
        rusqlite::params![page_id],
    )
    .map_err(|e| GBrainError::Database(format!("删除 shadow page 旧 chunk 失败: {}", e)))?;

    let chunks =
        crate::chunker::chunk_text(body, None, None, crate::types::ChunkSource::CompiledTruth);
    for chunk in &chunks {
        let chunk_text_tokens = crate::nlp::chinese::tokenize_content(&chunk.chunk_text);
        conn.execute(
            "INSERT INTO chunks
                (page_id, chunk_index, chunk_text, chunk_text_tokens, token_count, chunk_source, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'body', datetime('now'))",
            rusqlite::params![
                page_id,
                chunk.chunk_index,
                chunk.chunk_text,
                chunk_text_tokens,
                chunk.token_count
            ],
        )
        .map_err(|e| GBrainError::Database(format!("创建 shadow page chunk 失败: {}", e)))?;
    }

    Ok(())
}

fn page_content_hash(title: &str, body: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(title.as_bytes());
    hasher.update(body.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn truncate_shadow_status_detail(detail: &str) -> String {
    let mut truncated: String = detail.chars().take(500).collect();
    if detail.chars().count() > 500 {
        truncated.push_str("...");
    }
    truncated
}

/// 运行一次 re-embed 作业处理循环：认领一个 kb_reembed 或 kb_reembed_node 作业并处理。
/// 返回是否处理了一个作业。
pub fn run_reembed_worker_once(engine: &SqliteEngine, config: &Config) -> Result<bool> {
    let conn = engine.connection()?;
    let claimed = crate::kb::jobs::claim_next_reembed_job(conn)?;
    let Some((job_db_id, job_type, payload)) = claimed else {
        return Ok(false);
    };

    let embedder: Option<Arc<Embedder>> = config.openai_api_key.as_deref().map(|api_key| {
        Arc::new(Embedder::new(
            api_key,
            config.openai_base_url.as_deref(),
            Some(&config.embedding_model),
            Some(config.embedding_dimensions),
        ))
    });

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| GBrainError::InvalidInput(format!("创建异步运行时失败: {}", e)))?;

    let result = match job_type.as_str() {
        "kb_reembed_node" => {
            // 单节点修复：读取 node_id，嵌入内容，写入 kb_node_embeddings
            let node_id: i64 = payload.get("node_id").and_then(|v| v.as_i64()).unwrap_or(0);
            if node_id == 0 {
                fail_kb_job(
                    conn,
                    job_db_id,
                    "missing node_id in kb_reembed_node payload",
                )?;
                return Ok(true);
            }
            reembed_single_node(conn, &embedder, &rt, node_id, None)
        }
        "kb_reembed" => {
            // 文档级重嵌入：读取所有无 embedding 的节点，逐个嵌入
            let doc_id: i64 = payload
                .get("document_id")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let target_index_id: i64 = payload
                .get("target_embedding_index_id")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            reembed_document_nodes(conn, &embedder, &rt, doc_id, target_index_id)
        }
        _ => Err(GBrainError::InvalidInput(format!(
            "未知 re-embed 作业类型: {}",
            job_type
        ))),
    };

    match result {
        Ok(count) => {
            info!(job_db_id, node_count = count, "re-embed worker: 处理完成");
            complete_kb_job(conn, job_db_id)?;
            Ok(true)
        }
        Err(e) => {
            warn!(job_db_id, error = %e, "re-embed worker: 处理失败");
            fail_kb_job(conn, job_db_id, &e.to_string())?;
            Ok(true)
        }
    }
}

/// 运行一次 artifact 作业处理循环（设计文档 §8.8）：
/// 认领 `artifact_promote_extract` 作业并执行 promotion extraction。
/// 返回是否处理了一个作业。
///
/// 旧名 `run_artifact_worker_once` 已重命名，
/// 对外不再暴露 "promote" 命名细节。
pub fn run_artifact_worker_once(engine: &SqliteEngine, _config: &Config) -> Result<bool> {
    let conn = engine.connection()?;
    let queue = crate::jobs::JobQueue::new(conn);

    // 认领下一个 artifact_promote_extract 作业
    let job = queue
        .dequeue_by_type("artifact_promote_extract")
        .map_err(|e| GBrainError::Database(format!("认领 artifact_promote_extract 失败: {}", e)))?;

    let Some(job) = job else {
        return Ok(false);
    };

    let artifact_id: i64 = job
        .payload
        .get("artifact_id")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let kb_document_id: i64 = job
        .payload
        .get("kb_document_id")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    // 修复：优先使用 payload 中的稳定值，避免旧 job 被归属到新 occurrence 后串策略
    // payload 中的 occurrence_id 和 promotion_policy 是入队时绑定的，不会被后续上传覆盖
    let payload_occurrence_id: i64 = job
        .payload
        .get("occurrence_id")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let payload_promotion_policy: Option<String> = job
        .payload
        .get("promotion_policy")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    info!(
        job_id = job.id,
        artifact_id, kb_document_id, payload_occurrence_id, "artifact promote worker: 认领作业"
    );

    if artifact_id == 0 || kb_document_id == 0 {
        fail_kb_job(conn, job.id, "artifact_id 或 kb_document_id 无效")?;
        return Ok(true);
    }

    // 修复：优先使用 payload 中的 occurrence_id（入队时绑定的稳定值），
    // 仅在 payload 未携带时才从 projection 反查（兼容旧 job）
    let occurrence_id = if payload_occurrence_id > 0 {
        // 校验 projection 仍匹配 payload 中的值
        let proj_ref = format!("kb_document:{}", kb_document_id);
        let current_occ: Option<i64> = conn
            .query_row(
                "SELECT occurrence_id FROM artifact_projections
                 WHERE artifact_id = ?1 AND projection_type = 'kb_document' AND projection_ref = ?2 AND status = 'active'
                 LIMIT 1",
                rusqlite::params![artifact_id, proj_ref],
                |row| row.get::<_, Option<i64>>(0),
            )
            .ok()
            .flatten();
        if current_occ != Some(payload_occurrence_id) {
            warn!(
                artifact_id,
                payload_occurrence_id,
                current_occ,
                "artifact promote worker: projection occurrence_id 已变更，使用 payload 中的稳定值"
            );
        }
        payload_occurrence_id
    } else {
        // 兼容旧 job（payload 未携带 occurrence_id），从 projection 反查
        let proj_ref = format!("kb_document:{}", kb_document_id);
        conn
            .query_row(
                "SELECT occurrence_id FROM artifact_projections
                 WHERE artifact_id = ?1 AND projection_type = 'kb_document' AND projection_ref = ?2 AND status = 'active'
                 LIMIT 1",
                rusqlite::params![artifact_id, proj_ref],
                |row| row.get::<_, Option<i64>>(0),
            )
            .ok()
            .flatten()
            .unwrap_or(0)
    };

    // 执行 promotion extraction
    let result = crate::artifact::promotion::extract_promotion_candidates(
        conn,
        artifact_id,
        occurrence_id,
        kb_document_id,
    );

    match result {
        Ok(candidates) => {
            info!(
                job_id = job.id,
                artifact_id,
                candidate_count = candidates.len(),
                "artifact promote worker: 提取完成"
            );

            // 修复：先执行 auto_apply 再 complete job，与 KB worker 的 promotion 入队逻辑一致。
            // 之前先 complete_kb_job 再 auto_apply，如果 auto_apply 失败，
            // job 已完成不会重试，低风险候选永远不会被自动应用。
            // 现在先尝试 auto_apply，成功后再 complete job；
            // auto_apply 失败时用 fail_kb_job 标记失败，确保可重试。
            let promotion_policy = payload_promotion_policy.unwrap_or_else(|| {
                if occurrence_id > 0 {
                    crate::artifact::store::find_occurrence_by_id(conn, occurrence_id)
                        .ok()
                        .flatten()
                        .map(|o| o.promotion_policy)
                        .unwrap_or_else(|| "candidate".to_string())
                } else {
                    get_promotion_policy(conn, artifact_id)
                }
            });
            if promotion_policy == "auto_accept_low_risk" {
                // 修复：传入 occurrence_id，只自动应用本次上传产生的候选，
                // 避免旧候选被后续重复上传自动提升
                if let Err(e) = crate::artifact::promotion::auto_apply_candidates(
                    conn,
                    artifact_id,
                    Some(kb_document_id),
                    Some(occurrence_id),
                ) {
                    warn!(
                        artifact_id,
                        error = %e,
                        "artifact promote worker: 自动应用低风险候选失败，job 保持可重试状态"
                    );
                    // auto_apply 失败时标记 job 为失败，确保可重试
                    fail_kb_job(conn, job.id, &format!("自动应用低风险候选失败: {}", e))?;
                    return Ok(true);
                }
            }

            complete_kb_job(conn, job.id)?;

            Ok(true)
        }
        Err(e) => {
            warn!(job_id = job.id, artifact_id, error = %e, "artifact promote worker: 提取失败");
            fail_kb_job(conn, job.id, &e.to_string())?;
            Ok(true)
        }
    }
}

/// 对单个节点执行 re-embed，返回嵌入向量维度数
///
/// `target_index_id` 为 0 时自动解析为该节点所属 library 的 active index。
fn reembed_single_node(
    conn: &Connection,
    embedder: &Option<Arc<Embedder>>,
    rt: &tokio::runtime::Runtime,
    node_id: i64,
    target_index_id: Option<i64>,
) -> Result<usize> {
    // 解析 target_index_id：0 → active index
    let resolved_index_id = resolve_target_index(conn, node_id, target_index_id.unwrap_or(0))?;

    let (content, embedding_text): (String, String) = conn
        .query_row(
            "SELECT content, embedding_text FROM kb_document_nodes WHERE id=?1",
            rusqlite::params![node_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .map_err(|e| GBrainError::Database(format!("节点 {} 不存在: {}", node_id, e)))?;

    let text_to_embed = if embedding_text.is_empty() {
        &content
    } else {
        &embedding_text
    };
    let embedder = embedder
        .as_ref()
        .ok_or_else(|| GBrainError::InvalidInput("未配置 embedding API key".into()))?;
    let vectors = rt.block_on(embedder.embed_batch(&[text_to_embed]))?;
    let vec = vectors
        .into_iter()
        .next()
        .ok_or_else(|| GBrainError::InvalidInput("embedding 返回空结果".into()))?;
    let dims = vec.len();

    crate::kb::embedding_index::upsert_node_embedding_for_index(
        conn,
        node_id,
        resolved_index_id,
        &vec,
        dims as i32,
        "text-embedding-3-large",
    )?;
    Ok(dims)
}

/// 对文档中所有缺失 embedding 的节点执行批量 re-embed
///
/// `target_index_id` 为 0 时自动解析为该文档所属 library 的 active index。
fn reembed_document_nodes(
    conn: &Connection,
    embedder: &Option<Arc<Embedder>>,
    rt: &tokio::runtime::Runtime,
    document_id: i64,
    target_index_id: i64,
) -> Result<usize> {
    // 解析 target_index_id：0 → active index（通过文档的第一个节点查找所属 library）
    let resolved_index_id = if target_index_id > 0 {
        target_index_id
    } else {
        conn.query_row(
            "SELECT ei.id FROM kb_embedding_indexes ei \
             JOIN kb_document_nodes n ON n.library_id = ei.library_id \
             WHERE n.document_id = ?1 AND ei.is_active = 1 LIMIT 1",
            rusqlite::params![document_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|_| {
            GBrainError::InvalidInput(format!(
                "文档 {} 所属 library 没有 active embedding index",
                document_id
            ))
        })?
    };

    // 查找文档中无目标 index embedding 的节点
    let node_ids: Vec<(i64, String, String)> = {
        let mut stmt = conn.prepare(
            "SELECT n.id, n.content, n.embedding_text FROM kb_document_nodes n \
             WHERE n.document_id = ?1 \
             AND n.id NOT IN (SELECT node_id FROM kb_node_embeddings \
                              WHERE embedding_index_id = ?2)",
        )?;
        let rows = stmt.query_map(rusqlite::params![document_id, resolved_index_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        rows.filter_map(|r: rusqlite::Result<_>| r.ok()).collect()
    };

    if node_ids.is_empty() {
        return Ok(0);
    }

    let embedder = embedder
        .as_ref()
        .ok_or_else(|| GBrainError::InvalidInput("未配置 embedding API key".into()))?;

    // 批量嵌入（每批最多 20 个节点），始终写入 embedding_index_id
    for chunk in node_ids.chunks(20) {
        let texts: Vec<&str> = chunk
            .iter()
            .map(|(_, c, et)| {
                if et.is_empty() {
                    c.as_str()
                } else {
                    et.as_str()
                }
            })
            .collect();
        let vectors = rt.block_on(embedder.embed_batch(&texts))?;
        for ((node_id, _, _), vec) in chunk.iter().zip(vectors.iter()) {
            let dims = vec.len() as i32;
            crate::kb::embedding_index::upsert_node_embedding_for_index(
                conn,
                *node_id,
                resolved_index_id,
                vec,
                dims,
                "text-embedding-3-large",
            )?;
        }
    }
    Ok(node_ids.len())
}

/// 解析 target_index_id：0 → 查询节点所属 library 的 active index。
/// 找不到 active index 时返回明确错误，不再 fallback 到无效的 0。
fn resolve_target_index(conn: &Connection, node_id: i64, target_index_id: i64) -> Result<i64> {
    if target_index_id > 0 {
        return Ok(target_index_id);
    }
    conn.query_row(
        "SELECT ei.id FROM kb_embedding_indexes ei \
         JOIN kb_document_nodes n ON n.library_id = ei.library_id \
         WHERE n.id = ?1 AND ei.is_active = 1 LIMIT 1",
        rusqlite::params![node_id],
        |row| row.get(0),
    )
    .map_err(|_| {
        GBrainError::InvalidInput(format!(
            "节点 {} 所属 library 没有 active embedding index",
            node_id
        ))
    })
}

/// 以守护进程模式运行 KB worker：持续轮询并处理所有类型的 KB 作业。
/// 包括 kb_process_document（文档解析/切分/嵌入）、kb_reembed（文档级重嵌入）、
/// kb_reembed_node（单节点修复）。
///
/// `interval_secs` 为无作业时的轮询间隔。
pub fn run_kb_worker_loop(engine: &SqliteEngine, config: &Config, interval_secs: u64) -> ! {
    info!(interval_secs, "KB worker: 启动守护进程模式");
    loop {
        // 先处理文档处理作业，再处理 re-embed 作业
        let had_work = match run_kb_worker_once(engine, config) {
            Ok(true) => true,
            Ok(false) => match run_reembed_worker_once(engine, config) {
                Ok(true) => true,
                Ok(false) => match run_artifact_worker_once(engine, config) {
                    Ok(true) => true,
                    Ok(false) => false,
                    Err(e) => {
                        warn!(error = %e, "artifact promote worker: 处理循环出错");
                        false
                    }
                },
                Err(e) => {
                    warn!(error = %e, "re-embed worker: 处理循环出错");
                    false
                }
            },
            Err(e) => {
                warn!(error = %e, "KB worker: 处理循环出错");
                false
            }
        };
        if !had_work {
            // 无待处理作业，等待后重试
            std::thread::sleep(std::time::Duration::from_secs(interval_secs));
        }
    }
}

/// 在独立线程中启动 KB worker 守护进程。
pub fn spawn_kb_worker_thread(db_path: PathBuf, config: Config, interval_secs: u64) {
    // 防御性检查：kb_enabled=false 时不启动 worker
    if !config.kb_enabled {
        info!("KB subsystem is disabled, worker thread not spawned");
        return;
    }
    std::thread::Builder::new()
        .name("kb-worker".to_string())
        .spawn(move || {
            // 在 worker 线程中创建独立的数据库连接（使用与主线程相同的 Config）
            let mut engine = SqliteEngine::with_config(&db_path, config.clone());
            if let Err(e) = engine.connect() {
                warn!(error = %e, "KB worker: 数据库连接失败");
                return;
            }
            if let Err(e) = engine.init_schema() {
                warn!(error = %e, "KB worker: 初始化 schema 失败");
                return;
            }
            info!("KB worker: 后台线程已启动");
            run_kb_worker_loop(&engine, &config, interval_secs);
        })
        .expect("无法创建 KB worker 线程");
}

// ---------------------------------------------------------------------------
// P5-023: 多 worker 并发处理
// ---------------------------------------------------------------------------

/// 启动多个并发 KB worker 线程。
///
/// 每个 worker 拥有独立的数据库连接。`claim_next_kb_job` 的原子认领
/// 保证同一作业不会被两个 worker 同时处理。
///
/// `worker_count` 为并发 worker 数量（建议不超过 CPU 核心数）。
/// `shutdown` 信号用于优雅停止所有 worker。
pub fn spawn_kb_worker_pool(
    db_path: PathBuf,
    config: Config,
    interval_secs: u64,
    worker_count: usize,
    shutdown: Arc<AtomicBool>,
) {
    if !config.kb_enabled {
        info!("KB subsystem is disabled, worker pool not spawned");
        return;
    }
    if worker_count == 0 {
        info!("KB worker pool: worker_count=0, nothing to spawn");
        return;
    }
    info!(
        worker_count,
        "KB worker pool: 启动 {} 个 worker", worker_count
    );

    for i in 0..worker_count {
        let db_path = db_path.clone();
        let config = config.clone();
        let shutdown = shutdown.clone();
        let thread_name = format!("kb-worker-{}", i);

        std::thread::Builder::new()
            .name(thread_name.clone())
            .spawn(move || {
                let mut engine = SqliteEngine::with_config(&db_path, config.clone());
                if let Err(e) = engine.connect() {
                    warn!(worker = i, error = %e, "KB worker: 数据库连接失败");
                    return;
                }
                if let Err(e) = engine.init_schema() {
                    warn!(worker = i, error = %e, "KB worker: 初始化 schema 失败");
                    return;
                }
                info!(worker = i, "KB worker: 后台线程已启动");

                loop {
                    if shutdown.load(Ordering::Relaxed) {
                        info!(worker = i, "KB worker: 收到停止信号，退出");
                        break;
                    }
                    // 先处理文档处理作业，再处理 re-embed 作业，最后处理 artifact promote 作业
                    // 修复：pool 分支缺少 artifact_promote_extract 处理，
                    // KB 处理完成后入队的 promote 作业会一直 pending
                    let had_work = match run_kb_worker_once(&engine, &config) {
                        Ok(true) => true,
                        Ok(false) => match run_reembed_worker_once(&engine, &config) {
                            Ok(true) => true,
                            Ok(false) => match run_artifact_worker_once(&engine, &config) {
                                Ok(true) => true,
                                Ok(false) => false,
                                Err(e) => {
                                    warn!(worker = i, error = %e, "artifact promote worker: 出错");
                                    false
                                }
                            },
                            Err(e) => {
                                warn!(worker = i, error = %e, "re-embed worker: 出错");
                                false
                            }
                        },
                        Err(e) => {
                            warn!(worker = i, error = %e, "KB worker: 处理循环出错");
                            std::thread::sleep(std::time::Duration::from_secs(interval_secs));
                            continue;
                        }
                    };
                    if had_work {
                        continue;
                    }
                    // 无作业时短暂等待，但提前检查 shutdown
                    for _ in 0..interval_secs {
                        if shutdown.load(Ordering::Relaxed) {
                            info!(worker = i, "KB worker: 收到停止信号，退出");
                            return;
                        }
                        std::thread::sleep(std::time::Duration::from_secs(1));
                    }
                }
            })
            .unwrap_or_else(|_| panic!("无法创建 {} 线程", thread_name));
    }
}

/// KB 处理完成后，检查 kb_document 是否有对应的 artifact 投影，
/// 如果有且 promotion_policy 允许，则入队 `artifact_promote_extract` 作业。
///
/// 修复：入队前校验 kb_documents 的 processing_run_id 仍与传入的 run_id 匹配，
/// 防止旧 job 在 stats 成功后、入队 promotion 前，新 run 已通过 projection.rs
/// 抢占当前投影，导致旧 job 给新 occurrence 入队 promotion。
fn enqueue_artifact_promote_if_linked(
    conn: &Connection,
    kb_document_id: i64,
    run_id: &str,
) -> Result<()> {
    // 修复：将 run-id 校验、projection 查询和 job 插入合入同一 BEGIN IMMEDIATE 事务，
    // 消除竞态窗口。旧代码用 unchecked_transaction()（deferred 事务），
    // 并发上传在 promotion 入队阶段抢到写锁时可能在 INSERT job 时失败，
    // 而 KB job 已标记完成，promotion 入队只 warn 不会重试。
    // BEGIN IMMEDIATE 在 BEGIN 时即获取 RESERVED 锁，阻止其他写事务并发修改。
    conn.execute("BEGIN IMMEDIATE", [])
        .map_err(|e| GBrainError::Database(format!("开启 BEGIN IMMEDIATE 事务失败: {}", e)))?;

    // 校验 processing_run_id 仍匹配，防止旧 job 的 promotion 入队串到新 occurrence
    let current_run_id: String = conn
        .query_row(
            "SELECT processing_run_id FROM kb_documents WHERE id = ?1",
            rusqlite::params![kb_document_id],
            |row| row.get::<_, String>(0),
        )
        .map_err(|e| {
            conn.execute("ROLLBACK", []).ok();
            GBrainError::Database(format!("查询 processing_run_id 失败: {}", e))
        })?;
    if current_run_id != run_id {
        tracing::warn!(
            kb_document_id,
            expected_run_id = run_id,
            actual_run_id = %current_run_id,
            "KB worker: promotion 入队时 run_id 已过期，跳过入队"
        );
        let _ = conn.execute("ROLLBACK", []);
        return Ok(());
    }

    // 查找 kb_document 对应的 artifact 投影
    let proj_ref = format!("kb_document:{}", kb_document_id);
    // 修复：不再用 .ok() 吞掉所有数据库错误。
    // .ok() 把真实查询/prepare/step 错误当成"无 projection"跳过，
    // 但 KB job 已标记 completed，promotion 入队永久丢失。
    // 现在只把 QueryReturnedNoRows 转为 None（确实无投影），其它错误 rollback 后返回 Err。
    let result: Option<(i64, i64, String)> = match conn.query_row(
        "SELECT ap.artifact_id, ap.occurrence_id, ao.promotion_policy
         FROM artifact_projections ap
         JOIN artifact_occurrences ao ON ao.id = ap.occurrence_id
         WHERE ap.projection_ref = ?1 AND ap.status = 'active'
         LIMIT 1",
        rusqlite::params![proj_ref],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        },
    ) {
        Ok(tuple) => Some(tuple),
        Err(rusqlite::Error::QueryReturnedNoRows) => None,
        Err(e) => {
            conn.execute("ROLLBACK", []).ok();
            return Err(GBrainError::Database(format!(
                "查询 artifact 投影失败: {}",
                e
            )));
        }
    };

    let Some((artifact_id, occurrence_id, promotion_policy)) = result else {
        debug!(
            kb_document_id,
            "KB document 无关联 artifact 投影，跳过 promotion"
        );
        let _ = conn.execute("ROLLBACK", []);
        return Ok(());
    };

    // 根据 promotion_policy 决定是否入队：
    // - none: 不触发 promotion
    // - shadow: 仅影子页面，不生成候选
    // - candidate / auto_accept_low_risk: 触发 promotion extraction
    match promotion_policy.as_str() {
        "none" => {
            debug!(
                artifact_id,
                kb_document_id, "promotion_policy=none，跳过 promotion"
            );
            let _ = conn.execute("ROLLBACK", []);
            return Ok(());
        }
        "shadow" => {
            debug!(
                artifact_id,
                kb_document_id, "promotion_policy=shadow，仅影子页面，不生成候选"
            );
            let _ = conn.execute("ROLLBACK", []);
            return Ok(());
        }
        _ => {} // candidate / auto_accept_low_risk → 继续入队
    }

    // 修复：入队时把 occurrence_id 和 promotion_policy 写进 payload，
    // worker 只使用 payload 中的稳定值，避免旧 job 被归属到新 occurrence 后串策略
    conn.execute(
        "INSERT INTO jobs (job_type, payload, status, priority, created_at)
         VALUES ('artifact_promote_extract', ?1, 'pending', 0, datetime('now'))",
        rusqlite::params![serde_json::json!({
            "artifact_id": artifact_id,
            "kb_document_id": kb_document_id,
            "occurrence_id": occurrence_id,
            "promotion_policy": promotion_policy,
        })
        .to_string()],
    )
    .map_err(|e| {
        conn.execute("ROLLBACK", []).ok();
        GBrainError::Database(format!("入队 artifact_promote_extract 失败: {}", e))
    })?;

    // 修复：COMMIT 失败时事务可能仍保持打开，必须 ROLLBACK 防止连接状态污染。
    // SQLite COMMIT 返回 busy/错误时，事务不会自动回滚，后续复用同一连接
    // 会处于事务中，且 promotion 已无重试入口。
    conn.execute("COMMIT", []).map_err(|e| {
        let _ = conn.execute("ROLLBACK", []);
        GBrainError::Database(format!("提交 promotion 入队事务失败: {}", e))
    })?;

    info!(
        artifact_id,
        kb_document_id,
        occurrence_id,
        promotion_policy = %promotion_policy,
        "已入队 artifact_promote_extract 作业"
    );
    Ok(())
}

/// 获取 artifact 的 promotion_policy
///
/// 从 artifact_occurrences 表查询 promotion_policy，
/// 找不到时降级为 "candidate"（需要手动审核）。
fn get_promotion_policy(conn: &Connection, artifact_id: i64) -> String {
    conn.query_row(
        "SELECT ao.promotion_policy
         FROM artifact_occurrences ao
         WHERE ao.artifact_id = ?1 AND ao.status = 'active'
         LIMIT 1",
        rusqlite::params![artifact_id],
        |row| row.get::<_, String>(0),
    )
    .unwrap_or_else(|_| "candidate".to_string())
}
