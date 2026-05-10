//! MCP tool definitions — derived from operation definitions
//! Mirrors gbrain's src/mcp/tool-defs.ts
//!
//! Each tool is defined via OperationDef. The MCP input_schema JSON
//! is auto-generated from the structured definition, eliminating
//! the need for manual JSON construction.

use crate::operations::{OperationDef, ParamDef, ParamType};
use serde_json::Value;

/// Tool definition (MCP-compatible)
#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

impl From<&OperationDef> for ToolDef {
    fn from(op: &OperationDef) -> Self {
        ToolDef {
            name: op.name.to_string(),
            description: op.description.to_string(),
            input_schema: op.to_mcp_schema(),
        }
    }
}

/// All operation definitions — single source of truth for the brain API surface
static OPERATION_DEFS: &[OperationDef] = &[
    OperationDef {
        name: "query",
        description: "Hybrid search with vector + keyword + multi-query expansion",
        params: &[
            ParamDef { name: "query", description: "Search query", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "limit", description: "Max results (default 20)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "offset", description: "Skip first N results (for pagination)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "expand", description: "Enable multi-query expansion (default: true)", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "detail", description: "Result detail level: low (compiled truth only), medium (default, all with dedup), high (all chunks)", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "lang", description: "Filter code-aware retrieval by language", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "symbol_kind", description: "Filter code-aware retrieval by symbol kind", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "near_symbol", description: "Anchor two-pass code graph retrieval near this symbol", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "walk_depth", description: "Walk code graph neighbors up to this depth (0-2)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "include_meta", description: "Return {results, meta} with vector/expansion detail", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "search",
        description: "Hybrid search (keyword + vector + fallback + RRF fusion + boosts + dedup). Same as query but without expand/detail options.",
        params: &[
            ParamDef { name: "query", description: "Search query", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "limit", description: "Max results (default 20)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "offset", description: "Skip first N results (for pagination)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "lang", description: "Filter code-aware retrieval by language", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "symbol_kind", description: "Filter code-aware retrieval by symbol kind", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "near_symbol", description: "Anchor two-pass code graph retrieval near this symbol", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "walk_depth", description: "Walk code graph neighbors up to this depth (0-2)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "get_page",
        description: "Read a page by slug (supports optional fuzzy matching)",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "fuzzy", description: "Enable fuzzy slug resolution (default: false)", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "put_page",
        description: "Write/update a page (markdown with frontmatter). Chunks, embeds, reconciles tags, and (when auto_link/auto_timeline are enabled) extracts + reconciles graph links and timeline entries.",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "content", description: "Full markdown content with YAML frontmatter", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "delete_page",
        description: "Delete a page (requires confirm=true to prevent accidental deletion)",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "confirm", description: "Must be true to confirm deletion", required: true, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "list_pages",
        description: "List pages with optional filters",
        params: &[
            ParamDef { name: "type", description: "Filter by page type", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "tag", description: "Filter by tag", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "limit", description: "Max results (default 50)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "add_tag",
        description: "Add tag to page",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "tag", description: "Tag name", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "remove_tag",
        description: "Remove tag from page",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "tag", description: "Tag name", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "get_tags",
        description: "List tags for a page",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "add_link",
        description: "Create link between pages",
        params: &[
            ParamDef { name: "from", description: "Source slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "to", description: "Target slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "link_type", description: "Link type (e.g., invested_in, works_at)", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "context", description: "Context for the link", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "remove_link",
        description: "Remove link between pages",
        params: &[
            ParamDef { name: "from", description: "Source slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "to", description: "Target slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "link_type", description: "Link type to remove", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "get_links",
        description: "List outgoing links from a page",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "get_backlinks",
        description: "List incoming links to a page",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "traverse_graph",
        description: "Traverse link graph from a page. With link_type/direction, returns edges (GraphPath[]) instead of nodes.",
        params: &[
            ParamDef { name: "slug", description: "Starting slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "depth", description: "Max traversal depth (default 5, capped at 10)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "link_type", description: "Filter to one link type (per-edge filter, traversal only follows matching edges)", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "direction", description: "Traversal direction (default out)", required: false, param_type: ParamType::String, enum_values: Some(&["in", "out", "both"]), items_type: None },
        ],
    },
    OperationDef {
        name: "add_timeline_entry",
        description: "Add timeline entry to a page",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "date", description: "Date (YYYY-MM-DD)", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "summary", description: "Event summary", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "get_timeline",
        description: "Get timeline entries for a page",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "get_stats",
        description: "Brain statistics (page count, chunk count, pages by type, etc.)",
        params: &[],
    },
    OperationDef {
        name: "get_health",
        description: "Brain health dashboard (embed coverage, stale pages, orphans)",
        params: &[],
    },
    OperationDef {
        name: "get_versions",
        description: "Page version history",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "revert_version",
        description: "Revert page to a previous version",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "version_id", description: "Version ID to revert to", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "put_raw_data",
        description: "Store raw API response data for a page",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "source", description: "Data source (e.g., crustdata, happenstance)", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "data", description: "Raw data object", required: true, param_type: ParamType::Object, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "get_raw_data",
        description: "Retrieve raw data for a page",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "source", description: "Filter by source", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "resolve_slugs",
        description: "Fuzzy-resolve a partial slug to matching page slugs",
        params: &[
            ParamDef { name: "partial", description: "Partial slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "find_by_title_fuzzy",
        description: "Fuzzy search pages by title using trigram similarity",
        params: &[
            ParamDef { name: "query", description: "Search query (title to match)", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "dir_prefix", description: "Constrain to slug prefix (e.g., 'people', 'companies')", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "min_similarity", description: "Minimum similarity threshold 0.0-1.0 (default 0.55)", required: false, param_type: ParamType::Number, enum_values: None, items_type: None },
            ParamDef { name: "limit", description: "Max results (default 10)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "get_chunks",
        description: "Get content chunks for a page",
        params: &[
            ParamDef { name: "slug", description: "Page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "code_def",
        description: "Find code symbol definitions",
        params: &[
            ParamDef { name: "symbol", description: "Qualified or local symbol name", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "lang", description: "Filter by code language", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "limit", description: "Max results (default 20)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "code_refs",
        description: "Find code chunks referencing a symbol",
        params: &[
            ParamDef { name: "symbol", description: "Qualified or local symbol name", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "lang", description: "Filter by code language", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "limit", description: "Max results (default 20)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "search_code_chunks",
        description: "Search indexed code chunks by keyword/symbol text",
        params: &[
            ParamDef { name: "query", description: "Code search query", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "limit", description: "Max results (default 20)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "lang", description: "Filter by code language", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "symbol_kind", description: "Filter by symbol kind", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "get_callers",
        description: "Get code graph callers of a symbol",
        params: &[
            ParamDef { name: "slug", description: "Code page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "symbol", description: "Qualified or local symbol name", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "get_callees",
        description: "Get code graph callees of a symbol",
        params: &[
            ParamDef { name: "slug", description: "Code page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "symbol", description: "Qualified or local symbol name", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "get_code_edges_by_chunk",
        description: "Get code graph edges attached to a chunk id",
        params: &[
            ParamDef { name: "chunk_id", description: "Chunk id", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "reindex_code_page",
        description: "Rebuild code chunks and code edges for a code page",
        params: &[
            ParamDef { name: "slug", description: "Code page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "log_ingest",
        description: "Log an ingestion event",
        params: &[
            ParamDef { name: "source_type", description: "Source type (e.g., git, import, api)", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "source_ref", description: "Source reference", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "pages_updated", description: "List of updated page slugs", required: true, param_type: ParamType::Array, enum_values: None, items_type: Some(ParamType::String) },
            ParamDef { name: "summary", description: "Human-readable ingestion summary", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "get_ingest_log",
        description: "Get recent ingestion log entries",
        params: &[
            ParamDef { name: "limit", description: "Max entries (default 20)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "file_list",
        description: "List stored files",
        params: &[
            ParamDef { name: "slug", description: "Filter by page slug", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "file_upload",
        description: "Upload a file to storage",
        params: &[
            ParamDef { name: "path", description: "Local file path", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "page_slug", description: "Associate with page", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "file_url",
        description: "Get a URL for a stored file",
        params: &[
            ParamDef { name: "storage_path", description: "Storage path of the file", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "find_orphans",
        description: "Find pages with no inbound wikilinks. Essential for content enrichment cycles.",
        params: &[
            ParamDef { name: "include_pseudo", description: "Include auto-generated and pseudo pages (default: false)", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },
    // P1-9: sync_brain MCP tool (mirrors TS sync_brain operation)
    OperationDef {
        name: "sync_brain",
        description: "Sync brain from a Git repository. Reads .md files, chunking and embedding new/changed pages, removing deleted ones.",
        params: &[
            ParamDef { name: "repo_path", description: "Path to Git repository to sync from", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "force_full", description: "Force full sync instead of incremental (default: false)", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },
    // --- KB subsystem tools ---
    OperationDef {
        name: "kb_list_libraries",
        description: "List all knowledge base libraries with document and chunk counts",
        params: &[],
    },
    OperationDef {
        name: "kb_create_library",
        description: "Create a new knowledge base library",
        params: &[
            ParamDef { name: "name", description: "Library name", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "semantic_segmentation_enabled", description: "Enable semantic segmentation", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "raptor_enabled", description: "Enable Raptor tree summarization", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "raptor_llm_base_url", description: "Raptor LLM base URL override", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "raptor_llm_secret_ref", description: "Raptor LLM API key env var name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "raptor_llm_model", description: "Raptor LLM model name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "chunk_size", description: "Chunk size in characters", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "chunk_overlap", description: "Chunk overlap in characters", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "batch_max_documents", description: "Max documents per batch", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "batch_max_chunks", description: "Max chunks per batch", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_update_library",
        description: "Update a knowledge base library configuration",
        params: &[
            ParamDef { name: "library_id", description: "Library ID to update", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "name", description: "New library name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "semantic_segmentation_enabled", description: "Enable semantic segmentation", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "raptor_enabled", description: "Enable Raptor tree summarization", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_delete_library",
        description: "Delete a knowledge base library (requires confirm=true)",
        params: &[
            ParamDef { name: "library_id", description: "Library ID to delete", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "confirm", description: "Must be true to confirm deletion", required: true, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_upload_document",
        description: "Upload a document file to a knowledge base library for processing",
        params: &[
            ParamDef { name: "library_id", description: "Target library ID", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "file_path", description: "Local file path to upload", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "folder_id", description: "Optional folder ID", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_get_document_status",
        description: "Get the processing status of a document",
        params: &[
            ParamDef { name: "document_id", description: "Document ID", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_retry_document",
        description: "Retry processing a failed document",
        params: &[
            ParamDef { name: "document_id", description: "Document ID to retry", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_cancel_document_job",
        description: "Cancel a document processing job",
        params: &[
            ParamDef { name: "document_id", description: "Document ID to cancel", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_delete_document",
        description: "Delete a document from a library (requires confirm=true)",
        params: &[
            ParamDef { name: "document_id", description: "Document ID to delete", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "confirm", description: "Must be true to confirm deletion", required: true, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_list_documents",
        description: "List documents in a knowledge base library, optionally filtered by folder",
        params: &[
            ParamDef { name: "library_id", description: "Library ID", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "folder_id", description: "Optional folder ID to filter documents", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "limit", description: "Max results (default 50)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_search",
        description: "Search across knowledge base libraries using hybrid vector + keyword search",
        params: &[
            ParamDef { name: "query", description: "Search query", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "library_ids", description: "Library IDs to search (empty = all)", required: false, param_type: ParamType::Array, enum_values: None, items_type: Some(ParamType::Integer) },
            ParamDef { name: "level", description: "Raptor tree level filter", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "top_k", description: "Max results (default 10, max 50)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_create_folder",
        description: "Create a folder in a knowledge base library",
        params: &[
            ParamDef { name: "library_id", description: "Library ID", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "name", description: "Folder name", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "parent_id", description: "Parent folder ID (null = root)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
];

/// Build all tool definitions from structured operation definitions
pub fn build_tool_defs() -> Vec<ToolDef> {
    OPERATION_DEFS.iter().map(|op| op.into()).collect()
}

/// Get an operation definition by name
pub fn get_operation_def(name: &str) -> Option<&'static OperationDef> {
    OPERATION_DEFS.iter().find(|op| op.name == name)
}

/// Get all operation definitions
pub fn get_operation_defs() -> &'static [OperationDef] {
    OPERATION_DEFS
}
