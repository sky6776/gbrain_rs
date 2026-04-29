//! Configuration loading
//! Mirrors gbrain's src/core/config.ts
//!
//! LLM configuration follows the per-provider pattern from the TS version:
//! each LLM usage area (embedding, expansion, chunker, transcription) has
//! its own API key / base URL / model env vars that fall back to the
//! shared GBRAIN_OPENAI_* defaults when not set.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{debug, info};

/// Brain configuration
/// R3-03 fix: API keys are marked with `#[serde(skip_serializing)]` to prevent
/// accidental leakage when `Config::save()` writes config.json. Keys are still
/// deserialized from config files (for backward compatibility) but are never
/// written back out — they should only come from environment variables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    // --- Database ---
    pub database_path: Option<String>,
    pub wal_mode: bool,
    pub pool_size: usize,

    // --- Embedding (vector search) ---
    #[serde(skip_serializing)]
    pub openai_api_key: Option<String>,
    pub openai_base_url: Option<String>,
    pub embedding_model: String,
    pub embedding_dimensions: usize,

    // --- Chunking ---
    pub chunk_size: usize,
    pub chunk_overlap: usize,

    // --- Query Expansion (LLM) ---
    #[serde(skip_serializing)]
    pub expansion_api_key: Option<String>,
    pub expansion_base_url: Option<String>,
    pub expansion_model: String,

    // --- LLM Chunking (semantic) ---
    #[serde(skip_serializing)]
    pub chunker_api_key: Option<String>,
    pub chunker_base_url: Option<String>,
    pub chunker_model: String,

    // --- Transcription (speech-to-text) ---
    pub transcription_provider: String, // "groq" | "openai"
    #[serde(skip_serializing)]
    pub transcription_groq_api_key: Option<String>,
    pub transcription_groq_base_url: Option<String>,
    #[serde(skip_serializing)]
    pub transcription_openai_api_key: Option<String>,
    pub transcription_openai_base_url: Option<String>,

    // --- Logging ---
    pub log_level: String,             // "trace"|"debug"|"info"|"warn"|"error"
    pub log_to_file: bool,             // default true
    pub log_file_path: Option<String>, // None → $GBRAIN_DIR/logs/gbrain.log
    pub log_to_console: bool,          // default true

    // --- P2-6: Auto-link / Auto-timeline (mirrors TS) ---
    pub auto_link: bool,      // default true, GBRAIN_AUTO_LINK
    pub auto_timeline: bool,  // default true, GBRAIN_AUTO_TIMELINE

    // --- P2-3: Post-write lint (mirrors TS runPostWriteLint) ---
    pub post_write_lint: bool, // default false, GBRAIN_POST_WRITE_LINT
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // Database
            database_path: None,
            wal_mode: true,
            pool_size: 10,

            // Embedding
            openai_api_key: std::env::var("GBRAIN_OPENAI_API_KEY").ok(),
            openai_base_url: std::env::var("GBRAIN_OPENAI_BASE_URL").ok(),
            embedding_model: "text-embedding-3-large".to_string(),
            embedding_dimensions: 1536,

            // Chunking
            chunk_size: 500,
            chunk_overlap: 50,

            // Query Expansion — defaults to shared OpenAI config
            expansion_api_key: None,
            expansion_base_url: None,
            expansion_model: "gpt-4o-mini".to_string(),

            // LLM Chunker — defaults to shared OpenAI config
            chunker_api_key: None,
            chunker_base_url: None,
            chunker_model: "gpt-4o-mini".to_string(),

            // Transcription
            transcription_provider: "groq".to_string(),
            transcription_groq_api_key: None,
            transcription_groq_base_url: None,
            transcription_openai_api_key: None,
            transcription_openai_base_url: None,

            // Logging
            log_level: "info".to_string(),
            log_to_file: true,
            log_file_path: None,
            log_to_console: true,

            // P2-6: Auto-link / Auto-timeline
            auto_link: true,
            auto_timeline: true,

            // P2-3: Post-write lint
            post_write_lint: false,
        }
    }
}

impl Config {
    /// Load configuration from environment and config file
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let mut config = Self::default();

        // Try loading from config file
        let config_path = Self::base_dir().join("config.json");
        if config_path.exists() {
            debug!(path = %config_path.display(), "Loading config.json");
            let content = std::fs::read_to_string(&config_path)?;
            let file_config: Config = serde_json::from_str(&content)?;
            config.merge(file_config);
            info!(path = %config_path.display(), "Merged config.json");
        }

        // --- Database env vars ---
        if let Ok(path) = std::env::var("GBRAIN_DB_PATH") {
            config.database_path = Some(path);
        }

        // --- Embedding env vars ---
        if let Ok(key) = std::env::var("GBRAIN_OPENAI_API_KEY") {
            config.openai_api_key = Some(key);
        }
        if let Ok(url) = std::env::var("GBRAIN_OPENAI_BASE_URL") {
            config.openai_base_url = Some(url);
        }
        if let Ok(model) = std::env::var("GBRAIN_EMBEDDING_MODEL") {
            config.embedding_model = model;
        }
        if let Ok(dims) = std::env::var("GBRAIN_EMBEDDING_DIMENSIONS") {
            if let Ok(d) = dims.parse() {
                config.embedding_dimensions = d;
            }
        }

        // --- Query Expansion env vars ---
        if let Ok(key) = std::env::var("GBRAIN_EXPANSION_API_KEY") {
            config.expansion_api_key = Some(key);
        }
        if let Ok(url) = std::env::var("GBRAIN_EXPANSION_BASE_URL") {
            config.expansion_base_url = Some(url);
        }
        if let Ok(model) = std::env::var("GBRAIN_EXPANSION_MODEL") {
            config.expansion_model = model;
        }

        // --- LLM Chunker env vars ---
        if let Ok(key) = std::env::var("GBRAIN_CHUNKER_API_KEY") {
            config.chunker_api_key = Some(key);
        }
        if let Ok(url) = std::env::var("GBRAIN_CHUNKER_BASE_URL") {
            config.chunker_base_url = Some(url);
        }
        if let Ok(model) = std::env::var("GBRAIN_CHUNKER_MODEL") {
            config.chunker_model = model;
        }

        // --- Transcription env vars ---
        if let Ok(provider) = std::env::var("GBRAIN_TRANSCRIPTION_PROVIDER") {
            config.transcription_provider = provider;
        }
        if let Ok(key) = std::env::var("GBRAIN_TRANSCRIPTION_GROQ_API_KEY") {
            config.transcription_groq_api_key = Some(key);
        }
        if let Ok(url) = std::env::var("GBRAIN_TRANSCRIPTION_GROQ_BASE_URL") {
            config.transcription_groq_base_url = Some(url);
        }
        if let Ok(key) = std::env::var("GBRAIN_TRANSCRIPTION_OPENAI_API_KEY") {
            config.transcription_openai_api_key = Some(key);
        }
        if let Ok(url) = std::env::var("GBRAIN_TRANSCRIPTION_OPENAI_BASE_URL") {
            config.transcription_openai_base_url = Some(url);
        }

        // --- Logging env vars ---
        if let Ok(level) = std::env::var("GBRAIN_LOG_LEVEL") {
            config.log_level = level;
        }
        if let Ok(to_file) = std::env::var("GBRAIN_LOG_TO_FILE") {
            config.log_to_file = to_file.parse().unwrap_or(true);
        }
        if let Ok(path) = std::env::var("GBRAIN_LOG_FILE_PATH") {
            config.log_file_path = Some(path);
        }
        if let Ok(to_console) = std::env::var("GBRAIN_LOG_TO_CONSOLE") {
            config.log_to_console = to_console.parse().unwrap_or(true);
        }

        // --- P2-6: Auto-link / Auto-timeline env vars ---
        if let Ok(auto_link) = std::env::var("GBRAIN_AUTO_LINK") {
            config.auto_link = auto_link != "false" && auto_link != "0";
        }
        if let Ok(auto_timeline) = std::env::var("GBRAIN_AUTO_TIMELINE") {
            config.auto_timeline = auto_timeline != "false" && auto_timeline != "0";
        }
        // --- P2-3: Post-write lint env var ---
        if let Ok(post_write_lint) = std::env::var("GBRAIN_POST_WRITE_LINT") {
            config.post_write_lint = post_write_lint == "true" || post_write_lint == "1";
        }

        info!(
            db_path = %config.db_path().display(),
            log_level = %config.log_level,
            log_to_file = config.log_to_file,
            log_to_console = config.log_to_console,
            embedding_model = %config.embedding_model,
            embedding_dimensions = config.embedding_dimensions,
            "Configuration loaded"
        );

        Ok(config)
    }

    /// Get the base directory for gbrain data
    pub fn base_dir() -> PathBuf {
        std::env::var("GBRAIN_DIR")
            .map(PathBuf::from)
            .ok()
            .or_else(|| dirs::home_dir().map(|h| h.join(".gbrain")))
            .unwrap_or_else(|| PathBuf::from(".gbrain"))
    }

    /// Get the database path
    pub fn db_path(&self) -> PathBuf {
        self.database_path
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| Self::base_dir().join("brain.db"))
    }

    /// Get the files storage directory
    pub fn files_dir(&self) -> PathBuf {
        Self::base_dir().join("files")
    }

    /// Get the cache directory
    pub fn cache_dir(&self) -> PathBuf {
        Self::base_dir().join("cache")
    }

    /// P2-7: Get the source of the database URL (mirrors TS getDbUrlSource).
    /// Returns a human-readable string indicating where the DB path comes from:
    /// - "env:GBRAIN_DB_PATH" if set via environment variable
    /// - "config_file" if set in config.json
    /// - "default" if using the default path ($GBRAIN_DIR/brain.db)
    pub fn db_url_source(&self) -> &'static str {
        if std::env::var("GBRAIN_DB_PATH").is_ok() {
            "env:GBRAIN_DB_PATH"
        } else if self.database_path.is_some() {
            // database_path was set but NOT from env (env check above failed),
            // so it must be from config file
            "config_file"
        } else {
            "default"
        }
    }

    // --- Resolved LLM config helpers (fallback to shared OpenAI config) ---

    /// Resolved API key for query expansion (falls back to shared key)
    pub fn expansion_api_key_resolved(&self) -> Option<&str> {
        self.expansion_api_key
            .as_deref()
            .or(self.openai_api_key.as_deref())
    }

    /// Resolved base URL for query expansion (falls back to shared URL)
    pub fn expansion_base_url_resolved(&self) -> Option<&str> {
        self.expansion_base_url
            .as_deref()
            .or(self.openai_base_url.as_deref())
    }

    /// Resolved API key for LLM chunker (falls back to shared key)
    pub fn chunker_api_key_resolved(&self) -> Option<&str> {
        self.chunker_api_key
            .as_deref()
            .or(self.openai_api_key.as_deref())
    }

    /// Resolved base URL for LLM chunker (falls back to shared URL)
    pub fn chunker_base_url_resolved(&self) -> Option<&str> {
        self.chunker_base_url
            .as_deref()
            .or(self.openai_base_url.as_deref())
    }

    /// Resolved API key for transcription (provider-specific, then shared)
    pub fn transcription_api_key_resolved(&self) -> Option<&str> {
        match self.transcription_provider.as_str() {
            "openai" => self
                .transcription_openai_api_key
                .as_deref()
                .or(self.openai_api_key.as_deref()),
            _ => self.transcription_groq_api_key.as_deref(), // "groq" default — no shared fallback
        }
    }

    /// Resolved base URL for transcription (provider-specific, then shared)
    pub fn transcription_base_url_resolved(&self) -> Option<&str> {
        match self.transcription_provider.as_str() {
            "openai" => self
                .transcription_openai_base_url
                .as_deref()
                .or(self.openai_base_url.as_deref()),
            _ => self.transcription_groq_base_url.as_deref(),
        }
    }

    /// Save configuration to config.json with restrictive permissions (0o600 on Unix).
    /// Mirrors TS writeConfig() which uses mode: 0o600.
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config_path = Self::base_dir().join("config.json");
        std::fs::create_dir_all(Self::base_dir())?;
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&config_path, json)?;
        // Set restrictive permissions on Unix (owner read/write only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(&config_path, perms)?;
        }
        info!(path = %config_path.display(), "Saved config.json");
        Ok(())
    }

    /// Merge another config into this one (other takes precedence for Some values)
    fn merge(&mut self, other: Config) {
        if other.database_path.is_some() {
            self.database_path = other.database_path;
        }
        if other.openai_api_key.is_some() {
            self.openai_api_key = other.openai_api_key;
        }
        if other.openai_base_url.is_some() {
            self.openai_base_url = other.openai_base_url;
        }
        // Always take config file values for non-Option fields (they represent
        // explicit user choices, even if they match defaults)
        self.embedding_model = other.embedding_model;
        self.embedding_dimensions = other.embedding_dimensions;
        if other.expansion_api_key.is_some() {
            self.expansion_api_key = other.expansion_api_key;
        }
        if other.expansion_base_url.is_some() {
            self.expansion_base_url = other.expansion_base_url;
        }
        self.expansion_model = other.expansion_model;
        if other.chunker_api_key.is_some() {
            self.chunker_api_key = other.chunker_api_key;
        }
        if other.chunker_base_url.is_some() {
            self.chunker_base_url = other.chunker_base_url;
        }
        self.chunker_model = other.chunker_model;
        self.transcription_provider = other.transcription_provider;
        if other.transcription_groq_api_key.is_some() {
            self.transcription_groq_api_key = other.transcription_groq_api_key;
        }
        if other.transcription_groq_base_url.is_some() {
            self.transcription_groq_base_url = other.transcription_groq_base_url;
        }
        if other.transcription_openai_api_key.is_some() {
            self.transcription_openai_api_key = other.transcription_openai_api_key;
        }
        if other.transcription_openai_base_url.is_some() {
            self.transcription_openai_base_url = other.transcription_openai_base_url;
        }
        self.log_level = other.log_level;
        // Always take config file values for booleans (explicit choice, not magic comparison)
        self.log_to_file = other.log_to_file;
        if other.log_file_path.is_some() {
            self.log_file_path = other.log_file_path;
        }
        self.log_to_console = other.log_to_console;
        // P2-6: auto_link / auto_timeline — always take config file values
        self.auto_link = other.auto_link;
        self.auto_timeline = other.auto_timeline;
        // P2-3: post_write_lint — always take config file value
        self.post_write_lint = other.post_write_lint;
        // wal_mode and pool_size — always take config file values
        self.wal_mode = other.wal_mode;
        self.pool_size = other.pool_size;
        // chunk_size and chunk_overlap — always take config file values
        self.chunk_size = other.chunk_size;
        self.chunk_overlap = other.chunk_overlap;
    }
}
