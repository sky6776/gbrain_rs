//! 搜索评测模块 (P4-017~P4-022)
//!
//! 离线评测命令、Recall@K/MRR@K/NDCG@K 指标计算、搜索日志、反馈 API。

use crate::error::{GBrainError, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// 评测查询条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalQuery {
    pub id: i64,
    pub query_text: String,
    pub query_type: String,
    pub expected_document_ids: Vec<i64>,
}

/// 评测结果汇总
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResult {
    pub total_queries: usize,
    pub recall_at_20: f64,
    pub mrr_at_10: f64,
    pub ndcg_at_10: f64,
    pub no_result_rate: f64,
    pub p95_latency_ms: u64,
}

/// 计算 Recall@K
pub fn recall_at_k(expected: &[i64], retrieved: &[i64], k: usize) -> f64 {
    if expected.is_empty() {
        return 1.0;
    }
    let retrieved_set: std::collections::HashSet<_> = retrieved.iter().take(k).copied().collect();
    let hits = expected.iter().filter(|id| retrieved_set.contains(id)).count();
    hits as f64 / expected.len() as f64
}

/// 计算 MRR@K (Mean Reciprocal Rank)
pub fn mrr_at_k(expected: &[i64], retrieved: &[i64], k: usize) -> f64 {
    for (rank, id) in retrieved.iter().take(k).enumerate() {
        if expected.contains(id) {
            return 1.0 / ((rank + 1) as f64);
        }
    }
    0.0
}

/// 计算 NDCG@K (Normalized Discounted Cumulative Gain)
/// 简化为二值相关性（命中=1，未命中=0）
pub fn ndcg_at_k(expected: &[i64], retrieved: &[i64], k: usize) -> f64 {
    let expected_set: std::collections::HashSet<_> = expected.iter().collect();
    let ideal_count = expected.len().min(k) as f64;
    if ideal_count == 0.0 {
        return 1.0;
    }

    let mut dcg = 0.0f64;
    for (rank, id) in retrieved.iter().take(k).enumerate() {
        if expected_set.contains(id) {
            let gain = 1.0f64;
            dcg += gain / ((rank + 2) as f64).log2();
        }
    }

    let mut idcg = 0.0f64;
    for i in 0..ideal_count as usize {
        idcg += 1.0f64 / ((i + 2) as f64).log2();
    }

    if idcg == 0.0 {
        1.0
    } else {
        dcg / idcg
    }
}

/// 计算汇总评测指标
pub fn compute_eval_summary(
    queries: &[(Vec<i64>, Vec<i64>, u64)],
) -> EvalResult {
    let total = queries.len();
    if total == 0 {
        return EvalResult {
            total_queries: 0,
            recall_at_20: 0.0,
            mrr_at_10: 0.0,
            ndcg_at_10: 0.0,
            no_result_rate: 0.0,
            p95_latency_ms: 0,
        };
    }

    let mut recall_sum = 0.0;
    let mut mrr_sum = 0.0;
    let mut ndcg_sum = 0.0;
    let mut no_result = 0;
    let mut latencies: Vec<u64> = Vec::new();

    for (expected, retrieved, latency) in queries {
        recall_sum += recall_at_k(expected, retrieved, 20);
        mrr_sum += mrr_at_k(expected, retrieved, 10);
        ndcg_sum += ndcg_at_k(expected, retrieved, 10);
        if retrieved.is_empty() {
            no_result += 1;
        }
        latencies.push(*latency);
    }

    latencies.sort_unstable();
    let p95_idx = ((latencies.len() as f64) * 0.95).ceil() as usize - 1;

    EvalResult {
        total_queries: total,
        recall_at_20: recall_sum / total as f64,
        mrr_at_10: mrr_sum / total as f64,
        ndcg_at_10: ndcg_sum / total as f64,
        no_result_rate: no_result as f64 / total as f64,
        p95_latency_ms: latencies.get(p95_idx).copied().unwrap_or(0),
    }
}

/// 记录搜索日志
pub fn log_search(
    conn: &Connection,
    query_normalized: &str,
    library_ids: &[i64],
    profile: &str,
    planner_type: &str,
    result_count: usize,
    latency_ms: u64,
    cache_hit: bool,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO kb_search_logs (query_normalized, library_ids, profile, planner_type, \
         result_count, latency_ms, cache_hit) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            query_normalized,
            serde_json::to_string(library_ids).unwrap_or_default(),
            profile,
            planner_type,
            result_count as i32,
            latency_ms as i32,
            cache_hit as i32,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// 添加搜索反馈
pub fn add_search_feedback(
    conn: &Connection,
    search_log_id: Option<i64>,
    document_id: Option<i64>,
    node_id: Option<i64>,
    rating: i32,
    comment: &str,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO kb_search_feedback (search_log_id, document_id, node_id, rating, comment) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![search_log_id, document_id, node_id, rating.clamp(0, 5), comment],
    )?;
    Ok(conn.last_insert_rowid())
}

/// 添加评测查询
pub fn add_eval_query(
    conn: &Connection,
    library_id: i64,
    query_text: &str,
    query_type: &str,
    expected_document_ids: &[i64],
) -> Result<i64> {
    let ids_json = serde_json::to_string(expected_document_ids).unwrap_or_default();
    conn.execute(
        "INSERT INTO kb_search_eval_queries (library_id, query_text, query_type, expected_document_ids) \
         VALUES (?1, ?2, ?3, ?4)",
        params![library_id, query_text, query_type, ids_json],
    )?;
    Ok(conn.last_insert_rowid())
}

/// 获取某个 library 的所有评测查询
pub fn list_eval_queries(conn: &Connection, library_id: i64) -> Result<Vec<EvalQuery>> {
    let mut stmt = conn.prepare(
        "SELECT id, query_text, query_type, expected_document_ids \
         FROM kb_search_eval_queries WHERE library_id = ?1"
    )?;
    let rows = stmt.query_map(params![library_id], |row| {
        let ids_str: String = row.get(3)?;
        Ok(EvalQuery {
            id: row.get(0)?,
            query_text: row.get(1)?,
            query_type: row.get(2)?,
            expected_document_ids: serde_json::from_str(&ids_str).unwrap_or_default(),
        })
    })?;
    let results: Vec<EvalQuery> = rows.filter_map(|r| r.ok()).collect();
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recall_perfect() {
        assert!((recall_at_k(&[1, 2, 3], &[1, 2, 3, 4, 5], 20) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_recall_partial() {
        assert!((recall_at_k(&[1, 2, 3], &[1, 4, 5, 6, 7], 20) - (1.0 / 3.0)).abs() < 0.001);
    }

    #[test]
    fn test_mrr_first_hit() {
        let mrr = mrr_at_k(&[2], &[1, 2, 3], 10);
        assert!((mrr - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_mrr_no_hit() {
        assert!((mrr_at_k(&[10], &[1, 2, 3], 10)).abs() < 0.001);
    }

    #[test]
    fn test_ndcg_perfect() {
        assert!((ndcg_at_k(&[1, 2, 3], &[1, 2, 3, 4], 10) - 1.0).abs() < 0.001);
    }
}
