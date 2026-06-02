# gbrain-rs

中文 | [English](./README_EN.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)

**Personal Knowledge Brain Engine** — Rust port of [gbrain](https://github.com/garrytan/gbrain), with Single-Entry Multi-Projection Fusion Architecture (Artifact originals → KB/Shadow Pages/Candidate Changes/Attachments multi-projection + provenance audit + rollback), a KB subsystem (async document processing pipeline + RAPTOR recursive summarization enabled by default for new libraries), full Chinese NLP support (jieba tokenization + pinyin + FTS5 query rewriting), soft-delete lifecycle, time-decay search, and more. Built on embedded SQLite + FTS5; vector retrieval uses sqlite-vec when available and otherwise falls back to built-in BLOB storage, with no external database required.

> The original TypeScript version was developed by [Garry Tan](https://github.com/garrytan). Built with **Vibe coding**.

---

## Quick Start

```bash
# 1. Build and install
cargo install --path .

# 2. Initialize a knowledge base
gbrain init

# 3. Write to long-term memory
gbrain put people/alice --title "Alice" --content "An engineer skilled in Rust and systems programming"

# 4. Query knowledge
gbrain query "who is Alice"

# 5. Start MCP server (for AI agent integration)
gbrain serve
```

Keyword retrieval and local storage require no database or external service setup. Embeddings, query expansion, and reranking are optional and activate only on the corresponding lower-level or KB retrieval paths when API keys are configured.

---

## Features

- **Retrieval** — Unified facade queries currently use FTS5 keyword retrieval; the lower-level hybrid API can fuse keyword and optional vector results with RRF and accept expanded queries, while fuzzy trigram matching is exposed as a separate API
- **Knowledge Graph** — Wiki-link extraction, typed links, graph traversal, backlink symmetry verification
- **KB Subsystem** — Async document processing pipeline, with RAPTOR enabled by default for new libraries and executed when model configuration and library policy allow it; semantic chunking remains library-configured; includes multi-format parsers (Markdown/PDF/DOCX/XLSX/CSV/HTML/plaintext), automatic page-level PDF OCR detection/writeback, and automatic OCR import for uploaded JPG/PNG images; code-page indexing is a separate path
- **Chinese NLP** — jieba tokenization + pinyin + prefix wildcards, FTS5 query auto-rewriting, Chinese punctuation sentence-breaking and token counting, pre-tokenized column auto-sync
- **Single-Entry Multi-Projection Fusion** — Artifact uploads create KB document, shadow-page, page-update, or attachment projections, with link and timeline changes flowing through candidate review; includes provenance audit, promotion, version chains, rollback, and four internal Memory Query strategies
- **MCP Server** — Full Model Context Protocol (JSON-RPC 2.0) server, exposing Artifact facade and KB OCR tools
- **Zero Config** — Embedded SQLite, no external services required (embeddings optional)
- **Layered Enrichment** — Automatic entity detection and promotion (mention → stub → enriched)
- **Version History** — Full page versioning with rollback
- **Autopilot** — Self-maintenance daemon thread, auto-runs in background when `gbrain serve` starts. Periodically embeds stale content and runs integrity checks (default every 3600s, configurable via `GBRAIN_AUTOPILOT_INTERVAL`, at least 60s, disable via `GBRAIN_AUTOPILOT_ENABLED`)
- **Safety Guards** — Path traversal protection, slug validation, MCP remote-file confinement, parameterized queries against SQL injection
- **Code Knowledge Graph** — Code pages imported or reindexed as code can use Tree-sitter AST chunking and regex symbol indexing for definitions, references, and call graphs (Rust/TypeScript/JavaScript/Python/Go/Java/C/C++)
- **Audio Transcription Module** — The library API supports Groq Whisper (default) or OpenAI Whisper; no CLI/MCP command is currently exposed
- **Writer Validation Library API** — `BrainWriter` provides Strict/Lint/Off modes; the unified Artifact CLI/MCP interface does not expose mode selection
- **Soft-Delete Lifecycle** — Artifact deletion and restore are exposed; permanent deletion of Artifact sources is not implemented, while pages and KB documents have separate cleanup paths

---

## Build & Install

```bash
cargo build --release          # Build
cargo install --path .         # Install to ~/.cargo/bin/
gbrain init                    # Initialize knowledge base to ~/.gbrain/
```


---

## Data Directory

As features are used, the `~/.gbrain/` directory may contain:

```
~/.gbrain/
  brain.db           # SQLite database (FTS5 + vector BLOB fallback; sqlite-vec when available)
  config.json        # Runtime config (generated via gbrain config set)
  bin/               # Executable copy (copied during gbrain init)
  artifacts/         # Artifact original file storage (named by SHA256; KB documents reference this store)
  cache/             # Cache directory
  logs/              # Log files
    gbrain.log
```

KB documents created by Artifact upload reference originals in `artifacts/`; KB metadata is stored in `brain.db`. Customize the root directory via the `GBRAIN_DIR` environment variable.

---

## CLI Commands

### Global Options

| Option | Description |
|--------|-------------|
| `--db <PATH>` | Database path |
| `--json` | Output as JSON |
| `--dry-run` | Preview operations without executing |

Global options must precede the subcommand, for example `gbrain --json query "Alice"`.

### Knowledge Operations

| Command | Description |
|---------|-------------|
| `gbrain init` | Initialize a new knowledge base |
| `gbrain put <slug> [--title <TITLE>] [--content <TEXT> \| --file <PATH>] [--intent <INTENT>] [--dry-run] [--force]` | Write to long-term memory (intent: memory/evidence/promote) |
| `gbrain upload <path> [--intent <INTENT>] [--target <SLUG>] [--page <SLUG>] [--library <ID>] [--folder <ID>] [--promotion <POLICY>] [--dry-run]` | Upload file as knowledge source |
| `gbrain query <query> [--mode <MODE>] [--limit <N>] [--filter <SLUG>] [--include-sources]` | Unified knowledge query (mode: auto/memory/evidence/timeline) |
| `gbrain list [--limit <N>] [--offset <N>]` | List knowledge sources |
| `gbrain get <id_or_uid> [--include-projections] [--include-sources]` | Get knowledge source details |
| `gbrain delete <id_or_uid> [--dry-run]` | Soft-delete a knowledge source |
| `gbrain detach <id_or_uid> --from <slug> [--dry-run]` | Detach knowledge source from page |
| `gbrain restore <id_or_uid> [--dry-run]` | Restore a soft-deleted knowledge source |
| `gbrain reprocess <id_or_uid> [--dry-run]` | Reprocess knowledge source |
| `gbrain health` | Check knowledge source consistency |
| `gbrain kb ocr-status <doc_id>` | View KB document OCR status |
| `gbrain kb ocr-run <doc_id> [--pages]` | Trigger or enqueue OCR manually |
| `gbrain kb ocr-retry <doc_id>` | Retry failed or empty-result OCR pages |
| `gbrain serve` | Run as MCP stdio server |

#### Examples

```bash
# Initialize a knowledge base
gbrain init

# ===== Write =====
# Write to long-term memory (default intent: memory)
gbrain put people/alice --title "Alice" --content "An engineer skilled in Rust and systems programming"

# Write from file
gbrain put docs/guide --file ./guide.md --intent memory

# Preview write routing (dry-run)
gbrain put people/bob --content "Product manager" --dry-run

# Force overwrite a human-modified page
gbrain put people/alice --content "Updated content" --force

# ===== Upload =====
# Upload document with auto-routing
gbrain upload report.pdf --intent evidence

# Upload and associate with a specific page
gbrain upload note.txt --page people/alice --intent attachment

# Upload to specific KB library and folder
gbrain upload paper.pdf --library 1 --folder 2 --intent evidence

# Upload with promotion policy
gbrain upload document.md --intent memory --promotion auto-low-risk

# Preview upload routing
gbrain upload data.csv --dry-run

# ===== Query =====
# Unified knowledge query
gbrain query "who is Alice"

# Query by mode
gbrain query "Rust async" --mode memory
gbrain query "market analysis" --mode evidence --limit 10
gbrain query "recent updates" --mode timeline

# Filter to a specific page
gbrain query "performance optimization" --filter tech/rust

# Include source tracing
gbrain query "project A progress" --include-sources

# ===== View =====
# List knowledge sources
gbrain list --limit 20

# Get knowledge source details
gbrain get 1
gbrain get art_ab12cd34ef56 --include-projections --include-sources

# ===== Lifecycle Management =====
# Soft-delete a knowledge source
gbrain delete 5

# Preview deletion impact
gbrain delete 5 --dry-run

# Detach a source from a page
gbrain detach 5 --from people/alice

# Restore a deleted source
gbrain restore 5

# Reprocess a source
gbrain reprocess 5

# Health check
gbrain health

# ===== Review =====
# List suggested changes
gbrain review list --status pending

# Filter by status and target
gbrain review list --status applied --target people/alice

# View suggested change details
gbrain review show 1

# Apply a suggested change
gbrain review apply 1

# Reject a suggested change
gbrain review reject 2 --reason "Information outdated"

# Rollback an applied change
gbrain review rollback 1

# ===== Config =====
# View all configuration
gbrain config show

# Get single config value
gbrain config get embedding_model

# Set a config value
gbrain config set chunk_size 800
gbrain config set log_level debug

# ===== MCP Server =====
# Start MCP stdio server
gbrain serve

# ===== Advanced Usage =====
# Custom database path
gbrain --db /path/to/custom/brain.db init
gbrain --db /path/to/custom/brain.db put people/alice --content "Hello"

# JSON output (for scripting)
gbrain --json query "Alice"
gbrain --json get 1 --include-projections
gbrain --json health
gbrain --json review list --status pending

# Dry-run previews (all supporting commands)
gbrain put people/bob --content "test" --dry-run
gbrain --json upload report.pdf --dry-run
gbrain delete 5 --dry-run
gbrain detach 5 --from people/alice --dry-run
gbrain restore 5 --dry-run
gbrain reprocess 5 --dry-run

# ===== Intent-driven Workflows =====
# evidence: KB document evidence only, no brain page
gbrain put research/findings --content "Experiment data shows..." --intent evidence

# promote: shadow page + KB + candidates (requires review)
gbrain put people/new-hire --content "New hire info..." --intent promote

# upload promote + auto-accept low-risk
gbrain upload meeting-notes.md --intent promote --promotion auto-low-risk --target people/alice

# ===== KB OCR =====
# Inspect status, run OCR for selected pages, and retry failed or empty pages
gbrain kb ocr-status 1
gbrain kb ocr-run 1 --pages "1,3,5-10"
gbrain kb ocr-retry 1
```

### Review Operations

| Command | Description |
|---------|-------------|
| `gbrain review list [--status <STATUS>] [--target <SLUG>] [--limit <N>]` | List suggested changes |
| `gbrain review show <change_id>` | Show suggested change details |
| `gbrain review apply <change_id>` | Apply a suggested change |
| `gbrain review reject <change_id> [--reason <TEXT>]` | Reject a suggested change |
| `gbrain review rollback <change_id>` | Rollback an applied suggested change |

#### Examples

```bash
# List suggested changes
gbrain review list --status pending

# Apply a suggested change
gbrain review apply 1
```

### `gbrain config`

| Subcommand | Description |
|------------|-------------|
| `gbrain config show` | Show common config values (quick overview of 15 core items) |
| `gbrain config get <key>` | Get a single config value (supports all 29 keys listed below) |
| `gbrain config set <key> <value>` | Set a config value (also accepts two set-only OCR threshold keys) |

> **Note:** `config show` only displays the 15 most used core keys; `config get <key>` can access all 29 config items listed in the table below. `config set` also accepts `ocr_text_density_threshold` and `ocr_timeout_seconds_per_page`, which are saved to `config.json` but are not currently returned by `config get`.

**Available config keys:**

| Key | Type | Description | Default |
|-----|------|-------------|---------|
| `embedding_model` | string | Embedding model name | `text-embedding-3-large` |
| `embedding_dimensions` | integer | Embedding vector dimensions | `1536` |
| `expansion_model` | string | Query expansion LLM model | `gpt-4o-mini` |
| `chunker_model` | string | LLM chunking model (reserved; also used as a RAPTOR fallback model) | `gpt-4o-mini` |
| `chunk_size` | integer | Chunk size (characters) | `500` |
| `chunk_overlap` | integer | Chunk overlap (characters) | `50` |
| `log_level` | string | Log level (trace/debug/info/warn/error) | `info` |
| `log_to_file` | boolean | Enable file logging | `true` |
| `log_to_console` | boolean | Enable console logging | `true` |
| `auto_link` | boolean | Auto-extract links on write | `true` |
| `auto_timeline` | boolean | Auto-extract timeline on write | `true` |
| `post_write_lint` | boolean | Run validator checks after write and log the result | `false` |
| `kb_enabled` | boolean | Enable KB subsystem | `true` |
| `kb_raptor_model` | string | KB RAPTOR LLM model | `gpt-4o-mini` |
| `kb_max_file_size_mb` | integer | KB max file size (MB) | `50` |
| `kb_worker_enabled` | boolean | Enable KB background worker | `true` |
| `kb_worker_poll_interval_secs` | integer | KB worker poll interval (seconds) | `30` |
| `upload_default_promotion_policy` | string | Upload default promotion policy: none/shadow/candidate/auto-low-risk | `candidate` |
| `artifact_default_intent` | string | Artifact default intent: memory/evidence/promote | `memory` |
| `artifact_auto_create_inbox_library` | boolean | Auto-create Inbox library when missing | `true` |
| `artifact_manual_memory_to_kb` | boolean | Write memory intent to KB | `true` |
| `autopilot_enabled` | boolean | Enable autopilot background maintenance | `true` |
| `autopilot_interval_secs` | integer | Autopilot maintenance interval (seconds, min 60) | `3600` |
| `ocr_enabled` | boolean | Enable PDF/image OCR | `true` |
| `ocr_base_url` | string | GLM-OCR endpoint | `https://open.bigmodel.cn/api/paas/v4/layout_parsing` |
| `ocr_model` | string | OCR model | `glm-ocr` |
| `ocr_mode` | string | OCR selection mode: auto/all_pages | `auto` |
| `ocr_submit_mode` | string | OCR submission mode: pdf_first/pdf_range | `pdf_first` |
| `ocr_profile` | string | OCR profile: general/table/formula/handwriting | `general` |

---

## CLI Command Parameters

### `gbrain put`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `slug` | string | Yes | Target page slug (e.g., people/alice) |
| `--title` | string | No | Page title (optional, inferred from slug by default) |
| `--content` | string | No | Direct text content (alternative to --file) |
| `--file` | path | No | Read content from text file (alternative to --content; txt/md/csv/json/yaml etc., max 1MB) |
| `--intent` | string | No | Intent: memory(default, stable brain page+optional KB+auto-apply low-risk), evidence(KB only), promote(shadow page+KB+candidates) |
| `--force` | flag | No | Force overwrite of human-edited pages (default returns conflict) |
| `--dry-run` | flag | No | Return routing plan only, don't write |

### `gbrain upload`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | path | Yes | File path |
| `--intent` | string | No | Upload intent: auto(automatic routing), evidence(alias document), memory, attachment, promote (default auto) |
| `--target` | string | No | Target gbrain page slug (for promotion) |
| `--page` | string | No | Target page slug (for file attachment) |
| `--library` | integer | No | KB library ID |
| `--folder` | integer | No | KB folder ID |
| `--promotion` | string | No | Promotion policy: none/shadow/candidate/auto-low-risk(alias auto) |
| `--dry-run` | flag | No | Return routing plan only, don't execute |

### `gbrain query`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | Yes | Query text |
| `--mode` | string | No | Query mode: auto/memory/evidence/timeline (default auto) |
| `--limit` | integer | No | Max results |
| `--filter` | string | No | Filter by slug |
| `--include-sources` | flag | No | Include source tracing |

### `gbrain list`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `--limit` | integer | No | Max results (default 50) |
| `--offset` | integer | No | Offset |

### `gbrain get`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id_or_uid` | string | Yes | Artifact ID or UID |
| `--include-projections` | flag | No | Include projection details |
| `--include-sources` | flag | No | Include source tracing |

### `gbrain delete`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id_or_uid` | string | Yes | Artifact ID or UID |
| `--dry-run` | flag | No | Preview impact, don't delete |

### `gbrain detach`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id_or_uid` | string | Yes | Artifact ID or UID |
| `--from` | string | Yes | Target page slug |
| `--dry-run` | flag | No | Preview impact, don't execute |

### `gbrain restore`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id_or_uid` | string | Yes | Artifact ID or UID |
| `--dry-run` | flag | No | Preview restore impact, don't execute |

### `gbrain reprocess`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id_or_uid` | string | Yes | Artifact ID or UID |
| `--dry-run` | flag | No | Preview reprocess impact, don't execute |

### `gbrain kb OCR`

| Command/Parameter | Type | Required | Description |
|-------------------|------|----------|-------------|
| `ocr-status <doc_id>` | integer | Yes | View OCR status for a KB document |
| `ocr-run <doc_id>` | integer | Yes | Trigger or enqueue OCR for a KB document |
| `ocr-retry <doc_id>` | integer | Yes | Retry failed or empty-result OCR pages |
| `ocr-run --pages <RANGES>` | string | No | Page ranges to run, e.g. `1,3,5-10` |

### `gbrain review list`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `--status` | string | No | Filter by status: pending/accepted/rejected/applied/rolled_back |
| `--target` | string | No | Filter by target page slug |
| `--limit` | integer | No | Max results (default 50) |

### `gbrain review show`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `change_id` | integer | Yes | Suggested change ID |

### `gbrain review apply`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `change_id` | integer | Yes | Suggested change ID |

### `gbrain review reject`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `change_id` | integer | Yes | Suggested change ID |
| `--reason` | string | No | Rejection reason |

### `gbrain review rollback`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `change_id` | integer | Yes | Suggested change ID |

---

## MCP Integration

gbrain can run as an MCP server for AI tools like Claude, Cursor, etc.

### Start the Server

```bash
gbrain serve
```

### Claude Desktop Configuration

Add to `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "gbrain": {
      "command": "gbrain",
      "args": ["serve"]
    }
  }
}
```

### Cursor Configuration

Add to `.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "gbrain": {
      "command": "gbrain",
      "args": ["serve"]
    }
  }
}
```

### Security Model

The MCP server sets `remote=true` for remote callers, enabling additional security validations:
- Slug format validation (path traversal prevention)
- File read paths must stay within the MCP server process working directory and must not contain symlinks
- Upload file extension, MIME-type, and size validation
- Parameterized queries (SQL injection prevention)
- Filename safety checks

CLI uses `remote=false` and does not apply the MCP-specific working-directory and symlink restrictions.

---

## MCP Tools

gbrain exposes Artifact unified knowledge operation facade tools (`artifact_*`) and KB OCR extension tools via JSON-RPC 2.0 over stdio.

| Tool | Description |
|------|-------------|
| `artifact_put` | Write manual memory (slug + content + intent) |
| `artifact_upload` | Upload file as knowledge source (PDF/DOCX/MD etc.) |
| `artifact_query` | Unified knowledge query (memory/evidence/timeline modes) |
| `artifact_list` | List knowledge sources |
| `artifact_get` | Get knowledge source details (with occurrences/projections/sources) |
| `artifact_delete` | Soft-delete knowledge source (dry-run for impact preview) |
| `artifact_detach` | Remove association between source and a specific page |
| `artifact_restore` | Restore a soft-deleted knowledge source |
| `artifact_reprocess` | Reprocess all projections of a knowledge source |
| `artifact_health` | Knowledge source health check |
| `artifact_review_list` | List suggested changes |
| `artifact_review_get` | Get suggested change details |
| `artifact_review_apply` | Apply a suggested change |
| `artifact_review_reject` | Reject a suggested change |
| `artifact_review_rollback` | Roll back an applied suggested change |

### KB OCR Extensions

| Tool | Description |
|------|-------------|
| `kb_document_status` | View KB document processing and OCR status |
| `kb_ocr_run` | Trigger or enqueue OCR for a KB document |
| `kb_ocr_retry` | Retry failed or empty-result OCR pages |

#### Examples

MCP file requests can only read existing non-symlink files within the server process working directory; the `./imports/...` paths below are relative to the directory where `gbrain serve` is started.

```jsonc
// ===== Write =====
// Write manual memory
{ "tool": "artifact_put", "params": { "slug": "rust-async", "content": "Rust async programming uses async/await syntax...", "intent": "memory" } }

// Write from file
{ "tool": "artifact_put", "params": { "slug": "docs/guide", "file": "./imports/guide.md", "intent": "evidence" } }

// Preview routing plan
{ "tool": "artifact_put", "params": { "slug": "test", "content": "...", "dry_run": true } }

// Force overwrite
{ "tool": "artifact_put", "params": { "slug": "people/alice", "content": "Updated content", "force": true } }
```

```jsonc
// ===== Upload =====
// Upload document (auto-routing)
{ "tool": "artifact_upload", "params": { "path": "./imports/report.pdf", "intent": "auto" } }

// Upload as evidence
{ "tool": "artifact_upload", "params": { "path": "./imports/doc.pdf", "intent": "evidence", "library_id": 1, "folder_id": 2 } }

// Upload and generate suggested changes
{ "tool": "artifact_upload", "params": { "path": "./imports/doc.pdf", "intent": "promote", "target_slug": "people/alice", "promotion": "candidate" } }

// Upload as attachment
{ "tool": "artifact_upload", "params": { "path": "./imports/image.png", "intent": "attachment", "page_slug": "people/alice" } }

// Preview upload routing
{ "tool": "artifact_upload", "params": { "path": "./imports/data.csv", "dry_run": true } }
```

```jsonc
// ===== Query =====
// Unified knowledge query
{ "tool": "artifact_query", "params": { "query": "Rust async programming", "mode": "auto", "limit": 10 } }

// Query with source tracing
{ "tool": "artifact_query", "params": { "query": "Rust async programming", "mode": "memory", "include_sources": true } }

// Query KB evidence
{ "tool": "artifact_query", "params": { "query": "market analysis", "mode": "evidence" } }

// Query by timeline
{ "tool": "artifact_query", "params": { "query": "recent activity", "mode": "timeline" } }

// Filter to a specific page
{ "tool": "artifact_query", "params": { "query": "performance optimization", "filter_slug": "tech/rust" } }
```

```jsonc
// ===== View =====
// List knowledge sources
{ "tool": "artifact_list", "params": { "limit": 20, "offset": 0 } }

// Get knowledge source details (with projections and sources)
{ "tool": "artifact_get", "params": { "id_or_uid": "art_abc123", "include_sources": true, "include_projections": true } }

// Get by ID
{ "tool": "artifact_get", "params": { "id_or_uid": "1" } }
```

```jsonc
// ===== Lifecycle Management =====
// Preview deletion impact
{ "tool": "artifact_delete", "params": { "id_or_uid": "5", "dry_run": true } }

// Soft-delete
{ "tool": "artifact_delete", "params": { "id_or_uid": "5" } }

// Detach from a page
{ "tool": "artifact_detach", "params": { "id_or_uid": "5", "from": "people/alice" } }

// Restore deleted source
{ "tool": "artifact_restore", "params": { "id_or_uid": "5" } }

// Preview restore impact
{ "tool": "artifact_restore", "params": { "id_or_uid": "5", "dry_run": true } }

// Reprocess source
{ "tool": "artifact_reprocess", "params": { "id_or_uid": "5" } }

// Health check
{ "tool": "artifact_health", "params": {} }
```

```jsonc
// ===== Review =====
// List pending suggested changes
{ "tool": "artifact_review_list", "params": { "status": "pending" } }

// Filter by status and target
{ "tool": "artifact_review_list", "params": { "status": "applied", "target_slug": "people/alice", "limit": 50 } }

// View suggested change details
{ "tool": "artifact_review_get", "params": { "change_id": 1 } }

// Apply a suggested change
{ "tool": "artifact_review_apply", "params": { "change_id": 1 } }

// Reject a suggested change
{ "tool": "artifact_review_reject", "params": { "change_id": 2, "reason": "Information outdated" } }

// Rollback an applied change (reverts shadow page update + marks provenance as stale)
{ "tool": "artifact_review_rollback", "params": { "change_id": 1 } }
```

### Write Intent Reference

`artifact_put` and `artifact_upload` control how knowledge enters the system via the `intent` parameter:

| Tool | Valid intent Values | Default | Behavior |
|------|---------------------|---------|----------|
| `artifact_put` | `memory` / `evidence` / `promote` | `memory` | memory=stable brain page+optional KB, evidence=KB only (no brain page), promote=shadow page+KB+candidates |
| `artifact_upload` | `auto` / `evidence`(alias `document`) / `memory` / `attachment` / `promote` | `auto` | auto=smart routing by file type, evidence=KB document evidence, memory=curate into memory, attachment=file only, promote=explicit promotion with review |

### Promotion Policy Reference

`artifact_upload`'s `promotion` parameter controls automation of generating suggested changes from KB evidence:

| Policy | Alias | Description |
|--------|-------|-------------|
| `none` | — | No auto-promotion; no shadows or candidates |
| `shadow` | — | Create shadow pages only, no candidates |
| `candidate` | — | Generate candidates for human review (default) |
| `auto-low-risk` | — | Auto-accept low-risk candidates (entity mentions, link suggestions, etc.); high-risk still needs review |

---

## MCP Tool Parameters

### `artifact_put`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `slug` | string | Yes | Target page slug (e.g., people/alice) |
| `content` | string | No | Direct text content (alternative to file) |
| `file` | string | No | Local text file path (alternative to content; txt/md/csv/json/yaml etc., max 1MB) |
| `title` | string | No | Page title (optional, inferred from slug by default) |
| `intent` | string | No | Intent: memory(default, stable brain page+optional KB+auto-apply low-risk) / evidence(KB only) / promote(shadow page+KB+candidates) |
| `force` | boolean | No | Force overwrite of human-edited pages (default false, returns resolution=conflict on conflict) |
| `dry_run` | boolean | No | Return routing plan only, don't write |

### `artifact_upload`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | string | Yes | Local file path |
| `intent` | string | No | Upload intent: auto / evidence(alias document) / memory / attachment / promote (default auto) |
| `target_slug` | string | No | Target gbrain page slug (for generating suggested changes) |
| `page_slug` | string | No | Associated page slug (for attachments) |
| `library_id` | integer | No | KB library ID (optional, defaults to auto-selecting Inbox) |
| `folder_id` | integer | No | KB folder ID |
| `promotion` | string | No | Promotion policy: none / shadow / candidate / auto-low-risk |
| `dry_run` | boolean | No | Return routing plan only, don't write |

### `artifact_query`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | Yes | Query text |
| `mode` | string | No | Query mode: auto / memory / evidence / timeline (default auto) |
| `limit` | integer | No | Maximum results |
| `filter_slug` | string | No | Filter to specified page slug |
| `include_sources` | boolean | No | Show source tracing (artifact sources and citations) |

### `artifact_list`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `limit` | integer | No | Max results (default 50) |
| `offset` | integer | No | Offset |

### `artifact_get`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id_or_uid` | string | Yes | Artifact ID or UID (e.g., '1' or 'art_ab12cd34ef56') |
| `include_projections` | boolean | No | Include projection details |
| `include_sources` | boolean | No | Include source tracing |
| `include_content` | boolean | No | Include original content |

### `artifact_delete`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id_or_uid` | string | Yes | Artifact ID or UID |
| `dry_run` | boolean | No | Preview deletion impact, don't execute |

### `artifact_detach`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id_or_uid` | string | Yes | Artifact ID or UID |
| `from` | string | Yes | Target page slug |
| `dry_run` | boolean | No | Preview impact, don't execute |

### `artifact_restore`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id_or_uid` | string | Yes | Artifact ID or UID |
| `dry_run` | boolean | No | Preview restore impact, don't execute |

### `artifact_reprocess`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id_or_uid` | string | Yes | Artifact ID or UID |
| `dry_run` | boolean | No | Preview reprocess impact, don't execute |

### `artifact_health`

No parameters.

### `artifact_review_list`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `status` | string | No | Filter by status: pending / accepted / rejected / applied / rolled_back |
| `target_slug` | string | No | Filter by target page slug |
| `limit` | integer | No | Maximum results |

### `artifact_review_get`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `change_id` | integer | Yes | Change ID |

### `artifact_review_apply`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `change_id` | integer | Yes | Change ID |

### `artifact_review_reject`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `change_id` | integer | Yes | Change ID |
| `reason` | string | No | Rejection reason |
| `reviewer` | string | No | Reviewer identifier; defaults to `mcp` |

### `artifact_review_rollback`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `change_id` | integer | Yes | Change ID |

### `kb_document_status`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `document_id` | integer | Yes | KB document ID |

### `kb_ocr_run`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `document_id` | integer | Yes | KB document ID |
| `pages` | string | No | Page ranges, e.g. `1,3,5-10` |

### `kb_ocr_retry`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `document_id` | integer | Yes | KB document ID |
| `pages` | string | No | Retry only the specified page ranges |

### Known Limitations

- **`artifact_query` mode=graph** is not yet implemented. Code graph queries (symbol definitions/references/call relationships) are not available through the artifact facade.

---

## Environment Variables

> **API Compatibility Note**: Except for PDF/image OCR, which uses the Zhipu GLM-OCR `layout_parsing` endpoint, general model and audio calls support only OpenAI-compatible API formats (`/embeddings`, `/chat/completions`, `/audio/transcriptions`). Anthropic/Claude API is not supported. Set the relevant `*_BASE_URL` for compatible services; the OCR endpoint is protected by its own safety gate.

### LLM Configuration Groups

LLM configuration is split by call type and feature:

| Group | Environment Variables | Used By |
|-------|----------------------|---------|
| **Embeddings** | `GBRAIN_OPENAI_API_KEY` / `GBRAIN_OPENAI_BASE_URL` / `GBRAIN_EMBEDDING_MODEL` | Document chunk embedding (vectorization), semantic chunking (paragraph similarity), query vectors |
| **Query Expansion / Reranking** | `GBRAIN_EXPANSION_API_KEY` / `GBRAIN_EXPANSION_BASE_URL` / `GBRAIN_EXPANSION_MODEL` | Search query expansion, search reranking via chat/completions |
| **KB RAPTOR** | Library `raptor_llm_*`, `GBRAIN_KB_RAPTOR_*`, `GBRAIN_EXPANSION_*`, `GBRAIN_CHUNKER_*` | RAPTOR tree summarization |
| **LLM Chunker (reserved)** | `GBRAIN_CHUNKER_API_KEY` / `GBRAIN_CHUNKER_BASE_URL` / `GBRAIN_CHUNKER_MODEL` | Reserved for LLM-guided chunking; not wired into the current KB document pipeline, and also used as a RAPTOR fallback |
| **PDF/Image OCR (GLM-OCR)** | `GBRAIN_OCR_API_KEY` / `GBRAIN_OCR_BASE_URL` / `GBRAIN_OCR_MODEL` | Page-level PDF layout recognition, JPG/PNG single-image recognition, text writeback, and re-embedding |

### API Key Fallback Chain

Each module's API key falls back in this priority order:

```
Embeddings:    GBRAIN_OPENAI_API_KEY
Expansion:     GBRAIN_EXPANSION_API_KEY → GBRAIN_OPENAI_API_KEY
LLM Chunker (reserved): GBRAIN_CHUNKER_API_KEY → GBRAIN_OPENAI_API_KEY
KB RAPTOR:     library raptor_llm_secret_ref → GBRAIN_KB_RAPTOR_API_KEY → GBRAIN_EXPANSION_API_KEY → GBRAIN_CHUNKER_API_KEY
Reranking:     GBRAIN_EXPANSION_API_KEY → GBRAIN_OPENAI_API_KEY
PDF/Image OCR: GBRAIN_OCR_API_KEY → ZHIPU_API_KEY
```

Set `GBRAIN_OPENAI_API_KEY` to enable embeddings, query expansion, and search reranking with the OpenAI-compatible default endpoint/model. RAPTOR needs a library/KB RAPTOR secret or `GBRAIN_EXPANSION_API_KEY` / `GBRAIN_CHUNKER_API_KEY`. PDF/image OCR uses separate GLM-OCR credentials and does not fall back to `GBRAIN_OPENAI_API_KEY`.

### Base Configuration

| Variable | Description | Default |
|----------|-------------|---------|
| `GBRAIN_DIR` | Data storage root directory | `~/.gbrain` |
| `GBRAIN_DB_PATH` | Database file path | `$GBRAIN_DIR/brain.db` |

### Embeddings

| Variable | Description | Default |
|----------|-------------|---------|
| `GBRAIN_OPENAI_API_KEY` | OpenAI API key (for embeddings; also fallback for other modules) | — |
| `GBRAIN_OPENAI_BASE_URL` | OpenAI-compatible base URL (also fallback for other modules) | — |
| `GBRAIN_EMBEDDING_MODEL` | Embedding model name | `text-embedding-3-large` |
| `GBRAIN_EMBEDDING_DIMENSIONS` | Embedding vector dimensions | `1536` |

### Query Expansion / Reranking (Chat Completions)

| Variable | Description | Default |
|----------|-------------|---------|
| `GBRAIN_EXPANSION_API_KEY` | Query expansion and search reranking LLM API key | Falls back to `GBRAIN_OPENAI_API_KEY` |
| `GBRAIN_EXPANSION_BASE_URL` | Query expansion and search reranking LLM base URL | Falls back to `GBRAIN_OPENAI_BASE_URL`; default OpenAI endpoint is used if omitted |
| `GBRAIN_EXPANSION_MODEL` | Query expansion and search reranking LLM model | `gpt-4o-mini` |

### LLM Chunking

The current KB document pipeline uses Markdown/recursive chunking or embedding-based semantic chunking; it does not call the LLM-guided chunker in `src/chunker/llm.rs` yet. The variables below are reserved for that path, and `GBRAIN_CHUNKER_*` is also used as the final RAPTOR fallback.

| Variable | Description | Default |
|----------|-------------|---------|
| `GBRAIN_CHUNKER_API_KEY` | LLM chunking API key | Falls back to `GBRAIN_OPENAI_API_KEY` |
| `GBRAIN_CHUNKER_BASE_URL` | LLM chunking base URL | Falls back to `GBRAIN_OPENAI_BASE_URL` |
| `GBRAIN_CHUNKER_MODEL` | LLM chunking model | `gpt-4o-mini` |

### Audio Transcription

Transcription is available through the library module; no corresponding CLI or MCP tool is currently exposed.

| Variable | Description | Default |
|----------|-------------|---------|
| `GBRAIN_TRANSCRIPTION_PROVIDER` | Transcription service provider (`groq` / `openai`) | `groq` |
| `GBRAIN_TRANSCRIPTION_GROQ_API_KEY` | Groq transcription API key | — |
| `GBRAIN_TRANSCRIPTION_GROQ_BASE_URL` | Groq transcription base URL | — |
| `GBRAIN_TRANSCRIPTION_OPENAI_API_KEY` | OpenAI transcription API key | — |
| `GBRAIN_TRANSCRIPTION_OPENAI_BASE_URL` | OpenAI transcription base URL | — |

### PDF/Image OCR (GLM-OCR)

PDF parsing performs page-level OCR selection first. In `auto` mode, only pages with an empty or low-density text layer, images/vector or annotation risk, invisible or apparently garbled text, content parsing failures, or uncertain page geometry are sent to OCR. The decision is persisted as `ocr_scope` (`none` / `partial` / `full`), `needs_ocr_pages`, and `ocr_reasons_by_page`.

JPG/PNG uploads are queued directly as single-page OCR documents. Recognized text is written back as KB text nodes and re-embedded. Single images follow GLM-OCR's 10 MiB limit; images have no PDF-style native text layer, so if OCR is disabled, missing credentials, or blocked by library policy, there is no local text fallback.

External OCR is always allowed (library-level privacy switches have been removed). As long as the global switch `GBRAIN_OCR_ENABLED=true` and an API key is configured, PDF pages and JPG/PNG images will be sent to the external OCR service.

| Variable | Description | Default |
|----------|-------------|---------|
| `GBRAIN_OCR_ENABLED` | Global switch for PDF/image OCR | `true` |
| `GBRAIN_OCR_API_KEY` | GLM-OCR API key; read only from the environment | — |
| `ZHIPU_API_KEY` | Compatibility alias for the OCR API key, used only if `GBRAIN_OCR_API_KEY` is empty | — |
| `GBRAIN_OCR_BASE_URL` | GLM-OCR endpoint; a custom URL also requires the safety gate below | `https://open.bigmodel.cn/api/paas/v4/layout_parsing` |
| `GBRAIN_OCR_ALLOW_CUSTOM_BASE_URL` | Allow a custom OCR endpoint; accepted only from the environment | `false` |
| `GBRAIN_OCR_MODEL` / `GLMOCR_MODEL` | OCR model name, with the former taking precedence | `glm-ocr` |
| `GBRAIN_OCR_PROFILE` | Post-processing profile: `general` / `table` / `formula` / `handwriting`; not sent to the API | `general` |
| `GBRAIN_OCR_ENABLE_LAYOUT` / `GLMOCR_ENABLE_LAYOUT` | Request layout recognition, with the former taking precedence | `true` |
| `GBRAIN_OCR_MODE` | OCR selection mode: `auto` / `all_pages` | `auto` |
| `GBRAIN_OCR_SUBMIT_MODE` | PDF submission mode: `pdf_first` / `pdf_range` | `pdf_first` |
| `GBRAIN_OCR_SYNC_INLINE` | Execute OCR inline; background async jobs are used by default | `false` |
| `GBRAIN_OCR_TEXT_DENSITY_THRESHOLD` | Character threshold for a low-density text layer | `50` |
| `GBRAIN_OCR_MIN_LOW_DENSITY_RATIO` | Compatibility option for low-density ratio; currently retained as statistics and does not veto page selection | `0.5` |
| `GBRAIN_OCR_IMAGE_AREA_THRESHOLD` | Image area coverage threshold | `0.08` |
| `GBRAIN_OCR_IMAGE_COUNT_THRESHOLD` | Embedded image count threshold | `1` |
| `GBRAIN_OCR_TIMEOUT_SECONDS_PER_PAGE` / `GLM_OCR_TIMEOUT` / `GLMOCR_TIMEOUT` | Per-page request timeout in seconds, in priority order | `60` |
| `GBRAIN_OCR_MAX_PAGES_PER_REQUEST` | Maximum pages in one OCR request | `100` |
| `GBRAIN_OCR_MAX_PDF_BYTES_PER_REQUEST` | Maximum PDF bytes in one OCR request | `52,428,800` (50 MiB) |
| `GBRAIN_OCR_MAX_PAGES_PER_DOCUMENT` | Maximum pages attempted for OCR per document; `0` is clamped to `1` | `300` |
| `GBRAIN_OCR_MAX_CONCURRENCY` | Process-local limit for concurrent OCR work; `0` is clamped to `1` | `2` |
| `GBRAIN_OCR_TEMP_DIR_MAX_BYTES` | Process-local total byte budget for temporary split-PDF files | `536,870,912` (512 MiB) |
| `GBRAIN_OCR_RETURN_CROP_IMAGES` | Request returned crop images | `false` |
| `GBRAIN_OCR_NEED_LAYOUT_VISUALIZATION` | Request returned layout visualization results | `false` |

**Security notes:**

- `GBRAIN_OCR_API_KEY` and `GBRAIN_OCR_ALLOW_CUSTOM_BASE_URL` take effect only from environment variables; a configuration file cannot enable the custom-endpoint gate.
- Unless `GBRAIN_OCR_ALLOW_CUSTOM_BASE_URL=true` is explicitly set, requests use the official default endpoint even if `GBRAIN_OCR_BASE_URL` is configured.
- Enabling a custom endpoint writes an audit warning; the logged endpoint strips userinfo, query, and fragment components, while errors and persisted OCR responses redact sensitive values such as the API key.

### KB Subsystem

| Variable | Description | Default |
|----------|-------------|---------|
| `GBRAIN_KB_ENABLED` | Enable KB subsystem | `true` |
| `GBRAIN_KB_RAPTOR_API_KEY` | KB RAPTOR LLM API key | Used via the default `kb_raptor_secret_ref`; fallback then tries `GBRAIN_EXPANSION_API_KEY`, then `GBRAIN_CHUNKER_API_KEY` |
| `GBRAIN_KB_RAPTOR_BASE_URL` | KB RAPTOR LLM base URL | Falls back to `GBRAIN_EXPANSION_BASE_URL`, then `GBRAIN_CHUNKER_BASE_URL`, then the OpenAI default endpoint |
| `GBRAIN_KB_RAPTOR_MODEL` | KB RAPTOR LLM model | `gpt-4o-mini`; if the KB/library model is empty, the resolver can use `GBRAIN_EXPANSION_MODEL`, then `GBRAIN_CHUNKER_MODEL` |
| `GBRAIN_KB_MAX_FILE_SIZE_MB` | KB max file size (MB) | `50` |
| `GBRAIN_KB_ALLOWED_EXTENSIONS` | KB allowed file extensions (comma-separated) | `pdf,docx,xlsx,csv,html,htm,txt,md,markdown,rst,json,xml,yaml,yml,toml,tsv,png,jpg,jpeg` |
| `GBRAIN_KB_STORAGE_DIR` | KB file storage directory | — |
| `GBRAIN_KB_WORKER_ENABLED` | Enable KB background worker | `true` |
| `GBRAIN_KB_WORKER_POLL_INTERVAL` | KB worker poll interval (seconds) | `30` |
| `GBRAIN_AUTOPILOT_ENABLED` | Enable autopilot background maintenance thread (takes effect in `gbrain serve`) | `true` |
| `GBRAIN_AUTOPILOT_INTERVAL` | Autopilot maintenance interval (seconds, default 3600 = 1 hour, at least 60s) | `3600` |
| `GBRAIN_KB_SYNONYMS_FILE` | Synonyms file path (for search query expansion) | — |
| `GBRAIN_KB_ALIASES_FILE` | Alias mapping file path (for search query expansion) | — |

**KB Subsystem LLM Usage:**

| Feature | LLM Type | API Key / Base URL | Model Used |
|---------|----------|-------------------|------------|
| Document chunk embedding (vectorization) | Embeddings API | `GBRAIN_OPENAI_API_KEY` / `GBRAIN_OPENAI_BASE_URL` | `GBRAIN_EMBEDDING_MODEL` |
| Semantic chunking (paragraph similarity) | Embeddings API | `GBRAIN_OPENAI_API_KEY` / `GBRAIN_OPENAI_BASE_URL` | `GBRAIN_EMBEDDING_MODEL` |
| Page-level PDF/image OCR and text writeback | GLM-OCR `layout_parsing` | `GBRAIN_OCR_API_KEY` / `GBRAIN_OCR_BASE_URL` (custom URL requires `GBRAIN_OCR_ALLOW_CUSTOM_BASE_URL=true`) | `GBRAIN_OCR_MODEL` |
| RAPTOR hierarchical summarization | Chat Completions | Library `raptor_llm_*` → `GBRAIN_KB_RAPTOR_*` → `GBRAIN_EXPANSION_*` → `GBRAIN_CHUNKER_*` | Library/KB model → `GBRAIN_EXPANSION_MODEL` → `GBRAIN_CHUNKER_MODEL` → `gpt-4o-mini` when no KB model is set |
| Search reranking | Chat Completions | `GBRAIN_EXPANSION_API_KEY` / `GBRAIN_EXPANSION_BASE_URL`, falling back to `GBRAIN_OPENAI_*` | `GBRAIN_EXPANSION_MODEL` / `expansion_model` → `gpt-4o-mini` |

### Artifact Fusion Architecture

| Variable | Description | Default |
|----------|-------------|---------|
| `GBRAIN_ARTIFACT_STORAGE_DIR` | Artifact original file storage directory | `$GBRAIN_DIR/artifacts` |
| `GBRAIN_DEFAULT_KB_LIBRARY_ID` | Default KB library ID | — |
| `GBRAIN_UPLOAD_PROMOTION_POLICY` | Upload default promotion policy: none/shadow/candidate/auto-low-risk | `candidate` |
| `GBRAIN_ARTIFACT_DEFAULT_INTENT` | Artifact default intent: memory/evidence/promote | `memory` |
| `GBRAIN_ARTIFACT_AUTO_CREATE_INBOX_LIBRARY` | Auto-create Inbox library when missing | `true` |
| `GBRAIN_ARTIFACT_MANUAL_MEMORY_TO_KB` | Write memory intent to KB (set to `false` to write gbrain pages only) | `true` |

### Logging

| Variable | Description | Default |
|----------|-------------|---------|
| `GBRAIN_LOG_LEVEL` | Log level (trace/debug/info/warn/error) | `info` |
| `GBRAIN_LOG_TO_FILE` | Enable file logging | `true` |
| `GBRAIN_LOG_FILE_PATH` | Log file path | `$GBRAIN_DIR/logs/gbrain.log` |
| `GBRAIN_LOG_TO_CONSOLE` | Enable console logging | `true` |

### Behavior

| Variable | Description | Default |
|----------|-------------|---------|
| `GBRAIN_AUTO_LINK` | Auto-extract links on write | `true` |
| `GBRAIN_AUTO_TIMELINE` | Auto-extract timeline on write | `true` |
| `GBRAIN_POST_WRITE_LINT` | Run validator checks after write and log the result | `false` |

### Debug

| Variable | Description | Default |
|----------|-------------|---------|
| `GBRAIN_SEARCH_DEBUG` | Enable search debug logging (set to `1` or `true`) | — |
| `GBRAIN_PROGRESS_MODE` | Progress display mode (`human` / `json` / `quiet`) | Auto-detected |
| `GBRAIN_PROGRESS_JSON` | Set to `"1"` to enable JSON progress mode | — |

---

## Writer Modes

The `BrainWriter` library API provides three page-write strategies. The current `gbrain put` and `artifact_put` interfaces do not accept a `mode` parameter; `post_write_lint` only runs `validators` checks and logs results, rather than executing the six auto-fix rules from `lint.rs` below.

| Mode | Description |
|------|-------------|
| `Strict` | Full validation — requires frontmatter, rejects empty content, checks link reference validity |
| `Lint` | Zero-LLM quality checks — runs 6 rules, auto-fixes where possible |
| `Off` | Free write — skips all validation, writes directly |

### Lint Rules

Rules implemented in `lint.rs` (not currently exposed as a CLI/MCP command):

| Rule | Description |
|------|-------------|
| LLM preamble detection | Detect and remove typical AI-generated preambles ("Here is...", "Sure, I'll...") |
| Placeholder date detection | Detect unsubstituted date placeholders (e.g., `YYYY-MM-DD`) |
| Missing frontmatter | Detect missing YAML frontmatter |
| Broken citations | Detect wikilinks referencing non-existent pages |
| Empty sections | Detect sections with headings but no content |
| Unclosed code fences | Detect unclosed ``` code blocks |

---

## Soft-Delete Lifecycle

Knowledge source deletion follows a soft-delete mechanism to prevent accidental data loss:

```
Active source ──delete──→ Soft-deleted (still in storage, not queryable)
                            │
                            └──restore──→ Restored to active source
```

Artifact sources do not currently have a permanent-delete operation. Engine cleanup paths for pages or KB documents do not advance `source_artifacts` records to the `purged` state.

- `gbrain delete <id_or_uid>` — Soft-delete; source is marked deleted but data is retained
- `gbrain restore <id_or_uid>` — Restore a soft-deleted source
- `gbrain health` — Check knowledge source consistency

---

## Code Knowledge Graph

Based on Tree-sitter AST chunking + regex symbol indexing, supporting the following languages:

| Language | Tree-sitter Binding |
|----------|-------------------|
| Rust | `tree-sitter-rust` |
| TypeScript | `tree-sitter-typescript` |
| JavaScript | `tree-sitter-javascript` |
| Python | `tree-sitter-python` |
| Go | `tree-sitter-go` |
| Java | `tree-sitter-java` |
| C | `tree-sitter-c` |
| C++ | `tree-sitter-cpp` |

Tree-sitter indexing runs when code pages are written or when `Operations::reindex_code_page` is invoked. With `gbrain upload --intent auto`, recognized code extensions are directed to the existing code import/sync flow; upload does not automatically create KB or code-graph projections. The current `artifact_query` facade also does not expose `graph` mode.

---

## Testing

```bash
cargo test                    # All tests
cargo test --test engine_test # Engine integration tests
cargo test --test search_test # Search integration tests
cargo test --test fuzzy_test  # Fuzzy matching tests
cargo test --test dedup_test  # Deduplication tests
cargo test --test artifact_facade_test  # Artifact facade integration tests
cargo clippy                  # Lint
```

Tests use in-memory SQLite (`:memory:`) — no extra configuration needed.

---

## Architecture

Three-layer design:

1. **Engine Layer** — `BrainEngine` trait → `SqliteEngine` (SQLite + FTS5; sqlite-vec accelerates vector retrieval when available, otherwise BLOB fallback is used). Synchronous, direct database operations.

2. **Operations Layer** — Business logic: auto-chunking, tag extraction, link inference, safety validation, batch operations.

3. **Interface Layer** — CLI + MCP server. CLI uses `remote=false`; MCP sets `remote=true` for untrusted callers.

### Search Pipeline

Lower-level hybrid search pipeline (used by internal or programmatic search paths; the public `gbrain query` / `artifact_query` facade currently does not generate query vectors or LLM query expansions):

1. FTS5 BM25 keyword search (weights: title 10x, compiled_truth 5x, timeline 2x)
2. Optional vector search (sqlite-vec when available, otherwise BLOB fallback)
3. Supplement with an expanded OR keyword query when vector results are fewer than 3
4. RRF fusion (k=60) and normalization, accepting supplied expanded vector-result lists
5. compiled_truth weighted boost
6. When a query embedding is supplied, blend cosine similarity with normalized RRF for reranking
7. Backlink boost
8. Recency boost (time decay)
9. Intent type boost (entity/time/event)
10. Optional two-stage code graph expansion (`walk_depth` / `near_symbol`)
11. 6-layer dedup (slug top-3 → cross-source dedup → text similarity → type diversity → per-page cap → compiled_truth guarantee)

### KB Subsystem Architecture

Async five-stage document processing pipeline:

1. **Parse** — Document parsers (Markdown / PDF / DOCX / XLSX / CSV / HTML / plaintext); code graph indexing follows the separate page path
2. **Split** — Recursive splitter; semantic split mode (Savitzky-Golay smoothing + chunk_overlap overlap) is available when a library enables `semantic_enabled` and embeddings are configured
3. **Embed** — Vector embedding generation and persistence
4. **RAPTOR (enabled by default for new libraries)** — When `raptor_enabled=true` and runtime prerequisites are met, build a recursive summarization tree (K-Means++ clustering + LLM summarization, four-level fallback chain: library config → `GBRAIN_KB_RAPTOR_*` → `GBRAIN_EXPANSION_*` → `GBRAIN_CHUNKER_*`)
5. **Persist** — Transaction-protected node/vector writes

`raptor_enabled` is stored on each `kb_libraries` row. Newly created libraries, including an automatically created `Inbox`, default it to `true`. Once enabled, the pipeline automatically skips RAPTOR when a document has fewer than three nodes, external summarization is disallowed, or no RAPTOR LLM credential can be resolved. The current `gbrain config`, CLI `kb` subcommands, and MCP tools do not expose a disable switch.

For PDFs, parsing first creates a page-level OCR decision. Pages with empty/low-density text, images or vector content, annotation appearance risk, hidden text, suspected font-encoding problems, or parse/geometry uncertainty are conservatively selected. When OCR is needed, the system queues an asynchronous OCR job by default, writes recognized text back, and re-embeds it; set `GBRAIN_OCR_SYNC_INLINE=true` only to execute OCR inline in the document pipeline. For JPG/PNG uploads, the pipeline creates a single-page OCR document immediately and generates KB nodes after OCR completes.

### Chinese NLP Module

- **Tokenized Index** — jieba tokenization + pinyin + prefix wildcards, FTS5 query auto-rewriting
- **Chinese Chunking** — Chinese punctuation added to sentence/clause separator levels, CJK punctuation breaks without trailing spaces
- **Pre-tokenized Column** — schema V16 adds `_tokens` column, FTS5 uses `unicode61` tokenizer, auto-synced on write

### Single-Entry Multi-Projection Fusion Architecture (Artifact)

```
Upload Source (Single Entry Point)
  |
  +-- Route Planner (auto-decides based on intent + file type)
  |
  +-- Artifact Original Storage (SHA256 dedup, named by hash)
  |
  +-- Multi-Projection Auto-Creation:
      +-- KB Document Projection -> Document processing pipeline (parse->split->embed->RAPTOR enabled by default->persist)
      +-- Shadow Page Projection -> Shadow page (extract content -> generate wiki page)
      +-- Brain Page Update Projection -> Existing-page update
      +-- File Attachment Projection -> File attachment (simple file reference)
      +-- Promotion Candidate -> Candidate changes (may include link/timeline suggestions)
```

**Core Concepts:**

- **Artifact** — Uploaded original file, currently reaching active/deleted states (`purged` is reserved), source type (upload/sync/link/mcp), upload intent (auto/evidence/memory/attachment/promote; `document` is a legacy alias for `evidence`)
- **Projection** — Representation of the same Artifact in different subsystems, with version chain (superseded_by) and state (active/stale/superseded)
- **Candidate** — Suggested changes extracted from KB evidence, with risk level (low/medium/high) and review workflow (pending->accepted->applied / rejected / rolled_back)
- **Provenance** — Audit records tracing page facts back to their source Artifact and Candidate

**Unified Memory Query:**

4 query strategies auto-adapt to different scenarios:

| Strategy | Description |
|----------|-------------|
| `brain_first` | Search gbrain curated knowledge first, supplement with KB evidence if insufficient |
| `evidence_first` | Search KB document evidence first, ideal for queries requiring original sources |
| `provenance` | Trace fact origins, return provenance records |
| `timeline_first` | Sort by timeline first, ideal for time-related queries |

These four are internal Memory Query strategies. The CLI/MCP facade currently exposes only `auto` / `memory` / `evidence` / `timeline` modes and does not provide `provenance` or `graph` mode.

---

## Documentation

- [TS vs Rust Comparison Report](./docs/compare_report_en.md) / [中文](./docs/compare_report.md) — Comprehensive comparison of TypeScript and Rust versions (code scale, database, search, MCP, security, etc.)
- [TS vs Rust Module-Level Detail](./docs/module_detail_en.md) / [中文](./docs/module_detail.md) — Module-by-module comparison (engine layer, operations layer, search, chunking, enrichment, validators, etc.)

---

## License

MIT License
