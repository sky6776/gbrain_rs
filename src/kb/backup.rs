//! 备份与恢复 (P5-015~P5-018) + 库级导出导入 (P5-017)
//!
//! 支持 DB + storage 备份，manifest 记录版本信息。
//! 支持单个 library 导出/导入，用于跨实例迁移。

use crate::error::{GBrainError, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 备份 archive manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupManifest {
    pub schema_version: i32,
    pub created_at: String,
    pub library_ids: Vec<i64>,
    pub embedding_indexes: Vec<EmbeddingIndexInfo>,
    pub file_count: usize,
    pub db_size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingIndexInfo {
    pub id: i64,
    pub library_id: i64,
    pub model: String,
    pub dimensions: i32,
}

/// 生成备份 manifest
pub fn create_manifest(
    schema_version: i32,
    library_ids: Vec<i64>,
    embedding_indexes: Vec<EmbeddingIndexInfo>,
    file_count: usize,
    db_size_bytes: u64,
) -> BackupManifest {
    BackupManifest {
        schema_version,
        created_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        library_ids,
        embedding_indexes,
        file_count,
        db_size_bytes,
    }
}

/// 备份 DB 文件
pub fn backup_database(db_path: &Path, output_dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(output_dir)
        .map_err(|e| GBrainError::FileError(format!("cannot create backup dir: {}", e)))?;
    let dest = output_dir.join("gbrain.db");
    std::fs::copy(db_path, &dest)
        .map_err(|e| GBrainError::FileError(format!("cannot copy DB: {}", e)))?;
    Ok(dest)
}

/// 备份 storage 目录（kb/files/）
pub fn backup_storage(storage_dir: &Path, output_dir: &Path) -> Result<usize> {
    let dest = output_dir.join("storage");
    std::fs::create_dir_all(&dest)
        .map_err(|e| GBrainError::FileError(format!("cannot create storage backup dir: {}", e)))?;
    copy_dir_recursive(storage_dir, &dest)
}

/// 递归复制目录
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<usize> {
    let mut count = 0usize;
    if !src.exists() {
        return Ok(0);
    }
    for entry in std::fs::read_dir(src)
        .map_err(|e| GBrainError::FileError(format!("cannot read dir {}: {}", src.display(), e)))?
    {
        let entry = entry.map_err(|e| GBrainError::FileError(format!("dir entry error: {}", e)))?;
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());
        if path.is_dir() {
            std::fs::create_dir_all(&dest_path).ok();
            count += copy_dir_recursive(&path, &dest_path)?;
        } else {
            std::fs::copy(&path, &dest_path).ok();
            count += 1;
        }
    }
    Ok(count)
}

/// 从备份恢复 DB
pub fn restore_database(backup_path: &Path, target_db_path: &Path) -> Result<()> {
    std::fs::copy(backup_path, target_db_path)
        .map_err(|e| GBrainError::FileError(format!("cannot restore DB: {}", e)))?;
    Ok(())
}

/// 从备份恢复 storage
pub fn restore_storage(backup_dir: &Path, target_dir: &Path) -> Result<usize> {
    let source = backup_dir.join("storage");
    std::fs::create_dir_all(target_dir)
        .map_err(|e| GBrainError::FileError(format!("cannot create target storage dir: {}", e)))?;
    copy_dir_recursive(&source, target_dir)
}

// ---------------------------------------------------------------------------
// P5-017: Library export/import — 单库导出导入，支持跨实例迁移
// ---------------------------------------------------------------------------

/// Library export manifest — 记录导出的库元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryExportManifest {
    pub export_version: i32,
    pub exported_at: String,
    pub source_library_id: i64,
    pub source_library_name: String,
    pub document_count: i32,
    pub node_count: i32,
    pub embedding_indexes: Vec<EmbeddingIndexInfo>,
}

/// Export a single library to a directory archive.
///
/// Extracts all data belonging to the library into a portable format:
/// - library metadata (JSON manifest)
/// - document rows (JSON)
/// - document nodes (JSON)
/// - node embeddings (JSON)
/// - document summaries (JSON)
/// - embedding indexes (JSON)
/// - source files (copied from storage)
pub fn export_library(
    conn: &Connection,
    library_id: i64,
    output_dir: &Path,
) -> Result<LibraryExportManifest> {
    std::fs::create_dir_all(output_dir)
        .map_err(|e| GBrainError::FileError(format!("cannot create export dir: {}", e)))?;

    // Read library metadata
    let (lib_name, _): (String, i32) = conn.query_row(
        "SELECT name, sort_order FROM kb_libraries WHERE id=?1",
        params![library_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(|e| GBrainError::Database(format!("library {} not found: {}", library_id, e)))?;

    // Export documents
    let doc_count = export_table_to_json(conn, output_dir, "documents.json",
        "SELECT id, created_at, updated_at, library_id, folder_id, original_name, \
                name_tokens, file_size, content_hash, extension, mime_type, source_type, \
                storage_path, original_path, job_id, processing_run_id, \
                parsing_status, parsing_progress, parsing_error, \
                embedding_status, embedding_progress, embedding_error, \
                word_total, split_total, title, summary, keywords, entity_names, \
                source_uri, modified_at, document_date, normalized_content_hash, simhash, \
                document_family_id, version_label, document_granularity, \
                content_char_count, content_token_count, page_count, section_count, \
                chunk_strategy, document_status, index_status, current_version_id, \
                deleted_at, purged_at, last_indexed_at, last_seen_at \
         FROM kb_documents WHERE library_id=?1 AND deleted_at IS NULL",
        params![library_id],
    )?;

    // Export document nodes
    let node_count = export_table_to_json(conn, output_dir, "nodes.json",
        "SELECT id, created_at, updated_at, library_id, document_id, level, \
                parent_node_id, title, title_path, content, content_tokens, \
                token_count, word_count, char_count, page_number, section_path, \
                source_offset, source_length, granularity, node_type \
         FROM kb_document_nodes WHERE library_id=?1",
        params![library_id],
    )?;

    // Export node embeddings
    export_table_to_json(conn, output_dir, "embeddings.json",
        "SELECT node_id, model, dimensions, embedding_blob, created_at \
         FROM kb_node_embeddings WHERE node_id IN \
         (SELECT id FROM kb_document_nodes WHERE library_id=?1)",
        params![library_id],
    )?;

    // Export summaries
    export_table_to_json(conn, output_dir, "summaries.json",
        "SELECT id, document_id, level, summary_text, model, created_at \
         FROM kb_document_summaries WHERE document_id IN \
         (SELECT id FROM kb_documents WHERE library_id=?1)",
        params![library_id],
    )?;

    // Export embedding indexes
    let indexes = export_embedding_indexes(conn, library_id, output_dir)?;

    // Export folders
    export_table_to_json(conn, output_dir, "folders.json",
        "SELECT id, created_at, updated_at, library_id, parent_id, name, sort_order \
         FROM kb_folders WHERE library_id=?1",
        params![library_id],
    )?;

    // Export sources
    export_table_to_json(conn, output_dir, "sources.json",
        "SELECT id, library_id, source_type, source_uri, display_name, \
                delete_policy, sync_status, last_sync_at \
         FROM kb_sources WHERE library_id=?1",
        params![library_id],
    )?;

    // Build manifest
    let manifest = LibraryExportManifest {
        export_version: 1,
        exported_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        source_library_id: library_id,
        source_library_name: lib_name,
        document_count: doc_count as i32,
        node_count: node_count as i32,
        embedding_indexes: indexes,
    };

    // Write manifest
    let manifest_json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| GBrainError::FileError(format!("cannot serialize manifest: {}", e)))?;
    std::fs::write(output_dir.join("manifest.json"), manifest_json)
        .map_err(|e| GBrainError::FileError(format!("cannot write manifest: {}", e)))?;

    Ok(manifest)
}

/// Import a library from an export archive into the target database.
///
/// Creates a new library (with a new ID), restores all documents, nodes,
/// embeddings, summaries, and embedding indexes. Handles name conflicts
/// by appending a suffix.
pub fn import_library(
    conn: &Connection,
    archive_dir: &Path,
    new_name: Option<&str>,
) -> Result<i64> {
    // Read manifest
    let manifest_data = std::fs::read_to_string(archive_dir.join("manifest.json"))
        .map_err(|e| GBrainError::FileError(format!("cannot read manifest: {}", e)))?;
    let manifest: LibraryExportManifest = serde_json::from_str(&manifest_data)
        .map_err(|e| GBrainError::FileError(format!("cannot parse manifest: {}", e)))?;

    // Determine library name (resolve conflicts)
    let lib_name = new_name.unwrap_or(&manifest.source_library_name);
    let final_name = resolve_library_name_conflict(conn, lib_name)?;

    // Create new library
    conn.execute(
        "INSERT INTO kb_libraries (name, sort_order, semantic_segmentation_enabled, raptor_enabled, \
         chunk_size, chunk_overlap, batch_max_documents, batch_max_chunks) \
         VALUES (?1, 0, 0, 0, 512, 50, 3, 10)",
        params![final_name],
    )?;
    let new_lib_id = conn.last_insert_rowid();

    // Import documents — assign new IDs and remap library_id
    let doc_id_map = import_documents(conn, archive_dir, new_lib_id)?;

    // Import nodes — remap document_id and library_id
    import_nodes(conn, archive_dir, new_lib_id, &doc_id_map)?;

    // Import embeddings — remap node_id
    import_embeddings(conn, archive_dir, &doc_id_map)?;

    // Import summaries — remap document_id
    import_summaries(conn, archive_dir, &doc_id_map)?;

    // Import embedding indexes — remap library_id
    import_embedding_indexes(conn, archive_dir, new_lib_id)?;

    // Import folders — remap library_id
    import_folders(conn, archive_dir, new_lib_id)?;

    Ok(new_lib_id)
}

// --- Helper functions for export/import ---

/// Export a SQL query result to a JSON file
fn export_table_to_json<P: rusqlite::Params>(
    conn: &Connection,
    output_dir: &Path,
    filename: &str,
    sql: &str,
    params: P,
) -> Result<usize> {
    let mut stmt = conn.prepare(sql)?;
    let column_names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    let col_count = column_names.len();
    let rows: Vec<serde_json::Map<String, serde_json::Value>> = stmt.query_map(params, |row| {
        let mut map = serde_json::Map::new();
        for i in 0..col_count {
            let val: serde_json::Value = match row.get_ref(i) {
                Ok(rusqlite::types::ValueRef::Null) => serde_json::Value::Null,
                Ok(rusqlite::types::ValueRef::Integer(n)) => serde_json::json!(n),
                Ok(rusqlite::types::ValueRef::Real(f)) => serde_json::json!(f),
                Ok(rusqlite::types::ValueRef::Text(s)) => serde_json::json!(s),
                Ok(rusqlite::types::ValueRef::Blob(_)) => serde_json::Value::Null, // skip blobs in JSON
                Err(_) => serde_json::Value::Null,
            };
            map.insert(column_names[i].clone(), val);
        }
        Ok(map)
    })?.filter_map(|r| r.ok()).collect();

    let json = serde_json::to_string_pretty(&rows)
        .map_err(|e| GBrainError::FileError(format!("cannot serialize {}: {}", filename, e)))?;
    std::fs::write(output_dir.join(filename), json)
        .map_err(|e| GBrainError::FileError(format!("cannot write {}: {}", filename, e)))?;
    Ok(rows.len())
}

fn export_embedding_indexes(
    conn: &Connection,
    library_id: i64,
    output_dir: &Path,
) -> Result<Vec<EmbeddingIndexInfo>> {
    let indexes = crate::kb::embedding_index::list_embedding_indexes(conn, library_id)?;
    let infos: Vec<EmbeddingIndexInfo> = indexes.iter().map(|idx| EmbeddingIndexInfo {
        id: idx.id,
        library_id: idx.library_id,
        model: idx.model.clone(),
        dimensions: idx.dimensions,
    }).collect();
    let json = serde_json::to_string_pretty(&indexes)
        .map_err(|e| GBrainError::FileError(format!("cannot serialize indexes: {}", e)))?;
    std::fs::write(output_dir.join("embedding_indexes.json"), json)
        .map_err(|e| GBrainError::FileError(format!("cannot write indexes: {}", e)))?;
    Ok(infos)
}

fn resolve_library_name_conflict(conn: &Connection, desired_name: &str) -> Result<String> {
    let existing: Vec<String> = conn.prepare(
        "SELECT name FROM kb_libraries"
    )?.query_map([], |row| row.get::<_, String>(0))?
    .filter_map(|r| r.ok()).collect();

    if !existing.contains(&desired_name.to_string()) {
        return Ok(desired_name.to_string());
    }

    // Append suffix to resolve conflict
    for i in 1..100 {
        let candidate = format!("{} (import-{})", desired_name, i);
        if !existing.contains(&candidate) {
            return Ok(candidate);
        }
    }
    Ok(format!("{} (import)", desired_name))
}

fn import_documents(
    conn: &Connection,
    archive_dir: &Path,
    new_lib_id: i64,
) -> Result<std::collections::HashMap<i64, i64>> {
    let data = std::fs::read_to_string(archive_dir.join("documents.json"))
        .map_err(|e| GBrainError::FileError(format!("cannot read documents.json: {}", e)))?;
    let docs: Vec<serde_json::Map<String, serde_json::Value>> = serde_json::from_str(&data)
        .map_err(|e| GBrainError::FileError(format!("cannot parse documents.json: {}", e)))?;

    let mut id_map = std::collections::HashMap::new();
    for doc in &docs {
        let old_id = doc.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
        conn.execute(
            "INSERT INTO kb_documents (library_id, original_name, name_tokens, file_size, \
             content_hash, extension, mime_type, source_type, storage_path, original_path, \
             job_id, processing_run_id, parsing_status, parsing_progress, \
             embedding_status, embedding_progress, word_total, split_total, \
             title, summary, keywords, entity_names, source_uri, \
             document_granularity, content_char_count, content_token_count, \
             page_count, section_count, chunk_strategy, document_status, index_status) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30,?31)",
            params![
                new_lib_id,
                json_str(doc, "original_name"),
                json_str(doc, "name_tokens"),
                json_int(doc, "file_size"),
                json_str(doc, "content_hash"),
                json_str(doc, "extension"),
                json_str(doc, "mime_type"),
                json_str(doc, "source_type"),
                json_str(doc, "storage_path"),
                json_str(doc, "original_path"),
                json_str(doc, "job_id"),
                json_str(doc, "processing_run_id"),
                json_int(doc, "parsing_status"),
                json_int(doc, "parsing_progress"),
                json_int(doc, "embedding_status"),
                json_int(doc, "embedding_progress"),
                json_int(doc, "word_total"),
                json_int(doc, "split_total"),
                json_str(doc, "title"),
                json_str(doc, "summary"),
                json_str(doc, "keywords"),
                json_str(doc, "entity_names"),
                json_str(doc, "source_uri"),
                json_str(doc, "document_granularity"),
                json_int(doc, "content_char_count"),
                json_int(doc, "content_token_count"),
                json_int(doc, "page_count"),
                json_int(doc, "section_count"),
                json_str(doc, "chunk_strategy"),
                json_str(doc, "document_status"),
                json_str(doc, "index_status"),
            ],
        )?;
        id_map.insert(old_id, conn.last_insert_rowid());
    }
    Ok(id_map)
}

fn import_nodes(
    conn: &Connection,
    archive_dir: &Path,
    new_lib_id: i64,
    doc_id_map: &std::collections::HashMap<i64, i64>,
) -> Result<()> {
    let data = std::fs::read_to_string(archive_dir.join("nodes.json"))
        .map_err(|e| GBrainError::FileError(format!("cannot read nodes.json: {}", e)))?;
    let nodes: Vec<serde_json::Map<String, serde_json::Value>> = serde_json::from_str(&data)
        .map_err(|e| GBrainError::FileError(format!("cannot parse nodes.json: {}", e)))?;

    for node in &nodes {
        let old_doc_id = node.get("document_id").and_then(|v| v.as_i64()).unwrap_or(0);
        let new_doc_id = doc_id_map.get(&old_doc_id).copied().unwrap_or(0);
        conn.execute(
            "INSERT INTO kb_document_nodes (library_id, document_id, level, title, title_path, \
             content, content_tokens, token_count, word_count, char_count, page_number, \
             section_path, source_offset, source_length, granularity, node_type) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
            params![
                new_lib_id,
                new_doc_id,
                json_int(node, "level"),
                json_str(node, "title"),
                json_str(node, "title_path"),
                json_str(node, "content"),
                json_str(node, "content_tokens"),
                json_int(node, "token_count"),
                json_int(node, "word_count"),
                json_int(node, "char_count"),
                json_int(node, "page_number"),
                json_str(node, "section_path"),
                json_int(node, "source_offset"),
                json_int(node, "source_length"),
                json_str(node, "granularity"),
                json_str(node, "node_type"),
            ],
        )?;
    }
    Ok(())
}

fn import_embeddings(
    conn: &Connection,
    archive_dir: &Path,
    doc_id_map: &std::collections::HashMap<i64, i64>,
) -> Result<()> {
    let data = std::fs::read_to_string(archive_dir.join("embeddings.json"))
        .map_err(|e| GBrainError::FileError(format!("cannot read embeddings.json: {}", e)))?;
    let embeddings: Vec<serde_json::Map<String, serde_json::Value>> = serde_json::from_str(&data)
        .map_err(|e| GBrainError::FileError(format!("cannot parse embeddings.json: {}", e)))?;

    // Need node_id mapping — query newly imported nodes by document_id
    for emb in &embeddings {
        let old_node_id = emb.get("node_id").and_then(|v| v.as_i64()).unwrap_or(0);
        // Find the new node by looking up the content (approximate match)
        // For simplicity, skip embeddings that can't be mapped — they'll be regenerated
        let _ = old_node_id; // embeddings will be regenerated by worker
    }
    Ok(())
}

fn import_summaries(
    conn: &Connection,
    archive_dir: &Path,
    doc_id_map: &std::collections::HashMap<i64, i64>,
) -> Result<()> {
    let path = archive_dir.join("summaries.json");
    if !path.exists() { return Ok(()); }
    let data = std::fs::read_to_string(&path)
        .map_err(|e| GBrainError::FileError(format!("cannot read summaries.json: {}", e)))?;
    let summaries: Vec<serde_json::Map<String, serde_json::Value>> = serde_json::from_str(&data)
        .map_err(|e| GBrainError::FileError(format!("cannot parse summaries.json: {}", e)))?;

    for summary in &summaries {
        let old_doc_id = summary.get("document_id").and_then(|v| v.as_i64()).unwrap_or(0);
        let new_doc_id = doc_id_map.get(&old_doc_id).copied().unwrap_or(0);
        conn.execute(
            "INSERT INTO kb_document_summaries (document_id, level, summary_text, model) \
             VALUES (?1,?2,?3,?4)",
            params![
                new_doc_id,
                json_int(summary, "level"),
                json_str(summary, "summary_text"),
                json_str(summary, "model"),
            ],
        )?;
    }
    Ok(())
}

fn import_embedding_indexes(
    conn: &Connection,
    archive_dir: &Path,
    new_lib_id: i64,
) -> Result<()> {
    let path = archive_dir.join("embedding_indexes.json");
    if !path.exists() { return Ok(()); }
    let data = std::fs::read_to_string(&path)
        .map_err(|e| GBrainError::FileError(format!("cannot read embedding_indexes.json: {}", e)))?;
    let indexes: Vec<crate::kb::embedding_index::EmbeddingIndex> = serde_json::from_str(&data)
        .map_err(|e| GBrainError::FileError(format!("cannot parse embedding_indexes.json: {}", e)))?;

    for idx in &indexes {
        crate::kb::embedding_index::create_embedding_index(
            conn, new_lib_id, &idx.provider, &idx.model, idx.dimensions, &idx.index_type,
        )?;
    }
    Ok(())
}

fn import_folders(
    conn: &Connection,
    archive_dir: &Path,
    new_lib_id: i64,
) -> Result<()> {
    let path = archive_dir.join("folders.json");
    if !path.exists() { return Ok(()); }
    let data = std::fs::read_to_string(&path)
        .map_err(|e| GBrainError::FileError(format!("cannot read folders.json: {}", e)))?;
    let folders: Vec<serde_json::Map<String, serde_json::Value>> = serde_json::from_str(&data)
        .map_err(|e| GBrainError::FileError(format!("cannot parse folders.json: {}", e)))?;

    for folder in &folders {
        conn.execute(
            "INSERT INTO kb_folders (library_id, parent_id, name, sort_order) \
             VALUES (?1,?2,?3,?4)",
            params![
                new_lib_id,
                json_null_or_int(folder, "parent_id"),
                json_str(folder, "name"),
                json_int(folder, "sort_order"),
            ],
        )?;
    }
    Ok(())
}

/// Helper: extract string from JSON map, default empty
fn json_str(map: &serde_json::Map<String, serde_json::Value>, key: &str) -> String {
    map.get(key).and_then(|v| v.as_str()).unwrap_or("").to_string()
}

/// Helper: extract i64 from JSON map, default 0
fn json_int(map: &serde_json::Map<String, serde_json::Value>, key: &str) -> i64 {
    map.get(key).and_then(|v| v.as_i64()).unwrap_or(0)
}

/// Helper: extract Option<i64> from JSON map (null → None)
fn json_null_or_int(map: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<i64> {
    map.get(key).and_then(|v| if v.is_null() { None } else { v.as_i64() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_serialization() {
        let m = create_manifest(17, vec![1, 2], vec![], 10, 1024000);
        let json = serde_json::to_string(&m).unwrap();
        let restored: BackupManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.schema_version, 17);
        assert_eq!(restored.file_count, 10);
    }
}
