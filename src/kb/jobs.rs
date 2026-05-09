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
pub fn claim_next_kb_job(conn: &Connection) -> Result<Option<KbProcessPayload>> {
    let queue = JobQueue::new(conn);
    // Dequeue the next pending job filtering by KB type
    match queue.dequeue_by_type("kb_process_document") {
        Ok(Some(job)) => {
            let payload: KbProcessPayload = serde_json::from_value(job.payload).map_err(|e| {
                GBrainError::Serialization(format!("invalid KB job payload: {}", e))
            })?;
            Ok(Some(payload))
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
