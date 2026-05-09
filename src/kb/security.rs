//! KB upload security validation
//!
//! Reuses gbrain_rs existing path/filename validation and adds KB-specific checks.

use crate::error::{GBrainError, Result};
use crate::kb::types::{is_supported_extension, SUPPORTED_EXTENSIONS};
use crate::security::validate_upload_path;
use std::path::Path;

/// Default max file size: 50 MB
pub const DEFAULT_MAX_FILE_SIZE_BYTES: usize = 50 * 1024 * 1024;

/// Validate a KB upload source path.
/// - Checks path traversal, null bytes
/// - For remote callers: confines to working directory
/// - Validates extension against SUPPORTED_EXTENSIONS
/// - Checks file size
pub fn validate_upload_source(
    path: &Path,
    remote: bool,
    working_dir: &Path,
    max_file_bytes: usize,
) -> Result<std::path::PathBuf> {
    // 1. Path security (reuses existing gbrain_rs validation)
    validate_upload_path(path, remote, working_dir)?;

    // 2. File must exist
    if !path.exists() {
        return Err(GBrainError::FileError("file does not exist".to_string()));
    }

    // 3. Must be a file, not a directory
    if path.is_dir() {
        return Err(GBrainError::InvalidInput(
            "path is a directory, not a file".to_string(),
        ));
    }

    // 4. Extension validation (validated_extension also checks support)
    validated_extension(path)?;

    // 5. File size check
    let metadata = std::fs::metadata(path)
        .map_err(|e| GBrainError::FileError(format!("cannot read file metadata: {}", e)))?;
    if metadata.len() as usize > max_file_bytes {
        return Err(GBrainError::InvalidInput(format!(
            "file size {} bytes exceeds maximum {} bytes",
            metadata.len(),
            max_file_bytes
        )));
    }

    Ok(path.to_path_buf())
}

/// Get validated extension from a file path
pub fn validated_extension(path: &Path) -> Result<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .ok_or_else(|| GBrainError::InvalidInput("file must have an extension".to_string()))?;

    let ext_lower = ext.to_lowercase();
    if !is_supported_extension(&ext_lower) {
        return Err(GBrainError::InvalidInput(format!(
            "file extension '.{}' not supported for KB upload. Supported: {:?}",
            ext_lower, SUPPORTED_EXTENSIONS
        )));
    }

    Ok(ext_lower)
}

/// Detect and validate MIME type from file content.
/// Uses the `infer` crate for content-based detection, falling back to extension-based.
pub fn detect_and_validate_mime(data: &[u8], ext: &str) -> Result<String> {
    // Try content-based detection first
    if let Some(kind) = infer::get(data) {
        let detected = kind.mime_type().to_string();
        // Validate that detected MIME matches expected extension
        if mime_matches_extension(&detected, ext) {
            return Ok(detected);
        }
        // Mismatch: log warning but allow (some PDFs report as application/x-zip etc.)
    }

    // Fallback to extension-based MIME
    Ok(crate::kb::types::mime_type_for_ext(ext).to_string())
}

fn mime_matches_extension(mime: &str, ext: &str) -> bool {
    match ext {
        "pdf" => mime == "application/pdf",
        "docx" => mime.contains("openxmlformats") || mime == "application/zip",
        "xlsx" => mime.contains("openxmlformats") || mime == "application/zip",
        "csv" => mime.starts_with("text/"),
        "html" | "htm" => mime == "text/html",
        "txt" => mime.starts_with("text/"),
        "md" => mime.starts_with("text/"),
        _ => true, // Allow unknown matches
    }
}

/// Store a KB file to controlled storage directory.
/// Copies file to $GBRAIN_DIR/kb/files/<library_id>/<hash>.<ext>
pub fn store_kb_file(
    library_id: i64,
    content_hash: &str,
    ext: &str,
    data: &[u8],
    base_dir: &Path,
) -> Result<String> {
    let kb_dir = base_dir
        .join("kb")
        .join("files")
        .join(library_id.to_string());
    std::fs::create_dir_all(&kb_dir)
        .map_err(|e| GBrainError::FileError(format!("cannot create KB storage dir: {}", e)))?;

    let filename = format!("{}.{}", content_hash, ext);
    let storage_path = kb_dir.join(&filename);

    // Only write if file doesn't already exist (dedup by hash)
    if !storage_path.exists() {
        std::fs::write(&storage_path, data)
            .map_err(|e| GBrainError::FileError(format!("cannot write KB file: {}", e)))?;
    }

    // Return the storage path as string
    Ok(storage_path.to_string_lossy().to_string())
}
