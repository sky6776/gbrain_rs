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

/// 创建 embedding index 记录
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
    Ok(conn.last_insert_rowid())
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
}
