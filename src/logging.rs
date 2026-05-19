//! Logging initialization
//!
//! Configurable via env vars or config.json:
//! - `GBRAIN_LOG_LEVEL` — trace|debug|info|warn|error (default: info)
//! - `GBRAIN_LOG_TO_FILE` — true|false (default: true)
//! - `GBRAIN_LOG_FILE_PATH` — custom log file path (default: $GBRAIN_DIR/logs/gbrain.log)
//! - `GBRAIN_LOG_TO_CONSOLE` — true|false (default: true)

use crate::config::Config;
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize the logging system based on config.
///
/// Call this once at program startup (CLI or MCP server).
/// If called multiple times, subsequent calls are no-ops (tracing guards this).
pub fn init(config: &Config) {
    // Build env filter: RUST_LOG env var takes priority, then config.log_level
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log_level));

    if config.log_to_file {
        let log_path = config
            .log_file_path
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| Config::base_dir().join("logs").join("gbrain.log"));

        // Ensure parent directory exists
        if let Some(parent) = log_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let file_name = log_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "gbrain.log".to_string());
        let log_dir = log_path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| Config::base_dir().join("logs"));

        let file_appender = tracing_appender::rolling::daily(&log_dir, &file_name);
        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(file_appender)
            .with_ansi(false);

        if config.log_to_console {
            // 显式输出到 stderr，避免与 MCP stdio 的 stdout JSON-RPC 协议交错
            let console_layer = tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_target(false);
            tracing_subscriber::registry()
                .with(filter)
                .with(file_layer)
                .with(console_layer)
                .init();
        } else {
            tracing_subscriber::registry()
                .with(filter)
                .with(file_layer)
                .init();
        }
    } else if config.log_to_console {
        tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .with_env_filter(filter)
            .with_target(false)
            .init();
    }
    // If both file and console are disabled, no logging output (silent mode)
}
