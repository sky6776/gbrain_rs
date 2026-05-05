//! Search module: keyword + vector + hybrid + intent + dedup + expansion + fuzzy + two-pass

pub mod dedup;
pub mod eval;
pub mod expansion;
pub mod fuzzy;
pub mod hybrid;
pub mod intent;
pub mod keyword;
pub mod two_pass;
pub mod vector;

pub use dedup::{dedup_results, DedupOpts};
pub use expansion::{sanitize_expansion_output, sanitize_query_for_prompt};
pub use fuzzy::trigram_similarity;
pub use hybrid::{hybrid_search, HybridOpts};
pub use intent::{classify_intent, detail_for_intent, Intent, Intent as QueryIntent};
pub use two_pass::{expand_anchors, hydrate_chunks, ExpandedChunk, TwoPassOpts};
