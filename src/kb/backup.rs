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

    // 导出文档节点（使用当前 schema 的实际列）
    let node_count = export_table_to_json(conn, output_dir, "nodes.json",
        "SELECT id, created_at, updated_at, library_id, document_id, content, \
                content_tokens, level, parent_id, chunk_order, \
                section_id, title_path, page_number, source_start, source_end, \
                node_metadata, embedding_text \
         FROM kb_document_nodes WHERE library_id=?1",
        params![library_id],
    )?;

    // 导出节点 embedding（BLOB 列通过 hex() 编码，避免 JSON null 丢失数据）
    export_table_to_json(conn, output_dir, "embeddings.json",
        "SELECT node_id, hex(embedding) as embedding_hex, dimensions, model, embedded_at, embedding_index_id \
         FROM kb_node_embeddings WHERE node_id IN \
         (SELECT id FROM kb_document_nodes WHERE library_id=?1)",
        params![library_id],
    )?;

    // 导出摘要（使用当前 schema 的实际列）
    export_table_to_json(conn, output_dir, "summaries.json",
        "SELECT id, created_at, document_id, section_id, summary_type, \
                summary_text, summary_tokens, model \
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

    // Import embedding indexes FIRST — 后面的 embedding 导入依赖 index_id 映射
    let mut index_id_map = import_embedding_indexes(conn, archive_dir, new_lib_id)?;

    // 若备份中没有 embedding index，为导入库创建默认 index 并激活
    // 避免之后新增文档/re-embed 时报 "没有 active embedding index"
    // 同时将 0 → default_idx 加入映射，让旧 embedding（无 index_id）能正确归属
    if index_id_map.is_empty() {
        let default_idx = crate::kb::embedding_index::create_embedding_index(
            conn, new_lib_id, "openai", "text-embedding-3-large", 1536, "vec0",
        )?;
        crate::kb::embedding_index::activate_index(conn, default_idx)?;
        index_id_map.insert(0, default_idx);
    }

    // Import documents — assign new IDs and remap library_id
    let doc_id_map = import_documents(conn, archive_dir, new_lib_id)?;

    // 导入节点 — 重映射 document_id 和 library_id，返回 old→new node_id 映射
    let node_id_map = import_nodes(conn, archive_dir, new_lib_id, &doc_id_map)?;

    // 导入 embedding — 使用 node_id_map + index_id_map 重映射
    import_embeddings(conn, archive_dir, &node_id_map, &index_id_map)?;

    // Import summaries — remap document_id
    import_summaries(conn, archive_dir, &doc_id_map)?;

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
                Ok(rusqlite::types::ValueRef::Text(s)) => {
                    // rusqlite 的 ValueRef::Text 返回 &[u8]，必须转为 UTF-8 字符串
                    // 否则 serde_json 会将其序列化为字节数组
                    std::str::from_utf8(s)
                        .map(|s| serde_json::json!(s))
                        .unwrap_or(serde_json::Value::Null)
                }
                Ok(rusqlite::types::ValueRef::Blob(_)) => serde_json::Value::Null,
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
) -> Result<std::collections::HashMap<i64, i64>> {
    let data = std::fs::read_to_string(archive_dir.join("nodes.json"))
        .map_err(|e| GBrainError::FileError(format!("cannot read nodes.json: {}", e)))?;
    let nodes: Vec<serde_json::Map<String, serde_json::Value>> = serde_json::from_str(&data)
        .map_err(|e| GBrainError::FileError(format!("cannot parse nodes.json: {}", e)))?;

    let mut node_id_map = std::collections::HashMap::new();
    for node in &nodes {
        let old_node_id = node.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
        let old_doc_id = node.get("document_id").and_then(|v| v.as_i64()).unwrap_or(0);
        let new_doc_id = doc_id_map.get(&old_doc_id).copied().unwrap_or(0);
        conn.execute(
            "INSERT INTO kb_document_nodes (library_id, document_id, content, content_tokens, \
             level, parent_id, chunk_order, section_id, title_path, page_number, \
             source_start, source_end, node_metadata, embedding_text) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            params![
                new_lib_id,
                new_doc_id,
                json_str(node, "content"),
                json_str(node, "content_tokens"),
                json_int(node, "level"),
                json_null_or_int(node, "parent_id"),
                json_int(node, "chunk_order"),
                json_null_or_int(node, "section_id"),
                json_str(node, "title_path"),
                json_null_or_int(node, "page_number"),
                json_null_or_int(node, "source_start"),
                json_null_or_int(node, "source_end"),
                json_str(node, "node_metadata"),
                json_str(node, "embedding_text"),
            ],
        )?;
        node_id_map.insert(old_node_id, conn.last_insert_rowid());
    }
    Ok(node_id_map)
}

fn import_embeddings(
    conn: &Connection,
    archive_dir: &Path,
    node_id_map: &std::collections::HashMap<i64, i64>,
    index_id_map: &std::collections::HashMap<i64, i64>,
) -> Result<()> {
    let path = archive_dir.join("embeddings.json");
    if !path.exists() { return Ok(()); }
    let data = std::fs::read_to_string(&path)
        .map_err(|e| GBrainError::FileError(format!("cannot read embeddings.json: {}", e)))?;
    let embeddings: Vec<serde_json::Map<String, serde_json::Value>> = serde_json::from_str(&data)
        .map_err(|e| GBrainError::FileError(format!("cannot parse embeddings.json: {}", e)))?;

    for emb in &embeddings {
        let old_node_id = emb.get("node_id").and_then(|v| v.as_i64()).unwrap_or(0);
        let new_node_id = match node_id_map.get(&old_node_id).copied() {
            Some(id) => id,
            None => return Err(GBrainError::FileError(
                format!("embedding 引用的 node_id={} 在导入的节点中不存在", old_node_id)
            )),
        };
        let old_index_id = emb.get("embedding_index_id").and_then(|v| v.as_i64()).unwrap_or(0);
        let new_index_id = if old_index_id > 0 {
            index_id_map.get(&old_index_id).copied().ok_or_else(|| {
                GBrainError::FileError(format!(
                    "embedding 引用的 embedding_index_id={} 在导入的索引中不存在", old_index_id
                ))
            })?
        } else {
            // 旧数据没有 index_id，使用默认 index（导入时自动创建或从备份恢复的第一个）
            index_id_map.values().next().copied().ok_or_else(|| {
                GBrainError::FileError(
                    "无法解析 embedding 的 index 归属：index_id_map 为空".into()
                )
            })?
        };

        let hex_str = emb.get("embedding_hex").and_then(|v| v.as_str()).unwrap_or("");
        if hex_str.is_empty() {
            return Err(GBrainError::FileError("embedding_hex 字段为空".into()));
        }
        let blob = hex::decode(hex_str).map_err(|e| {
            GBrainError::FileError(format!("embedding hex 解码失败: {}", e))
        })?;

        // 校验 blob 长度必须是 4 的倍数（每个 f32 占 4 字节）
        if blob.len() % 4 != 0 {
            return Err(GBrainError::FileError(format!(
                "embedding blob 长度 {} 不是 4 的倍数，数据可能损坏", blob.len()
            )));
        }

        // blob → f32 向量（用于统一写入函数）
        let embedding_vec: Vec<f32> = blob
            .chunks_exact(4)
            .map(|chunk| {
                let bytes: [u8; 4] = chunk.try_into()
                    .expect("chunks_exact(4) 保证 4 字节");
                f32::from_le_bytes(bytes)
            })
            .collect();
        if embedding_vec.is_empty() {
            return Err(GBrainError::FileError("embedding blob 解码后为空".into()));
        }

        // 校验备份中的 dimensions 与实际解码长度一致
        let backup_dims = emb.get("dimensions").and_then(|v| v.as_i64()).unwrap_or(0) as usize;
        if backup_dims > 0 && backup_dims != embedding_vec.len() {
            return Err(GBrainError::FileError(format!(
                "embedding 维度不一致：备份声明 dimensions={}，实际解码长度={}",
                backup_dims, embedding_vec.len()
            )));
        }

        // 校验目标 index 的维度与向量维度一致
        let index_dims: i32 = conn.query_row(
            "SELECT dimensions FROM kb_embedding_indexes WHERE id = ?1",
            rusqlite::params![new_index_id],
            |row| row.get(0),
        ).map_err(|_| GBrainError::FileError(format!(
            "目标 embedding index {} 不存在", new_index_id
        )))?;
        if index_dims as usize != embedding_vec.len() {
            return Err(GBrainError::FileError(format!(
                "embedding 维度与目标 index 不匹配：向量={}，index {} dimensions={}",
                embedding_vec.len(), new_index_id, index_dims
            )));
        }

        let dimensions = embedding_vec.len() as i32;
        let model = emb.get("model").and_then(|v| v.as_str()).unwrap_or("text-embedding-3-large");

        // 使用统一函数写入：BLOB 表 + per-index vec 表同步更新
        crate::kb::embedding_index::upsert_node_embedding_for_index(
            conn, new_node_id, new_index_id, &embedding_vec, dimensions, model,
        )?;
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
            "INSERT INTO kb_document_summaries (document_id, section_id, summary_type, \
             summary_text, summary_tokens, model) \
             VALUES (?1,?2,?3,?4,?5,?6)",
            params![
                new_doc_id,
                json_null_or_int(summary, "section_id"),
                json_str(summary, "summary_type"),
                json_str(summary, "summary_text"),
                json_str(summary, "summary_tokens"),
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
) -> Result<std::collections::HashMap<i64, i64>> {
    let path = archive_dir.join("embedding_indexes.json");
    if !path.exists() { return Ok(std::collections::HashMap::new()); }
    let data = std::fs::read_to_string(&path)
        .map_err(|e| GBrainError::FileError(format!("cannot read embedding_indexes.json: {}", e)))?;
    let indexes: Vec<crate::kb::embedding_index::EmbeddingIndex> = serde_json::from_str(&data)
        .map_err(|e| GBrainError::FileError(format!("cannot parse embedding_indexes.json: {}", e)))?;

    // 记住备份中哪个 index 是 active
    let old_active_id = indexes.iter().find(|i| i.is_active).map(|i| i.id);

    let mut id_map = std::collections::HashMap::new();
    for idx in &indexes {
        let old_id = idx.id;
        let new_id = crate::kb::embedding_index::create_embedding_index(
            conn, new_lib_id, &idx.provider, &idx.model, idx.dimensions, &idx.index_type,
        )?;
        id_map.insert(old_id, new_id);
    }

    // 恢复 active 状态：优先使用备份中的 active index，否则激活第一个
    if let Some(old_id) = old_active_id {
        if let Some(&new_id) = id_map.get(&old_id) {
            crate::kb::embedding_index::activate_index(conn, new_id)?;
        }
    } else if let Some(&first_id) = id_map.values().next() {
        crate::kb::embedding_index::activate_index(conn, first_id)?;
    }

    Ok(id_map)
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
