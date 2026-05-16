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
pub(crate) static OPERATION_DEFS: &[OperationDef] = &[
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
            // P0-016: Governance and model configuration
            ParamDef { name: "embedding_provider", description: "Embedding provider name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "embedding_model", description: "Embedding model name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "embedding_dimensions", description: "Embedding vector dimensions", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "search_profile", description: "Search profile name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "rerank_enabled", description: "Enable reranking", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "rerank_provider", description: "Rerank provider name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "summary_enabled", description: "Enable summarization", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "external_embedding_allowed", description: "Allow external embedding calls", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "external_rerank_allowed", description: "Allow external rerank calls", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "external_summary_allowed", description: "Allow external summary calls", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "external_ocr_allowed", description: "Allow external OCR calls", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "redaction_enabled", description: "Enable redaction of sensitive content", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
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
            ParamDef { name: "raptor_llm_base_url", description: "Raptor LLM base URL override", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "raptor_llm_secret_ref", description: "Raptor LLM API key env var name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "raptor_llm_model", description: "Raptor LLM model name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "chunk_size", description: "Chunk size in characters", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "chunk_overlap", description: "Chunk overlap in characters", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            // P0-016: Governance and model configuration
            ParamDef { name: "embedding_provider", description: "Embedding provider name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "embedding_model", description: "Embedding model name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "embedding_dimensions", description: "Embedding vector dimensions", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "search_profile", description: "Search profile name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "rerank_enabled", description: "Enable reranking", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "rerank_provider", description: "Rerank provider name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "summary_enabled", description: "Enable summarization", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "external_embedding_allowed", description: "Allow external embedding calls", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "external_rerank_allowed", description: "Allow external rerank calls", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "external_summary_allowed", description: "Allow external summary calls", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "external_ocr_allowed", description: "Allow external OCR calls", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "redaction_enabled", description: "Enable redaction of sensitive content", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
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
            ParamDef { name: "offset", description: "Skip first N results (for pagination)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_search",
        description: "Search across knowledge base libraries using hybrid vector + keyword + summary + table + metadata search with RRF fusion and rerank",
        params: &[
            ParamDef { name: "query", description: "Search query", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "library_ids", description: "Library IDs to search (empty = all)", required: false, param_type: ParamType::Array, enum_values: None, items_type: Some(ParamType::Integer) },
            ParamDef { name: "level", description: "Raptor tree level filter", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "top_k", description: "Max results (default 10, max 50)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "profile", description: "Search profile: fast|balanced|accurate|file_lookup|table", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "debug", description: "Enable debug mode (returns planner/rerank/fallback info)", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "include_context", description: "Include context before/after matched nodes", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "context_before", description: "Characters of context before match (default 200)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "context_after", description: "Characters of context after match (default 200)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "include_highlights", description: "Return highlight character ranges", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "group_by_document", description: "Group results by document", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "folder_id", description: "Filter to folder", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            // FIX9-03: 允许调用方指定 embedding 维度，覆盖全局配置
            ParamDef { name: "embedding_dimensions", description: "Override embedding dimensions for query vector", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            // FIX10-R3: 允许调用方指定 embedding_index_id，使用特定 index 的模型配置
            ParamDef { name: "embedding_index_id", description: "Specific embedding index ID to use for query vector (must belong to target library)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
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
    // P5/P6 operations tools
    OperationDef {
        name: "kb_purge_document",
        description: "Permanently destroy a soft-deleted document and all its associated data",
        params: &[
            ParamDef { name: "document_id", description: "Document ID to purge", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "confirm", description: "Must be true to confirm permanent destruction", required: true, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_check_index_health",
        description: "Run index health check (orphan nodes/embeddings/summaries, missing FTS, split mismatches)",
        params: &[],
    },
    OperationDef {
        name: "kb_repair_index",
        description: "Repair missing FTS entries for document nodes",
        params: &[],
    },
    OperationDef {
        name: "kb_backup",
        description: "Backup KB database to output directory",
        params: &[
            ParamDef { name: "output", description: "Output directory path", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_restore",
        description: "Restore KB database from backup directory",
        params: &[
            ParamDef { name: "input", description: "Input directory path containing backup", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_add_eval_query",
        description: "Add a search evaluation query with expected document IDs",
        params: &[
            ParamDef { name: "library_id", description: "Library ID", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "query", description: "Evaluation query text", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "query_type", description: "Query type classification", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "expected_document_ids", description: "Comma-separated expected document IDs", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },
    OperationDef {
        name: "kb_add_search_feedback",
        description: "Submit relevance feedback for a search result",
        params: &[
            ParamDef { name: "search_log_id", description: "Search log entry ID", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "document_id", description: "Document ID rated", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "node_id", description: "Node ID rated", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "rating", description: "Relevance rating 0-5", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "comment", description: "Optional feedback comment", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },

    // ========================================================================
    // 单入口多投影融合架构 — upload_source / memory_query / promotion / artifact
    // ========================================================================

    OperationDef {
        name: "upload_source",
        description: "Upload a source file (unified entry point for gbrain + KB + file storage). The system automatically creates Source Artifact, KB projection, shadow page, and file attachment based on intent.",
        params: &[
            ParamDef { name: "path", description: "Local file path to upload", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "intent", description: "Upload intent: auto, document, attachment, memory, promote", required: false, param_type: ParamType::String, enum_values: Some(&["auto", "document", "attachment", "memory", "promote"]), items_type: None },
            ParamDef { name: "library_id", description: "KB library ID (optional, uses default if not specified)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "target_slug", description: "Target gbrain page slug for promotion", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "page_slug", description: "Target page slug for file attachment", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "folder_id", description: "KB folder ID", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "promotion", description: "Promotion policy: none, shadow, candidate, auto-low-risk", required: false, param_type: ParamType::String, enum_values: Some(&["none", "shadow", "candidate", "auto-low-risk"]), items_type: None },
            ParamDef { name: "dry_run", description: "Only return route plan without committing", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },

    OperationDef {
        name: "memory_query",
        description: "Unified memory query. Searches both gbrain curated knowledge and KB document evidence. The planner automatically selects the best strategy based on query intent.",
        params: &[
            ParamDef { name: "query", description: "Query text", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "strategy", description: "Query strategy: brain_first, evidence_first, provenance, timeline_first", required: false, param_type: ParamType::String, enum_values: Some(&["brain_first", "evidence_first", "provenance", "timeline_first"]), items_type: None },
            ParamDef { name: "limit", description: "Maximum results", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "filter_slug", description: "Filter by brain slug (applies to all strategies: limits brain hits, evidence hits, and timeline hits to this page)", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "include_evidence", description: "Include KB evidence hits", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
            ParamDef { name: "include_provenance", description: "Include provenance records", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },

    OperationDef {
        name: "promotion_list_candidates",
        description: "List promotion candidates (suggested changes extracted from KB evidence)",
        params: &[
            ParamDef { name: "status", description: "Filter by status: pending, accepted, rejected, applied, rolled_back, stale, superseded", required: false, param_type: ParamType::String, enum_values: Some(&["pending", "accepted", "rejected", "applied", "rolled_back", "stale", "superseded"]), items_type: None },
            ParamDef { name: "candidate_type", description: "Filter by type: document_summary, entity_mention, link_suggestion, timeline_event, fact_claim, page_create, page_update", required: false, param_type: ParamType::String, enum_values: Some(&["document_summary", "entity_mention", "link_suggestion", "timeline_event", "fact_claim", "page_create", "page_update"]), items_type: None },
            ParamDef { name: "target_slug", description: "Filter by target slug", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "limit", description: "Maximum results", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },

    OperationDef {
        name: "promotion_get_candidate",
        description: "Get details of a promotion candidate",
        params: &[
            ParamDef { name: "candidate_id", description: "Candidate ID", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },

    OperationDef {
        name: "promotion_accept_candidate",
        description: "Accept a promotion candidate",
        params: &[
            ParamDef { name: "candidate_id", description: "Candidate ID", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "reviewer", description: "Reviewer name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "notes", description: "Review notes", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },

    OperationDef {
        name: "promotion_reject_candidate",
        description: "Reject a promotion candidate",
        params: &[
            ParamDef { name: "candidate_id", description: "Candidate ID", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "reviewer", description: "Reviewer name", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "reason", description: "Rejection reason", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },

    OperationDef {
        name: "promotion_apply_candidate",
        description: "Apply an approved promotion candidate to gbrain",
        params: &[
            ParamDef { name: "candidate_id", description: "Candidate ID", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },

    OperationDef {
        name: "promotion_rollback_candidate",
        description: "Rollback an applied promotion candidate, undoing shadow page updates and marking provenance stale (§31)",
        params: &[
            ParamDef { name: "candidate_id", description: "Candidate ID to rollback", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },

    OperationDef {
        name: "gc_orphan_projections",
        description: "Garbage collect orphaned projections and clean up stale projection records (§31)",
        params: &[
            ParamDef { name: "stale_days", description: "Delete projections orphaned/superseded for more than N days (default: 30)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "dry_run", description: "Preview what would be cleaned without making changes", required: false, param_type: ParamType::Boolean, enum_values: None, items_type: None },
        ],
    },

    OperationDef {
        name: "projection_supersede",
        description: "Supersede an old projection with a new one, marking the old as superseded and setting superseded_by (§31 version chain)",
        params: &[
            ParamDef { name: "old_proj_id", description: "Old projection ID to supersede", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "new_proj_id", description: "New projection ID that replaces the old one", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },

    OperationDef {
        name: "projection_history",
        description: "Query projection version chain history by projection_key (§31). Supports optional artifact_id and projection_type filters to avoid mixing projections from different artifacts sharing the same key.",
        params: &[
            ParamDef { name: "projection_key", description: "Projection key to query history for", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "artifact_id", description: "Optional artifact ID to filter by (avoids mixing projections from different artifacts)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "projection_type", description: "Optional projection type to filter by (e.g. 'kb_document')", required: false, param_type: ParamType::String, enum_values: None, items_type: None },
            ParamDef { name: "limit", description: "Maximum history records to return (default: 20)", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },

    OperationDef {
        name: "artifact_list",
        description: "List source artifacts",
        params: &[
            ParamDef { name: "limit", description: "Maximum results", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
            ParamDef { name: "offset", description: "Offset", required: false, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },

    OperationDef {
        name: "artifact_get",
        description: "Get source artifact details by ID or UID",
        params: &[
            ParamDef { name: "id_or_uid", description: "Artifact ID or UID (e.g. '1' or 'art_ab12cd34ef56')", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
        ],
    },

    OperationDef {
        name: "artifact_delete",
        description: "Soft delete a source artifact (marks all projections as stale)",
        params: &[
            ParamDef { name: "artifact_id", description: "Artifact ID", required: true, param_type: ParamType::Integer, enum_values: None, items_type: None },
        ],
    },

    OperationDef {
        name: "artifact_health",
        description: "Check artifact projection consistency and health",
        params: &[],
    },

    OperationDef {
        name: "get_provenance",
        description: "Get provenance records for a brain page (trace where facts came from)",
        params: &[
            ParamDef { name: "brain_slug", description: "Brain page slug", required: true, param_type: ParamType::String, enum_values: None, items_type: None },
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
