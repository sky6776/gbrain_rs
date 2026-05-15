//! gbrain-core — Personal knowledge brain engine
//!
//! Rust implementation of gbrain's core brain functionality using
//! SQLite + sqlite-vec + FTS5 as the storage engine.

pub mod artifact;
pub mod autopilot;
pub mod backoff;
pub mod budget;
pub mod chunker;
pub mod code_index;
pub mod completeness;
pub mod config;
pub mod embedding;
pub mod engine;
pub mod enrichment;
pub mod error;
pub mod fail_improve;
pub mod file_storage;
pub mod jobs;
pub mod kb;
pub mod link_extraction;
pub mod lint;
pub mod logging;
pub mod markdown;
pub mod mcp;
pub mod nlp;
pub mod operations;
pub mod progress;
pub mod resolver;
pub mod scaffold;
pub mod schema;
pub mod search;
pub mod security;
pub mod sqlite_engine;
pub mod sync;
pub mod transcription;
pub mod types;
pub mod validators;
pub mod writer;

pub use autopilot::Autopilot;
pub use engine::BrainEngine;
pub use error::{GBrainError, OperationError};
pub use sqlite_engine::SqliteEngine;
pub use types::*;
