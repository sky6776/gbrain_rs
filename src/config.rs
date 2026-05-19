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
// 修复：添加 #[serde(default)]，旧 config.json 缺少新增字段时仍能正常加载
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
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
    pub auto_link: bool,     // default true, GBRAIN_AUTO_LINK
    pub auto_timeline: bool, // default true, GBRAIN_AUTO_TIMELINE

    // --- P2-3: Post-write lint (mirrors TS runPostWriteLint) ---
    pub post_write_lint: bool, // default false, GBRAIN_POST_WRITE_LINT

    // --- KB subsystem config ---
    pub kb_enabled: bool,
    pub kb_raptor_secret_ref: Option<String>,
    pub kb_raptor_base_url: Option<String>,
    pub kb_raptor_model: String,
    pub kb_max_file_size_mb: usize,
    pub kb_allowed_extensions: Vec<String>,
    pub kb_storage_dir: Option<String>,
    pub kb_worker_enabled: bool,
    pub kb_worker_poll_interval_secs: u64,
    pub autopilot_enabled: bool,
    pub autopilot_interval_secs: u64,
    /// P3-003: 同义词文件路径
    pub kb_synonyms_file: Option<String>,
    /// P3-004: 别名映射文件路径
    pub kb_aliases_file: Option<String>,

    // --- 单入口多投影融合架构 ---
    /// Artifact 存储目录（默认 $GBRAIN_DIR/artifacts）
    pub artifact_storage_dir: Option<String>,
    /// 默认 KB library ID（upload_source 使用）
    pub default_kb_library_id: Option<i64>,
    /// 上传默认提升策略
    pub upload_default_promotion_policy: String,
    /// artifact 默认意图。默认 "memory"。可选值: memory, evidence, promote
    pub artifact_default_intent: String,
    /// 当 artifact_put 需要写入 KB 但没有 Inbox 库时，是否自动创建。默认 true。
    pub artifact_auto_create_inbox_library: bool,
    /// artifact_put 的 memory 意图是否写入 KB。默认 true。设为 false 则仅写入 gbrain 页面。
    pub artifact_manual_memory_to_kb: bool,
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

            kb_enabled: true,
            kb_raptor_secret_ref: Some("GBRAIN_KB_RAPTOR_API_KEY".to_string()),
            kb_raptor_base_url: None,
            kb_raptor_model: "gpt-4o-mini".to_string(),
            kb_max_file_size_mb: 50,
            kb_allowed_extensions: vec![
                // 修复：默认 KB 允许列表与 KB_SUPPORTED_EXTENSIONS 保持一致，
                // 否则 route planner 认为可处理的文件会被 MCP 安全检查拒绝
                "pdf".into(),
                "docx".into(),
                "xlsx".into(),
                "csv".into(),
                "tsv".into(),
                "html".into(),
                "htm".into(),
                "txt".into(),
                "md".into(),
                "markdown".into(),
                "rst".into(),
                "json".into(),
                "xml".into(),
                "yaml".into(),
                "yml".into(),
                "toml".into(),
            ],
            kb_storage_dir: None,
            kb_worker_enabled: true,
            kb_worker_poll_interval_secs: 30,
            autopilot_enabled: true,
            autopilot_interval_secs: 3600,
            kb_synonyms_file: None,
            kb_aliases_file: None,

            // 单入口多投影融合架构
            artifact_storage_dir: None,
            default_kb_library_id: None,
            upload_default_promotion_policy: "candidate".to_string(),
            // artifact 默认意图为 memory（写入 gbrain 页面 + KB）
            artifact_default_intent: "memory".to_string(),
            // 当 artifact_put 需要写入 KB 但没有 Inbox 库时，自动创建
            artifact_auto_create_inbox_library: true,
            // artifact_put 的 memory 意图默认写入 KB
            artifact_manual_memory_to_kb: true,
        }
    }
}

/// Resolved RAPTOR LLM configuration (avoids cross-module dependency).
#[derive(Debug, Clone)]
pub struct ResolvedRaptorConfig {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
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

        // KB config
        config.kb_enabled = std::env::var("GBRAIN_KB_ENABLED")
            .map(|v| v == "true")
            .unwrap_or(config.kb_enabled);
        config.kb_raptor_secret_ref = std::env::var("GBRAIN_KB_RAPTOR_API_KEY")
            .ok()
            .map(|_| "GBRAIN_KB_RAPTOR_API_KEY".to_string())
            .or(config.kb_raptor_secret_ref);
        if let Ok(url) = std::env::var("GBRAIN_KB_RAPTOR_BASE_URL") {
            config.kb_raptor_base_url = Some(url);
        }
        if let Ok(model) = std::env::var("GBRAIN_KB_RAPTOR_MODEL") {
            config.kb_raptor_model = model;
        }
        if let Ok(size) = std::env::var("GBRAIN_KB_MAX_FILE_SIZE_MB") {
            if let Ok(s) = size.parse() {
                config.kb_max_file_size_mb = s;
            }
        }
        if let Ok(ext) = std::env::var("GBRAIN_KB_ALLOWED_EXTENSIONS") {
            config.kb_allowed_extensions = ext.split(',').map(|s| s.trim().to_string()).collect();
        }
        if let Ok(dir) = std::env::var("GBRAIN_KB_STORAGE_DIR") {
            config.kb_storage_dir = Some(dir);
        }
        config.kb_worker_enabled =
            parse_env_bool("GBRAIN_KB_WORKER_ENABLED").unwrap_or(config.kb_worker_enabled);
        if let Ok(secs) = std::env::var("GBRAIN_KB_WORKER_POLL_INTERVAL") {
            if let Ok(s) = secs.parse() {
                config.kb_worker_poll_interval_secs = s;
            }
        }
        config.autopilot_enabled =
            parse_env_bool("GBRAIN_AUTOPILOT_ENABLED").unwrap_or(config.autopilot_enabled);
        if let Ok(secs) = std::env::var("GBRAIN_AUTOPILOT_INTERVAL") {
            let s: u64 = match secs.parse() {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(
                        "GBRAIN_AUTOPILOT_INTERVAL 无效值 '{}': {}，使用默认 {}s",
                        secs,
                        e,
                        config.autopilot_interval_secs
                    );
                    // 不覆盖，保留 config 默认值
                    config.autopilot_interval_secs
                }
            };
            if s < 60 {
                tracing::warn!("GBRAIN_AUTOPILOT_INTERVAL={}s < 60s，clamp 到 60s", s);
                config.autopilot_interval_secs = 60;
            } else {
                config.autopilot_interval_secs = s;
            }
        }

        // --- 单入口多投影融合架构 env vars ---
        if let Ok(dir) = std::env::var("GBRAIN_ARTIFACT_STORAGE_DIR") {
            config.artifact_storage_dir = Some(dir);
        }
        if let Ok(id) = std::env::var("GBRAIN_DEFAULT_KB_LIBRARY_ID") {
            if let Ok(library_id) = id.parse::<i64>() {
                config.default_kb_library_id = Some(library_id);
            }
        }
        if let Ok(policy) = std::env::var("GBRAIN_UPLOAD_PROMOTION_POLICY") {
            config.upload_default_promotion_policy = policy;
        }
        // artifact 默认意图（默认 "memory"，可选: memory, evidence, promote）
        if let Ok(intent) = std::env::var("GBRAIN_ARTIFACT_DEFAULT_INTENT") {
            config.artifact_default_intent = intent;
        }
        // 当 artifact_put 需要写入 KB 但没有 Inbox 库时，是否自动创建
        config.artifact_auto_create_inbox_library =
            std::env::var("GBRAIN_ARTIFACT_AUTO_CREATE_INBOX_LIBRARY")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(config.artifact_auto_create_inbox_library);
        // artifact_put 的 memory 意图是否写入 KB（默认 true，设为 false 则仅写入 gbrain 页面）
        config.artifact_manual_memory_to_kb = std::env::var("GBRAIN_ARTIFACT_MANUAL_MEMORY_TO_KB")
            .map(|v| v != "false" && v != "0")
            .unwrap_or(config.artifact_manual_memory_to_kb);

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

    /// 获取 artifact store 目录（统一 resolver）
    ///
    /// 修复：上传用 engine.gbrain_dir()/artifacts（db_path 的父目录），
    /// 备份用 Config::base_dir()/artifacts（~/.gbrain/），两者不一致。
    /// 当用户配置了自定义 database_path 但没配置 artifact_storage_dir 时，
    /// 上传写入 db_path 父目录下的 artifacts，备份却从 ~/.gbrain/artifacts 读取，
    /// 导致原始文件漏备份。
    ///
    /// 统一规则：
    /// 1. 用户显式配置 artifact_storage_dir → 直接使用
    /// 2. 未配置 → 使用 db_path 的父目录 + "artifacts"（与上传一致）
    pub fn artifact_dir(&self) -> PathBuf {
        self.artifact_storage_dir
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let db_path = self.db_path();
                db_path
                    .parent()
                    .map(|p| p.join("artifacts"))
                    .unwrap_or_else(|| Self::base_dir().join("artifacts"))
            })
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

    /// 解析 RAPTOR LLM 配置，按优先级：
    /// 1. GBRAIN_KB_RAPTOR_* 环境变量
    /// 2. kb_raptor_secret_ref 指向的环境变量
    /// 3. expansion_api/chunker_api 的 fallback
    /// 4. 默认值 "https://api.openai.com/v1" / "gpt-4o-mini"
    pub fn raptor_config_resolved(&self) -> ResolvedRaptorConfig {
        // API key: GBRAIN_KB_RAPTOR_API_KEY env → kb_raptor_secret_ref → expansion → chunker → shared
        let api_key = std::env::var("GBRAIN_KB_RAPTOR_API_KEY")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| {
                self.kb_raptor_secret_ref
                    .as_deref()
                    .and_then(|ref_name| std::env::var(ref_name).ok())
                    .filter(|s| !s.is_empty())
            })
            .or_else(|| self.expansion_api_key_resolved().map(|s| s.to_string()))
            .or_else(|| self.chunker_api_key_resolved().map(|s| s.to_string()))
            .unwrap_or_default();
        // base_url: KB env → KB config → expansion → chunker → shared
        let base_url = std::env::var("GBRAIN_KB_RAPTOR_BASE_URL")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| self.kb_raptor_base_url.clone().filter(|s| !s.is_empty()))
            .or_else(|| self.expansion_base_url_resolved().map(|s| s.to_string()))
            .or_else(|| self.chunker_base_url_resolved().map(|s| s.to_string()))
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
        // model: KB env → KB config → expansion → chunker → default
        let model = std::env::var("GBRAIN_KB_RAPTOR_MODEL")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                if !self.kb_raptor_model.is_empty() {
                    self.kb_raptor_model.clone()
                } else if !self.expansion_model.is_empty() {
                    self.expansion_model.clone()
                } else if !self.chunker_model.is_empty() {
                    self.chunker_model.clone()
                } else {
                    "gpt-4o-mini".to_string()
                }
            });
        ResolvedRaptorConfig {
            api_key,
            base_url,
            model,
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

    /// 根据 key 读取配置值（用于 `gbrain config get`）。
    /// 返回 Some(value) 表示 key 是 Config 字段，None 表示 key 是 SQLite engine 专用 key。
    pub fn get_field(&self, key: &str) -> Option<String> {
        match key {
            "embedding_model" => Some(self.embedding_model.clone()),
            "embedding_dimensions" => Some(self.embedding_dimensions.to_string()),
            "expansion_model" => Some(self.expansion_model.clone()),
            "chunker_model" => Some(self.chunker_model.clone()),
            "chunk_size" => Some(self.chunk_size.to_string()),
            "chunk_overlap" => Some(self.chunk_overlap.to_string()),
            "log_level" => Some(self.log_level.clone()),
            "log_to_file" => Some(self.log_to_file.to_string()),
            "log_to_console" => Some(self.log_to_console.to_string()),
            "auto_link" => Some(self.auto_link.to_string()),
            "auto_timeline" => Some(self.auto_timeline.to_string()),
            "post_write_lint" => Some(self.post_write_lint.to_string()),
            "kb_enabled" => Some(self.kb_enabled.to_string()),
            "kb_raptor_model" => Some(self.kb_raptor_model.clone()),
            "kb_max_file_size_mb" => Some(self.kb_max_file_size_mb.to_string()),
            "kb_worker_enabled" => Some(self.kb_worker_enabled.to_string()),
            "kb_worker_poll_interval_secs" => Some(self.kb_worker_poll_interval_secs.to_string()),
            "autopilot_enabled" => Some(self.autopilot_enabled.to_string()),
            "autopilot_interval_secs" => Some(self.autopilot_interval_secs.to_string()),
            "upload_default_promotion_policy" => Some(self.upload_default_promotion_policy.clone()),
            "artifact_default_intent" => Some(self.artifact_default_intent.clone()),
            "artifact_auto_create_inbox_library" => {
                Some(self.artifact_auto_create_inbox_library.to_string())
            }
            "artifact_manual_memory_to_kb" => Some(self.artifact_manual_memory_to_kb.to_string()),
            // SQLite engine 专用 key，不在 Config 中
            "writer.lint_on_put_page" => None,
            _ => None,
        }
    }

    /// 根据 key 设置配置值（用于 `gbrain config set`）。
    /// 返回 Ok(()) 表示成功设置，Err(msg) 表示 key 未识别或值无效。
    pub fn apply_set(&mut self, key: &str, value: &str) -> Result<(), String> {
        match key {
            "embedding_model" => self.embedding_model = value.to_string(),
            "embedding_dimensions" => {
                let v: usize = value
                    .parse()
                    .map_err(|_| format!("{} 需要整数，不是: {}", key, value))?;
                self.embedding_dimensions = v;
            }
            "expansion_model" => self.expansion_model = value.to_string(),
            "chunker_model" => self.chunker_model = value.to_string(),
            "chunk_size" => {
                let v: usize = value
                    .parse()
                    .map_err(|_| format!("{} 需要整数，不是: {}", key, value))?;
                self.chunk_size = v;
            }
            "chunk_overlap" => {
                let v: usize = value
                    .parse()
                    .map_err(|_| format!("{} 需要整数，不是: {}", key, value))?;
                self.chunk_overlap = v;
            }
            "log_level" => {
                let valid = ["trace", "debug", "info", "warn", "error"];
                if !valid.contains(&value) {
                    return Err(format!(
                        "log_level 无效值: {}，有效值: {}",
                        value,
                        valid.join(", ")
                    ));
                }
                self.log_level = value.to_string();
            }
            "log_to_file" => self.log_to_file = parse_bool(key, value)?,
            "log_to_console" => self.log_to_console = parse_bool(key, value)?,
            "auto_link" => self.auto_link = parse_bool(key, value)?,
            "auto_timeline" => self.auto_timeline = parse_bool(key, value)?,
            "post_write_lint" => self.post_write_lint = parse_bool(key, value)?,
            "kb_enabled" => self.kb_enabled = parse_bool(key, value)?,
            "kb_raptor_model" => self.kb_raptor_model = value.to_string(),
            "kb_max_file_size_mb" => {
                let v: usize = value
                    .parse()
                    .map_err(|_| format!("{} 需要整数，不是: {}", key, value))?;
                self.kb_max_file_size_mb = v;
            }
            "kb_worker_enabled" => self.kb_worker_enabled = parse_bool(key, value)?,
            "kb_worker_poll_interval_secs" => {
                let v: u64 = value
                    .parse()
                    .map_err(|_| format!("{} 需要整数，不是: {}", key, value))?;
                self.kb_worker_poll_interval_secs = v;
            }
            "autopilot_enabled" => self.autopilot_enabled = parse_bool(key, value)?,
            "autopilot_interval_secs" => {
                let v: u64 = value
                    .parse()
                    .map_err(|_| format!("{} 需要整数，不是: {}", key, value))?;
                if v < 60 {
                    return Err(format!(
                        "autopilot_interval_secs 最小值为 60 秒，当前值: {}",
                        v
                    ));
                }
                self.autopilot_interval_secs = v;
            }
            "upload_default_promotion_policy" => {
                // 校验合法枚举值，避免无效 policy 在 apply_promotion_policy 被静默忽略
                let valid = ["none", "shadow", "candidate", "auto-low-risk"];
                if !valid.contains(&value) {
                    return Err(format!(
                        "upload_default_promotion_policy 无效值: {}，有效值: {}",
                        value,
                        valid.join(", ")
                    ));
                }
                self.upload_default_promotion_policy = value.to_string()
            }
            "artifact_default_intent" => {
                // 校验合法枚举值，避免无效 intent 在 routing 被静默退回 Auto
                let valid = ["memory", "evidence", "promote"];
                if !valid.contains(&value) {
                    return Err(format!(
                        "artifact_default_intent 无效值: {}，有效值: {}",
                        value,
                        valid.join(", ")
                    ));
                }
                self.artifact_default_intent = value.to_string()
            }
            "artifact_auto_create_inbox_library" => {
                self.artifact_auto_create_inbox_library = parse_bool(key, value)?
            }
            "artifact_manual_memory_to_kb" => {
                self.artifact_manual_memory_to_kb = parse_bool(key, value)?
            }
            // SQLite engine 专用 key（保留兼容），不在 Config 中，仅写入 DB
            "writer.lint_on_put_page" => {
                return Err(format!(
                    "{} 是 SQLite engine 专用 key，需连接数据库后才能设置",
                    key
                ))
            }
            _ => return Err(format!("未知配置 key: {}", key)),
        }
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
        // KB subsystem — always take config file values
        self.kb_enabled = other.kb_enabled;
        if other.kb_raptor_secret_ref.is_some() {
            self.kb_raptor_secret_ref = other.kb_raptor_secret_ref;
        }
        if other.kb_raptor_base_url.is_some() {
            self.kb_raptor_base_url = other.kb_raptor_base_url;
        }
        self.kb_raptor_model = other.kb_raptor_model;
        self.kb_max_file_size_mb = other.kb_max_file_size_mb;
        if !other.kb_allowed_extensions.is_empty() {
            self.kb_allowed_extensions = other.kb_allowed_extensions;
        }
        if other.kb_storage_dir.is_some() {
            self.kb_storage_dir = other.kb_storage_dir;
        }
        self.kb_worker_enabled = other.kb_worker_enabled;
        self.kb_worker_poll_interval_secs = other.kb_worker_poll_interval_secs;
        self.autopilot_enabled = other.autopilot_enabled;
        self.autopilot_interval_secs = other.autopilot_interval_secs;
        if other.kb_synonyms_file.is_some() {
            self.kb_synonyms_file = other.kb_synonyms_file.clone();
        }
        if other.kb_aliases_file.is_some() {
            self.kb_aliases_file = other.kb_aliases_file.clone();
        }
        // 单入口多投影融合架构 — config file values
        if other.artifact_storage_dir.is_some() {
            self.artifact_storage_dir = other.artifact_storage_dir;
        }
        if other.default_kb_library_id.is_some() {
            self.default_kb_library_id = other.default_kb_library_id;
        }
        if !other.upload_default_promotion_policy.is_empty() {
            self.upload_default_promotion_policy = other.upload_default_promotion_policy;
        }
        // artifact_default_intent — always take config file value
        if !other.artifact_default_intent.is_empty() {
            self.artifact_default_intent = other.artifact_default_intent;
        }
        // artifact_auto_create_inbox_library — always take config file value
        self.artifact_auto_create_inbox_library = other.artifact_auto_create_inbox_library;
        // artifact_manual_memory_to_kb — always take config file value
        self.artifact_manual_memory_to_kb = other.artifact_manual_memory_to_kb;
    }
}

/// 解析环境变量布尔值，接受 true/false/1/0/TURE/FALSE（大小写不敏感）。
/// 非法值会打 warn 日志并返回 None，由调用方决定回退策略。
fn parse_env_bool(var_name: &str) -> Option<bool> {
    let val = std::env::var(var_name).ok()?;
    match val.to_lowercase().as_str() {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => {
            tracing::warn!(
                "{} 无效值 '{}'，有效值: true/false/1/0，已忽略",
                var_name,
                val
            );
            None
        }
    }
}

/// 严格解析布尔值，只接受 true/false/1/0，拒绝拼写错误。
/// 用于 `gbrain config set` 的参数校验，防止 `flase` 被静默视为 true。
fn parse_bool(key: &str, value: &str) -> Result<bool, String> {
    match value {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(format!(
            "{} 需要布尔值 (true/false/1/0)，不是: {}",
            key, value
        )),
    }
}
