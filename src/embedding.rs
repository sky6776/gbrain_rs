//! OpenAI embedding API client
//! Mirrors gbrain's src/core/embedding.ts
//!
//! Supports batch embedding (up to 100 texts per call),
//! retry with exponential backoff, Retry-After header parsing,
//! input truncation, and batch completion callbacks.

use crate::backoff::{BackoffOpts, backoff_delay_ms};
use crate::error::{GBrainError, Result};
use serde::Deserialize;
use tracing::{debug, info, warn};

/// Default embedding model
pub const DEFAULT_MODEL: &str = "text-embedding-3-large";

/// Default embedding dimensions
pub const DEFAULT_DIMENSIONS: usize = 1536;

/// Maximum batch size for embedding API
pub const MAX_BATCH_SIZE: usize = 100;

/// Maximum input text length for embedding (characters)
const MAX_EMBEDDING_INPUT_CHARS: usize = 8000;

/// Embedding API response
#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
    usage: EmbeddingUsage,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
    index: usize,
}

#[derive(Debug, Deserialize)]
struct EmbeddingUsage {
    total_tokens: i64,
}

/// Embedding client
pub struct Embedder {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    model: String,
    dimensions: usize,
    on_batch_complete: Option<Box<dyn Fn(usize, i64) + Send + Sync>>,
}

impl Embedder {
    /// Create a new embedding client
    pub fn new(
        api_key: &str,
        base_url: Option<&str>,
        model: Option<&str>,
        dimensions: Option<usize>,
    ) -> Self {
        debug!(
            base_url = base_url.unwrap_or("https://api.openai.com/v1"),
            model = model.unwrap_or(DEFAULT_MODEL),
            dimensions = dimensions.unwrap_or(DEFAULT_DIMENSIONS),
            "Creating embedding client"
        );
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.to_string(),
            base_url: base_url.unwrap_or("https://api.openai.com/v1").to_string(),
            model: model.unwrap_or(DEFAULT_MODEL).to_string(),
            dimensions: dimensions.unwrap_or(DEFAULT_DIMENSIONS),
            on_batch_complete: None,
        }
    }

    /// Set a callback that fires after each successful batch
    pub fn set_batch_callback(&mut self, cb: impl Fn(usize, i64) + Send + Sync + 'static) {
        self.on_batch_complete = Some(Box::new(cb));
    }

    /// Check if the client is configured (has API key)
    pub fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }

    /// Embed a single text string
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let results = self.embed_batch(&[text]).await?;
        results
            .into_iter()
            .next()
            .ok_or_else(|| GBrainError::Embedding("No embedding returned".to_string()))
    }

    /// Embed a batch of text strings (up to MAX_BATCH_SIZE)
    pub async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        if texts.len() > MAX_BATCH_SIZE {
            return Err(GBrainError::Embedding(format!(
                "Batch size {} exceeds maximum {}",
                texts.len(),
                MAX_BATCH_SIZE
            )));
        }

        // Truncate input texts to MAX_EMBEDDING_INPUT_CHARS at word boundaries
        let truncated_texts: Vec<String> = texts
            .iter()
            .map(|t| {
                if t.len() > MAX_EMBEDDING_INPUT_CHARS {
                    let truncated = truncate_at_word_boundary(t, MAX_EMBEDDING_INPUT_CHARS);
                    warn!(
                        original_len = t.len(),
                        truncated_len = truncated.len(),
                        "Truncating embedding input at word boundary"
                    );
                    truncated.to_string()
                } else {
                    t.to_string()
                }
            })
            .collect();

        info!(
            model = %self.model,
            text_count = texts.len(),
            dimensions = self.dimensions,
            "Starting embedding batch request"
        );

        let url = format!("{}/embeddings", self.base_url);

        let body = serde_json::json!({
            "model": self.model,
            "input": truncated_texts,
            "dimensions": self.dimensions,
        });

        // Retry with exponential backoff (using shared BackoffOpts + backoff_delay_ms)
        let backoff_opts = BackoffOpts {
            max_retries: 5,
            base_ms: 4000,
            max_ms: 120000,
            jitter: true,
        };

        let mut attempt = 0u32;

        loop {
            attempt += 1;
            debug!(
                attempt,
                max_retries = backoff_opts.max_retries,
                "Sending embedding request"
            );

            let response = self
                .client
                .post(&url)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    if resp.status().is_success() {
                        let embedding_resp: EmbeddingResponse = resp.json().await.map_err(|e| {
                            GBrainError::Embedding(format!("Failed to parse response: {}", e))
                        })?;

                        let token_count = embedding_resp.usage.total_tokens;
                        let result_count = embedding_resp.data.len();

                        // Validate embedding dimensions match expected configuration
                        for d in &embedding_resp.data {
                            if d.embedding.len() != self.dimensions {
                                warn!(
                                    expected = self.dimensions,
                                    actual = d.embedding.len(),
                                    index = d.index,
                                    "Embedding dimension mismatch — skipping batch"
                                );
                                return Err(GBrainError::Embedding(format!(
                                    "Embedding dimension mismatch: expected {}, got {} at index {}",
                                    self.dimensions, d.embedding.len(), d.index
                                )));
                            }
                            // Validate all values are finite (no NaN or Infinity)
                            if d.embedding.iter().any(|v| !v.is_finite()) {
                                warn!(
                                    index = d.index,
                                    "Embedding contains non-finite values (NaN/Inf) — rejecting batch"
                                );
                                return Err(GBrainError::Embedding(format!(
                                    "Embedding contains non-finite values at index {}",
                                    d.index
                                )));
                            }
                        }

                        // Validate response count matches input count
                        if embedding_resp.data.len() != texts.len() {
                            warn!(
                                expected = texts.len(),
                                actual = embedding_resp.data.len(),
                                "Embedding response count mismatch"
                            );
                            return Err(GBrainError::Embedding(format!(
                                "Response count mismatch: expected {} embeddings, got {}",
                                texts.len(), embedding_resp.data.len()
                            )));
                        }

                        // Sort by index to maintain order
                        let mut data = embedding_resp.data;
                        data.sort_by_key(|d| d.index);

                        info!(token_count, result_count, "Embedding batch complete");

                        // Fire batch callback
                        if let Some(ref cb) = self.on_batch_complete {
                            cb(result_count, token_count);
                        }

                        return Ok(data.into_iter().map(|d| d.embedding).collect());
                    }

                    let status = resp.status();
                    let headers = resp.headers().clone();
                    let error_text = resp.text().await.unwrap_or_default();

                    // Retry on rate limit (429) or server error (5xx)
                    if (status.as_u16() == 429 || status.as_u16() >= 500) && attempt <= backoff_opts.max_retries
                    {
                        // Parse Retry-After header
                        let retry_after_ms = headers
                            .get("retry-after")
                            .and_then(|v| v.to_str().ok())
                            .and_then(|v| v.parse::<u64>().ok())
                            .map(|s| s * 1000)
                            .unwrap_or(0);

                        let calculated_ms = backoff_delay_ms(attempt, &backoff_opts);
                        let wait_ms = retry_after_ms.max(calculated_ms);

                        warn!(
                            status = status.as_u16(),
                            attempt,
                            wait_ms,
                            retry_after_ms,
                            "Rate limited or server error, retrying with backoff"
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(wait_ms)).await;
                        continue;
                    }

                    warn!(status = status.as_u16(), error = %error_text, "Embedding API error");
                    return Err(GBrainError::Embedding(format!(
                        "API error {}: {}",
                        status, error_text
                    )));
                }
                Err(e) => {
                    warn!(attempt, error = %e, "Embedding request failed");
                    if attempt <= backoff_opts.max_retries {
                        let delay_ms = backoff_delay_ms(attempt, &backoff_opts);
                        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                        continue;
                    }
                    return Err(GBrainError::Embedding(format!(
                        "Request failed after {} attempts: {}",
                        backoff_opts.max_retries + 1, e
                    )));
                }
            }
        }
    }

    /// Get the model name
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Get the dimensions
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }
}

/// Truncate text to at most `max_chars` characters, breaking at a word boundary
/// when possible. Looks for the last space before `max_chars` and truncates there.
/// Falls back to hard truncation at `max_chars` if no space is found.
pub fn truncate_at_word_boundary(text: &str, max_chars: usize) -> &str {
    if text.len() <= max_chars {
        return text;
    }
    let limit = text.floor_char_boundary(max_chars);
    if let Some(space_pos) = text[..limit].rfind(' ') {
        &text[..space_pos]
    } else {
        &text[..limit]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_at_word_boundary_short_text() {
        // Text shorter than limit is returned as-is
        assert_eq!(truncate_at_word_boundary("hello world", 100), "hello world");
    }

    #[test]
    fn test_truncate_at_word_boundary_exact_limit() {
        // Text exactly at limit is returned as-is
        let text = "a".repeat(8000);
        assert_eq!(truncate_at_word_boundary(&text, 8000).len(), 8000);
    }

    #[test]
    fn test_truncate_at_word_boundary_breaks_at_space() {
        // Should break at the last space before the limit, not mid-word
        let word = "abcdefghij";
        let text = format!("{} {} {}", word, word, word); // "abcdefghij abcdefghij abcdefghij"
        // Limit in the middle of the third word — should truncate after the second word
        let truncated = truncate_at_word_boundary(&text, word.len() * 2 + 2 + 5);
        // The truncation should end at a space boundary (may include trailing space)
        // The key invariant: no partial word at the end
        assert!(
            truncated == "abcdefghij abcdefghij " || truncated == "abcdefghij abcdefghij",
            "Expected truncation to end at a space boundary, got: {:?}",
            truncated
        );
        assert!(!truncated.is_empty());
    }

    #[test]
    fn test_truncate_at_word_boundary_no_space_fallback() {
        // Long text with no spaces falls back to hard truncation
        let text = "abcdefghij".repeat(1000); // 10000 chars, no spaces
        let truncated = truncate_at_word_boundary(&text, 8000);
        assert_eq!(truncated.len(), 8000);
    }

    #[test]
    fn test_truncate_at_word_boundary_utf8_safe() {
        // Multi-byte UTF-8 characters should not be split
        let text = "Hello ".to_string() + &"\u{1F600}".repeat(2000); // lots of emoji
        let truncated = truncate_at_word_boundary(&text, 8000);
        // Must be a valid UTF-8 string (no panic = success)
        assert!(truncated.len() <= 8000 || text.len() <= 8000);
    }
}
