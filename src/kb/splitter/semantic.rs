//! 基于嵌入相似度的语义分割器

use super::{AsyncDocumentSplitter, Chunks};
use crate::embedding::Embedder;
use crate::error::GBrainError;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// 基于嵌入相似度的语义分割器
///
/// 通过计算相邻段落的嵌入向量余弦相似度，
/// 在相似度低于百分位阈值且当前块达到最小长度时进行分割。
pub struct SemanticSplitter {
    embedder: Arc<Embedder>,
    percentile_threshold: f64,
    min_chunk_size: usize,
    chunk_size: usize,
    #[allow(dead_code)]
    chunk_overlap: usize,
}

impl SemanticSplitter {
    pub fn new(embedder: Arc<Embedder>) -> Self {
        Self {
            embedder,
            percentile_threshold: 0.6,
            min_chunk_size: 300,
            chunk_size: 512,
            chunk_overlap: 50,
        }
    }

    pub fn with_config(embedder: Arc<Embedder>, chunk_size: usize, chunk_overlap: usize) -> Self {
        Self {
            embedder,
            percentile_threshold: 0.6,
            min_chunk_size: chunk_size / 2,
            chunk_size,
            chunk_overlap,
        }
    }

    pub async fn split(&self, text: &str) -> Result<Chunks, GBrainError> {
        let paragraphs: Vec<&str> = text
            .split("\n\n")
            .filter(|p| !p.trim().is_empty())
            .collect();

        if paragraphs.len() <= 1 {
            return Ok(vec![text.to_string()]);
        }

        let embeddings = self.embedder.embed_batch(&paragraphs).await?;

        if embeddings.len() < 2 {
            return Ok(vec![text.to_string()]);
        }

        let mut similarities = Vec::new();
        for i in 0..embeddings.len() - 1 {
            let sim = cosine_similarity(&embeddings[i], &embeddings[i + 1]);
            similarities.push(sim);
        }

        let mut sorted_sims = similarities.clone();
        sorted_sims.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let threshold_idx = ((sorted_sims.len() as f64 * self.percentile_threshold) as usize)
            .min(sorted_sims.len().saturating_sub(1));
        let threshold = sorted_sims.get(threshold_idx).copied().unwrap_or(0.5);

        let mut chunks = Vec::new();
        let mut current = String::new();

        for (i, para) in paragraphs.iter().enumerate() {
            if !current.is_empty() {
                current.push_str("\n\n");
            }
            current.push_str(para);

            let should_split = i < similarities.len()
                && similarities[i] < threshold
                && current.chars().count() >= self.min_chunk_size;

            // Also split if chunk exceeds chunk_size
            let exceeds_size = current.chars().count() > self.chunk_size;

            if should_split || exceeds_size {
                chunks.push(current.trim().to_string());
                current = String::new();
            }
        }

        if !current.trim().is_empty() {
            chunks.push(current.trim().to_string());
        }

        Ok(chunks)
    }
}

impl AsyncDocumentSplitter for SemanticSplitter {
    fn split_async<'a>(
        &'a self,
        text: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Chunks, GBrainError>> + Send + 'a>> {
        Box::pin(self.split(text))
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
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
