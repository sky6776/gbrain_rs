//! KB hybrid search: vector KNN + FTS5 BM25 + RRF fusion
//!
//! Implements a two-retriever search pipeline:
//! 1. FTS5 keyword search using `kb_doc_fts` with jieba-tokenized query
//! 2. Vector search using sqlite-vec (with BLOB fallback via `kb_node_embeddings`)
//! 3. RRF (Reciprocal Rank Fusion) merge with k=60
//! 4. Fetch node details with document and library names
//!
//! All functions accept `&Connection` so callers control the transaction scope.

use crate::error::Result;
use crate::kb::types::*;
use crate::nlp::chinese;
use rusqlite::Connection;
use std::collections::HashMap;

/// RRF smoothing constant. Higher k dampens the effect of individual rank
/// positions, making the merge more robust to outlier rankings.
const RRF_K: usize = 60;

/// Perform KB hybrid search: vector KNN + FTS5 BM25 merged via RRF.
///
/// - `conn`: SQLite connection to the brain database
/// - `input`: Search parameters (library IDs, query text, level filter, top-k)
/// - `query_vector`: Pre-computed embedding for the query. If None, only FTS5
///   keyword search is performed (degraded mode).
///
/// Returns results sorted by descending RRF score.
pub fn kb_search(
    conn: &Connection,
    input: &KbSearchInput,
    query_vector: Option<&[f32]>,
) -> Result<Vec<KbSearchResult>> {
    let fetch_k = (input.top_k * 3).max(30);

    // Vector search (skip if no embedding provided)
    let vec_results = match query_vector {
        Some(vec) => kb_vector_search(conn, vec, &input.library_ids, input.level, fetch_k)?,
        None => Vec::new(),
    };

    // FTS5 keyword search with jieba tokenization
    let fts_results = kb_fts_search(conn, &input.query, &input.library_ids, input.level, fetch_k)?;

    // RRF merge
    let merged = rrf_merge(vec_results, fts_results);

    // Fetch full node details
    let results = fetch_node_details(conn, &merged, input.top_k)?;

    Ok(results)
}

/// Vector similarity search using sqlite-vec KNN.
///
/// Falls back to brute-force cosine similarity over `kb_node_embeddings`
/// BLOB storage if sqlite-vec is unavailable or returns no results.
///
/// Returns results ordered by rank (position in the result list).
pub fn kb_vector_search(
    conn: &Connection,
    embedding: &[f32],
    library_ids: &[i64],
    level: Option<i32>,
    top_k: usize,
) -> Result<Vec<RankedResult>> {
    let query_blob = embedding_to_blob(embedding);

    // Try sqlite-vec first
    let result = try_vec_knn(conn, &query_blob, library_ids, level, top_k);

    match result {
        Ok(results) if !results.is_empty() => Ok(results),
        _ => {
            // Fallback to brute-force cosine similarity on BLOB table
            vector_search_fallback(conn, embedding, library_ids, level, top_k)
        }
    }
}

/// FTS5 keyword search using `kb_doc_fts` with jieba tokenization.
///
/// The query string is tokenized via `chinese::build_fts_match_query` which
/// uses jieba segmentation for Chinese text and produces a prefix-match FTS5
/// query string. Results are ranked by BM25 score (ascending = more relevant).
pub fn kb_fts_search(
    conn: &Connection,
    query: &str,
    library_ids: &[i64],
    level: Option<i32>,
    top_k: usize,
) -> Result<Vec<RankedResult>> {
    let token_query = chinese::build_fts_match_query(query);

    if token_query.is_empty() {
        return Ok(Vec::new());
    }

    let mut sql = String::from(
        "SELECT f.rowid \
         FROM kb_doc_fts f \
         INNER JOIN kb_document_nodes n ON n.id = f.rowid \
         WHERE kb_doc_fts MATCH ?1",
    );

    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(token_query)];

    if !library_ids.is_empty() {
        let start = param_values.len() + 1;
        let placeholders: Vec<String> = library_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", start + i))
            .collect();
        sql.push_str(&format!(
            " AND n.library_id IN ({})",
            placeholders.join(",")
        ));
        for &id in library_ids {
            param_values.push(Box::new(id));
        }
    }

    if let Some(lvl) = level {
        let idx = param_values.len() + 1;
        sql.push_str(&format!(" AND n.level = ?{}", idx));
        param_values.push(Box::new(lvl));
    }

    let limit_idx = param_values.len() + 1;
    sql.push_str(&format!(
        " ORDER BY bm25(kb_doc_fts) ASC LIMIT ?{}",
        limit_idx
    ));
    param_values.push(Box::new(top_k as i64));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(param_refs.as_slice())?;

    let mut results = Vec::new();
    while let Some(row) = rows.next()? {
        let node_id: i64 = row.get(0)?;
        results.push(RankedResult {
            node_id,
            rank: results.len() + 1,
            score: 0.0,
        });
    }

    Ok(results)
}

/// Reciprocal Rank Fusion merge of two ranked result lists.
///
/// Each result's contribution is `1 / (k + rank)` where k is the smoothing
/// constant (60 by default). Results appearing in both lists get combined
/// scores, rewarding agreement between retrieval methods.
///
/// Returns results sorted by descending fused score, with rank reassigned.
pub fn rrf_merge(
    vec_results: Vec<RankedResult>,
    fts_results: Vec<RankedResult>,
) -> Vec<RankedResult> {
    let mut scores: HashMap<i64, f64> = HashMap::new();

    for r in &vec_results {
        *scores.entry(r.node_id).or_insert(0.0) += 1.0 / (RRF_K + r.rank) as f64;
    }

    for r in &fts_results {
        *scores.entry(r.node_id).or_insert(0.0) += 1.0 / (RRF_K + r.rank) as f64;
    }

    let mut merged: Vec<RankedResult> = scores
        .into_iter()
        .map(|(node_id, score)| RankedResult {
            node_id,
            rank: 0,
            score,
        })
        .collect();

    merged.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for (i, r) in merged.iter_mut().enumerate() {
        r.rank = i + 1;
    }

    merged
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Attempt sqlite-vec KNN search.
fn try_vec_knn(
    conn: &Connection,
    query_blob: &[u8],
    library_ids: &[i64],
    level: Option<i32>,
    top_k: usize,
) -> Result<Vec<RankedResult>> {
    let mut sql = String::from(
        "SELECT v.node_id \
         FROM vec_kb_nodes v \
         INNER JOIN kb_document_nodes n ON n.id = v.node_id \
         WHERE v.embedding MATCH ?1 AND k = ?2",
    );

    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> =
        vec![Box::new(query_blob.to_vec()), Box::new(top_k as i64)];

    if !library_ids.is_empty() {
        let start = param_values.len() + 1;
        let placeholders: Vec<String> = library_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", start + i))
            .collect();
        sql.push_str(&format!(
            " AND n.library_id IN ({})",
            placeholders.join(",")
        ));
        for &id in library_ids {
            param_values.push(Box::new(id));
        }
    }

    if let Some(lvl) = level {
        let idx = param_values.len() + 1;
        sql.push_str(&format!(" AND n.level = ?{}", idx));
        param_values.push(Box::new(lvl));
    }

    sql.push_str(" ORDER BY v.distance ASC");

    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();

    let mut results = Vec::new();

    // sqlite-vec may not be available; wrap in best-effort block
    if let Ok(mut stmt) = conn.prepare(&sql) {
        if let Ok(mut rows) = stmt.query(param_refs.as_slice()) {
            while let Ok(Some(row)) = rows.next() {
                if let Ok(node_id) = row.get::<_, i64>(0) {
                    results.push(RankedResult {
                        node_id,
                        rank: results.len() + 1,
                        score: 0.0,
                    });
                }
            }
        }
    }

    Ok(results)
}

/// Brute-force cosine similarity search over kb_node_embeddings BLOB table.
///
/// This is the fallback when sqlite-vec is not available. It loads all
/// candidate embeddings into memory and computes cosine similarity.
fn vector_search_fallback(
    conn: &Connection,
    embedding: &[f32],
    library_ids: &[i64],
    level: Option<i32>,
    top_k: usize,
) -> Result<Vec<RankedResult>> {
    let mut sql = String::from(
        "SELECT ne.node_id, ne.embedding \
         FROM kb_node_embeddings ne \
         INNER JOIN kb_document_nodes n ON n.id = ne.node_id \
         WHERE 1=1",
    );

    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if !library_ids.is_empty() {
        let start = param_values.len() + 1;
        let placeholders: Vec<String> = library_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", start + i))
            .collect();
        sql.push_str(&format!(
            " AND n.library_id IN ({})",
            placeholders.join(",")
        ));
        for &id in library_ids {
            param_values.push(Box::new(id));
        }
    }

    if let Some(lvl) = level {
        let idx = param_values.len() + 1;
        sql.push_str(&format!(" AND n.level = ?{}", idx));
        param_values.push(Box::new(lvl));
    }

    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        let node_id: i64 = row.get(0)?;
        let blob: Vec<u8> = row.get(1)?;
        Ok((node_id, blob))
    })?;

    let mut candidates: Vec<(i64, f64)> = Vec::new();
    for row in rows {
        let (node_id, blob) = row?;
        let node_embedding = blob_to_embedding(&blob);
        let sim = cosine_similarity_f64(embedding, &node_embedding);
        candidates.push((node_id, sim));
    }

    candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    candidates.truncate(top_k);

    Ok(candidates
        .iter()
        .enumerate()
        .map(|(i, (node_id, _))| RankedResult {
            node_id: *node_id,
            rank: i + 1,
            score: 0.0,
        })
        .collect())
}

/// Fetch full details for merged results, joining with document and library
/// tables to populate `document_name` and `library_name`.
fn fetch_node_details(
    conn: &Connection,
    merged: &[RankedResult],
    top_k: usize,
) -> Result<Vec<KbSearchResult>> {
    if merged.is_empty() {
        return Ok(Vec::new());
    }

    let node_ids: Vec<i64> = merged.iter().take(top_k).map(|r| r.node_id).collect();
    let score_map: HashMap<i64, f64> = merged
        .iter()
        .take(top_k)
        .map(|r| (r.node_id, r.score))
        .collect();

    let placeholders: Vec<String> = node_ids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect();
    let sql = format!(
        "SELECT n.id, n.document_id, n.content, n.level, \
                d.original_name, n.library_id, l.name \
         FROM kb_document_nodes n \
         INNER JOIN kb_documents d ON d.id = n.document_id \
         INNER JOIN kb_libraries l ON l.id = n.library_id \
         WHERE n.id IN ({})",
        placeholders.join(",")
    );

    let param_values: Vec<Box<dyn rusqlite::types::ToSql>> = node_ids
        .iter()
        .map(|&id| Box::new(id) as Box<dyn rusqlite::types::ToSql>)
        .collect();
    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(KbSearchResult {
            node_id: row.get(0)?,
            document_id: row.get(1)?,
            content: row.get(2)?,
            level: row.get(3)?,
            document_name: row.get(4)?,
            library_id: row.get(5)?,
            library_name: row.get(6)?,
            score: 0.0,
        })
    })?;

    let mut results: Vec<KbSearchResult> = rows.filter_map(|r| r.ok()).collect();

    // Assign RRF scores from the merge step
    for result in &mut results {
        if let Some(&score) = score_map.get(&result.node_id) {
            result.score = score;
        }
    }

    // Sort by descending score
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(results)
}

/// Convert f32 vector to BLOB (little-endian).
///
/// Each f32 is encoded as 4 bytes in little-endian byte order.
/// Compatible with both `kb_node_embeddings.embedding` and `vec_kb_nodes.embedding`.
fn embedding_to_blob(vector: &[f32]) -> Vec<u8> {
    vector.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Decode a BLOB back into an f32 vector.
fn blob_to_embedding(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .filter_map(|chunk| {
            let bytes: [u8; 4] = chunk.try_into().ok()?;
            Some(f32::from_le_bytes(bytes))
        })
        .collect()
}

/// Compute cosine similarity between two f32 vectors, returned as f64.
fn cosine_similarity_f64(a: &[f32], b: &[f32]) -> f64 {
    let len = a.len().min(b.len());
    if len == 0 {
        return 0.0;
    }
    let dot: f64 = a[..len]
        .iter()
        .zip(b[..len].iter())
        .map(|(x, y)| (*x as f64) * (*y as f64))
        .sum();
    let norm_a: f64 = a[..len]
        .iter()
        .map(|x| (*x as f64).powi(2))
        .sum::<f64>()
        .sqrt();
    let norm_b: f64 = b[..len]
        .iter()
        .map(|x| (*x as f64).powi(2))
        .sum::<f64>()
        .sqrt();
    if !norm_a.is_finite() || !norm_b.is_finite() || norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    let result = dot / (norm_a * norm_b);
    if result.is_finite() {
        result
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rrf_merge_single_list() {
        let vec_results = vec![
            RankedResult {
                node_id: 1,
                rank: 1,
                score: 0.0,
            },
            RankedResult {
                node_id: 2,
                rank: 2,
                score: 0.0,
            },
        ];
        let fts_results = vec![];
        let merged = rrf_merge(vec_results, fts_results);

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].node_id, 1); // rank 1 should score higher
        assert!(merged[0].score > merged[1].score);
    }

    #[test]
    fn test_rrf_merge_combined() {
        let vec_results = vec![
            RankedResult {
                node_id: 1,
                rank: 1,
                score: 0.0,
            },
            RankedResult {
                node_id: 2,
                rank: 2,
                score: 0.0,
            },
        ];
        let fts_results = vec![
            RankedResult {
                node_id: 2,
                rank: 1,
                score: 0.0,
            },
            RankedResult {
                node_id: 3,
                rank: 2,
                score: 0.0,
            },
        ];
        let merged = rrf_merge(vec_results, fts_results);

        // Node 2 appears in both lists, should rank highest
        assert_eq!(merged[0].node_id, 2);
        assert!(merged[0].score > merged[1].score);
    }

    #[test]
    fn test_embedding_blob_roundtrip() {
        let original: Vec<f32> = vec![0.1, -0.2, 0.3, 0.0, 1.0];
        let blob = embedding_to_blob(&original);
        let decoded = blob_to_embedding(&blob);

        assert_eq!(decoded.len(), original.len());
        for (a, b) in original.iter().zip(decoded.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let v = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity_f64(&v, &v);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity_f64(&a, &b);
        assert!((sim - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 0.0];
        let sim = cosine_similarity_f64(&a, &b);
        assert!((sim - 0.0).abs() < 1e-6);
    }
}
