//! gbrain CLI
//! Mirrors gbrain's src/cli.ts

use clap::{Parser, Subcommand};
use gbrain_core::autopilot::Autopilot;
use gbrain_core::config::Config;
use gbrain_core::embedding::Embedder;
use gbrain_core::engine::BrainEngine;
use gbrain_core::error::{GBrainError, Result};
use gbrain_core::lint::{lint_pages, LintOpts};
use gbrain_core::logging;
use gbrain_core::mcp::McpServer;
use gbrain_core::operations::{ExtractMode, OpContext, Operations};
use gbrain_core::sqlite_engine::SqliteEngine;
use gbrain_core::types::*;
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

    /// Dry-run mode: preview operations without committing
    #[arg(long)]
    dry_run: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new brain
    Init,

    /// Get a page by slug
    Get {
        /// Page slug (e.g. people/alice)
        slug: String,
    },

    /// Create or update a page
    Put {
        /// Page slug (e.g. people/alice)
        slug: String,

        /// Page title
        #[arg(long)]
        title: String,

        /// Page content (markdown)
        #[arg(long)]
        content: Option<String>,

        /// Read content from file
        #[arg(long)]
        file: Option<PathBuf>,

        /// Page type
        #[arg(long)]
        page_type: Option<String>,
    },

    /// Delete a page
    Delete {
        /// Page slug
        slug: String,

        /// Skip confirmation
        #[arg(long)]
        force: bool,
    },

    /// Restore a soft-deleted page
    Restore {
        /// Page slug
        slug: String,
    },

    /// Permanently purge soft-deleted pages older than the cutoff
    PurgeDeleted {
        /// Age cutoff in hours
        #[arg(long, default_value = "72")]
        older_than_hours: i64,
    },

    /// List pages
    List {
        /// Filter by page type
        #[arg(long)]
        page_type: Option<String>,

        /// Maximum results
        #[arg(long, default_value = "50")]
        limit: usize,
    },

    /// Query the brain (alias: ask)
    #[command(alias = "ask")]
    Query {
        /// Search query
        query: String,

        /// Maximum results
        #[arg(long, default_value = "20")]
        limit: usize,

        /// Result detail level: low, medium, high
        #[arg(long)]
        detail: Option<String>,

        /// Filter code-aware retrieval by language (e.g. rust, typescript, python)
        #[arg(long = "lang")]
        language: Option<String>,

        /// Filter code-aware retrieval by symbol kind (function, method, class, struct, etc.)
        #[arg(long = "symbol-kind")]
        symbol_kind: Option<String>,

        /// Anchor two-pass code graph retrieval near a symbol
        #[arg(long = "near-symbol")]
        near_symbol: Option<String>,

        /// Walk code graph neighbors up to this depth (0-2)
        #[arg(long = "walk-depth", default_value = "0")]
        walk_depth: usize,

        /// Enable LLM query expansion when configured
        #[arg(long, default_value_t = false)]
        expand: bool,
    },

    /// Backlink operations (list, check, fix)
    Backlinks {
        #[command(subcommand)]
        command: BacklinksCommand,
    },

    /// Traverse the knowledge graph
    Graph {
        /// Starting page slug
        slug: String,

        /// Traversal depth
        #[arg(long, default_value = "2")]
        depth: usize,
    },

    /// Resolve partial slugs
    Resolve {
        /// Partial slug
        partial: String,
    },

    /// Get brain statistics
    Stats,

    /// Get brain health
    Health,

    /// P2-4: Diagnose brain health (mirrors TS gbrain doctor)
    Doctor {
        /// Fast mode — skip expensive checks
        #[arg(long)]
        fast: bool,
    },

    /// P2-5: Check data integrity (mirrors TS gbrain integrity)
    Integrity,

    /// P2-5: Detect orphan pages (mirrors TS gbrain orphans)
    Orphans,

    /// Get ingest log
    IngestLog {
        /// Maximum entries
        #[arg(long, default_value = "20")]
        limit: usize,
    },

    /// Run as MCP stdio server
    Serve,

    /// Lint brain pages for quality issues (zero LLM)
    Lint {
        /// Specific page slug to lint (default: all pages)
        slug: Option<String>,

        /// Fix issues automatically where possible
        #[arg(long)]
        fix: bool,

        /// Don't write changes, just report
        #[arg(long)]
        dry_run: bool,
    },

    /// Install gbrain binary to ~/.gbrain/bin/
    Install,

    /// Run self-maintaining autopilot daemon (embed, integrity, health)
    Autopilot {
        /// Run once and exit (default: loop continuously)
        #[arg(long)]
        once: bool,

        /// Interval in seconds between cycles (default: 3600)
        #[arg(long, default_value = "3600")]
        interval: u64,
    },

    /// Output all MCP tool definitions as JSON
    ToolsJson,

    /// Manage brain config (get/set/list)
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },

    /// Generate a timestamped brain report
    Report {
        /// Report type (e.g., enrichment-sweep, maintenance, lint)
        #[arg(long)]
        report_type: String,

        /// Report title
        #[arg(long)]
        title: Option<String>,

        /// Report content (inline, or from stdin)
        #[arg(long)]
        content: Option<String>,
    },

    /// Export pages to markdown files
    Export {
        /// Output directory (default: stdout-like listing)
        #[arg(long)]
        dir: Option<String>,

        /// Filter by page type
        #[arg(long)]
        page_type: Option<String>,

        /// Specific page slugs to export
        slugs: Vec<String>,
    },

    /// Import markdown files from a directory
    Import {
        /// Directory to scan for .md files
        dir: String,

        /// Generate embeddings for imported content
        #[arg(long)]
        embed: bool,

        /// Auto-link imported pages to existing pages
        #[arg(long)]
        auto_link: bool,
    },

    /// Generate embeddings for un-embedded chunks
    Embed {
        /// Batch size for embedding API calls
        #[arg(long, default_value = "20")]
        batch_size: usize,

        /// Specific page slugs to embed (omit for all)
        slugs: Vec<String>,
    },

    /// Query the knowledge graph between pages
    GraphQuery {
        /// Starting page slug
        from: String,

        /// Target page slug (optional, for path finding)
        #[arg(long)]
        to: Option<String>,

        /// Maximum traversal depth
        #[arg(long, default_value = "3")]
        depth: usize,

        /// Filter by link type
        #[arg(long)]
        link_type: Option<String>,
    },

    /// Code index and symbol graph operations
    Code {
        #[command(subcommand)]
        command: CodeCommands,
    },

    /// File storage operations
    File {
        #[command(subcommand)]
        command: FileCommands,
    },

    /// Batch extract links and/or timeline entries from all pages (mirrors TS gbrain extract)
    Extract {
        /// What to extract: links, timeline, or all
        #[arg(long, default_value = "all")]
        mode: String,
    },

    /// Run KB document processing worker (claim jobs from queue, process, complete/fail)
    KbWorker {
        /// Run once and exit (default: loop continuously)
        #[arg(long)]
        once: bool,

        /// Polling interval in seconds when no jobs are available
        #[arg(long, default_value = "30")]
        interval: u64,
    },

    /// Run KB search evaluation for a library
    KbEval {
        /// Library ID to evaluate
        #[arg(long)]
        library_id: i64,
    },

    /// Backup KB database and storage
    KbBackup {
        /// Output directory for backup
        #[arg(long)]
        output: String,
    },

    /// Restore KB from backup
    KbRestore {
        /// Input directory containing backup
        #[arg(long)]
        input: String,
    },

    /// Add a local directory as KB import source
    KbSourceAdd {
        /// Library ID
        #[arg(long)]
        library_id: i64,
        /// Directory path to import
        #[arg(long)]
        path: String,
    },

    /// Sync a KB import source
    KbSyncSource {
        /// Source ID to sync
        #[arg(long)]
        source_id: i64,
    },

    /// KB jobs management (list/pause/resume)
    KbJobs {
        #[command(subcommand)]
        command: KbJobsCommand,
    },

    /// Export a KB library to a directory archive
    KbExportLibrary {
        /// Library ID to export
        #[arg(long)]
        library_id: i64,
        /// Output directory for export
        #[arg(long)]
        output: String,
    },

    /// Import a KB library from an export archive
    KbImportLibrary {
        /// Input directory containing the export archive
        #[arg(long)]
        archive: String,
        /// New library name (optional, defaults to original name from manifest)
        #[arg(long)]
        new_name: Option<String>,
    },

    /// Re-embed documents with a new embedding model/index
    KbReembed {
        /// Library ID to re-embed
        #[arg(long)]
        library_id: i64,
        /// Target embedding index ID
        #[arg(long)]
        embedding_index_id: Option<i64>,
    },

    /// Compare two embedding indexes using eval queries
    KbEvalCompare {
        /// First embedding index ID
        #[arg(long)]
        index_id_1: i64,
        /// Second embedding index ID to compare
        #[arg(long)]
        index_id_2: i64,
    },

    /// Check KB index health and optionally repair
    KbHealthCheck {
        /// Library ID to check
        #[arg(long)]
        library_id: Option<i64>,
        /// Repair issues found
        #[arg(long)]
        repair: bool,
    },

    /// Rebuild a single document's index
    KbRebuildDocument {
        /// Document ID to rebuild
        #[arg(long)]
        document_id: i64,
    },

    /// Rebuild an entire library's index
    KbRebuildLibrary {
        /// Library ID to rebuild
        #[arg(long)]
        library_id: i64,
    },

    /// Purge deleted KB documents older than retention period
    KbPurgeDeleted {
        /// Library ID (optional, purges all if not specified)
        #[arg(long)]
        library_id: Option<i64>,
        /// Older than N days
        #[arg(long, default_value = "30")]
        older_than_days: i32,
    },

    // ========================================================================
    // 单入口多投影融合架构 — Upload / MemoryQuery / Promotion
    // ========================================================================
    /// Upload a source file (unified entry point for gbrain + KB + file storage)
    Upload {
        /// File path to upload
        path: PathBuf,

        /// Upload intent: auto, document, attachment, memory, promote
        #[arg(long, default_value = "auto")]
        intent: String,

        /// KB library ID
        #[arg(long)]
        library_id: Option<i64>,

        /// Target gbrain page slug (for promotion)
        #[arg(long)]
        target: Option<String>,

        /// Target page slug (for file attachment)
        #[arg(long)]
        page: Option<String>,

        /// KB folder ID
        #[arg(long)]
        folder_id: Option<i64>,

        // 修复：改为 Option，区分用户显式指定和默认值
        // 用户未指定时让 intent 推断策略（如 Memory → auto_accept_low_risk）
        #[arg(long)]
        promotion: Option<String>,

        /// Dry run: only return route plan
        #[arg(long, default_value_t = false)]
        dry_run: bool,

        /// Output as JSON
        #[arg(long, default_value_t = false)]
        json: bool,
    },

    /// Unified memory query (alias: ask-memory)
    #[command(alias = "ask-memory")]
    MemoryQuery {
        /// Query text
        query: String,

        /// Query strategy: brain_first, evidence_first, provenance, timeline_first
        #[arg(long, default_value = "brain_first")]
        strategy: String,

        /// Maximum results
        #[arg(long, default_value = "10")]
        limit: i64,

        /// Filter by slug
        #[arg(long)]
        filter_slug: Option<String>,

        /// Include KB evidence
        #[arg(long, default_value_t = true)]
        include_evidence: bool,

        /// Include provenance
        #[arg(long, default_value_t = false)]
        include_provenance: bool,

        /// Output as JSON
        #[arg(long, default_value_t = false)]
        json: bool,
    },

    /// List promotion candidates
    PromotionList {
        /// Filter by status: pending, approved, rejected, applied
        #[arg(long)]
        status: Option<String>,

        /// Filter by candidate type
        #[arg(long)]
        candidate_type: Option<String>,

        /// Filter by target slug
        #[arg(long)]
        target_slug: Option<String>,

        /// Maximum results
        #[arg(long, default_value = "50")]
        limit: i64,

        /// Output as JSON
        #[arg(long, default_value_t = false)]
        json: bool,
    },

    /// Get promotion candidate details
    PromotionGet {
        /// Candidate ID
        candidate_id: i64,
    },

    /// Accept a promotion candidate
    PromotionAccept {
        /// Candidate ID
        candidate_id: i64,

        /// Reviewer name
        #[arg(long, default_value = "cli")]
        reviewer: String,

        /// Review notes
        #[arg(long)]
        notes: Option<String>,
    },

    /// Reject a promotion candidate
    PromotionReject {
        /// Candidate ID
        candidate_id: i64,

        /// Reviewer name
        #[arg(long, default_value = "cli")]
        reviewer: String,

        /// Review notes
        #[arg(long)]
        notes: Option<String>,
    },

    /// Apply an approved promotion candidate
    PromotionApply {
        /// Candidate ID
        candidate_id: i64,
    },

    /// Auto-apply low-risk candidates for an artifact
    PromotionAutoApply {
        /// Artifact ID
        artifact_id: i64,
    },

    /// Batch apply all pending promotion candidates (§10.5)
    PromotionBatchApply {
        /// Filter by artifact ID (optional)
        #[arg(long)]
        artifact_id: Option<i64>,

        /// Filter by risk level: low, medium, high
        #[arg(long)]
        risk: Option<String>,

        /// Preview candidates without applying
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },

    /// Rollback an applied promotion candidate (§31)
    PromotionRollback {
        /// Candidate ID to rollback
        candidate_id: i64,
    },

    /// Garbage collect orphaned projections (§31)
    GcOrphanProjections {
        /// Delete projections orphaned/superseded for more than N days
        #[arg(long, default_value = "30")]
        stale_days: u32,

        /// Preview what would be cleaned without making changes
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },

    /// Supersede an old projection with a new one (§31 version chain)
    ProjectionSupersede {
        /// Old projection ID to supersede
        old_proj_id: i64,

        /// New projection ID that replaces the old one
        new_proj_id: i64,
    },

    /// Query projection version chain history (§31)
    ProjectionHistory {
        /// Projection key to query history for
        projection_key: String,

        /// 限定 artifact ID，避免同一 key 下多个 artifact 的投影混合
        #[arg(long)]
        artifact_id: Option<i64>,

        /// 限定 projection type（如 kb_document）
        #[arg(long)]
        projection_type: Option<String>,

        /// Maximum history records to return
        #[arg(long, default_value = "20")]
        limit: i64,
    },

    /// List source artifacts
    ArtifactList {
        /// Maximum results
        #[arg(long, default_value = "50")]
        limit: i64,

        /// Offset
        #[arg(long, default_value = "0")]
        offset: i64,

        /// Output as JSON
        #[arg(long, default_value_t = false)]
        json: bool,
    },

    /// Get source artifact details
    ArtifactGet {
        /// Artifact ID or UID
        id_or_uid: String,
    },

    /// Delete a source artifact (soft delete)
    ArtifactDelete {
        /// Artifact ID
        artifact_id: i64,
    },

    /// Check artifact health
    ArtifactHealth,
}

#[derive(Subcommand)]
enum KbJobsCommand {
    /// List pending/processing KB jobs
    List {
        /// Filter by library ID
        #[arg(long)]
        library_id: Option<i64>,
    },
    /// Pause processing for a library
    Pause {
        #[arg(long)]
        library_id: i64,
    },
    /// Resume processing for a library
    Resume {
        #[arg(long)]
        library_id: i64,
    },
}

#[derive(Subcommand)]
enum FileCommands {
    /// List stored files
    List {
        /// Filter by page slug
        slug: Option<String>,
    },

    /// Upload a file to storage
    Upload {
        /// Local file path to upload
        path: PathBuf,

        /// Associate with page slug
        #[arg(long)]
        page: Option<String>,
    },

    /// Sync a directory of files to storage
    Sync {
        /// Directory to sync
        dir: PathBuf,
    },

    /// Verify all file records
    Verify,

    /// Get local path for a stored file
    Url {
        /// Storage path of the file
        storage_path: String,
    },
}

#[derive(Subcommand)]
enum CodeCommands {
    /// Rebuild code chunks and code edges for a page
    Reindex { slug: String },

    /// Find symbol definitions
    Def {
        symbol: String,
        #[arg(long = "lang")]
        language: Option<String>,
        #[arg(long, default_value = "20")]
        limit: usize,
    },

    /// Find code chunks that reference a symbol
    Refs {
        symbol: String,
        #[arg(long = "lang")]
        language: Option<String>,
        #[arg(long, default_value = "20")]
        limit: usize,
    },

    /// Search indexed code chunks
    Search {
        query: String,
        #[arg(long, default_value = "20")]
        limit: usize,
        #[arg(long = "lang")]
        language: Option<String>,
        #[arg(long = "symbol-kind")]
        symbol_kind: Option<String>,
    },

    /// Show callers of a symbol
    Callers { slug: String, symbol: String },

    /// Show callees of a symbol
    Callees { slug: String, symbol: String },

    /// Show code edges attached to a chunk id
    Edges { chunk_id: i64 },
}

/// Config subcommands
#[derive(Subcommand)]
enum ConfigCommand {
    /// Show all config values
    Show,
    /// Get a single config value
    Get { key: String },
    /// Set a config value
    Set { key: String, value: String },
}

/// Backlink subcommands (mirrors TS gbrain backlinks check/fix)
#[derive(Subcommand)]
enum BacklinksCommand {
    /// List backlinks for a page
    List {
        /// Page slug
        slug: String,
    },
    /// Check for missing backlinks (wiki-links without corresponding DB entries)
    Check {
        /// Page slug (omit to check all pages)
        slug: Option<String>,
    },
    /// Fix missing backlinks by adding entries to the links table
    Fix {
        /// Page slug (omit to fix all pages)
        slug: Option<String>,
        /// Preview changes without committing
        #[arg(long)]
        dry_run: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    // Initialize logging from config
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

    info!(db_path = %db_path, "Connecting to brain database");

    let mut engine = SqliteEngine::new(PathBuf::from(db_path.clone()).as_path());
    engine.connect()?;
    engine.init_schema()?;

    let mut ctx = OpContext::default();
    if cli.dry_run {
        ctx.dry_run = true;
        info!("Dry-run mode enabled — no changes will be committed");
    }
    let ops = Operations::with_config(&engine, ctx, config.clone());

    match cli.command {
        Commands::Init => {
            // Copy current executable to ~/.gbrain/bin/
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

        Commands::Get { slug } => match ops.get_page(&slug)? {
            Some(page) => {
                if cli.json {
                    info!("{}", serde_json::to_string_pretty(&page)?);
                } else {
                    info!("# {}", page.title);
                    info!("{}", page.compiled_truth);
                }
            }
            None => {
                warn!(slug = %slug, "Page not found");
                std::process::exit(1);
            }
        },

        Commands::Put {
            slug,
            title,
            content,
            file,
            page_type,
        } => {
            let content = if let Some(path) = file {
                info!(path = %path.display(), "Reading content from file");
                std::fs::read_to_string(&path)?
            } else {
                content.unwrap_or_default()
            };

            let pt = page_type.as_deref().map(PageType::from_str_lossy);

            let page = ops.put_page(&slug, &title, &content, pt, None)?;
            if cli.json {
                info!("{}", serde_json::to_string_pretty(&page)?);
            } else {
                info!(slug = %page.slug, title = %page.title, "Page saved");
            }
        }

        Commands::Delete { slug, force } => {
            if !force {
                warn!(slug = %slug, "Are you sure you want to soft-delete this page? Use --force to confirm.");
                return Ok(());
            }
            ops.delete_page(&slug)?;
            info!(slug = %slug, "Page soft-deleted");
        }

        Commands::Restore { slug } => {
            if ops.engine.restore_page(&slug)? {
                info!(slug = %slug, "Page restored");
            } else {
                warn!(slug = %slug, "Page was not soft-deleted or does not exist");
            }
        }

        Commands::PurgeDeleted { older_than_hours } => {
            let slugs = ops.engine.purge_deleted_pages(older_than_hours)?;
            if cli.json {
                info!("{}", serde_json::to_string_pretty(&slugs)?);
            } else {
                info!(
                    count = slugs.len(),
                    older_than_hours, "Purged soft-deleted pages"
                );
            }
        }

        Commands::List { page_type, limit } => {
            let filters = PageFilters {
                page_type: page_type.as_deref().map(PageType::from_str_lossy),
                tag: None,
                limit: Some(limit),
                offset: None,
                updated_after: None,
                include_deleted: false,
                slug_prefix: None,
            };

            let pages = ops.list_pages(filters)?;
            if cli.json {
                info!("{}", serde_json::to_string_pretty(&pages)?);
            } else {
                for page in &pages {
                    info!("{} [{}] {}", page.slug, page.page_type, page.title);
                }
                info!("{} pages", pages.len());
            }
        }

        Commands::Query {
            query,
            limit,
            detail,
            language,
            symbol_kind,
            near_symbol,
            walk_depth,
            expand,
        } => {
            let detail_level = detail.as_deref().and_then(|d| match d {
                "low" => Some(DetailLevel::Low),
                "medium" => Some(DetailLevel::Medium),
                "high" => Some(DetailLevel::High),
                _ => None,
            });
            let opts = SearchOpts {
                limit: Some(limit),
                detail_level,
                language,
                symbol_kind,
                near_symbol,
                walk_depth: Some(walk_depth.min(2)),
                ..Default::default()
            };

            let result_with_meta = ops.query_with_meta(&query, opts, expand)?;
            if cli.json {
                info!("{}", serde_json::to_string_pretty(&result_with_meta)?);
            } else {
                for result in &result_with_meta.results {
                    info!("{} | {} | {:.3}", result.slug, result.title, result.score);
                    if !result.chunk_text.is_empty() {
                        info!(
                            "  {}",
                            result.chunk_text.chars().take(100).collect::<String>()
                        );
                    }
                }
                info!(
                    "{} results (vector_enabled={}, expansion_applied={}, detail={:?})",
                    result_with_meta.results.len(),
                    result_with_meta.meta.vector_enabled,
                    result_with_meta.meta.expansion_applied,
                    result_with_meta.meta.detail_resolved
                );
            }
        }

        Commands::Backlinks { command } => match command {
            BacklinksCommand::List { slug } => {
                let links = ops.get_backlinks(&slug)?;
                if cli.json {
                    info!("{}", serde_json::to_string_pretty(&links)?);
                } else {
                    for link in &links {
                        info!(
                            "{} -> {} [{}]",
                            link.from_slug, link.to_slug, link.link_type
                        );
                    }
                    info!("{} backlinks", links.len());
                }
            }
            BacklinksCommand::Check { slug } => {
                let missing = ops.check_backlinks(slug.as_deref())?;
                if cli.json {
                    let result: Vec<serde_json::Value> = missing
                        .iter()
                        .map(|(from, to)| serde_json::json!({"from": from, "to": to}))
                        .collect();
                    info!("{}", serde_json::to_string_pretty(&result)?);
                } else if missing.is_empty() {
                    info!("No missing backlinks found.");
                } else {
                    info!("Missing backlinks ({}):", missing.len());
                    for (from, to) in &missing {
                        info!("  {} -> {}", from, to);
                    }
                }
            }
            BacklinksCommand::Fix { slug, dry_run } => {
                let added = ops.fix_backlinks(slug.as_deref(), dry_run)?;
                if dry_run {
                    info!("Dry run: {} backlinks would be added.", added);
                } else {
                    info!("Fixed: {} backlinks added.", added);
                }
            }
        },

        Commands::Graph { slug, depth } => {
            let nodes = ops.traverse_graph(&slug, depth)?;
            if cli.json {
                info!("{}", serde_json::to_string_pretty(&nodes)?);
            } else {
                for node in &nodes {
                    let indent = "  ".repeat(node.depth);
                    info!(
                        "{}{} [{}] ({})",
                        indent, node.slug, node.page_type, node.title
                    );
                    for link in &node.links {
                        info!("{}  -> {} [{}]", indent, link.to_slug, link.link_type);
                    }
                }
            }
        }

        Commands::Resolve { partial } => {
            let slugs = ops.resolve_slugs(&partial)?;
            if cli.json {
                info!("{}", serde_json::to_string_pretty(&slugs)?);
            } else {
                for slug in &slugs {
                    info!("{}", slug);
                }
            }
        }

        Commands::Stats => {
            let stats = ops.get_stats()?;
            if cli.json {
                info!("{}", serde_json::to_string_pretty(&stats)?);
            } else {
                info!("Pages: {}", stats.page_count);
                info!("Chunks: {}", stats.chunk_count);
                info!("Embedded: {}", stats.embedded_count);
                info!("Links: {}", stats.link_count);
                info!("Tags: {}", stats.tag_count);
                info!("Timeline entries: {}", stats.timeline_entry_count);
                if !stats.pages_by_type.is_empty() {
                    info!("Pages by type:");
                    let mut types: Vec<_> = stats.pages_by_type.iter().collect();
                    types.sort_by_key(|(_, count)| -**count);
                    for (page_type, count) in types {
                        info!("  {}: {}", page_type, count);
                    }
                }
            }
        }

        Commands::Health => {
            let health = ops.get_health()?;
            if cli.json {
                info!("{}", serde_json::to_string_pretty(&health)?);
            } else {
                info!("Brain score: {:.1}/100", health.brain_score);
                info!("Pages: {}", health.page_count);
                info!("Embed coverage: {:.1}%", health.embed_coverage * 100.0);
                info!("Link coverage: {:.1}%", health.link_coverage * 100.0);
                info!(
                    "Timeline coverage: {:.1}%",
                    health.timeline_coverage * 100.0
                );
                info!("Orphan pages: {}", health.orphan_pages);
                info!("Dead links: {}", health.dead_links);
                info!("Stale pages: {}", health.stale_pages);
            }
        }

        // P2-4: Doctor command — comprehensive diagnostics (mirrors TS gbrain doctor)
        Commands::Doctor { fast } => {
            info!("=== gbrain doctor ===");
            // DB connection check
            info!("[OK] Database connected: {}", db_path);
            // Health report
            let health = ops.get_health()?;
            info!("Brain score: {:.1}/100", health.brain_score);
            info!("Embed coverage: {:.1}%", health.embed_coverage * 100.0);
            info!("Orphan pages: {}", health.orphan_pages);
            info!("Dead links: {}", health.dead_links);
            if !fast {
                // Full diagnostics
                let stats = ops.engine.get_stats()?;
                info!(
                    "Pages: {}, Chunks: {}, Links: {}",
                    stats.page_count, stats.chunk_count, stats.link_count
                );
                // Orphan detection
                let orphans = engine.detect_orphans()?;
                if orphans.is_empty() {
                    info!("[OK] No orphan pages");
                } else {
                    warn!("[WARN] {} orphan page(s):", orphans.len());
                    for slug in &orphans {
                        warn!("  - {}", slug);
                    }
                }
                // Dead link detection
                let dead = engine.detect_dead_links()?;
                if dead.is_empty() {
                    info!("[OK] No dead links");
                } else {
                    warn!("[WARN] {} dead link(s):", dead.len());
                    for (from, to) in &dead {
                        warn!("  - {} -> {}", from, to);
                    }
                }
                // Artifact projection consistency check
                let artifact_health = ops.artifact_health_check()?;
                if artifact_health.issues.is_empty() {
                    info!("[OK] Artifact projections consistent");
                } else {
                    warn!("[WARN] {} artifact issue(s):", artifact_health.issues.len());
                    for issue in &artifact_health.issues {
                        warn!(
                            "  - [{}] {}: {}",
                            issue.severity, issue.issue_type, issue.description
                        );
                    }
                }
            }
            info!("=== doctor complete ===");
        }

        // P2-5: Integrity check (mirrors TS gbrain integrity)
        Commands::Integrity => {
            info!("=== integrity check ===");
            let orphans = engine.detect_orphans()?;
            let dead = engine.detect_dead_links()?;
            let issues = orphans.len() + dead.len();
            if orphans.is_empty() {
                info!("[OK] No orphan pages");
            } else {
                warn!("[WARN] {} orphan page(s):", orphans.len());
                for slug in &orphans {
                    warn!("  - {}", slug);
                }
            }
            if dead.is_empty() {
                info!("[OK] No dead links");
            } else {
                warn!("[WARN] {} dead link(s):", dead.len());
                for (from, to) in &dead {
                    warn!("  - {} -> {}", from, to);
                }
            }
            info!("=== {} issue(s) found ===", issues);
        }

        // P2-5: Orphan detection (mirrors TS gbrain orphans)
        Commands::Orphans => {
            let orphans = engine.detect_orphans()?;
            if cli.json {
                info!("{}", serde_json::to_string_pretty(&orphans)?);
            } else if orphans.is_empty() {
                info!("No orphan pages found");
            } else {
                for slug in &orphans {
                    info!("{}", slug);
                }
                info!("{} orphan page(s)", orphans.len());
            }
        }

        Commands::IngestLog { limit } => {
            let entries = ops.get_ingest_log(Some(limit))?;
            if cli.json {
                info!("{}", serde_json::to_string_pretty(&entries)?);
            } else {
                for entry in &entries {
                    info!(
                        "[{}] {} - {} ({} pages)",
                        entry.created_at,
                        entry.source,
                        entry.status,
                        entry.pages_updated.len()
                    );
                }
            }
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
            let mut server = McpServer::with_config(engine, config.clone());
            server.run()?;
            return Ok(());
        }

        Commands::Lint { slug, fix, dry_run } => {
            let lint_opts = LintOpts { fix, dry_run };
            let results = lint_pages(&engine, slug.as_deref(), lint_opts);
            let mut total_issues = 0;
            let mut total_errors = 0;
            let mut total_fixed = 0;
            for result in &results {
                total_issues += result.issues.len();
                if result.has_errors() {
                    total_errors += result
                        .issues
                        .iter()
                        .filter(|i| i.severity == gbrain_core::lint::LintSeverity::Error)
                        .count();
                }
                if result.fixed_content.is_some() {
                    total_fixed += 1;
                }
                for issue in &result.issues {
                    match issue.severity {
                        gbrain_core::lint::LintSeverity::Error => {
                            error!(
                                "[{}] {} ({}): {}",
                                issue.severity, result.slug, issue.rule, issue.message
                            );
                        }
                        gbrain_core::lint::LintSeverity::Warning => {
                            warn!(
                                "[{}] {} ({}): {}",
                                issue.severity, result.slug, issue.rule, issue.message
                            );
                        }
                        gbrain_core::lint::LintSeverity::Info => {
                            info!(
                                "[{}] {} ({}): {}",
                                issue.severity, result.slug, issue.rule, issue.message
                            );
                        }
                    }
                }
            }
            if cli.json {
                info!("{}", serde_json::to_string_pretty(&results)?);
            } else {
                let fix_msg = if fix && total_fixed > 0 {
                    format!(", {} pages fixed", total_fixed)
                } else {
                    String::new()
                };
                let dry_msg = if dry_run { " (dry run)" } else { "" };
                info!(
                    "{} pages linted, {} issues ({} errors){}{}",
                    results.len(),
                    total_issues,
                    total_errors,
                    fix_msg,
                    dry_msg
                );
            }
        }

        Commands::Install => {
            let bin_dir = Config::base_dir().join("bin");
            std::fs::create_dir_all(&bin_dir)?;
            let dest = bin_dir.join("gbrain");

            // Find the current executable path
            let current_exe = std::env::current_exe()?;
            std::fs::copy(&current_exe, &dest)?;
            info!("Installed gbrain to {}", bin_dir.display());

            // Add to PATH hint
            info!("Add to PATH: export PATH=\"{}:$PATH\"", bin_dir.display());
        }

        Commands::Autopilot { once, interval } => {
            let autopilot = Autopilot::new(&engine, config.clone());
            if once {
                autopilot.run_once()?;
                info!("Autopilot: one-shot cycle complete");
            } else {
                autopilot.run_loop(interval);
            }
        }

        Commands::ToolsJson => {
            let tools = gbrain_core::mcp::tool_defs::build_tool_defs();
            info!("{}", serde_json::to_string_pretty(&tools)?);
        }

        Commands::Config { command } => match command {
            ConfigCommand::Show => {
                let keys = ["auto_link", "auto_timeline", "writer.lint_on_put_page"];
                for key in &keys {
                    if let Some(val) = ops.engine.get_config(key)? {
                        info!("{} = {}", key, val);
                    } else {
                        info!("{} = (not set)", key);
                    }
                }
            }
            ConfigCommand::Get { key } => match ops.engine.get_config(&key)? {
                Some(val) => info!("{}", val),
                None => info!("(not set)"),
            },
            ConfigCommand::Set { key, value } => {
                ops.engine.set_config(&key, &value)?;
                info!("{} = {}", key, value);
            }
        },

        Commands::Report {
            report_type,
            title,
            content,
        } => {
            let now = chrono::Utc::now();
            let dir = Config::base_dir().join("reports").join(&report_type);
            std::fs::create_dir_all(&dir)?;
            let filename = format!("{}.md", now.format("%Y-%m-%d-%H%M"));
            let path = dir.join(&filename);
            let title = title.unwrap_or_else(|| report_type.clone());
            let body = content.unwrap_or_default();
            let report_content = format!(
                "---\ntitle: {}\ntype: report\nreport_type: {}\ndate: {}\ntime: {}\n---\n\n{}",
                title,
                report_type,
                now.format("%Y-%m-%d"),
                now.format("%H:%M"),
                body
            );
            std::fs::write(&path, report_content)?;
            info!("Report saved: {}", path.display());
        }

        Commands::Export {
            dir,
            page_type,
            slugs,
        } => {
            let pages = if slugs.is_empty() {
                let filters = PageFilters {
                    page_type: page_type.as_deref().map(PageType::from_str_lossy),
                    limit: None,
                    offset: None,
                    tag: None,
                    updated_after: None,
                    include_deleted: false,
                    slug_prefix: None,
                };
                ops.engine.list_pages(filters)?
            } else {
                slugs
                    .iter()
                    .filter_map(|s| ops.get_page(s).ok().flatten())
                    .collect()
            };
            if let Some(out_dir) = dir {
                std::fs::create_dir_all(&out_dir)?;
                for page in &pages {
                    let path = std::path::PathBuf::from(&out_dir)
                        .join(format!("{}.md", page.slug.replace('/', "_")));
                    let content = format!(
                        "---\ntype: {}\ntitle: {}\n---\n\n{}",
                        page.page_type, page.title, page.compiled_truth
                    );
                    std::fs::write(&path, content)?;
                    info!("Exported: {}", path.display());
                }
                info!("Exported {} pages to {}", pages.len(), out_dir);
            } else if cli.json {
                info!("{}", serde_json::to_string_pretty(&pages)?);
            } else {
                for page in &pages {
                    info!(
                        "---\ntype: {}\ntitle: {}\nslug: {}\n---\n{}",
                        page.page_type, page.title, page.slug, page.compiled_truth
                    );
                }
            }
        }

        Commands::Import {
            dir,
            embed: do_embed,
            auto_link,
        } => {
            let import_dir = std::path::Path::new(&dir);
            if !import_dir.is_dir() {
                return Err(GBrainError::InvalidInput(format!(
                    "Not a directory: {}",
                    dir
                )));
            }
            let mut count = 0;
            let mut imported_slugs = Vec::new();
            let mut dirs = vec![import_dir.to_path_buf()];
            while let Some(current) = dirs.pop() {
                for entry in std::fs::read_dir(&current)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_dir() {
                        dirs.push(path);
                    } else if path.extension().and_then(|s| s.to_str()) == Some("md") {
                        if should_skip_import_file(&path) {
                            continue;
                        }
                        let content = std::fs::read_to_string(&path)?;
                        // R3-08: Derive slug from relative path within import dir for
                        // structured slugs (e.g. "people/alice"), and validate it
                        // to prevent path traversal from malicious filenames.
                        let relative = path.strip_prefix(import_dir).unwrap_or(&path);
                        let slug = relative
                            .with_extension("")
                            .to_string_lossy()
                            .replace('\\', "/");
                        // Validate slug — skip files with invalid/traversal slugs
                        if gbrain_core::security::validate_page_slug(&slug).is_err() {
                            tracing::warn!(slug = %slug, "Skipping file with invalid slug");
                            continue;
                        }
                        let parsed = gbrain_core::markdown::parse_markdown(&content);
                        if let Some(fm_slug) =
                            parsed.frontmatter.get("slug").and_then(|v| v.as_str())
                        {
                            if fm_slug != slug {
                                tracing::warn!(path = %path.display(), frontmatter_slug = %fm_slug, path_slug = %slug, "Skipping file with mismatched frontmatter slug");
                                continue;
                            }
                        }
                        let title = parsed
                            .frontmatter
                            .get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or(&slug)
                            .to_string();
                        ops.put_page(&slug, &title, &content, None, None)?;
                        imported_slugs.push(slug);
                        count += 1;
                    } else if is_supported_code_file(&path) {
                        if should_skip_import_file(&path) {
                            continue;
                        }
                        let content = std::fs::read_to_string(&path)?;
                        let relative = path.strip_prefix(import_dir).unwrap_or(&path);
                        let slug = code_slug_from_relative(relative);
                        if gbrain_core::security::validate_page_slug(&slug).is_err() {
                            tracing::warn!(slug = %slug, path = %path.display(), "Skipping code file with invalid slug");
                            continue;
                        }
                        let title = relative
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or(&slug)
                            .to_string();
                        let language = language_from_path(&path).unwrap_or("text");
                        let import_content = format!(
                            "---\nfile: {}\nlanguage: {}\n---\n\n{}",
                            relative.to_string_lossy().replace('\\', "/"),
                            language,
                            content
                        );
                        ops.put_page(&slug, &title, &import_content, Some(PageType::Code), None)?;
                        imported_slugs.push(slug);
                        count += 1;
                    }
                }
            }
            info!("Imported {} pages from {}", count, dir);
            if do_embed {
                let embedded = embed_stale_chunks(&ops, config, 100, Some(&imported_slugs))?;
                info!(embedded, "Embedded imported chunks");
            }
            if auto_link {
                info!("Auto-link enabled — links extracted during put_page");
            }
        }

        Commands::Embed { batch_size, slugs } => {
            let embedded = embed_stale_chunks(
                &ops,
                config,
                batch_size,
                if slugs.is_empty() { None } else { Some(&slugs) },
            )?;
            info!(embedded, "Embed complete");
        }

        Commands::GraphQuery {
            from,
            to,
            depth,
            link_type,
        } => {
            let direction = if link_type.is_some() {
                Direction::Out
            } else {
                Direction::Both
            };
            if let Some(target) = to {
                let opts = TraverseOpts {
                    depth,
                    direction,
                    link_type,
                };
                let paths = ops.engine.traverse_paths(&from, &target, opts)?;
                if cli.json {
                    info!("{}", serde_json::to_string_pretty(&paths)?);
                } else {
                    for p in &paths {
                        info!(
                            "{} -> {} [{}] depth={}",
                            p.from_slug, p.to_slug, p.link_type, p.depth
                        );
                    }
                    info!("{} paths found", paths.len());
                }
            } else {
                let nodes = ops.traverse_graph(&from, depth)?;
                if cli.json {
                    info!("{}", serde_json::to_string_pretty(&nodes)?);
                } else {
                    for node in &nodes {
                        info!(
                            "  {:indent$}{} [{}]",
                            "",
                            node.slug,
                            node.page_type,
                            indent = node.depth * 2
                        );
                    }
                    info!("{} nodes reachable", nodes.len());
                }
            }
        }

        Commands::Code { command } => match command {
            CodeCommands::Reindex { slug } => {
                let count = ops.reindex_code_page(&slug)?;
                info!(slug = %slug, chunk_count = count, "Code page reindexed");
            }
            CodeCommands::Def {
                symbol,
                language,
                limit,
            } => {
                let chunks = ops.find_code_definitions(&symbol, language.as_deref(), limit)?;
                if cli.json {
                    info!("{}", serde_json::to_string_pretty(&chunks)?);
                } else {
                    for c in &chunks {
                        info!(
                            "{}#{} [{}:{}-{}]",
                            c.slug,
                            c.symbol_name.as_deref().unwrap_or("<file>"),
                            c.language.as_deref().unwrap_or(""),
                            c.start_line.unwrap_or_default(),
                            c.end_line.unwrap_or_default()
                        );
                    }
                    info!("{} definition chunk(s)", chunks.len());
                }
            }
            CodeCommands::Refs {
                symbol,
                language,
                limit,
            } => {
                let refs = ops.find_code_references(&symbol, language.as_deref(), limit)?;
                if cli.json {
                    info!("{}", serde_json::to_string_pretty(&refs)?);
                } else {
                    for r in &refs {
                        info!(
                            "{}#{} [{}:{}-{}] score={:.3}",
                            r.slug,
                            r.symbol_name.as_deref().unwrap_or("<file>"),
                            r.language.as_deref().unwrap_or(""),
                            r.start_line.unwrap_or_default(),
                            r.end_line.unwrap_or_default(),
                            r.score
                        );
                    }
                    info!("{} reference chunk(s)", refs.len());
                }
            }
            CodeCommands::Search {
                query,
                limit,
                language,
                symbol_kind,
            } => {
                let results = ops.search_keyword_chunks(
                    &query,
                    SearchOpts {
                        limit: Some(limit),
                        page_type: Some(PageType::Code),
                        language,
                        symbol_kind,
                        ..Default::default()
                    },
                )?;
                if cli.json {
                    info!("{}", serde_json::to_string_pretty(&results)?);
                } else {
                    for r in &results {
                        info!(
                            "{}#{} [{}:{}-{}] score={:.3}",
                            r.slug,
                            r.symbol_name.as_deref().unwrap_or("<file>"),
                            r.language.as_deref().unwrap_or(""),
                            r.start_line.unwrap_or_default(),
                            r.end_line.unwrap_or_default(),
                            r.score
                        );
                    }
                    info!("{} code chunk(s) found", results.len());
                }
            }
            CodeCommands::Callers { slug, symbol } => {
                let edges = ops.get_callers_of(&slug, &symbol)?;
                if cli.json {
                    info!("{}", serde_json::to_string_pretty(&edges)?);
                } else {
                    for e in &edges {
                        info!(
                            "{}#{} -> {}#{}",
                            e.from_slug, e.from_symbol, e.to_slug, e.to_symbol
                        );
                    }
                    info!("{} caller edge(s)", edges.len());
                }
            }
            CodeCommands::Callees { slug, symbol } => {
                let edges = ops.get_callees_of(&slug, &symbol)?;
                if cli.json {
                    info!("{}", serde_json::to_string_pretty(&edges)?);
                } else {
                    for e in &edges {
                        info!(
                            "{}#{} -> {}#{}",
                            e.from_slug, e.from_symbol, e.to_slug, e.to_symbol
                        );
                    }
                    info!("{} callee edge(s)", edges.len());
                }
            }
            CodeCommands::Edges { chunk_id } => {
                let edges = ops.get_edges_by_chunk(chunk_id)?;
                if cli.json {
                    info!("{}", serde_json::to_string_pretty(&edges)?);
                } else {
                    for e in &edges {
                        info!(
                            "{}#{} -> {}#{}",
                            e.from_slug, e.from_symbol, e.to_slug, e.to_symbol
                        );
                    }
                    info!("{} edge(s)", edges.len());
                }
            }
        },

        Commands::File { command } => match command {
            FileCommands::List { slug } => {
                let files = ops.file_list(slug.as_deref(), None)?;
                if cli.json {
                    info!("{}", serde_json::to_string_pretty(&files)?);
                } else if files.is_empty() {
                    if slug.is_some() {
                        info!("No files for page: {}", slug.as_deref().unwrap_or(""));
                    } else {
                        info!("No files stored.");
                    }
                } else {
                    info!("{} file(s):", files.len());
                    for f in &files {
                        let size = if f.size_bytes > 1024 * 1024 {
                            format!("{}MB", f.size_bytes / (1024 * 1024))
                        } else if f.size_bytes > 1024 {
                            format!("{}KB", f.size_bytes / 1024)
                        } else {
                            format!("{}B", f.size_bytes)
                        };
                        info!(
                            "  {} / {}  [{}, {}]",
                            f.slug,
                            f.filename,
                            size,
                            f.mime_type.as_deref().unwrap_or("?")
                        );
                    }
                }
            }

            FileCommands::Upload { path, page } => {
                if !path.exists() {
                    error!(path = %path.display(), "File not found");
                    std::process::exit(1);
                }

                let slug = page.as_deref().unwrap_or("unsorted");
                let opts = FileUploadOptions {
                    slug: slug.to_string(),
                    overwrite: false,
                    max_size_bytes: None,
                };
                let record = ops.file_upload(&path, slug, opts)?;
                if cli.json {
                    info!("{}", serde_json::to_string_pretty(&record)?);
                } else {
                    let size = if record.size_bytes > 1024 * 1024 {
                        format!("{}MB", record.size_bytes / (1024 * 1024))
                    } else if record.size_bytes > 1024 {
                        format!("{}KB", record.size_bytes / 1024)
                    } else {
                        format!("{}B", record.size_bytes)
                    };
                    info!(storage_path = %record.storage_path, size = %size, "Uploaded");
                }
            }

            FileCommands::Sync { dir } => {
                if !dir.exists() {
                    error!(dir = %dir.display(), "Directory not found");
                    std::process::exit(1);
                }

                let result = ops.file_sync(&dir)?;
                if cli.json {
                    info!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    info!(
                        uploaded = result.uploaded,
                        skipped = result.skipped,
                        "Files sync complete"
                    );
                }
            }

            FileCommands::Verify => {
                let result = ops.file_verify()?;
                if cli.json {
                    info!("{}", serde_json::to_string_pretty(&result)?);
                } else if result.mismatches == 0 && result.missing == 0 {
                    info!(
                        verified = result.verified,
                        "Files verified, 0 mismatches, 0 missing"
                    );
                } else {
                    error!(
                        mismatches = result.mismatches,
                        missing = result.missing,
                        "VERIFY FAILED"
                    );
                    std::process::exit(1);
                }
            }

            FileCommands::Url { storage_path } => match ops.file_url_by_path(&storage_path) {
                Ok(url) => {
                    if cli.json {
                        info!(
                            "{}",
                            serde_json::to_string_pretty(&serde_json::json!({
                                "url": url,
                                "storage_path": storage_path
                            }))?
                        );
                    } else {
                        info!(url = %url, "File URL");
                    }
                }
                Err(e) => {
                    error!(storage_path = %storage_path, error = %e, "Failed to get file URL");
                    std::process::exit(1);
                }
            },
        },

        Commands::Extract { mode } => {
            let extract_mode = match mode.to_lowercase().as_str() {
                "links" => ExtractMode::Links,
                "timeline" => ExtractMode::Timeline,
                _ => ExtractMode::All,
            };
            info!(mode = %mode, "Starting batch extraction");
            let result = ops.extract(extract_mode)?;
            if cli.json {
                info!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                info!(
                    links_added = result.links_added,
                    timeline_added = result.timeline_added,
                    errors = result.errors,
                    pages_scanned = result.pages_scanned,
                    "Extract complete"
                );
            }
        }

        Commands::KbEval { library_id } => {
            let conn = engine.connection()?;
            let queries = gbrain_core::kb::eval::list_eval_queries(conn, library_id)?;
            println!(
                "Eval queries for library {}: {} found",
                library_id,
                queries.len()
            );
            for q in &queries {
                println!("  [{}] {}: {}", q.query_type, q.query_text, q.id);
            }
            return Ok(());
        }
        Commands::KbBackup { output } => {
            let output_dir = std::path::Path::new(&output);
            let db_path = config.db_path();
            let dest = gbrain_core::kb::backup::backup_database(&db_path, output_dir)?;
            // FIX9-10: 同时备份 storage 目录
            let storage_dir = config
                .kb_storage_dir
                .as_deref()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| gbrain_core::config::Config::base_dir().join("kb_files"));
            if storage_dir.exists() {
                gbrain_core::kb::backup::backup_storage(&storage_dir, output_dir)?;
            }
            // 备份 artifact store 目录（使用统一 resolver，与上传保持一致）
            let artifact_dir = config.artifact_dir();
            if artifact_dir.exists() {
                gbrain_core::kb::backup::backup_artifact_store(&artifact_dir, output_dir)?;
            }
            println!("Backup complete: {}", dest.display());
            return Ok(());
        }
        Commands::KbRestore { input } => {
            let input_dir = std::path::Path::new(&input);
            let db_path = config.db_path();
            // P1 修复：Windows 下打开的 SQLite 文件不能被 rename，必须先断开连接
            engine.disconnect()?;
            gbrain_core::kb::backup::restore_database(&input_dir.join("gbrain.db"), &db_path)?;
            engine.connect()?;
            engine.init_schema()?;
            // FIX9-10: 同时恢复 storage 目录
            let storage_dir = config
                .kb_storage_dir
                .as_deref()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| gbrain_core::config::Config::base_dir().join("kb_files"));
            if input_dir.join("storage").exists() {
                gbrain_core::kb::backup::restore_storage(input_dir, &storage_dir)?;
            }
            // 恢复 artifact store 目录（使用统一 resolver，与上传保持一致）
            let artifact_dir = config.artifact_dir();
            gbrain_core::kb::backup::restore_artifact_store(input_dir, &artifact_dir)?;
            println!("Restore complete");
            return Ok(());
        }
        Commands::KbSourceAdd { library_id, path } => {
            let source_path = std::path::Path::new(&path);
            if !source_path.is_dir() {
                return Err(GBrainError::InvalidInput(format!(
                    "Path is not a directory: {}",
                    path
                )));
            }
            let kb = engine.kb_engine()?;
            let source_id = kb.create_source(
                library_id,
                "local_directory",
                &path,
                source_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("source"),
                "soft_delete",
            )?;
            let files = gbrain_core::kb::sync::scan_directory(
                source_path,
                &["pdf", "docx", "xlsx", "csv", "html", "htm", "txt", "md"],
            )?;
            let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
            let conn = engine.connection()?;
            let mut inserted = 0u32;
            for file in &files {
                let hash = gbrain_core::kb::sync::compute_file_hash(file)?;
                let file_size = std::fs::metadata(file).map(|m| m.len() as i64).unwrap_or(0);
                let item_path = file.to_string_lossy().to_string();
                kb.insert_source_item(source_id, &item_path, &hash, file_size, &now)?;
                // FIX9-17: 自动创建 document 并入队处理，否则添加 source 后不会得到可搜索文档
                let ext = file
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                let run_id = gbrain_core::kb::jobs::new_run_id();
                let doc = gbrain_core::kb::types::Document {
                    library_id,
                    original_name: file
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string(),
                    content_hash: hash.to_string(),
                    file_size,
                    extension: ext.clone(),
                    source_type: "source_sync".to_string(),
                    storage_path: item_path.clone(),
                    original_path: item_path.clone(),
                    processing_run_id: run_id.clone(),
                    ..Default::default()
                };
                let doc_id = kb.create_document(&doc)?;
                kb.update_source_item(
                    source_id,
                    &item_path,
                    None,
                    Some("synced"),
                    None,
                    Some(doc_id),
                    Some(&now),
                )?;
                let job_db_id = gbrain_core::kb::jobs::enqueue_kb_process_job(
                    conn,
                    &gbrain_core::kb::jobs::KbProcessPayload {
                        kind: "kb_process_document".to_string(),
                        document_id: doc_id,
                        library_id,
                        processing_run_id: run_id,
                        storage_path: item_path.clone(),
                        extension: ext,
                    },
                )?;
                kb.update_document_job_id(doc_id, &job_db_id.to_string())?;
                inserted += 1;
            }
            println!(
                "Source added: id={}, {} files registered and queued for library {}",
                source_id, inserted, library_id
            );
            return Ok(());
        }
        Commands::KbSyncSource { source_id } => {
            let kb = engine.kb_engine()?;
            let source = kb.get_source(source_id)?.ok_or_else(|| {
                GBrainError::InvalidInput(format!("Source {} not found", source_id))
            })?;
            let (
                _id,
                library_id,
                _source_type,
                source_uri,
                _display_name,
                delete_policy,
                _sync_status,
            ) = source;
            let source_dir = std::path::Path::new(&source_uri);
            if !source_dir.is_dir() {
                return Err(GBrainError::InvalidInput(format!(
                    "Source directory does not exist: {}",
                    source_uri
                )));
            }
            let files = gbrain_core::kb::sync::scan_directory(
                source_dir,
                &["pdf", "docx", "xlsx", "csv", "html", "htm", "txt", "md"],
            )?;
            let conn = engine.connection()?;
            let scan_results = gbrain_core::kb::sync::incremental_scan(conn, source_id, &files)?;
            let summary = gbrain_core::kb::sync::summarize_scan(&scan_results);
            let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();

            for (file_path, action, new_hash) in &scan_results {
                let item_path = file_path.to_string_lossy().to_string();
                match action {
                    gbrain_core::kb::sync::SyncAction::New => {
                        let hash = new_hash.as_deref().unwrap_or("");
                        let file_size = std::fs::metadata(file_path)
                            .map(|m| m.len() as i64)
                            .unwrap_or(0);
                        kb.insert_source_item(source_id, &item_path, hash, file_size, &now)?;
                        let ext = file_path
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("")
                            .to_lowercase();
                        let run_id = gbrain_core::kb::jobs::new_run_id();
                        let doc = gbrain_core::kb::types::Document {
                            library_id,
                            original_name: file_path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("")
                                .to_string(),
                            content_hash: hash.to_string(),
                            file_size,
                            extension: ext.clone(),
                            source_type: "source_sync".to_string(),
                            storage_path: item_path.clone(),
                            original_path: item_path.clone(),
                            processing_run_id: run_id.clone(),
                            ..Default::default()
                        };
                        let doc_id = kb.create_document(&doc)?;
                        kb.update_source_item(
                            source_id,
                            &item_path,
                            None,
                            Some("synced"),
                            None,
                            Some(doc_id),
                            Some(&now),
                        )?;
                        // FIX9-16: 新文档入队处理，否则不会解析/分块/建索引
                        let job_db_id = gbrain_core::kb::jobs::enqueue_kb_process_job(
                            conn,
                            &gbrain_core::kb::jobs::KbProcessPayload {
                                kind: "kb_process_document".to_string(),
                                document_id: doc_id,
                                library_id,
                                processing_run_id: run_id,
                                storage_path: item_path.clone(),
                                extension: ext,
                            },
                        )?;
                        kb.update_document_job_id(doc_id, &job_db_id.to_string())?;
                    }
                    gbrain_core::kb::sync::SyncAction::Changed => {
                        if let Some(hash) = new_hash {
                            kb.update_source_item(
                                source_id,
                                &item_path,
                                Some(hash),
                                Some("changed"),
                                None,
                                None,
                                Some(&now),
                            )?;
                            // FIX9-16: 变更文档入队重新处理
                            // 查找此 source item 关联的 document_id
                            let doc_id_result: Option<i64> = conn
                                .query_row(
                                    "SELECT document_id FROM kb_source_items \
                                     WHERE source_id = ?1 AND item_path = ?2 AND document_id IS NOT NULL",
                                    rusqlite::params![source_id, &item_path],
                                    |row| row.get(0),
                                )
                                .ok()
                                .flatten();
                            if let Some(doc_id) = doc_id_result {
                                // 同步更新 kb_documents 的 content_hash/file_size/storage_path
                                let file_size = std::fs::metadata(file_path)
                                    .map(|m| m.len() as i64)
                                    .unwrap_or(0);
                                kb.update_document_source_metadata(
                                    doc_id, hash, file_size, &item_path,
                                )?;
                                let run_id = gbrain_core::kb::jobs::new_run_id();
                                kb.update_document_run_id(doc_id, &run_id)?;
                                // 重置文档状态为 queued/pending，避免 UI 状态与实际 job 不一致
                                kb.reset_document_processing(doc_id)?;
                                let job_db_id = gbrain_core::kb::jobs::enqueue_kb_process_job(
                                    conn,
                                    &gbrain_core::kb::jobs::KbProcessPayload {
                                        kind: "kb_process_document".to_string(),
                                        document_id: doc_id,
                                        library_id,
                                        processing_run_id: run_id,
                                        storage_path: item_path.clone(),
                                        extension: file_path
                                            .extension()
                                            .and_then(|e| e.to_str())
                                            .unwrap_or("")
                                            .to_lowercase(),
                                    },
                                )?;
                                kb.update_document_job_id(doc_id, &job_db_id.to_string())?;
                            }
                        }
                    }
                    gbrain_core::kb::sync::SyncAction::Missing => {
                        gbrain_core::kb::sync::apply_delete_policy(
                            conn,
                            &item_path,
                            &delete_policy,
                        )?;
                    }
                    gbrain_core::kb::sync::SyncAction::Unchanged => {}
                }
            }
            println!(
                "Sync source {}: new={}, changed={}, missing={}, unchanged={}",
                source_id,
                summary.new_count,
                summary.changed_count,
                summary.missing_count,
                summary.unchanged_count
            );
            return Ok(());
        }

        Commands::KbJobs { command } => {
            let conn = engine.connection()?;
            match command {
                KbJobsCommand::List { library_id } => {
                    let jobs = gbrain_core::kb::jobs::list_kb_jobs(conn, library_id)?;
                    println!("KB jobs: {} found", jobs.len());
                    for job in &jobs {
                        println!("  id={} status={} document_id={}", job.0, job.1, job.2);
                    }
                }
                KbJobsCommand::Pause { library_id } => {
                    gbrain_core::kb::jobs::pause_library_jobs(conn, library_id)?;
                    println!("Paused KB jobs for library {}", library_id);
                }
                KbJobsCommand::Resume { library_id } => {
                    gbrain_core::kb::jobs::resume_library_jobs(conn, library_id)?;
                    println!("Resumed KB jobs for library {}", library_id);
                }
            }
            return Ok(());
        }

        Commands::KbExportLibrary { library_id, output } => {
            let conn = engine.connection()?;
            let output_dir = std::path::Path::new(&output);
            // FIX9-13: 传入 storage_dir 以复制源文件
            let storage_dir = config
                .kb_storage_dir
                .as_deref()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| gbrain_core::config::Config::base_dir().join("kb_files"));
            let storage_ref: Option<&std::path::Path> = if storage_dir.exists() {
                Some(&storage_dir)
            } else {
                None
            };
            let manifest =
                gbrain_core::kb::backup::export_library(conn, library_id, output_dir, storage_ref)?;
            println!(
                "Exported library '{}' ({} docs, {} nodes) to {}",
                manifest.source_library_name, manifest.document_count, manifest.node_count, output
            );
            return Ok(());
        }

        Commands::KbImportLibrary { archive, new_name } => {
            let conn = engine.connection()?;
            let archive_dir = std::path::Path::new(&archive);
            // FIX9-13: 传入 target_storage_dir 以重写 storage_path
            let storage_dir = config
                .kb_storage_dir
                .as_deref()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| gbrain_core::config::Config::base_dir().join("kb_files"));
            let storage_ref: Option<&std::path::Path> = if storage_dir.exists() {
                Some(&storage_dir)
            } else {
                None
            };
            let new_lib_id = gbrain_core::kb::backup::import_library(
                conn,
                archive_dir,
                new_name.as_deref(),
                storage_ref,
            )?;
            println!("Imported library, new library_id={}", new_lib_id);
            return Ok(());
        }

        Commands::KbReembed {
            library_id,
            embedding_index_id,
        } => {
            let conn = engine.connection()?;
            let index_id = embedding_index_id.unwrap_or(0);
            gbrain_core::kb::embedding_index::queue_reembed_jobs(conn, library_id, index_id)?;
            println!(
                "Queued re-embed jobs for library {} (index_id={})",
                library_id, index_id
            );
            return Ok(());
        }

        Commands::KbEvalCompare {
            index_id_1,
            index_id_2,
        } => {
            let conn = engine.connection()?;
            let report =
                gbrain_core::kb::eval::compare_embedding_indexes(conn, index_id_1, index_id_2)?;
            println!("Embedding index comparison:\n{}", report);
            return Ok(());
        }

        Commands::KbHealthCheck { library_id, repair } => {
            let conn = engine.connection()?;
            let summary = gbrain_core::kb::health::check_index_health(conn)?;
            println!(
                "Health: {} ({} issues)",
                summary.overall_status, summary.issues_count
            );
            for check in &summary.checks {
                println!(
                    "  {} [{}]: {} (affected: {})",
                    check.check_name, check.status, check.detail, check.affected_count
                );
            }
            if repair && summary.issues_count > 0 {
                let repaired = gbrain_core::kb::health::repair_fts(conn)?;
                println!("Repaired {} FTS entries", repaired);
            }
            let _ = library_id;
            return Ok(());
        }

        Commands::KbRebuildDocument { document_id } => {
            let conn = engine.connection()?;
            gbrain_core::kb::health::rebuild_document_index(conn, document_id)?;
            println!("Rebuild queued for document {}", document_id);
            return Ok(());
        }

        Commands::KbRebuildLibrary { library_id } => {
            let conn = engine.connection()?;
            gbrain_core::kb::health::rebuild_library_index(conn, library_id)?;
            println!("Rebuild queued for library {}", library_id);
            return Ok(());
        }

        Commands::KbPurgeDeleted {
            library_id,
            older_than_days,
        } => {
            let conn = engine.connection()?;
            let purged = gbrain_core::kb::health::purge_deleted(conn, older_than_days)?;
            println!(
                "Purged {} deleted documents older than {} days",
                purged, older_than_days
            );
            let _ = library_id;
            return Ok(());
        }

        // ========================================================================
        // 单入口多投影融合架构 — Upload / MemoryQuery / Promotion / Artifact
        // ========================================================================
        Commands::Upload {
            path,
            intent,
            library_id,
            target,
            page,
            folder_id,
            promotion,
            dry_run,
            json,
        } => {
            if !path.exists() {
                error!(path = %path.display(), "File not found");
                std::process::exit(1);
            }

            let content = std::fs::read(&path)?;
            let original_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            let upload_intent = match intent.to_lowercase().as_str() {
                "auto" => gbrain_core::artifact::types::UploadIntent::Auto,
                "document" => gbrain_core::artifact::types::UploadIntent::Document,
                "attachment" => gbrain_core::artifact::types::UploadIntent::Attachment,
                "memory" => gbrain_core::artifact::types::UploadIntent::Memory,
                "promote" => gbrain_core::artifact::types::UploadIntent::Promote,
                _ => gbrain_core::artifact::types::UploadIntent::Auto,
            };

            // 修复：仅在用户显式指定时转换为 PromotionPolicy，否则让 intent 推断
            let promotion_policy = promotion.as_ref().map(|p| match p.to_lowercase().as_str() {
                "none" => gbrain_core::artifact::types::PromotionPolicy::None,
                "shadow" => gbrain_core::artifact::types::PromotionPolicy::Shadow,
                "candidate" => gbrain_core::artifact::types::PromotionPolicy::Candidate,
                "auto-low-risk" | "auto_accept_low_risk" => {
                    gbrain_core::artifact::types::PromotionPolicy::AutoAcceptLowRisk
                }
                _ => gbrain_core::artifact::types::PromotionPolicy::Candidate,
            });

            let input = gbrain_core::artifact::types::UploadSourceInput {
                content,
                path: Some(path.clone()),
                original_name,
                source_kind: gbrain_core::artifact::types::SourceKind::Upload,
                source_uri: path.to_string_lossy().to_string(),
                intent: upload_intent,
                target_slug: target,
                page_slug: page,
                library_id,
                folder_id,
                promotion_policy,
                owner_ref: None,
                metadata: None,
                dry_run,
            };

            let result = ops.upload_source(input)?;
            if json {
                info!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                info!(
                    "Artifact: {} (uid={}, sha256={})",
                    result.artifact_id, result.artifact_uid, result.sha256
                );
                info!(
                    "Occurrence: {} (uid={})",
                    result.occurrence_id, result.occurrence_uid
                );
                info!("New: {}", result.is_new);
                info!(
                    "Route: KB={}, Brain={}, File={}, Shadow={}",
                    result.route_plan.to_kb,
                    result.route_plan.to_brain,
                    result.route_plan.to_file,
                    result.route_plan.to_shadow
                );
                info!("Promotion: {}", result.route_plan.promotion);
                for proj in &result.projections {
                    info!(
                        "  Projection: {} key={} ref={} created={} status={}",
                        proj.projection_type,
                        proj.projection_key,
                        proj.projection_ref,
                        proj.created,
                        proj.status
                    );
                }
            }
        }

        Commands::MemoryQuery {
            query,
            strategy,
            limit,
            filter_slug,
            include_evidence,
            include_provenance,
            json,
        } => {
            let query_strategy = match strategy.to_lowercase().as_str() {
                "brain_first" => gbrain_core::artifact::types::QueryStrategy::BrainFirst,
                "evidence_first" => gbrain_core::artifact::types::QueryStrategy::EvidenceFirst,
                "provenance" => gbrain_core::artifact::types::QueryStrategy::Provenance,
                "timeline_first" => gbrain_core::artifact::types::QueryStrategy::TimelineFirst,
                _ => gbrain_core::artifact::types::QueryStrategy::BrainFirst,
            };

            let input = gbrain_core::artifact::types::UnifiedQueryInput {
                query,
                strategy: query_strategy,
                limit: Some(limit),
                filter_slug,
                include_evidence,
                include_provenance,
            };

            let result = ops.memory_query(input)?;
            if json {
                info!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                info!("Strategy: {}", result.strategy);
                info!("Total hits: {}", result.total_hits);
                for hit in &result.brain_hits {
                    info!(
                        "  Brain: {} | {} | {:.3}",
                        hit.slug, hit.title, hit.relevance
                    );
                }
                for hit in &result.evidence_hits {
                    info!(
                        "  KB: doc_id={} | {} | {:.3}",
                        hit.kb_document_id, hit.title, hit.relevance
                    );
                }
                for prov in &result.provenance_records {
                    info!(
                        "  Provenance: slug={} field={} hash={}",
                        prov.brain_slug, prov.brain_field, prov.fact_hash
                    );
                }
                for hit in &result.timeline_hits {
                    info!(
                        "  Timeline: {} | {} | artifact_id={}",
                        hit.event_date, hit.description, hit.artifact_id
                    );
                }
            }
        }

        Commands::PromotionList {
            status,
            candidate_type,
            target_slug,
            limit,
            json,
        } => {
            let candidates = ops.promotion_list_candidates(
                status.as_deref(),
                candidate_type.as_deref(),
                target_slug.as_deref(),
                limit,
                0,
            )?;
            if json {
                info!("{}", serde_json::to_string_pretty(&candidates)?);
            } else {
                for c in &candidates {
                    info!(
                        "  [{}] {} type={} target={} risk={} confidence={:.2}",
                        c.status, c.id, c.candidate_type, c.target_slug, c.risk_level, c.confidence
                    );
                }
                info!("{} candidates", candidates.len());
            }
        }

        Commands::PromotionGet { candidate_id } => {
            let candidate = ops.promotion_get_candidate(candidate_id)?;
            match candidate {
                Some(c) => info!("{}", serde_json::to_string_pretty(&c)?),
                None => warn!("Candidate {} not found", candidate_id),
            }
        }

        Commands::PromotionAccept {
            candidate_id,
            reviewer,
            notes,
        } => {
            let input = gbrain_core::artifact::types::ReviewCandidateInput {
                candidate_id,
                action: "accept".to_string(),
                reviewer,
                notes,
            };
            let result = ops.promotion_review_candidate(input)?;
            info!("Candidate {} accepted by {}", result.id, result.reviewer);
        }

        Commands::PromotionReject {
            candidate_id,
            reviewer,
            notes,
        } => {
            let input = gbrain_core::artifact::types::ReviewCandidateInput {
                candidate_id,
                action: "reject".to_string(),
                reviewer,
                notes,
            };
            let result = ops.promotion_review_candidate(input)?;
            info!("Candidate {} rejected by {}", result.id, result.reviewer);
        }

        Commands::PromotionApply { candidate_id } => {
            let result = ops.promotion_apply_candidate(candidate_id)?;
            info!("Candidate {} applied", result.id);
        }

        Commands::PromotionAutoApply { artifact_id } => {
            let applied = ops.promotion_auto_apply(artifact_id)?;
            info!(
                "Auto-applied {} low-risk candidates for artifact {}",
                applied.len(),
                artifact_id
            );
        }

        Commands::PromotionBatchApply {
            artifact_id,
            risk,
            dry_run,
        } => {
            let result = ops.promotion_batch_apply(artifact_id, risk.as_deref(), dry_run)?;
            if dry_run {
                info!(
                    "Dry run: {} candidates would be applied",
                    result.total_candidates
                );
                for c in &result.candidates {
                    info!("  {}", c);
                }
            } else {
                info!(
                    "Batch apply: total={}, applied={}, failed={}",
                    result.total_candidates, result.applied, result.failed
                );
                for f in &result.failures {
                    info!("  FAIL: {}", f);
                }
            }
        }

        Commands::PromotionRollback { candidate_id } => {
            let result = ops.promotion_rollback_candidate(candidate_id)?;
            info!(
                "Candidate {} rolled back (was: {})",
                candidate_id, result.status
            );
        }

        Commands::GcOrphanProjections {
            stale_days,
            dry_run,
        } => {
            let result = ops.gc_orphan_projections(stale_days, dry_run)?;
            if dry_run {
                info!(
                    "Dry run: {} orphaned projections found, {} stale records would be deleted",
                    result.orphaned_count, result.deleted_count
                );
            } else {
                info!(
                    "GC complete: orphaned={}, deleted={}, KB cleaned={}, shadow pages cleaned={}",
                    result.orphaned_count,
                    result.deleted_count,
                    result.kb_vector_cleaned,
                    result.shadow_page_cleaned
                );
                for e in &result.errors {
                    info!("  ERROR: {}", e);
                }
            }
        }

        Commands::ProjectionSupersede {
            old_proj_id,
            new_proj_id,
        } => {
            ops.supersede_projection(old_proj_id, new_proj_id)?;
            info!("Projection superseded: {} -> {}", old_proj_id, new_proj_id);
        }

        Commands::ProjectionHistory {
            projection_key,
            artifact_id,
            projection_type,
            limit,
        } => {
            let history = ops.get_projection_history(
                &projection_key,
                artifact_id,
                projection_type.as_deref(),
                limit,
            )?;
            if history.is_empty() {
                info!("No projection history found for key: {}", projection_key);
            } else {
                info!("Projection history for '{}':", projection_key);
                for p in &history {
                    info!(
                        "  id={}, type={}, ref={}, status={}, superseded_by={:?}",
                        p.id, p.projection_type, p.projection_ref, p.status, p.superseded_by
                    );
                }
            }
        }

        Commands::ArtifactList {
            limit,
            offset,
            json,
        } => {
            let artifacts = ops.list_artifacts(limit, offset)?;
            if json {
                info!("{}", serde_json::to_string_pretty(&artifacts)?);
            } else {
                for a in &artifacts {
                    info!(
                        "  [{}] {} uid={} sha256={} size={} status={}",
                        a.id,
                        a.original_name,
                        a.artifact_uid,
                        &a.sha256[..16.min(a.sha256.len())],
                        a.size_bytes,
                        a.status
                    );
                }
                info!("{} artifacts", artifacts.len());
            }
        }

        Commands::ArtifactGet { id_or_uid } => {
            let artifact = if id_or_uid.starts_with("art_") {
                ops.get_artifact_by_uid(&id_or_uid)?
            } else {
                let id = id_or_uid.parse::<i64>().ok();
                match id {
                    Some(id) => ops.get_artifact(id)?,
                    None => None,
                }
            };
            match artifact {
                Some(a) => {
                    info!("{}", serde_json::to_string_pretty(&a)?);
                    // 显示投影
                    let projections = ops.get_artifact_projections(a.id)?;
                    for p in &projections {
                        info!(
                            "  Projection: {} key={} ref={} status={}",
                            p.projection_type, p.projection_key, p.projection_ref, p.status
                        );
                    }
                }
                None => warn!("Artifact '{}' not found", id_or_uid),
            }
        }

        Commands::ArtifactDelete { artifact_id } => {
            ops.delete_artifact(artifact_id)?;
            info!("Artifact {} soft-deleted", artifact_id);
        }

        Commands::ArtifactHealth => {
            let report = ops.artifact_health_check()?;
            info!("{}", serde_json::to_string_pretty(&report)?);
        }

        Commands::KbWorker { once, interval } => {
            if once {
                let processed = gbrain_core::kb::run_kb_worker_once(&engine, config)?;
                if processed {
                    info!("KB worker: processed one job");
                } else {
                    info!("KB worker: no pending jobs");
                }
                // 同时处理 artifact promotion 作业
                let promoted = gbrain_core::kb::run_artifact_promote_worker_once(&engine, config)?;
                if promoted {
                    info!("Artifact promote worker: processed one job");
                }
            } else {
                info!(interval, "KB worker: starting daemon mode");
                gbrain_core::kb::run_kb_worker_loop(&engine, config, interval);
            }
            return Ok(());
        }
    }

    engine.disconnect()?;
    Ok(())
}

fn embed_stale_chunks(
    ops: &Operations<'_>,
    config: &Config,
    batch_size: usize,
    slugs: Option<&[String]>,
) -> Result<usize> {
    let api_key = config.openai_api_key.as_deref().ok_or_else(|| {
        GBrainError::InvalidInput("GBRAIN_OPENAI_API_KEY is required for embedding".to_string())
    })?;
    let embedder = Embedder::new(
        api_key,
        config.openai_base_url.as_deref(),
        Some(&config.embedding_model),
        Some(config.embedding_dimensions),
    );
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| GBrainError::InvalidInput(format!("failed to start async runtime: {}", e)))?;

    let stale_chunks: Vec<StaleChunk> = if let Some(slugs) = slugs {
        let mut rows = Vec::new();
        for slug in slugs {
            for c in ops.engine.get_chunks(slug)? {
                rows.push(StaleChunk {
                    slug: slug.clone(),
                    chunk_id: c.id,
                    chunk_index: c.chunk_index,
                    chunk_text: c.chunk_text,
                    source: c.source,
                    token_count: c.token_count,
                    model: c.model,
                });
            }
        }
        rows
    } else {
        ops.engine.list_stale_chunks(None)?
    };

    info!(
        chunk_count = stale_chunks.len(),
        batch_size, "Starting embed"
    );
    let mut embedded = 0;
    for batch in stale_chunks.chunks(batch_size.max(1)) {
        let texts: Vec<&str> = batch.iter().map(|c| c.chunk_text.as_str()).collect();
        let embeddings = rt
            .block_on(embedder.embed_batch(&texts))
            .map_err(|e| GBrainError::Embedding(e.to_string()))?;
        let mut by_slug: std::collections::HashMap<String, Vec<ChunkInput>> =
            std::collections::HashMap::new();
        for (row, embedding) in batch.iter().zip(embeddings.into_iter()) {
            let existing = ops.engine.get_chunk_by_id(row.chunk_id)?;
            by_slug
                .entry(row.slug.clone())
                .or_default()
                .push(ChunkInput {
                    chunk_index: row.chunk_index,
                    chunk_text: row.chunk_text.clone(),
                    source: row.source.clone(),
                    token_count: row.token_count,
                    embedding: Some(embedding),
                    model: Some(config.embedding_model.clone()),
                    language: existing.as_ref().and_then(|c| c.language.clone()),
                    symbol_name: existing.as_ref().and_then(|c| c.symbol_name.clone()),
                    symbol_type: existing.as_ref().and_then(|c| c.symbol_type.clone()),
                    start_line: existing.as_ref().and_then(|c| c.start_line),
                    end_line: existing.as_ref().and_then(|c| c.end_line),
                    parent_symbol_path: existing
                        .as_ref()
                        .and_then(|c| c.parent_symbol_path.clone()),
                    symbol_name_qualified: existing
                        .as_ref()
                        .and_then(|c| c.symbol_name_qualified.clone()),
                    doc_comment: existing.as_ref().and_then(|c| c.doc_comment.clone()),
                });
        }
        for (slug, chunks) in by_slug {
            embedded += ops.engine.upsert_chunks(&slug, &chunks)?;
        }
    }
    Ok(embedded)
}

fn is_supported_code_file(path: &std::path::Path) -> bool {
    language_from_path(path).is_some()
}

fn language_from_path(path: &std::path::Path) -> Option<&'static str> {
    match path.extension().and_then(|s| s.to_str()).unwrap_or("") {
        "rs" => Some("rust"),
        "ts" => Some("typescript"),
        "tsx" => Some("tsx"),
        "js" | "jsx" | "mjs" | "cjs" => Some("javascript"),
        "py" => Some("python"),
        "go" => Some("go"),
        "java" => Some("java"),
        "c" | "h" => Some("c"),
        "cpp" | "cc" | "cxx" | "hpp" => Some("cpp"),
        _ => None,
    }
}

fn code_slug_from_relative(relative: &std::path::Path) -> String {
    let without_ext = relative.with_extension("");
    let mut segments = Vec::new();
    for segment in without_ext.components() {
        let text = segment.as_os_str().to_string_lossy();
        let slugified = slug_segment(&text);
        if !slugified.is_empty() {
            segments.push(slugified);
        }
    }
    if segments.is_empty() {
        "code/imported".to_string()
    } else {
        format!("code/{}", segments.join("/"))
    }
}

fn slug_segment(value: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in value.chars().flat_map(|c| c.to_lowercase()) {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            out.push(ch);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn should_skip_import_file(path: &std::path::Path) -> bool {
    const MAX_IMPORT_BYTES: u64 = 5 * 1024 * 1024;
    match std::fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                warn!(path = %path.display(), "Skipping symlink during import");
                return true;
            }
            if meta.len() > MAX_IMPORT_BYTES {
                warn!(
                    path = %path.display(),
                    bytes = meta.len(),
                    "Skipping oversized file during import"
                );
                return true;
            }
            false
        }
        Err(e) => {
            warn!(path = %path.display(), error = %e, "Skipping unreadable file during import");
            true
        }
    }
}
