//! Semantic splitter using embedding similarity

use crate::embedding::Embedder;
use crate::error::GBrainError;

pub type Chunks = Vec<String>;

pub struct SemanticSplitter<'a> {
    embedder: &'a Embedder,
    percentile_threshold: f64,
    min_chunk_size: usize,
}

impl<'a> SemanticSplitter<'a> {
    pub fn new(embedder: &'a Embedder) -> Self {
        Self {
            embedder,
            percentile_threshold: 0.6,
            min_chunk_size: 300,
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

            if should_split {
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

fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    let dot: f64 = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| (*x as f64) * (*y as f64))
        .sum();
    let norm_a: f64 = a.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}
