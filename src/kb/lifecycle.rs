//! KB 文档生命周期管理
//!
//! 状态机、版本守卫、软删除、purge。

use crate::error::{GBrainError, Result};
use crate::kb::engine::KbEngine;
use rusqlite::{params, Connection};

/// 文档处理状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentStatus {
    Queued,
    Processing,
    Ready,
    Failed,
    Deleted,
}

impl DocumentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Processing => "processing",
            Self::Ready => "ready",
            Self::Failed => "failed",
            Self::Deleted => "deleted",
        }
    }

    /// 验证状态转换是否合法
    pub fn can_transition_to(&self, target: Self) -> bool {
        matches!(
            (self, target),
            (Self::Queued, Self::Processing)
                | (Self::Processing, Self::Ready)
                | (Self::Processing, Self::Failed)
                | (Self::Failed, Self::Queued) // 重试
                | (Self::Ready, Self::Queued) // 重新处理
                | (_, Self::Deleted) // 任何状态都可删除
        )
    }
}

/// 转换文档状态，带合法性检查。
pub fn transition_document_status(
    conn: &Connection,
    document_id: i64,
    new_status: DocumentStatus,
    run_id: Option<&str>,
    error_message: Option<&str>,
) -> Result<()> {
    // 读取当前状态
    let current_status: String = conn
        .query_row(
            "SELECT document_status FROM kb_documents WHERE id = ?1",
            params![document_id],
            |row| row.get(0),
        )
        .map_err(|e| GBrainError::Database(format!("document not found: {}", e)))?;

    let current = match current_status.as_str() {
        "queued" => DocumentStatus::Queued,
        "processing" => DocumentStatus::Processing,
        "ready" => DocumentStatus::Ready,
        "failed" => DocumentStatus::Failed,
        "deleted" => DocumentStatus::Deleted,
        _ => DocumentStatus::Queued,
    };

    if !current.can_transition_to(new_status) {
        return Err(GBrainError::InvalidInput(format!(
            "invalid status transition: {} -> {}",
            current.as_str(),
            new_status.as_str()
        )));
    }

    // 如果提供了 run_id，执行版本守卫检查
    if let Some(rid) = run_id {
        let stored_run_id: String = conn.query_row(
            "SELECT processing_run_id FROM kb_documents WHERE id = ?1",
            params![document_id],
            |row| row.get(0),
        )?;
        if !stored_run_id.is_empty() && stored_run_id != rid {
            return Err(GBrainError::InvalidInput(format!(
                "stale run: current run_id={} does not match provided run_id={}",
                stored_run_id, rid
            )));
        }
    }

    let error = error_message.unwrap_or("");
    conn.execute(
        "UPDATE kb_documents SET document_status = ?1, index_status = ?2, \
         parsing_error = CASE WHEN ?3 != '' THEN ?3 ELSE parsing_error END, \
         updated_at = datetime('now') WHERE id = ?4",
        params![new_status.as_str(), new_status.as_str(), error, document_id],
    )?;

    Ok(())
}

/// 将文档标记为软删除。默认搜索结果过滤软删除文档。
pub fn soft_delete_document(conn: &Connection, document_id: i64) -> Result<()> {
    conn.execute(
        "UPDATE kb_documents SET deleted_at = datetime('now'), \
         document_status = 'deleted', updated_at = datetime('now') WHERE id = ?1",
        params![document_id],
    )?;
    Ok(())
}

/// 彻底清理文档及其所有关联数据（purge）。
pub fn purge_document(kb: &KbEngine, document_id: i64) -> Result<()> {
    kb.transaction(|conn| {
        // 验证文档存在且已软删除
        let deleted_at: Option<String> = conn
            .query_row(
                "SELECT deleted_at FROM kb_documents WHERE id = ?1",
                params![document_id],
                |row| row.get(0),
            )
            .map_err(|_| GBrainError::InvalidInput("document not found or not deleted".to_string()))?;

        if deleted_at.is_none() {
            return Err(GBrainError::InvalidInput(
                "document must be soft-deleted before purge".to_string(),
            ));
        }

        // 清理表格行
        conn.execute(
            "DELETE FROM kb_table_rows WHERE table_id IN \
             (SELECT id FROM kb_tables WHERE document_id = ?1)",
            params![document_id],
        )?;
        conn.execute("DELETE FROM kb_tables WHERE document_id = ?1", params![document_id])?;

        // 清理摘要
        conn.execute(
            "DELETE FROM kb_document_summaries WHERE document_id = ?1",
            params![document_id],
        )?;

        // 清理 source_items 引用
        conn.execute(
            "UPDATE kb_source_items SET document_id = NULL WHERE document_id = ?1",
            params![document_id],
        )?;

        // 清理搜索反馈
        conn.execute(
            "DELETE FROM kb_search_feedback WHERE document_id = ?1",
            params![document_id],
        )?;

        // 清理 FTS 条目（通过删除 nodes 触发触发器）和 embedding 数据
        let node_ids: Vec<i64> = {
            let mut stmt =
                conn.prepare("SELECT id FROM kb_document_nodes WHERE document_id = ?1")?;
            let rows = stmt.query_map(params![document_id], |row| row.get(0))?;
            rows.filter_map(|r| r.ok()).collect()
        };

        for node_id in &node_ids {
            conn.execute(
                "DELETE FROM kb_node_embeddings WHERE node_id = ?1",
                params![node_id],
            )?;
        }
        conn.execute(
            "DELETE FROM kb_document_nodes WHERE document_id = ?1",
            params![document_id],
        )?;

        // 清理版本记录
        conn.execute(
            "DELETE FROM kb_document_versions WHERE document_id = ?1",
            params![document_id],
        )?;

        // 删除存储文件（如果有 storage_path）
        let storage_path: Option<String> = conn
            .query_row(
                "SELECT storage_path FROM kb_documents WHERE id = ?1",
                params![document_id],
                |row| row.get(0),
            )
            .ok()
            .filter(|s: &String| !s.is_empty());
        if let Some(path) = storage_path {
            let _ = std::fs::remove_file(&path);
        }

        // 标记为已 purge
        conn.execute(
            "UPDATE kb_documents SET purged_at = datetime('now'), \
             storage_path = '', updated_at = datetime('now') WHERE id = ?1",
            params![document_id],
        )?;

        Ok(())
    })
}

/// 创建文档版本快照（在更新前调用）。
pub fn create_document_version(
    conn: &Connection,
    document_id: i64,
    version_label: &str,
    processing_run_id: &str,
    char_count: i32,
    node_count: i32,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO kb_document_versions \
         (document_id, version_label, processing_run_id, char_count, node_count, index_status) \
         VALUES (?1, ?2, ?3, ?4, ?5, 'archived')",
        params![document_id, version_label, processing_run_id, char_count, node_count],
    )?;
    let version_id = conn.last_insert_rowid();

    // 更新 current_version_id
    conn.execute(
        "UPDATE kb_documents SET current_version_id = ?1, updated_at = datetime('now') \
         WHERE id = ?2",
        params![version_id, document_id],
    )?;

    Ok(version_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_transitions_valid() {
        assert!(DocumentStatus::Queued.can_transition_to(DocumentStatus::Processing));
        assert!(DocumentStatus::Processing.can_transition_to(DocumentStatus::Ready));
        assert!(DocumentStatus::Processing.can_transition_to(DocumentStatus::Failed));
        assert!(DocumentStatus::Failed.can_transition_to(DocumentStatus::Queued));
        assert!(DocumentStatus::Ready.can_transition_to(DocumentStatus::Deleted));
    }

    #[test]
    fn test_status_transitions_invalid() {
        assert!(!DocumentStatus::Ready.can_transition_to(DocumentStatus::Processing));
        assert!(!DocumentStatus::Ready.can_transition_to(DocumentStatus::Failed));
        assert!(!DocumentStatus::Queued.can_transition_to(DocumentStatus::Ready));
        assert!(!DocumentStatus::Deleted.can_transition_to(DocumentStatus::Queued));
    }

    #[test]
    fn test_status_as_str() {
        assert_eq!(DocumentStatus::Queued.as_str(), "queued");
        assert_eq!(DocumentStatus::Ready.as_str(), "ready");
        assert_eq!(DocumentStatus::Failed.as_str(), "failed");
    }
}
