//! KB Engine — database CRUD operations for the KB subsystem
//!
//! Shares the same SQLite connection as SqliteEngine.

use crate::error::{GBrainError, Result};
use crate::kb::types::*;
use rusqlite::{params, Connection};

pub struct KbEngine<'a> {
    conn: &'a Connection,
}

impl<'a> KbEngine<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Execute in a transaction (RAII: auto-rollback on Drop if commit not called)
    pub fn transaction<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let tx = self.conn.unchecked_transaction()?;
        let result = f(self.conn);
        match &result {
            Ok(_) => tx.commit()?,
            Err(_) => {
                let _ = tx.rollback();
            }
        }
        result
    }

    /// Read-only query
    pub fn query<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        f(self.conn)
    }

    // --- Library CRUD ---

    pub fn list_libraries(&self) -> Result<Vec<Library>> {
        self.query(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, created_at, updated_at, name, \
                        semantic_segmentation_enabled, raptor_enabled, \
                        raptor_llm_base_url, raptor_llm_secret_ref, raptor_llm_model, \
                        chunk_size, chunk_overlap, batch_max_documents, batch_max_chunks, \
                        sort_order, \
                        embedding_provider, embedding_model, embedding_dimensions, \
                        search_profile, rerank_enabled, rerank_provider, summary_enabled, \
                        external_embedding_allowed, external_rerank_allowed, \
                        external_summary_allowed, external_ocr_allowed, redaction_enabled \
                 FROM kb_libraries ORDER BY sort_order DESC, id DESC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(Library {
                    id: row.get(0)?,
                    created_at: row.get(1)?,
                    updated_at: row.get(2)?,
                    name: row.get(3)?,
                    semantic_segmentation_enabled: row.get::<_, i32>(4)? != 0,
                    raptor_enabled: row.get::<_, i32>(5)? != 0,
                    raptor_llm_base_url: row.get(6)?,
                    raptor_llm_secret_ref: row.get(7)?,
                    raptor_llm_model: row.get(8)?,
                    chunk_size: row.get::<_, i32>(9)? as usize,
                    chunk_overlap: row.get::<_, i32>(10)? as usize,
                    batch_max_documents: row.get::<_, i32>(11)? as usize,
                    batch_max_chunks: row.get::<_, i32>(12)? as usize,
                    sort_order: row.get(13)?,
                    // P0-016: governance fields 从数据库读取
                    embedding_provider: row.get(14)?,
                    embedding_model: row.get(15)?,
                    embedding_dimensions: row.get(16)?,
                    search_profile: row.get(17)?,
                    rerank_enabled: row.get::<_, i32>(18)? != 0,
                    rerank_provider: row.get(19)?,
                    summary_enabled: row.get::<_, i32>(20)? != 0,
                    external_embedding_allowed: row.get::<_, i32>(21)? != 0,
                    external_rerank_allowed: row.get::<_, i32>(22)? != 0,
                    external_summary_allowed: row.get::<_, i32>(23)? != 0,
                    external_ocr_allowed: row.get::<_, i32>(24)? != 0,
                    redaction_enabled: row.get::<_, i32>(25)? != 0,
                })
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| GBrainError::Database(e.to_string()))
        })
    }

    /// List libraries with document_count and chunk_count stats.
    pub fn list_libraries_with_stats(&self) -> Result<Vec<LibraryListItem>> {
        self.query(|conn| {
            let mut stmt = conn.prepare(
                "SELECT l.id, l.name, l.sort_order, l.raptor_enabled, \
                        l.semantic_segmentation_enabled, l.raptor_llm_secret_ref, \
                        COALESCE(d.doc_count, 0), COALESCE(n.chunk_count, 0) \
                 FROM kb_libraries l \
                 LEFT JOIN (SELECT library_id, COUNT(*) as doc_count FROM kb_documents GROUP BY library_id) d \
                    ON l.id = d.library_id \
                 LEFT JOIN (SELECT library_id, COUNT(*) as chunk_count FROM kb_document_nodes GROUP BY library_id) n \
                    ON l.id = n.library_id \
                 ORDER BY l.sort_order DESC, l.id DESC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(LibraryListItem {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    document_count: row.get(6)?,
                    chunk_count: row.get(7)?,
                    sort_order: row.get(2)?,
                    raptor_enabled: row.get::<_, i32>(3)? != 0,
                    semantic_segmentation_enabled: row.get::<_, i32>(4)? != 0,
                    has_raptor_secret: !row.get::<_, String>(5)?.is_empty(),
                })
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| GBrainError::Database(e.to_string()))
        })
    }

    pub fn create_library(&self, input: &CreateLibraryInput) -> Result<i64> {
        self.transaction(|conn| {
            let chunk_size = input.chunk_size.unwrap_or(512).clamp(200, 5000) as i32;
            let chunk_overlap = input.chunk_overlap.unwrap_or(50).clamp(0, 1000) as i32;
            let batch_max_docs = input.batch_max_documents.unwrap_or(3).clamp(1, 5) as i32;
            let batch_max_chunks = input.batch_max_chunks.unwrap_or(10).clamp(1, 20) as i32;
            let raptor_enabled = input.raptor_enabled.unwrap_or(false) as i32;
            let semantic = input.semantic_segmentation_enabled.unwrap_or(false) as i32;

            // Get next sort_order from MAX+1
            let max_sort: i32 = conn
                .query_row(
                    "SELECT COALESCE(MAX(sort_order), 0) FROM kb_libraries",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            conn.execute(
                "INSERT INTO kb_libraries \
                 (name, semantic_segmentation_enabled, raptor_enabled, \
                  raptor_llm_base_url, raptor_llm_secret_ref, raptor_llm_model, \
                  chunk_size, chunk_overlap, batch_max_documents, batch_max_chunks, sort_order) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    input.name,
                    semantic,
                    raptor_enabled,
                    input.raptor_llm_base_url.as_deref().unwrap_or(""),
                    input.raptor_llm_secret_ref.as_deref().unwrap_or(""),
                    input.raptor_llm_model.as_deref().unwrap_or(""),
                    chunk_size,
                    chunk_overlap,
                    batch_max_docs,
                    batch_max_chunks,
                    max_sort + 1,
                ],
            )?;
            let lib_id = conn.last_insert_rowid();

            // 自动创建默认 embedding index 并设为 active
            let dims = input.embedding_dimensions.unwrap_or(1536);
            let provider = input.embedding_provider.as_deref().unwrap_or("openai");
            let model = input
                .embedding_model
                .as_deref()
                .unwrap_or("text-embedding-3-large");
            let index_id = crate::kb::embedding_index::create_embedding_index(
                conn, lib_id, provider, model, dims, "vec0",
            )?;
            crate::kb::embedding_index::activate_index(conn, index_id)?;

            Ok(lib_id)
        })
    }

    pub fn get_library(&self, id: i64) -> Result<Library> {
        self.query(|conn| {
            conn.query_row(
                "SELECT id, created_at, updated_at, name, \
                        semantic_segmentation_enabled, raptor_enabled, \
                        raptor_llm_base_url, raptor_llm_secret_ref, raptor_llm_model, \
                        chunk_size, chunk_overlap, batch_max_documents, batch_max_chunks, \
                        sort_order \
                 FROM kb_libraries WHERE id = ?1",
                [id],
                |row| {
                    Ok(Library {
                        id: row.get(0)?,
                        created_at: row.get(1)?,
                        updated_at: row.get(2)?,
                        name: row.get(3)?,
                        semantic_segmentation_enabled: row.get::<_, i32>(4)? != 0,
                        raptor_enabled: row.get::<_, i32>(5)? != 0,
                        raptor_llm_base_url: row.get(6)?,
                        raptor_llm_secret_ref: row.get(7)?,
                        raptor_llm_model: row.get(8)?,
                        chunk_size: row.get::<_, i32>(9)? as usize,
                        chunk_overlap: row.get::<_, i32>(10)? as usize,
                        batch_max_documents: row.get::<_, i32>(11)? as usize,
                        batch_max_chunks: row.get::<_, i32>(12)? as usize,
                        sort_order: row.get(13)?,
                        // P0-016: new governance fields with defaults
                        embedding_provider: String::new(),
                        embedding_model: String::new(),
                        embedding_dimensions: None,
                        search_profile: String::new(),
                        rerank_enabled: true,
                        rerank_provider: String::new(),
                        summary_enabled: false,
                        external_embedding_allowed: true,
                        external_rerank_allowed: true,
                        external_summary_allowed: true,
                        external_ocr_allowed: true,
                        redaction_enabled: false,
                    })
                },
            )
            .map_err(|e| GBrainError::Database(format!("Library not found: {}", e)))
        })
    }

    pub fn update_library(&self, id: i64, input: &UpdateLibraryInput) -> Result<()> {
        self.transaction(|conn| {
            let mut sets = Vec::new();
            let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

            if let Some(ref name) = input.name {
                sets.push("name = ?".to_string());
                param_values.push(Box::new(name.clone()));
            }
            if let Some(semantic) = input.semantic_segmentation_enabled {
                sets.push("semantic_segmentation_enabled = ?".to_string());
                param_values.push(Box::new(semantic as i32));
            }
            if let Some(raptor) = input.raptor_enabled {
                sets.push("raptor_enabled = ?".to_string());
                param_values.push(Box::new(raptor as i32));
            }
            if let Some(ref url) = input.raptor_llm_base_url {
                sets.push("raptor_llm_base_url = ?".to_string());
                param_values.push(Box::new(url.clone()));
            }
            if let Some(ref secret) = input.raptor_llm_secret_ref {
                sets.push("raptor_llm_secret_ref = ?".to_string());
                param_values.push(Box::new(secret.clone()));
            }
            if let Some(ref model) = input.raptor_llm_model {
                sets.push("raptor_llm_model = ?".to_string());
                param_values.push(Box::new(model.clone()));
            }
            if let Some(chunk_size) = input.chunk_size {
                sets.push("chunk_size = ?".to_string());
                param_values.push(Box::new(chunk_size.clamp(200, 5000) as i32));
            }
            if let Some(chunk_overlap) = input.chunk_overlap {
                sets.push("chunk_overlap = ?".to_string());
                param_values.push(Box::new(chunk_overlap.clamp(0, 1000) as i32));
            }

            if sets.is_empty() {
                return Ok(());
            }

            sets.push("updated_at = datetime('now')".to_string());
            param_values.push(Box::new(id));

            let sql = format!("UPDATE kb_libraries SET {} WHERE id = ?", sets.join(", "));
            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                param_values.iter().map(|p| p.as_ref()).collect();

            conn.execute(&sql, param_refs.as_slice())?;
            Ok(())
        })
    }

    pub fn delete_library(&self, id: i64) -> Result<()> {
        self.transaction(|conn| {
            // Get node IDs for vector cleanup
            let node_ids: Vec<i64> = {
                let mut stmt =
                    conn.prepare("SELECT id FROM kb_document_nodes WHERE library_id = ?1")?;
                let rows = stmt.query_map([id], |row| row.get(0))?;
                rows.filter_map(|r| r.ok()).collect()
            };

            // Delete vectors for specific node IDs (no FK cascade on vec_kb_nodes)
            if !node_ids.is_empty() {
                for &node_id in &node_ids {
                    let _ = conn.execute(
                        "DELETE FROM vec_kb_nodes WHERE node_id = ?1",
                        params![node_id],
                    );
                    let _ = conn.execute(
                        "DELETE FROM kb_node_embeddings WHERE node_id = ?1",
                        params![node_id],
                    );
                }
            }

            // Delete library (CASCADE handles folders, documents, document_nodes)
            conn.execute("DELETE FROM kb_libraries WHERE id = ?1", [id])?;
            Ok(())
        })
    }

    // --- Document CRUD ---

    pub fn create_document(&self, doc: &Document) -> Result<i64> {
        self.transaction(|conn| {
            conn.execute(
                "INSERT INTO kb_documents \
                 (library_id, folder_id, original_name, name_tokens, \
                  file_size, content_hash, extension, mime_type, \
                  source_type, storage_path, original_path, job_id, processing_run_id) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    doc.library_id,
                    doc.folder_id,
                    doc.original_name,
                    doc.name_tokens,
                    doc.file_size,
                    doc.content_hash,
                    doc.extension,
                    doc.mime_type,
                    doc.source_type,
                    doc.storage_path,
                    doc.original_path,
                    doc.job_id,
                    doc.processing_run_id,
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    pub fn get_document(&self, id: i64) -> Result<Document> {
        self.query(|conn| {
            conn.query_row(
                "SELECT id, created_at, updated_at, library_id, folder_id, \
                        original_name, name_tokens, file_size, content_hash, \
                        extension, mime_type, source_type, storage_path, original_path, \
                        job_id, processing_run_id, \
                        parsing_status, parsing_progress, parsing_error, \
                        embedding_status, embedding_progress, embedding_error, \
                        word_total, split_total \
                 FROM kb_documents WHERE id = ?1",
                [id],
                |row| {
                    Ok(Document {
                        id: row.get(0)?,
                        created_at: row.get(1)?,
                        updated_at: row.get(2)?,
                        library_id: row.get(3)?,
                        folder_id: row.get(4)?,
                        original_name: row.get(5)?,
                        name_tokens: row.get(6)?,
                        file_size: row.get(7)?,
                        content_hash: row.get(8)?,
                        extension: row.get(9)?,
                        mime_type: row.get(10)?,
                        source_type: row.get(11)?,
                        storage_path: row.get(12)?,
                        original_path: row.get(13)?,
                        job_id: row.get(14)?,
                        processing_run_id: row.get(15)?,
                        parsing_status: row.get(16)?,
                        parsing_progress: row.get(17)?,
                        parsing_error: row.get(18)?,
                        embedding_status: row.get(19)?,
                        embedding_progress: row.get(20)?,
                        embedding_error: row.get(21)?,
                        word_total: row.get(22)?,
                        split_total: row.get(23)?,
                        // P0-010: new extended fields with defaults
                        title: String::new(),
                        summary: String::new(),
                        keywords: String::new(),
                        entity_names: String::new(),
                        source_uri: String::new(),
                        modified_at: None,
                        document_date: None,
                        normalized_content_hash: String::new(),
                        simhash: String::new(),
                        document_family_id: None,
                        version_label: String::new(),
                        document_granularity: "micro".to_string(),
                        content_char_count: 0,
                        content_token_count: 0,
                        page_count: 0,
                        section_count: 0,
                        chunk_strategy: "auto".to_string(),
                        document_status: "queued".to_string(),
                        index_status: "pending".to_string(),
                        current_version_id: None,
                        deleted_at: None,
                        purged_at: None,
                        last_indexed_at: None,
                        last_seen_at: None,
                        ocr_status: "not_needed".to_string(),
                        ocr_text_coverage: 0.0,
                    })
                },
            )
            .map_err(|e| GBrainError::Database(format!("Document not found: {}", e)))
        })
    }

    pub fn list_documents(
        &self,
        library_id: i64,
        folder_id: Option<i64>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<DocumentListItem>> {
        self.query(|conn| {
            let sql = if folder_id.is_some() {
                "SELECT id, original_name, extension, file_size, \
                        parsing_status, parsing_progress, embedding_status, embedding_progress, \
                        job_id, folder_id, updated_at, \
                        title, document_granularity \
                 FROM kb_documents WHERE library_id = ?1 AND folder_id = ?2 \
                 ORDER BY updated_at DESC LIMIT ?3 OFFSET ?4"
            } else {
                "SELECT id, original_name, extension, file_size, \
                        parsing_status, parsing_progress, embedding_status, embedding_progress, \
                        job_id, folder_id, updated_at, \
                        title, document_granularity \
                 FROM kb_documents WHERE library_id = ?1 \
                 ORDER BY updated_at DESC LIMIT ?3 OFFSET ?4"
            };

            let mut stmt = conn.prepare(sql)?;

            // Use a single closure to map rows, avoiding the if/else closure-type mismatch
            let map_row =
                |row: &rusqlite::Row| -> std::result::Result<DocumentListItem, rusqlite::Error> {
                    Ok(DocumentListItem {
                        id: row.get(0)?,
                        original_name: row.get(1)?,
                        extension: row.get(2)?,
                        file_size: row.get(3)?,
                        parsing_status: row.get(4)?,
                        parsing_progress: row.get(5)?,
                        embedding_status: row.get(6)?,
                        embedding_progress: row.get(7)?,
                        job_id: row.get(8)?,
                        folder_id: row.get(9)?,
                        updated_at: row.get(10)?,
                        title: row.get(11)?,
                        document_granularity: row.get(12)?,
                    })
                };

            let rows: Vec<DocumentListItem> = if let Some(fid) = folder_id {
                stmt.query_map(
                    params![library_id, fid, limit as i64, offset as i64],
                    map_row,
                )?
                .filter_map(|r| r.ok())
                .collect()
            } else {
                stmt.query_map(params![library_id, limit as i64, offset as i64], map_row)?
                    .filter_map(|r| r.ok())
                    .collect()
            };

            Ok(rows)
        })
    }

    pub fn find_document_by_hash(
        &self,
        library_id: i64,
        content_hash: &str,
    ) -> Result<Option<Document>> {
        self.query(|conn| {
            let result = conn.query_row(
                "SELECT id, created_at, updated_at, library_id, folder_id, \
                        original_name, name_tokens, file_size, content_hash, \
                        extension, mime_type, source_type, storage_path, original_path, \
                        job_id, processing_run_id, \
                        parsing_status, parsing_progress, parsing_error, \
                        embedding_status, embedding_progress, embedding_error, \
                        word_total, split_total \
                 FROM kb_documents WHERE library_id = ?1 AND content_hash = ?2",
                params![library_id, content_hash],
                |row| {
                    Ok(Document {
                        id: row.get(0)?,
                        created_at: row.get(1)?,
                        updated_at: row.get(2)?,
                        library_id: row.get(3)?,
                        folder_id: row.get(4)?,
                        original_name: row.get(5)?,
                        name_tokens: row.get(6)?,
                        file_size: row.get(7)?,
                        content_hash: row.get(8)?,
                        extension: row.get(9)?,
                        mime_type: row.get(10)?,
                        source_type: row.get(11)?,
                        storage_path: row.get(12)?,
                        original_path: row.get(13)?,
                        job_id: row.get(14)?,
                        processing_run_id: row.get(15)?,
                        parsing_status: row.get(16)?,
                        parsing_progress: row.get(17)?,
                        parsing_error: row.get(18)?,
                        embedding_status: row.get(19)?,
                        embedding_progress: row.get(20)?,
                        embedding_error: row.get(21)?,
                        word_total: row.get(22)?,
                        split_total: row.get(23)?,
                        // P0-010: new extended fields with defaults
                        title: String::new(),
                        summary: String::new(),
                        keywords: String::new(),
                        entity_names: String::new(),
                        source_uri: String::new(),
                        modified_at: None,
                        document_date: None,
                        normalized_content_hash: String::new(),
                        simhash: String::new(),
                        document_family_id: None,
                        version_label: String::new(),
                        document_granularity: "micro".to_string(),
                        content_char_count: 0,
                        content_token_count: 0,
                        page_count: 0,
                        section_count: 0,
                        chunk_strategy: "auto".to_string(),
                        document_status: "queued".to_string(),
                        index_status: "pending".to_string(),
                        current_version_id: None,
                        deleted_at: None,
                        purged_at: None,
                        last_indexed_at: None,
                        last_seen_at: None,
                        ocr_status: "not_needed".to_string(),
                        ocr_text_coverage: 0.0,
                    })
                },
            );
            match result {
                Ok(doc) => Ok(Some(doc)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(GBrainError::Database(e.to_string())),
            }
        })
    }

    pub fn update_document_status(
        &self,
        id: i64,
        parsing_status: Option<i32>,
        parsing_progress: Option<i32>,
        parsing_error: Option<&str>,
        embedding_status: Option<i32>,
        embedding_progress: Option<i32>,
        embedding_error: Option<&str>,
    ) -> Result<()> {
        self.transaction(|conn| {
            let mut sets = Vec::new();
            let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

            if let Some(s) = parsing_status {
                sets.push("parsing_status = ?".to_string());
                param_values.push(Box::new(s));
            }
            if let Some(p) = parsing_progress {
                sets.push("parsing_progress = ?".to_string());
                param_values.push(Box::new(p));
            }
            if let Some(e) = parsing_error {
                sets.push("parsing_error = ?".to_string());
                param_values.push(Box::new(e.to_string()));
            }
            if let Some(s) = embedding_status {
                sets.push("embedding_status = ?".to_string());
                param_values.push(Box::new(s));
            }
            if let Some(p) = embedding_progress {
                sets.push("embedding_progress = ?".to_string());
                param_values.push(Box::new(p));
            }
            if let Some(e) = embedding_error {
                sets.push("embedding_error = ?".to_string());
                param_values.push(Box::new(e.to_string()));
            }

            if sets.is_empty() {
                return Ok(());
            }

            sets.push("updated_at = datetime('now')".to_string());
            param_values.push(Box::new(id));

            let sql = format!("UPDATE kb_documents SET {} WHERE id = ?", sets.join(", "));
            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                param_values.iter().map(|p| p.as_ref()).collect();

            conn.execute(&sql, param_refs.as_slice())?;
            Ok(())
        })
    }

    /// 更新文档的 job_id
    pub fn update_document_job_id(&self, id: i64, job_id: &str) -> Result<()> {
        self.transaction(|conn| {
            conn.execute(
                "UPDATE kb_documents SET job_id = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![job_id, id],
            )?;
            Ok(())
        })
    }

    /// 更新文档的 processing_run_id
    pub fn update_document_run_id(&self, id: i64, processing_run_id: &str) -> Result<()> {
        self.transaction(|conn| {
            conn.execute(
                "UPDATE kb_documents SET processing_run_id = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![processing_run_id, id],
            )?;
            Ok(())
        })
    }

    pub fn update_document_stats(&self, id: i64, word_total: i32, split_total: i32) -> Result<()> {
        self.transaction(|conn| {
            conn.execute(
                "UPDATE kb_documents SET word_total = ?1, split_total = ?2, \
                 parsing_status = ?3, embedding_status = ?3, \
                 updated_at = datetime('now') \
                 WHERE id = ?4",
                params![word_total, split_total, STATUS_COMPLETED, id],
            )?;
            Ok(())
        })
    }

    pub fn delete_document(&self, id: i64) -> Result<()> {
        let doc = self.get_document(id)?;
        let storage_path = doc.storage_path;
        let source_type = doc.source_type;

        self.transaction(|conn| {
            // Get node IDs for vector cleanup
            let node_ids: Vec<i64> = {
                let mut stmt =
                    conn.prepare("SELECT id FROM kb_document_nodes WHERE document_id = ?1")?;
                let rows = stmt.query_map([id], |row| row.get(0))?;
                rows.filter_map(|r| r.ok()).collect()
            };

            // Delete vectors for specific node IDs (no FK cascade on vec_kb_nodes)
            if !node_ids.is_empty() {
                for &node_id in &node_ids {
                    let _ = conn.execute(
                        "DELETE FROM vec_kb_nodes WHERE node_id = ?1",
                        params![node_id],
                    );
                    let _ = conn.execute(
                        "DELETE FROM kb_node_embeddings WHERE node_id = ?1",
                        params![node_id],
                    );
                }
            }

            // Delete document (CASCADE handles document_nodes)
            conn.execute("DELETE FROM kb_documents WHERE id = ?1", [id])?;

            Ok(())
        })?;

        // 仅对 upload 类型删除物理文件；ingest 类型的 storage_path 是用户原始文件，不应删除
        if !storage_path.is_empty() && source_type == "upload" {
            let path = std::path::Path::new(&storage_path);
            if path.exists() {
                if let Err(e) = std::fs::remove_file(path) {
                    tracing::warn!("删除存储文件失败 {}: {}", storage_path, e);
                }
            }
        }

        Ok(())
    }

    // --- DocumentNode operations ---

    pub fn get_document_nodes(&self, document_id: i64) -> Result<Vec<DocumentNode>> {
        self.query(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, created_at, updated_at, library_id, document_id, \
                        content, content_tokens, level, parent_id, chunk_order \
                 FROM kb_document_nodes \
                 WHERE document_id = ?1 \
                 ORDER BY level ASC, chunk_order ASC",
            )?;
            let rows = stmt.query_map([document_id], |row| {
                Ok(DocumentNode {
                    id: row.get(0)?,
                    created_at: row.get(1)?,
                    updated_at: row.get(2)?,
                    library_id: row.get(3)?,
                    document_id: row.get(4)?,
                    content: row.get(5)?,
                    content_tokens: row.get(6)?,
                    level: row.get(7)?,
                    parent_id: row.get(8)?,
                    chunk_order: row.get(9)?,
                    // P0-011: new node metadata fields
                    section_id: None,
                    title_path: String::new(),
                    page_number: None,
                    source_start: None,
                    source_end: None,
                    node_metadata: String::new(),
                    embedding_text: String::new(),
                })
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| GBrainError::Database(e.to_string()))
        })
    }

    pub fn delete_document_nodes(&self, document_id: i64) -> Result<()> {
        self.transaction(|conn| {
            // Get node IDs for vector cleanup (no FK cascade on vec_kb_nodes)
            let node_ids: Vec<i64> = {
                let mut stmt =
                    conn.prepare("SELECT id FROM kb_document_nodes WHERE document_id = ?1")?;
                let rows = stmt.query_map([document_id], |row| row.get(0))?;
                rows.filter_map(|r| r.ok()).collect()
            };

            // Delete vectors first (no FK cascade on vec_kb_nodes / kb_node_embeddings)
            for &node_id in &node_ids {
                let _ = conn.execute(
                    "DELETE FROM vec_kb_nodes WHERE node_id = ?1",
                    params![node_id],
                );
                let _ = conn.execute(
                    "DELETE FROM kb_node_embeddings WHERE node_id = ?1",
                    params![node_id],
                );
            }

            // Delete nodes (kb_doc_fts is auto-synced via triggers)
            conn.execute(
                "DELETE FROM kb_document_nodes WHERE document_id = ?1",
                [document_id],
            )?;
            Ok(())
        })
    }

    pub fn count_document_nodes(&self, library_id: i64) -> Result<i64> {
        self.query(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM kb_document_nodes WHERE library_id = ?1",
                [library_id],
                |row| row.get(0),
            )
            .map_err(|e| GBrainError::Database(e.to_string()))
        })
    }

    pub fn count_documents(&self, library_id: i64) -> Result<i64> {
        self.query(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM kb_documents WHERE library_id = ?1",
                [library_id],
                |row| row.get(0),
            )
            .map_err(|e| GBrainError::Database(e.to_string()))
        })
    }

    // --- Lifecycle helpers ---

    /// 软删除文档：设置 deleted_at 并将 document_status 设为 deleted
    pub fn soft_delete_document(&self, id: i64) -> Result<()> {
        self.transaction(|conn| crate::kb::lifecycle::soft_delete_document(conn, id))
    }

    /// 彻底清理已软删除的文档及其所有关联数据
    pub fn purge_document(&self, id: i64) -> Result<()> {
        crate::kb::lifecycle::purge_document(self, id)
    }

    /// 转换文档处理状态，带状态机合法性检查和可选的 run_id 守卫
    pub fn transition_document_status(
        &self,
        id: i64,
        new_status: crate::kb::lifecycle::DocumentStatus,
        run_id: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<()> {
        self.query(|conn| {
            crate::kb::lifecycle::transition_document_status(
                conn,
                id,
                new_status,
                run_id,
                error_message,
            )
        })
    }

    /// 创建文档版本快照
    pub fn create_document_version(
        &self,
        document_id: i64,
        version_label: &str,
        processing_run_id: &str,
        char_count: i32,
        node_count: i32,
    ) -> Result<i64> {
        self.transaction(|conn| {
            crate::kb::lifecycle::create_document_version(
                conn,
                document_id,
                version_label,
                processing_run_id,
                char_count,
                node_count,
            )
        })
    }

    /// 写入文档元数据（title/author/keywords/entities/source/dates）
    #[allow(clippy::too_many_arguments)]
    pub fn update_document_metadata(
        &self,
        id: i64,
        title: &str,
        _author: &str,
        keywords: &str,
        entity_names: &str,
        source_uri: &str,
        document_date: Option<&str>,
        modified_at: Option<&str>,
    ) -> Result<()> {
        self.transaction(|conn| {
            let title_val = if !title.is_empty() { title } else { "" };
            conn.execute(
                "UPDATE kb_documents SET title = COALESCE(NULLIF(?1, ''), title), \
                 keywords = ?2, entity_names = ?3, source_uri = COALESCE(NULLIF(?4, ''), source_uri), \
                 document_date = COALESCE(?5, document_date), \
                 modified_at = COALESCE(?6, modified_at), \
                 updated_at = datetime('now') \
                 WHERE id = ?7",
                rusqlite::params![
                    title_val,
                    keywords,
                    entity_names,
                    source_uri,
                    document_date,
                    modified_at,
                    id,
                ],
            )?;
            Ok(())
        })
    }

    /// 更新文档的 granularity 和 chunk 策略
    pub fn update_document_granularity(
        &self,
        id: i64,
        granularity: &str,
        chunk_strategy: &str,
        char_count: i32,
        page_count: i32,
    ) -> Result<()> {
        self.transaction(|conn| {
            conn.execute(
                "UPDATE kb_documents SET document_granularity = ?1, chunk_strategy = ?2, \
                 content_char_count = ?3, page_count = ?4, updated_at = datetime('now') \
                 WHERE id = ?5",
                params![granularity, chunk_strategy, char_count, page_count, id],
            )?;
            Ok(())
        })
    }

    /// P2-019: 更新文档 OCR 状态和文本覆盖率
    pub fn update_document_ocr(
        &self,
        id: i64,
        ocr_status: &str,
        ocr_text_coverage: f64,
    ) -> Result<()> {
        self.transaction(|conn| {
            conn.execute(
                "UPDATE kb_documents SET ocr_status = ?1, ocr_text_coverage = ?2, \
                 updated_at = datetime('now') WHERE id = ?3",
                params![ocr_status, ocr_text_coverage, id],
            )?;
            Ok(())
        })
    }

    // --- Run guard ---

    pub fn ensure_document_run_current(
        &self,
        document_id: i64,
        processing_run_id: &str,
    ) -> Result<()> {
        self.query(|conn| {
            let current: String = conn
                .query_row(
                    "SELECT processing_run_id FROM kb_documents WHERE id = ?1",
                    [document_id],
                    |row| row.get(0),
                )
                .map_err(|e| GBrainError::Database(format!("Document not found: {}", e)))?;

            if current != processing_run_id {
                return Err(GBrainError::InvalidInput(
                    "stale KB processing job; document has a newer run".to_string(),
                ));
            }
            Ok(())
        })
    }

    // --- Folder CRUD ---

    pub fn list_folders(&self, library_id: i64) -> Result<Vec<Folder>> {
        self.query(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, created_at, updated_at, library_id, parent_id, name, sort_order \
                 FROM kb_folders WHERE library_id = ?1 \
                 ORDER BY sort_order ASC, id ASC",
            )?;
            let rows = stmt.query_map([library_id], |row| {
                Ok(Folder {
                    id: row.get(0)?,
                    created_at: row.get(1)?,
                    updated_at: row.get(2)?,
                    library_id: row.get(3)?,
                    parent_id: row.get(4)?,
                    name: row.get(5)?,
                    sort_order: row.get(6)?,
                    children: Vec::new(),
                })
            })?;

            let flat: Vec<Folder> = rows.filter_map(|r| r.ok()).collect();
            Ok(build_folder_tree(flat))
        })
    }

    pub fn create_folder(&self, input: &CreateFolderInput) -> Result<i64> {
        self.transaction(|conn| {
            // Verify library exists
            let exists: bool = conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM kb_libraries WHERE id = ?1",
                    [input.library_id],
                    |row| row.get(0),
                )
                .unwrap_or(false);

            if !exists {
                return Err(GBrainError::InvalidInput("Library not found".to_string()));
            }

            // Verify parent folder exists and belongs to same library
            if let Some(pid) = input.parent_id {
                let parent_lib: i64 = conn
                    .query_row(
                        "SELECT library_id FROM kb_folders WHERE id = ?1",
                        [pid],
                        |row| row.get(0),
                    )
                    .map_err(|_| {
                        GBrainError::InvalidInput("Parent folder not found".to_string())
                    })?;

                if parent_lib != input.library_id {
                    return Err(GBrainError::InvalidInput(
                        "Parent folder belongs to different library".to_string(),
                    ));
                }
            }

            let max_sort: i32 = conn
                .query_row(
                    "SELECT COALESCE(MAX(sort_order), 0) FROM kb_folders WHERE library_id = ?1",
                    [input.library_id],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            conn.execute(
                "INSERT INTO kb_folders (library_id, parent_id, name, sort_order) \
                 VALUES (?1, ?2, ?3, ?4)",
                params![input.library_id, input.parent_id, input.name, max_sort + 1],
            )?;

            Ok(conn.last_insert_rowid())
        })
    }

    pub fn delete_folder(&self, folder_id: i64) -> Result<()> {
        self.transaction(|conn| {
            // Move all documents in this folder and its descendants to ungrouped
            // Use recursive CTE to find all descendant folder IDs
            let descendant_ids: Vec<i64> = {
                let mut stmt = conn.prepare(
                    "WITH RECURSIVE descendants(id) AS (\
                     SELECT ?1 UNION ALL \
                     SELECT f.id FROM kb_folders f INNER JOIN descendants d ON f.parent_id = d.id\
                     ) SELECT id FROM descendants",
                )?;
                let rows = stmt.query_map([folder_id], |row| row.get(0))?;
                rows.filter_map(|r| r.ok()).collect()
            };

            for &fid in &descendant_ids {
                conn.execute(
                    "UPDATE kb_documents SET folder_id = NULL WHERE folder_id = ?1",
                    [fid],
                )?;
            }

            // Delete all descendant folders and the target folder itself
            // (documents already moved to ungrouped above)
            for &fid in &descendant_ids {
                conn.execute("DELETE FROM kb_folders WHERE id = ?1", [fid])?;
            }

            Ok(())
        })
    }

    pub fn move_document_to_folder(&self, document_id: i64, folder_id: Option<i64>) -> Result<()> {
        self.transaction(|conn| {
            if let Some(fid) = folder_id {
                // Verify folder belongs to same library as document
                let lib_id: i64 = conn
                    .query_row(
                        "SELECT library_id FROM kb_folders WHERE id = ?1",
                        [fid],
                        |row| row.get(0),
                    )
                    .map_err(|_| GBrainError::InvalidInput("Folder not found".to_string()))?;

                let doc_lib: i64 = conn
                    .query_row(
                        "SELECT library_id FROM kb_documents WHERE id = ?1",
                        [document_id],
                        |row| row.get(0),
                    )
                    .map_err(|_| GBrainError::InvalidInput("Document not found".to_string()))?;

                if lib_id != doc_lib {
                    return Err(GBrainError::InvalidInput(
                        "Folder belongs to different library".to_string(),
                    ));
                }
            }

            conn.execute(
                "UPDATE kb_documents SET folder_id = ?1 WHERE id = ?2",
                params![folder_id, document_id],
            )?;
            Ok(())
        })
    }

    // --- Section CRUD (P1-008) ---

    /// 写入文档章节
    pub fn insert_section(
        &self,
        document_id: i64,
        parent_section_id: Option<i64>,
        title: &str,
        title_path: &str,
        heading_level: i32,
        section_order: i32,
        page_number: Option<i32>,
        source_start: Option<i32>,
        source_end: Option<i32>,
    ) -> Result<i64> {
        self.transaction(|conn| {
            conn.execute(
                "INSERT INTO kb_document_sections (document_id, parent_section_id, title, \
                 title_path, heading_level, section_order, page_number, source_start, source_end) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![
                    document_id,
                    parent_section_id,
                    title,
                    title_path,
                    heading_level,
                    section_order,
                    page_number,
                    source_start,
                    source_end
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// 获取文档的所有章节
    pub fn get_sections_for_document(
        &self,
        document_id: i64,
    ) -> Result<Vec<(i64, String, String, i32)>> {
        self.query(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, title, title_path, heading_level FROM kb_document_sections \
                 WHERE document_id = ?1 ORDER BY section_order",
            )?;
            let rows = stmt.query_map(params![document_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?;
            let results: Vec<(i64, String, String, i32)> = rows.filter_map(|r| r.ok()).collect();
            Ok(results)
        })
    }

    // --- Summary CRUD (P4-011) ---

    /// 写入文档/章节/表格摘要
    pub fn insert_summary(
        &self,
        document_id: i64,
        section_id: Option<i64>,
        summary_type: &str,
        summary_text: &str,
        model: &str,
    ) -> Result<i64> {
        self.transaction(|conn| {
            let tokens = crate::nlp::chinese::tokenize_content(summary_text);
            conn.execute(
                "INSERT INTO kb_document_summaries (document_id, section_id, summary_type, \
                 summary_text, summary_tokens, model) VALUES (?1,?2,?3,?4,?5,?6)",
                params![
                    document_id,
                    section_id,
                    summary_type,
                    summary_text,
                    tokens,
                    model
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// 获取文档摘要
    pub fn get_summaries_for_document(
        &self,
        document_id: i64,
    ) -> Result<Vec<(i64, String, String)>> {
        self.query(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, summary_type, summary_text FROM kb_document_summaries \
                 WHERE document_id = ?1",
            )?;
            let rows = stmt.query_map(params![document_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?;
            let results: Vec<(i64, String, String)> = rows.filter_map(|r| r.ok()).collect();
            Ok(results)
        })
    }

    // --- Source CRUD (P6-001/002) ---

    /// 创建导入源
    pub fn create_source(
        &self,
        library_id: i64,
        source_type: &str,
        source_uri: &str,
        display_name: &str,
        delete_policy: &str,
    ) -> Result<i64> {
        self.transaction(|conn| {
            conn.execute(
                "INSERT INTO kb_sources (library_id, source_type, source_uri, display_name, \
                 delete_policy, sync_status) VALUES (?1,?2,?3,?4,?5,'idle')",
                params![
                    library_id,
                    source_type,
                    source_uri,
                    display_name,
                    delete_policy
                ],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// 列出库的导入源
    pub fn list_sources(&self, library_id: i64) -> Result<Vec<(i64, String, String, String)>> {
        self.query(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, source_type, source_uri, display_name FROM kb_sources WHERE library_id=?1"
            )?;
            let rows = stmt.query_map(params![library_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?;
            let results: Vec<(i64, String, String, String)> = rows.filter_map(|r| r.ok()).collect();
            Ok(results)
        })
    }

    /// 删除导入源
    pub fn delete_source(&self, source_id: i64) -> Result<()> {
        self.transaction(|conn| {
            conn.execute(
                "DELETE FROM kb_source_items WHERE source_id=?1",
                params![source_id],
            )?;
            conn.execute("DELETE FROM kb_sources WHERE id=?1", params![source_id])?;
            Ok(())
        })
    }

    // --- Source Items CRUD (P6-002) ---

    /// 插入 source item（扫描后的文件条目）
    pub fn insert_source_item(
        &self,
        source_id: i64,
        item_path: &str,
        content_hash: &str,
        file_size: i64,
        last_seen_at: &str,
    ) -> Result<i64> {
        self.transaction(|conn| {
            conn.execute(
                "INSERT INTO kb_source_items (source_id, item_path, content_hash, file_size, \
                 last_seen_at, sync_status) VALUES (?1,?2,?3,?4,?5,'pending')",
                params![source_id, item_path, content_hash, file_size, last_seen_at],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    /// 更新 source item（hash 变化或同步状态更新）
    pub fn update_source_item(
        &self,
        source_id: i64,
        item_path: &str,
        content_hash: Option<&str>,
        sync_status: Option<&str>,
        sync_error: Option<&str>,
        document_id: Option<i64>,
        last_seen_at: Option<&str>,
    ) -> Result<()> {
        self.transaction(|conn| {
            if let Some(hash) = content_hash {
                conn.execute(
                    "UPDATE kb_source_items SET content_hash=?1 WHERE source_id=?2 AND item_path=?3",
                    params![hash, source_id, item_path],
                )?;
            }
            if let Some(status) = sync_status {
                conn.execute(
                    "UPDATE kb_source_items SET sync_status=?1 WHERE source_id=?2 AND item_path=?3",
                    params![status, source_id, item_path],
                )?;
            }
            if let Some(error) = sync_error {
                conn.execute(
                    "UPDATE kb_source_items SET sync_error=?1 WHERE source_id=?2 AND item_path=?3",
                    params![error, source_id, item_path],
                )?;
            }
            if let Some(doc_id) = document_id {
                conn.execute(
                    "UPDATE kb_source_items SET document_id=?1 WHERE source_id=?2 AND item_path=?3",
                    params![doc_id, source_id, item_path],
                )?;
            }
            if let Some(seen) = last_seen_at {
                conn.execute(
                    "UPDATE kb_source_items SET last_seen_at=?1 WHERE source_id=?2 AND item_path=?3",
                    params![seen, source_id, item_path],
                )?;
            }
            Ok(())
        })
    }

    /// 列出 source 的所有 items
    pub fn list_source_items(
        &self,
        source_id: i64,
    ) -> Result<Vec<(i64, String, String, Option<i64>, String)>> {
        self.query(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, item_path, content_hash, document_id, sync_status \
                 FROM kb_source_items WHERE source_id=?1 ORDER BY item_path",
            )?;
            let rows = stmt.query_map(params![source_id], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })?;
            let results: Vec<(i64, String, String, Option<i64>, String)> =
                rows.filter_map(|r| r.ok()).collect();
            Ok(results)
        })
    }

    /// 按 source_id 批量删除 source items
    pub fn delete_source_items_by_source(&self, source_id: i64) -> Result<i64> {
        self.transaction(|conn| {
            let count = conn.execute(
                "DELETE FROM kb_source_items WHERE source_id=?1",
                params![source_id],
            )?;
            Ok(count as i64)
        })
    }
}

/// Build a tree from a flat list of folders.
///
/// Uses a two-pass approach:
/// 1. Index all folders by id
/// 2. Attach children to their parents, extract roots
fn build_folder_tree(flat: Vec<Folder>) -> Vec<Folder> {
    if flat.is_empty() {
        return flat;
    }

    // Collect parent_id -> child indices mapping
    let mut children_map: std::collections::HashMap<i64, Vec<usize>> =
        std::collections::HashMap::new();
    let mut root_indices: Vec<usize> = Vec::new();

    for (i, folder) in flat.iter().enumerate() {
        match folder.parent_id {
            Some(pid) => {
                children_map.entry(pid).or_default().push(i);
            }
            None => {
                root_indices.push(i);
            }
        }
    }

    // Recursively build tree starting from roots
    let mut result = Vec::new();
    for &root_idx in &root_indices {
        let folder = build_subtree(&flat, root_idx, &children_map);
        result.push(folder);
    }
    result
}

fn build_subtree(
    flat: &[Folder],
    idx: usize,
    children_map: &std::collections::HashMap<i64, Vec<usize>>,
) -> Folder {
    let folder = &flat[idx];
    let child_indices = children_map.get(&folder.id).cloned().unwrap_or_default();

    let children: Vec<Folder> = child_indices
        .into_iter()
        .map(|ci| build_subtree(flat, ci, children_map))
        .collect();

    Folder {
        id: folder.id,
        created_at: folder.created_at.clone(),
        updated_at: folder.updated_at.clone(),
        library_id: folder.library_id,
        parent_id: folder.parent_id,
        name: folder.name.clone(),
        sort_order: folder.sort_order,
        children,
    }
}
