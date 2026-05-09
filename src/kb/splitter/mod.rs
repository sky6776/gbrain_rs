//! Document splitter module

pub mod markdown_header;
pub mod recursive;
pub mod semantic;

use crate::error::GBrainError;

pub type Chunks = Vec<String>;

/// Document splitter trait
pub trait DocumentSplitter: Send + Sync {
    fn split(&self, text: &str) -> Result<Chunks, GBrainError>;
}

/// Splitter configuration
#[derive(Debug, Clone)]
pub struct SplitterConfig {
    pub file_path: String,
    pub chunk_size: usize,
    pub chunk_overlap: usize,
    pub semantic_enabled: bool,
}

/// Create splitter based on config.
/// Priority: Markdown header > recursive char
pub fn create_splitter(config: &SplitterConfig) -> Box<dyn DocumentSplitter> {
    let ext = std::path::Path::new(&config.file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if ext == "md" || ext == "markdown" {
        return Box::new(markdown_header::MarkdownHeaderSplitter::new());
    }

    Box::new(recursive::RecursiveCharSplitter::new(
        config.chunk_size,
        config.chunk_overlap,
    ))
}

pub use markdown_header::MarkdownHeaderSplitter;
pub use recursive::RecursiveCharSplitter;
pub use semantic::SemanticSplitter;
