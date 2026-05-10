//! 备份与恢复 (P5-015~P5-018)
//!
//! 支持 DB + storage 备份，manifest 记录版本信息。

use crate::error::{GBrainError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 备份 archive manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupManifest {
    pub schema_version: i32,
    pub created_at: String,
    pub library_ids: Vec<i64>,
    pub embedding_indexes: Vec<EmbeddingIndexInfo>,
    pub file_count: usize,
    pub db_size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingIndexInfo {
    pub id: i64,
    pub library_id: i64,
    pub model: String,
    pub dimensions: i32,
}

/// 生成备份 manifest
pub fn create_manifest(
    schema_version: i32,
    library_ids: Vec<i64>,
    embedding_indexes: Vec<EmbeddingIndexInfo>,
    file_count: usize,
    db_size_bytes: u64,
) -> BackupManifest {
    BackupManifest {
        schema_version,
        created_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        library_ids,
        embedding_indexes,
        file_count,
        db_size_bytes,
    }
}

/// 备份 DB 文件
pub fn backup_database(db_path: &Path, output_dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(output_dir)
        .map_err(|e| GBrainError::FileError(format!("cannot create backup dir: {}", e)))?;
    let dest = output_dir.join("gbrain.db");
    std::fs::copy(db_path, &dest)
        .map_err(|e| GBrainError::FileError(format!("cannot copy DB: {}", e)))?;
    Ok(dest)
}

/// 备份 storage 目录（kb/files/）
pub fn backup_storage(storage_dir: &Path, output_dir: &Path) -> Result<usize> {
    let dest = output_dir.join("storage");
    std::fs::create_dir_all(&dest)
        .map_err(|e| GBrainError::FileError(format!("cannot create storage backup dir: {}", e)))?;
    copy_dir_recursive(storage_dir, &dest)
}

/// 递归复制目录
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<usize> {
    let mut count = 0usize;
    if !src.exists() {
        return Ok(0);
    }
    for entry in std::fs::read_dir(src)
        .map_err(|e| GBrainError::FileError(format!("cannot read dir {}: {}", src.display(), e)))?
    {
        let entry = entry.map_err(|e| GBrainError::FileError(format!("dir entry error: {}", e)))?;
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());
        if path.is_dir() {
            std::fs::create_dir_all(&dest_path).ok();
            count += copy_dir_recursive(&path, &dest_path)?;
        } else {
            std::fs::copy(&path, &dest_path).ok();
            count += 1;
        }
    }
    Ok(count)
}

/// 从备份恢复 DB
pub fn restore_database(backup_path: &Path, target_db_path: &Path) -> Result<()> {
    std::fs::copy(backup_path, target_db_path)
        .map_err(|e| GBrainError::FileError(format!("cannot restore DB: {}", e)))?;
    Ok(())
}

/// 从备份恢复 storage
pub fn restore_storage(backup_dir: &Path, target_dir: &Path) -> Result<usize> {
    let source = backup_dir.join("storage");
    std::fs::create_dir_all(target_dir)
        .map_err(|e| GBrainError::FileError(format!("cannot create target storage dir: {}", e)))?;
    copy_dir_recursive(&source, target_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_serialization() {
        let m = create_manifest(17, vec![1, 2], vec![], 10, 1024000);
        let json = serde_json::to_string(&m).unwrap();
        let restored: BackupManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.schema_version, 17);
        assert_eq!(restored.file_count, 10);
    }
}
