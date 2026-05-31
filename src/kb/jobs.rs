//! KB job payload/status helpers
//!
//! Reuses the global `jobs` table for KB document processing tasks.

use crate::error::{GBrainError, Result};
use crate::jobs::{JobInput, JobQueue, JobStatus};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// L2 修复：作业优先级集中配置，便于调整调度策略
// 数值越高越优先处理，默认 0 为标准优先级

/// KB 文档处理作业优先级
const PRIORITY_KB_PROCESS: i32 = 0;
/// OCR 作业优先级（供 pipeline/mcp 等外部入队点引用）
#[allow(dead_code)]
const PRIORITY_KB_OCR: i32 = 0;
/// 重嵌入作业优先级（低于标准，避免占用前台资源）
#[allow(dead_code)]
const PRIORITY_KB_REEMBED: i32 = -1;
/// 同义词挖掘作业优先级（最低，后台静默执行）
const PRIORITY_KB_SYNONYMS: i32 = -2;

/// KB document processing job payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KbProcessPayload {
    pub kind: String, // "kb_process_document"
    pub document_id: i64,
    pub library_id: i64,
    pub processing_run_id: String,
    pub storage_path: String,
    pub extension: String,
}

/// Phase 3: OCR 文档处理 job payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KbOcrPayload {
    pub kind: String, // "kb_ocr_document"
    pub document_id: i64,
    pub library_id: i64,
    pub processing_run_id: String,
    pub storage_path: String,
    /// 需要 OCR 的页码列表（1-based）
    pub pages: Vec<i32>,
    pub submit_mode: String,
    pub provider: String,
    pub model: String,
    /// 是否返回裁剪图片
    pub return_crop_images: bool,
    /// 是否需要版面可视化
    pub need_layout_visualization: bool,
}

/// P5-013: Reembed job payload — re-embed documents into a new embedding index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KbReembedPayload {
    pub kind: String, // "kb_reembed"
    /// Scope: "document" or "library"
    pub scope: String,
    /// Document ID (when scope is "document")
    pub document_id: Option<i64>,
    /// Library ID (required for both scopes)
    pub library_id: i64,
    /// Target embedding index ID to write embeddings into
    pub target_embedding_index_id: i64,
}

/// Generate a new processing run ID
pub fn new_run_id() -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        chrono::Utc::now()
            .timestamp_nanos_opt()
            .unwrap_or(0)
            .to_le_bytes(),
    );
    hasher.update(rand::random::<[u8; 16]>());
    let hash = hasher.finalize();
    format!("run_{}", hex::encode(&hash[..8]))
}

/// Generate a new job ID
pub fn new_job_id() -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        chrono::Utc::now()
            .timestamp_nanos_opt()
            .unwrap_or(0)
            .to_le_bytes(),
    );
    hasher.update(rand::random::<[u8; 16]>());
    let hash = hasher.finalize();
    format!("job_{}", hex::encode(&hash[..8]))
}

/// Enqueue a KB document processing job.
/// Returns the database row ID of the new job.
pub fn enqueue_kb_process_job(conn: &Connection, payload: &KbProcessPayload) -> Result<i64> {
    let queue = JobQueue::new(conn);

    let input = JobInput {
        job_type: "kb_process_document".to_string(),
        payload: serde_json::to_value(payload)
            .map_err(|e| GBrainError::Serialization(e.to_string()))?,
        priority: Some(PRIORITY_KB_PROCESS),
        max_attempts: Some(3),
    };
    queue.enqueue(input)
}

/// Get the status of a KB document's processing job by database row ID
pub fn get_kb_job_status(conn: &Connection, job_db_id: i64) -> Result<Option<JobStatus>> {
    let queue = JobQueue::new(conn);
    match queue.get(job_db_id) {
        Ok(Some(job)) => Ok(Some(job.status)),
        Ok(None) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Cancel a KB document's processing job by database row ID
pub fn cancel_kb_job(conn: &Connection, job_db_id: i64) -> Result<()> {
    let queue = JobQueue::new(conn);
    queue.cancel(job_db_id)
}

/// Claim and get the next pending KB job.
/// Dequeues from the global jobs table filtering by job_type.
/// Returns (job_db_id, payload) so the caller can complete/fail the job.
pub fn claim_next_kb_job(conn: &Connection) -> Result<Option<(i64, KbProcessPayload)>> {
    let queue = JobQueue::new(conn);
    // Dequeue the next pending job filtering by KB type
    match queue.dequeue_by_type("kb_process_document") {
        Ok(Some(job)) => {
            let payload: KbProcessPayload = serde_json::from_value(job.payload).map_err(|e| {
                GBrainError::Serialization(format!("invalid KB job payload: {}", e))
            })?;
            Ok(Some((job.id, payload)))
        }
        Ok(None) => Ok(None),
        Err(e) => Err(e),
    }
}

/// 认领下一个 re-embed 作业（kb_reembed 或 kb_reembed_node）。
///
/// 优先认领 kb_reembed_node（单节点修复），再认领 kb_reembed（文档级重嵌入）。
/// 返回 (job_db_id, job_type, payload_json) 供 worker 处理。
pub fn claim_next_reembed_job(
    conn: &Connection,
) -> Result<Option<(i64, String, serde_json::Value)>> {
    let queue = JobQueue::new(conn);
    // 优先处理单节点修复
    if let Ok(Some(job)) = queue.dequeue_by_type("kb_reembed_node") {
        return Ok(Some((job.id, "kb_reembed_node".into(), job.payload)));
    }
    if let Ok(Some(job)) = queue.dequeue_by_type("kb_reembed") {
        return Ok(Some((job.id, "kb_reembed".into(), job.payload)));
    }
    Ok(None)
}

/// Mark a KB job as completed by database row ID
pub fn complete_kb_job(conn: &Connection, job_db_id: i64) -> Result<()> {
    let queue = JobQueue::new(conn);
    queue.complete(job_db_id)
}

/// Mark a KB job as failed by database row ID
pub fn fail_kb_job(conn: &Connection, job_db_id: i64, error: &str) -> Result<()> {
    let queue = JobQueue::new(conn);
    queue.fail(job_db_id, error)
}

// ---------------------------------------------------------------------------
// E6: Token synonym mining job
// ---------------------------------------------------------------------------

/// E6: Synonym mining job payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KbMineSynonymsPayload {
    pub kind: String, // "kb_mine_synonyms"
    /// Library ID to mine (None = all libraries)
    pub library_id: Option<i64>,
    /// Full rebuild (ignore existing embeddings)
    pub full: bool,
}

/// Enqueue a synonym mining job. Only enqueues if one isn't already pending.
pub fn enqueue_mine_synonyms_job(
    conn: &Connection,
    library_id: Option<i64>,
    full: bool,
) -> Result<i64> {
    let queue = JobQueue::new(conn);
    // Avoid duplicate pending jobs
    let existing = conn
        .query_row(
            "SELECT COUNT(*) FROM jobs WHERE job_type = 'kb_mine_synonyms' AND status = 'pending'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0);
    if existing > 0 {
        return Ok(0);
    }
    let payload = KbMineSynonymsPayload {
        kind: "kb_mine_synonyms".to_string(),
        library_id,
        full,
    };
    queue.enqueue(JobInput {
        job_type: "kb_mine_synonyms".to_string(),
        payload: serde_json::to_value(&payload)
            .map_err(|e| GBrainError::Serialization(e.to_string()))?,
        priority: Some(PRIORITY_KB_SYNONYMS),
        max_attempts: Some(2),
    })
}

/// Claim the next pending synonym mining job.
pub fn claim_next_mine_synonyms_job(
    conn: &Connection,
) -> Result<Option<(i64, KbMineSynonymsPayload)>> {
    let queue = JobQueue::new(conn);
    match queue.dequeue_by_type("kb_mine_synonyms") {
        Ok(Some(job)) => {
            let payload: KbMineSynonymsPayload =
                serde_json::from_value(job.payload).map_err(|e| {
                    GBrainError::Serialization(format!("invalid mine_synonyms payload: {}", e))
                })?;
            Ok(Some((job.id, payload)))
        }
        Ok(None) => Ok(None),
        Err(e) => Err(e),
    }
}

// ---------------------------------------------------------------------------
// P5-026/P6-010: Job management helpers (list/pause/resume)
// ---------------------------------------------------------------------------

use rusqlite::params;

/// 列出 KB 作业，可选按 library 过滤。
/// 返回 (job_id, status, document_id) 元组列表。
pub fn list_kb_jobs(conn: &Connection, library_id: Option<i64>) -> Result<Vec<(i64, String, i64)>> {
    let sql = if library_id.is_some() {
        "SELECT j.id, j.status, j.payload FROM jobs j \
         WHERE j.job_type='kb_process_document' \
         AND j.payload->>'$.library_id' = ?1 \
         ORDER BY j.id DESC LIMIT 100"
    } else {
        "SELECT j.id, j.status, j.payload FROM jobs j \
         WHERE j.job_type='kb_process_document' \
         ORDER BY j.id DESC LIMIT 100"
    };
    let mut stmt = conn.prepare(sql)?;

    // 统一闭包：解析 payload 中的 document_id
    let parse_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<(i64, String, i64)> {
        let id: i64 = row.get(0)?;
        let status: String = row.get(1)?;
        let payload_str: String = row.get(2)?;
        let payload: serde_json::Value =
            serde_json::from_str(&payload_str).unwrap_or(serde_json::Value::Null);
        let doc_id: i64 = payload
            .get("document_id")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        Ok((id, status, doc_id))
    };

    let results: Vec<(i64, String, i64)> = if let Some(lib_id) = library_id {
        stmt.query_map(params![lib_id], parse_row)?
            .filter_map(|r| r.ok())
            .collect()
    } else {
        stmt.query_map([], parse_row)?
            .filter_map(|r| r.ok())
            .collect()
    };
    Ok(results)
}

/// Pause KB job processing for a library by cancelling pending jobs.
pub fn pause_library_jobs(conn: &Connection, library_id: i64) -> Result<()> {
    conn.execute(
        "UPDATE jobs SET status='cancelled' \
         WHERE job_type='kb_process_document' AND status='pending' \
         AND payload->>'$.library_id' = ?1",
        params![library_id],
    )?;
    Ok(())
}

/// Resume KB job processing for a library by re-queuing cancelled jobs.
pub fn resume_library_jobs(conn: &Connection, library_id: i64) -> Result<()> {
    conn.execute(
        "UPDATE jobs SET status='pending' \
         WHERE job_type='kb_process_document' AND status='cancelled' \
         AND payload->>'$.library_id' = ?1",
        params![library_id],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Phase 3: OCR 作业入队与认领
// ---------------------------------------------------------------------------

/// 入队 OCR 文档处理作业。
/// 返回新作业的数据库行 ID。
pub fn enqueue_kb_ocr_job(conn: &Connection, payload: &KbOcrPayload) -> Result<i64> {
    let queue = JobQueue::new(conn);
    let input = JobInput {
        job_type: "kb_ocr_document".to_string(),
        payload: serde_json::to_value(payload)
            .map_err(|e| GBrainError::Serialization(e.to_string()))?,
        priority: Some(PRIORITY_KB_OCR),
        max_attempts: Some(3),
    };
    queue.enqueue(input)
}

/// 认领下一个待处理的 OCR 作业。
/// 返回 (job_db_id, payload) 供 worker 处理。
pub fn claim_next_ocr_job(conn: &Connection) -> Result<Option<(i64, KbOcrPayload)>> {
    let queue = JobQueue::new(conn);
    match queue.dequeue_by_type("kb_ocr_document") {
        Ok(Some(job)) => {
            let payload: KbOcrPayload = serde_json::from_value(job.payload).map_err(|e| {
                GBrainError::Serialization(format!("无效的 OCR 作业 payload: {}", e))
            })?;
            Ok(Some((job.id, payload)))
        }
        Ok(None) => Ok(None),
        Err(e) => Err(e),
    }
}
