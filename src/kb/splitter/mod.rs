//! 文档分割器模块

pub mod markdown_header;
pub mod recursive;
pub mod semantic;

use crate::embedding::Embedder;
use crate::error::GBrainError;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub type Chunks = Vec<String>;

/// 同步文档分割器 trait
pub trait DocumentSplitter: Send + Sync {
    fn split(&self, text: &str) -> Result<Chunks, GBrainError>;
}

/// 异步文档分割器 trait (用于基于嵌入的分割器)
///
/// 使用 `Pin<Box<dyn Future>>` 返回类型以支持 dyn trait object。
pub trait AsyncDocumentSplitter: Send + Sync {
    fn split_async<'a>(
        &'a self,
        text: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Chunks, GBrainError>> + Send + 'a>>;
}

/// 分割器配置
#[derive(Debug, Clone)]
pub struct SplitterConfig {
    pub file_path: String,
    pub chunk_size: usize,
    pub chunk_overlap: usize,
    pub semantic_enabled: bool,
}

/// 根据配置创建同步分割器。
/// 优先级: Markdown header > 递归字符分割
/// 语义分割器不在此处返回 — 异步管道请使用 `create_async_splitter`。
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

/// 根据配置创建异步分割器。
/// 当 `semantic_enabled` 且提供了 embedder 时，返回语义分割器。
/// 否则回退到与 `create_splitter` 相同的逻辑，包装为异步适配器。
pub fn create_async_splitter(
    config: &SplitterConfig,
    embedder: Option<Arc<Embedder>>,
) -> Result<Box<dyn AsyncDocumentSplitter>, GBrainError> {
    if config.semantic_enabled {
        if let Some(emb) = embedder {
            tracing::info!("使用语义分割器 (基于嵌入相似度)");
            return Ok(Box::new(semantic::SemanticSplitter::new(emb)));
        }
        tracing::warn!("semantic_enabled=true 但无 Embedder, 回退到递归分割");
    }

    let ext = std::path::Path::new(&config.file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if ext == "md" || ext == "markdown" {
        Ok(Box::new(SyncAsAsyncAdapter(
            markdown_header::MarkdownHeaderSplitter::new(),
        )))
    } else {
        Ok(Box::new(SyncAsAsyncAdapter(
            recursive::RecursiveCharSplitter::new(
                config.chunk_size,
                config.chunk_overlap,
            ),
        )))
    }
}

/// 将同步 DocumentSplitter 适配为 AsyncDocumentSplitter 的包装器
struct SyncAsAsyncAdapter<T: DocumentSplitter>(T);

impl<T: DocumentSplitter + Send + Sync> AsyncDocumentSplitter for SyncAsAsyncAdapter<T> {
    fn split_async<'a>(
        &'a self,
        text: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Chunks, GBrainError>> + Send + 'a>> {
        Box::pin(async move { self.0.split(text) })
    }
}

pub use markdown_header::MarkdownHeaderSplitter;
pub use recursive::RecursiveCharSplitter;
pub use semantic::SemanticSplitter;