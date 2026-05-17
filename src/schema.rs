//! SQLite schema DDL
//! Mirrors gbrain's src/core/pglite-schema.ts
//!
//! Complete SQLite schema with FTS5, triggers, and indexes.

/// Current schema version
pub const SCHEMA_VERSION: i32 = 27;

/// Complete schema DDL
pub const SCHEMA_DDL: &str = r#"
-- PRAGMAs
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
PRAGMA synchronous = NORMAL;
PRAGMA cache_size = -64000;
PRAGMA temp_store = MEMORY;

-- Schema version tracking
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Pages
CREATE TABLE IF NOT EXISTS pages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    slug TEXT NOT NULL UNIQUE,
    page_type TEXT NOT NULL DEFAULT 'note',
    title TEXT NOT NULL DEFAULT '',
    compiled_truth TEXT NOT NULL DEFAULT '',
    timeline TEXT NOT NULL DEFAULT '',
    frontmatter TEXT NOT NULL DEFAULT '',
    content_hash TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    deleted_at TEXT,
    title_tokens TEXT NOT NULL DEFAULT '',
    compiled_truth_tokens TEXT NOT NULL DEFAULT '',
    timeline_tokens TEXT NOT NULL DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_pages_slug ON pages(slug);
CREATE INDEX IF NOT EXISTS idx_pages_page_type ON pages(page_type);
CREATE INDEX IF NOT EXISTS idx_pages_updated_at ON pages(updated_at);
CREATE INDEX IF NOT EXISTS idx_pages_deleted_at ON pages(deleted_at);

-- FTS5 virtual table for full-text search (weighted: title > compiled_truth > timeline)
CREATE VIRTUAL TABLE IF NOT EXISTS pages_fts USING fts5(
    slug,
    title,
    title_tokens,
    compiled_truth,
    compiled_truth_tokens,
    timeline,
    timeline_tokens,
    content='pages',
    content_rowid='id',
    tokenize='unicode61'
);

-- Triggers to keep FTS5 in sync
CREATE TRIGGER IF NOT EXISTS pages_fts_insert AFTER INSERT ON pages BEGIN
    INSERT INTO pages_fts(rowid, slug, title, title_tokens, compiled_truth, compiled_truth_tokens, timeline, timeline_tokens)
    VALUES (new.id, new.slug, new.title, new.title_tokens, new.compiled_truth, new.compiled_truth_tokens, new.timeline, new.timeline_tokens);
END;

CREATE TRIGGER IF NOT EXISTS pages_fts_update AFTER UPDATE ON pages BEGIN
    INSERT INTO pages_fts(pages_fts, rowid, slug, title, title_tokens, compiled_truth, compiled_truth_tokens, timeline, timeline_tokens)
    VALUES ('delete', old.id, old.slug, old.title, old.title_tokens, old.compiled_truth, old.compiled_truth_tokens, old.timeline, old.timeline_tokens);
    INSERT INTO pages_fts(rowid, slug, title, title_tokens, compiled_truth, compiled_truth_tokens, timeline, timeline_tokens)
    VALUES (new.id, new.slug, new.title, new.title_tokens, new.compiled_truth, new.compiled_truth_tokens, new.timeline, new.timeline_tokens);
END;

CREATE TRIGGER IF NOT EXISTS pages_fts_delete AFTER DELETE ON pages BEGIN
    INSERT INTO pages_fts(pages_fts, rowid, slug, title, title_tokens, compiled_truth, compiled_truth_tokens, timeline, timeline_tokens)
    VALUES ('delete', old.id, old.slug, old.title, old.title_tokens, old.compiled_truth, old.compiled_truth_tokens, old.timeline, old.timeline_tokens);
END;

-- FTS5 trigram virtual table for fuzzy title matching
-- Provides indexed substring matching (approximates pg_trgm GIN index)
CREATE VIRTUAL TABLE IF NOT EXISTS pages_trgm USING fts5(
    title,
    content='pages',
    content_rowid='id',
    tokenize="trigram"
);

-- Triggers to keep trigram index in sync
CREATE TRIGGER IF NOT EXISTS pages_trgm_insert AFTER INSERT ON pages BEGIN
    INSERT INTO pages_trgm(rowid, title)
    VALUES (new.id, new.title);
END;

CREATE TRIGGER IF NOT EXISTS pages_trgm_update AFTER UPDATE ON pages BEGIN
    INSERT INTO pages_trgm(pages_trgm, rowid, title)
    VALUES ('delete', old.id, old.title);
    INSERT INTO pages_trgm(rowid, title)
    VALUES (new.id, new.title);
END;

CREATE TRIGGER IF NOT EXISTS pages_trgm_delete AFTER DELETE ON pages BEGIN
    INSERT INTO pages_trgm(pages_trgm, rowid, title)
    VALUES ('delete', old.id, old.title);
END;

-- Chunks
CREATE TABLE IF NOT EXISTS chunks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    page_id INTEGER NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    chunk_index INTEGER NOT NULL DEFAULT 0,
    chunk_text TEXT NOT NULL DEFAULT '',
    chunk_source TEXT NOT NULL DEFAULT 'compiled_truth',
    token_count INTEGER NOT NULL DEFAULT 0,
    model TEXT NOT NULL DEFAULT 'text-embedding-3-large',
    embedded_at TEXT,
    language TEXT,
    symbol_name TEXT,
    symbol_type TEXT,
    start_line INTEGER,
    end_line INTEGER,
    parent_symbol_path TEXT,
    symbol_name_qualified TEXT,
    doc_comment TEXT,
    chunk_text_tokens TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(page_id, chunk_index, chunk_source)
);

CREATE INDEX IF NOT EXISTS idx_chunks_page_id ON chunks(page_id);
CREATE INDEX IF NOT EXISTS idx_chunks_language ON chunks(language);
CREATE INDEX IF NOT EXISTS idx_chunks_symbol_name ON chunks(symbol_name);
CREATE INDEX IF NOT EXISTS idx_chunks_symbol_qualified ON chunks(symbol_name_qualified);
CREATE INDEX IF NOT EXISTS idx_chunks_parent_symbol ON chunks(parent_symbol_path);

-- FTS5 virtual table for chunk/code-chunk search.
CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
    chunk_text,
    chunk_text_tokens,
    language,
    symbol_name,
    symbol_type,
    content='chunks',
    content_rowid='id',
    tokenize='unicode61'
);

CREATE TRIGGER IF NOT EXISTS chunks_fts_insert AFTER INSERT ON chunks BEGIN
    INSERT INTO chunks_fts(rowid, chunk_text, chunk_text_tokens, language, symbol_name, symbol_type)
    VALUES (new.id, new.chunk_text, new.chunk_text_tokens,
            coalesce(new.language, ''), coalesce(new.symbol_name, ''), coalesce(new.symbol_type, ''));
END;

CREATE TRIGGER IF NOT EXISTS chunks_fts_update AFTER UPDATE ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, chunk_text, chunk_text_tokens, language, symbol_name, symbol_type)
    VALUES ('delete', old.id, old.chunk_text, old.chunk_text_tokens,
            coalesce(old.language, ''), coalesce(old.symbol_name, ''), coalesce(old.symbol_type, ''));
    INSERT INTO chunks_fts(rowid, chunk_text, chunk_text_tokens, language, symbol_name, symbol_type)
    VALUES (new.id, new.chunk_text, new.chunk_text_tokens,
            coalesce(new.language, ''), coalesce(new.symbol_name, ''), coalesce(new.symbol_type, ''));
END;

CREATE TRIGGER IF NOT EXISTS chunks_fts_delete AFTER DELETE ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, chunk_text, chunk_text_tokens, language, symbol_name, symbol_type)
    VALUES ('delete', old.id, old.chunk_text, old.chunk_text_tokens,
            coalesce(old.language, ''), coalesce(old.symbol_name, ''), coalesce(old.symbol_type, ''));
END;

-- Portable fallback embedding store. sqlite-vec is used when available; this
-- table keeps vector search functional in zero-config builds where vec0 is not
-- loadable.
CREATE TABLE IF NOT EXISTS chunk_embeddings (
    chunk_id INTEGER PRIMARY KEY REFERENCES chunks(id) ON DELETE CASCADE,
    embedding BLOB NOT NULL,
    dimensions INTEGER NOT NULL,
    model TEXT NOT NULL DEFAULT 'text-embedding-3-large',
    embedded_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Links
CREATE TABLE IF NOT EXISTS links (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    from_slug TEXT NOT NULL,
    to_slug TEXT NOT NULL,
    link_type TEXT NOT NULL DEFAULT 'mentions',
    context TEXT NOT NULL DEFAULT '',
    link_source TEXT NOT NULL DEFAULT 'auto',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(from_slug, to_slug, link_type, link_source)
);

CREATE INDEX IF NOT EXISTS idx_links_from_slug ON links(from_slug);
CREATE INDEX IF NOT EXISTS idx_links_to_slug ON links(to_slug);

-- Code edges: symbol call/reference graph extracted from code chunks.
CREATE TABLE IF NOT EXISTS code_edges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    from_slug TEXT NOT NULL,
    from_symbol TEXT NOT NULL,
    from_symbol_qualified TEXT,
    to_slug TEXT NOT NULL,
    to_symbol TEXT NOT NULL,
    to_symbol_qualified TEXT,
    edge_type TEXT NOT NULL DEFAULT 'calls',
    confidence REAL NOT NULL DEFAULT 1.0,
    context TEXT,
    from_chunk_id INTEGER REFERENCES chunks(id) ON DELETE CASCADE,
    to_chunk_id INTEGER REFERENCES chunks(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(from_slug, from_symbol, to_slug, to_symbol, edge_type, from_chunk_id)
);

CREATE INDEX IF NOT EXISTS idx_code_edges_from ON code_edges(from_slug, from_symbol);
CREATE INDEX IF NOT EXISTS idx_code_edges_to ON code_edges(to_slug, to_symbol);
CREATE INDEX IF NOT EXISTS idx_code_edges_from_chunk ON code_edges(from_chunk_id);
CREATE INDEX IF NOT EXISTS idx_code_edges_to_chunk ON code_edges(to_chunk_id);

-- Code edges by symbol: unresolved edges where target chunk hasn't been imported yet
CREATE TABLE IF NOT EXISTS code_edges_symbol (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    from_chunk_id INTEGER NOT NULL REFERENCES chunks(id) ON DELETE CASCADE,
    from_symbol_qualified TEXT NOT NULL,
    to_symbol_qualified TEXT NOT NULL,
    edge_type TEXT NOT NULL DEFAULT 'calls',
    edge_metadata TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(from_chunk_id, to_symbol_qualified, edge_type)
);

CREATE INDEX IF NOT EXISTS idx_code_edges_symbol_from ON code_edges_symbol(from_chunk_id);
CREATE INDEX IF NOT EXISTS idx_code_edges_symbol_to ON code_edges_symbol(to_symbol_qualified, edge_type);

-- Tags
CREATE TABLE IF NOT EXISTS tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    page_id INTEGER NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    tag TEXT NOT NULL,
    UNIQUE(page_id, tag)
);

CREATE INDEX IF NOT EXISTS idx_tags_page_id ON tags(page_id);
CREATE INDEX IF NOT EXISTS idx_tags_tag ON tags(tag);

-- Timeline
CREATE TABLE IF NOT EXISTS timeline (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    page_id INTEGER NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    date TEXT NOT NULL DEFAULT '',
    source TEXT NOT NULL DEFAULT '',
    summary TEXT NOT NULL DEFAULT '',
    detail TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(page_id, date, summary)
);

CREATE INDEX IF NOT EXISTS idx_timeline_page_id ON timeline(page_id);
CREATE INDEX IF NOT EXISTS idx_timeline_date ON timeline(date);

-- Raw data
CREATE TABLE IF NOT EXISTS raw_data (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    page_id INTEGER NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    source TEXT NOT NULL DEFAULT '',
    data TEXT NOT NULL DEFAULT '{}',
    fetched_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(page_id, source)
);

CREATE INDEX IF NOT EXISTS idx_raw_data_page_id ON raw_data(page_id);

-- Page versions
CREATE TABLE IF NOT EXISTS page_versions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    page_id INTEGER NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    compiled_truth TEXT NOT NULL DEFAULT '',
    frontmatter TEXT NOT NULL DEFAULT '',
    title TEXT NOT NULL DEFAULT '',
    page_type TEXT NOT NULL DEFAULT 'note',
    snapshot_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_page_versions_page_id ON page_versions(page_id);

-- Config
CREATE TABLE IF NOT EXISTS config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL DEFAULT ''
);

-- Ingest log
CREATE TABLE IF NOT EXISTS ingest_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source TEXT NOT NULL DEFAULT '',
    pages_updated TEXT NOT NULL DEFAULT '[]',
    status TEXT NOT NULL DEFAULT 'pending',
    error TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Files
CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    page_slug TEXT NOT NULL DEFAULT '',
    filename TEXT NOT NULL DEFAULT '',
    storage_path TEXT NOT NULL DEFAULT '',
    mime_type TEXT,
    size_bytes INTEGER NOT NULL DEFAULT 0,
    checksum TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_files_page_slug ON files(page_slug);
"#;

/// Generate sqlite-vec virtual table DDL
/// Returns DDL for the vec_chunks virtual table
pub fn vec_chunks_ddl(dimensions: usize) -> String {
    format!(
        r#"CREATE VIRTUAL TABLE IF NOT EXISTS vec_chunks USING vec0(
    chunk_id INTEGER PRIMARY KEY,
    embedding float[{}]
);"#,
        dimensions
    )
}

/// Migration DDL for schema version 2: add pages_trgm FTS5 trigram virtual table
pub const MIGRATION_V2_DDL: &str = r#"
-- FTS5 trigram virtual table for fuzzy title matching
CREATE VIRTUAL TABLE IF NOT EXISTS pages_trgm USING fts5(
    title,
    content='pages',
    content_rowid='id',
    tokenize="trigram"
);

-- Triggers to keep trigram index in sync
CREATE TRIGGER IF NOT EXISTS pages_trgm_insert AFTER INSERT ON pages BEGIN
    INSERT INTO pages_trgm(rowid, title)
    VALUES (new.id, new.title);
END;

CREATE TRIGGER IF NOT EXISTS pages_trgm_update AFTER UPDATE ON pages BEGIN
    INSERT INTO pages_trgm(pages_trgm, rowid, title)
    VALUES ('delete', old.id, old.title);
    INSERT INTO pages_trgm(rowid, title)
    VALUES (new.id, new.title);
END;

CREATE TRIGGER IF NOT EXISTS pages_trgm_delete AFTER DELETE ON pages BEGIN
    INSERT INTO pages_trgm(pages_trgm, rowid, title)
    VALUES ('delete', old.id, old.title);
END;

-- Rebuild trigram index from existing pages
INSERT INTO pages_trgm(rowid, title) SELECT id, title FROM pages;
"#;

/// Migration DDL for schema version 3: add link provenance columns
pub const MIGRATION_V3_DDL: &str = r#"
-- Add link provenance columns for tracking auto-extracted link origins
ALTER TABLE links ADD COLUMN origin_slug TEXT NOT NULL DEFAULT '';
ALTER TABLE links ADD COLUMN origin_field TEXT NOT NULL DEFAULT '';
"#;

/// Migration DDL for schema version 4: add link direction column
pub const MIGRATION_V4_DDL: &str = r#"
-- Add direction column for incoming/outgoing link semantics
ALTER TABLE links ADD COLUMN direction TEXT NOT NULL DEFAULT 'outgoing';
-- Add index for direction-based queries
CREATE INDEX IF NOT EXISTS idx_links_direction ON links(direction);
-- Migrate existing 'auto' link_source values to 'markdown' (the most common source)
UPDATE links SET link_source = 'markdown' WHERE link_source = 'auto';
"#;

/// Migration DDL for schema version 5: embedding model tracking + ingest_log enhancements
pub const MIGRATION_V5_DDL: &str = r#"
-- Track which embedding model was used for each chunk
ALTER TABLE chunks ADD COLUMN model TEXT NOT NULL DEFAULT 'text-embedding-3-large';

-- Split ingest_log source into source_type + source_ref, add summary
ALTER TABLE ingest_log ADD COLUMN source_type TEXT NOT NULL DEFAULT '';
ALTER TABLE ingest_log ADD COLUMN source_ref TEXT NOT NULL DEFAULT '';
ALTER TABLE ingest_log ADD COLUMN summary TEXT NOT NULL DEFAULT '';

-- Migrate existing ingest_log.source into source_type (best-effort heuristic)
UPDATE ingest_log SET source_type = source, source_ref = '' WHERE source_type = '';
"#;

/// Job queue table DDL
pub const JOBS_TABLE_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_type TEXT NOT NULL,
    payload TEXT NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'pending',
    priority INTEGER NOT NULL DEFAULT 0,
    attempts INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL DEFAULT 3,
    error TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    started_at TEXT,
    completed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status);
CREATE INDEX IF NOT EXISTS idx_jobs_type_status ON jobs(job_type, status);
"#;

/// Migration DDL for schema version 6: chunk-level FTS5 for keyword search
pub const MIGRATION_V6_DDL: &str = r#"
-- Chunk-level FTS5 virtual table for chunk-aware keyword search
CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
    chunk_text,
    content='chunks',
    content_rowid='id'
);

-- Triggers to keep chunks_fts in sync
CREATE TRIGGER IF NOT EXISTS chunks_fts_insert AFTER INSERT ON chunks BEGIN
    INSERT INTO chunks_fts(rowid, chunk_text)
    VALUES (new.id, new.chunk_text);
END;

CREATE TRIGGER IF NOT EXISTS chunks_fts_update AFTER UPDATE ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, chunk_text)
    VALUES ('delete', old.id, old.chunk_text);
    INSERT INTO chunks_fts(rowid, chunk_text)
    VALUES (new.id, new.chunk_text);
END;

CREATE TRIGGER IF NOT EXISTS chunks_fts_delete AFTER DELETE ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, chunk_text)
    VALUES ('delete', old.id, old.chunk_text);
END;

-- Populate from existing chunks
INSERT INTO chunks_fts(rowid, chunk_text) SELECT id, chunk_text FROM chunks;
"#;

/// Migration DDL for schema version 7: weighted FTS5 + timeline dedup constraint
///
/// Two changes:
/// 1. Rebuild pages_fts to include timeline column for weighted search
///    (title weight > compiled_truth weight > timeline weight)
/// 2. Add UNIQUE(page_id, date, summary) constraint on timeline table
///    to prevent duplicate entries from accumulating.
pub const MIGRATION_V7_DDL: &str = r#"
-- 1. Rebuild pages_fts with timeline column for weighted search
-- Drop old triggers first
DROP TRIGGER IF EXISTS pages_fts_insert;
DROP TRIGGER IF EXISTS pages_fts_update;
DROP TRIGGER IF EXISTS pages_fts_delete;
-- Drop old FTS5 table
DROP TABLE IF EXISTS pages_fts;
-- Recreate with timeline column (weighted: title > compiled_truth > timeline)
CREATE VIRTUAL TABLE pages_fts USING fts5(
    slug,
    title,
    compiled_truth,
    timeline,
    content='pages',
    content_rowid='id'
);
-- Recreate triggers with timeline column
CREATE TRIGGER pages_fts_insert AFTER INSERT ON pages BEGIN
    INSERT INTO pages_fts(rowid, slug, title, compiled_truth, timeline)
    VALUES (new.id, new.slug, new.title, new.compiled_truth, new.timeline);
END;
CREATE TRIGGER pages_fts_update AFTER UPDATE ON pages BEGIN
    INSERT INTO pages_fts(pages_fts, rowid, slug, title, compiled_truth, timeline)
    VALUES ('delete', old.id, old.slug, old.title, old.compiled_truth, old.timeline);
    INSERT INTO pages_fts(rowid, slug, title, compiled_truth, timeline)
    VALUES (new.id, new.slug, new.title, new.compiled_truth, new.timeline);
END;
CREATE TRIGGER pages_fts_delete AFTER DELETE ON pages BEGIN
    INSERT INTO pages_fts(pages_fts, rowid, slug, title, compiled_truth, timeline)
    VALUES ('delete', old.id, old.slug, old.title, old.compiled_truth, old.timeline);
END;
-- Rebuild FTS5 index from existing pages
INSERT INTO pages_fts(rowid, slug, title, compiled_truth, timeline)
SELECT id, slug, title, compiled_truth, timeline FROM pages;

-- 2. Add timeline dedup constraint
-- SQLite doesn't support ALTER TABLE ADD CONSTRAINT, so we recreate the table.
-- First, deduplicate existing entries (keep earliest by id)
DELETE FROM timeline WHERE id NOT IN (
    SELECT MIN(id) FROM timeline GROUP BY page_id, date, summary
);
-- Create new table with UNIQUE constraint
CREATE TABLE timeline_v7 (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    page_id INTEGER NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    date TEXT NOT NULL DEFAULT '',
    source TEXT NOT NULL DEFAULT '',
    summary TEXT NOT NULL DEFAULT '',
    detail TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(page_id, date, summary)
);
INSERT INTO timeline_v7 SELECT * FROM timeline;
DROP TABLE timeline;
ALTER TABLE timeline_v7 RENAME TO timeline;
CREATE INDEX IF NOT EXISTS idx_timeline_page_id ON timeline(page_id);
CREATE INDEX IF NOT EXISTS idx_timeline_date ON timeline(date);
"#;

/// Migration DDL for schema version 8: add title and page_type to page_versions
///
/// The create_version function and get_versions query reference title and page_type
/// columns that were missing from the original page_versions table schema.
/// This migration adds those columns so version snapshots capture title and page_type.
pub const MIGRATION_V8_DDL: &str = r#"
-- Add title and page_type columns to page_versions
ALTER TABLE page_versions ADD COLUMN title TEXT NOT NULL DEFAULT '';
ALTER TABLE page_versions ADD COLUMN page_type TEXT NOT NULL DEFAULT 'note';
"#;

/// Migration DDL for schema version 9: soft delete, portable embeddings,
/// and code/fenced-code chunk metadata.
pub const MIGRATION_V9_DDL: &str = r#"
ALTER TABLE pages ADD COLUMN deleted_at TEXT;
CREATE INDEX IF NOT EXISTS idx_pages_deleted_at ON pages(deleted_at);

ALTER TABLE chunks ADD COLUMN language TEXT;
ALTER TABLE chunks ADD COLUMN symbol_name TEXT;
ALTER TABLE chunks ADD COLUMN symbol_type TEXT;
ALTER TABLE chunks ADD COLUMN start_line INTEGER;
ALTER TABLE chunks ADD COLUMN end_line INTEGER;
CREATE INDEX IF NOT EXISTS idx_chunks_language ON chunks(language);
CREATE INDEX IF NOT EXISTS idx_chunks_symbol_name ON chunks(symbol_name);

CREATE TABLE IF NOT EXISTS chunk_embeddings (
    chunk_id INTEGER PRIMARY KEY REFERENCES chunks(id) ON DELETE CASCADE,
    embedding BLOB NOT NULL,
    dimensions INTEGER NOT NULL,
    model TEXT NOT NULL DEFAULT 'text-embedding-3-large',
    embedded_at TEXT NOT NULL DEFAULT (datetime('now'))
);
"#;

/// Migration DDL for schema version 10: code chunk FTS and code symbol graph.
pub const MIGRATION_V10_DDL: &str = r#"
DROP TRIGGER IF EXISTS chunks_fts_insert;
DROP TRIGGER IF EXISTS chunks_fts_update;
DROP TRIGGER IF EXISTS chunks_fts_delete;
DROP TABLE IF EXISTS chunks_fts;

CREATE VIRTUAL TABLE chunks_fts USING fts5(
    chunk_text,
    language,
    symbol_name,
    symbol_type,
    content='chunks',
    content_rowid='id'
);

CREATE TRIGGER IF NOT EXISTS chunks_fts_insert AFTER INSERT ON chunks BEGIN
    INSERT INTO chunks_fts(rowid, chunk_text, language, symbol_name, symbol_type)
    VALUES (new.id, new.chunk_text, coalesce(new.language, ''), coalesce(new.symbol_name, ''), coalesce(new.symbol_type, ''));
END;

CREATE TRIGGER IF NOT EXISTS chunks_fts_update AFTER UPDATE ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, chunk_text, language, symbol_name, symbol_type)
    VALUES ('delete', old.id, old.chunk_text, coalesce(old.language, ''), coalesce(old.symbol_name, ''), coalesce(old.symbol_type, ''));
    INSERT INTO chunks_fts(rowid, chunk_text, language, symbol_name, symbol_type)
    VALUES (new.id, new.chunk_text, coalesce(new.language, ''), coalesce(new.symbol_name, ''), coalesce(new.symbol_type, ''));
END;

CREATE TRIGGER IF NOT EXISTS chunks_fts_delete AFTER DELETE ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, chunk_text, language, symbol_name, symbol_type)
    VALUES ('delete', old.id, old.chunk_text, coalesce(old.language, ''), coalesce(old.symbol_name, ''), coalesce(old.symbol_type, ''));
END;

INSERT INTO chunks_fts(rowid, chunk_text, language, symbol_name, symbol_type)
SELECT id, chunk_text, coalesce(language, ''), coalesce(symbol_name, ''), coalesce(symbol_type, '')
FROM chunks
WHERE NOT EXISTS (SELECT 1 FROM chunks_fts WHERE rowid = chunks.id);

CREATE TABLE IF NOT EXISTS code_edges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    from_slug TEXT NOT NULL,
    from_symbol TEXT NOT NULL,
    to_slug TEXT NOT NULL,
    to_symbol TEXT NOT NULL,
    edge_type TEXT NOT NULL DEFAULT 'calls',
    confidence REAL NOT NULL DEFAULT 1.0,
    context TEXT,
    from_chunk_id INTEGER REFERENCES chunks(id) ON DELETE CASCADE,
    to_chunk_id INTEGER REFERENCES chunks(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(from_slug, from_symbol, to_slug, to_symbol, edge_type, from_chunk_id)
);

CREATE INDEX IF NOT EXISTS idx_code_edges_from ON code_edges(from_slug, from_symbol);
CREATE INDEX IF NOT EXISTS idx_code_edges_to ON code_edges(to_slug, to_symbol);
CREATE INDEX IF NOT EXISTS idx_code_edges_from_chunk ON code_edges(from_chunk_id);
CREATE INDEX IF NOT EXISTS idx_code_edges_to_chunk ON code_edges(to_chunk_id);
"#;

/// Migration DDL for schema version 11: qualified symbol paths, doc comments, and
/// unresolved symbol edges for forward-declaration code graph support.
pub const MIGRATION_V11_DDL: &str = r#"
-- Add parent scope path (comma-separated, e.g. "BrainEngine,searchKeyword")
ALTER TABLE chunks ADD COLUMN parent_symbol_path TEXT;
-- Add language-aware qualified name (e.g. "BrainEngine.searchKeyword")
ALTER TABLE chunks ADD COLUMN symbol_name_qualified TEXT;
-- Add extracted doc comment above symbol
ALTER TABLE chunks ADD COLUMN doc_comment TEXT;

CREATE INDEX IF NOT EXISTS idx_chunks_symbol_qualified ON chunks(symbol_name_qualified);
CREATE INDEX IF NOT EXISTS idx_chunks_parent_symbol ON chunks(parent_symbol_path);

-- Add qualified symbol name columns to code_edges for two-pass retrieval
ALTER TABLE code_edges ADD COLUMN from_symbol_qualified TEXT;
ALTER TABLE code_edges ADD COLUMN to_symbol_qualified TEXT;
CREATE INDEX IF NOT EXISTS idx_code_edges_to_symbol_qualified ON code_edges(to_symbol_qualified);

-- Unresolved symbol edges: edges where target chunk hasn't been imported yet.
-- Allows recording cross-module calls/references before all code is indexed.
CREATE TABLE IF NOT EXISTS code_edges_symbol (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    from_chunk_id INTEGER NOT NULL REFERENCES chunks(id) ON DELETE CASCADE,
    from_symbol_qualified TEXT NOT NULL,
    to_symbol_qualified TEXT NOT NULL,
    edge_type TEXT NOT NULL DEFAULT 'calls',
    edge_metadata TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(from_chunk_id, to_symbol_qualified, edge_type)
);

CREATE INDEX IF NOT EXISTS idx_code_edges_symbol_from ON code_edges_symbol(from_chunk_id);
CREATE INDEX IF NOT EXISTS idx_code_edges_symbol_to ON code_edges_symbol(to_symbol_qualified, edge_type);
"#;

/// Migration DDL for schema version 12: KB libraries and folders
pub const MIGRATION_V12_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS kb_libraries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    name TEXT NOT NULL,
    semantic_segmentation_enabled INTEGER NOT NULL DEFAULT 0,
    raptor_enabled INTEGER NOT NULL DEFAULT 0,
    raptor_llm_base_url TEXT NOT NULL DEFAULT '',
    raptor_llm_secret_ref TEXT NOT NULL DEFAULT '',
    raptor_llm_model TEXT NOT NULL DEFAULT '',
    chunk_size INTEGER NOT NULL DEFAULT 512,
    chunk_overlap INTEGER NOT NULL DEFAULT 50,
    batch_max_documents INTEGER NOT NULL DEFAULT 3,
    batch_max_chunks INTEGER NOT NULL DEFAULT 10,
    sort_order INTEGER NOT NULL DEFAULT 0,
    embedding_provider TEXT NOT NULL DEFAULT '',
    embedding_model TEXT NOT NULL DEFAULT '',
    embedding_dimensions INTEGER,
    search_profile TEXT NOT NULL DEFAULT '',
    rerank_enabled INTEGER NOT NULL DEFAULT 1,
    rerank_provider TEXT NOT NULL DEFAULT '',
    summary_enabled INTEGER NOT NULL DEFAULT 0,
    external_embedding_allowed INTEGER NOT NULL DEFAULT 1,
    external_rerank_allowed INTEGER NOT NULL DEFAULT 1,
    external_summary_allowed INTEGER NOT NULL DEFAULT 1,
    external_ocr_allowed INTEGER NOT NULL DEFAULT 1,
    redaction_enabled INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_kb_libraries_name ON kb_libraries(name);

CREATE TABLE IF NOT EXISTS kb_folders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    library_id INTEGER NOT NULL REFERENCES kb_libraries(id) ON DELETE CASCADE,
    parent_id INTEGER REFERENCES kb_folders(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_kb_folders_library_id ON kb_folders(library_id);
CREATE INDEX IF NOT EXISTS idx_kb_folders_parent_id ON kb_folders(parent_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_kb_folders_library_parent_name
    ON kb_folders(library_id, COALESCE(parent_id, -1), name);
"#;

/// Migration DDL for schema version 13: KB documents and document nodes
pub const MIGRATION_V13_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS kb_documents (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    library_id INTEGER NOT NULL REFERENCES kb_libraries(id) ON DELETE CASCADE,
    folder_id INTEGER REFERENCES kb_folders(id) ON DELETE SET NULL,
    original_name TEXT NOT NULL,
    name_tokens TEXT NOT NULL DEFAULT '',
    file_size INTEGER NOT NULL DEFAULT 0,
    content_hash TEXT NOT NULL,
    extension TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    source_type TEXT NOT NULL DEFAULT 'local',
    storage_path TEXT NOT NULL DEFAULT '',
    original_path TEXT NOT NULL DEFAULT '',
    job_id TEXT NOT NULL DEFAULT '',
    processing_run_id TEXT NOT NULL DEFAULT '',
    parsing_status INTEGER NOT NULL DEFAULT 0,
    parsing_progress INTEGER NOT NULL DEFAULT 0,
    parsing_error TEXT NOT NULL DEFAULT '',
    embedding_status INTEGER NOT NULL DEFAULT 0,
    embedding_progress INTEGER NOT NULL DEFAULT 0,
    embedding_error TEXT NOT NULL DEFAULT '',
    word_total INTEGER NOT NULL DEFAULT 0,
    split_total INTEGER NOT NULL DEFAULT 0,
    title TEXT NOT NULL DEFAULT '',
    summary TEXT NOT NULL DEFAULT '',
    keywords TEXT NOT NULL DEFAULT '',
    entity_names TEXT NOT NULL DEFAULT '',
    source_uri TEXT NOT NULL DEFAULT '',
    modified_at TEXT,
    document_date TEXT,
    normalized_content_hash TEXT NOT NULL DEFAULT '',
    simhash TEXT NOT NULL DEFAULT '',
    document_family_id TEXT,
    version_label TEXT NOT NULL DEFAULT '',
    document_granularity TEXT NOT NULL DEFAULT 'micro',
    content_char_count INTEGER NOT NULL DEFAULT 0,
    content_token_count INTEGER NOT NULL DEFAULT 0,
    page_count INTEGER NOT NULL DEFAULT 0,
    section_count INTEGER NOT NULL DEFAULT 0,
    chunk_strategy TEXT NOT NULL DEFAULT 'auto',
    document_status TEXT NOT NULL DEFAULT 'queued',
    index_status TEXT NOT NULL DEFAULT 'pending',
    current_version_id INTEGER,
    deleted_at TEXT,
    purged_at TEXT,
    last_indexed_at TEXT,
    last_seen_at TEXT,
    ocr_status TEXT NOT NULL DEFAULT 'not_needed',
    ocr_text_coverage REAL NOT NULL DEFAULT 0.0
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_kb_docs_library_hash
    ON kb_documents(library_id, content_hash) WHERE deleted_at IS NULL AND purged_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_kb_docs_library_id ON kb_documents(library_id);
CREATE INDEX IF NOT EXISTS idx_kb_docs_library_id_id ON kb_documents(library_id, id);
CREATE INDEX IF NOT EXISTS idx_kb_docs_document_status ON kb_documents(document_status);
CREATE INDEX IF NOT EXISTS idx_kb_docs_deleted_at ON kb_documents(deleted_at);
CREATE INDEX IF NOT EXISTS idx_kb_docs_granularity ON kb_documents(document_granularity);

CREATE TABLE IF NOT EXISTS kb_document_nodes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    library_id INTEGER NOT NULL REFERENCES kb_libraries(id) ON DELETE CASCADE,
    document_id INTEGER NOT NULL REFERENCES kb_documents(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    content_tokens TEXT NOT NULL DEFAULT '',
    level INTEGER NOT NULL DEFAULT 0,
    parent_id INTEGER REFERENCES kb_document_nodes(id) ON DELETE SET NULL,
    chunk_order INTEGER NOT NULL DEFAULT 0,
    section_id INTEGER,
    title_path TEXT NOT NULL DEFAULT '',
    page_number INTEGER,
    source_start INTEGER,
    source_end INTEGER,
    node_metadata TEXT NOT NULL DEFAULT '{}',
    embedding_text TEXT NOT NULL DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_kb_nodes_library_id ON kb_document_nodes(library_id);
CREATE INDEX IF NOT EXISTS idx_kb_nodes_document_id ON kb_document_nodes(document_id);
CREATE INDEX IF NOT EXISTS idx_kb_nodes_parent_id ON kb_document_nodes(parent_id);
CREATE INDEX IF NOT EXISTS idx_kb_nodes_level ON kb_document_nodes(level);
CREATE INDEX IF NOT EXISTS idx_kb_nodes_doc_level_order
    ON kb_document_nodes(document_id, level, chunk_order);
CREATE INDEX IF NOT EXISTS idx_kb_nodes_section_id ON kb_document_nodes(section_id);
CREATE INDEX IF NOT EXISTS idx_kb_nodes_page_number ON kb_document_nodes(page_number);

CREATE TABLE IF NOT EXISTS kb_node_embeddings (
    node_id INTEGER NOT NULL REFERENCES kb_document_nodes(id) ON DELETE CASCADE,
    embedding BLOB NOT NULL,
    dimensions INTEGER NOT NULL,
    model TEXT NOT NULL DEFAULT 'text-embedding-3-large',
    embedded_at TEXT NOT NULL DEFAULT (datetime('now')),
    embedding_index_id INTEGER NOT NULL REFERENCES kb_embedding_indexes(id) ON DELETE CASCADE,
    PRIMARY KEY (node_id, embedding_index_id)
);

CREATE INDEX IF NOT EXISTS idx_kb_node_emb_index_id ON kb_node_embeddings(embedding_index_id);

CREATE TABLE IF NOT EXISTS kb_document_versions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    document_id INTEGER NOT NULL REFERENCES kb_documents(id) ON DELETE CASCADE,
    version_label TEXT NOT NULL DEFAULT '',
    processing_run_id TEXT NOT NULL DEFAULT '',
    char_count INTEGER NOT NULL DEFAULT 0,
    node_count INTEGER NOT NULL DEFAULT 0,
    index_status TEXT NOT NULL DEFAULT 'pending'
);
CREATE INDEX IF NOT EXISTS idx_kb_doc_versions_doc ON kb_document_versions(document_id);

CREATE TABLE IF NOT EXISTS kb_document_sections (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    document_id INTEGER NOT NULL REFERENCES kb_documents(id) ON DELETE CASCADE,
    parent_section_id INTEGER REFERENCES kb_document_sections(id) ON DELETE SET NULL,
    title TEXT NOT NULL DEFAULT '',
    title_path TEXT NOT NULL DEFAULT '',
    heading_level INTEGER NOT NULL DEFAULT 0,
    section_order INTEGER NOT NULL DEFAULT 0,
    page_number INTEGER,
    source_start INTEGER,
    source_end INTEGER,
    content_summary TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_kb_sections_doc ON kb_document_sections(document_id);
CREATE INDEX IF NOT EXISTS idx_kb_sections_parent ON kb_document_sections(parent_section_id);

CREATE TABLE IF NOT EXISTS kb_document_summaries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    document_id INTEGER NOT NULL REFERENCES kb_documents(id) ON DELETE CASCADE,
    section_id INTEGER REFERENCES kb_document_sections(id) ON DELETE CASCADE,
    summary_type TEXT NOT NULL DEFAULT 'document',
    summary_text TEXT NOT NULL DEFAULT '',
    summary_tokens TEXT NOT NULL DEFAULT '',
    model TEXT NOT NULL DEFAULT '',
    UNIQUE(document_id, section_id, summary_type)
);
CREATE INDEX IF NOT EXISTS idx_kb_summaries_doc ON kb_document_summaries(document_id);

CREATE TABLE IF NOT EXISTS kb_search_eval_queries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    library_id INTEGER REFERENCES kb_libraries(id) ON DELETE CASCADE,
    query_text TEXT NOT NULL,
    query_type TEXT NOT NULL DEFAULT 'manual',
    expected_document_ids TEXT NOT NULL DEFAULT '[]',
    notes TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS kb_index_state (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    index_name TEXT NOT NULL UNIQUE,
    index_version INTEGER NOT NULL DEFAULT 1,
    index_type TEXT NOT NULL DEFAULT 'vector',
    dimensions INTEGER,
    model TEXT NOT NULL DEFAULT '',
    state TEXT NOT NULL DEFAULT 'active',
    doc_count INTEGER NOT NULL DEFAULT 0,
    last_rebuilt_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS kb_embedding_indexes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id INTEGER NOT NULL REFERENCES kb_libraries(id) ON DELETE CASCADE,
    provider TEXT NOT NULL DEFAULT '',
    model TEXT NOT NULL,
    dimensions INTEGER NOT NULL,
    index_type TEXT NOT NULL DEFAULT 'vec0',
    is_active INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_kb_emb_idx_library ON kb_embedding_indexes(library_id);

CREATE TABLE IF NOT EXISTS kb_search_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    query_normalized TEXT NOT NULL DEFAULT '',
    library_ids TEXT NOT NULL DEFAULT '[]',
    profile TEXT NOT NULL DEFAULT '',
    planner_type TEXT NOT NULL DEFAULT '',
    result_count INTEGER NOT NULL DEFAULT 0,
    latency_ms INTEGER NOT NULL DEFAULT 0,
    cache_hit INTEGER NOT NULL DEFAULT 0,
    debug_mode INTEGER NOT NULL DEFAULT 0,
    embedding_index_id INTEGER,
    result_document_ids TEXT NOT NULL DEFAULT '[]'
);

CREATE TABLE IF NOT EXISTS kb_search_feedback (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    search_log_id INTEGER REFERENCES kb_search_logs(id) ON DELETE SET NULL,
    document_id INTEGER REFERENCES kb_documents(id) ON DELETE SET NULL,
    node_id INTEGER REFERENCES kb_document_nodes(id) ON DELETE SET NULL,
    rating INTEGER NOT NULL DEFAULT 0,
    comment TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS kb_tables (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    document_id INTEGER NOT NULL REFERENCES kb_documents(id) ON DELETE CASCADE,
    sheet_name TEXT NOT NULL DEFAULT '',
    headers TEXT NOT NULL DEFAULT '[]',
    column_count INTEGER NOT NULL DEFAULT 0,
    row_count INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_kb_tables_doc ON kb_tables(document_id);

CREATE TABLE IF NOT EXISTS kb_table_rows (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    table_id INTEGER NOT NULL REFERENCES kb_tables(id) ON DELETE CASCADE,
    row_index INTEGER NOT NULL DEFAULT 0,
    row_text TEXT NOT NULL DEFAULT '',
    row_tokens TEXT NOT NULL DEFAULT '',
    row_json TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX IF NOT EXISTS idx_kb_table_rows_table ON kb_table_rows(table_id);

CREATE TABLE IF NOT EXISTS kb_external_model_calls (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    library_id INTEGER REFERENCES kb_libraries(id) ON DELETE SET NULL,
    document_id INTEGER REFERENCES kb_documents(id) ON DELETE SET NULL,
    call_type TEXT NOT NULL DEFAULT '',
    provider TEXT NOT NULL DEFAULT '',
    model TEXT NOT NULL DEFAULT '',
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    latency_ms INTEGER NOT NULL DEFAULT 0,
    cost_estimate REAL NOT NULL DEFAULT 0.0,
    success INTEGER NOT NULL DEFAULT 1,
    error_message TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_kb_ext_calls_library ON kb_external_model_calls(library_id);

CREATE TABLE IF NOT EXISTS kb_sources (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    library_id INTEGER NOT NULL REFERENCES kb_libraries(id) ON DELETE CASCADE,
    source_type TEXT NOT NULL DEFAULT 'local',
    source_uri TEXT NOT NULL DEFAULT '',
    display_name TEXT NOT NULL DEFAULT '',
    connector_config TEXT NOT NULL DEFAULT '{}',
    delete_policy TEXT NOT NULL DEFAULT 'mark_only',
    sync_status TEXT NOT NULL DEFAULT 'idle',
    last_synced_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_kb_sources_library ON kb_sources(library_id);

CREATE TABLE IF NOT EXISTS kb_source_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    source_id INTEGER NOT NULL REFERENCES kb_sources(id) ON DELETE CASCADE,
    document_id INTEGER REFERENCES kb_documents(id) ON DELETE SET NULL,
    external_id TEXT NOT NULL DEFAULT '',
    item_path TEXT NOT NULL DEFAULT '',
    content_hash TEXT NOT NULL DEFAULT '',
    file_size INTEGER NOT NULL DEFAULT 0,
    last_seen_at TEXT,
    sync_status TEXT NOT NULL DEFAULT 'pending',
    sync_error TEXT NOT NULL DEFAULT '',
    UNIQUE(source_id, item_path)
);
CREATE INDEX IF NOT EXISTS idx_kb_source_items_source ON kb_source_items(source_id);
"#;

/// Migration DDL for schema version 14: KB FTS5 virtual tables + triggers + vec0
pub const MIGRATION_V14_DDL: &str = r#"
CREATE VIRTUAL TABLE IF NOT EXISTS kb_doc_fts USING fts5(
    tokens,
    library_id,
    document_id UNINDEXED,
    level UNINDEXED,
    content='',
    tokenize='unicode61'
);

CREATE TRIGGER IF NOT EXISTS kb_nodes_fts_insert
AFTER INSERT ON kb_document_nodes BEGIN
    INSERT INTO kb_doc_fts(rowid, tokens, library_id, document_id, level)
    VALUES (new.id, new.content_tokens, new.library_id, new.document_id, new.level);
END;

CREATE TRIGGER IF NOT EXISTS kb_nodes_fts_update
AFTER UPDATE OF content_tokens, library_id, document_id, level ON kb_document_nodes BEGIN
    INSERT INTO kb_doc_fts(kb_doc_fts, rowid, tokens, library_id, document_id, level)
    VALUES('delete', old.id, old.content_tokens, old.library_id, old.document_id, old.level);
    INSERT INTO kb_doc_fts(rowid, tokens, library_id, document_id, level)
    VALUES (new.id, new.content_tokens, new.library_id, new.document_id, new.level);
END;

CREATE TRIGGER IF NOT EXISTS kb_nodes_fts_delete
AFTER DELETE ON kb_document_nodes BEGIN
    INSERT INTO kb_doc_fts(kb_doc_fts, rowid, tokens, library_id, document_id, level)
    VALUES('delete', old.id, old.content_tokens, old.library_id, old.document_id, old.level);
END;

CREATE VIRTUAL TABLE IF NOT EXISTS kb_doc_name_fts USING fts5(
    name_tokens,
    library_id,
    document_id UNINDEXED,
    content='',
    tokenize='unicode61'
);

CREATE TRIGGER IF NOT EXISTS kb_docs_fts_insert
AFTER INSERT ON kb_documents BEGIN
    INSERT INTO kb_doc_name_fts(rowid, name_tokens, library_id, document_id)
    VALUES (new.id, new.name_tokens, new.library_id, new.id);
END;

CREATE TRIGGER IF NOT EXISTS kb_docs_fts_delete
AFTER DELETE ON kb_documents BEGIN
    INSERT INTO kb_doc_name_fts(kb_doc_name_fts, rowid, name_tokens, library_id, document_id)
    VALUES('delete', old.id, old.name_tokens, old.library_id, old.id);
END;

CREATE TRIGGER IF NOT EXISTS kb_docs_fts_update
AFTER UPDATE OF name_tokens, library_id ON kb_documents BEGIN
    INSERT INTO kb_doc_name_fts(kb_doc_name_fts, rowid, name_tokens, library_id, document_id)
    VALUES('delete', old.id, old.name_tokens, old.library_id, old.id);
    INSERT INTO kb_doc_name_fts(rowid, name_tokens, library_id, document_id)
    VALUES (new.id, new.name_tokens, new.library_id, new.id);
END;
"#;

/// Migration DDL for schema version 15: KB embedding fallback table
pub const MIGRATION_V15_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS kb_node_embeddings (
    node_id INTEGER PRIMARY KEY REFERENCES kb_document_nodes(id) ON DELETE CASCADE,
    embedding BLOB NOT NULL,
    dimensions INTEGER NOT NULL,
    model TEXT NOT NULL DEFAULT 'text-embedding-3-large',
    embedded_at TEXT NOT NULL DEFAULT (datetime('now'))
);
"#;

/// 数据库迁移 V16：中文 NLP 预分词 FTS5
///
/// 为 pages 和 chunks 表添加 _tokens 列，重建 pages_fts 和 chunks_fts，
/// 使用原始列与分词列组合 + unicode61 分词器。
pub const MIGRATION_V16_DDL: &str = r#"
-- 为中文 FTS5 支持添加预分词列
ALTER TABLE pages ADD COLUMN title_tokens TEXT NOT NULL DEFAULT '';
ALTER TABLE pages ADD COLUMN compiled_truth_tokens TEXT NOT NULL DEFAULT '';
ALTER TABLE pages ADD COLUMN timeline_tokens TEXT NOT NULL DEFAULT '';
ALTER TABLE chunks ADD COLUMN chunk_text_tokens TEXT NOT NULL DEFAULT '';

-- 重建 pages_fts，添加分词列 + unicode61 分词器
DROP TRIGGER IF EXISTS pages_fts_insert;
DROP TRIGGER IF EXISTS pages_fts_update;
DROP TRIGGER IF EXISTS pages_fts_delete;
DROP TABLE IF EXISTS pages_fts;

CREATE VIRTUAL TABLE pages_fts USING fts5(
    slug,
    title,
    title_tokens,
    compiled_truth,
    compiled_truth_tokens,
    timeline,
    timeline_tokens,
    content='pages',
    content_rowid='id',
    tokenize='unicode61'
);

CREATE TRIGGER pages_fts_insert AFTER INSERT ON pages BEGIN
    INSERT INTO pages_fts(rowid, slug, title, title_tokens, compiled_truth, compiled_truth_tokens, timeline, timeline_tokens)
    VALUES (new.id, new.slug, new.title, new.title_tokens, new.compiled_truth, new.compiled_truth_tokens, new.timeline, new.timeline_tokens);
END;

CREATE TRIGGER pages_fts_update AFTER UPDATE ON pages BEGIN
    INSERT INTO pages_fts(pages_fts, rowid, slug, title, title_tokens, compiled_truth, compiled_truth_tokens, timeline, timeline_tokens)
    VALUES ('delete', old.id, old.slug, old.title, old.title_tokens, old.compiled_truth, old.compiled_truth_tokens, old.timeline, old.timeline_tokens);
    INSERT INTO pages_fts(rowid, slug, title, title_tokens, compiled_truth, compiled_truth_tokens, timeline, timeline_tokens)
    VALUES (new.id, new.slug, new.title, new.title_tokens, new.compiled_truth, new.compiled_truth_tokens, new.timeline, new.timeline_tokens);
END;

CREATE TRIGGER pages_fts_delete AFTER DELETE ON pages BEGIN
    INSERT INTO pages_fts(pages_fts, rowid, slug, title, title_tokens, compiled_truth, compiled_truth_tokens, timeline, timeline_tokens)
    VALUES ('delete', old.id, old.slug, old.title, old.title_tokens, old.compiled_truth, old.compiled_truth_tokens, old.timeline, old.timeline_tokens);
END;

-- 重建 chunks_fts，添加分词列 + unicode61 分词器
DROP TRIGGER IF EXISTS chunks_fts_insert;
DROP TRIGGER IF EXISTS chunks_fts_update;
DROP TRIGGER IF EXISTS chunks_fts_delete;
DROP TABLE IF EXISTS chunks_fts;

CREATE VIRTUAL TABLE chunks_fts USING fts5(
    chunk_text,
    chunk_text_tokens,
    language,
    symbol_name,
    symbol_type,
    content='chunks',
    content_rowid='id',
    tokenize='unicode61'
);

CREATE TRIGGER chunks_fts_insert AFTER INSERT ON chunks BEGIN
    INSERT INTO chunks_fts(rowid, chunk_text, chunk_text_tokens, language, symbol_name, symbol_type)
    VALUES (new.id, new.chunk_text, new.chunk_text_tokens,
            coalesce(new.language, ''), coalesce(new.symbol_name, ''), coalesce(new.symbol_type, ''));
END;

CREATE TRIGGER chunks_fts_update AFTER UPDATE ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, chunk_text, chunk_text_tokens, language, symbol_name, symbol_type)
    VALUES ('delete', old.id, old.chunk_text, old.chunk_text_tokens,
            coalesce(old.language, ''), coalesce(old.symbol_name, ''), coalesce(old.symbol_type, ''));
    INSERT INTO chunks_fts(rowid, chunk_text, chunk_text_tokens, language, symbol_name, symbol_type)
    VALUES (new.id, new.chunk_text, new.chunk_text_tokens,
            coalesce(new.language, ''), coalesce(new.symbol_name, ''), coalesce(new.symbol_type, ''));
END;

CREATE TRIGGER chunks_fts_delete AFTER DELETE ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, chunk_text, chunk_text_tokens, language, symbol_name, symbol_type)
    VALUES ('delete', old.id, old.chunk_text, old.chunk_text_tokens,
            coalesce(old.language, ''), coalesce(old.symbol_name, ''), coalesce(old.symbol_type, ''));
END;
"#;

/// Generate sqlite-vec virtual table DDL for KB nodes
pub fn vec_kb_nodes_ddl(dimensions: usize) -> String {
    format!(
        r#"CREATE VIRTUAL TABLE IF NOT EXISTS vec_kb_nodes USING vec0(
    node_id INTEGER PRIMARY KEY,
    embedding float[{}]
);"#,
        dimensions
    )
}

/// 数据库迁移 V17：KB P0 Foundation — 扩展 kb_documents/kb_document_nodes/kb_libraries + 新增 13 张表
///
/// 为 KB 子系统补齐核心字段和表结构，支持文档分级、生命周期、多 Embedding 索引、
/// 搜索评测、表格索引、导入源追踪、外部模型审计等能力。
pub const MIGRATION_V17_DDL: &str = r#"
-- ============================================================================
-- kb_documents 扩展：增加 25 个新字段
-- ============================================================================
ALTER TABLE kb_documents ADD COLUMN title TEXT NOT NULL DEFAULT '';
ALTER TABLE kb_documents ADD COLUMN summary TEXT NOT NULL DEFAULT '';
ALTER TABLE kb_documents ADD COLUMN keywords TEXT NOT NULL DEFAULT '';
ALTER TABLE kb_documents ADD COLUMN entity_names TEXT NOT NULL DEFAULT '';
ALTER TABLE kb_documents ADD COLUMN source_uri TEXT NOT NULL DEFAULT '';
ALTER TABLE kb_documents ADD COLUMN modified_at TEXT;
ALTER TABLE kb_documents ADD COLUMN document_date TEXT;
ALTER TABLE kb_documents ADD COLUMN normalized_content_hash TEXT NOT NULL DEFAULT '';
ALTER TABLE kb_documents ADD COLUMN simhash TEXT NOT NULL DEFAULT '';
ALTER TABLE kb_documents ADD COLUMN document_family_id TEXT;
ALTER TABLE kb_documents ADD COLUMN version_label TEXT NOT NULL DEFAULT '';
ALTER TABLE kb_documents ADD COLUMN document_granularity TEXT NOT NULL DEFAULT 'micro';
ALTER TABLE kb_documents ADD COLUMN content_char_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE kb_documents ADD COLUMN content_token_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE kb_documents ADD COLUMN page_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE kb_documents ADD COLUMN section_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE kb_documents ADD COLUMN chunk_strategy TEXT NOT NULL DEFAULT 'auto';
ALTER TABLE kb_documents ADD COLUMN document_status TEXT NOT NULL DEFAULT 'queued';
ALTER TABLE kb_documents ADD COLUMN index_status TEXT NOT NULL DEFAULT 'pending';
ALTER TABLE kb_documents ADD COLUMN current_version_id INTEGER;
ALTER TABLE kb_documents ADD COLUMN deleted_at TEXT;
ALTER TABLE kb_documents ADD COLUMN purged_at TEXT;
ALTER TABLE kb_documents ADD COLUMN last_indexed_at TEXT;
ALTER TABLE kb_documents ADD COLUMN last_seen_at TEXT;

CREATE INDEX IF NOT EXISTS idx_kb_docs_document_status ON kb_documents(document_status);
CREATE INDEX IF NOT EXISTS idx_kb_docs_deleted_at ON kb_documents(deleted_at);
CREATE INDEX IF NOT EXISTS idx_kb_docs_granularity ON kb_documents(document_granularity);

-- ============================================================================
-- kb_document_nodes 扩展：增加 7 个新字段
-- ============================================================================
ALTER TABLE kb_document_nodes ADD COLUMN section_id INTEGER;
ALTER TABLE kb_document_nodes ADD COLUMN title_path TEXT NOT NULL DEFAULT '';
ALTER TABLE kb_document_nodes ADD COLUMN page_number INTEGER;
ALTER TABLE kb_document_nodes ADD COLUMN source_start INTEGER;
ALTER TABLE kb_document_nodes ADD COLUMN source_end INTEGER;
ALTER TABLE kb_document_nodes ADD COLUMN node_metadata TEXT NOT NULL DEFAULT '{}';
ALTER TABLE kb_document_nodes ADD COLUMN embedding_text TEXT NOT NULL DEFAULT '';

CREATE INDEX IF NOT EXISTS idx_kb_nodes_section_id ON kb_document_nodes(section_id);
CREATE INDEX IF NOT EXISTS idx_kb_nodes_page_number ON kb_document_nodes(page_number);

-- ============================================================================
-- kb_libraries 扩展：增加 11 个治理和模型配置字段
-- ============================================================================
ALTER TABLE kb_libraries ADD COLUMN embedding_provider TEXT NOT NULL DEFAULT '';
ALTER TABLE kb_libraries ADD COLUMN embedding_model TEXT NOT NULL DEFAULT '';
ALTER TABLE kb_libraries ADD COLUMN embedding_dimensions INTEGER;
ALTER TABLE kb_libraries ADD COLUMN search_profile TEXT NOT NULL DEFAULT '';
ALTER TABLE kb_libraries ADD COLUMN rerank_enabled INTEGER NOT NULL DEFAULT 1;
ALTER TABLE kb_libraries ADD COLUMN rerank_provider TEXT NOT NULL DEFAULT '';
ALTER TABLE kb_libraries ADD COLUMN summary_enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE kb_libraries ADD COLUMN external_embedding_allowed INTEGER NOT NULL DEFAULT 1;
ALTER TABLE kb_libraries ADD COLUMN external_rerank_allowed INTEGER NOT NULL DEFAULT 1;
ALTER TABLE kb_libraries ADD COLUMN external_summary_allowed INTEGER NOT NULL DEFAULT 1;
ALTER TABLE kb_libraries ADD COLUMN external_ocr_allowed INTEGER NOT NULL DEFAULT 1;
ALTER TABLE kb_libraries ADD COLUMN redaction_enabled INTEGER NOT NULL DEFAULT 0;

-- ============================================================================
-- 新增 13 张表
-- ============================================================================

-- 1. 文档版本表
CREATE TABLE IF NOT EXISTS kb_document_versions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    document_id INTEGER NOT NULL REFERENCES kb_documents(id) ON DELETE CASCADE,
    version_label TEXT NOT NULL DEFAULT '',
    processing_run_id TEXT NOT NULL DEFAULT '',
    char_count INTEGER NOT NULL DEFAULT 0,
    node_count INTEGER NOT NULL DEFAULT 0,
    index_status TEXT NOT NULL DEFAULT 'pending'
);
CREATE INDEX IF NOT EXISTS idx_kb_doc_versions_doc ON kb_document_versions(document_id);

-- 2. 文档章节表
CREATE TABLE IF NOT EXISTS kb_document_sections (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    document_id INTEGER NOT NULL REFERENCES kb_documents(id) ON DELETE CASCADE,
    parent_section_id INTEGER REFERENCES kb_document_sections(id) ON DELETE SET NULL,
    title TEXT NOT NULL DEFAULT '',
    title_path TEXT NOT NULL DEFAULT '',
    heading_level INTEGER NOT NULL DEFAULT 0,
    section_order INTEGER NOT NULL DEFAULT 0,
    page_number INTEGER,
    source_start INTEGER,
    source_end INTEGER,
    content_summary TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_kb_sections_doc ON kb_document_sections(document_id);
CREATE INDEX IF NOT EXISTS idx_kb_sections_parent ON kb_document_sections(parent_section_id);

-- 3. 文档摘要表
CREATE TABLE IF NOT EXISTS kb_document_summaries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    document_id INTEGER NOT NULL REFERENCES kb_documents(id) ON DELETE CASCADE,
    section_id INTEGER REFERENCES kb_document_sections(id) ON DELETE CASCADE,
    summary_type TEXT NOT NULL DEFAULT 'document',
    summary_text TEXT NOT NULL DEFAULT '',
    summary_tokens TEXT NOT NULL DEFAULT '',
    model TEXT NOT NULL DEFAULT '',
    UNIQUE(document_id, section_id, summary_type)
);
CREATE INDEX IF NOT EXISTS idx_kb_summaries_doc ON kb_document_summaries(document_id);

-- 4. 搜索评测查询集
CREATE TABLE IF NOT EXISTS kb_search_eval_queries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    library_id INTEGER REFERENCES kb_libraries(id) ON DELETE CASCADE,
    query_text TEXT NOT NULL,
    query_type TEXT NOT NULL DEFAULT 'manual',
    expected_document_ids TEXT NOT NULL DEFAULT '[]',
    notes TEXT NOT NULL DEFAULT ''
);

-- 5. 索引状态表（驱动缓存失效和增量重建）
CREATE TABLE IF NOT EXISTS kb_index_state (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    index_name TEXT NOT NULL UNIQUE,
    index_version INTEGER NOT NULL DEFAULT 1,
    index_type TEXT NOT NULL DEFAULT 'vector',
    dimensions INTEGER,
    model TEXT NOT NULL DEFAULT '',
    state TEXT NOT NULL DEFAULT 'active',
    doc_count INTEGER NOT NULL DEFAULT 0,
    last_rebuilt_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 6. Embedding 索引注册表（多模型并存）
CREATE TABLE IF NOT EXISTS kb_embedding_indexes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id INTEGER NOT NULL REFERENCES kb_libraries(id) ON DELETE CASCADE,
    provider TEXT NOT NULL DEFAULT '',
    model TEXT NOT NULL,
    dimensions INTEGER NOT NULL,
    index_type TEXT NOT NULL DEFAULT 'vec0',
    is_active INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_kb_emb_idx_library ON kb_embedding_indexes(library_id);

-- 7. 搜索日志表
CREATE TABLE IF NOT EXISTS kb_search_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    query_normalized TEXT NOT NULL DEFAULT '',
    library_ids TEXT NOT NULL DEFAULT '[]',
    profile TEXT NOT NULL DEFAULT '',
    planner_type TEXT NOT NULL DEFAULT '',
    result_count INTEGER NOT NULL DEFAULT 0,
    latency_ms INTEGER NOT NULL DEFAULT 0,
    cache_hit INTEGER NOT NULL DEFAULT 0,
    debug_mode INTEGER NOT NULL DEFAULT 0,
    embedding_index_id INTEGER,
    result_document_ids TEXT NOT NULL DEFAULT '[]'
);

-- 8. 搜索反馈表
CREATE TABLE IF NOT EXISTS kb_search_feedback (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    search_log_id INTEGER REFERENCES kb_search_logs(id) ON DELETE SET NULL,
    document_id INTEGER REFERENCES kb_documents(id) ON DELETE SET NULL,
    node_id INTEGER REFERENCES kb_document_nodes(id) ON DELETE SET NULL,
    rating INTEGER NOT NULL DEFAULT 0,
    comment TEXT NOT NULL DEFAULT ''
);

-- 9. 表格元数据表
CREATE TABLE IF NOT EXISTS kb_tables (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    document_id INTEGER NOT NULL REFERENCES kb_documents(id) ON DELETE CASCADE,
    sheet_name TEXT NOT NULL DEFAULT '',
    headers TEXT NOT NULL DEFAULT '[]',
    column_count INTEGER NOT NULL DEFAULT 0,
    row_count INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_kb_tables_doc ON kb_tables(document_id);

-- 10. 表格行数据表
CREATE TABLE IF NOT EXISTS kb_table_rows (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    table_id INTEGER NOT NULL REFERENCES kb_tables(id) ON DELETE CASCADE,
    row_index INTEGER NOT NULL DEFAULT 0,
    row_text TEXT NOT NULL DEFAULT '',
    row_tokens TEXT NOT NULL DEFAULT '',
    row_json TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX IF NOT EXISTS idx_kb_table_rows_table ON kb_table_rows(table_id);

-- 11. 外部模型调用审计表
CREATE TABLE IF NOT EXISTS kb_external_model_calls (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    library_id INTEGER REFERENCES kb_libraries(id) ON DELETE SET NULL,
    document_id INTEGER REFERENCES kb_documents(id) ON DELETE SET NULL,
    call_type TEXT NOT NULL DEFAULT '',
    provider TEXT NOT NULL DEFAULT '',
    model TEXT NOT NULL DEFAULT '',
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    latency_ms INTEGER NOT NULL DEFAULT 0,
    cost_estimate REAL NOT NULL DEFAULT 0.0,
    success INTEGER NOT NULL DEFAULT 1,
    error_message TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_kb_ext_calls_library ON kb_external_model_calls(library_id);

-- 12. 导入源表
CREATE TABLE IF NOT EXISTS kb_sources (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    library_id INTEGER NOT NULL REFERENCES kb_libraries(id) ON DELETE CASCADE,
    source_type TEXT NOT NULL DEFAULT 'local',
    source_uri TEXT NOT NULL DEFAULT '',
    display_name TEXT NOT NULL DEFAULT '',
    connector_config TEXT NOT NULL DEFAULT '{}',
    delete_policy TEXT NOT NULL DEFAULT 'mark_only',
    sync_status TEXT NOT NULL DEFAULT 'idle',
    last_synced_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_kb_sources_library ON kb_sources(library_id);

-- 13. 导入源条目表（增量同步追踪）
CREATE TABLE IF NOT EXISTS kb_source_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    source_id INTEGER NOT NULL REFERENCES kb_sources(id) ON DELETE CASCADE,
    document_id INTEGER REFERENCES kb_documents(id) ON DELETE SET NULL,
    external_id TEXT NOT NULL DEFAULT '',
    item_path TEXT NOT NULL DEFAULT '',
    content_hash TEXT NOT NULL DEFAULT '',
    file_size INTEGER NOT NULL DEFAULT 0,
    last_seen_at TEXT,
    sync_status TEXT NOT NULL DEFAULT 'pending',
    sync_error TEXT NOT NULL DEFAULT '',
    UNIQUE(source_id, item_path)
);
CREATE INDEX IF NOT EXISTS idx_kb_source_items_source ON kb_source_items(source_id);
"#;

/// 数据库迁移 V18：KB P2-019 — OCR 回写字段
///
/// 为 kb_documents 增加 OCR 状态和文本覆盖率字段，支持 OCR 结果回写后更新文档状态。
pub const MIGRATION_V18_DDL: &str = r#"
ALTER TABLE kb_documents ADD COLUMN ocr_status TEXT NOT NULL DEFAULT 'not_needed';
ALTER TABLE kb_documents ADD COLUMN ocr_text_coverage REAL NOT NULL DEFAULT 0.0;
"#;

/// 数据库迁移 V19：KB P5-011~014 — embedding_index_id + per-index vec tables + reembed job + eval comparison
///
/// 1. Add embedding_index_id column to kb_node_embeddings (NULL for backward compat)
/// 2. Add index on embedding_index_id for search filtering
/// 3. Migrate existing rows: assign NULL rows to the default (first active) index per library
pub const MIGRATION_V19_DDL: &str = r#"
-- P5-011: Add embedding_index_id to kb_node_embeddings
ALTER TABLE kb_node_embeddings ADD COLUMN embedding_index_id INTEGER REFERENCES kb_embedding_indexes(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_kb_node_emb_index_id ON kb_node_embeddings(embedding_index_id);

-- Backfill: assign existing NULL rows to the active index of their library (if one exists)
-- For rows whose library has no active index yet, leave NULL (backward compat)
UPDATE kb_node_embeddings
SET embedding_index_id = (
    SELECT ei.id FROM kb_embedding_indexes ei
    INNER JOIN kb_document_nodes dn ON dn.id = kb_node_embeddings.node_id
    INNER JOIN kb_documents d ON d.id = dn.document_id
    WHERE ei.library_id = d.library_id AND ei.is_active = 1
    LIMIT 1
)
WHERE embedding_index_id IS NULL
AND EXISTS (
    SELECT 1 FROM kb_embedding_indexes ei
    INNER JOIN kb_document_nodes dn ON dn.id = kb_node_embeddings.node_id
    INNER JOIN kb_documents d ON d.id = dn.document_id
    WHERE ei.library_id = d.library_id AND ei.is_active = 1
);
"#;

/// 数据库迁移 V20：kb_node_embeddings 复合主键 (node_id, embedding_index_id)
///
/// V19 新增了 embedding_index_id 列但 PK 仍是 node_id，导致 INSERT OR REPLACE
/// 对同一 node 的不同 index 会互相覆盖。改为复合主键后，同一 node 可以拥有
/// 多条 embedding 记录（每条对应不同的 embedding_index_id）。
/// 不再使用 0 作为默认/哨兵 index；新建库时自动创建 active embedding index。
pub const MIGRATION_V20_DDL: &str = r#"
-- 回填 NULL embedding_index_id：通过所属 library 的 active index 解析
UPDATE kb_node_embeddings
SET embedding_index_id = (
    SELECT ei.id FROM kb_embedding_indexes ei
    INNER JOIN kb_document_nodes dn ON dn.id = kb_node_embeddings.node_id
    INNER JOIN kb_documents d ON d.id = dn.document_id
    WHERE ei.library_id = d.library_id AND ei.is_active = 1
    LIMIT 1
)
WHERE embedding_index_id IS NULL;

-- 无法解析的剩余 NULL 行：该 library 无 active index，删除孤立 embedding
DELETE FROM kb_node_embeddings WHERE embedding_index_id IS NULL;

-- 重建表：复合主键 (node_id, embedding_index_id)，FK 级联删除
CREATE TABLE kb_node_embeddings_v20 (
    node_id INTEGER NOT NULL REFERENCES kb_document_nodes(id) ON DELETE CASCADE,
    embedding BLOB NOT NULL,
    dimensions INTEGER NOT NULL,
    model TEXT NOT NULL DEFAULT 'text-embedding-3-large',
    embedded_at TEXT NOT NULL DEFAULT (datetime('now')),
    embedding_index_id INTEGER NOT NULL REFERENCES kb_embedding_indexes(id) ON DELETE CASCADE,
    PRIMARY KEY (node_id, embedding_index_id)
);

INSERT INTO kb_node_embeddings_v20 SELECT * FROM kb_node_embeddings;
DROP TABLE kb_node_embeddings;
ALTER TABLE kb_node_embeddings_v20 RENAME TO kb_node_embeddings;

CREATE INDEX IF NOT EXISTS idx_kb_node_emb_index_id ON kb_node_embeddings(embedding_index_id);
"#;

/// V21: 为 kb_source_items 添加 file_size 列；为 kb_search_logs 添加 embedding_index_id 和 result_document_ids 列
pub const MIGRATION_V21_DDL: &str = r#"
ALTER TABLE kb_source_items ADD COLUMN file_size INTEGER NOT NULL DEFAULT 0;
ALTER TABLE kb_search_logs ADD COLUMN embedding_index_id INTEGER;
ALTER TABLE kb_search_logs ADD COLUMN result_document_ids TEXT NOT NULL DEFAULT '[]';
"#;

/// V22: 将 idx_kb_docs_library_hash 从全量唯一索引改为部分唯一索引，
/// 仅对未软删除且未清除的记录生效，允许相同内容在软删除/清除后重新上传
pub const MIGRATION_V22_DDL: &str = r#"
-- 删除旧的全量唯一索引
DROP INDEX IF EXISTS idx_kb_docs_library_hash;
-- 创建部分唯一索引：仅对未删除且未清除的记录强制唯一性
CREATE UNIQUE INDEX IF NOT EXISTS idx_kb_docs_library_hash
    ON kb_documents(library_id, content_hash) WHERE deleted_at IS NULL AND purged_at IS NULL;
"#;

/// V23: 单入口多投影融合架构 — 新增 5 张表
///
/// source_artifacts: 原件内容对象，按 sha256 去重
/// artifact_occurrences: 每次上传/同步/关联事件
/// artifact_projections: artifact 到 KB/brain/file 投影的映射
/// promotion_candidates: 从 KB 证据抽取的候选变更
/// provenance_ledger: gbrain 事实与 KB 证据的来源追溯
pub const MIGRATION_V23_DDL: &str = r#"
-- ============================================================================
-- 单入口多投影融合架构：新增 5 张核心表
-- ============================================================================

-- 1. 原件内容对象（按 sha256 去重，不等同于 KB 文档或 gbrain page）
CREATE TABLE IF NOT EXISTS source_artifacts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    artifact_uid TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_seen_at TEXT,

    sha256 TEXT NOT NULL,
    original_name TEXT NOT NULL DEFAULT '',
    extension TEXT NOT NULL DEFAULT '',
    mime_type TEXT NOT NULL DEFAULT '',
    size_bytes INTEGER NOT NULL DEFAULT 0,

    storage_path TEXT NOT NULL DEFAULT '',
    canonical_slug TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'active',
    metadata_json TEXT NOT NULL DEFAULT '{}',

    deleted_at TEXT,
    purged_at TEXT
);

-- sha256 唯一索引（仅对未清除的记录）
CREATE UNIQUE INDEX IF NOT EXISTS idx_source_artifacts_sha256
    ON source_artifacts(sha256) WHERE purged_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_source_artifacts_slug ON source_artifacts(canonical_slug);
CREATE INDEX IF NOT EXISTS idx_source_artifacts_status ON source_artifacts(status);

-- 2. 上传/同步/关联事件（保存每次上传的上下文）
CREATE TABLE IF NOT EXISTS artifact_occurrences (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    occurrence_uid TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),

    artifact_id INTEGER NOT NULL REFERENCES source_artifacts(id) ON DELETE CASCADE,
    source_kind TEXT NOT NULL DEFAULT 'upload',
    source_uri TEXT NOT NULL DEFAULT '',
    original_path TEXT NOT NULL DEFAULT '',
    original_name TEXT NOT NULL DEFAULT '',
    owner_ref TEXT NOT NULL DEFAULT '',

    intent TEXT NOT NULL DEFAULT 'auto',
    target_slug TEXT NOT NULL DEFAULT '',
    page_slug TEXT NOT NULL DEFAULT '',
    library_id INTEGER,
    folder_id INTEGER,
    promotion_policy TEXT NOT NULL DEFAULT 'candidate',

    status TEXT NOT NULL DEFAULT 'active',
    stale_reason TEXT NOT NULL DEFAULT '',
    metadata_json TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_artifact_occ_artifact ON artifact_occurrences(artifact_id);
CREATE INDEX IF NOT EXISTS idx_artifact_occ_source ON artifact_occurrences(source_kind, source_uri);
CREATE INDEX IF NOT EXISTS idx_artifact_occ_target ON artifact_occurrences(target_slug);
CREATE INDEX IF NOT EXISTS idx_artifact_occ_page ON artifact_occurrences(page_slug);

-- 3. 投影映射（artifact 到 KB/brain/file 的投影记录）
CREATE TABLE IF NOT EXISTS artifact_projections (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),

    artifact_id INTEGER NOT NULL REFERENCES source_artifacts(id) ON DELETE CASCADE,
    occurrence_id INTEGER REFERENCES artifact_occurrences(id) ON DELETE SET NULL,
    projection_type TEXT NOT NULL,
    projection_key TEXT NOT NULL DEFAULT '',
    projection_ref TEXT NOT NULL DEFAULT '',

    status TEXT NOT NULL DEFAULT 'active',
    version_hash TEXT NOT NULL DEFAULT '',
    stale_reason TEXT NOT NULL DEFAULT '',
    metadata_json TEXT NOT NULL DEFAULT '{}',
    superseded_by INTEGER REFERENCES artifact_projections(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_artifact_proj_artifact ON artifact_projections(artifact_id);
CREATE INDEX IF NOT EXISTS idx_artifact_proj_occurrence ON artifact_projections(occurrence_id);
CREATE INDEX IF NOT EXISTS idx_artifact_proj_type ON artifact_projections(projection_type);
CREATE INDEX IF NOT EXISTS idx_artifact_proj_ref ON artifact_projections(projection_ref);
CREATE INDEX IF NOT EXISTS idx_artifact_proj_superseded ON artifact_projections(superseded_by);
-- partial unique index：仅 status='active' 时保证同一 key 唯一。
-- 历史行（superseded/stale/orphaned）不参与唯一约束，可自由共存。
-- 这允许 insert_projection 先标旧行 superseded 再插新 active，不会撞约束。
CREATE UNIQUE INDEX IF NOT EXISTS idx_artifact_proj_active_unique
    ON artifact_projections(artifact_id, projection_type, projection_key)
    WHERE status = 'active';

-- 4. 候选变更（从 KB 证据抽取，默认进入 review）
CREATE TABLE IF NOT EXISTS promotion_candidates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),

    artifact_id INTEGER NOT NULL REFERENCES source_artifacts(id) ON DELETE CASCADE,
    occurrence_id INTEGER REFERENCES artifact_occurrences(id) ON DELETE SET NULL,
    kb_document_id INTEGER REFERENCES kb_documents(id) ON DELETE SET NULL,
    kb_node_id INTEGER REFERENCES kb_document_nodes(id) ON DELETE SET NULL,

    candidate_type TEXT NOT NULL,
    target_slug TEXT NOT NULL DEFAULT '',
    target_field TEXT NOT NULL DEFAULT '',

    title TEXT NOT NULL DEFAULT '',
    proposed_payload TEXT NOT NULL DEFAULT '{}',
    evidence_json TEXT NOT NULL DEFAULT '{}',

    confidence REAL NOT NULL DEFAULT 0.0,
    risk_level TEXT NOT NULL DEFAULT 'medium',
    status TEXT NOT NULL DEFAULT 'pending',
    reviewer TEXT NOT NULL DEFAULT '',
    review_notes TEXT NOT NULL DEFAULT '',
    applied_at TEXT,
    candidate_fingerprint TEXT NOT NULL DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_promo_candidates_artifact ON promotion_candidates(artifact_id);
CREATE INDEX IF NOT EXISTS idx_promo_candidates_occurrence ON promotion_candidates(occurrence_id);
CREATE INDEX IF NOT EXISTS idx_promo_candidates_doc ON promotion_candidates(kb_document_id);
CREATE INDEX IF NOT EXISTS idx_promo_candidates_status ON promotion_candidates(status);
CREATE INDEX IF NOT EXISTS idx_promo_candidates_target ON promotion_candidates(target_slug);
-- 唯一索引：同一指纹只允许一条 pending/accepted/applied 记录，防止重试路径重复创建候选
CREATE UNIQUE INDEX IF NOT EXISTS idx_promo_candidates_fingerprint
    ON promotion_candidates(candidate_fingerprint)
    WHERE candidate_fingerprint != '' AND status IN ('pending', 'accepted', 'applied');

-- 5. 来源追溯（gbrain 事实与 KB 证据的来源关系）
CREATE TABLE IF NOT EXISTS provenance_ledger (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),

    artifact_id INTEGER REFERENCES source_artifacts(id) ON DELETE SET NULL,
    occurrence_id INTEGER REFERENCES artifact_occurrences(id) ON DELETE SET NULL,
    kb_document_id INTEGER REFERENCES kb_documents(id) ON DELETE SET NULL,
    kb_node_id INTEGER REFERENCES kb_document_nodes(id) ON DELETE SET NULL,
    promotion_candidate_id INTEGER REFERENCES promotion_candidates(id) ON DELETE SET NULL,

    brain_slug TEXT NOT NULL DEFAULT '',
    brain_field TEXT NOT NULL DEFAULT '',
    fact_hash TEXT NOT NULL DEFAULT '',

    quote_text TEXT NOT NULL DEFAULT '',
    quote_start INTEGER,
    quote_end INTEGER,
    page_number INTEGER,

    confidence REAL NOT NULL DEFAULT 0.0,
    status TEXT NOT NULL DEFAULT 'active',
    stale_reason TEXT NOT NULL DEFAULT '',
    metadata_json TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_provenance_artifact ON provenance_ledger(artifact_id);
CREATE INDEX IF NOT EXISTS idx_provenance_occurrence ON provenance_ledger(occurrence_id);
CREATE INDEX IF NOT EXISTS idx_provenance_kb_doc ON provenance_ledger(kb_document_id);
CREATE INDEX IF NOT EXISTS idx_provenance_kb_node ON provenance_ledger(kb_node_id);
CREATE INDEX IF NOT EXISTS idx_provenance_brain_slug ON provenance_ledger(brain_slug);
CREATE INDEX IF NOT EXISTS idx_provenance_fact_hash ON provenance_ledger(fact_hash);
"#;

/// V24: artifact_events 审计表（§7.6）
///
/// 记录 artifact 系统的关键事件，用于审计和回滚支持。
pub const MIGRATION_V24_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS artifact_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    artifact_id INTEGER REFERENCES source_artifacts(id) ON DELETE SET NULL,
    occurrence_id INTEGER REFERENCES artifact_occurrences(id) ON DELETE SET NULL,
    event_type TEXT NOT NULL,
    actor TEXT NOT NULL DEFAULT '',
    payload_json TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_artifact_events_artifact ON artifact_events(artifact_id);
CREATE INDEX IF NOT EXISTS idx_artifact_events_occurrence ON artifact_events(occurrence_id);
CREATE INDEX IF NOT EXISTS idx_artifact_events_type ON artifact_events(event_type);
"#;

/// V25: 投影版本链 — 添加 superseded_by 列（§31）
///
/// 当同 artifact 的新投影替代旧投影时，旧投影的 superseded_by 指向新投影 ID，
/// 形成完整的版本历史链。
pub const MIGRATION_V25_DDL: &str = r#"
ALTER TABLE artifact_projections ADD COLUMN superseded_by INTEGER REFERENCES artifact_projections(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_artifact_proj_superseded ON artifact_projections(superseded_by);
"#;

/// V26: 修复 projection 唯一约束 — 移除表级 UNIQUE，改用 partial unique index
///
/// 旧约束 UNIQUE(artifact_id, projection_type, projection_key, status) 在
/// insert_projection 先插新 active 再标旧 active 为 superseded 时会撞唯一约束，
/// 导致重复上传同一文档失败。partial index 仅 status='active' 时唯一，
/// 允许先标旧行 superseded 再插新 active。
pub const MIGRATION_V26_DDL: &str = r#"
-- SQLite 不支持 ALTER TABLE DROP CONSTRAINT，需要重建表来移除旧 UNIQUE 约束
-- 步骤：创建新表（无表级 UNIQUE）→ 复制数据 → 删旧表 → 重命名新表 → 重建索引

CREATE TABLE IF NOT EXISTS artifact_projections_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),

    artifact_id INTEGER NOT NULL REFERENCES source_artifacts(id) ON DELETE CASCADE,
    occurrence_id INTEGER REFERENCES artifact_occurrences(id) ON DELETE SET NULL,
    projection_type TEXT NOT NULL,
    projection_key TEXT NOT NULL DEFAULT '',
    projection_ref TEXT NOT NULL DEFAULT '',

    status TEXT NOT NULL DEFAULT 'active',
    version_hash TEXT NOT NULL DEFAULT '',
    stale_reason TEXT NOT NULL DEFAULT '',
    metadata_json TEXT NOT NULL DEFAULT '{}',
    superseded_by INTEGER REFERENCES artifact_projections_new(id) ON DELETE SET NULL
);

INSERT INTO artifact_projections_new
    SELECT id, created_at, updated_at, artifact_id, occurrence_id,
           projection_type, projection_key, projection_ref,
           status, version_hash, stale_reason, metadata_json, superseded_by
    FROM artifact_projections;

DROP TABLE artifact_projections;

ALTER TABLE artifact_projections_new RENAME TO artifact_projections;

-- 重建原有索引
CREATE INDEX IF NOT EXISTS idx_artifact_proj_artifact ON artifact_projections(artifact_id);
CREATE INDEX IF NOT EXISTS idx_artifact_proj_occurrence ON artifact_projections(occurrence_id);
CREATE INDEX IF NOT EXISTS idx_artifact_proj_type ON artifact_projections(projection_type);
CREATE INDEX IF NOT EXISTS idx_artifact_proj_ref ON artifact_projections(projection_ref);
CREATE INDEX IF NOT EXISTS idx_artifact_proj_superseded ON artifact_projections(superseded_by);

-- 新增 partial unique index：仅 status='active' 时保证同一 key 唯一
CREATE UNIQUE INDEX IF NOT EXISTS idx_artifact_proj_active_unique
    ON artifact_projections(artifact_id, projection_type, projection_key)
    WHERE status = 'active';
"#;

pub const MIGRATION_V27_DDL: &str = r#"
-- 为 promotion_candidates 添加候选指纹列，用于重试路径去重
-- fingerprint = SHA256(artifact_id|candidate_type|target_slug|target_field|proposed_payload)
-- 同一 artifact + 同一内容不应重复创建候选
ALTER TABLE promotion_candidates ADD COLUMN candidate_fingerprint TEXT NOT NULL DEFAULT '';

-- 唯一索引：同一指纹只允许一条 pending/accepted/applied 记录
-- 使用 partial index 排除 rolled_back/rejected/stale/superseded 等终态，
-- 允许回滚后的候选重新创建
CREATE UNIQUE INDEX IF NOT EXISTS idx_promo_candidates_fingerprint
    ON promotion_candidates(candidate_fingerprint)
    WHERE candidate_fingerprint != '' AND status IN ('pending', 'accepted', 'applied');
"#;

/// Get all schema migrations as (version, DDL) pairs
pub fn get_migrations() -> Vec<(i32, &'static str)> {
    vec![
        (2, MIGRATION_V2_DDL),
        (3, MIGRATION_V3_DDL),
        (4, MIGRATION_V4_DDL),
        (5, MIGRATION_V5_DDL),
        (6, MIGRATION_V6_DDL),
        (7, MIGRATION_V7_DDL),
        (8, MIGRATION_V8_DDL),
        (9, MIGRATION_V9_DDL),
        (10, MIGRATION_V10_DDL),
        (11, MIGRATION_V11_DDL),
        (12, MIGRATION_V12_DDL),
        (13, MIGRATION_V13_DDL),
        (14, MIGRATION_V14_DDL),
        (15, MIGRATION_V15_DDL),
        (16, MIGRATION_V16_DDL),
        (17, MIGRATION_V17_DDL),
        (18, MIGRATION_V18_DDL),
        (19, MIGRATION_V19_DDL),
        (20, MIGRATION_V20_DDL),
        (21, MIGRATION_V21_DDL),
        (22, MIGRATION_V22_DDL),
        (23, MIGRATION_V23_DDL),
        (24, MIGRATION_V24_DDL),
        (25, MIGRATION_V25_DDL),
        (26, MIGRATION_V26_DDL),
        (27, MIGRATION_V27_DDL),
    ]
}
