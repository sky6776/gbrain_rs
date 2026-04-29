//! File storage operations
//! Mirrors gbrain's src/core/storage.ts

use crate::engine::BrainEngine;
use crate::error::{GBrainError, Result};
use crate::security::{validate_contained, validate_filename, validate_page_slug, validate_upload_path};
use crate::types::*;
use std::path::Path;

/// Upload a file to brain storage
pub fn upload_file<E: BrainEngine>(
    engine: &E,
    source_path: &Path,
    slug: &str,
    opts: FileUploadOptions,
    remote: bool,
    working_dir: &Path,
) -> Result<FileRecord> {
    validate_page_slug(slug)?;
    validate_upload_path(source_path, remote, working_dir)?;

    // For remote callers, also use validate_contained for robust path traversal protection
    // (canonicalizes both paths and verifies containment)
    if remote {
        validate_contained(source_path, working_dir, remote)?;
    }

    let filename = source_path
        .file_name()
        .ok_or_else(|| GBrainError::FileError("No filename".to_string()))?
        .to_string_lossy();

    validate_filename(&filename)?;

    engine.file_upload(source_path, slug, opts)
}

/// Read a file from brain storage, validating that the path is contained within the base directory.
/// Mirrors TS LocalStorage.contained() — prevents path traversal on file reads.
/// Note: Symlink rejection is not applied here since this is a read operation (no TOCTOU risk).
pub fn read_file(path: &Path, base_dir: &Path) -> Result<std::path::PathBuf> {
    validate_contained(path, base_dir, false)
}

/// List files in brain storage
pub fn list_files<E: BrainEngine>(
    engine: &E,
    slug: Option<&str>,
    limit: Option<usize>,
) -> Result<Vec<FileRecord>> {
    engine.file_list(slug, limit)
}

/// Get a file URL
pub fn get_file_url<E: BrainEngine>(engine: &E, file_id: i64, mode: FileUrlMode) -> Result<String> {
    engine.file_url(file_id, mode)
}
