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
                        dead_links,
                        "Autopilot: integrity check found issues"
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
    fn embed_unembedded_chunks(&self, _embedder: &Embedder) -> Result<usize, crate::error::GBrainError> {
        // Check how many chunks exist in total
        let stats = self.engine.get_stats()?;
        if stats.chunk_count == 0 {
            debug!("Autopilot: no chunks found, nothing to embed");
            return Ok(0);
        }

        // This is a stub path — the actual embedding logic requires
        // sqlite-vec integration which is not yet fully implemented.
        // For now, we report the count of chunks that would need embedding.
        debug!(
            chunk_count = stats.chunk_count,
            "Autopilot: embedding stub — sqlite-vec integration pending"
        );
        Ok(0)
    }

    /// Run integrity checks: detect orphans and dead links
    fn run_integrity_check(&self) -> Result<(i64, i64), crate::error::GBrainError> {
        let health = self.engine.get_health()?;
        Ok((health.orphan_pages, health.dead_links))
    }
}
