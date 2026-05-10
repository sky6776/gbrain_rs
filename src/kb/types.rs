//! KB data model types

use crate::error::GBrainError;
use serde::{Deserialize, Serialize};

// --- Status constants ---
pub const STATUS_PENDING: i32 = 0;
pub const STATUS_PROCESSING: i32 = 1;
pub const STATUS_COMPLETED: i32 = 2;
pub const STATUS_FAILED: i32 = 3;

/// 扩展名对应的 MIME 类型映射（格式映射，非安全校验）
/// 安全校验的允许扩展名列表由 Config.kb_allowed_extensions 控制。
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
    // P0-016: 库级治理和模型配置
    pub embedding_provider: String,
    pub embedding_model: String,
    pub embedding_dimensions: Option<i32>,
    pub search_profile: String,
    pub rerank_enabled: bool,
    pub rerank_provider: String,
    pub summary_enabled: bool,
    pub external_embedding_allowed: bool,
    pub external_rerank_allowed: bool,
    pub external_summary_allowed: bool,
    pub external_ocr_allowed: bool,
    pub redaction_enabled: bool,
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
    // P0-016: 库级治理和模型配置
    #[serde(default)]
    pub embedding_provider: Option<String>,
    #[serde(default)]
    pub embedding_model: Option<String>,
    #[serde(default)]
    pub embedding_dimensions: Option<i32>,
    #[serde(default)]
    pub search_profile: Option<String>,
    #[serde(default)]
    pub rerank_enabled: Option<bool>,
    #[serde(default)]
    pub rerank_provider: Option<String>,
    #[serde(default)]
    pub summary_enabled: Option<bool>,
    #[serde(default)]
    pub external_embedding_allowed: Option<bool>,
    #[serde(default)]
    pub external_rerank_allowed: Option<bool>,
    #[serde(default)]
    pub external_summary_allowed: Option<bool>,
    #[serde(default)]
    pub external_ocr_allowed: Option<bool>,
    #[serde(default)]
    pub redaction_enabled: Option<bool>,
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
    // P0-016: 库级治理和模型配置
    pub embedding_provider: Option<String>,
    pub embedding_model: Option<String>,
    pub embedding_dimensions: Option<i32>,
    pub search_profile: Option<String>,
    pub rerank_enabled: Option<bool>,
    pub rerank_provider: Option<String>,
    pub summary_enabled: Option<bool>,
    pub external_embedding_allowed: Option<bool>,
    pub external_rerank_allowed: Option<bool>,
    pub external_summary_allowed: Option<bool>,
    pub external_ocr_allowed: Option<bool>,
    pub redaction_enabled: Option<bool>,
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
    // P0-010: 扩展文档元数据字段
    pub title: String,
    pub summary: String,
    pub keywords: String,
    pub entity_names: String,
    pub source_uri: String,
    pub modified_at: Option<String>,
    pub document_date: Option<String>,
    pub normalized_content_hash: String,
    pub simhash: String,
    pub document_family_id: Option<String>,
    pub version_label: String,
    pub document_granularity: String,
    pub content_char_count: i32,
    pub content_token_count: i32,
    pub page_count: i32,
    pub section_count: i32,
    pub chunk_strategy: String,
    pub document_status: String,
    pub index_status: String,
    pub current_version_id: Option<i64>,
    pub deleted_at: Option<String>,
    pub purged_at: Option<String>,
    pub last_indexed_at: Option<String>,
    pub last_seen_at: Option<String>,
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
            // P0-010: 新字段默认值
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_granularity: Option<String>,
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
    // P0-011: 扩展节点元数据字段
    pub section_id: Option<i64>,
    pub title_path: String,
    pub page_number: Option<i32>,
    pub source_start: Option<i32>,
    pub source_end: Option<i32>,
    pub node_metadata: String,
    pub embedding_text: String,
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
    // P0-011: 扩展节点元数据字段
    pub title_path: String,
    pub page_number: Option<i32>,
    pub source_start: Option<i32>,
    pub source_end: Option<i32>,
    pub node_metadata: String,
    pub embedding_text: String,
}

// --- Search model ---

#[derive(Debug, Clone)]
pub struct KbSearchInput {
    pub library_ids: Vec<i64>,
    pub query: String,
    pub level: Option<i32>,
    pub top_k: usize,
    // P3-028: 扩展搜索参数
    pub profile: Option<String>,
    pub planner_override: Option<String>,
    pub debug: bool,
    pub include_context: bool,
    pub context_before: usize,
    pub context_after: usize,
    pub include_highlights: bool,
    pub group_by_document: bool,
    pub folder_id: Option<i64>,
}

impl Default for KbSearchInput {
    fn default() -> Self {
        Self {
            library_ids: vec![],
            query: String::new(),
            level: None,
            top_k: 10,
            profile: None,
            planner_override: None,
            debug: false,
            include_context: false,
            context_before: 200,
            context_after: 200,
            include_highlights: false,
            group_by_document: false,
            folder_id: None,
        }
    }
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
    // P3-024~027: 扩展返回字段
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_number: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight_ranges: Option<Vec<(usize, usize)>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug_signals: Option<serde_json::Value>,
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

// --- Parser block abstraction (P1-010) ---

/// 所有 parser 输出的统一 block 结构，屏蔽格式差异。
#[derive(Debug, Clone)]
pub struct ParsedBlock {
    /// 文本内容（原文）
    pub text: String,
    /// 标题路径，如 "第一章 / 1.1 概述"
    pub title_path: String,
    /// 页码（PDF 等，非页面格式为 None）
    pub page_number: Option<i32>,
    /// 在原文件中的起始字符偏移
    pub source_start: Option<i32>,
    /// 在原文件中的结束字符偏移
    pub source_end: Option<i32>,
    /// block 类型：paragraph, heading, list_item, table, code, whole_document
    pub block_type: String,
    /// 扩展元数据 JSON
    pub metadata: String,
}

impl ParsedBlock {
    /// 创建一个简单段落 block
    pub fn paragraph(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            title_path: String::new(),
            page_number: None,
            source_start: None,
            source_end: None,
            block_type: "paragraph".to_string(),
            metadata: String::new(),
        }
    }

    /// 创建一个全文 block（用于 micro/small 文档）
    pub fn whole_document(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            title_path: String::new(),
            page_number: None,
            source_start: None,
            source_end: None,
            block_type: "whole_document".to_string(),
            metadata: String::new(),
        }
    }
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
