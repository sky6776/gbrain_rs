//! KB data model types

use crate::error::GBrainError;
use serde::{Deserialize, Serialize};

// --- Status constants ---
pub const STATUS_PENDING: i32 = 0;
pub const STATUS_PROCESSING: i32 = 1;
pub const STATUS_COMPLETED: i32 = 2;
pub const STATUS_FAILED: i32 = 3;

// --- Supported extensions ---
pub const SUPPORTED_EXTENSIONS: &[&str] =
    &["pdf", "docx", "xlsx", "csv", "html", "htm", "txt", "md"];

pub fn is_supported_extension(ext: &str) -> bool {
    SUPPORTED_EXTENSIONS.contains(&ext.to_lowercase().as_str())
}

pub fn mime_type_for_ext(ext: &str) -> &'static str {
    match ext.to_lowercase().as_str() {
        "pdf" => "application/pdf",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "csv" => "text/csv",
        "html" | "htm" => "text/html",
        "txt" => "text/plain",
        "md" => "text/markdown",
        _ => "application/octet-stream",
    }
}

// --- Library model ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library {
    pub id: i64,
    pub created_at: String,
    pub updated_at: String,
    pub name: String,
    pub semantic_segmentation_enabled: bool,
    pub raptor_enabled: bool,
    pub raptor_llm_base_url: String,
    #[serde(skip_serializing)]
    pub raptor_llm_secret_ref: String,
    pub raptor_llm_model: String,
    pub chunk_size: usize,
    pub chunk_overlap: usize,
    pub batch_max_documents: usize,
    pub batch_max_chunks: usize,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateLibraryInput {
    pub name: String,
    #[serde(default)]
    pub semantic_segmentation_enabled: Option<bool>,
    #[serde(default)]
    pub raptor_enabled: Option<bool>,
    #[serde(default)]
    pub raptor_llm_base_url: Option<String>,
    #[serde(default)]
    pub raptor_llm_secret_ref: Option<String>,
    #[serde(default)]
    pub raptor_llm_model: Option<String>,
    #[serde(default)]
    pub chunk_size: Option<usize>,
    #[serde(default)]
    pub chunk_overlap: Option<usize>,
    #[serde(default)]
    pub batch_max_documents: Option<usize>,
    #[serde(default)]
    pub batch_max_chunks: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateLibraryInput {
    pub name: Option<String>,
    pub semantic_segmentation_enabled: Option<bool>,
    pub raptor_enabled: Option<bool>,
    pub raptor_llm_base_url: Option<String>,
    pub raptor_llm_secret_ref: Option<String>,
    pub raptor_llm_model: Option<String>,
    pub chunk_size: Option<usize>,
    pub chunk_overlap: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryListItem {
    pub id: i64,
    pub name: String,
    pub document_count: i64,
    pub chunk_count: i64,
    pub sort_order: i32,
    pub raptor_enabled: bool,
    pub semantic_segmentation_enabled: bool,
    pub has_raptor_secret: bool,
}

// --- Folder model ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: i64,
    pub created_at: String,
    pub updated_at: String,
    pub library_id: i64,
    pub parent_id: Option<i64>,
    pub name: String,
    pub sort_order: i32,
    #[serde(default)]
    pub children: Vec<Folder>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFolderInput {
    pub library_id: i64,
    pub parent_id: Option<i64>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderStats {
    pub folder_id: i64,
    pub doc_count: i64,
    pub latest_doc_updated_at: Option<String>,
}

// --- Document model ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: i64,
    pub created_at: String,
    pub updated_at: String,
    pub library_id: i64,
    pub folder_id: Option<i64>,
    pub original_name: String,
    pub name_tokens: String,
    pub file_size: i64,
    pub content_hash: String,
    pub extension: String,
    pub mime_type: String,
    pub source_type: String,
    pub storage_path: String,
    pub original_path: String,
    pub job_id: String,
    pub processing_run_id: String,
    pub parsing_status: i32,
    pub parsing_progress: i32,
    pub parsing_error: String,
    pub embedding_status: i32,
    pub embedding_progress: i32,
    pub embedding_error: String,
    pub word_total: i32,
    pub split_total: i32,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            id: 0,
            created_at: String::new(),
            updated_at: String::new(),
            library_id: 0,
            folder_id: None,
            original_name: String::new(),
            name_tokens: String::new(),
            file_size: 0,
            content_hash: String::new(),
            extension: String::new(),
            mime_type: String::new(),
            source_type: "local".to_string(),
            storage_path: String::new(),
            original_path: String::new(),
            job_id: String::new(),
            processing_run_id: String::new(),
            parsing_status: STATUS_PENDING,
            parsing_progress: 0,
            parsing_error: String::new(),
            embedding_status: STATUS_PENDING,
            embedding_progress: 0,
            embedding_error: String::new(),
            word_total: 0,
            split_total: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentListItem {
    pub id: i64,
    pub original_name: String,
    pub extension: String,
    pub file_size: i64,
    pub parsing_status: i32,
    pub parsing_progress: i32,
    pub embedding_status: i32,
    pub embedding_progress: i32,
    pub job_id: String,
    pub folder_id: Option<i64>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadDocumentResponse {
    pub document_id: i64,
    pub job_id: String,
    pub processing_run_id: String,
    pub status: String,
    pub duplicate_of: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentStatus {
    pub document_id: i64,
    pub job_id: String,
    pub processing_run_id: String,
    pub parsing_status: i32,
    pub parsing_progress: i32,
    pub parsing_error: String,
    pub embedding_status: i32,
    pub embedding_progress: i32,
    pub embedding_error: String,
    pub word_total: i32,
    pub split_total: i32,
}

// --- DocumentNode model ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentNode {
    pub id: i64,
    pub created_at: String,
    pub updated_at: String,
    pub library_id: i64,
    pub document_id: i64,
    pub content: String,
    pub content_tokens: String,
    pub level: i32,
    pub parent_id: Option<i64>,
    pub chunk_order: i32,
}

/// In-memory RAPTOR node (with vector, not stored in DB directly)
#[derive(Debug, Clone)]
pub struct RaptorNode {
    pub id: i64,
    pub library_id: i64,
    pub document_id: i64,
    pub content: String,
    pub level: i32,
    pub parent_id: Option<i64>,
    pub chunk_order: i32,
    pub vector: Option<Vec<f32>>,
}

// --- Search model ---

#[derive(Debug, Clone)]
pub struct KbSearchInput {
    pub library_ids: Vec<i64>,
    pub query: String,
    pub level: Option<i32>,
    pub top_k: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KbSearchResult {
    pub node_id: i64,
    pub document_id: i64,
    pub document_name: String,
    pub content: String,
    pub level: i32,
    pub score: f64,
    pub library_id: i64,
    pub library_name: String,
}

#[derive(Debug, Clone)]
pub struct RankedResult {
    pub node_id: i64,
    pub rank: usize,
    pub score: f64,
}

// --- Pipeline model ---

#[derive(Debug, Clone)]
pub struct ProcessResult {
    pub word_total: i32,
    pub split_total: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Parsing,
    Splitting,
    Embedding,
    Raptor,
    Persist,
}

#[derive(Debug)]
pub struct PhaseError {
    pub phase: Phase,
    pub source: Box<dyn std::error::Error + Send + Sync>,
}

impl PhaseError {
    pub fn new<E>(phase: Phase, source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self {
            phase,
            source: Box::new(source),
        }
    }
}

impl std::fmt::Display for PhaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "phase {:?}: {}", self.phase, self.source)
    }
}

impl std::error::Error for PhaseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}

impl From<PhaseError> for GBrainError {
    fn from(err: PhaseError) -> Self {
        GBrainError::InvalidInput(err.to_string())
    }
}
