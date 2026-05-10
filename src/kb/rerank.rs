//! Rerank 模块 (P3-016~P3-021, P4-003~P4-005)
//!
//! 模型 rerank 优先策略 + 本地 fallback。

use serde::{Deserialize, Serialize};

/// Rerank 配置
#[derive(Debug, Clone)]
pub struct RerankConfig {
    pub model_rerank_enabled: bool,
    pub rerank_provider: String,
    pub rerank_model: String,
    pub rerank_timeout_ms: u64,
    pub rerank_max_candidates: usize,
    pub external_rerank_allowed: bool,
}

impl Default for RerankConfig {
    fn default() -> Self {
        Self {
            model_rerank_enabled: true,
            rerank_provider: String::new(),
            rerank_model: "gpt-4o-mini".into(),
            rerank_timeout_ms: 5000,
            rerank_max_candidates: 50,
            external_rerank_allowed: true,
        }
    }
}

/// Fallback 原因
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FallbackReason {
    NotConfigured,
    Timeout,
    ApiError,
    BudgetExceeded,
    PrivacyBlocked,
}

impl FallbackReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotConfigured => "not_configured",
            Self::Timeout => "timeout",
            Self::ApiError => "api_error",
            Self::BudgetExceeded => "budget_exceeded",
            Self::PrivacyBlocked => "privacy_blocked",
        }
    }
}

/// Rerank 结果
#[derive(Debug, Clone)]
pub struct RerankResult {
    pub model_rerank_attempted: bool,
    pub model_rerank_succeeded: bool,
    pub fallback_used: bool,
    pub fallback_reason: Option<FallbackReason>,
    pub provider: String,
    pub candidates_reranked: usize,
}

/// 本地 rerank 信号
#[derive(Debug, Clone)]
pub struct LocalRankSignals {
    pub fts_score: f64,
    pub vector_score: f64,
    pub title_score: f64,
    pub summary_score: f64,
    pub table_score: f64,
    pub metadata_score: f64,
    pub freshness_score: f64,
    pub granularity_score: f64,
    pub exact_match_score: f64,
}

impl Default for LocalRankSignals {
    fn default() -> Self {
        Self {
            fts_score: 0.0,
            vector_score: 0.0,
            title_score: 0.0,
            summary_score: 0.0,
            table_score: 0.0,
            metadata_score: 0.0,
            freshness_score: 0.0,
            granularity_score: 0.0,
            exact_match_score: 0.0,
        }
    }
}

/// 使用本地信号加权排序
pub fn local_rerank(
    candidates: &[(i64, LocalRankSignals)],
    weights: &[f64],
) -> Vec<(i64, f64)> {
    let mut scored: Vec<(i64, f64)> = candidates
        .iter()
        .map(|(doc_id, signals)| {
            let score = signals.fts_score * weights.get(0).copied().unwrap_or(0.3)
                + signals.vector_score * weights.get(1).copied().unwrap_or(0.3)
                + signals.title_score * weights.get(2).copied().unwrap_or(0.2)
                + signals.exact_match_score * weights.get(3).copied().unwrap_or(0.0)
                + signals.metadata_score * weights.get(4).copied().unwrap_or(0.1)
                + signals.freshness_score * weights.get(5).copied().unwrap_or(0.1);
            (*doc_id, score)
        })
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored
}

/// Reciprocal Rank Fusion (k=60)
pub fn rrf_merge(results_lists: &[Vec<(i64, f64)>], k: f64) -> Vec<(i64, f64)> {
    let mut scores: std::collections::HashMap<i64, f64> = std::collections::HashMap::new();
    for results in results_lists {
        for (rank, (doc_id, _)) in results.iter().enumerate() {
            *scores.entry(*doc_id).or_insert(0.0) += 1.0 / (k + (rank + 1) as f64);
        }
    }
    let mut merged: Vec<(i64, f64)> = scores.into_iter().collect();
    merged.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rrf_merge() {
        let list1 = vec![(1, 0.9), (2, 0.8)];
        let list2 = vec![(2, 0.7), (3, 0.6)];
        let merged = rrf_merge(&[list1, list2], 60.0);
        assert!(!merged.is_empty());
        // doc 2 appears in both lists, should rank high
        assert_eq!(merged[0].0, 2);
    }

    #[test]
    fn test_local_rerank() {
        let candidates = vec![
            (1, LocalRankSignals { fts_score: 0.9, ..Default::default() }),
            (2, LocalRankSignals { title_score: 0.8, vector_score: 0.3, ..Default::default() }),
        ];
        let weights = vec![0.4, 0.3, 0.2, 0.1, 0.0, 0.0];
        let ranked = local_rerank(&candidates, &weights);
        assert_eq!(ranked.len(), 2);
    }

    #[test]
    fn test_fallback_reason_as_str() {
        assert_eq!(FallbackReason::Timeout.as_str(), "timeout");
        assert_eq!(FallbackReason::PrivacyBlocked.as_str(), "privacy_blocked");
    }
}
