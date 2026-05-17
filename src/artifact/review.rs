//! 建议变更 — promotion 的用户友好包装（设计文档 §4.1.5）
//!
//! 对外暴露为 "suggested changes" / "review" 概念，
//! 内部仍然使用 promotion_candidates 表。

use crate::artifact::promotion;
use crate::artifact::types::{ArtifactReviewItem, PromotionCandidate, ReviewCandidateInput};
use crate::error::Result;
use rusqlite::Connection;

/// 列出建议变更
pub fn list_suggested_changes(
    conn: &Connection,
    status: Option<&str>,
    target_slug: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<ArtifactReviewItem>> {
    let candidates = promotion::list_candidates(conn, status, None, target_slug, limit, offset)?;
    Ok(candidates
        .into_iter()
        .map(candidate_to_review_item)
        .collect())
}

/// 获取建议变更详情
pub fn get_suggested_change(
    conn: &Connection,
    change_id: i64,
) -> Result<Option<ArtifactReviewItem>> {
    let candidate = promotion::find_candidate_by_id(conn, change_id)?;
    Ok(candidate.map(candidate_to_review_item))
}

/// 应用建议变更
pub fn apply_suggested_change(conn: &Connection, change_id: i64) -> Result<PromotionCandidate> {
    promotion::apply_candidate(conn, change_id)
}

/// 拒绝建议变更
pub fn reject_suggested_change(
    conn: &Connection,
    input: &ReviewCandidateInput,
) -> Result<PromotionCandidate> {
    promotion::review_candidate(conn, input)
}

/// 回滚已应用的建议变更
pub fn rollback_suggested_change(conn: &Connection, change_id: i64) -> Result<PromotionCandidate> {
    promotion::rollback_candidate(conn, change_id)
}

/// 将内部 PromotionCandidate 转换为用户友好的 ArtifactReviewItem
fn candidate_to_review_item(c: PromotionCandidate) -> ArtifactReviewItem {
    ArtifactReviewItem {
        change_id: c.id,
        target_slug: c.target_slug.clone(),
        status: c.status.clone(),
        risk_level: c.risk_level.clone(),
        summary: c.title.clone(),
        evidence: if c.evidence_json.is_empty() {
            None
        } else {
            serde_json::from_str::<serde_json::Value>(&c.evidence_json).ok()
        },
        created_at: Some(c.created_at.clone()),
    }
}
