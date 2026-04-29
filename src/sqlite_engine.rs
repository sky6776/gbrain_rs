//! SQLite engine implementation
//! Mirrors gbrain's src/core/pglite-engine.ts

use crate::config::Config;
use crate::engine::BrainEngine;
use crate::error::{GBrainError, Result};
use crate::schema::SCHEMA_DDL;
use crate::types::*;
use rusqlite::{params, Connection, Transaction};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, info, trace, warn};

/// Convert empty string Option to None to maintain consistency
/// between put_page (which stores "" for None) and get_page (which reads Some("") back).
/// This ensures `page.timeline.is_none()` correctly identifies pages without timeline.
fn empty_to_none(s: Option<String>) -> Option<String> {
    match s {
        Some(ref v) if v.is_empty() => None,
        other => other,
    }
}

/// SQLite-based brain engine
pub struct SqliteEngine {
    conn: Option<Connection>,
    db_path: String,
    #[allow(dead_code)]
    config: Config,
    embedding_dimensions: usize,
}

impl SqliteEngine {
    pub fn new(db_path: &Path) -> Self {
        let config = Config::load().unwrap_or_else(|e| {
            tracing::warn!("Config load failed, using defaults: {}", e);
            Config::default()
        });
        let dims = config.embedding_dimensions;
        Self {
            conn: None,
            db_path: db_path.to_string_lossy().to_string(),
            config,
            embedding_dimensions: dims,
        }
    }

    pub fn with_config(db_path: impl AsRef<Path>, config: Config) -> Self {
        let dims = config.embedding_dimensions;
        Self {
            conn: None,
            db_path: db_path.as_ref().to_string_lossy().to_string(),
            config,
            embedding_dimensions: dims,
        }
    }

    pub fn in_memory() -> Self {
        let config = Config::load().unwrap_or_else(|e| {
            tracing::warn!("Config load failed, using defaults: {}", e);
            Config::default()
        });
        let dims = config.embedding_dimensions;
        Self {
            conn: None,
            db_path: ":memory:".to_string(),
            config,
            embedding_dimensions: dims,
        }
    }

    fn conn(&self) -> Result<&Connection> {
        self.conn.as_ref().ok_or(GBrainError::NotConnected)
    }

    /// Run a function inside a SQLite transaction (raw Transaction access)
    pub fn transaction<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Transaction) -> Result<T>,
    {
        let conn = self.conn()?;
        let tx = conn.unchecked_transaction()?;
        let result = f(&tx);
        match result {
            Ok(value) => {
                tx.commit()?;
                Ok(value)
            }
            Err(e) => Err(e),
        }
    }

    /// P0-2: Run a function inside a SQLite transaction with engine access.
    /// This wraps the entire operation in BEGIN IMMEDIATE for write-lock
    /// protection against concurrent writes (mirrors TS pg_advisory_xact_lock).
    /// The closure receives &self so it can call engine methods normally,
    /// but all DB operations within the closure are part of the same transaction.
    pub fn transaction_with_engine<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Self) -> Result<T>,
    {
        let conn = self.conn()?;
        // BEGIN IMMEDIATE acquires a write lock immediately, preventing
        // concurrent writers from interfering (mirrors TS pg_advisory_xact_lock)
        conn.execute("BEGIN IMMEDIATE", [])?;
        let result = f(self);
        match result {
            Ok(value) => {
                match conn.execute("COMMIT", []) {
                    Ok(_) => Ok(value),
                    Err(e) => {
                        conn.execute("ROLLBACK", []).ok(); // ignore rollback errors
                        Err(e.into())
                    }
                }
            }
            Err(e) => {
                conn.execute("ROLLBACK", []).ok(); // ignore rollback errors
                Err(e)
            }
        }
    }

    /// Trigram Jaccard similarity, mirroring pg_trgm's algorithm.
    /// Delegates to the shared implementation in `search::fuzzy`.
    fn trigram_similarity(a: &str, b: &str) -> f64 {
        crate::search::fuzzy::trigram_similarity(a, b)
    }

    /// Check if the pages_trgm FTS5 virtual table exists
    /// Used to decide whether to use trigram-indexed pre-filtering
    fn has_trgm_table(conn: &Connection) -> bool {
        conn.prepare("SELECT 1 FROM pages_trgm LIMIT 0").is_ok()
    }

    /// Get fuzzy match candidates via FTS5 trigram index pre-filter
    /// Extracts trigrams from the query and uses FTS5 MATCH to find candidates
    fn fuzzy_candidates_via_trgm(
        &self,
        conn: &Connection,
        query_lower: &str,
        dir_prefix: Option<&str>,
    ) -> Result<Vec<(String, String)>> {
        // Extract trigrams from the query for FTS5 MATCH
        let padded = format!("  {}  ", query_lower);
        let chars: Vec<char> = padded.chars().collect();
        let trigrams: Vec<String> = chars
            .windows(3)
            .map(|w| w.iter().collect::<String>())
            .collect();

        // Build FTS5 OR query from trigrams
        // Use a subset of trigrams to avoid overly broad matches
        let match_expr = if trigrams.is_empty() {
            return Ok(Vec::new());
        } else {
            // Use up to 16 trigrams for the MATCH expression
            // Strip FTS5-special characters from each trigram to prevent injection
            let parts: Vec<String> = trigrams.iter().take(16).map(|t| {
                let safe_t: String = t.chars()
                    .filter(|c| !matches!(c, '"' | '\'' | '(' | ')' | '{' | '}' | ':' | '^' | '*' | '.' | '[' | ']' | '+' | '-'))
                    .collect();
                if safe_t.is_empty() { String::new() } else { format!("title:\"{}\"", safe_t) }
            }).filter(|s| !s.is_empty()).collect();
            if parts.is_empty() { return Ok(Vec::new()); }
            parts.join(" OR ")
        };

        let sql = if dir_prefix.is_some() {
            "SELECT p.slug, p.title FROM pages_trgm pt JOIN pages p ON p.id = pt.rowid WHERE pages_trgm MATCH ?1 AND p.slug LIKE ?2 LIMIT 100"
        } else {
            "SELECT p.slug, p.title FROM pages_trgm pt JOIN pages p ON p.id = pt.rowid WHERE pages_trgm MATCH ?1 LIMIT 100"
        };

        let mut stmt = conn.prepare(sql)?;
        let candidates: Vec<(String, String)> = if let Some(prefix) = dir_prefix {
            let prefix_pattern = format!("{}%", prefix);
            stmt.query_map(params![match_expr, prefix_pattern], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .filter_map(|r| { if let Err(e) = &r { warn!(error = %e, "Row decode error"); } r.ok() })
            .collect()
        } else {
            stmt.query_map(params![match_expr], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .filter_map(|r| { if let Err(e) = &r { warn!(error = %e, "Row decode error"); } r.ok() })
            .collect()
        };

        Ok(candidates)
    }

    /// Get fuzzy match candidates via full table scan (fallback)
    /// Used when FTS5 trigram index is not available
    fn fuzzy_candidates_via_scan(
        &self,
        conn: &Connection,
        dir_prefix: Option<&str>,
    ) -> Result<Vec<(String, String)>> {
        let sql = if dir_prefix.is_some() {
            "SELECT slug, title FROM pages WHERE slug LIKE ?1"
        } else {
            "SELECT slug, title FROM pages LIMIT 5000"
        };

        let mut stmt = conn.prepare(sql)?;
        let candidates: Vec<(String, String)> = if let Some(prefix) = dir_prefix {
            let prefix_pattern = format!("{}%", prefix);
            stmt.query_map(params![prefix_pattern], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .filter_map(|r| { if let Err(e) = &r { warn!(error = %e, "Row decode error"); } r.ok() })
            .collect()
        } else {
            stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .filter_map(|r| { if let Err(e) = &r { warn!(error = %e, "Row decode error"); } r.ok() })
            .collect()
        };

        Ok(candidates)
    }

    /// Trigram-based title search for resolve_slugs fallback
    /// Returns ranked slug results based on trigram similarity to the partial query
    fn trigram_title_search(&self, conn: &Connection, partial: &str) -> Result<Vec<String>> {
        let partial_lower = partial.to_lowercase();

        // Try to use FTS5 trigram index for pre-filtering
        let candidates: Vec<(String, String)> = if Self::has_trgm_table(conn) {
            self.fuzzy_candidates_via_trgm(conn, &partial_lower, None)?
        } else {
            self.fuzzy_candidates_via_scan(conn, None)?
        };

        // Score and rank by trigram similarity
        let mut scored: Vec<(String, f64)> = candidates
            .into_iter()
            .map(|(slug, title)| {
                let score = Self::trigram_similarity(&title, &partial_lower);
                (slug, score)
            })
            .filter(|(_, score)| *score >= 0.55)
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(20);

        Ok(scored.into_iter().map(|(slug, _)| slug).collect())
    }

    /// Run all pending schema migrations
    pub fn run_pending_migrations(&self) -> Result<()> {
        let conn = self.conn()?;

        let current_version: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_version",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let migrations = crate::schema::get_migrations();
        for (version, ddl) in migrations {
            if version > current_version {
                debug!("Applying migration v{}", version);
                // Wrap each migration in a transaction for atomicity
                // Use unchecked_transaction to keep rusqlite's transaction depth tracking in sync
                let tx = conn.unchecked_transaction()?;
                let ddl_result = conn.execute_batch(ddl);
                match ddl_result {
                    Ok(_) => {
                        let ver_result = conn.execute(
                            "INSERT OR IGNORE INTO schema_version (version, applied_at) VALUES (?1, datetime('now'))",
                            params![version],
                        );
                        match ver_result {
                            Ok(_) => {
                                info!("Migration v{} applied", version);
                                tx.commit()?;
                            }
                            Err(e) => {
                                warn!("Migration v{} version insert failed: {}", version, e);
                                // unchecked_transaction's Drop will ROLLBACK automatically
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Migration v{} skipped: {}", version, e);
                        // unchecked_transaction's Drop will ROLLBACK automatically
                    }
                }
            }
        }

        // For fresh databases where SCHEMA_DDL already includes all columns,
        // record the current schema version so pending migrations are skipped
        if current_version == 0 {
            conn.execute(
                "INSERT OR IGNORE INTO schema_version (version) VALUES (?1)",
                rusqlite::params![crate::schema::SCHEMA_VERSION],
            )?;
        }

        Ok(())
    }

    /// Normalize a partial slug for matching: lowercase, replace spaces/underscores with hyphens,
    /// collapse multiple hyphens, strip leading/trailing hyphens
    pub fn slugify_for_match(s: &str) -> String {
        let normalized: String = s
            .chars()
            .map(|c| {
                if c == ' ' || c == '_' {
                    '-'
                } else {
                    c.to_ascii_lowercase()
                }
            })
            .collect();
        // Collapse multiple hyphens
        let mut result = String::with_capacity(normalized.len());
        let mut prev_hyphen = false;
        for c in normalized.chars() {
            if c == '-' {
                if !prev_hyphen {
                    result.push(c);
                }
                prev_hyphen = true;
            } else {
                result.push(c);
                prev_hyphen = false;
            }
        }
        // Strip leading/trailing hyphens
        let trimmed = result.trim_matches('-');
        trimmed.to_string()
    }
}

impl BrainEngine for SqliteEngine {
    fn kind(&self) -> &'static str {
        "sqlite"
    }

    // ── Lifecycle ──────────────────────────────────────────────

    fn connect(&mut self) -> Result<()> {
        debug!(db_path = %self.db_path, "Opening SQLite connection");
        // R3-07: Use Connection::open_in_memory() for ":memory:" paths.
        // Connection::open(":memory:") creates a FILE named ":memory:" on disk,
        // not a true in-memory database. Only Connection::open_in_memory()
        // (or the URI "file::memory:") creates a transient in-memory DB.
        let conn = if self.db_path == ":memory:" {
            Connection::open_in_memory()?
        } else {
            Connection::open(&self.db_path)?
        };
        // Apply connection-level PRAGMAs on every new connection.
        // WAL mode persists at the database-file level, but foreign_keys
        // and busy_timeout reset on each new connection.
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -64000;
             PRAGMA temp_store = MEMORY;"
        )?;
        self.conn = Some(conn);
        info!(db_path = %self.db_path, "SQLite connection established");
        Ok(())
    }

    fn disconnect(&mut self) -> Result<()> {
        if let Some(conn) = self.conn.take() {
            debug!(db_path = %self.db_path, "Closing SQLite connection");
            conn.close()
                .map_err(|e| GBrainError::Database(e.1.to_string()))?;
            info!(db_path = %self.db_path, "SQLite connection closed");
        }
        Ok(())
    }

    fn init_schema(&self) -> Result<()> {
        debug!("Initializing database schema");
        let conn = self.conn()?;
        conn.execute_batch(SCHEMA_DDL)?;

        // Try to create sqlite-vec virtual table
        let vec_ddl = crate::schema::vec_chunks_ddl(self.embedding_dimensions);
        let _ = conn.execute_batch(&vec_ddl); // Ignore error if extension not loaded

        // Run pending migrations
        self.run_pending_migrations()?;

        info!("Database schema initialized");
        Ok(())
    }

    // ── Pages CRUD ─────────────────────────────────────────────

    fn get_page(&self, slug: &str) -> Result<Option<Page>> {
        trace!(slug = %slug, "Querying page");
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, slug, page_type, title, compiled_truth, timeline, frontmatter, content_hash, created_at, updated_at
             FROM pages WHERE slug = ?1"
        )?;

        let result = stmt.query_row(params![slug], |row| {
            Ok(Page {
                id: row.get(0)?,
                slug: row.get(1)?,
                page_type: PageType::from_str_lossy(&row.get::<_, String>(2)?),
                title: row.get(3)?,
                compiled_truth: row.get(4)?,
                timeline: empty_to_none(row.get(5)?),
                frontmatter: empty_to_none(row.get(6)?),
                content_hash: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        });

        match result {
            Ok(page) => Ok(Some(page)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn put_page(&self, slug: &str, input: PageInput) -> Result<Page> {
        debug!(slug = %slug, page_type = %input.page_type, title = %input.title, "Upserting page");
        let conn = self.conn()?;

        let timeline_str = input
            .timeline
            .as_ref()
            .map(|v| v.to_string())
            .unwrap_or_default();
        let frontmatter_str = input
            .frontmatter
            .as_ref()
            .map(|v| v.to_string())
            .unwrap_or_default();

        conn.execute(
            "INSERT INTO pages (slug, page_type, title, compiled_truth, timeline, frontmatter, content_hash, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))
             ON CONFLICT(slug) DO UPDATE SET
                page_type = excluded.page_type,
                title = excluded.title,
                compiled_truth = excluded.compiled_truth,
                timeline = excluded.timeline,
                frontmatter = excluded.frontmatter,
                content_hash = excluded.content_hash,
                updated_at = datetime('now')",
            params![
                slug,
                input.page_type.to_string(),
                input.title,
                input.compiled_truth,
                timeline_str,
                frontmatter_str,
                input.content_hash,
            ],
        )?;

        self.get_page(slug)?.ok_or_else(|| GBrainError::PageNotFound(slug.to_string()))
    }

    fn delete_page(&self, slug: &str) -> Result<()> {
        debug!(slug = %slug, "Deleting page");
        self.transaction(|tx| {
            let rows = tx.execute("DELETE FROM pages WHERE slug = ?1", params![slug])?;
            if rows == 0 {
                warn!(slug = %slug, "Page not found for deletion");
                return Err(GBrainError::PageNotFound(slug.to_string()));
            }
            // Clean up slug-based references not covered by CASCADE
            tx.execute("DELETE FROM links WHERE from_slug = ?1 OR to_slug = ?1", params![slug])?;
            tx.execute("DELETE FROM files WHERE page_slug = ?1", params![slug])?;
            Ok(())
        })
    }

    fn list_pages(&self, filters: PageFilters) -> Result<Vec<Page>> {
        let conn = self.conn()?;

        let mut sql = String::from(
            "SELECT id, slug, page_type, title, compiled_truth, timeline, frontmatter, content_hash, created_at, updated_at FROM pages WHERE 1=1"
        );
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(pt) = filters.page_type {
            sql.push_str(" AND page_type = ?");
            param_values.push(Box::new(pt.to_string()));
        }
        if let Some(tag) = filters.tag {
            sql.push_str(" AND id IN (SELECT page_id FROM tags WHERE tag = ?)");
            param_values.push(Box::new(tag));
        }
        if let Some(ref updated_after) = filters.updated_after {
            sql.push_str(" AND updated_at > ?");
            param_values.push(Box::new(updated_after.clone()));
        }

        sql.push_str(" ORDER BY updated_at DESC");

        if let Some(limit) = filters.limit {
            sql.push_str(" LIMIT ?");
            param_values.push(Box::new(limit));
        }
        if let Some(offset) = filters.offset {
            sql.push_str(" OFFSET ?");
            param_values.push(Box::new(offset));
        }

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let pages = stmt
            .query_map(param_refs.as_slice(), |row| {
                Ok(Page {
                    id: row.get(0)?,
                    slug: row.get(1)?,
                    page_type: PageType::from_str_lossy(&row.get::<_, String>(2)?),
                    title: row.get(3)?,
                    compiled_truth: row.get(4)?,
                    timeline: empty_to_none(row.get(5)?),
                    frontmatter: empty_to_none(row.get(6)?),
                    content_hash: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(pages)
    }

    fn resolve_slugs(&self, partial: &str) -> Result<Vec<String>> {
        let conn = self.conn()?;

        // Step 1: Exact match
        let exact: Option<String> = conn
            .query_row(
                "SELECT slug FROM pages WHERE slug = ?1",
                params![partial],
                |row| row.get(0),
            )
            .ok();
        if let Some(slug) = exact {
            return Ok(vec![slug]);
        }

        // Step 1.5: Slugify normalization — try normalized form of partial
        let slugified = Self::slugify_for_match(partial);
        if slugified != partial {
            let slugified_match: Option<String> = conn
                .query_row(
                    "SELECT slug FROM pages WHERE slug = ?1",
                    params![slugified],
                    |row| row.get(0),
                )
                .ok();
            if let Some(slug) = slugified_match {
                return Ok(vec![slug]);
            }
        }

        // Step 2: FTS5 prefix match (fast, exact-prefix)
        // Escape FTS5 special characters in partial to prevent query syntax injection
        let escaped_partial = crate::search::keyword::escape_fts_term(partial);
        if !escaped_partial.is_empty() {
            let match_expr = format!("\"{}\"*", escaped_partial);
            let mut stmt =
                conn.prepare("SELECT slug FROM pages_fts WHERE slug MATCH ?1 LIMIT 20")?;
            let fts_results: Vec<String> = stmt
                .query_map(params![match_expr], |row| row.get(0))?
                .filter_map(|r| { if let Err(e) = &r { warn!(error = %e, "Row decode error"); } r.ok() })
                .collect();

            if !fts_results.is_empty() {
                return Ok(fts_results);
            }
        }

        // Step 3: Trigram similarity match (typo-tolerant, ranked)
        let trgm_results = self.trigram_title_search(conn, partial)?;
        if !trgm_results.is_empty() {
            return Ok(trgm_results);
        }

        // Step 4: LIKE fallback (last resort, unranked)
        // Escape LIKE wildcards % and _ to prevent injection
        let escaped = partial.replace('%', "\\%").replace('_', "\\_");
        let mut stmt = conn.prepare("SELECT slug FROM pages WHERE slug LIKE ?1 ESCAPE '\\' LIMIT 20")?;
        let like_pattern = format!("%{}%", escaped);
        let results: Vec<String> = stmt
            .query_map(params![like_pattern], |row| row.get(0))?
            .filter_map(|r| { if let Err(e) = &r { warn!(error = %e, "Row decode error"); } r.ok() })
            .collect();

        Ok(results)
    }

    fn get_all_slugs(&self) -> Result<Vec<String>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare("SELECT slug FROM pages ORDER BY slug")?;
        let slugs: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(|r| { if let Err(e) = &r { warn!(error = %e, "Row decode error"); } r.ok() })
            .collect();
        Ok(slugs)
    }

    // ── Search ─────────────────────────────────────────────────

    fn search_keyword(&self, query: &str, opts: SearchOpts) -> Result<Vec<SearchResult>> {
        trace!(query = %query, limit = opts.limit.unwrap_or(20), "FTS5 chunk-level keyword search");
        let conn = self.conn()?;
        let limit = opts.limit.unwrap_or(20);

        // Use chunk-level FTS5 (chunks_fts) for chunk-aware results
        // Fall back to page-level FTS5 (pages_fts) if chunks_fts doesn't exist
        let has_chunks_fts: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='chunks_fts'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|c| c > 0)
            .unwrap_or(false);
        // Check if chunks_fts actually has data
        let chunks_has_data: bool = if has_chunks_fts {
            conn.query_row(
                "SELECT COUNT(*) FROM chunks_fts",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|c| c > 0)
            .unwrap_or(false)
        } else {
            false
        };

        if has_chunks_fts && chunks_has_data {
            // P0-1: Include p.page_type in query so type diversity dedup works
            // P1-1: Support page_type filter and exclude_slugs
            // P1-2: Support detail_level filter (exclude timeline chunks for Low)
            // P1-4: Compute stale flag from embedding staleness (embedded_at < updated_at)
            let mut sql = String::from(
                "SELECT p.slug, p.title, snippet(chunks_fts, 2, '<<', '>>', '...', 64) as snippet,
                        bm25(chunks_fts) as score,
                        c.id as chunk_id, c.chunk_index, c.chunk_source, c.page_id,
                        p.page_type,
                        (c.embedded_at IS NULL OR c.embedded_at < p.updated_at) as stale,
                        p.updated_at
                 FROM chunks_fts
                 JOIN chunks c ON c.id = chunks_fts.rowid
                 JOIN pages p ON p.id = c.page_id
                 WHERE chunks_fts MATCH ?1",
            );
            if opts.page_type.is_some() {
                sql.push_str(" AND p.page_type = ?3");
            }
            if let Some(ref exclude) = opts.exclude_slugs {
                if !exclude.is_empty() {
                    // Build placeholder list for exclude slugs
                    let start_idx = if opts.page_type.is_some() { 4 } else { 3 };
                    let placeholders: Vec<String> = (0..exclude.len())
                        .map(|i| format!("?{}", start_idx + i))
                        .collect();
                    sql.push_str(&format!(" AND p.slug NOT IN ({})", placeholders.join(", ")));
                }
            }
            if opts.detail_level == Some(DetailLevel::Low) {
                sql.push_str(" AND c.chunk_source = 'compiled_truth'");
            }
            sql.push_str(" ORDER BY score LIMIT ?2");

            let mut stmt = conn.prepare(&sql)?;

            // Bind parameters dynamically
            let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
            param_values.push(Box::new(query.to_string())); // ?1
            param_values.push(Box::new(limit)); // ?2
            if let Some(ref pt) = opts.page_type {
                param_values.push(Box::new(pt.to_string())); // ?3
            } else {
                // No page_type filter, but exclude_slugs starts at ?3
            }
            if let Some(ref exclude) = opts.exclude_slugs {
                for s in exclude {
                    param_values.push(Box::new(s.clone()));
                }
            }

            let param_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();

            let results: Vec<SearchResult> = stmt
                .query_map(param_refs.as_slice(), |row| {
                    let score: f64 = row.get(3)?;
                    let source_str: String = row.get::<_, String>(6).unwrap_or_default();
                    let source = if source_str == "timeline" {
                        ChunkSource::Timeline
                    } else {
                        ChunkSource::CompiledTruth
                    };
                    let page_type_str: String = row.get::<_, String>(8).unwrap_or_default();
                    let page_type = if page_type_str.is_empty() {
                        None
                    } else {
                        Some(PageType::from_str_lossy(&page_type_str))
                    };
                    let stale: bool = row.get::<_, bool>(9).unwrap_or(false);
                    let updated_at: Option<String> = row.get(10).ok();
                    Ok(SearchResult {
                        slug: row.get(0)?,
                        title: row.get(1)?,
                        chunk_text: row.get(2)?,
                        score: -score,
                        page_id: row.get(7)?,
                        chunk_id: row.get(4)?,
                        chunk_index: row.get(5)?,
                        source: Some(source),
                        detail_level: opts.detail_level.unwrap_or(DetailLevel::Medium),
                        page_type,
                        stale,
                        updated_at,
                    })
                })?
                .filter_map(|r| { if let Err(e) = &r { warn!(error = %e, "Row decode error"); } r.ok() })
                .collect();
            Ok(results)
        } else {
            // Fallback: page-level FTS5 (backward compat)
            // P0-1: Include p.page_type in fallback query too
            // Optimize9: Weighted bm25 — title (10x) > compiled_truth (5x) > timeline (2x) > slug (1x)
            // Mirrors TS tsvector weights: title=A, compiled_truth=B, timeline=C
            let mut sql = String::from(
                "SELECT p.slug, p.title, snippet(pages_fts, 2, '<<', '>>', '...', 64) as snippet,
                        bm25(pages_fts, 1.0, 10.0, 5.0, 2.0) as score,
                        p.page_type,
                        NOT EXISTS(SELECT 1 FROM chunks c WHERE c.page_id = p.id
                                   AND c.embedded_at IS NOT NULL AND c.embedded_at >= p.updated_at) as stale,
                        p.updated_at
                 FROM pages_fts
                 JOIN pages p ON p.id = pages_fts.rowid
                 WHERE pages_fts MATCH ?1",
            );
            if opts.page_type.is_some() {
                sql.push_str(" AND p.page_type = ?3");
            }
            // Bug 15 fix: When detail_level is Low, only return pages with compiled_truth content
            // (mirrors chunk-level query's AND c.chunk_source = 'compiled_truth' filter)
            if opts.detail_level == Some(DetailLevel::Low) {
                sql.push_str(" AND p.compiled_truth IS NOT NULL AND p.compiled_truth != ''");
            }
            if let Some(ref exclude) = opts.exclude_slugs {
                if !exclude.is_empty() {
                    let start_idx = if opts.page_type.is_some() { 4 } else { 3 };
                    let placeholders: Vec<String> = (0..exclude.len())
                        .map(|i| format!("?{}", start_idx + i))
                        .collect();
                    sql.push_str(&format!(" AND p.slug NOT IN ({})", placeholders.join(", ")));
                }
            }
            sql.push_str(" ORDER BY score LIMIT ?2");

            let mut stmt = conn.prepare(&sql)?;

            let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
            param_values.push(Box::new(query.to_string())); // ?1
            param_values.push(Box::new(limit)); // ?2
            if let Some(ref pt) = opts.page_type {
                param_values.push(Box::new(pt.to_string())); // ?3
            }
            if let Some(ref exclude) = opts.exclude_slugs {
                for s in exclude {
                    param_values.push(Box::new(s.clone()));
                }
            }

            let param_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();

            let results: Vec<SearchResult> = stmt
                .query_map(param_refs.as_slice(), |row| {
                    let score: f64 = row.get(3)?;
                    let page_type_str: String = row.get::<_, String>(4).unwrap_or_default();
                    let page_type = if page_type_str.is_empty() {
                        None
                    } else {
                        Some(PageType::from_str_lossy(&page_type_str))
                    };
                    let stale: bool = row.get::<_, bool>(5).unwrap_or(false);
                    let updated_at: Option<String> = row.get(6).ok();
                    Ok(SearchResult {
                        slug: row.get(0)?,
                        title: row.get(1)?,
                        chunk_text: row.get(2)?,
                        score: -score,
                        page_id: None,
                        chunk_id: None,
                        chunk_index: None,
                        source: Some(ChunkSource::CompiledTruth),
                        detail_level: opts.detail_level.unwrap_or(DetailLevel::Medium),
                        page_type,
                        stale,
                        updated_at,
                    })
                })?
                .filter_map(|r| { if let Err(e) = &r { warn!(error = %e, "Row decode error"); } r.ok() })
                .collect();
            Ok(results)
        }
    }

    fn search_vector(&self, embedding: &[f32], opts: SearchOpts) -> Result<Vec<SearchResult>> {
        // P1-3: sqlite-vec vector search implementation
        // Mirrors TS: searchVector() via pgvector cosine distance
        trace!(limit = opts.limit.unwrap_or(20), emb_dims = embedding.len(), "Vector search (sqlite-vec)");
        let conn = self.conn()?;

        // Check if vec_chunks table exists
        let has_vec: bool = conn.prepare("SELECT 1 FROM vec_chunks LIMIT 0").is_ok();
        if !has_vec || embedding.is_empty() {
            return Ok(Vec::new());
        }

        let limit = opts.limit.unwrap_or(20);

        // Serialize query embedding as f32 LE blob for sqlite-vec
        let query_blob: Vec<u8> = embedding
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();

        // Query vec_chunks for nearest neighbors by cosine distance
        // sqlite-vec vec0 virtual table supports: SELECT ... WHERE embedding MATCH ? ORDER BY distance
        // Build dynamic SQL to support page_type and exclude_slugs filters
        let mut sql = String::from(
            "SELECT v.chunk_id, v.distance, c.chunk_text, c.chunk_index, c.chunk_source, c.page_id,
                    p.slug, p.title, p.page_type, p.updated_at,
                    (c.embedded_at IS NULL OR c.embedded_at < p.updated_at) as stale
             FROM vec_chunks v
             JOIN chunks c ON c.id = v.chunk_id
             JOIN pages p ON p.id = c.page_id
             WHERE v.embedding MATCH ?1"
        );
        if opts.page_type.is_some() {
            sql.push_str(" AND p.page_type = ?3");
        }
        // P-detail: Filter by chunk_source when detail_level is Low (consistency with keyword search)
        if opts.detail_level == Some(DetailLevel::Low) {
            sql.push_str(" AND c.chunk_source = 'compiled_truth'");
        }
        if let Some(ref exclude) = opts.exclude_slugs {
            if !exclude.is_empty() {
                let start_idx = if opts.page_type.is_some() { 4 } else { 3 };
                let placeholders: Vec<String> = (0..exclude.len())
                    .map(|i| format!("?{}", start_idx + i))
                    .collect();
                sql.push_str(&format!(" AND p.slug NOT IN ({})", placeholders.join(", ")));
            }
        }
        sql.push_str(" ORDER BY v.distance LIMIT ?2");

        let mut stmt = conn.prepare(&sql)?;

        // Bind parameters dynamically
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        param_values.push(Box::new(query_blob.clone())); // ?1
        param_values.push(Box::new(limit)); // ?2
        if let Some(ref pt) = opts.page_type {
            param_values.push(Box::new(pt.to_string())); // ?3
        }
        if let Some(ref exclude) = opts.exclude_slugs {
            for s in exclude {
                param_values.push(Box::new(s.clone()));
            }
        }

        let param_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();

        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            let chunk_id: i64 = row.get(0)?;
            let distance: f32 = row.get(1)?;
            let chunk_text: String = row.get(2)?;
            let chunk_index: i32 = row.get(3)?;
            let chunk_source_str: String = row.get(4)?;
            let page_id: i64 = row.get(5)?;
            let slug: String = row.get(6)?;
            let title: String = row.get(7)?;
            let page_type_str: String = row.get::<_, String>(8).unwrap_or_default();
            let updated_at: Option<String> = row.get(9).ok();
            let stale: bool = row.get::<_, bool>(10).unwrap_or(false);
            Ok((chunk_id, distance, chunk_text, chunk_index, chunk_source_str, page_id, slug, title, page_type_str, updated_at, stale))
        })?;

        let detail = opts.detail_level.unwrap_or(DetailLevel::Medium);
        let mut results = Vec::new();
        for row_result in rows {
            let (chunk_id, distance, chunk_text, chunk_index, chunk_source_str, page_id, slug, title, page_type_str, updated_at, stale) = row_result?;

            let source = match chunk_source_str.as_str() {
                "compiled_truth" => ChunkSource::CompiledTruth,
                "timeline" => ChunkSource::Timeline,
                _ => ChunkSource::CompiledTruth,
            };

            let page_type = if page_type_str.is_empty() {
                None
            } else {
                Some(PageType::from_str_lossy(&page_type_str))
            };

            // Convert cosine distance to similarity score
            // sqlite-vec returns distance = 1 - cosine_similarity for cosine metric
            let score = (1.0f32 - distance) as f64;

            results.push(SearchResult {
                slug,
                title,
                chunk_text,
                score,
                page_id: Some(page_id),
                chunk_id: Some(chunk_id),
                chunk_index: Some(chunk_index),
                source: Some(source),
                detail_level: detail,
                page_type,
                stale,
                updated_at,
            });
        }

        debug!(result_count = results.len(), "Vector search complete (sqlite-vec)");
        Ok(results)
    }

    fn get_embeddings_by_chunk_ids(&self, chunk_ids: &[i64]) -> Result<Vec<(i64, Vec<f32>)>> {
        let conn = self.conn()?;

        // Check if vec_chunks table exists
        if conn.prepare("SELECT 1 FROM vec_chunks LIMIT 0").is_err() {
            return Ok(Vec::new());
        }

        if chunk_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Batch query: single IN clause instead of per-id loop (fixes N+1)
        let placeholders: Vec<String> = chunk_ids.iter().enumerate().map(|(i, _)| format!("?{}", i + 1)).collect();
        let sql = format!(
            "SELECT chunk_id, embedding FROM vec_chunks WHERE chunk_id IN ({})",
            placeholders.join(",")
        );
        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::types::ToSql> = chunk_ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
        let rows: Vec<(i64, Vec<u8>)> = stmt
            .query_map(params.as_slice(), |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut results = Vec::with_capacity(rows.len());
        for (id, blob) in rows {
            let embedding: Vec<f32> = blob
                .chunks_exact(4)
                .filter_map(|chunk| {
                    let bytes: [u8; 4] = chunk.try_into().ok()?;
                    Some(f32::from_le_bytes(bytes))
                })
                .collect();
            results.push((id, embedding));
        }

        Ok(results)
    }

    // ── Chunks ─────────────────────────────────────────────────

    fn upsert_chunks(&self, slug: &str, chunks: &[ChunkInput]) -> Result<usize> {
        debug!(slug = %slug, chunk_count = chunks.len(), "Upserting chunks");

        self.transaction(|tx| {
            // Get page_id for slug
            let page_id: i64 = tx.query_row(
                "SELECT id FROM pages WHERE slug = ?1",
                params![slug],
                |row| row.get(0),
            )?;

            let mut count = 0;
            for chunk in chunks {
                let source_str = chunk.source.to_string();
                let result = tx.execute(
                    "INSERT INTO chunks (page_id, chunk_index, chunk_text, chunk_source, token_count)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(page_id, chunk_index, chunk_source) DO UPDATE SET
                        chunk_text = excluded.chunk_text,
                        token_count = excluded.token_count,
                        embedded_at = NULL",
                    params![
                        page_id,
                        chunk.chunk_index,
                        chunk.chunk_text,
                        source_str,
                        chunk.token_count
                    ],
                );
                if result.is_ok() {
                    count += 1;
                }
            }

            Ok(count)
        })
    }

    fn get_chunks(&self, slug: &str) -> Result<Vec<Chunk>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT c.id, c.page_id, p.slug, c.chunk_index, c.chunk_text, c.chunk_source, c.token_count, c.created_at
             FROM chunks c JOIN pages p ON p.id = c.page_id
             WHERE p.slug = ?1
             ORDER BY c.chunk_index"
        )?;

        let chunks: Vec<Chunk> = stmt
            .query_map(params![slug], |row| {
                Ok(Chunk {
                    id: row.get(0)?,
                    page_id: row.get(1)?,
                    slug: row.get(2)?,
                    chunk_index: row.get(3)?,
                    chunk_text: row.get(4)?,
                    source: match row.get::<_, String>(5)?.as_str() {
                        "timeline" => ChunkSource::Timeline,
                        _ => ChunkSource::CompiledTruth,
                    },
                    token_count: row.get::<_, Option<i32>>(6)?.unwrap_or(0),
                    created_at: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
                })
            })?
            .filter_map(|r| { if let Err(e) = &r { warn!(error = %e, "Row decode error"); } r.ok() })
            .collect();

        Ok(chunks)
    }

    fn delete_chunks(&self, slug: &str) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM chunks WHERE page_id = (SELECT id FROM pages WHERE slug = ?1)",
            params![slug],
        )?;
        Ok(())
    }

    // ── Links ──────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    fn add_link(
        &self,
        from_slug: &str,
        to_slug: &str,
        context: Option<&str>,
        link_type: Option<&str>,
        source: Option<&str>,
        _confidence: Option<f64>,
        _metadata: Option<serde_json::Value>,
    ) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO links (from_slug, to_slug, link_type, context, link_source, origin_slug, origin_field)
             VALUES (?1, ?2, ?3, ?4, ?5, '', '')
             ON CONFLICT(from_slug, to_slug, link_type, link_source) DO NOTHING",
            params![
                from_slug,
                to_slug,
                link_type.unwrap_or("mentions"),
                context.unwrap_or(""),
                source.unwrap_or("markdown"),
            ],
        )?;
        Ok(())
    }

    fn add_links_batch(&self, inputs: &[LinkBatchInput]) -> Result<usize> {
        self.transaction(|tx| {
            let mut count = 0;
            for input in inputs {
                let link_source = match input.link_source.as_ref() {
                    Some(LinkSource::Markdown) => "markdown",
                    Some(LinkSource::Frontmatter) => "frontmatter",
                    Some(LinkSource::Manual) => "manual",
                    None => "markdown",
                };
                let direction = match input.direction.as_ref() {
                    Some(LinkDirection::Outgoing) => "outgoing",
                    Some(LinkDirection::Incoming) => "incoming",
                    None => "outgoing",
                };
                let result = tx.execute(
                    "INSERT INTO links (from_slug, to_slug, link_type, context, link_source, origin_slug, origin_field, direction)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                     ON CONFLICT(from_slug, to_slug, link_type, link_source) DO NOTHING",
                    params![
                        input.from_slug,
                        input.to_slug,
                        input.link_type.as_deref().unwrap_or("mentions"),
                        input.context.as_deref().unwrap_or(""),
                        link_source,
                        input.origin_slug.as_deref().unwrap_or(""),
                        input.origin_field.as_deref().unwrap_or(""),
                        direction,
                    ],
                );
                if let Ok(n) = result {
                    count += n;
                }
            }
            Ok(count)
        })
    }

    fn remove_link(
        &self,
        from_slug: &str,
        to_slug: &str,
        link_type: Option<&str>,
        _context: Option<&str>,
        link_source: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn()?;
        match (link_type, link_source) {
            (Some(lt), Some(ls)) => {
                conn.execute(
                    "DELETE FROM links WHERE from_slug = ?1 AND to_slug = ?2 AND link_type = ?3 AND link_source = ?4",
                    params![from_slug, to_slug, lt, ls],
                )?;
            }
            (Some(lt), None) => {
                conn.execute(
                    "DELETE FROM links WHERE from_slug = ?1 AND to_slug = ?2 AND link_type = ?3",
                    params![from_slug, to_slug, lt],
                )?;
            }
            (None, Some(ls)) => {
                conn.execute(
                    "DELETE FROM links WHERE from_slug = ?1 AND to_slug = ?2 AND link_source = ?3",
                    params![from_slug, to_slug, ls],
                )?;
            }
            (None, None) => {
                conn.execute(
                    "DELETE FROM links WHERE from_slug = ?1 AND to_slug = ?2",
                    params![from_slug, to_slug],
                )?;
            }
        }
        Ok(())
    }

    fn get_links(&self, slug: &str) -> Result<Vec<Link>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, from_slug, to_slug, link_type, context, link_source, origin_slug, origin_field, direction, created_at
             FROM links WHERE from_slug = ?1 ORDER BY created_at DESC",
        )?;
        let links: Vec<Link> = stmt
            .query_map(params![slug], |row| {
                let link_source_str: String = row.get::<_, String>(5)?;
                let direction_str: String = row.get::<_, String>(8)?;
                Ok(Link {
                    id: row.get(0)?,
                    from_slug: row.get(1)?,
                    to_slug: row.get(2)?,
                    link_type: row.get(3)?,
                    context: row.get::<_, Option<String>>(4)?,
                    link_source: Some(LinkSource::from_str_lossy(&link_source_str)),
                    origin_slug: row.get::<_, Option<String>>(6)?,
                    origin_field: row.get::<_, Option<String>>(7)?,
                    direction: Some(LinkDirection::from_str_lossy(&direction_str)),
                    created_at: row.get(9)?,
                })
            })?
            .filter_map(|r| { if let Err(e) = &r { warn!(error = %e, "Row decode error"); } r.ok() })
            .collect();
        Ok(links)
    }

    fn get_backlinks(&self, slug: &str) -> Result<Vec<Link>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, from_slug, to_slug, link_type, context, link_source, origin_slug, origin_field, direction, created_at
             FROM links WHERE to_slug = ?1 ORDER BY created_at DESC",
        )?;
        let links: Vec<Link> = stmt
            .query_map(params![slug], |row| {
                let link_source_str: String = row.get::<_, String>(5)?;
                let direction_str: String = row.get::<_, String>(8)?;
                Ok(Link {
                    id: row.get(0)?,
                    from_slug: row.get(1)?,
                    to_slug: row.get(2)?,
                    link_type: row.get(3)?,
                    context: row.get::<_, Option<String>>(4)?,
                    link_source: Some(LinkSource::from_str_lossy(&link_source_str)),
                    origin_slug: row.get::<_, Option<String>>(6)?,
                    origin_field: row.get::<_, Option<String>>(7)?,
                    direction: Some(LinkDirection::from_str_lossy(&direction_str)),
                    created_at: row.get(9)?,
                })
            })?
            .filter_map(|r| { if let Err(e) = &r { warn!(error = %e, "Row decode error"); } r.ok() })
            .collect();
        Ok(links)
    }

    fn remove_links_by_origin(&self, from_slug: &str, origin_source: &str) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM links WHERE from_slug = ?1 AND link_source = ?2",
            params![from_slug, origin_source],
        )?;
        Ok(())
    }

    fn find_by_title_fuzzy(
        &self,
        query: &str,
        dir_prefix: Option<&str>,
        min_similarity: Option<f64>,
        limit: Option<usize>,
    ) -> Result<Vec<FuzzyMatch>> {
        let conn = self.conn()?;
        let limit = limit.unwrap_or(10);
        let min_sim = min_similarity.unwrap_or(0.55).clamp(0.0, 1.0);
        let query_lower = query.to_lowercase();

        // Validate dir_prefix: reject LIKE wildcards and path traversal
        if let Some(prefix) = dir_prefix {
            if prefix.contains("%") || prefix.contains("_") || prefix.contains("..") {
                return Err(GBrainError::InvalidInput(format!(
                    "Invalid dir_prefix: {}",
                    prefix
                )));
            }
        }

        // Phase 1: Get candidates
        // Try FTS5 trigram index first, fall back to full scan
        let candidates: Vec<(String, String)> = if Self::has_trgm_table(conn) {
            self.fuzzy_candidates_via_trgm(conn, &query_lower, dir_prefix)?
        } else {
            self.fuzzy_candidates_via_scan(conn, dir_prefix)?
        };

        // Phase 2: Score candidates with trigram_similarity and sort
        let mut matches: Vec<FuzzyMatch> = candidates
            .into_iter()
            .map(|(slug, title)| {
                let score = Self::trigram_similarity(&title, &query_lower);
                FuzzyMatch { slug, title, score }
            })
            .filter(|m| m.score >= min_sim)
            .collect();

        matches.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        matches.truncate(limit);

        Ok(matches)
    }

    fn traverse_graph(&self, slug: &str, depth: usize) -> Result<Vec<GraphNode>> {
        let conn = self.conn()?;
        let mut nodes = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut queue = vec![(slug.to_string(), 0)];

        // Prepare statements once outside the loop (fixes N+1 prepare overhead)
        let mut page_stmt = conn.prepare("SELECT page_type, title FROM pages WHERE slug = ?1")?;
        let mut links_stmt = conn.prepare("SELECT to_slug, link_type FROM links WHERE from_slug = ?1")?;

        while let Some((current_slug, current_depth)) = queue.pop() {
            if current_depth > depth || visited.contains(&current_slug) {
                continue;
            }
            visited.insert(current_slug.clone());

            // Get page info
            let page_info: Option<(String, String)> = page_stmt
                .query_row(params![current_slug], |row| Ok((row.get(0)?, row.get(1)?)))
                .ok();

            let (page_type, title) =
                page_info.unwrap_or_else(|| ("note".to_string(), current_slug.clone()));

            // Get outgoing links
            let links: Vec<NodeLink> = links_stmt
                .query_map(params![current_slug], |row| {
                    Ok(NodeLink {
                        to_slug: row.get(0)?,
                        link_type: row.get(1)?,
                    })
                })?
                .filter_map(|r| { if let Err(e) = &r { warn!(error = %e, "Row decode error"); } r.ok() })
                .collect();

            // Add unvisited targets to queue
            for link in &links {
                if !visited.contains(&link.to_slug) {
                    queue.push((link.to_slug.clone(), current_depth + 1));
                }
            }

            nodes.push(GraphNode {
                slug: current_slug,
                page_type,
                title,
                depth: current_depth,
                links,
            });
        }

        Ok(nodes)
    }

    fn traverse_paths(&self, from: &str, to: &str, opts: TraverseOpts) -> Result<Vec<GraphPath>> {
        let conn = self.conn()?;
        let max_depth = if opts.depth > 0 { opts.depth } else { 6 };
        let mut edges = Vec::new();
        let mut visited = std::collections::HashSet::new();
        // (slug, depth)
        let mut queue: Vec<(String, usize)> = vec![(from.to_string(), 0)];
        visited.insert(from.to_string());

        while let Some((current, depth)) = queue.pop() {
            if depth >= max_depth {
                continue;
            }

            // Build query based on direction and optional link_type filter
            let query = match (&opts.direction, &opts.link_type) {
                (Direction::In, Some(_)) => {
                    "SELECT from_slug, to_slug, link_type, context FROM links \
                     WHERE to_slug = ?1 AND link_type = ?2"
                }
                (Direction::In, None) => {
                    "SELECT from_slug, to_slug, link_type, context FROM links \
                     WHERE to_slug = ?1"
                }
                (Direction::Out, Some(_)) => {
                    "SELECT from_slug, to_slug, link_type, context FROM links \
                     WHERE from_slug = ?1 AND link_type = ?2"
                }
                (Direction::Out, None) => {
                    "SELECT from_slug, to_slug, link_type, context FROM links \
                     WHERE from_slug = ?1"
                }
                (Direction::Both, Some(_)) => {
                    "SELECT from_slug, to_slug, link_type, context FROM links \
                     WHERE (from_slug = ?1 OR to_slug = ?1) AND link_type = ?2"
                }
                (Direction::Both, None) => {
                    "SELECT from_slug, to_slug, link_type, context FROM links \
                     WHERE from_slug = ?1 OR to_slug = ?1"
                }
            };

            let mut stmt = conn.prepare(query)?;

            let rows: Vec<(String, String, String, String)> = if opts.link_type.is_some() {
                let lt = opts.link_type.as_deref().unwrap_or("");
                stmt.query_map(params![current, lt], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3).unwrap_or_default(),
                    ))
                })?
                .filter_map(|r| { if let Err(e) = &r { warn!(error = %e, "Row decode error"); } r.ok() })
                .collect()
            } else {
                stmt.query_map(params![current], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3).unwrap_or_default(),
                    ))
                })?
                .filter_map(|r| { if let Err(e) = &r { warn!(error = %e, "Row decode error"); } r.ok() })
                .collect()
            };

            for (from_slug, to_slug_raw, link_type, context) in rows {
                // Normalize: for Both direction, emit edge from→to consistently
                let (from_s, to_s) = if from_slug == current {
                    (from_slug.clone(), to_slug_raw)
                } else {
                    // Incoming edge: swap to represent as current → neighbor
                    (to_slug_raw.clone(), from_slug.clone())
                };

                // Record the edge
                edges.push(GraphPath {
                    from_slug: from_s,
                    to_slug: to_s.clone(),
                    link_type,
                    context,
                    depth: depth + 1,
                });

                // Enqueue neighbor if not visited and target matches or we want full traversal
                let neighbor = if to_s == current {
                    // This was an incoming edge, the neighbor is actually from_s
                    from_slug.clone()
                } else {
                    to_s
                };

                if !visited.contains(&neighbor) {
                    visited.insert(neighbor.clone());
                    if neighbor == to || max_depth > 1 {
                        queue.push((neighbor, depth + 1));
                    }
                }
            }

            // Cap results
            if edges.len() >= 50 {
                break;
            }
        }

        // Filter: only keep edges that are on paths leading to the target
        // If 'to' is specified, filter to only relevant edges
        if to != from {
            let target_reachable: std::collections::HashSet<String> = edges
                .iter()
                .filter(|e| e.to_slug == to)
                .flat_map(|e| vec![e.from_slug.clone(), e.to_slug.clone()])
                .collect();
            edges.retain(|e| target_reachable.contains(&e.from_slug) || target_reachable.contains(&e.to_slug));
        }

        Ok(edges)
    }

    fn get_backlink_counts(&self, slugs: &[String]) -> Result<HashMap<String, i64>> {
        // P2-10: Only fetch counts for requested slugs, not all slugs
        if slugs.is_empty() {
            return Ok(HashMap::new());
        }
        let conn = self.conn()?;
        let placeholders: Vec<String> = slugs.iter().enumerate().map(|(i, _)| format!("?{}", i + 1)).collect();
        let sql = format!(
            "SELECT to_slug, COUNT(*) as cnt FROM links WHERE to_slug IN ({}) GROUP BY to_slug",
            placeholders.join(",")
        );
        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::types::ToSql> = slugs.iter().map(|s| s as &dyn rusqlite::types::ToSql).collect();
        let counts: HashMap<String, i64> = stmt
            .query_map(params.as_slice(), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .filter_map(|r| { if let Err(e) = &r { warn!(error = %e, "Row decode error"); } r.ok() })
            .collect();
        Ok(counts)
    }

    // ── Tags ───────────────────────────────────────────────────

    fn add_tag(&self, slug: &str, tag: &str) -> Result<()> {
        let conn = self.conn()?;
        // Check page exists first to give a clear error message
        let page_id: Option<i64> = conn.query_row(
            "SELECT id FROM pages WHERE slug = ?1",
            params![slug],
            |row| row.get(0),
        ).ok();
        let page_id = match page_id {
            Some(id) => id,
            None => return Err(GBrainError::PageNotFound(slug.to_string())),
        };
        conn.execute(
            "INSERT INTO tags (page_id, tag) VALUES (?1, ?2)
             ON CONFLICT(page_id, tag) DO NOTHING",
            params![page_id, tag],
        )?;
        Ok(())
    }

    fn remove_tag(&self, slug: &str, tag: &str) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "DELETE FROM tags WHERE page_id = (SELECT id FROM pages WHERE slug = ?1) AND tag = ?2",
            params![slug, tag],
        )?;
        Ok(())
    }

    fn get_tags(&self, slug: &str) -> Result<Vec<String>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT tag FROM tags WHERE page_id = (SELECT id FROM pages WHERE slug = ?1) ORDER BY tag"
        )?;
        let tags: Vec<String> = stmt
            .query_map(params![slug], |row| row.get(0))?
            .filter_map(|r| { if let Err(e) = &r { warn!(error = %e, "Row decode error"); } r.ok() })
            .collect();
        Ok(tags)
    }

    // ── Timeline ───────────────────────────────────────────────

    fn add_timeline_entry(
        &self,
        slug: &str,
        entry: TimelineInput,
        skip_existence_check: bool,
    ) -> Result<()> {
        let conn = self.conn()?;
        // Verify page exists unless skip_existence_check is true
        if !skip_existence_check {
            let exists: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM pages WHERE slug = ?1)",
                    params![slug],
                    |row| row.get(0),
                )
                .unwrap_or(false);
            if !exists {
                return Err(GBrainError::PageNotFound(slug.to_string()));
            }
        }
        let source = entry.source.as_deref().unwrap_or("");
        let detail = entry.detail.as_deref().unwrap_or("");
        conn.execute(
            "INSERT OR IGNORE INTO timeline (page_id, date, source, summary, detail)
             VALUES ((SELECT id FROM pages WHERE slug = ?1), ?2, ?3, ?4, ?5)",
            params![slug, entry.date, source, entry.summary, detail],
        )?;
        Ok(())
    }

    fn add_timeline_entries_batch(&self, slug: &str, entries: &[TimelineInput]) -> Result<usize> {
        self.transaction(|tx| {
            let mut count = 0;
            for entry in entries {
                let result = tx.execute(
                    "INSERT OR IGNORE INTO timeline (page_id, date, source, summary, detail)
                     VALUES ((SELECT id FROM pages WHERE slug = ?1), ?2, ?3, ?4, ?5)",
                    params![slug, entry.date, entry.source, entry.summary, entry.detail],
                );
                if let Ok(n) = result {
                    count += n;
                }
            }
            Ok(count)
        })
    }

    /// P2-8: Multi-slug timeline batch insert (mirrors TS: each entry has its own slug)
    fn add_timeline_multi_batch(&self, batches: &[TimelineBatchInput]) -> Result<usize> {
        self.transaction(|tx| {
            let mut count = 0;
            for batch in batches {
                for entry in &batch.entries {
                    let result = tx.execute(
                        "INSERT OR IGNORE INTO timeline (page_id, date, source, summary, detail)
                         VALUES ((SELECT id FROM pages WHERE slug = ?1), ?2, ?3, ?4, ?5)",
                        params![batch.slug, entry.date, entry.source, entry.summary, entry.detail],
                    );
                    if let Ok(n) = result {
                        count += n;
                    }
                }
            }
            Ok(count)
        })
    }

    fn get_timeline(
        &self,
        slug: &str,
        opts: Option<TimelineQueryOpts>,
    ) -> Result<Vec<TimelineEntry>> {
        let conn = self.conn()?;
        let limit = opts.as_ref().and_then(|o| o.limit).unwrap_or(50);
        let after = opts.as_ref().and_then(|o| o.after.as_deref()).unwrap_or("");
        let before = opts.as_ref().and_then(|o| o.before.as_deref()).unwrap_or("");

        // Build query with optional date range filters
        let query = if !after.is_empty() && !before.is_empty() {
            "SELECT t.id, p.slug, t.date, t.source, t.summary, t.detail, t.created_at FROM timeline t JOIN pages p ON p.id = t.page_id WHERE p.slug = ?1 AND t.date >= ?3 AND t.date <= ?4 ORDER BY t.date DESC LIMIT ?2"
        } else if !after.is_empty() {
            "SELECT t.id, p.slug, t.date, t.source, t.summary, t.detail, t.created_at FROM timeline t JOIN pages p ON p.id = t.page_id WHERE p.slug = ?1 AND t.date >= ?3 ORDER BY t.date DESC LIMIT ?2"
        } else if !before.is_empty() {
            "SELECT t.id, p.slug, t.date, t.source, t.summary, t.detail, t.created_at FROM timeline t JOIN pages p ON p.id = t.page_id WHERE p.slug = ?1 AND t.date <= ?3 ORDER BY t.date DESC LIMIT ?2"
        } else {
            "SELECT t.id, p.slug, t.date, t.source, t.summary, t.detail, t.created_at FROM timeline t JOIN pages p ON p.id = t.page_id WHERE p.slug = ?1 ORDER BY t.date DESC LIMIT ?2"
        };

        let mut stmt = conn.prepare(query)?;

        let entries: Vec<TimelineEntry> = if !after.is_empty() && !before.is_empty() {
            stmt.query_map(params![slug, limit, after, before], |row| {
                Ok(TimelineEntry {
                    id: row.get(0)?, slug: row.get(1)?, date: row.get(2)?,
                    source: row.get(3)?, summary: row.get(4)?, detail: row.get(5)?, created_at: row.get(6)?,
                })
            })?.filter_map(|r| { if let Err(e) = &r { warn!(error = %e, "Row decode error"); } r.ok() }).collect()
        } else if !after.is_empty() || !before.is_empty() {
            let filter_date = if !after.is_empty() { after } else { before };
            stmt.query_map(params![slug, limit, filter_date], |row| {
                Ok(TimelineEntry {
                    id: row.get(0)?, slug: row.get(1)?, date: row.get(2)?,
                    source: row.get(3)?, summary: row.get(4)?, detail: row.get(5)?, created_at: row.get(6)?,
                })
            })?.filter_map(|r| { if let Err(e) = &r { warn!(error = %e, "Row decode error"); } r.ok() }).collect()
        } else {
            stmt.query_map(params![slug, limit], |row| {
                Ok(TimelineEntry {
                    id: row.get(0)?, slug: row.get(1)?, date: row.get(2)?,
                    source: row.get(3)?, summary: row.get(4)?, detail: row.get(5)?, created_at: row.get(6)?,
                })
            })?.filter_map(|r| { if let Err(e) = &r { warn!(error = %e, "Row decode error"); } r.ok() }).collect()
        };

        Ok(entries)
    }

    // ── Raw Data ───────────────────────────────────────────────

    fn put_raw_data(&self, slug: &str, key: &str, data: serde_json::Value) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO raw_data (page_id, source, data)
             VALUES ((SELECT id FROM pages WHERE slug = ?1), ?2, ?3)
             ON CONFLICT(page_id, source) DO UPDATE SET data = excluded.data, fetched_at = datetime('now')",
            params![slug, key, data.to_string()],
        )?;
        Ok(())
    }

    fn get_raw_data(&self, slug: &str, key: &str) -> Result<Option<serde_json::Value>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT data FROM raw_data WHERE page_id = (SELECT id FROM pages WHERE slug = ?1) AND source = ?2",
            params![slug, key],
            |row| row.get::<_, String>(0),
        );

        match result {
            Ok(json_str) => {
                let value: serde_json::Value = serde_json::from_str(&json_str)?;
                Ok(Some(value))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    // ── Versions ───────────────────────────────────────────────

    fn create_version(&self, slug: &str) -> Result<i64> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO page_versions (page_id, compiled_truth, frontmatter, title, page_type)
             SELECT id, compiled_truth, frontmatter, title, page_type FROM pages WHERE slug = ?1",
            params![slug],
        )?;
        Ok(conn.last_insert_rowid())
    }

    fn get_versions(&self, slug: &str, limit: Option<usize>) -> Result<Vec<PageVersion>> {
        let conn = self.conn()?;
        let limit = limit.unwrap_or(10);

        // Use v.title and v.page_type from the version snapshot, not the current page values
        let mut stmt = conn.prepare(
            "SELECT v.id, p.slug, v.page_type, v.title, v.compiled_truth, v.frontmatter, v.snapshot_at
             FROM page_versions v
             JOIN pages p ON p.id = v.page_id
             WHERE p.slug = ?1
             ORDER BY v.snapshot_at DESC
             LIMIT ?2"
        )?;

        let versions: Vec<PageVersion> = stmt
            .query_map(params![slug, limit], |row| {
                Ok(PageVersion {
                    id: row.get(0)?,
                    slug: row.get(1)?,
                    page_type: row.get(2)?,
                    title: row.get(3)?,
                    compiled_truth: row.get(4)?,
                    frontmatter: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })?
            .filter_map(|r| { if let Err(e) = &r { warn!(error = %e, "Row decode error"); } r.ok() })
            .collect();

        Ok(versions)
    }

    fn revert_to_version(&self, slug: &str, version_id: i64) -> Result<()> {
        self.transaction_with_engine(|_engine| {
            let conn = self.conn()?;
            // Fetch the compiled_truth from the version, verifying it belongs to this page
            let compiled_truth: String = conn.query_row(
                "SELECT pv.compiled_truth FROM page_versions pv
                 JOIN pages p ON p.id = pv.page_id
                 WHERE pv.id = ?1 AND p.slug = ?2",
                params![version_id, slug],
                |row| row.get(0),
            ).map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => GBrainError::PageNotFound(
                    format!("Version {} does not exist or does not belong to page '{}'", version_id, slug)
                ),
                e => GBrainError::Database(e.to_string()),
            })?;
            // Also restore title and page_type from the version snapshot (V8 columns)
            // Clear content_hash so skip-if-unchanged won't incorrectly skip subsequent writes
            conn.execute(
                "UPDATE pages SET
                    compiled_truth = ?1,
                    frontmatter = (SELECT frontmatter FROM page_versions WHERE id = ?2),
                    title = (SELECT title FROM page_versions WHERE id = ?2),
                    page_type = (SELECT page_type FROM page_versions WHERE id = ?2),
                    content_hash = NULL,
                    updated_at = datetime('now')
                 WHERE slug = ?3",
                params![compiled_truth, version_id, slug],
            )?;
            Ok(())
        })
    }

    // ── Stats + Health ─────────────────────────────────────────

    fn get_stats(&self) -> Result<BrainStats> {
        let conn = self.conn()?;

        let page_count: i64 = conn.query_row("SELECT COUNT(*) FROM pages", [], |row| row.get(0))?;
        let chunk_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))?;
        let embedded_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM chunks WHERE embedded_at IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let link_count: i64 = conn.query_row("SELECT COUNT(*) FROM links", [], |row| row.get(0))?;
        let tag_count: i64 = conn.query_row("SELECT COUNT(*) FROM tags", [], |row| row.get(0))?;
        let timeline_entry_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM timeline", [], |row| row.get(0))?;

        // pages_by_type: mirrors TS BrainStats.pages_by_type
        let mut stmt = conn
            .prepare("SELECT page_type, COUNT(*) FROM pages GROUP BY page_type ORDER BY page_type")?;
        let pages_by_type: std::collections::HashMap<String, i64> = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .filter_map(|r| { if let Err(e) = &r { warn!(error = %e, "Row decode error"); } r.ok() })
            .collect();

        Ok(BrainStats {
            page_count,
            chunk_count,
            embedded_count,
            link_count,
            tag_count,
            timeline_entry_count,
            pages_by_type,
        })
    }

    fn get_health(&self) -> Result<BrainHealth> {
        let conn = self.conn()?;
        let stats = self.get_stats()?;

        let page_count = stats.page_count.max(1) as f64;

        // Embed coverage
        let embed_coverage = if stats.chunk_count > 0 {
            stats.embedded_count as f64 / stats.chunk_count as f64
        } else {
            0.0
        };

        // Link coverage (pages with at least one link)
        let linked_pages: i64 = conn
            .query_row("SELECT COUNT(DISTINCT from_slug) FROM links", [], |row| {
                row.get(0)
            })
            .unwrap_or(0);
        let link_coverage = linked_pages as f64 / page_count;

        // Timeline coverage
        let timeline_pages: i64 = conn
            .query_row("SELECT COUNT(DISTINCT page_id) FROM timeline", [], |row| {
                row.get(0)
            })
            .unwrap_or(0);
        let timeline_coverage = timeline_pages as f64 / page_count;

        // Orphan pages — zero inbound AND zero outbound links (mirrors TS: "islanded pages")
        let orphan_pages: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pages p WHERE NOT EXISTS (SELECT 1 FROM links l WHERE l.to_slug = p.slug) AND NOT EXISTS (SELECT 1 FROM links l WHERE l.from_slug = p.slug)",
            [], |row| row.get(0)
        ).unwrap_or(0);

        // Dead links (links to non-existent pages)
        let dead_links: i64 = conn.query_row(
            "SELECT COUNT(*) FROM links l WHERE NOT EXISTS (SELECT 1 FROM pages p WHERE p.slug = l.to_slug)",
            [], |row| row.get(0)
        ).unwrap_or(0);

        // Stale pages (not updated in 30 days)
        let stale_pages: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pages WHERE updated_at < datetime('now', '-30 days')",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // Score components (weights: embed 35, link 25, timeline 15, orphans 15, dead_links 10)
        let embed_coverage_score = embed_coverage * 35.0;
        let link_density_score = link_coverage.min(1.0) * 25.0;
        let timeline_coverage_score = timeline_coverage.min(1.0) * 15.0;
        let no_orphans_score = if orphan_pages == 0 {
            15.0
        } else {
            (1.0 - orphan_pages as f64 / page_count).max(0.0) * 15.0
        };
        let no_dead_links_score = if dead_links == 0 {
            10.0
        } else {
            (1.0 - dead_links as f64 / stats.link_count.max(1) as f64).max(0.0) * 10.0
        };

        // Missing embeddings (chunks without vec_chunks entries)
        let missing_embeddings: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM chunks c WHERE NOT EXISTS (SELECT 1 FROM vec_chunks v WHERE v.chunk_id = c.id)",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // Most connected pages (top 5 by total link count)
        let most_connected: Vec<MostConnectedPage> = {
            match conn.prepare(
                "SELECT slug, (
                    SELECT COUNT(*) FROM links WHERE from_slug = p.slug
                ) + (
                    SELECT COUNT(*) FROM links WHERE to_slug = p.slug
                ) as total_links
                FROM pages p
                ORDER BY total_links DESC
                LIMIT 5",
            ) {
                Ok(mut stmt) => {
                stmt.query_map([], |row| {
                    Ok(MostConnectedPage {
                        slug: row.get(0)?,
                        link_count: row.get(1)?,
                    })
                })
                .ok()
                .map(|rows| rows.filter_map(|r| { if let Err(e) = &r { warn!(error = %e, "Row decode error"); } r.ok() }).collect())
                .unwrap_or_default()
                }
                Err(_) => Vec::new(),
            }
        };

        let brain_score = embed_coverage_score
            + link_density_score
            + timeline_coverage_score
            + no_orphans_score
            + no_dead_links_score;

        Ok(BrainHealth {
            brain_score,
            page_count: stats.page_count,
            embed_coverage,
            stale_pages,
            orphan_pages,
            dead_links,
            link_coverage,
            timeline_coverage,
            embed_coverage_score,
            link_density_score,
            timeline_coverage_score,
            no_orphans_score,
            no_dead_links_score,
            missing_embeddings,
            most_connected,
        })
    }

    // ── P2-5: Integrity + Orphan Detection ─────────────────────

    fn detect_orphans(&self) -> Result<Vec<String>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT p.slug FROM pages p WHERE NOT EXISTS (SELECT 1 FROM links l WHERE l.to_slug = p.slug) AND NOT EXISTS (SELECT 1 FROM links l WHERE l.from_slug = p.slug)"
        )?;
        let orphans: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(|r| { if let Err(e) = &r { warn!(error = %e, "Row decode error"); } r.ok() })
            .collect();
        Ok(orphans)
    }

    fn detect_dead_links(&self) -> Result<Vec<(String, String)>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT l.from_slug, l.to_slug FROM links l WHERE NOT EXISTS (SELECT 1 FROM pages p WHERE p.slug = l.to_slug)"
        )?;
        let dead: Vec<(String, String)> = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
            .filter_map(|r| { if let Err(e) = &r { warn!(error = %e, "Row decode error"); } r.ok() })
            .collect();
        Ok(dead)
    }

    // ── Ingest Log ─────────────────────────────────────────────

    fn log_ingest(&self, entry: IngestLogInput) -> Result<()> {
        let conn = self.conn()?;
        let pages_json = serde_json::to_string(&entry.pages_updated)?;
        let legacy_source = if entry.source_ref.is_empty() {
            entry.source_type.clone()
        } else {
            format!("{}:{}", entry.source_type, entry.source_ref)
        };
        conn.execute(
            "INSERT INTO ingest_log (source, source_type, source_ref, summary, pages_updated, status, error)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                legacy_source,
                entry.source_type,
                entry.source_ref,
                entry.summary,
                pages_json,
                entry.status,
                entry.error,
            ],
        )?;
        Ok(())
    }

    fn get_ingest_log(&self, limit: Option<usize>) -> Result<Vec<IngestLogEntry>> {
        let conn = self.conn()?;
        let limit = limit.unwrap_or(50);

        let mut stmt = conn.prepare(
            "SELECT id, source, source_type, source_ref, summary, pages_updated, status, error, created_at
             FROM ingest_log ORDER BY created_at DESC LIMIT ?1",
        )?;

        let entries: Vec<IngestLogEntry> = stmt
            .query_map(params![limit], |row| {
                let pages_json: String = row.get(5)?;
                let pages: Vec<String> = serde_json::from_str(&pages_json).unwrap_or_default();
                Ok(IngestLogEntry {
                    id: row.get(0)?,
                    source: row.get(1)?,
                    source_type: row.get::<_, String>(2).unwrap_or_default(),
                    source_ref: row.get::<_, String>(3).unwrap_or_default(),
                    summary: row.get::<_, String>(4).unwrap_or_default(),
                    pages_updated: pages,
                    status: row.get::<_, String>(6).unwrap_or_default(),
                    error: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })?
            .filter_map(|r| { if let Err(e) = &r { warn!(error = %e, "Row decode error"); } r.ok() })
            .collect();

        Ok(entries)
    }

    // ── Sync ───────────────────────────────────────────────────

    fn update_slug(&self, old_slug: &str, new_slug: &str) -> Result<()> {
        self.transaction(|tx| {
            // Update the page slug
            tx.execute(
                "UPDATE pages SET slug = ?1 WHERE slug = ?2",
                params![new_slug, old_slug],
            )?;
            // Update links referencing old slug
            tx.execute(
                "UPDATE links SET from_slug = ?1 WHERE from_slug = ?2",
                params![new_slug, old_slug],
            )?;
            tx.execute(
                "UPDATE links SET to_slug = ?1 WHERE to_slug = ?2",
                params![new_slug, old_slug],
            )?;
            // Update links.origin_slug (provenance metadata)
            tx.execute(
                "UPDATE links SET origin_slug = ?1 WHERE origin_slug = ?2",
                params![new_slug, old_slug],
            )?;
            // Update files.page_slug (direct slug reference, not FK)
            tx.execute(
                "UPDATE files SET page_slug = ?1 WHERE page_slug = ?2",
                params![new_slug, old_slug],
            )?;
            Ok(())
        })
    }

    fn rewrite_links(&self, old_slug: &str, new_slug: &str) -> Result<()> {
        self.transaction(|tx| {
            tx.execute(
                "UPDATE links SET from_slug = ?1 WHERE from_slug = ?2",
                params![new_slug, old_slug],
            )?;
            tx.execute(
                "UPDATE links SET to_slug = ?1 WHERE to_slug = ?2",
                params![new_slug, old_slug],
            )?;
            Ok(())
        })
    }

    // ── Config ─────────────────────────────────────────────────

    fn get_config(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT value FROM config WHERE key = ?1",
            params![key],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn set_config(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO config (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    // ── Migration Support ──────────────────────────────────────

    fn run_migration(&self, version: i32, sql: &str) -> Result<()> {
        self.transaction_with_engine(|_engine| {
            let conn = self.conn()?;
            conn.execute_batch(sql)?;
            conn.execute(
                "INSERT OR IGNORE INTO schema_version (version, applied_at) VALUES (?1, datetime('now'))",
                params![version],
            )?;
            Ok(())
        })
    }

    fn get_chunks_with_embeddings(&self) -> Result<Vec<(i64, String, Vec<f32>)>> {
        // This requires sqlite-vec; gracefully degrade
        Ok(Vec::new())
    }

    // ── File Storage ────────────────────────────────────────────

    fn file_upload(
        &self,
        source_path: &Path,
        slug: &str,
        opts: FileUploadOptions,
    ) -> Result<FileRecord> {
        debug!(source_path = %source_path.display(), slug = %slug, "Uploading file");
        let conn = self.conn()?;

        // Honor FileUploadOptions: check overwrite before inserting
        if !opts.overwrite {
            let filename = source_path
                .file_name()
                .ok_or_else(|| GBrainError::FileError("No filename".to_string()))?
                .to_string_lossy()
                .to_string();
            let existing: i64 = conn.query_row(
                "SELECT COUNT(*) FROM files WHERE page_slug = ?1 AND filename = ?2",
                params![slug, filename],
                |row| row.get(0),
            ).unwrap_or(0);
            if existing > 0 {
                return Err(GBrainError::InvalidInput(format!(
                    "file already exists for slug '{}' (set overwrite=true to replace)", slug
                )));
            }
        }

        // Read file
        let data = std::fs::read(source_path)?;

        // Check size from in-memory data (eliminates TOCTOU race with metadata check)
        if let Some(max_size) = opts.max_size_bytes {
            if data.len() > max_size {
                return Err(GBrainError::InvalidInput(format!(
                    "file size {} exceeds maximum {} bytes", data.len(), max_size
                )));
            }
        }

        let size_bytes = data.len() as i64;

        // Compute hash
        let hash = format!("{:x}", Sha256::digest(&data));

        // Detect MIME type
        let mime_type = infer::get(&data).map(|t| t.mime_type().to_string());

        // Get filename
        let filename = source_path
            .file_name()
            .ok_or_else(|| GBrainError::FileError("No filename".to_string()))?
            .to_string_lossy()
            .to_string();

        // Storage path: files/<slug>/<filename>
        let storage_path = format!("files/{}/{}", slug, filename);

        // Insert DB record first (before disk write) to prevent orphaned files
        conn.execute(
            "INSERT INTO files (page_slug, filename, storage_path, mime_type, size_bytes, checksum)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![slug, filename, storage_path, mime_type, size_bytes, hash],
        )?;

        // Write to disk after DB insert succeeds
        let base_dir = Config::base_dir();
        let file_dir = base_dir.join(&storage_path);
        if let Some(parent) = file_dir.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if let Err(e) = std::fs::write(&file_dir, &data) {
            // Disk write failed — clean up DB record to avoid inconsistency
            let _ = conn.execute(
                "DELETE FROM files WHERE page_slug = ?1 AND filename = ?2",
                params![slug, filename],
            );
            return Err(GBrainError::FileError(format!("Failed to write file: {}", e)));
        }

        let id = conn.last_insert_rowid();
        let created_at: String = conn.query_row(
            "SELECT created_at FROM files WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )?;

        Ok(FileRecord {
            id,
            slug: slug.to_string(),
            filename,
            storage_path,
            mime_type,
            size_bytes,
            hash: Some(hash),
            created_at,
        })
    }

    fn file_list(&self, slug: Option<&str>, limit: Option<usize>) -> Result<Vec<FileRecord>> {
        let conn = self.conn()?;
        let limit = limit.unwrap_or(50);

        let sql = if slug.is_some() {
            "SELECT id, page_slug, filename, storage_path, mime_type, size_bytes, checksum, created_at
             FROM files WHERE page_slug = ?1 ORDER BY created_at DESC LIMIT ?2"
        } else {
            "SELECT id, page_slug, filename, storage_path, mime_type, size_bytes, checksum, created_at
             FROM files ORDER BY created_at DESC LIMIT ?2"
        };

        let mut stmt = conn.prepare(sql)?;

        let records: Vec<FileRecord> = if let Some(s) = slug {
            stmt.query_map(params![s, limit], |row| {
                Ok(FileRecord {
                    id: row.get(0)?,
                    slug: row.get(1)?,
                    filename: row.get(2)?,
                    storage_path: row.get(3)?,
                    mime_type: row.get(4)?,
                    size_bytes: row.get::<_, Option<i64>>(5)?.unwrap_or(0),
                    hash: row.get(6)?,
                    created_at: row.get(7)?,
                })
            })?
            .filter_map(|r| { if let Err(e) = &r { warn!(error = %e, "Row decode error"); } r.ok() })
            .collect()
        } else {
            stmt.query_map(params![limit], |row| {
                Ok(FileRecord {
                    id: row.get(0)?,
                    slug: row.get(1)?,
                    filename: row.get(2)?,
                    storage_path: row.get(3)?,
                    mime_type: row.get(4)?,
                    size_bytes: row.get::<_, Option<i64>>(5)?.unwrap_or(0),
                    hash: row.get(6)?,
                    created_at: row.get(7)?,
                })
            })?
            .filter_map(|r| { if let Err(e) = &r { warn!(error = %e, "Row decode error"); } r.ok() })
            .collect()
        };

        Ok(records)
    }

    fn file_url(&self, file_id: i64, mode: FileUrlMode) -> Result<String> {
        let conn = self.conn()?;
        let storage_path: String = conn.query_row(
            "SELECT storage_path FROM files WHERE id = ?1",
            params![file_id],
            |row| row.get(0),
        )?;

        match mode {
            FileUrlMode::LocalPath => {
                let base_dir = Config::base_dir();
                Ok(base_dir.join(&storage_path).to_string_lossy().to_string())
            }
            FileUrlMode::Http { port } => Ok(format!("http://localhost:{}/{}", port, storage_path)),
        }
    }

    fn file_url_by_storage_path(&self, storage_path: &str) -> Result<String> {
        let conn = self.conn()?;
        let path: String = conn
            .query_row(
                "SELECT storage_path FROM files WHERE storage_path = ?1",
                params![storage_path],
                |row| row.get(0),
            )
            .map_err(|_| GBrainError::FileError(format!("File not found: {}", storage_path)))?;

        let base_dir = Config::base_dir();
        Ok(base_dir.join(&path).to_string_lossy().to_string())
    }

    fn file_verify(&self) -> Result<FileVerifyResult> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT storage_path, checksum FROM files ORDER BY storage_path LIMIT 1000")?;

        let mut verified = 0;
        let mut mismatches = 0;
        let mut missing = 0;

        let base_dir = Config::base_dir();

        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })?;

        for row in rows {
            let (storage_path, checksum) = row?;
            if storage_path.is_empty() || checksum.is_none() {
                mismatches += 1;
                continue;
            }
            // Verify file actually exists on disk and checksum matches
            let full_path = base_dir.join(&storage_path);
            match std::fs::read(&full_path) {
                Ok(data) => {
                    let actual_hash = format!("{:x}", Sha256::digest(&data));
                    if checksum.as_deref() == Some(actual_hash.as_str()) {
                        verified += 1;
                    } else {
                        mismatches += 1;
                    }
                }
                Err(_) => {
                    missing += 1;
                }
            }
        }

        Ok(FileVerifyResult {
            verified,
            mismatches,
            missing,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trigram_similarity_identical() {
        let score = SqliteEngine::trigram_similarity("hello", "hello");
        assert!(
            (score - 1.0).abs() < 0.001,
            "identical strings should be ~1.0, got {}",
            score
        );
    }

    #[test]
    fn test_trigram_similarity_empty() {
        assert_eq!(SqliteEngine::trigram_similarity("", "hello"), 0.0);
        assert_eq!(SqliteEngine::trigram_similarity("hello", ""), 0.0);
        assert_eq!(SqliteEngine::trigram_similarity("", ""), 0.0);
    }

    #[test]
    fn test_trigram_similarity_case_insensitive() {
        let score = SqliteEngine::trigram_similarity("Hello World", "hello world");
        assert!(
            (score - 1.0).abs() < 0.001,
            "case-insensitive should be ~1.0, got {}",
            score
        );
    }

    #[test]
    fn test_trigram_similarity_partial_match() {
        let score = SqliteEngine::trigram_similarity("Alice Wonderland", "Alice Wonder");
        assert!(
            score > 0.5,
            "partial match should score > 0.5, got {}",
            score
        );
    }

    #[test]
    fn test_trigram_similarity_no_match() {
        let score = SqliteEngine::trigram_similarity("xyz", "abc");
        assert!(
            score < 0.3,
            "unrelated strings should score < 0.3, got {}",
            score
        );
    }

    #[test]
    fn test_trigram_similarity_cjk() {
        let score = SqliteEngine::trigram_similarity("你好世界", "你好世界");
        assert!(
            (score - 1.0).abs() < 0.001,
            "CJK identical should be ~1.0, got {}",
            score
        );

        let partial = SqliteEngine::trigram_similarity("你好世界", "你好");
        assert!(
            partial > 0.2,
            "CJK partial match should score > 0.2, got {}",
            partial
        );
    }

    #[test]
    fn test_trigram_similarity_single_char() {
        let score = SqliteEngine::trigram_similarity("a", "a");
        assert!(
            (score - 1.0).abs() < 0.001,
            "single char identical should be ~1.0, got {}",
            score
        );
    }

    #[test]
    fn test_trigram_similarity_symmetric() {
        let score_ab = SqliteEngine::trigram_similarity("hello", "world");
        let score_ba = SqliteEngine::trigram_similarity("world", "hello");
        assert!(
            (score_ab - score_ba).abs() < 0.001,
            "similarity should be symmetric"
        );
    }

    #[test]
    fn test_trigram_similarity_padding_effect() {
        // pg_trgm pads with two spaces on each side
        // This means "ab" and "a" share boundary trigrams
        let score = SqliteEngine::trigram_similarity("ab", "a");
        assert!(
            score > 0.0,
            "padded trigrams should find some overlap, got {}",
            score
        );
    }
}
