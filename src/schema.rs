//! SQLite schema DDL
//! Mirrors gbrain's src/core/pglite-schema.ts
//!
//! Complete SQLite schema with FTS5, triggers, and indexes.

/// Current schema version
pub const SCHEMA_VERSION: i32 = 11;

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
    deleted_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_pages_slug ON pages(slug);
CREATE INDEX IF NOT EXISTS idx_pages_page_type ON pages(page_type);
CREATE INDEX IF NOT EXISTS idx_pages_updated_at ON pages(updated_at);
CREATE INDEX IF NOT EXISTS idx_pages_deleted_at ON pages(deleted_at);

-- FTS5 virtual table for full-text search (weighted: title > compiled_truth > timeline)
CREATE VIRTUAL TABLE IF NOT EXISTS pages_fts USING fts5(
    slug,
    title,
    compiled_truth,
    timeline,
    content='pages',
    content_rowid='id'
);

-- Triggers to keep FTS5 in sync
CREATE TRIGGER IF NOT EXISTS pages_fts_insert AFTER INSERT ON pages BEGIN
    INSERT INTO pages_fts(rowid, slug, title, compiled_truth, timeline)
    VALUES (new.id, new.slug, new.title, new.compiled_truth, new.timeline);
END;

CREATE TRIGGER IF NOT EXISTS pages_fts_update AFTER UPDATE ON pages BEGIN
    INSERT INTO pages_fts(pages_fts, rowid, slug, title, compiled_truth, timeline)
    VALUES ('delete', old.id, old.slug, old.title, old.compiled_truth, old.timeline);
    INSERT INTO pages_fts(rowid, slug, title, compiled_truth, timeline)
    VALUES (new.id, new.slug, new.title, new.compiled_truth, new.timeline);
END;

CREATE TRIGGER IF NOT EXISTS pages_fts_delete AFTER DELETE ON pages BEGIN
    INSERT INTO pages_fts(pages_fts, rowid, slug, title, compiled_truth, timeline)
    VALUES ('delete', old.id, old.slug, old.title, old.compiled_truth, old.timeline);
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
    ]
}
