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
    Mcp,

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

    /// Search indexed code chunks
    Search {
        query: String,
        #[arg(long, default_value = "20")]
        limit: usize,
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
    let config = Config::load().unwrap_or_default();
    logging::init(&config);

    if let Err(e) = run(cli, &config) {
        error!("Fatal error: {}", e);
        std::process::exit(1);
    }
}

fn run(cli: Cli, config: &Config) -> Result<()> {
    let db_path = cli
        .db
        .unwrap_or_else(|| config.db_path().to_str().unwrap_or("brain.db").to_string());

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

        Commands::Query { query, limit } => {
            let opts = SearchOpts {
                limit: Some(limit),
                detail_level: Some(DetailLevel::Medium),
                ..Default::default()
            };

            let results = ops.query(&query, opts)?;
            if cli.json {
                info!("{}", serde_json::to_string_pretty(&results)?);
            } else {
                for result in &results {
                    info!("{} | {} | {:.3}", result.slug, result.title, result.score);
                    if !result.chunk_text.is_empty() {
                        info!(
                            "  {}",
                            result.chunk_text.chars().take(100).collect::<String>()
                        );
                    }
                }
                info!("{} results", results.len());
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

        Commands::Mcp => {
            info!("Starting MCP stdio server");
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
            let mut dirs = vec![import_dir.to_path_buf()];
            while let Some(current) = dirs.pop() {
                for entry in std::fs::read_dir(&current)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_dir() {
                        dirs.push(path);
                    } else if path.extension().and_then(|s| s.to_str()) == Some("md") {
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
                        count += 1;
                    }
                }
            }
            info!("Imported {} pages from {}", count, dir);
            if do_embed {
                info!("Embed flag set — use 'gbrain embed' separately");
            }
            if auto_link {
                info!("Auto-link enabled — links extracted during put_page");
            }
        }

        Commands::Embed { batch_size, slugs } => {
            let api_key = config.openai_api_key.as_deref().ok_or_else(|| {
                GBrainError::InvalidInput(
                    "GBRAIN_OPENAI_API_KEY is required for embedding".to_string(),
                )
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
                .map_err(|e| {
                    GBrainError::InvalidInput(format!("failed to start async runtime: {}", e))
                })?;
            let stale_chunks: Vec<StaleChunk> = if slugs.is_empty() {
                ops.engine.list_stale_chunks(None)?
            } else {
                let mut rows = Vec::new();
                for slug in &slugs {
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
            };
            info!(
                chunk_count = stale_chunks.len(),
                batch_size, "Starting embed"
            );
            let target_chunk_count = stale_chunks.len();
            let mut embedded = 0;
            for batch in stale_chunks.chunks(batch_size.max(1)) {
                let texts: Vec<&str> = batch.iter().map(|c| c.chunk_text.as_str()).collect();
                let embeddings = rt
                    .block_on(embedder.embed_batch(&texts))
                    .map_err(|e| GBrainError::Embedding(e.to_string()))?;
                let mut by_slug: std::collections::HashMap<String, Vec<ChunkInput>> =
                    std::collections::HashMap::new();
                for (row, embedding) in batch.iter().zip(embeddings.into_iter()) {
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
                            language: None,
                            symbol_name: None,
                            symbol_type: None,
                            start_line: None,
                            end_line: None,
                        });
                }
                for (slug, chunks) in by_slug {
                    embedded += ops.engine.upsert_chunks(&slug, &chunks)?;
                }
            }
            info!(embedded, target_chunk_count, "Embed complete");
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
            CodeCommands::Search { query, limit } => {
                let results = ops.search_keyword_chunks(
                    &query,
                    SearchOpts {
                        limit: Some(limit),
                        page_type: Some(PageType::Code),
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
    }

    engine.disconnect()?;
    Ok(())
}
