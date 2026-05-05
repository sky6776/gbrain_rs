//! Autopilot / self-maintaining brain
//! Mirrors gbrain's src/commands/autopilot.ts
//!
//! Periodic background task that:
//! - Embeds new/stale content
//! - Runs integrity checks (dead links, orphans)
//! - Reports brain health

use crate::config::Config;
use crate::embedding::Embedder;
use crate::engine::BrainEngine;
use crate::sqlite_engine::SqliteEngine;
use tracing::{debug, info, warn};

/// Autopilot orchestrates periodic brain maintenance tasks
pub struct Autopilot<'a> {
    engine: &'a SqliteEngine,
    _config: Config,
    embedder: Option<Embedder>,
}

impl<'a> Autopilot<'a> {
    pub fn new(engine: &'a SqliteEngine, config: Config) -> Self {
        let embedder = config.openai_api_key.as_deref().map(|api_key| {
            Embedder::new(
                api_key,
                config.openai_base_url.as_deref(),
                Some(&config.embedding_model),
                Some(config.embedding_dimensions),
            )
        });

        Self {
            engine,
            _config: config,
            embedder,
        }
    }

    /// Run one maintenance cycle
    pub fn run_once(&self) -> Result<(), crate::error::GBrainError> {
        info!("Autopilot: starting maintenance cycle");

        // 1. Embed new/stale chunks
        if let Some(ref embedder) = self.embedder {
            match self.embed_unembedded_chunks(embedder) {
                Ok(count) => info!(count, "Autopilot: embedded new chunks"),
                Err(e) => warn!(error = %e, "Autopilot: embedding step failed"),
            }
        } else {
            debug!("Autopilot: no embedder configured, skipping embedding step");
        }

        // 2. Integrity check
        match self.run_integrity_check() {
            Ok((orphans, dead_links)) => {
                if orphans > 0 || dead_links > 0 {
                    info!(
                        orphans,
                        dead_links, "Autopilot: integrity check found issues"
                    );
                } else {
                    info!("Autopilot: integrity check passed");
                }
            }
            Err(e) => warn!(error = %e, "Autopilot: integrity check failed"),
        }

        // 3. Health report
        match self.engine.get_health() {
            Ok(health) => {
                info!(
                    brain_score = %health.brain_score,
                    embed_coverage = %health.embed_coverage,
                    stale_pages = health.stale_pages,
                    orphan_pages = health.orphan_pages,
                    dead_links = health.dead_links,
                    "Autopilot: health report"
                );
            }
            Err(e) => warn!(error = %e, "Autopilot: health check failed"),
        }

        info!("Autopilot: maintenance cycle complete");
        Ok(())
    }

    /// Run maintenance in a loop with a given interval
    pub fn run_loop(&self, interval_secs: u64) -> ! {
        info!(interval_secs, "Autopilot: starting daemon mode");
        loop {
            if let Err(e) = self.run_once() {
                warn!(error = %e, "Autopilot: cycle failed, will retry");
            }
            std::thread::sleep(std::time::Duration::from_secs(interval_secs));
        }
    }

    /// Embed chunks that don't yet have embeddings
    fn embed_unembedded_chunks(
        &self,
        embedder: &Embedder,
    ) -> Result<usize, crate::error::GBrainError> {
        let stale = self.engine.list_stale_chunks(Some(1000))?;
        if stale.is_empty() {
            debug!("Autopilot: no stale chunks found, nothing to embed");
            return Ok(0);
        }
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| {
                crate::error::GBrainError::InvalidInput(format!(
                    "failed to start async runtime: {}",
                    e
                ))
            })?;
        let mut embedded = 0;
        for batch in stale.chunks(50) {
            let texts: Vec<&str> = batch.iter().map(|c| c.chunk_text.as_str()).collect();
            let embeddings = rt.block_on(embedder.embed_batch(&texts))?;
            let mut by_slug: std::collections::HashMap<String, Vec<crate::types::ChunkInput>> =
                std::collections::HashMap::new();
            for (row, embedding) in batch.iter().zip(embeddings.into_iter()) {
                by_slug
                    .entry(row.slug.clone())
                    .or_default()
                    .push(crate::types::ChunkInput {
                        chunk_index: row.chunk_index,
                        chunk_text: row.chunk_text.clone(),
                        source: row.source.clone(),
                        token_count: row.token_count,
                        embedding: Some(embedding),
                        model: Some(
                            row.model
                                .clone()
                                .unwrap_or_else(|| "text-embedding-3-large".to_string()),
                        ),
                        language: None,
                        symbol_name: None,
                        symbol_type: None,
                        start_line: None,
                        end_line: None,
                        parent_symbol_path: None,
                        symbol_name_qualified: None,
                        doc_comment: None,
                    });
            }
            for (slug, chunks) in by_slug {
                embedded += self.engine.upsert_chunks(&slug, &chunks)?;
            }
        }
        Ok(embedded)
    }

    /// Run integrity checks: detect orphans and dead links
    fn run_integrity_check(&self) -> Result<(i64, i64), crate::error::GBrainError> {
        let health = self.engine.get_health()?;
        Ok((health.orphan_pages, health.dead_links))
    }
}
