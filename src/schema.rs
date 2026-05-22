//! SQLite schema DDL
//!
//! 完整的 SQLite schema，包含 FTS5、触发器和索引。
/// 当前 schema 版本号，新数据库会直接写入此版本以跳过历史迁移
pub const SCHEMA_VERSION: i32 = 28;

/// 完整的 schema DDL，新数据库一次性创建
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
    origin_slug TEXT NOT NULL DEFAULT '',
    origin_field TEXT NOT NULL DEFAULT '',
    direction TEXT NOT NULL DEFAULT 'outgoing',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(from_slug, to_slug, link_type, link_source)
);

CREATE INDEX IF NOT EXISTS idx_links_from_slug ON links(from_slug);
CREATE INDEX IF NOT EXISTS idx_links_to_slug ON links(to_slug);
CREATE INDEX IF NOT EXISTS idx_links_direction ON links(direction);

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
    source_type TEXT NOT NULL DEFAULT '',
    source_ref TEXT NOT NULL DEFAULT '',
    summary TEXT NOT NULL DEFAULT '',
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

-- 任务队列
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

/// 生成 sqlite-vec KB 节点虚拟表 DDL
pub fn vec_kb_nodes_ddl(dimensions: usize) -> String {
    format!(
        r#"CREATE VIRTUAL TABLE IF NOT EXISTS vec_kb_nodes USING vec0(
    node_id INTEGER PRIMARY KEY,
    embedding float[{}]
);"#,
        dimensions
    )
}
