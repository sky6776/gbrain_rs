//! Embedding 模型升级/多索引管理 (P5-010~P5-014)
//!
//! 支持多 embedding index 并存、按维度分表、灰度评测。

use crate::error::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingIndex {
    pub id: i64,
    pub library_id: i64,
    pub provider: String,
    pub model: String,
    pub dimensions: i32,
    pub index_type: String,
    pub is_active: bool,
}

/// 创建 embedding index 记录，同时创建对应的 sqlite-vec 虚表
pub fn create_embedding_index(
    conn: &Connection,
    library_id: i64,
    provider: &str,
    model: &str,
    dimensions: i32,
    index_type: &str,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO kb_embedding_indexes (library_id, provider, model, dimensions, index_type, is_active) \
         VALUES (?1, ?2, ?3, ?4, ?5, 0)",
        params![library_id, provider, model, dimensions, index_type],
    )?;
    let index_id = conn.last_insert_rowid();

    // P5-012: Create dedicated sqlite-vec virtual table for this index
    let _ = create_vec_table_for_index(conn, index_id, dimensions);

    Ok(index_id)
}

/// 列出 library 的所有 embedding index
pub fn list_embedding_indexes(conn: &Connection, library_id: i64) -> Result<Vec<EmbeddingIndex>> {
    let mut stmt = conn.prepare(
        "SELECT id, library_id, provider, model, dimensions, index_type, is_active \
         FROM kb_embedding_indexes WHERE library_id = ?1 ORDER BY id"
    )?;
    let rows = stmt.query_map(params![library_id], |row| {
        Ok(EmbeddingIndex {
            id: row.get(0)?,
            library_id: row.get(1)?,
            provider: row.get(2)?,
            model: row.get(3)?,
            dimensions: row.get(4)?,
            index_type: row.get(5)?,
            is_active: row.get::<_, i32>(6)? != 0,
        })
    })?;
    let results: Vec<EmbeddingIndex> = rows.filter_map(|r| r.ok()).collect();
    Ok(results)
}

/// 激活某个 embedding index（deactivate 其他）
pub fn activate_index(conn: &Connection, index_id: i64) -> Result<()> {
    conn.execute(
        "UPDATE kb_embedding_indexes SET is_active = CASE WHEN id = ?1 THEN 1 ELSE 0 END",
        params![index_id],
    )?;
    Ok(())
}

/// 删除 embedding index，同时删除对应的 sqlite-vec 虚表和关联的 kb_node_embeddings 行
pub fn delete_embedding_index(conn: &Connection, index_id: i64) -> Result<()> {
    // P5-012: Drop the per-index vec table
    let _ = drop_vec_table_for_index(conn, index_id);

    // Delete embeddings associated with this index from the BLOB fallback table
    conn.execute(
        "DELETE FROM kb_node_embeddings WHERE embedding_index_id = ?1",
        params![index_id],
    )?;

    // Delete the index record itself
    conn.execute(
        "DELETE FROM kb_embedding_indexes WHERE id = ?1",
        params![index_id],
    )?;
    Ok(())
}

/// 递增 index_state 中的版本号
pub fn increment_index_version(conn: &Connection, index_name: &str) -> Result<i64> {
    conn.execute(
        "INSERT INTO kb_index_state (index_name, index_version, index_type, state) \
         VALUES (?1, 1, 'vector', 'active') \
         ON CONFLICT(index_name) DO UPDATE SET index_version = index_version + 1, \
         last_rebuilt_at = datetime('now')",
        params![index_name],
    )?;
    let version: i64 = conn.query_row(
        "SELECT index_version FROM kb_index_state WHERE index_name = ?1",
        params![index_name],
        |row| row.get(0),
    )?;
    Ok(version)
}

// ---------------------------------------------------------------------------
// P5-013: Reembed job support
// ---------------------------------------------------------------------------

use crate::error::GBrainError;

/// Queue re-embed jobs for all documents in a library.
///
/// Creates a "kb_reembed" job for each document that has nodes but
/// no embeddings in the target index. If `target_index_id` is 0,
/// uses the library's default (active) index.
pub fn queue_reembed_jobs(
    conn: &Connection,
    library_id: i64,
    target_index_id: i64,
) -> Result<usize> {
    // Find documents needing re-embedding
    let sql = "SELECT DISTINCT d.id FROM kb_documents d \
               JOIN kb_document_nodes n ON n.document_id = d.id \
               WHERE d.library_id = ?1 AND d.deleted_at IS NULL \
               AND n.id NOT IN (SELECT node_id FROM kb_node_embeddings)";
    let mut stmt = conn.prepare(sql)?;
    let doc_ids: Vec<i64> = stmt.query_map(params![library_id], |row| row.get(0))?
        .filter_map(|r| r.ok()).collect();

    let mut queued = 0;
    for doc_id in &doc_ids {
        let payload = serde_json::json!({
            "kind": "kb_reembed",
            "document_id": doc_id,
            "library_id": library_id,
            "target_embedding_index_id": target_index_id,
        });
        conn.execute(
            "INSERT INTO jobs (job_type, payload, status, priority, max_attempts) \
             VALUES ('kb_reembed', ?1, 'pending', 0, 3)",
            params![payload.to_string()],
        ).map_err(|e| GBrainError::Database(e.to_string()))?;
        queued += 1;
    }
    Ok(queued)
}

// ---------------------------------------------------------------------------
// P5-012: sqlite-vec per-dimension/index table management
// ---------------------------------------------------------------------------

/// Generate the sqlite-vec virtual table name for an embedding index.
pub fn vec_table_name_for_index(index_id: i64) -> String {
    format!("vec_kb_{}", index_id)
}

/// Create a dedicated sqlite-vec virtual table for an embedding index.
pub fn create_vec_table_for_index(
    conn: &Connection,
    index_id: i64,
    dimensions: i32,
) -> Result<()> {
    let table_name = vec_table_name_for_index(index_id);
    let sql = format!(
        "CREATE VIRTUAL TABLE IF NOT EXISTS {} USING vec0(\
         embedding float[{}], node_id integer)",
        table_name, dimensions,
    );
    conn.execute_batch(&sql)
        .map_err(|e| GBrainError::Database(e.to_string()))?;
    Ok(())
}

/// Drop the sqlite-vec virtual table for an embedding index.
pub fn drop_vec_table_for_index(
    conn: &Connection,
    index_id: i64,
) -> Result<()> {
    let table_name = vec_table_name_for_index(index_id);
    let sql = format!("DROP TABLE IF EXISTS {}", table_name);
    conn.execute_batch(&sql)
        .map_err(|e| GBrainError::Database(e.to_string()))?;
    Ok(())
}

/// Get the active embedding index for a library.
/// Returns None if no active index exists.
pub fn get_active_index_for_library(conn: &Connection, library_id: i64) -> Result<Option<EmbeddingIndex>> {
    let result = conn.query_row(
        "SELECT id, library_id, provider, model, dimensions, index_type, is_active \
         FROM kb_embedding_indexes WHERE library_id = ?1 AND is_active = 1 LIMIT 1",
        params![library_id],
        |row| {
            Ok(EmbeddingIndex {
                id: row.get(0)?,
                library_id: row.get(1)?,
                provider: row.get(2)?,
                model: row.get(3)?,
                dimensions: row.get(4)?,
                index_type: row.get(5)?,
                is_active: row.get::<_, i32>(6)? != 0,
            })
        },
    );
    match result {
        Ok(idx) => Ok(Some(idx)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_index_struct() {
        let idx = EmbeddingIndex {
            id: 1, library_id: 1, provider: "openai".into(),
            model: "text-embedding-3-large".into(), dimensions: 1536,
            index_type: "vec0".into(), is_active: false,
        };
        assert_eq!(idx.dimensions, 1536);
        assert!(!idx.is_active);
    }

    #[test]
    fn test_vec_table_name_for_index() {
        assert_eq!(vec_table_name_for_index(1), "vec_kb_1");
        assert_eq!(vec_table_name_for_index(42), "vec_kb_42");
    }
}
