//! 统一上传引擎 — upload_source 的核心实现
//!
//! 负责：
//! 1. 计算文件 SHA256 去重
//! 2. 写入 Artifact Store（按 hash 命名）
//! 3. 创建/复用 source_artifacts 记录
//! 4. 创建 artifact_occurrences 记录
//! 5. 根据路由计划创建各投影（KB / gbrain / 文件存储）
//! 6. 创建影子页面

use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

use crate::artifact::projection;
use crate::error::{GBrainError, Result};
use crate::security::validate_page_slug;

use super::store;
use super::types::*;

/// 计算内容的 SHA256 哈希
pub fn compute_sha256(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    format!("{:x}", hasher.finalize())
}

/// 生成 artifact_uid（art_ + 12位hash前缀 + 4位随机后缀）
pub fn generate_artifact_uid(sha256: &str) -> String {
    let prefix = &sha256[..12.min(sha256.len())];
    // 使用时间戳低 16 位作为伪随机后缀
    let suffix = format!(
        "{:04x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
            % 65536
    );
    format!("art_{}{}", prefix, suffix)
}

/// 生成 occurrence_uid（occ_ + 12位hash前缀 + 4位随机后缀）
pub fn generate_occurrence_uid(artifact_uid: &str) -> String {
    let suffix = format!(
        "{:04x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
            % 65536
    );
    format!(
        "occ_{}{}",
        &artifact_uid[..16.min(artifact_uid.len())],
        suffix
    )
}

/// 获取当前时间字符串
fn now_str() -> String {
    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// 写入 Artifact Store（按 hash 去重，路径: $GBRAIN_DIR/artifacts/<sha256[0..2]>/<sha256>.<ext>）
pub fn write_artifact_file(
    content: &[u8],
    sha256: &str,
    extension: &str,
    artifact_dir: &Path,
) -> Result<PathBuf> {
    // 按前两位 hash 分桶
    let bucket = &sha256[..2.min(sha256.len())];
    let dir = artifact_dir.join(bucket);
    fs::create_dir_all(&dir)
        .map_err(|e| GBrainError::FileError(format!("创建 artifact 目录失败: {}", e)))?;

    let filename = if extension.is_empty() {
        sha256.to_string()
    } else {
        format!("{}.{}", sha256, extension)
    };
    let path = dir.join(&filename);

    // write-if-absent：如果文件已存在且大小匹配，跳过写入
    if path.exists() {
        if let Ok(metadata) = fs::metadata(&path) {
            if metadata.len() as usize == content.len() {
                debug!("Artifact 文件已存在，跳过写入: {:?}", path);
                return Ok(path);
            }
        }
    }

    // 原子写入：先写临时文件，再重命名
    let tmp_path = dir.join(format!("{}.tmp", sha256));
    let mut file = fs::File::create(&tmp_path)
        .map_err(|e| GBrainError::FileError(format!("创建临时文件失败: {}", e)))?;
    file.write_all(content)
        .map_err(|e| GBrainError::FileError(format!("写入临时文件失败: {}", e)))?;
    file.flush()
        .map_err(|e| GBrainError::FileError(format!("刷新临时文件失败: {}", e)))?;
    drop(file);

    // 如果目标已存在（并发写入），直接删除临时文件
    if path.exists() {
        let _ = fs::remove_file(&tmp_path);
        return Ok(path);
    }

    fs::rename(&tmp_path, &path)
        .map_err(|e| GBrainError::FileError(format!("重命名临时文件失败: {}", e)))?;

    Ok(path)
}

/// 验证 artifact 文件完整性
pub fn verify_artifact_integrity(path: &Path, expected_sha256: &str) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    let content = fs::read(path)
        .map_err(|e| GBrainError::FileError(format!("读取 artifact 文件失败: {}", e)))?;
    let actual_sha256 = compute_sha256(&content);
    Ok(actual_sha256 == expected_sha256)
}

/// 统一上传核心逻辑
///
/// 步骤：
/// 1. 计算 SHA256
/// 2. 查找已有 artifact（去重）
/// 3. 写入 artifact store
/// 4. 创建/复用 source_artifacts 记录
/// 5. 创建 artifact_occurrences 记录
/// 6. 根据路由计划创建投影
pub fn upload_source(
    conn: &Connection,
    input: &UploadSourceInput,
    artifact_dir: &Path,
    _gbrain_dir: &Path,
    config_default_library_id: Option<i64>,
    config_default_promotion_policy: &str,
) -> Result<UploadSourceOutput> {
    // 1. 计算 SHA256
    let sha256 = compute_sha256(&input.content);

    // 2. 推断扩展名和 MIME 类型
    let extension = infer_extension(&input.original_name);
    let mime_type = infer_mime_type(&extension);

    // 3. 推断路由计划
    let route_plan = infer_route_plan(&extension, &mime_type, &input.intent);

    // 修复：提升策略优先级：用户显式指定 > config 默认值 > intent 推断
    // 之前 config 的 upload_default_promotion_policy 不参与决策，用户配置它不会生效
    let route_plan = if let Some(policy) = &input.promotion_policy {
        RoutePlan {
            promotion: policy.clone(),
            ..route_plan
        }
    } else if !config_default_promotion_policy.is_empty()
        && config_default_promotion_policy != "candidate"
    {
        // config 默认值非空且非默认的 "candidate" 时使用配置值
        // "candidate" 是初始默认值，与 intent 推断结果一致，无需覆盖
        if let Ok(policy) = config_default_promotion_policy.parse() {
            RoutePlan {
                promotion: policy,
                ..route_plan
            }
        } else {
            route_plan
        }
    } else {
        route_plan
    };

    // dry_run 模式：仅返回路由计划，不实际写入（必须在任何落库/落盘之前判断）
    if input.dry_run {
        return Ok(UploadSourceOutput {
            artifact_id: 0,
            artifact_uid: String::new(),
            occurrence_id: 0,
            occurrence_uid: String::new(),
            sha256,
            is_new: true,
            route_plan,
            projections: Vec::new(),
        });
    }

    // 4. 查找已有 artifact
    let existing = store::find_artifact_by_sha256(conn, &sha256)
        .map_err(|e| GBrainError::Database(format!("查找 artifact 失败: {}", e)))?;

    let (artifact_id, artifact_uid, is_new) = if let Some(existing) = existing {
        // 修复：复用已软删除的 artifact 时需要 reactivation，
        // 否则新 occurrence/projection 会挂到 deleted artifact 上，
        // 列表查不到，GC 又把它当孤儿投影处理
        if existing.status == "deleted" {
            store::reactivate_artifact(conn, existing.id)
                .map_err(|e| GBrainError::Database(format!("重新激活 artifact 失败: {}", e)))?;
        } else {
            store::touch_artifact(conn, existing.id)
                .map_err(|e| GBrainError::Database(format!("更新 artifact 失败: {}", e)))?;
        }
        (existing.id, existing.artifact_uid.clone(), false)
    } else {
        // 5. 写入 artifact store
        let storage_path = write_artifact_file(&input.content, &sha256, &extension, artifact_dir)?;
        let storage_path_str = storage_path.to_string_lossy().to_string();

        // 6. 生成 UID 和 slug
        let artifact_uid = generate_artifact_uid(&sha256);
        let canonical_slug = make_canonical_slug(&input.original_name, &sha256);

        let now = now_str();
        let artifact = SourceArtifact {
            id: 0,
            artifact_uid: artifact_uid.clone(),
            created_at: now.clone(),
            updated_at: now.clone(),
            last_seen_at: Some(now.clone()),
            sha256: sha256.clone(),
            original_name: input.original_name.clone(),
            extension: extension.clone(),
            mime_type: mime_type.clone(),
            size_bytes: input.content.len() as i64,
            storage_path: storage_path_str,
            canonical_slug: canonical_slug.clone(),
            status: "active".to_string(),
            metadata_json: input
                .metadata
                .as_ref()
                .map(|v| v.to_string())
                .unwrap_or_else(|| "{}".to_string()),
            deleted_at: None,
            purged_at: None,
        };

        let id = store::insert_artifact(conn, &artifact)
            .map_err(|e| GBrainError::Database(format!("插入 artifact 失败: {}", e)))?;

        // 记录 artifact_created 事件（§7.6）
        let _ = store::record_event(
            conn,
            Some(id),
            None,
            "artifact_created",
            "upload_source",
            &serde_json::json!({"sha256": sha256, "original_name": input.original_name})
                .to_string(),
        );

        (id, artifact_uid, true)
    };

    // 7. 创建 occurrence
    let occurrence_uid = generate_occurrence_uid(&artifact_uid);
    let now = now_str();

    // 设计文档 §17: 如果 library_id 为 None 但路由计划需要 KB，
    // 将在投影创建阶段自动解析默认库，occurrence 中记录原始输入的 library_id
    let occurrence = ArtifactOccurrence {
        id: 0,
        occurrence_uid: occurrence_uid.clone(),
        created_at: now.clone(),
        updated_at: now.clone(),
        artifact_id,
        source_kind: input.source_kind.to_string(),
        source_uri: input.source_uri.clone(),
        // 修复：从 input.path 或 source_uri 传入 original_path，
        // 之前固定写空字符串，导致 KB document 的 original_path 也继承空值，
        // 削弱后续审计、追溯和恢复诊断能力
        original_path: input
            .path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| input.source_uri.clone()),
        original_name: input.original_name.clone(),
        owner_ref: input.owner_ref.clone().unwrap_or_default(),
        intent: input.intent.to_string(),
        target_slug: input.target_slug.clone().unwrap_or_default(),
        page_slug: input.page_slug.clone().unwrap_or_default(),
        library_id: input.library_id,
        folder_id: input.folder_id,
        // 修复：使用最终决定的 promotion（可能是 intent 推断的或用户显式指定的）
        promotion_policy: route_plan.promotion.to_string(),
        status: "active".to_string(),
        metadata_json: input
            .metadata
            .as_ref()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "{}".to_string()),
    };

    let occurrence_id = store::insert_occurrence(conn, &occurrence)
        .map_err(|e| GBrainError::Database(format!("插入 occurrence 失败: {}", e)))?;

    // 记录 occurrence_created 事件（§7.6）
    let _ = store::record_event(
        conn,
        Some(artifact_id),
        Some(occurrence_id),
        "occurrence_created",
        "upload_source",
        &serde_json::json!({"source_kind": input.source_kind, "intent": input.intent}).to_string(),
    );

    // 9. 创建投影
    let mut projections = Vec::new();
    let mut kb_document_id_for_shadow: Option<i64> = None;

    // KB 投影 — 设计文档 §17 默认 library 策略优先级链：
    // 1. 用户显式 library_id
    // 2. config: default_kb_library_id
    // 3. 名为 Default 或 Inbox 的 library
    // 4. 不存在则自动创建 Inbox
    if route_plan.to_kb {
        let resolved_library_id =
            resolve_default_library(conn, input.library_id, config_default_library_id)?;
        let proj_key = format!("library:{}", resolved_library_id);
        // 使用 projection::create_kb_projection 创建 KB 投影和 kb_document
        // 它会自动更新 projection_ref 为实际的 kb_document:{id}
        let proj_ref =
            projection::create_kb_projection(conn, artifact_id, occurrence_id, resolved_library_id)
                .map_err(|e| GBrainError::Database(format!("创建 KB 投影失败: {}", e)))?;
        // 修复：从 proj_ref 解析 kb_document_id，传给 shadow page frontmatter
        kb_document_id_for_shadow = proj_ref
            .strip_prefix("kb_document:")
            .and_then(|s| s.parse::<i64>().ok());
        projections.push(ProjectionResult {
            projection_type: ProjectionType::KbDocument,
            projection_key: proj_key,
            projection_ref: proj_ref,
            created: true,
            status: "active".to_string(),
        });
    }

    // 影子页面投影 — 设计文档 §9/§15.1: 使用 put_page 创建 documents/<slug> 页面
    // 影子页面在 upload_source 阶段直接写入 pages 表（MVP 最小追溯链要求）
    let mut _shadow_page_slug: Option<String> = None;
    if route_plan.to_shadow {
        let slug = format!(
            "documents/{}",
            make_canonical_slug(&input.original_name, &sha256)
        );
        // 验证 slug
        if validate_page_slug(&slug).is_ok() {
            let proj_key = format!("slug:{}", slug);
            let proj_ref = format!("slug:{}", slug);
            let now = now_str();
            let proj = ArtifactProjection {
                id: 0,
                created_at: now.clone(),
                updated_at: now.clone(),
                artifact_id,
                occurrence_id: Some(occurrence_id),
                projection_type: ProjectionType::BrainShadowPage.to_string(),
                projection_key: proj_key.clone(),
                projection_ref: proj_ref.clone(),
                status: "active".to_string(),
                version_hash: String::new(),
                stale_reason: String::new(),
                metadata_json: "{}".to_string(),
                superseded_by: None,
            };
            store::insert_projection(conn, &proj)
                .map_err(|e| GBrainError::Database(format!("插入影子页面投影失败: {}", e)))?;
            projections.push(ProjectionResult {
                projection_type: ProjectionType::BrainShadowPage,
                projection_key: proj_key,
                projection_ref: proj_ref,
                created: true,
                status: "active".to_string(),
            });

            // 实际写入 gbrain pages 表（§9.2/§9.3 frontmatter + body 模板）
            // §15.2 强制规则: shadow page 中候选实体不使用 wikilink
            let artifact = store::find_artifact_by_id(conn, artifact_id)
                .map_err(|e| GBrainError::Database(format!("查找 artifact 失败: {}", e)))?
                .ok_or_else(|| {
                    GBrainError::PageNotFound(format!("artifact {} 不存在", artifact_id))
                })?;
            let (title, frontmatter, body) =
                create_shadow_page_content(&artifact, &occurrence, kb_document_id_for_shadow);

            // 修复：补齐 title_tokens、compiled_truth_tokens、content_hash，
            // 与 put_page 保持一致，确保中文分词索引正确
            let title_tokens = crate::nlp::chinese::tokenize_content(&title);
            let truth_tokens = crate::nlp::chinese::tokenize_content(&body);
            let content_hash = {
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(title.as_bytes());
                hasher.update(body.as_bytes());
                format!("{:x}", hasher.finalize())
            };

            // 修复：之前使用 INSERT OR REPLACE，重复上传同一 artifact 时会覆盖
            // 人工维护过的 shadow page 内容和 page id。
            // 改为 INSERT OR IGNORE：已存在时不覆盖，只补充缺失的 frontmatter/metadata
            let rows_affected = conn.execute(
                "INSERT OR IGNORE INTO pages (slug, title, compiled_truth, page_type, frontmatter, content_hash, title_tokens, compiled_truth_tokens, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 'source', ?4, ?5, ?6, ?7, datetime('now'), datetime('now'))",
                rusqlite::params![slug, title, body, frontmatter, content_hash, title_tokens, truth_tokens],
            ).map_err(|e| GBrainError::Database(format!("写入影子页面失败: {}", e)))?;

            if rows_affected > 0 {
                // 新建页面时同步创建基础 chunk，确保 BrainFirst 搜索能找到
                // 修复：补齐 chunk_text_tokens/token_count/chunk_source，
                // 与 sqlite_engine.rs put_chunks 路径一致，确保中文 FTS 召回完整
                let chunk_text = if body.chars().count() > 800 {
                    format!("{}...", body.chars().take(800).collect::<String>())
                } else {
                    body.clone()
                };
                let chunk_text_tokens = crate::nlp::chinese::tokenize_content(&chunk_text);
                let token_count = chunk_text_tokens.split_whitespace().count() as i64;
                conn.execute(
                    "INSERT INTO chunks (page_id, chunk_index, chunk_text, chunk_text_tokens, token_count, chunk_source, created_at)
                     VALUES ((SELECT id FROM pages WHERE slug = ?1), 0, ?2, ?3, ?4, 'body', datetime('now'))",
                    rusqlite::params![slug, chunk_text, chunk_text_tokens, token_count],
                )
                .map_err(|e| GBrainError::Database(format!("创建影子页面 chunk 失败: {}", e)))?;
            } else {
                // 页面已存在，只更新 frontmatter 和 content_hash（不覆盖 compiled content）
                debug!("shadow page 已存在，补充 frontmatter: {}", slug);
                let _ = conn.execute(
                    "UPDATE pages SET frontmatter = ?1, content_hash = ?2, updated_at = datetime('now')
                     WHERE slug = ?3 AND (frontmatter IS NULL OR frontmatter = '' OR content_hash IS NULL OR content_hash = '')",
                    rusqlite::params![frontmatter, content_hash, slug],
                );
            }

            _shadow_page_slug = Some(slug);
        }
    }

    // 文件存储投影
    if route_plan.to_file {
        if let Some(page_slug) = &input.page_slug {
            let proj_key = format!("page:{}:file:{}", page_slug, input.original_name);
            let proj_ref = format!("file:{}", artifact_id);
            let now = now_str();
            let proj = ArtifactProjection {
                id: 0,
                created_at: now.clone(),
                updated_at: now.clone(),
                artifact_id,
                occurrence_id: Some(occurrence_id),
                projection_type: ProjectionType::FileAttachment.to_string(),
                projection_key: proj_key.clone(),
                projection_ref: proj_ref.clone(),
                status: "active".to_string(),
                version_hash: String::new(),
                stale_reason: String::new(),
                metadata_json: "{}".to_string(),
                superseded_by: None,
            };
            store::insert_projection(conn, &proj)
                .map_err(|e| GBrainError::Database(format!("插入文件投影失败: {}", e)))?;
            projections.push(ProjectionResult {
                projection_type: ProjectionType::FileAttachment,
                projection_key: proj_key,
                projection_ref: proj_ref,
                created: true,
                status: "active".to_string(),
            });
        }
    }

    // gbrain page 投影
    if route_plan.to_brain {
        if let Some(target_slug) = &input.target_slug {
            let proj_key = format!("slug:{}", target_slug);
            let proj_ref = format!("slug:{}", target_slug);
            let now = now_str();
            let proj = ArtifactProjection {
                id: 0,
                created_at: now.clone(),
                updated_at: now.clone(),
                artifact_id,
                occurrence_id: Some(occurrence_id),
                projection_type: ProjectionType::BrainPageUpdate.to_string(),
                projection_key: proj_key.clone(),
                projection_ref: proj_ref.clone(),
                status: "active".to_string(),
                version_hash: String::new(),
                stale_reason: String::new(),
                metadata_json: "{}".to_string(),
                superseded_by: None,
            };
            store::insert_projection(conn, &proj)
                .map_err(|e| GBrainError::Database(format!("插入 brain page 投影失败: {}", e)))?;
            projections.push(ProjectionResult {
                projection_type: ProjectionType::BrainPageUpdate,
                projection_key: proj_key,
                projection_ref: proj_ref,
                created: true,
                status: "active".to_string(),
            });
        }
    }

    Ok(UploadSourceOutput {
        artifact_id,
        artifact_uid,
        occurrence_id,
        occurrence_uid,
        sha256,
        is_new,
        route_plan,
        projections,
    })
}

/// 创建影子页面
///
/// 使用现有 put_page 创建 `documents/<slug>` 页面，
/// frontmatter 包含 artifact_id、occurrence_id、kb_document_id、source_hash 等。
pub fn create_shadow_page_content(
    artifact: &SourceArtifact,
    occurrence: &ArtifactOccurrence,
    kb_document_id: Option<i64>,
) -> (String, String, String) {
    // 标题
    let title = if artifact.original_name.is_empty() {
        artifact.canonical_slug.clone()
    } else {
        artifact.original_name.clone()
    };

    // 修复：frontmatter 使用 JSON 格式（operations.rs:1232 按 JSON 解析 frontmatter）
    // 之前写成 YAML 格式（---key: value---），导致 frontmatter 解析失败
    let frontmatter = serde_json::json!({
        "page_type": "source",
        "artifact_id": artifact.artifact_uid,
        "artifact_occurrence_id": occurrence.occurrence_uid,
        "kb_document_id": kb_document_id.map_or(serde_json::Value::Null, |id| serde_json::json!(id)),
        "source_hash": format!("sha256:{}", artifact.sha256),
        "source_type": "uploaded_document",
        "source_mime_type": artifact.mime_type,
        "source_original_name": artifact.original_name,
        "source_ref": format!("artifact://{}", artifact.artifact_uid),
        "promotion_status": "auto_summary",
        "projection_status": "active",
    }).to_string();

    // body
    let size_mb = artifact.size_bytes as f64 / 1024.0 / 1024.0;
    let body = format!(
        "# {}\n\n\
         ## Summary\n\n\
         Pending KB processing\n\n\
         ## Key Details\n\n\
         - Type: {}\n\
         - Size: {:.1} MB\n\
         - KB Document: {}\n\
         - Artifact: {}\n\
         - Hash: sha256:{}\n\n\
         ## Entities\n\n\
         Pending review\n\n\
         ## Candidate Timeline\n\n\
         Pending review\n\n\
         ## Source\n\n\
         - Original: artifact://{}\n\
         - Occurrence: occurrence://{}\n\
         - KB: kb_document://{}",
        title,
        artifact.extension.to_uppercase(),
        size_mb,
        kb_document_id.map_or("null".to_string(), |id| id.to_string()),
        artifact.artifact_uid,
        artifact.sha256,
        artifact.artifact_uid,
        occurrence.occurrence_uid,
        kb_document_id.map_or("null".to_string(), |id| id.to_string()),
    );

    (title, frontmatter, body)
}

/// 解析默认 library ID
///
/// 设计文档 §17 优先级链：
/// 1. 用户显式 library_id（已传入）
/// 2. config: default_kb_library_id
/// 3. 名为 Default 或 Inbox 的 library
/// 4. 不存在则自动创建 Inbox
fn resolve_default_library(
    conn: &Connection,
    explicit_library_id: Option<i64>,
    config_default_library_id: Option<i64>,
) -> Result<i64> {
    // 1. 用户显式传入
    if let Some(id) = explicit_library_id {
        return Ok(id);
    }

    // 2. config 配置的默认库
    if let Some(id) = config_default_library_id {
        // 验证该库存在且未删除
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM kb_libraries WHERE id = ?1 AND deleted_at IS NULL",
                rusqlite::params![id],
                |row| row.get(0),
            )
            .unwrap_or(false);
        if exists {
            return Ok(id);
        }
    }

    // 3-4. 从 kb_libraries 表查找或创建默认库
    // 先尝试查找名为 "Inbox" 或 "Default" 的库
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM kb_libraries WHERE name IN ('Inbox', 'Default') AND deleted_at IS NULL ORDER BY name LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok();

    if let Some(id) = existing {
        return Ok(id);
    }

    // 没有找到，自动创建 Inbox 库
    let now = now_str();
    conn.execute(
        "INSERT INTO kb_libraries (name, created_at, updated_at)
         VALUES ('Inbox', ?1, ?1)",
        rusqlite::params![now],
    )
    .map_err(|e| GBrainError::Database(format!("自动创建 Inbox 库失败: {}", e)))?;

    let id = conn.last_insert_rowid();
    info!("自动创建默认 Inbox 库: id={}", id);
    Ok(id)
}

/// 手动写入长期记忆（设计文档 §8.6）
///
/// 创建 text/manual artifact 并投影到 gbrain 页面。
/// 内部步骤：
/// 1. 将 title/content 规范化为 markdown 文本
/// 2. 计算 hash
/// 3. 写 artifact store
/// 4. 创建 source artifact 和 occurrence
/// 5. 创建 brain_page_update projection
/// 6. 调用原 gbrain put_page 逻辑
/// 7. 可选入 KB
pub fn put_manual_memory(
    conn: &Connection,
    slug: &str,
    content: &str,
    title: Option<&str>,
    _intent: Option<&str>,
    artifact_dir: &Path,
    _gbrain_dir: &Path,
    config_default_library_id: Option<i64>,
    config_promotion_policy: &str,
    manual_memory_to_kb: bool,
) -> Result<UploadSourceOutput> {
    // 1. 规范化为 markdown
    let page_title = title.unwrap_or(slug);
    let md_content = if content.starts_with("---\n") || content.starts_with("---\r\n") {
        content.to_string()
    } else {
        format!("# {}\n\n{}", page_title, content)
    };
    let content_bytes = md_content.as_bytes();

    // 2. 计算 SHA256
    let sha256 = compute_sha256(content_bytes);

    // 3. 写入 artifact store
    let storage_path = write_artifact_file(content_bytes, &sha256, "md", artifact_dir)?;
    let storage_path_str = storage_path
        .to_str()
        .unwrap_or("unknown")
        .to_string();

    // 4. 创建/复用 source_artifacts 记录
    let now = now_str();
    let existing = store::find_artifact_by_sha256(conn, &sha256)?;
    let (artifact_id, artifact_uid, is_new) = if let Some(existing) = existing {
        debug!("复用已有 artifact: id={}, uid={}", existing.id, existing.artifact_uid);
        (existing.id, existing.artifact_uid.clone(), false)
    } else {
        let artifact_uid = generate_artifact_uid(&sha256);
        let artifact = SourceArtifact {
            id: 0,
            artifact_uid: artifact_uid.clone(),
            created_at: now.clone(),
            updated_at: now.clone(),
            last_seen_at: Some(now.clone()),
            sha256: sha256.clone(),
            original_name: format!("{}.md", slug.replace('/', "-")),
            extension: "md".to_string(),
            mime_type: "text/markdown".to_string(),
            size_bytes: content_bytes.len() as i64,
            storage_path: storage_path_str,
            canonical_slug: slug.to_string(),
            status: "active".to_string(),
            metadata_json: serde_json::json!({
                "source_kind": "manual",
                "input_mode": "content",
                "target_slug": slug,
                "created_by": "artifact_put"
            })
            .to_string(),
            deleted_at: None,
            purged_at: None,
        };
        let id = store::insert_artifact(conn, &artifact)?;
        info!("创建手动 artifact: id={}, uid={}, slug={}", id, artifact_uid, slug);
        (id, artifact_uid, true)
    };

    // 5. 创建 occurrence
    let occurrence_uid = generate_occurrence_uid(&artifact_uid);
    let occurrence = ArtifactOccurrence {
        id: 0,
        occurrence_uid: occurrence_uid.clone(),
        created_at: now.clone(),
        updated_at: now.clone(),
        artifact_id,
        source_kind: "manual".to_string(),
        source_uri: format!("artifact://{}", artifact_uid),
        original_path: slug.to_string(),
        original_name: format!("{}.md", slug.replace('/', "-")),
        owner_ref: "cli".to_string(),
        intent: "memory".to_string(),
        target_slug: slug.to_string(),
        page_slug: slug.to_string(),
        library_id: config_default_library_id,
        folder_id: None,
        promotion_policy: config_promotion_policy.to_string(),
        status: "active".to_string(),
        metadata_json: "{}".to_string(),
    };
    let occurrence_id = store::insert_occurrence(conn, &occurrence)?;

    // 6. 创建 brain_page_update projection
    let proj_key = format!("page_update:{}", slug);
    let proj_ref = format!("brain_page:{}", slug);
    let proj = ArtifactProjection {
        id: 0,
        created_at: now.clone(),
        updated_at: now.clone(),
        artifact_id,
        occurrence_id: Some(occurrence_id),
        projection_type: ProjectionType::BrainPageUpdate.to_string(),
        projection_key: proj_key.clone(),
        projection_ref: proj_ref.clone(),
        status: "active".to_string(),
        version_hash: sha256.clone(),
        stale_reason: String::new(),
        metadata_json: "{}".to_string(),
        superseded_by: None,
    };
    store::insert_projection(conn, &proj)?;

    let mut projections = vec![ProjectionResult {
        projection_type: ProjectionType::BrainPageUpdate,
        projection_key: proj_key,
        projection_ref: proj_ref,
        created: is_new,
        status: "active".to_string(),
    }];

    // 7. 可选入 KB
    if manual_memory_to_kb {
        if let Some(library_id) = config_default_library_id {
            let kb_proj_key = format!("library:{}", library_id);
            let kb_proj_ref = format!("kb_doc:{}", artifact_id);
            let kb_proj = ArtifactProjection {
                id: 0,
                created_at: now.clone(),
                updated_at: now.clone(),
                artifact_id,
                occurrence_id: Some(occurrence_id),
                projection_type: ProjectionType::KbDocument.to_string(),
                projection_key: kb_proj_key.clone(),
                projection_ref: kb_proj_ref.clone(),
                status: "active".to_string(),
                version_hash: sha256.clone(),
                stale_reason: String::new(),
                metadata_json: "{}".to_string(),
                superseded_by: None,
            };
            store::insert_projection(conn, &kb_proj)?;
            projections.push(ProjectionResult {
                projection_type: ProjectionType::KbDocument,
                projection_key: kb_proj_key,
                projection_ref: kb_proj_ref,
                created: is_new,
                status: "active".to_string(),
            });
        }
    }

    // 构建路由计划
    let route_plan = RoutePlan {
        to_kb: manual_memory_to_kb && config_default_library_id.is_some(),
        to_brain: true,
        to_shadow: false,
        to_file: false,
        promotion: PromotionPolicy::AutoAcceptLowRisk,
    };

    Ok(UploadSourceOutput {
        artifact_id,
        artifact_uid,
        occurrence_id,
        occurrence_uid,
        sha256,
        is_new,
        route_plan,
        projections,
    })
}
