//! Token synonym lookup for query expansion (E6 design).
//!
//! Runtime path: O(log n) SQLite lookup, zero embedding API calls.
//! Synonyms are mined offline by `mine_synonyms` (PR-3).

use rusqlite::{params, Connection};
use std::collections::HashMap;
use tracing::{info, warn};

use crate::embedding::Embedder;
use crate::error::{GBrainError, Result};

// ---------------------------------------------------------------------------
// Runtime lookup (PR-2)
// ---------------------------------------------------------------------------

/// Maximum number of synonyms returned per token at query time.
pub const MAX_RUNTIME_SYNONYMS: usize = 3;

/// Return the first active embedding index ID, or `None` if none exists.
///
/// Token synonyms are shared across libraries — they are keyed by
/// `embedding_index_id` which follows the current `GBRAIN_EMBEDDING_MODEL`.
/// Any active index suffices because all active indexes use the same model.
pub fn active_embedding_index_id(conn: &Connection) -> Option<i64> {
    conn.query_row(
        "SELECT id FROM kb_embedding_indexes WHERE is_active = 1 LIMIT 1",
        [],
        |row| row.get::<_, i64>(0),
    )
    .ok()
}

/// Look up synonym expansions for `token` from `kb_token_synonyms`.
///
/// Returns up to `limit` synonyms ordered by descending cosine similarity.
/// Gracefully returns an empty `Vec` when:
/// - no active embedding index exists (cold start)
/// - the `kb_token_synonyms` table does not yet exist
/// - no synonyms have been mined for this token
pub fn lookup_token_synonyms(conn: &Connection, token: &str, limit: usize) -> Vec<String> {
    let Some(active_idx_id) = active_embedding_index_id(conn) else {
        return Vec::new();
    };
    let Ok(mut stmt) = conn.prepare(
        "SELECT synonym FROM kb_token_synonyms \
         WHERE embedding_index_id = ?1 AND token = ?2 \
         ORDER BY score DESC LIMIT ?3",
    ) else {
        return Vec::new();
    };
    let rows = stmt.query_map(params![active_idx_id, token, limit as i64], |row| {
        row.get::<_, String>(0)
    });
    match rows {
        Ok(iter) => iter.flatten().collect(),
        Err(_) => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Offline mining (PR-3)
// ---------------------------------------------------------------------------

/// Default mining parameters — zero-config usable.
pub const DEFAULT_MIN_DOC_FREQ: usize = 10;
pub const DEFAULT_MAX_DOC_FREQ_RATIO: f64 = 0.3;
pub const DEFAULT_MIN_TOKEN_CHAR_LEN: usize = 2;
pub const DEFAULT_KNN_K: usize = 6;
pub const DEFAULT_SIMILARITY_THRESHOLD: f64 = 0.75;
pub const DEFAULT_MAX_SYNONYMS_PER_TOKEN: usize = 5;
pub const DEFAULT_MINING_BATCH_SIZE: usize = 200;
/// 并发 embed batch 数
pub const DEFAULT_MINING_CONCURRENCY: usize = 4;
/// embedding batch 最大重试次数
const MAX_EMBED_RETRIES: usize = 3;

/// Options controlling the offline synonym mining job.
#[derive(Debug)]
pub struct MineSynonymsOpts {
    pub library_id: Option<i64>,
    pub full: bool,
    pub min_doc_freq: usize,
    pub max_doc_freq_ratio: f64,
    pub min_token_char_len: usize,
    pub knn_k: usize,
    pub similarity_threshold: f64,
    pub max_synonyms_per_token: usize,
    pub batch_size: usize,
    /// 并发 embed batch 数（默认 4）
    pub concurrency: usize,
}

impl Default for MineSynonymsOpts {
    fn default() -> Self {
        Self {
            library_id: None,
            full: false,
            min_doc_freq: DEFAULT_MIN_DOC_FREQ,
            max_doc_freq_ratio: DEFAULT_MAX_DOC_FREQ_RATIO,
            min_token_char_len: DEFAULT_MIN_TOKEN_CHAR_LEN,
            knn_k: DEFAULT_KNN_K,
            similarity_threshold: DEFAULT_SIMILARITY_THRESHOLD,
            max_synonyms_per_token: DEFAULT_MAX_SYNONYMS_PER_TOKEN,
            batch_size: DEFAULT_MINING_BATCH_SIZE,
            concurrency: DEFAULT_MINING_CONCURRENCY,
        }
    }
}

/// Statistics returned by `mine_synonyms`.
#[derive(Debug, Default)]
pub struct MineSynonymsStats {
    pub candidates: usize,
    pub new_embeddings: usize,
    pub total_embeddings: usize,
    pub synonyms_written: usize,
}

// -- helpers ----------------------------------------------------------------

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f64 = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| f64::from(*x) * f64::from(*y))
        .sum();
    let na: f64 = a
        .iter()
        .map(|x| f64::from(*x) * f64::from(*x))
        .sum::<f64>()
        .sqrt();
    let nb: f64 = b
        .iter()
        .map(|x| f64::from(*x) * f64::from(*x))
        .sum::<f64>()
        .sqrt();
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    (dot / (na * nb)) as f32
}

fn is_substring_or_superstring(a: &str, b: &str) -> bool {
    a.contains(b) || b.contains(a)
}

fn blob_to_f32(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

fn f32_to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

// -- step 1: extract candidate tokens with DF --------------------------------

fn extract_candidate_tokens(
    conn: &Connection,
    opts: &MineSynonymsOpts,
) -> Result<Vec<(String, usize)>> {
    let lib_id = opts.library_id;
    let mut df: HashMap<String, usize> = HashMap::new();
    let mut total_nodes: usize = 0;

    // Collect all node content strings, then process for DF
    let contents: Vec<String> = match lib_id {
        Some(id) => {
            let mut stmt = conn
                .prepare(
                    "SELECT content FROM kb_document_nodes WHERE library_id = ?1 AND level = 0",
                )
                .map_err(|e| GBrainError::Database(e.to_string()))?;
            let mapped = stmt
                .query_map(params![id], |row| row.get::<_, String>(0))
                .map_err(|e| GBrainError::Database(e.to_string()))?;
            mapped.filter_map(|r| r.ok()).collect()
        }
        None => {
            let mut stmt = conn
                .prepare("SELECT content FROM kb_document_nodes WHERE level = 0")
                .map_err(|e| GBrainError::Database(e.to_string()))?;
            let mapped = stmt
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|e| GBrainError::Database(e.to_string()))?;
            mapped.filter_map(|r| r.ok()).collect()
        }
    };

    for content in &contents {
        let tokens_str = crate::nlp::chinese::tokenize_content(content);
        let unique: std::collections::HashSet<String> = tokens_str
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
        for token in &unique {
            *df.entry(token.clone()).or_insert(0) += 1;
        }
        total_nodes += 1;
    }

    info!(
        total_nodes,
        unique_tokens = df.len(),
        "Token extraction complete"
    );

    let min_df = opts.min_doc_freq;
    let max_df = ((total_nodes as f64) * opts.max_doc_freq_ratio) as usize;
    let min_chars = opts.min_token_char_len;

    let mut candidates: Vec<(String, usize)> = df
        .into_iter()
        .filter(|(_, freq)| *freq >= min_df && *freq <= max_df)
        .filter(|(token, _)| token.chars().count() >= min_chars)
        .collect();

    // Important words first
    candidates.sort_by(|a, b| b.1.cmp(&a.1));
    Ok(candidates)
}

// -- step 2: load existing embeddings ----------------------------------------

fn load_existing_token_embeddings(
    conn: &Connection,
    index_id: i64,
) -> Result<HashMap<String, Vec<f32>>> {
    let Ok(mut stmt) = conn
        .prepare("SELECT token, embedding FROM kb_token_embeddings WHERE embedding_index_id = ?1")
    else {
        return Ok(HashMap::new());
    };
    let rows = stmt
        .query_map(params![index_id], |row| {
            let token: String = row.get(0)?;
            let blob: Vec<u8> = row.get(1)?;
            Ok((token, blob))
        })
        .map_err(|e| GBrainError::Database(e.to_string()))?;

    let mut map = HashMap::new();
    for row in rows {
        let (token, blob) = row.map_err(|e| GBrainError::Database(e.to_string()))?;
        map.insert(token, blob_to_f32(&blob));
    }
    Ok(map)
}

// -- step 3: embed and store (带重试 + 并发) ----------------------------------

/// embedding batch 带指数退避重试。
/// 成功返回 `Some(vectors)`，最终失败返回 `None`。
fn embed_batch_with_retry(
    embedder: &Embedder,
    rt: &tokio::runtime::Runtime,
    texts: &[&str],
) -> Option<Vec<Vec<f32>>> {
    for attempt in 0..MAX_EMBED_RETRIES {
        match rt.block_on(embedder.embed_batch(texts)) {
            Ok(v) => return Some(v),
            Err(e) => {
                warn!(
                    attempt = attempt + 1,
                    max = MAX_EMBED_RETRIES,
                    error = %e,
                    "Batch embedding 失败"
                );
                if attempt + 1 < MAX_EMBED_RETRIES {
                    // 指数退避：2^attempt 秒
                    std::thread::sleep(std::time::Duration::from_secs(1u64 << attempt));
                }
            }
        }
    }
    None
}

/// embed 新 token 并写入 kb_token_embeddings。
///
/// - 按 `concurrency` 分组并发 embed（`std::thread::scope`）
/// - 写入 DB 仍然串行（`Connection` 非线程安全）
/// - 每个 batch 失败时自动重试最多 `MAX_EMBED_RETRIES` 次
fn embed_and_store_tokens(
    conn: &Connection,
    embedder: &Embedder,
    rt: &tokio::runtime::Runtime,
    tokens: &[(String, usize)],
    index_id: i64,
    batch_size: usize,
    concurrency: usize,
) -> Result<usize> {
    let chunks: Vec<_> = tokens.chunks(batch_size).collect();
    let mut embedded = 0usize;

    // 按 concurrency 分组处理
    for group in chunks.chunks(concurrency.max(1)) {
        // 并发 embed：每组内各 chunk 同时请求 embedding API
        let group_results: Vec<Option<Vec<Vec<f32>>>> = if concurrency > 1 {
            std::thread::scope(|s| {
                group
                    .iter()
                    .map(|chunk| {
                        let texts: Vec<String> = chunk.iter().map(|(t, _)| t.clone()).collect();
                        s.spawn(move || {
                            let refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
                            embed_batch_with_retry(embedder, rt, &refs)
                        })
                    })
                    .collect::<Vec<_>>()
                    .into_iter()
                    .map(|h| h.join().unwrap_or(None))
                    .collect()
            })
        } else {
            // concurrency=1：直接在当前线程执行，避免不必要的线程创建
            group
                .iter()
                .map(|chunk| {
                    let texts: Vec<&str> = chunk.iter().map(|(t, _)| t.as_str()).collect();
                    embed_batch_with_retry(embedder, rt, &texts)
                })
                .collect()
        };

        // 串行写入 DB（Connection 非线程安全）
        for (chunk, result) in group.iter().zip(group_results.iter()) {
            match result {
                None => {
                    warn!(count = chunk.len(), "Batch embedding 最终失败，跳过该批");
                    continue;
                }
                Some(vectors) => {
                    for ((token, doc_freq), vec) in chunk.iter().zip(vectors.iter()) {
                        let blob = f32_to_blob(vec);
                        conn.execute(
                            "INSERT OR REPLACE INTO kb_token_embeddings \
                             (token, embedding_index_id, embedding, doc_freq) \
                             VALUES (?1, ?2, ?3, ?4)",
                            params![token, index_id, blob, *doc_freq as i64],
                        )
                        .map_err(|e| GBrainError::Database(e.to_string()))?;
                        embedded += 1;
                    }
                    info!(batch = chunk.len(), embedded, "Embedded token batch");
                }
            }
        }
    }
    Ok(embedded)
}

// -- step 4: KNN mining (sqlite-vec + 内存回退) ------------------------------

/// token 向量虚表名（与节点向量 `vec_kb_{id}` 区分）
fn token_vec_table_name(index_id: i64) -> String {
    format!("vec_token_{}", index_id)
}

/// 内存暴力 KNN（O(n²)），用于测试和 sqlite-vec 不可用时的回退
fn knn_mine_brute_force(
    all: &HashMap<String, Vec<f32>>,
    opts: &MineSynonymsOpts,
) -> Vec<(String, String, f32)> {
    let tokens: Vec<String> = all.keys().cloned().collect();
    let mut pairs = Vec::new();

    for (i, token_a) in tokens.iter().enumerate() {
        let emb_a = &all[token_a];
        let mut neighbors: Vec<(f32, String)> = Vec::new();

        for (j, token_b) in tokens.iter().enumerate() {
            if i == j {
                continue;
            }
            let sim = cosine_similarity(emb_a, &all[token_b]);
            if sim >= opts.similarity_threshold as f32 {
                neighbors.push((sim, token_b.clone()));
            }
        }

        neighbors.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        neighbors.truncate(opts.knn_k.saturating_sub(1));

        for (score, token_b) in neighbors {
            if is_substring_or_superstring(token_a, &token_b) {
                continue;
            }
            pairs.push((token_a.clone(), token_b, score));
        }
    }
    pairs
}

/// 使用 sqlite-vec 虚表做 KNN 挖掘。
///
/// 流程：创建临时虚表 → 批量 INSERT → 逐 token KNN 查询 → 清理虚表。
fn knn_mine_via_vec(
    conn: &Connection,
    index_id: i64,
    dimensions: i32,
    all: &HashMap<String, Vec<f32>>,
    opts: &MineSynonymsOpts,
) -> Result<Vec<(String, String, f32)>> {
    let table = token_vec_table_name(index_id);

    // 1. 创建 cosine 距离度量的 sqlite-vec 虚表
    conn.execute_batch(&format!(
        "CREATE VIRTUAL TABLE IF NOT EXISTS {} USING vec0(\
         embedding float[{}] metric cosine)",
        table, dimensions,
    ))
    .map_err(|e| GBrainError::Database(e.to_string()))?;

    // 2. 清空旧数据并批量插入
    conn.execute_batch(&format!("DELETE FROM {}", table))
        .map_err(|e| GBrainError::Database(e.to_string()))?;

    let tokens: Vec<String> = all.keys().cloned().collect();
    {
        let mut stmt = conn
            .prepare(&format!(
                "INSERT INTO {} (rowid, embedding) VALUES (?1, ?2)",
                table
            ))
            .map_err(|e| GBrainError::Database(e.to_string()))?;

        for (i, token) in tokens.iter().enumerate() {
            let blob = f32_to_blob(&all[token]);
            stmt.execute(params![(i + 1) as i64, blob])
                .map_err(|e| GBrainError::Database(e.to_string()))?;
        }
    }

    // 3. 对每个 token 执行 KNN 查询
    //    cosine 距离 = 1 - cosine_similarity
    let max_distance = 1.0_f32 - opts.similarity_threshold as f32;
    let k = opts.knn_k + 1; // +1 因为查询自身会出现在结果中
    let mut pairs = Vec::new();

    for (i, token_a) in tokens.iter().enumerate() {
        let query_blob = f32_to_blob(&all[token_a]);
        let mut stmt = conn
            .prepare(&format!(
                "SELECT rowid, distance FROM {} \
                 WHERE embedding MATCH ?1 AND k = ?2 \
                 ORDER BY distance",
                table
            ))
            .map_err(|e| GBrainError::Database(e.to_string()))?;

        let neighbors: Vec<(i64, f32)> = stmt
            .query_map(params![query_blob, k as i64], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, f32>(1)?))
            })
            .map_err(|e| GBrainError::Database(e.to_string()))?
            .filter_map(|r| r.ok())
            .filter(|(rowid, dist)| {
                // 排除自身（rowid = i+1），过滤低于阈值的
                *rowid != (i as i64) + 1 && *dist <= max_distance
            })
            .collect();

        for (rowid, distance) in neighbors {
            let token_b = &tokens[(rowid - 1) as usize];
            let score = 1.0 - distance; // cosine distance → similarity
            if is_substring_or_superstring(token_a, token_b) {
                continue;
            }
            pairs.push((token_a.clone(), token_b.clone(), score));
        }
    }

    // 4. 清理临时虚表
    let _ = conn.execute_batch(&format!("DROP TABLE IF EXISTS {}", table));

    Ok(pairs)
}

/// KNN 同义词挖掘入口。
///
/// 优先使用 sqlite-vec 虚表（可处理大规模 token），不可用时回退到内存暴力搜索。
fn knn_mine(
    conn: &Connection,
    index_id: i64,
    dimensions: i32,
    all: &HashMap<String, Vec<f32>>,
    opts: &MineSynonymsOpts,
) -> Vec<(String, String, f32)> {
    match knn_mine_via_vec(conn, index_id, dimensions, all, opts) {
        Ok(pairs) => pairs,
        Err(e) => {
            warn!(error = %e, "sqlite-vec KNN 失败，回退到内存暴力搜索");
            knn_mine_brute_force(all, opts)
        }
    }
}

// -- step 5: write synonyms bidirectionally ----------------------------------

fn write_synonyms_bidirectional(
    conn: &Connection,
    pairs: Vec<(String, String, f32)>,
    index_id: i64,
    max_per_token: usize,
) -> Result<usize> {
    let mut by_token: HashMap<String, Vec<(String, f32)>> = HashMap::new();
    for (token, synonym, score) in &pairs {
        by_token
            .entry(token.clone())
            .or_default()
            .push((synonym.clone(), *score));
        // Reverse direction
        by_token
            .entry(synonym.clone())
            .or_default()
            .push((token.clone(), *score));
    }

    let mut written = 0usize;
    for (token, mut synonyms) in by_token {
        synonyms.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        synonyms.truncate(max_per_token);
        for (synonym, score) in synonyms {
            conn.execute(
                "INSERT OR REPLACE INTO kb_token_synonyms \
                 (token, synonym, score, embedding_index_id) \
                 VALUES (?1, ?2, ?3, ?4)",
                params![token, synonym, score, index_id],
            )
            .map_err(|e| GBrainError::Database(e.to_string()))?;
            written += 1;
        }
    }
    Ok(written)
}

// -- main entry point --------------------------------------------------------

/// Run the offline synonym mining pipeline.
///
/// Steps: extract tokens → embed → KNN → write synonyms.
/// Requires an active embedding index and a configured `Embedder`.
/// The `rt` tokio runtime is used for async embedding calls.
pub fn mine_synonyms(
    conn: &Connection,
    embedder: &Embedder,
    dimensions: i32,
    rt: &tokio::runtime::Runtime,
    opts: &MineSynonymsOpts,
) -> Result<MineSynonymsStats> {
    let mut stats = MineSynonymsStats::default();

    // 1. Resolve active embedding index
    let (index_id, dims): (i64, i32) = {
        let sql = match opts.library_id {
            Some(_) => {
                "SELECT id, dimensions FROM kb_embedding_indexes \
                        WHERE library_id = ?1 AND is_active = 1 LIMIT 1"
            }
            None => {
                "SELECT id, dimensions FROM kb_embedding_indexes \
                     WHERE is_active = 1 LIMIT 1"
            }
        };
        let r = match opts.library_id {
            Some(lib_id) => conn.query_row(sql, params![lib_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i32>(1)?))
            }),
            None => conn.query_row(sql, [], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i32>(1)?))
            }),
        };
        r.map_err(|e| {
            GBrainError::Database(format!(
                "No active embedding index found (configure GBRAIN_EMBEDDING_MODEL): {}",
                e
            ))
        })?
    };
    let dims = if dimensions > 0 { dimensions } else { dims };
    info!(index_id, dims, "Starting synonym mining");

    // 2. Extract candidate tokens with DF
    let candidates = extract_candidate_tokens(conn, opts)?;
    stats.candidates = candidates.len();
    info!(candidates = candidates.len(), "Extracted candidate tokens");
    if candidates.is_empty() {
        info!("No candidate tokens found, nothing to mine");
        return Ok(stats);
    }

    // 3. Load existing embeddings
    let mut all_embeddings = load_existing_token_embeddings(conn, index_id)?;

    // 4. Filter new tokens (unless full rebuild)
    let new_tokens: Vec<(String, usize)> = if opts.full {
        candidates
    } else {
        candidates
            .into_iter()
            .filter(|(t, _)| !all_embeddings.contains_key(t))
            .collect()
    };
    info!(
        new_tokens = new_tokens.len(),
        existing = all_embeddings.len(),
        "Token breakdown"
    );

    // 5. Embed new tokens
    if !new_tokens.is_empty() {
        let embedded = embed_and_store_tokens(
            conn,
            embedder,
            rt,
            &new_tokens,
            index_id,
            opts.batch_size,
            opts.concurrency,
        )?;
        stats.new_embeddings = embedded;
        // Reload all embeddings (old + new)
        all_embeddings = load_existing_token_embeddings(conn, index_id)?;
    }
    stats.total_embeddings = all_embeddings.len();

    // 6. KNN mine（优先 sqlite-vec，回退到内存暴力搜索）
    let pairs = knn_mine(conn, index_id, dims, &all_embeddings, opts);
    info!(candidate_pairs = pairs.len(), "KNN mining complete");

    // 7. Write synonyms bidirectionally
    let written = write_synonyms_bidirectional(conn, pairs, index_id, opts.max_synonyms_per_token)?;
    stats.synonyms_written = written;
    info!(written, "Synonym mining complete");

    Ok(stats)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(&schema::SCHEMA_DDL).unwrap();
        conn
    }

    /// Insert a library + active embedding index and return the index id.
    fn insert_active_index(conn: &Connection, library_id: i64) -> i64 {
        conn.execute(
            "INSERT OR IGNORE INTO kb_libraries (id, name) VALUES (?1, 'test')",
            params![library_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO kb_embedding_indexes \
             (id, library_id, provider, model, dimensions, index_type, is_active) \
             VALUES (1, ?1, 'openai', 'text-embedding-3-large', 1536, 'dense', 1)",
            params![library_id],
        )
        .unwrap();
        1
    }

    // -- PR-2 lookup tests ---------------------------------------------------

    #[test]
    fn empty_when_no_active_index() {
        let conn = test_conn();
        let result = lookup_token_synonyms(&conn, "积分", 3);
        assert!(result.is_empty());
    }

    #[test]
    fn empty_when_table_empty() {
        let conn = test_conn();
        insert_active_index(&conn, 1);
        let result = lookup_token_synonyms(&conn, "积分", 3);
        assert!(result.is_empty());
    }

    #[test]
    fn returns_matching_synonyms() {
        let conn = test_conn();
        let idx_id = insert_active_index(&conn, 1);
        conn.execute(
            "INSERT INTO kb_token_synonyms (token, synonym, score, embedding_index_id) \
             VALUES ('积分', 'points', 0.92, ?1)",
            params![idx_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO kb_token_synonyms (token, synonym, score, embedding_index_id) \
             VALUES ('积分', 'lp', 0.88, ?1)",
            params![idx_id],
        )
        .unwrap();

        let result = lookup_token_synonyms(&conn, "积分", 3);
        assert_eq!(result, vec!["points", "lp"]);
    }

    #[test]
    fn respects_limit() {
        let conn = test_conn();
        let idx_id = insert_active_index(&conn, 1);
        conn.execute(
            "INSERT INTO kb_token_synonyms (token, synonym, score, embedding_index_id) \
             VALUES ('积分', 'points', 0.92, ?1)",
            params![idx_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO kb_token_synonyms (token, synonym, score, embedding_index_id) \
             VALUES ('积分', 'lp', 0.88, ?1)",
            params![idx_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO kb_token_synonyms (token, synonym, score, embedding_index_id) \
             VALUES ('积分', 'credit', 0.85, ?1)",
            params![idx_id],
        )
        .unwrap();

        let result = lookup_token_synonyms(&conn, "积分", 2);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "points"); // highest score first
    }

    #[test]
    fn no_match_returns_empty() {
        let conn = test_conn();
        insert_active_index(&conn, 1);
        let result = lookup_token_synonyms(&conn, "nonexistent_token", 3);
        assert!(result.is_empty());
    }

    #[test]
    fn index_isolation() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO kb_libraries (id, name) VALUES (1, 'test1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO kb_embedding_indexes \
             (id, library_id, provider, model, dimensions, index_type, is_active) \
             VALUES (1, 1, 'openai', 'text-embedding-3-large', 1536, 'dense', 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO kb_libraries (id, name) VALUES (2, 'test2')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO kb_embedding_indexes \
             (id, library_id, provider, model, dimensions, index_type, is_active) \
             VALUES (2, 2, 'openai', 'text-embedding-3-large', 1536, 'dense', 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO kb_token_synonyms \
             (token, synonym, score, embedding_index_id) VALUES ('积分', 'points', 0.92, 2)",
            [],
        )
        .unwrap();

        let result = lookup_token_synonyms(&conn, "积分", 3);
        assert!(result.is_empty());
    }

    // -- PR-3 mining tests ---------------------------------------------------

    #[test]
    fn cosine_similarity_basic() {
        let a = vec![1.0_f32, 0.0, 0.0];
        let b = vec![0.0_f32, 1.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 0.0).abs() < 1e-6);

        let c = vec![1.0_f32, 0.0, 0.0];
        assert!((cosine_similarity(&a, &c) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn substring_filter() {
        assert!(is_substring_or_superstring("积分", "分"));
        assert!(is_substring_or_superstring("transaction", "trans"));
        assert!(!is_substring_or_superstring("积分", "points"));
    }

    #[test]
    fn blob_roundtrip() {
        let original = vec![1.0_f32, -2.5, 0.0, 3.14];
        let blob = f32_to_blob(&original);
        let restored = blob_to_f32(&blob);
        assert_eq!(original, restored);
    }

    #[test]
    fn write_bidirectional_creates_both_directions() {
        let conn = test_conn();
        let idx_id = insert_active_index(&conn, 1);

        let pairs = vec![("积分".to_string(), "points".to_string(), 0.92_f32)];
        let written = write_synonyms_bidirectional(&conn, pairs, idx_id, 5).unwrap();
        assert_eq!(written, 2); // A→B + B→A

        // Verify both directions
        let ab: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT synonym FROM kb_token_synonyms WHERE token = '积分' AND embedding_index_id = ?1")
                .unwrap();
            stmt.query_map(params![idx_id], |row| row.get::<_, String>(0))
                .unwrap()
                .flatten()
                .collect()
        };
        assert_eq!(ab, vec!["points"]);

        let ba: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT synonym FROM kb_token_synonyms WHERE token = 'points' AND embedding_index_id = ?1")
                .unwrap();
            stmt.query_map(params![idx_id], |row| row.get::<_, String>(0))
                .unwrap()
                .flatten()
                .collect()
        };
        assert_eq!(ba, vec!["积分"]);
    }

    #[test]
    fn write_bidirectional_respects_max_per_token() {
        let conn = test_conn();
        let idx_id = insert_active_index(&conn, 1);

        let pairs = vec![
            ("a".to_string(), "b".to_string(), 0.9),
            ("a".to_string(), "c".to_string(), 0.8),
            ("a".to_string(), "d".to_string(), 0.7),
        ];
        let written = write_synonyms_bidirectional(&conn, pairs, idx_id, 2).unwrap();
        // a has 3 synonyms → capped to 2 (b, c)
        // b has 1 synonym (a) → 1
        // c has 1 synonym (a) → 1
        // d has 1 synonym (a) → but a→d was capped, so d still gets a→d? No...
        // Actually: a gets b,c,d but capped to 2 (b,c)
        // b gets a → 1
        // c gets a → 1
        // d gets a → 1 (the reverse direction is independent)
        assert_eq!(written, 5); // a→b, a→c, b→a, c→a, d→a
    }

    #[test]
    fn knn_mine_basic() {
        // Construct embeddings where token_a and token_b are similar,
        // token_c is orthogonal.
        let all: HashMap<String, Vec<f32>> = HashMap::from([
            ("a".to_string(), vec![1.0, 0.0]),
            ("b".to_string(), vec![0.95, 0.05]), // very close to a
            ("c".to_string(), vec![0.0, 1.0]),   // orthogonal
        ]);
        let opts = MineSynonymsOpts {
            knn_k: 3,
            similarity_threshold: 0.9,
            ..Default::default()
        };
        let pairs = knn_mine_brute_force(&all, &opts);
        // a↔b should be found, c should have no pairs above 0.9
        assert!(pairs.iter().any(|(a, b, _)| a == "a" && b == "b"));
        assert!(pairs.iter().any(|(a, b, _)| a == "b" && b == "a"));
        assert!(!pairs.iter().any(|(a, _, _)| a == "c"));
    }

    #[test]
    fn knn_mine_filters_substrings() {
        let all: HashMap<String, Vec<f32>> = HashMap::from([
            ("trans".to_string(), vec![1.0, 0.0]),
            ("transaction".to_string(), vec![0.99, 0.01]), // similar AND substring
        ]);
        let opts = MineSynonymsOpts {
            knn_k: 3,
            similarity_threshold: 0.9,
            ..Default::default()
        };
        let pairs = knn_mine_brute_force(&all, &opts);
        assert!(pairs.is_empty()); // filtered because "trans" ⊂ "transaction"
    }

    #[test]
    fn extract_candidates_respects_df_bounds() {
        let conn = test_conn();
        insert_active_index(&conn, 1);
        // Insert a parent document (FK requirement)
        conn.execute(
            "INSERT INTO kb_documents \
             (id, library_id, original_name, content_hash, extension, mime_type) \
             VALUES (1, 1, 'test', 'hash', 'txt', 'text/plain')",
            [],
        )
        .unwrap();
        // 10 nodes with "积分" (common word), 3 nodes with "生僻" (rare word),
        // 5 nodes without either (background noise)
        for i in 0..10u32 {
            conn.execute(
                "INSERT INTO kb_document_nodes (library_id, document_id, content, level) \
                 VALUES (1, 1, ?1, 0)",
                params![format!("积分系统使用说明第{}页", i)],
            )
            .unwrap();
        }
        for i in 0..3u32 {
            conn.execute(
                "INSERT INTO kb_document_nodes (library_id, document_id, content, level) \
                 VALUES (1, 1, ?1, 0)",
                params![format!("生僻概念解释说明第{}页", i)],
            )
            .unwrap();
        }
        for i in 0..5u32 {
            conn.execute(
                "INSERT INTO kb_document_nodes (library_id, document_id, content, level) \
                 VALUES (1, 1, ?1, 0)",
                params![format!("通用背景文档第{}页", i)],
            )
            .unwrap();
        }

        let opts = MineSynonymsOpts {
            min_doc_freq: 5,         // "生僻" won't pass (DF=3)
            max_doc_freq_ratio: 0.9, // "积分" will pass (DF=10/18=56%)
            min_token_char_len: 2,
            ..Default::default()
        };
        let tokens = extract_candidate_tokens(&conn, &opts).unwrap();
        let token_set: Vec<&str> = tokens.iter().map(|(t, _)| t.as_str()).collect();
        // "积分" should appear (DF=10 >= 5, 10/18=56% <= 90%)
        assert!(
            token_set.iter().any(|t| t.contains("积分")),
            "expected 积分 in {:?}",
            token_set
        );
        // "生僻" should NOT appear (DF=3 < min_doc_freq=5)
        assert!(
            !token_set.iter().any(|t| t.contains("生僻")),
            "生僻 should have been filtered by min_doc_freq"
        );
    }
}
