//! KB (Knowledge Base) subsystem for gbrain_rs
//!
//! Handles document upload, parsing, splitting, embedding, RAPTOR summarization,
//! and hybrid search (vector + FTS5 + RRF).

pub mod chinese;
pub mod engine;
pub mod jobs;
pub mod parser;
pub mod pipeline;
pub mod raptor;
pub mod search;
pub mod security;
pub mod splitter;
pub mod types;
pub mod worker;

pub use engine::KbEngine;
pub use pipeline::{ingest_directory, process_document, process_document_async};
pub use search::kb_search;
pub use types::*;
pub use worker::{run_kb_worker_loop, run_kb_worker_once, spawn_kb_worker_thread};
