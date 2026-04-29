//! Text chunking module
//!
//! Three chunking strategies:
//! - **Recursive** (`recursive.rs`): Fast, deterministic, splits by paragraph and character count
//! - **LLM-guided** (`llm.rs`): Uses an LLM to identify natural section boundaries
//! - **Semantic** (`semantic.rs`): Savitzky-Golay smoothed boundary detection from heading/paragraph signals

pub mod llm;
pub mod recursive;
pub mod semantic;

// Re-export the primary recursive chunker at module level for backward compatibility
pub use recursive::{chunk_text, estimate_tokens, DEFAULT_CHUNK_OVERLAP, DEFAULT_CHUNK_SIZE};
pub use semantic::{chunk_semantic, SemanticChunkerConfig};
