//! KB job payload/status helpers
//!
//! Reuses the global `jobs` table for KB document processing tasks.

use crate::error::{GBrainError, Result};
use crate::jobs::{JobInput, JobQueue, JobStatus};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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
        priority: Some(0),
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
// P5-026/P6-010: Job management helpers (list/pause/resume)
// ---------------------------------------------------------------------------

use rusqlite::params;

/// 列出 KB 作业，可选按 library 过滤。
/// 返回 (job_id, status, document_id) 元组列表。
pub fn list_kb_jobs(
    conn: &Connection,
    library_id: Option<i64>,
) -> Result<Vec<(i64, String, i64)>> {
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
        let payload: serde_json::Value = serde_json::from_str(&payload_str).unwrap_or(serde_json::Value::Null);
        let doc_id: i64 = payload.get("document_id")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        Ok((id, status, doc_id))
    };

    let results: Vec<(i64, String, i64)> = if let Some(lib_id) = library_id {
        stmt.query_map(params![lib_id], parse_row)?
            .filter_map(|r| r.ok()).collect()
    } else {
        stmt.query_map([], parse_row)?
            .filter_map(|r| r.ok()).collect()
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
