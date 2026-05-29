//! gbrain CLI
//! Mirrors gbrain's src/cli.ts

use clap::{Parser, Subcommand};
use gbrain_core::config::Config;
use gbrain_core::engine::BrainEngine;
use gbrain_core::error::{GBrainError, Result};
use gbrain_core::logging;
use gbrain_core::mcp::McpServer;
use gbrain_core::operations::{OpContext, Operations};
use gbrain_core::sqlite_engine::SqliteEngine;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use tracing::{error, info, warn};

#[derive(Parser)]
#[command(name = "gbrain", version, about = "Personal knowledge brain")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Database path
    #[arg(long)]
    db: Option<String>,

    /// Output as JSON
    #[arg(long)]
    json: bool,

    /// Dry-run mode: preview operations before saving
    #[arg(long)]
    dry_run: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// 初始化知识库
    Init,

    /// 运行 MCP stdio 服务器
    Serve,

    /// 配置管理
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },

    // ========================================================================
    // 以下为 Artifact 知识操作命令（原 artifact put/upload/query/list/get/delete 等）
    // ========================================================================
    /// 手动写入长期记忆（原 artifact put）
    Put {
        /// 目标页面 slug
        slug: String,
        /// 页面标题
        #[arg(long)]
        title: Option<String>,
        /// 直接输入内容
        #[arg(long, group = "input")]
        content: Option<String>,
        /// 从文件读取内容
        #[arg(long, group = "input")]
        file: Option<String>,
        /// 意图: memory(默认), evidence, promote
        /// P1-2 修复：改为 Option，未指定时传 None，让 artifact_default_intent 配置生效
        #[arg(long)]
        intent: Option<String>,
        /// 仅预览路由计划
        #[arg(long)]
        dry_run: bool,
        /// 强制覆盖已被人工修改的页面（P1 修复：默认不覆盖人工修改的页面）
        #[arg(long)]
        force: bool,
    },

    /// 上传文件作为知识源（原 artifact upload）
    Upload {
        /// 本地文件路径
        path: String,
        /// 上传意图: auto, evidence, memory, attachment, promote
        #[arg(long, default_value = "auto")]
        intent: String,
        /// 目标 gbrain 页面 slug
        #[arg(long)]
        target: Option<String>,
        /// 关联页面 slug（附件）
        #[arg(long)]
        page: Option<String>,
        /// KB 库 ID
        #[arg(long)]
        library: Option<i64>,
        /// KB 文件夹 ID
        #[arg(long)]
        folder: Option<i64>,
        /// 提升策略: none, shadow, candidate, auto-low-risk
        #[arg(long)]
        promotion: Option<String>,
        /// 仅预览路由计划
        #[arg(long)]
        dry_run: bool,
    },

    /// 统一知识查询（原 artifact query）
    Query {
        /// 查询文本
        query: String,
        /// 查询模式: auto, memory, evidence, timeline
        #[arg(long, default_value = "auto")]
        mode: String,
        /// 最大结果数
        #[arg(long)]
        limit: Option<usize>,
        /// 过滤到指定页面 slug
        #[arg(long)]
        filter: Option<String>,
        /// 显示来源追溯
        #[arg(long)]
        include_sources: bool,
    },

    /// 列出知识源（原 artifact list）
    List {
        /// 最大结果数
        #[arg(long, default_value = "50")]
        limit: i64,
        /// 偏移量
        #[arg(long, default_value = "0")]
        offset: i64,
    },

    /// 获取知识源详情（原 artifact get）
    Get {
        /// Artifact ID 或 UID
        id_or_uid: String,
        /// 包含投影详情
        #[arg(long)]
        include_projections: bool,
        /// 包含来源追溯
        #[arg(long)]
        include_sources: bool,
    },

    /// 软删除知识源（原 artifact delete）
    Delete {
        /// Artifact ID 或 UID
        id_or_uid: String,
        /// 预览删除影响
        #[arg(long)]
        dry_run: bool,
    },

    /// 移除知识源与某次使用的关联（原 artifact detach）
    Detach {
        /// Artifact ID 或 UID
        id_or_uid: String,
        /// 目标页面 slug
        #[arg(long)]
        from: String,
        /// 预览影响
        #[arg(long)]
        dry_run: bool,
    },

    /// 恢复已软删除的知识源（原 artifact restore）
    Restore {
        /// Artifact ID 或 UID
        id_or_uid: String,
        /// 预览恢复影响
        #[arg(long)]
        dry_run: bool,
    },

    /// 重新处理知识源（原 artifact reprocess）
    Reprocess {
        /// Artifact ID 或 UID
        id_or_uid: String,
        /// 预览重新处理影响
        #[arg(long)]
        dry_run: bool,
    },

    /// 检查知识源一致性（原 artifact health）
    Health,

    /// 建议变更操作（原 artifact review）
    Review {
        #[command(subcommand)]
        command: ReviewCommands,
    },

    /// KB 知识库管理
    Kb {
        #[command(subcommand)]
        command: KbCommands,
    },
}

/// 配置管理子命令
#[derive(Subcommand)]
enum ConfigCommand {
    /// 显示所有配置值
    Show,
    /// 获取单个配置值
    Get { key: String },
    /// 设置配置值
    Set { key: String, value: String },
}

/// 建议变更子命令（设计文档 §4.1.5）
#[derive(Debug, clap::Subcommand)]
pub enum ReviewCommands {
    /// 列出建议变更
    List {
        /// 过滤状态: pending, accepted, rejected, applied, rolled_back
        #[arg(long)]
        status: Option<String>,
        /// 过滤目标页面 slug
        #[arg(long)]
        target: Option<String>,
        /// 最大结果数
        #[arg(long, default_value = "50")]
        limit: i64,
    },
    /// 查看建议变更详情
    Show {
        /// 变更 ID
        change_id: i64,
    },
    /// 应用建议变更
    Apply {
        /// 变更 ID
        change_id: i64,
    },
    /// 拒绝建议变更
    Reject {
        /// 变更 ID
        change_id: i64,
        /// 拒绝原因
        #[arg(long)]
        reason: Option<String>,
    },
    /// 回滚已应用的建议变更
    Rollback {
        /// 变更 ID
        change_id: i64,
    },
}

/// KB 知识库管理子命令（Phase 4）
#[derive(Debug, clap::Subcommand)]
pub enum KbCommands {
    /// 查看 KB 文档的 OCR 状态
    OcrStatus {
        /// 文档 ID
        doc_id: i64,
    },
    /// 手动触发 OCR 处理
    OcrRun {
        /// 文档 ID
        doc_id: i64,
        /// 指定要 OCR 的页码（逗号分隔，如 "1,3,5-10"），不指定则自动检测
        #[arg(long)]
        pages: Option<String>,
    },
    /// 重试失败的 OCR 作业
    OcrRetry {
        /// 文档 ID
        doc_id: i64,
    },
}

/// 解析页码范围字符串（如 "1,3,5-10" → vec![1,3,5,6,7,8,9,10]）。
/// 页码为 1-based，必须落在 `1..=max_page` 内。
fn parse_page_ranges(input: &str, max_page: i32) -> Result<Vec<i32>> {
    if max_page <= 0 {
        return Err(GBrainError::InvalidInput(
            "无法确定 PDF 总页数，不能校验 OCR 页码范围".to_string(),
        ));
    }

    let mut pages = Vec::new();
    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return Err(GBrainError::InvalidInput(
                "OCR 页码范围包含空片段".to_string(),
            ));
        }

        let (start, end) = if let Some((left, right)) = part.split_once('-') {
            if right.contains('-') || left.trim().is_empty() || right.trim().is_empty() {
                return Err(GBrainError::InvalidInput(format!(
                    "OCR 页码范围格式无效: {}",
                    part
                )));
            }
            let start = left.trim().parse::<i32>().map_err(|_| {
                GBrainError::InvalidInput(format!("OCR 页码不是有效整数: {}", left.trim()))
            })?;
            let end = right.trim().parse::<i32>().map_err(|_| {
                GBrainError::InvalidInput(format!("OCR 页码不是有效整数: {}", right.trim()))
            })?;
            (start, end)
        } else {
            let page = part.parse::<i32>().map_err(|_| {
                GBrainError::InvalidInput(format!("OCR 页码不是有效整数: {}", part))
            })?;
            (page, page)
        };

        if start < 1 || end < 1 {
            return Err(GBrainError::InvalidInput(format!(
                "OCR 页码必须从 1 开始: {}",
                part
            )));
        }
        if start > end {
            return Err(GBrainError::InvalidInput(format!(
                "OCR 页码范围起始页大于结束页: {}",
                part
            )));
        }
        if end > max_page {
            return Err(GBrainError::InvalidInput(format!(
                "OCR 页码范围超出 PDF 总页数 {}: {}",
                max_page, part
            )));
        }

        for p in start..=end {
            if !pages.contains(&p) {
                pages.push(p);
            }
        }
    }

    if pages.is_empty() {
        return Err(GBrainError::InvalidInput("OCR 页码范围为空".to_string()));
    }

    pages.sort();
    Ok(pages)
}

fn resolve_pdf_page_count(
    conn: &rusqlite::Connection,
    doc_id: i64,
    storage_path: &str,
) -> Result<i32> {
    let total: i32 = conn
        .query_row(
            "SELECT page_count FROM kb_documents WHERE id = ?1",
            rusqlite::params![doc_id],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if total > 0 {
        return Ok(total);
    }

    let data = std::fs::read(storage_path)
        .map_err(|e| GBrainError::FileError(format!("读取 PDF 文件以确定页数失败: {}", e)))?;
    let pdf_total = gbrain_core::kb::parser::pdf::count_pdf_pages(&data)? as i32;
    if pdf_total <= 0 {
        return Err(GBrainError::InvalidInput("无法确定 PDF 总页数".to_string()));
    }
    Ok(pdf_total)
}

fn pdf_page_analyses_from_metadata(
    parsed: &gbrain_core::kb::parser::ParsedDocument,
) -> Result<Vec<gbrain_core::kb::ocr_detector::PdfPageAnalysis>> {
    let page_analyses_raw = parsed
        .metadata
        .get("page_analyses")
        .and_then(|v| serde_json::from_str::<Vec<serde_json::Value>>(v).ok())
        .unwrap_or_default();

    if page_analyses_raw.is_empty() {
        return Err(GBrainError::InvalidInput(
            "无法读取 PDF 页级分析，不能自动检测 OCR 页码".to_string(),
        ));
    }

    Ok(page_analyses_raw
        .iter()
        .map(|pa| {
            let page_number = pa.get("page_number").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let text = pa
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let char_count = pa.get("char_count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let image_area_ratio = pa
                .get("image_area_ratio")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let image_count = pa.get("image_count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let has_vector = pa
                .get("has_vector_or_unknown_objects")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let image_regions = if image_count > 0 {
                (0..image_count)
                    .map(|_| gbrain_core::kb::ocr_detector::PdfImageRegion {
                        bbox: None,
                        area_ratio: image_area_ratio / image_count as f64,
                    })
                    .collect()
            } else if image_area_ratio > 0.0 {
                vec![gbrain_core::kb::ocr_detector::PdfImageRegion {
                    bbox: None,
                    area_ratio: image_area_ratio,
                }]
            } else {
                vec![]
            };

            gbrain_core::kb::ocr_detector::PdfPageAnalysis {
                page_number,
                text,
                text_blocks: vec![],
                char_count,
                image_regions,
                image_area_ratio,
                has_vector_or_unknown_objects: has_vector,
                width: pa.get("width").and_then(|v| v.as_u64()).map(|v| v as u32),
                height: pa.get("height").and_then(|v| v.as_u64()).map(|v| v as u32),
                content_parse_failed: pa
                    .get("content_parse_failed")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                has_vector_drawing_ops: pa
                    .get("has_vector_drawing_ops")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                has_invisible_text: pa
                    .get("has_invisible_text")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                font_encoding_suspected: pa
                    .get("font_encoding_suspected")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            }
        })
        .collect())
}

fn detect_ocr_pages_for_pdf(
    storage_path: &str,
    config: &Config,
) -> Result<gbrain_core::kb::ocr_detector::OcrDetection> {
    let pdf_data = std::fs::read(storage_path).map_err(|e| {
        GBrainError::FileError(format!("读取 PDF 文件以自动检测 OCR 页失败: {}", e))
    })?;
    let registry = gbrain_core::kb::parser::ParserRegistry::new();
    let parsed = registry.parse("pdf", &pdf_data)?;
    let page_analyses = pdf_page_analyses_from_metadata(&parsed)?;
    let ocr_mode = gbrain_core::kb::ocr_provider::OcrMode::from_str(&config.ocr_mode);
    let detection = gbrain_core::kb::ocr_detector::detect_ocr_pages(
        &page_analyses,
        config.ocr_text_density_threshold,
        config.ocr_image_area_threshold,
        config.ocr_image_count_threshold,
        config.ocr_min_low_density_ratio,
        &ocr_mode,
    );
    Ok(detection)
}

fn main() {
    let cli = Cli::parse();

    // 从配置初始化日志
    let mut config = Config::load().unwrap_or_default();
    logging::init(&config);

    if let Err(e) = run(cli, &mut config) {
        error!("Fatal error: {}", e);
        std::process::exit(1);
    }
}

fn run(cli: Cli, config: &mut Config) -> Result<()> {
    let db_path = cli
        .db
        .unwrap_or_else(|| config.db_path().to_str().unwrap_or("brain.db").to_string());

    // 修复：当 --db 覆盖了 DB 路径时，同步到 config，使 artifact_dir()
    // 等基于 db_path 推导的目录与实际 DB 路径一致，避免 DB 写到 X 但
    // artifact 写到默认配置库旁边
    if config.database_path.as_ref() != Some(&db_path) {
        config.database_path = Some(db_path.clone());
    }

    // --dry-run 时不应创建/初始化数据库。
    // 在此提前处理不需要 DB 的命令，避免 connect() 的 Connection::open 创建 DB 文件。

    // ---------- Init dry_run（不需要 DB）----------
    if let Commands::Init = &cli.command {
        if cli.dry_run {
            info!("Dry-run: 将初始化知识库到 {}", db_path);
            info!("Dry-run: 将复制可执行文件到 ~/.gbrain/bin/");
            return Ok(());
        }
        // 显式创建 DB 父目录，避免默认日志路径不创建 ~/.gbrain/ 时
        // Connection::open 因父目录不存在而失败
        if let Some(parent) = PathBuf::from(&db_path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        // 非 dry-run init 将落入下方的数据库连接和 init_schema 流程
    }

    // ---------- Upload dry_run 预览（需要文件校验但不需要 DB）----------
    if let Commands::Upload {
        path,
        intent,
        target: _,
        page: _,
        library: _,
        folder: _,
        promotion,
        dry_run,
    } = &cli.command
    {
        let effective_dry_run = cli.dry_run || *dry_run;
        if effective_dry_run {
            let file_path = PathBuf::from(path);

            let ext_for_route = file_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            // 使用 FromStr 严格解析，拼写错误直接报错而非静默退回 Auto
            let upload_intent: gbrain_core::artifact::types::UploadIntent =
                intent.parse().unwrap_or_else(|e| {
                    error!("{}", e);
                    std::process::exit(1);
                });

            // 推断路由计划
            let route_plan =
                gbrain_core::artifact::types::infer_route_plan(&ext_for_route, "", &upload_intent);

            // 根据路由决定允许的扩展名（与真实上传路径一致）
            let mut allowed_extensions: Vec<String> = config.kb_allowed_extensions.clone();
            if route_plan.to_file {
                for extra in [
                    "png", "jpg", "jpeg", "gif", "bmp", "svg", "webp", "avif", "ico", "tiff",
                    "tif", "zip", "tar", "gz", "json", "xml", "yaml", "yml", "toml",
                ] {
                    let s = extra.to_string();
                    if !allowed_extensions.contains(&s) {
                        allowed_extensions.push(s);
                    }
                }
            }
            let max_file_bytes = config.kb_max_file_size_mb * 1024 * 1024;

            // 验证文件：路径安全、大小、扩展名（与真实上传路径共享校验逻辑）
            let validated_path = gbrain_core::kb::security::validate_upload_source(
                &file_path,
                false,
                &gbrain_core::config::Config::base_dir(),
                max_file_bytes,
                &allowed_extensions,
            )?;

            // 读取文件内容并校验 MIME 类型
            let file_content = std::fs::read(&validated_path)?;
            let ext = validated_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            let _mime = gbrain_core::kb::security::detect_and_validate_mime(&file_content, &ext)?;

            // 使用 FromStr 严格解析，未知值直接报错
            let promotion_policy = promotion.as_ref().map(|p| {
                p.parse::<gbrain_core::artifact::types::PromotionPolicy>()
                    .unwrap_or_else(|e| {
                        error!("{}", e);
                        std::process::exit(1);
                    })
            });

            // 应用 promotion 策略
            let route_plan = gbrain_core::artifact::types::apply_promotion_policy(
                route_plan,
                &promotion_policy,
                &config.upload_default_promotion_policy,
            );

            // 输出预览（支持 --json）
            if cli.json {
                let real_sha256 = {
                    let mut hasher = Sha256::new();
                    hasher.update(&file_content);
                    format!("{:x}", hasher.finalize())
                };
                let preview = gbrain_core::artifact::types::UploadSourceOutput {
                    artifact_id: 0,
                    artifact_uid: "dry-run".to_string(),
                    occurrence_id: 0,
                    occurrence_uid: "dry-run".to_string(),
                    sha256: real_sha256,
                    is_new: true,
                    route_plan,
                    projections: vec![],
                };
                info!("{}", serde_json::to_string_pretty(&preview)?);
            } else {
                info!("Dry-run 上传预览:");
                info!("  文件: {}", file_path.display());
                info!("  意图: {}", intent);
                info!(
                    "  Route: KB={}, Brain={}, File={}, Shadow={}",
                    route_plan.to_kb, route_plan.to_brain, route_plan.to_file, route_plan.to_shadow
                );
                info!("  Promotion: {}", route_plan.promotion);
            }
            return Ok(());
        }
    }

    // ---------- Config 命令预处理：config.json 型 key 不需要数据库 ----------
    // 对于 Config 中的字段，直接在 config.json 读写，避免 fresh install 时
    // 纯配置操作因 ~/.gbrain/ 父目录不存在或 DB 未初始化而失败
    if let Commands::Config { command } = &cli.command {
        match command {
            ConfigCommand::Set { key, value } => {
                // SQLite engine 专用 key 需要 DB 连接，继续走 DB 路径
                if key != "writer.lint_on_put_page" {
                    match config.apply_set(key, value) {
                        Ok(()) => {
                            // Config 字段，直接保存到 config.json，不需要数据库
                            if !cli.dry_run {
                                // 确保 ~/.gbrain/ 目录存在
                                let _ = std::fs::create_dir_all(Config::base_dir());
                                config.save().map_err(|e| {
                                    GBrainError::Config(format!("保存 config.json 失败: {}", e))
                                })?;
                            }
                            info!("{} = {}", key, value);
                            return Ok(());
                        }
                        Err(msg) => {
                            // 未知 key 或无效值，直接报错退出
                            error!("配置错误: {}", msg);
                            std::process::exit(1);
                        }
                    }
                }
                // SQLite engine 专用 key（writer.lint_on_put_page），
                // 继续走 DB 连接路径
            }
            ConfigCommand::Get { key } => {
                if let Some(val) = config.get_field(key) {
                    info!("{}", val);
                    return Ok(());
                }
                // 仅 writer.lint_on_put_page 是已知的 SQLite engine 专用 key，
                // 需要继续走 DB 连接；其余未知 key 直接报错
                if *key != "writer.lint_on_put_page" {
                    error!("未知配置 key: {}。使用 config show 查看可用 key。", key);
                    std::process::exit(1);
                }
                // SQLite engine 专用 key，继续走 DB 连接路径
            }
            ConfigCommand::Show => {
                // 显示 config.json 中的常用字段后直接返回，
                // 不再落入 DB 连接路径，避免 fresh install 下创建数据库
                for key in &[
                    "embedding_model",
                    "embedding_dimensions",
                    "expansion_model",
                    "chunker_model",
                    "chunk_size",
                    "chunk_overlap",
                    "auto_link",
                    "auto_timeline",
                    "post_write_lint",
                    "upload_default_promotion_policy",
                    "artifact_default_intent",
                    "kb_max_file_size_mb",
                    "kb_worker_enabled",
                    "autopilot_enabled",
                    "autopilot_interval_secs",
                ] {
                    if let Some(val) = config.get_field(key) {
                        info!("{} = {}", key, val);
                    }
                }
                return Ok(());
            }
        }
    }

    // ---------- --dry-run 时不需要数据库的命令 ----------
    // 合并全局 --dry-run 和子命令级 --dry-run（如 put/delete/detach/restore/reprocess --dry-run）
    // 避免子命令级 dry_run 漏过检查，导致 engine.connect() 创建 DB 文件
    let any_dry_run = cli.dry_run
        || match &cli.command {
            Commands::Put { dry_run, .. } => *dry_run,
            Commands::Delete { dry_run, .. } => *dry_run,
            Commands::Detach { dry_run, .. } => *dry_run,
            Commands::Restore { dry_run, .. } => *dry_run,
            Commands::Reprocess { dry_run, .. } => *dry_run,
            _ => false,
        };
    // 这些命令的 dry-run 路径只是打印预览信息，无需访问数据库
    if any_dry_run {
        match &cli.command {
            Commands::Config {
                command: ConfigCommand::Set { key, value },
            } => {
                info!("Dry-run: 将设置 {} = {}", key, value);
                return Ok(());
            }
            Commands::Review {
                command: ReviewCommands::Apply { change_id },
            } => {
                info!("Dry-run: 将应用建议变更 {}", change_id);
                return Ok(());
            }
            Commands::Review {
                command: ReviewCommands::Reject { change_id, .. },
            } => {
                info!("Dry-run: 将拒绝建议变更 {}", change_id);
                return Ok(());
            }
            Commands::Review {
                command: ReviewCommands::Rollback { change_id },
            } => {
                info!("Dry-run: 将回滚建议变更 {}", change_id);
                return Ok(());
            }
            _ => {
                // 其余命令的 dry-run 需要 DB 才能生成预览（如 delete 需查询 artifact），
                // 但 dry-run 不应创建或初始化数据库
                let db_path_buf = PathBuf::from(&db_path);
                if !db_path_buf.exists() {
                    // 数据库尚不存在，无可预览内容
                    info!("Dry-run: 数据库 {} 尚不存在，无操作可预览", db_path);
                    return Ok(());
                }
                // 数据库已存在，落入下方连接流程（将跳过 init_schema）
            }
        }
    }

    // ---------- 需要数据库的命令 ----------
    let mut engine = SqliteEngine::new(PathBuf::from(db_path.clone()).as_path());
    // dry-run 时使用只读连接，跳过 journal_mode=WAL 等写入型 PRAGMA，避免修改数据库状态
    if any_dry_run {
        engine.connect_readonly()?;
    } else {
        info!(db_path = %db_path, "Connecting to brain database");
        engine.connect()?;
        engine.init_schema()?;
    }

    let mut ctx = OpContext::default();
    if any_dry_run {
        ctx.dry_run = true;
        info!("Dry-run mode enabled — no changes will be committed");
    }
    let ops = Operations::with_config(&engine, ctx, config.clone());

    match cli.command {
        Commands::Init => {
            // 将当前可执行文件复制到 ~/.gbrain/bin/
            let bin_dir = Config::base_dir().join("bin");
            std::fs::create_dir_all(&bin_dir)?;
            let current_exe = std::env::current_exe()?;
            let exe_name = current_exe
                .file_name()
                .unwrap_or(std::ffi::OsStr::new("gbrain"));
            let dest = bin_dir.join(exe_name);
            if current_exe != dest {
                std::fs::copy(&current_exe, &dest)?;
                info!(
                    src = %current_exe.display(),
                    dest = %dest.display(),
                    "Copied executable to bin directory"
                );
            }
            info!(db_path = %db_path, "Brain initialized");
        }

        Commands::Serve => {
            info!("Starting MCP stdio server");
            // 当 KB 子系统启用且 worker 启用时，在后台启动 KB worker 线程
            if config.kb_enabled && config.kb_worker_enabled {
                let kb_db_path = PathBuf::from(db_path.clone());
                gbrain_core::kb::spawn_kb_worker_thread(
                    kb_db_path,
                    config.clone(),
                    config.kb_worker_poll_interval_secs,
                );
                info!("KB worker: 后台线程已随 MCP 服务启动");
            }
            // 后台启动 autopilot 维护线程（嵌入过期内容 + 完整性检查 + 健康报告）
            if config.autopilot_enabled {
                gbrain_core::autopilot::spawn_autopilot_thread(
                    PathBuf::from(db_path.clone()),
                    config.clone(),
                    config.autopilot_interval_secs,
                );
                info!("Autopilot: 后台线程已随 MCP 服务启动");
            }
            let mut server = McpServer::with_config(engine, config.clone());
            server.run()?;
            return Ok(());
        }

        Commands::Config { command } => match command {
            ConfigCommand::Get { key } => {
                // Config 字段已在 DB 连接前返回，此处只处理 SQLite engine 专用 key
                match ops.engine.get_config(&key)? {
                    Some(val) => info!("{}", val),
                    None => info!("(not set)"),
                }
            }
            ConfigCommand::Set { key, value } => {
                // Config 字段已在 DB 连接前通过 apply_set+save 处理并返回，
                // 此处只处理 SQLite engine 专用 key（如 writer.lint_on_put_page）
                ops.engine.set_config(&key, &value)?;
                info!("{} = {}", key, value);
            }
            // Show 已在 DB 连接前返回，不再落入此处
            ConfigCommand::Show => unreachable!("Show 已在 DB 连接前处理"),
        },

        // ========================================================================
        // Artifact 知识操作（原 artifact put/upload/query/list/get/delete 等）
        // ========================================================================
        Commands::Put {
            slug,
            title,
            content,
            file,
            intent,
            dry_run,
            force,
        } => {
            // 读取内容：优先从文件读取，否则使用直接输入
            let page_content = if let Some(ref path) = file {
                let file_path = std::path::PathBuf::from(path);
                // P2-12 修复：artifact_put --file 使用与 put_memory 相同的 1MB 大小上限
                // 和文本文件专用扩展名白名单，而非 KB 上传的 50MB 上限和 KB 扩展名列表。
                // 之前使用 kb_max_file_size_mb（默认 50MB），导致 1MB~50MB 的文本文件
                // 先完整读入，再在 service 层被拒绝。
                let allowed_extensions: Vec<String> =
                    gbrain_core::artifact::service::TEXT_FILE_EXTENSIONS
                        .iter()
                        .map(|s| s.to_string())
                        .collect();
                let max_file_bytes = gbrain_core::artifact::service::MAX_PUT_MEMORY_CONTENT_BYTES;
                let validated_path = gbrain_core::kb::security::validate_upload_source(
                    &file_path,
                    false,
                    &ops.ctx.working_dir,
                    max_file_bytes,
                    &allowed_extensions,
                )?;
                let ext = validated_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                let raw_content = std::fs::read(&validated_path)?;
                let _mime =
                    gbrain_core::kb::security::detect_and_validate_mime(&raw_content, &ext)?;
                // artifact_put 只接受文本内容，需要转为 String
                String::from_utf8(raw_content).map_err(|e| {
                    gbrain_core::error::GBrainError::InvalidInput(format!(
                        "文件内容不是有效 UTF-8 文本: {}",
                        e
                    ))
                })?
            } else {
                content.unwrap_or_default()
            };

            // 安全校验：内容不能为空
            if page_content.is_empty() {
                error!("内容不能为空，请使用 --content 或 --file 参数");
                std::process::exit(1);
            }

            // 委托给 ArtifactService.put_memory
            // P1-2 修复：intent 改为 Option，未指定时传 None，
            // 让 artifact_default_intent 配置生效
            let svc = ops.artifact_service();
            // P1修复：统一全局 --dry-run 与子命令 dry_run
            let effective_dry_run = cli.dry_run || dry_run;
            let result = svc.put_memory(
                &slug,
                &page_content,
                title.as_deref(),
                intent.as_deref(),
                effective_dry_run,
                force,
            )?;

            if cli.json {
                info!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                if effective_dry_run {
                    info!(
                        "Artifact put 预览: {}",
                        serde_json::to_string_pretty(&result)?
                    );
                } else {
                    info!(
                        "Artifact put 完成: slug={}, 结果={}",
                        slug,
                        serde_json::to_string_pretty(&result)?
                    );
                }
            }
        }

        Commands::Upload {
            path,
            intent,
            target,
            page,
            library,
            folder,
            promotion,
            dry_run,
        } => {
            let file_path = PathBuf::from(&path);

            // 安全校验：复用 MCP 的 validate_upload_source
            // 使用 FromStr 严格解析，拼写错误直接报错
            let upload_intent: gbrain_core::artifact::types::UploadIntent =
                intent.parse().unwrap_or_else(|e| {
                    error!("{}", e);
                    std::process::exit(1);
                });

            let ext_for_route = file_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            let route_plan =
                gbrain_core::artifact::types::infer_route_plan(&ext_for_route, "", &upload_intent);

            // 根据路由决定允许的扩展名
            let mut allowed_extensions: Vec<String> = config.kb_allowed_extensions.clone();
            if route_plan.to_file {
                // P2修复：补齐 avif/ico/tiff/tif，与 IMAGE_EXTENSIONS 和 MCP upload 白名单保持一致
                for extra in [
                    "png", "jpg", "jpeg", "gif", "bmp", "svg", "webp", "avif", "ico", "tiff",
                    "tif", "zip", "tar", "gz", "json", "xml", "yaml", "yml", "toml",
                ] {
                    let s = extra.to_string();
                    if !allowed_extensions.contains(&s) {
                        allowed_extensions.push(s);
                    }
                }
            }
            let max_file_bytes = config.kb_max_file_size_mb * 1024 * 1024;

            let validated_path = gbrain_core::kb::security::validate_upload_source(
                &file_path,
                false,
                &gbrain_core::config::Config::base_dir(),
                max_file_bytes,
                &allowed_extensions,
            )?;

            let file_content = std::fs::read(&validated_path)?;
            let ext = validated_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            let _mime = gbrain_core::kb::security::detect_and_validate_mime(&file_content, &ext)?;

            let original_name = validated_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            let promotion_policy =
                // 使用 FromStr 严格解析，未知值直接报错
                promotion.as_ref().map(|p| {
                    p.parse::<gbrain_core::artifact::types::PromotionPolicy>()
                        .unwrap_or_else(|e| {
                            error!("{}", e);
                            std::process::exit(1);
                        })
                });

            // P1修复：统一全局 --dry-run 与子命令 dry_run
            let effective_dry_run = cli.dry_run || dry_run;
            let input = gbrain_core::artifact::types::UploadSourceInput {
                content: file_content,
                path: Some(file_path.clone()),
                original_name,
                source_kind: gbrain_core::artifact::types::SourceKind::Upload,
                source_uri: file_path.to_string_lossy().to_string(),
                intent: upload_intent,
                target_slug: target,
                page_slug: page,
                library_id: library,
                folder_id: folder,
                promotion_policy,
                owner_ref: None,
                metadata: None,
                dry_run: effective_dry_run,
            };

            let svc = ops.artifact_service();
            let result = svc.upload_file(input)?;
            if cli.json {
                info!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                info!(
                    "Artifact: {} (uid={}, sha256={})",
                    result.artifact_id, result.artifact_uid, result.sha256
                );
                info!(
                    "Route: KB={}, Brain={}, File={}, Shadow={}",
                    result.route_plan.to_kb,
                    result.route_plan.to_brain,
                    result.route_plan.to_file,
                    result.route_plan.to_shadow
                );
                info!("Promotion: {}", result.route_plan.promotion);
            }
        }

        Commands::Query {
            query,
            mode,
            limit,
            filter,
            include_sources,
        } => {
            // 委托给 ArtifactService.query_facade
            let svc = ops.artifact_service();
            let input = gbrain_core::artifact::types::ArtifactQueryInput {
                query,
                mode: Some(mode),
                limit,
                filter_slug: filter,
                include_sources: Some(include_sources),
            };
            let result = svc.query_facade(&input)?;
            if cli.json {
                info!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                info!("模式: {} | 总命中: {}", result.mode, result.meta.total);
                for m in &result.memories {
                    info!("  记忆: {} | {} | {:.3}", m.slug, m.title, m.score);
                }
                for e in &result.evidence {
                    info!("  证据: {} | {:.3}", e.title, e.score);
                }
                for t in &result.timeline {
                    info!(
                        "  时间线: {} | {} | {}",
                        t.timestamp,
                        t.description,
                        t.slug.as_deref().unwrap_or("")
                    );
                }
            }
        }

        Commands::List { limit, offset } => {
            let svc = ops.artifact_service();
            let artifacts = svc.list_artifacts(limit, offset)?;
            if cli.json {
                info!("{}", serde_json::to_string_pretty(&artifacts)?);
            } else {
                for a in &artifacts {
                    info!(
                        "  [{}] {} uid={} size={} status={}",
                        a.slug,
                        a.original_name.as_deref().unwrap_or("-"),
                        a.uid,
                        a.size_bytes
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                        a.status
                    );
                }
                info!("{} 知识源", artifacts.len());
            }
        }

        Commands::Get {
            id_or_uid,
            include_projections,
            include_sources,
        } => {
            let svc = ops.artifact_service();
            let detail =
                svc.get_artifact_detail(&id_or_uid, include_projections, include_sources, false)?;
            match detail {
                Some(d) => {
                    info!("{}", serde_json::to_string_pretty(&d)?);
                }
                None => warn!("知识源 '{}' 未找到", id_or_uid),
            }
        }

        Commands::Delete { id_or_uid, dry_run } => {
            // P1修复：统一全局 --dry-run 与子命令 dry_run
            let effective_dry_run = cli.dry_run || dry_run;
            let svc = ops.artifact_service();

            if effective_dry_run {
                let preview = svc.delete_artifact_dry_run(&id_or_uid)?;
                info!("{}", serde_json::to_string_pretty(&preview)?);
            } else {
                let artifact_id = svc.resolve_artifact_id(&id_or_uid)?;
                svc.delete_artifact(artifact_id)?;
                info!("知识源 {} 已软删除", id_or_uid);
            }
        }

        Commands::Detach {
            id_or_uid,
            from,
            dry_run,
        } => {
            // P1修复：统一全局 --dry-run 与子命令 dry_run
            let effective_dry_run = cli.dry_run || dry_run;
            // 委托给 ArtifactService.detach
            let svc = ops.artifact_service();
            let result = svc.detach(&id_or_uid, &from, effective_dry_run)?;
            if cli.json {
                info!("{}", serde_json::to_string_pretty(&result)?);
            } else if effective_dry_run {
                info!("预览: {}", result["description"]);
            } else {
                info!(
                    "已解除关联: artifact_id={} from_slug={} detached={}",
                    result["artifact_id"], result["from_slug"], result["detached_occurrences"]
                );
            }
        }

        Commands::Restore { id_or_uid, dry_run } => {
            // P1修复：统一全局 --dry-run 与子命令 dry_run
            let effective_dry_run = cli.dry_run || dry_run;
            // 委托给 ArtifactService.restore
            let svc = ops.artifact_service();
            let result = svc.restore(&id_or_uid, effective_dry_run)?;
            if cli.json {
                info!("{}", serde_json::to_string_pretty(&result)?);
            } else if effective_dry_run {
                info!("预览: {}", result["description"]);
            } else {
                info!(
                    "已恢复: artifact_id={} occurrences={} projections={}",
                    result["artifact_id"],
                    result["restored_occurrences"],
                    result["restored_projections"]
                );
            }
        }

        Commands::Reprocess { id_or_uid, dry_run } => {
            // P1修复：统一全局 --dry-run 与子命令 dry_run
            let effective_dry_run = cli.dry_run || dry_run;
            // 委托给 ArtifactService.reprocess
            let svc = ops.artifact_service();
            let result = svc.reprocess(&id_or_uid, effective_dry_run)?;
            if cli.json {
                info!("{}", serde_json::to_string_pretty(&result)?);
            } else if effective_dry_run {
                info!("预览: {}", result["description"]);
            } else {
                info!(
                    "已请求重新处理: artifact_id={} stale_projections={} status={}",
                    result["artifact_id"], result["stale_projections"], result["status"]
                );
            }
        }

        Commands::Health => {
            // 委托给 ArtifactService.health_check
            let svc = ops.artifact_service();
            let report = svc.health_check()?;
            info!("{}", serde_json::to_string_pretty(&report)?);
        }

        Commands::Review { command } => match command {
            ReviewCommands::List {
                status,
                target,
                limit,
            } => {
                // 委托给 ArtifactService.list_suggested_changes
                let svc = ops.artifact_service();
                let items =
                    svc.list_suggested_changes(status.as_deref(), target.as_deref(), limit, 0)?;
                if cli.json {
                    info!("{}", serde_json::to_string_pretty(&items)?);
                } else {
                    for item in &items {
                        info!(
                            "  [{}] {} target={} risk={} summary={}",
                            item.status,
                            item.change_id,
                            item.target_slug,
                            item.risk_level,
                            item.summary
                        );
                    }
                    info!("{} 建议变更", items.len());
                }
            }
            ReviewCommands::Show { change_id } => {
                // 委托给 ArtifactService.get_suggested_change
                let svc = ops.artifact_service();
                match svc.get_suggested_change(change_id)? {
                    Some(item) => info!("{}", serde_json::to_string_pretty(&item)?),
                    None => warn!("建议变更 {} 未找到", change_id),
                }
            }
            ReviewCommands::Apply { change_id } => {
                // 全局 --dry-run 已在 DB 连接前提前返回，此处不再需要检查
                let svc = ops.artifact_service();
                let result = svc.apply_suggested_change(change_id)?;
                info!("建议变更 {} 已应用", result.change_id);
            }
            ReviewCommands::Reject { change_id, reason } => {
                // 全局 --dry-run 已在 DB 连接前提前返回，此处不再需要检查
                let svc = ops.artifact_service();
                let input = gbrain_core::artifact::types::ReviewCandidateInput {
                    candidate_id: change_id,
                    action: "reject".to_string(),
                    reviewer: "cli".to_string(),
                    notes: reason,
                };
                let result = svc.reject_suggested_change(input)?;
                info!("建议变更 {} 已拒绝", result.change_id);
            }
            ReviewCommands::Rollback { change_id } => {
                // 全局 --dry-run 已在 DB 连接前提前返回，此处不再需要检查
                let svc = ops.artifact_service();
                let result = svc.rollback_suggested_change(change_id)?;
                info!("建议变更 {} 已回滚 (原状态: {})", change_id, result.status);
            }
        },

        Commands::Kb { command } => match command {
            KbCommands::OcrStatus { doc_id } => {
                let conn = ops.engine.connection()?;
                let kb = gbrain_core::kb::engine::KbEngine::new(conn);

                // 查询文档信息
                let doc = kb.get_document(doc_id)?;

                let mut result = serde_json::json!({
                    "document_id": doc.id,
                    "title": doc.title,
                    "ocr_status": doc.ocr_status,
                    "ocr_text_coverage": doc.ocr_text_coverage,
                });

                // 查询页级 OCR 状态（按当前 run 过滤，避免展示旧 run 数据）
                let conn = ops.engine.connection()?;
                let mut stmt = conn.prepare(
                    "SELECT page_number, status, provider, model, confidence, error \
                     FROM kb_document_ocr_pages WHERE document_id = ?1 AND processing_run_id = ?2 ORDER BY page_number",
                )?;
                let pages: Vec<serde_json::Value> = stmt
                    .query_map(rusqlite::params![doc_id, &doc.processing_run_id], |row| {
                        let page_number: i32 = row.get(0)?;
                        let status: String = row.get(1)?;
                        let provider: String = row.get(2)?;
                        let model: String = row.get(3)?;
                        let confidence: Option<f64> = row.get(4)?;
                        let error: Option<String> = row.get(5)?;
                        Ok(serde_json::json!({
                            "page_number": page_number,
                            "status": status,
                            "provider": provider,
                            "model": model,
                            "confidence": confidence,
                            "error": error,
                        }))
                    })?
                    .filter_map(|r| r.ok())
                    .collect();

                let failed_pages: Vec<i64> = pages
                    .iter()
                    .filter(|page| page["status"].as_str() == Some("failed"))
                    .filter_map(|page| page["page_number"].as_i64())
                    .collect();
                result["ocr_pages"] = serde_json::json!(pages);
                result["ocr_failed_pages"] = serde_json::json!(failed_pages);

                let mut stmt = conn.prepare(
                    "SELECT label, COUNT(*) FROM kb_document_ocr_blocks \
                     WHERE document_id = ?1 AND processing_run_id = ?2 GROUP BY label ORDER BY label",
                )?;
                let block_counts: serde_json::Map<String, serde_json::Value> = stmt
                    .query_map(rusqlite::params![doc_id, &doc.processing_run_id], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                    })?
                    .filter_map(|row| row.ok())
                    .map(|(label, count)| (label, serde_json::json!(count)))
                    .collect();
                result["ocr_block_counts"] = serde_json::Value::Object(block_counts);

                let reasons_by_page: serde_json::Value = conn
                    .query_row(
                        "SELECT json_extract(COALESCE(metadata_json, '{}'), '$.ocr_reasons_by_page') \
                         FROM kb_documents WHERE id = ?1",
                        rusqlite::params![doc_id],
                        |row| {
                            let value: String = row.get(0)?;
                            Ok(serde_json::from_str(&value).unwrap_or(serde_json::Value::Null))
                        },
                    )
                    .unwrap_or(serde_json::Value::Null);
                if !reasons_by_page.is_null() {
                    result["ocr_reasons_by_page"] = reasons_by_page;
                }

                info!("{}", serde_json::to_string_pretty(&result)?);
            }

            KbCommands::OcrRun { doc_id, pages } => {
                let conn = ops.engine.connection()?;

                // 查询文档信息
                let doc_row: (String, String, i64, String, String) = conn
                    .query_row(
                        "SELECT title, storage_path, library_id, processing_run_id, extension \
                         FROM kb_documents WHERE id = ?1",
                        rusqlite::params![doc_id],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, i64>(2)?,
                                row.get::<_, String>(3)?,
                                row.get::<_, String>(4)?,
                            ))
                        },
                    )
                    .map_err(|e| {
                        gbrain_core::error::GBrainError::Database(format!("查询文档失败: {}", e))
                    })?;

                let (title, storage_path, library_id, run_id, extension) = doc_row;
                let is_image =
                    gbrain_core::artifact::types::is_ocr_image_file(&extension.to_lowercase());

                // 检查库隐私策略
                {
                    let kb = gbrain_core::kb::engine::KbEngine::new(conn);
                    let library = kb.get_library(library_id).map_err(|e| {
                        gbrain_core::error::GBrainError::Database(format!("查询库失败: {}", e))
                    })?;
                    if !library.external_ocr_allowed {
                        return Err(gbrain_core::error::GBrainError::InvalidInput(
                            "库已关闭外部 OCR，无法执行 OCR".to_string(),
                        ));
                    }
                    if library.redaction_enabled {
                        return Err(gbrain_core::error::GBrainError::InvalidInput(
                            "库已启用脱敏，禁止外部 OCR".to_string(),
                        ));
                    }
                }

                let total_pages = if is_image {
                    // 图片文档固定 1 页，不需要 PDF 解析器
                    1
                } else {
                    resolve_pdf_page_count(conn, doc_id, &storage_path)?
                };
                let config_local = gbrain_core::config::Config::load().unwrap_or_default();

                // 解析页码：区分显式指定与自动检测
                let (ocr_pages, _explicit_pages, detection_reasons) = if is_image {
                    if let Some(ref pages_str_val) = pages {
                        // 图片文档：用户显式传入页码时，仍校验（max_page=1，非法页码会报错）
                        let parsed_pages = parse_page_ranges(pages_str_val, 1)?;
                        let reasons: std::collections::BTreeMap<String, Vec<String>> = parsed_pages
                            .iter()
                            .map(|p| (p.to_string(), vec!["image_input".to_string()]))
                            .collect();
                        (parsed_pages, true, reasons)
                    } else {
                        // 图片文档未指定页码：默认第 1 页
                        let reasons: std::collections::BTreeMap<String, Vec<String>> =
                            std::iter::once(("1".to_string(), vec!["image_input".to_string()]))
                                .collect();
                        (vec![1], true, reasons)
                    }
                } else if let Some(ref pages_str) = pages {
                    let parsed_pages = parse_page_ranges(pages_str, total_pages)?;
                    // 显式指定：原因统一为 manual_requested
                    let reasons: std::collections::BTreeMap<String, Vec<String>> = parsed_pages
                        .iter()
                        .map(|p| (p.to_string(), vec!["manual_requested".to_string()]))
                        .collect();
                    (parsed_pages, true, reasons)
                } else {
                    // 自动检测：使用 detector 返回的真实原因
                    let detection = detect_ocr_pages_for_pdf(&storage_path, &config_local)?;
                    let reasons: std::collections::BTreeMap<String, Vec<String>> = detection
                        .reasons_by_page
                        .iter()
                        .map(|(k, v)| {
                            (
                                k.to_string(),
                                v.iter()
                                    .map(|r| {
                                        serde_json::to_string(r)
                                            .unwrap_or_default()
                                            .trim_matches('"')
                                            .to_string()
                                    })
                                    .collect(),
                            )
                        })
                        .collect();
                    (detection.ocr_pages, false, reasons)
                };

                if ocr_pages.is_empty() {
                    info!(
                        "自动检测未发现需要 OCR 的页面: document_id={}, title='{}'",
                        doc_id, title
                    );
                    // 持久化本次 none 检测结论，防止状态接口展示上一次运行的选择结果
                    conn.execute(
                        "UPDATE kb_documents SET \
                         metadata_json = json_set(COALESCE(metadata_json, '{}'), \
                         '$.needs_ocr_pages', json('[]'), \
                         '$.ocr_reasons_by_page', json('{}'), \
                         '$.ocr_scope', 'none'), \
                         updated_at = datetime('now') \
                         WHERE id = ?1 AND processing_run_id = ?2",
                        rusqlite::params![doc_id, run_id],
                    )?;
                    return Ok(());
                }

                // 全局 OCR 开关检查
                if !config_local.ocr_enabled {
                    return Err(gbrain_core::error::GBrainError::InvalidInput(
                        "全局 OCR 已关闭 (GBRAIN_OCR_ENABLED=false)，无法执行 OCR".to_string(),
                    ));
                }
                // OCR API key 检查
                if config_local.ocr_api_key.is_none() {
                    return Err(gbrain_core::error::GBrainError::InvalidInput(
                        "未配置 OCR API key (需设置 GBRAIN_OCR_API_KEY 或 ZHIPU_API_KEY)"
                            .to_string(),
                    ));
                }

                let payload = gbrain_core::kb::jobs::KbOcrPayload {
                    kind: "kb_ocr_document".to_string(),
                    document_id: doc_id,
                    library_id,
                    processing_run_id: run_id.clone(),
                    storage_path,
                    pages: ocr_pages.clone(),
                    submit_mode: config_local.ocr_submit_mode.clone(),
                    provider: "glm_ocr".to_string(),
                    model: config_local.ocr_model.clone(),
                    return_crop_images: config_local.ocr_return_crop_images,
                    need_layout_visualization: config_local.ocr_need_layout_visualization,
                };

                let tx = conn.unchecked_transaction()?;
                let enqueue_result = (|| -> Result<i64> {
                    let rows = conn.execute(
                        "UPDATE kb_documents SET ocr_status = 'queued', ocr_text_coverage = 0.0, \
                         document_status = CASE WHEN ?3 THEN 'ocr_pending' ELSE document_status END, \
                         index_status = CASE WHEN ?3 THEN 'pending' ELSE index_status END, \
                         parsing_error = CASE WHEN ?3 THEN '' ELSE parsing_error END, \
                         parsing_status = CASE WHEN ?3 THEN 1 ELSE parsing_status END, \
                         parsing_progress = CASE WHEN ?3 THEN 0 ELSE parsing_progress END, \
                         updated_at = datetime('now') WHERE id = ?1 AND processing_run_id = ?2",
                        rusqlite::params![doc_id, run_id, is_image],
                    )?;
                    if rows == 0 {
                        return Err(gbrain_core::error::GBrainError::InvalidInput(
                            "文档 processing_run_id 已变化，跳过 OCR 入队".to_string(),
                        ));
                    }

                    // 同步更新 OCR 检测元数据：区分显式手动选页与自动检测，
                    // ocr_scope 依据选中页数与总页数计算为 none/partial/full。
                    let needs_ocr_pages_str =
                        serde_json::to_string(&ocr_pages).unwrap_or_else(|_| "[]".to_string());
                    let reasons_str = serde_json::to_string(&detection_reasons)
                        .unwrap_or_else(|_| "{}".to_string());
                    let ocr_scope = if ocr_pages.is_empty() {
                        "none"
                    } else if ocr_pages.len() as i32 >= total_pages {
                        "full"
                    } else {
                        "partial"
                    };
                    conn.execute(
                        "UPDATE kb_documents SET \
                         metadata_json = json_set(COALESCE(metadata_json, '{}'), \
                         '$.needs_ocr_pages', json(?1), \
                         '$.ocr_reasons_by_page', json(?2), \
                         '$.ocr_scope', ?3), \
                         updated_at = datetime('now') \
                         WHERE id = ?4 AND processing_run_id = ?5",
                        rusqlite::params![
                            needs_ocr_pages_str,
                            reasons_str,
                            ocr_scope,
                            doc_id,
                            run_id
                        ],
                    )?;

                    for &page_num in &ocr_pages {
                        gbrain_core::kb::ocr::update_ocr_page_status(
                            conn,
                            doc_id,
                            page_num,
                            "pending",
                            "CLI 手动触发 OCR，等待处理",
                            "glm_ocr",
                            &config_local.ocr_model,
                            &run_id,
                        )?;
                    }

                    let queue = gbrain_core::jobs::JobQueue::new(conn);
                    queue.enqueue(gbrain_core::jobs::JobInput {
                        job_type: "kb_ocr_document".to_string(),
                        payload: serde_json::to_value(&payload)?,
                        priority: Some(0),
                        max_attempts: Some(3),
                    })
                })();
                let job_row_id = match enqueue_result {
                    Ok(id) => {
                        tx.commit()?;
                        id
                    }
                    Err(e) => {
                        let _ = tx.rollback();
                        return Err(e);
                    }
                };

                info!(
                    "已入队 OCR 作业: document_id={}, title='{}', pages={:?}, job_row_id={}",
                    doc_id, title, ocr_pages, job_row_id
                );
            }

            KbCommands::OcrRetry { doc_id } => {
                let conn = ops.engine.connection()?;

                // 查询文档信息
                let doc_row: (String, i64, String, String) = conn
                    .query_row(
                        "SELECT title, library_id, processing_run_id, extension \
                         FROM kb_documents WHERE id = ?1",
                        rusqlite::params![doc_id],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, i64>(1)?,
                                row.get::<_, String>(2)?,
                                row.get::<_, String>(3)?,
                            ))
                        },
                    )
                    .map_err(|e| {
                        gbrain_core::error::GBrainError::Database(format!("查询文档失败: {}", e))
                    })?;

                let (title, library_id, run_id, extension) = doc_row;
                let is_image =
                    gbrain_core::artifact::types::is_ocr_image_file(&extension.to_lowercase());

                // 检查库隐私策略
                {
                    let kb = gbrain_core::kb::engine::KbEngine::new(conn);
                    let library = kb.get_library(library_id).map_err(|e| {
                        gbrain_core::error::GBrainError::Database(format!("查询库失败: {}", e))
                    })?;
                    if !library.external_ocr_allowed {
                        return Err(gbrain_core::error::GBrainError::InvalidInput(
                            "库已关闭外部 OCR，无法执行 OCR 重试".to_string(),
                        ));
                    }
                    if library.redaction_enabled {
                        return Err(gbrain_core::error::GBrainError::InvalidInput(
                            "库已启用脱敏，禁止外部 OCR".to_string(),
                        ));
                    }
                }

                // 全局 OCR 开关和 API key 检查
                {
                    let config_local = gbrain_core::config::Config::load().unwrap_or_default();
                    if !config_local.ocr_enabled {
                        return Err(gbrain_core::error::GBrainError::InvalidInput(
                            "全局 OCR 已关闭 (GBRAIN_OCR_ENABLED=false)，无法执行 OCR 重试"
                                .to_string(),
                        ));
                    }
                    if config_local.ocr_api_key.is_none() {
                        return Err(gbrain_core::error::GBrainError::InvalidInput(
                            "未配置 OCR API key (需设置 GBRAIN_OCR_API_KEY 或 ZHIPU_API_KEY)"
                                .to_string(),
                        ));
                    }
                }

                // 查询失败的页（含 needed：图片被策略拦住后标为 needed，策略恢复后应可重试）
                let mut failed_pages = Vec::new();
                let mut stmt = conn.prepare(
                    "SELECT page_number FROM kb_document_ocr_pages \
                     WHERE document_id = ?1 AND processing_run_id = ?2 \
                     AND status IN ('failed', 'empty_ocr', 'needed') ORDER BY page_number",
                )?;
                let rows = stmt.query_map(rusqlite::params![doc_id, run_id], |row| {
                    row.get::<_, i32>(0)
                })?;
                for p in rows.flatten() {
                    failed_pages.push(p);
                }
                drop(stmt);

                if failed_pages.is_empty() {
                    info!("文档 {} ('{}') 无失败的 OCR 页", doc_id, title);
                    return Ok(());
                }

                // 获取 storage_path
                let storage_path: String = conn
                    .query_row(
                        "SELECT storage_path FROM kb_documents WHERE id = ?1",
                        rusqlite::params![doc_id],
                        |row| row.get::<_, String>(0),
                    )
                    .unwrap_or_default();

                let config_local = gbrain_core::config::Config::load().unwrap_or_default();
                let payload = gbrain_core::kb::jobs::KbOcrPayload {
                    kind: "kb_ocr_document".to_string(),
                    document_id: doc_id,
                    library_id,
                    processing_run_id: run_id.clone(),
                    storage_path,
                    pages: failed_pages.clone(),
                    submit_mode: config_local.ocr_submit_mode.clone(),
                    provider: "glm_ocr".to_string(),
                    model: config_local.ocr_model.clone(),
                    return_crop_images: config_local.ocr_return_crop_images,
                    need_layout_visualization: config_local.ocr_need_layout_visualization,
                };

                let tx = conn.unchecked_transaction()?;
                let enqueue_result = (|| -> Result<i64> {
                    conn.execute(
                        "UPDATE kb_document_ocr_pages SET status = 'pending', error = '' \
                         WHERE document_id = ?1 AND processing_run_id = ?2 \
                         AND status IN ('failed', 'empty_ocr', 'needed')",
                        rusqlite::params![doc_id, run_id],
                    )?;

                    let rows = conn.execute(
                        "UPDATE kb_documents SET ocr_status = 'queued', ocr_text_coverage = 0.0, \
                         document_status = CASE WHEN ?3 THEN 'ocr_pending' ELSE document_status END, \
                         index_status = CASE WHEN ?3 THEN 'pending' ELSE index_status END, \
                         parsing_error = CASE WHEN ?3 THEN '' ELSE parsing_error END, \
                         parsing_status = CASE WHEN ?3 THEN 1 ELSE parsing_status END, \
                         parsing_progress = CASE WHEN ?3 THEN 0 ELSE parsing_progress END, \
                         updated_at = datetime('now') WHERE id = ?1 AND processing_run_id = ?2",
                        rusqlite::params![doc_id, run_id, is_image],
                    )?;
                    if rows == 0 {
                        return Err(gbrain_core::error::GBrainError::InvalidInput(
                            "文档 processing_run_id 已变化，跳过 OCR 重试入队".to_string(),
                        ));
                    }

                    let queue = gbrain_core::jobs::JobQueue::new(conn);
                    queue.enqueue(gbrain_core::jobs::JobInput {
                        job_type: "kb_ocr_document".to_string(),
                        payload: serde_json::to_value(&payload)?,
                        priority: Some(0),
                        max_attempts: Some(3),
                    })
                })();
                let job_row_id = match enqueue_result {
                    Ok(id) => {
                        tx.commit()?;
                        id
                    }
                    Err(e) => {
                        let _ = tx.rollback();
                        return Err(e);
                    }
                };

                info!(
                    "已入队 OCR 重试: document_id={}, title='{}', failed_pages={:?}, job_row_id={}",
                    doc_id, title, failed_pages, job_row_id
                );
            }
        },
    }

    engine.disconnect()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_page_ranges_validates_bounds() {
        assert_eq!(
            parse_page_ranges("1,3,5-7", 10).unwrap(),
            vec![1, 3, 5, 6, 7]
        );
        assert!(parse_page_ranges("0", 10).is_err());
        assert!(parse_page_ranges("3-2", 10).is_err());
        assert!(parse_page_ranges("11", 10).is_err());
        assert!(parse_page_ranges("1,,2", 10).is_err());
    }
}
