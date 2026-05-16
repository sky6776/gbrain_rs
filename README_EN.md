# gbrain-rs

中文 | [English](./README_EN.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)

**Personal Knowledge Brain Engine** — Rust port of [gbrain](https://github.com/garrytan/gbrain), with Single-Entry Multi-Projection Fusion Architecture (Artifact originals → KB/Shadow Pages/Candidate Changes/Attachments multi-projection + provenance audit + rollback), KB subsystem (async document processing pipeline + RAPTOR recursive summarization tree), full Chinese NLP support (jieba tokenization + pinyin + FTS5 query rewriting), soft-delete lifecycle (restore/purge-deleted), time-decay search, and more. Built on SQLite + sqlite-vec + FTS5 with a zero-config embedded architecture — ready to use out of the box.

> The original TypeScript version was developed by [Garry Tan](https://github.com/garrytan). Built with **Vibe coding**.

---

## Quick Start

```bash
# 1. Build
cargo build --release

# 2. Initialize a knowledge base
gbrain init

# 3. Create a page
gbrain put people/alice --title "Alice" --content "An engineer skilled in Rust and systems programming"

# 4. Search
gbrain query "who is Alice"

# 5. Start MCP server (for AI agent integration)
gbrain serve
```

No database or external services to configure — works out of the box. AI features like embeddings and query expansion are optional and activate automatically when API keys are configured.

---

## Features

- **Hybrid Search** — BM25 keywords + vector cosine similarity + fuzzy trigrams, merged via Reciprocal Rank Fusion (RRF), with multi-query expansion
- **Knowledge Graph** — Wiki-link extraction, typed links, graph traversal, backlink symmetry verification
- **KB Subsystem** — Async five-stage document processing pipeline (parse → split → embed → RAPTOR → persist), RAPTOR recursive summarization tree, document upload and processing, multi-format parsers (Markdown/PDF/DOCX/XLSX/CSV/HTML/plaintext/code), semantic chunking (Savitzky-Golay smoothing + chunk_overlap overlap)
- **Chinese NLP** — jieba tokenization + pinyin + prefix wildcards, FTS5 query auto-rewriting, Chinese punctuation sentence-breaking and token counting, pre-tokenized column auto-sync
- **Single-Entry Multi-Projection Fusion** — Artifact upload automatically routes to multiple projections (KB document / shadow page / candidate changes / file attachment / links / timeline), provenance audit ledger, candidate review & promotion workflow, version chain with rollback (Projection Supersede / Rollback), unified memory query (Memory Query, 4 strategies)
- **MCP Server** — Full Model Context Protocol (JSON-RPC 2.0) server with 74 tools for AI agent integration
- **Zero Config** — Embedded SQLite, no external services required (embeddings optional)
- **Layered Enrichment** — Automatic entity detection and promotion (mention → stub → enriched)
- **Version History** — Full page versioning with rollback
- **Autopilot** — Self-maintenance daemon that auto-embeds stale content and runs integrity checks
- **Safety Guards** — Path traversal protection, slug validation, remote-call input sanitization, parameterized queries against SQL injection
- **Code Knowledge Graph** — Tree-sitter AST code chunking + regex symbol indexing with symbol definitions, references, and call graph (Rust/TypeScript/JavaScript/Python/Go/Java/C/C++)
- **Audio Transcription** — Groq Whisper (default) or OpenAI Whisper support
- **Writer Modes** — Strict (full validation) / Lint (zero-LLM quality checks) / Off (free write) strategies
- **Soft-Delete Lifecycle** — Delete → restore → permanent purge, with time-based batch cleanup

---

## Build & Install

```bash
cargo build --release          # Build
cargo install --path .         # Install to ~/.cargo/bin/
gbrain install                 # Install to ~/.gbrain/bin/
```


---

## Data Directory

After initialization, the `~/.gbrain/` directory structure is:

```
~/.gbrain/
  brain.db           # SQLite database (FTS5 + sqlite-vec)
  config.json        # Runtime config (generated via gbrain config set)
  artifacts/          # Artifact original file storage (named by SHA256)
  files/             # Uploaded file storage
  cache/             # Cache directory
  logs/              # Log files
    gbrain.log
```

Customize the root directory via the `GBRAIN_DIR` environment variable.

---

## CLI Commands

### Global Options

| Option | Description |
|--------|-------------|
| `--db <PATH>` | Database path |
| `--json` | Output as JSON |
| `--dry-run` | Preview operations without executing |

### Core

| Command | Description |
|---------|-------------|
| `gbrain init` | Initialize a new knowledge base |
| `gbrain get <slug>` | Read a page by slug |
| `gbrain put <slug> --title <TITLE> [--content <TEXT> \| --file <PATH>]` | Create or update a page |
| `gbrain delete <slug> [--force]` | Soft-delete a page |
| `gbrain restore <slug>` | Restore a soft-deleted page |
| `gbrain purge-deleted [--older-than-hours <N>]` | Permanently clean up old soft-deleted pages |
| `gbrain list [--page-type <TYPE>] [--limit <N>]` | List pages (filterable) |
| `gbrain query <query> [--limit <N>] [--lang <LANG>] [--symbol-kind <KIND>]` | Hybrid search (alias: `ask`), with code filtering and two-stage retrieval |

#### Examples

```bash
# Initialize a knowledge base
gbrain init

# Create a page
gbrain put people/alice --title "Alice" --content "An engineer skilled in Rust and systems programming"

# Search
gbrain query "Rust async programming" --limit 5 --lang rust

# Restore a mistakenly deleted page
gbrain restore people/alice
```

### Search & Graph

| Command | Description |
|---------|-------------|
| `gbrain resolve <partial>` | Fuzzy-resolve a partial slug |
| `gbrain graph <slug> [--depth <N>]` | Traverse the knowledge graph from a page |
| `gbrain graph-query <from> [--to <slug>] [--depth <N>] [--link-type <TYPE>]` | Query graph relationships between pages |
| `gbrain code search/def/refs/callers/callees/edges` | Code chunk, symbol definition/reference, and call graph queries |

#### Examples

```bash
# Fuzzy-resolve a slug
gbrain resolve ali

# Traverse the knowledge graph
gbrain graph people/alice --depth 3

# Query relationship path between two pages
gbrain graph-query people/alice --to companies/acme --depth 3

# Find a Rust function definition
gbrain code def --symbol "parse_config" --lang rust
```

### Backlinks

| Command | Description |
|---------|-------------|
| `gbrain backlinks list <slug>` | List backlinks for a page |
| `gbrain backlinks check [slug]` | Check for missing backlinks |
| `gbrain backlinks fix [slug]` | Fix missing backlinks |

#### Examples

```bash
# List backlinks
gbrain backlinks list people/alice

# Check backlink integrity for all pages
gbrain backlinks check

# Fix missing backlinks
gbrain backlinks fix people/alice
```

### Data Management

| Command | Description |
|---------|-------------|
| `gbrain embed [slugs...] [--batch-size <N>]` | Generate and persist embeddings for stale chunks |
| `gbrain import <dir> [--embed] [--auto-link]` | Import Markdown and supported code files; skips when frontmatter slug mismatches path |
| `gbrain export [slugs...] [--dir <DIR>] [--page-type <TYPE>]` | Export pages as Markdown |
| `gbrain extract [--mode links\|timeline\|all]` | Batch extract links/timeline |
| `gbrain lint [slug] [--fix] [--dry-run]` | Zero-LLM quality checks (6 rules) |

#### Examples

```bash
# Import a directory and generate embeddings
gbrain import ./my-notes --embed --auto-link

# Export all pages
gbrain export --dir ./backup

# Batch extract links and timeline
gbrain extract --mode all

# Run lint checks and auto-fix
gbrain lint --fix
```

### File Storage

| Command | Description |
|---------|-------------|
| `gbrain file upload <path> [--page <slug>]` | Upload a file |
| `gbrain file list [slug]` | List stored files |
| `gbrain file sync <dir>` | Sync a directory to storage |
| `gbrain file verify` | Verify all file records |
| `gbrain file url <storage-path>` | Get local path/URL for a file |

#### Examples

```bash
# Upload a file and associate with a page
gbrain file upload report.pdf --page projects/annual-report

# List files associated with a page
gbrain file list projects/annual-report

# Get file path
gbrain file url files/report.pdf
```

### Health & Maintenance

| Command | Description |
|---------|-------------|
| `gbrain stats` | Knowledge base statistics |
| `gbrain health` | Health dashboard |
| `gbrain doctor [--fast]` | Comprehensive diagnostics |
| `gbrain integrity` | Check data integrity |
| `gbrain orphans` | Detect orphan pages |
| `gbrain autopilot [--once] [--interval <SECS>]` | Self-maintenance daemon |

#### Examples

```bash
# View knowledge base statistics
gbrain stats

# Quick diagnostics
gbrain doctor --fast

# Run self-maintenance once
gbrain autopilot --once

# Continuous self-maintenance (every 10 minutes)
gbrain autopilot --interval 600
```

### Config & Misc

| Command | Description |
|---------|-------------|
| `gbrain config show` | Show all config values |
| `gbrain config get <key>` | Get a config value |
| `gbrain config set <key> <value>` | Set a config value |
| `gbrain report --report-type <TYPE> [--title <TITLE>] [--content <TEXT>]` | Generate a knowledge base report |
| `gbrain ingest-log [--limit <N>]` | View ingest log |
| `gbrain tools-json` | Output MCP tool definitions as JSON |
| `gbrain serve` | Run as an MCP stdio server |

#### Examples

```bash
# View all configuration
gbrain config show

# Set embedding model
gbrain config set embedding_model text-embedding-3-large

# Generate a maintenance report
gbrain report --report-type maintenance --title "Weekly Check"

# Start MCP server
gbrain serve
```

### KB Subsystem

| Command | Description |
|---------|-------------|
| `gbrain kb-worker [--once] [--interval <SECS>]` | Start KB document processing worker (dequeues jobs from queue) |
| `gbrain kb-eval --library-id <ID>` | Run KB search quality evaluation |
| `gbrain kb-backup --output <DIR>` | Backup KB database and storage |
| `gbrain kb-restore --input <DIR>` | Restore KB from backup |
| `gbrain kb-source-add --library-id <ID> --path <DIR>` | Add local directory as KB import source |
| `gbrain kb-sync-source --source-id <ID>` | Sync KB import source |
| `gbrain kb-jobs list/pause/resume --library-id <ID>` | KB job management (list/pause/resume) |
| `gbrain kb-export-library --library-id <ID> --output <DIR>` | Export KB library to directory |
| `gbrain kb-import-library --archive <DIR> [--new-name <NAME>]` | Import KB library from export |
| `gbrain kb-reembed --library-id <ID> [--embedding-index-id <ID>]` | Re-embed documents (with new model) |
| `gbrain kb-eval-compare --index-id-1 <ID> --index-id-2 <ID>` | Compare search quality of two embedding indexes |
| `gbrain kb-health-check [--library-id <ID>] [--repair]` | Check KB index health (optional repair) |
| `gbrain kb-rebuild-document --document-id <ID>` | Rebuild a single document's index |
| `gbrain kb-rebuild-library --library-id <ID>` | Rebuild an entire library's index |
| `gbrain kb-purge-deleted [--library-id <ID>] [--older-than-days <N>]` | Purge soft-deleted KB documents |

#### Examples

```bash
# Start the KB processing worker (one pass)
gbrain kb-worker --once

# Run search quality evaluation
gbrain kb-eval --library-id 1

# Backup KB database
gbrain kb-backup --output ./kb-backup

# Add a local directory as import source
gbrain kb-source-add --library-id 1 --path ./docs

# Re-embed documents with a new model
gbrain kb-reembed --library-id 1

# Check KB health
gbrain kb-health-check

# Purge soft-deleted documents older than 30 days
gbrain kb-purge-deleted --older-than-days 30
```

### Single-Entry Multi-Projection Fusion (Artifact)

| Command | Description |
|---------|-------------|
| `gbrain upload <path> [--intent <INTENT>] [--library-id <ID>] [--target <SLUG>] [--promotion <POLICY>] [--dry-run]` | Upload a source file (unified entry point), auto-route to multiple projections. intent: auto/document/attachment/memory/promote; promotion: none/shadow/candidate/auto-low-risk |
| `gbrain memory-query <query> [--strategy <STRATEGY>] [--limit <N>] [--filter-slug <SLUG>]` | Unified memory query (alias: ask-memory). strategy: brain_first/evidence_first/provenance/timeline_first |
| `gbrain artifact list [--limit <N>] [--offset <N>]` | List all artifacts |
| `gbrain artifact get <id_or_uid>` | Get artifact details (supports ID or UID like `art_ab12cd34ef56`) |
| `gbrain artifact delete <artifact_id>` | Soft delete artifact (marks all projections as stale) |
| `gbrain artifact health` | Check artifact projection consistency and health |

### Candidate Changes & Promotion

| Command | Description |
|---------|-------------|
| `gbrain promotion list [--status <STATUS>] [--candidate-type <TYPE>] [--target-slug <SLUG>]` | List promotion candidates |
| `gbrain promotion get <candidate_id>` | Get candidate details |
| `gbrain promotion accept <candidate_id> [--reviewer <NAME>] [--notes <TEXT>]` | Accept a candidate |
| `gbrain promotion reject <candidate_id> [--reviewer <NAME>] [--notes <TEXT>]` | Reject a candidate |
| `gbrain promotion apply <candidate_id>` | Apply an accepted candidate to gbrain |
| `gbrain promotion auto-apply <artifact_id>` | Auto-apply low-risk candidates |
| `gbrain promotion batch-apply [--artifact-id <ID>] [--risk <LEVEL>] [--dry-run]` | Batch apply candidates |
| `gbrain promotion rollback <candidate_id>` | Rollback an applied candidate |

### Projection Management

| Command | Description |
|---------|-------------|
| `gbrain projection supersede <old_proj_id> <new_proj_id>` | Supersede an old projection with a new one (version chain) |
| `gbrain projection history <projection_key> [--artifact-id <ID>] [--projection-type <TYPE>] [--limit <N>]` | Query projection version chain history |
| `gbrain gc-orphan-projections [--stale-days <N>] [--dry-run]` | Garbage collect orphaned/superseded projections |

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
- Input content sanitization
- Parameterized queries (SQL injection prevention)
- Filename safety checks

CLI uses `remote=false` directly, bypassing remote security restrictions.

---

## MCP Tools

gbrain provides 74 MCP tools for AI agent integration via JSON-RPC 2.0 over stdio.

### Search

| Tool | Description |
|------|-------------|
| `query` | Hybrid search (vector + keyword + expansion), with detail levels, code filtering, two-stage retrieval, and search metadata |
| `find_by_title_fuzzy` | Fuzzy title search based on trigram similarity |
| `resolve_slugs` | Fuzzy-resolve partial slugs to matching pages |

#### Examples

```json
// Hybrid search
{ "tool": "query", "params": { "query": "Rust async programming", "limit": 5, "lang": "rust" } }
```

```json
// Fuzzy title search
{ "tool": "find_by_title_fuzzy", "params": { "query": "Alice", "min_similarity": 0.6 } }
```

```json
// Fuzzy-resolve slugs
{ "tool": "resolve_slugs", "params": { "partial": "ali" } }
```

### Page CRUD

| Tool | Description |
|------|-------------|
| `get_page` | Read a page (supports fuzzy matching) |
| `put_page` | Write/update a page (Markdown + frontmatter) |
| `delete_page` | Soft-delete a page (requires confirm=true) |
| `list_pages` | List pages (filter by type/tag/limit) |
| `get_chunks` | Get content chunks for a page |

#### Examples

```json
// Read a page
{ "tool": "get_page", "params": { "slug": "people/alice" } }
```

```json
// Create/update a page
{ "tool": "put_page", "params": { "slug": "people/alice", "content": "---\ntitle: Alice\n---\nAn engineer" } }
```

```json
// Soft-delete a page (requires confirmation)
{ "tool": "delete_page", "params": { "slug": "people/alice", "confirm": true } }
```

```json
// List pages
{ "tool": "list_pages", "params": { "type": "person", "limit": 10 } }
```

### Tags

| Tool | Description |
|------|-------------|
| `add_tag` | Add a tag to a page |
| `remove_tag` | Remove a tag from a page |
| `get_tags` | List tags for a page |

#### Examples

```json
// Add a tag
{ "tool": "add_tag", "params": { "slug": "people/alice", "tag": "engineer" } }
```

```json
// List tags
{ "tool": "get_tags", "params": { "slug": "people/alice" } }
```

### Links & Graph

| Tool | Description |
|------|-------------|
| `add_link` | Create a typed link between pages |
| `remove_link` | Remove a link between pages |
| `get_links` | List outbound links for a page |
| `get_backlinks` | List inbound links for a page |
| `traverse_graph` | Traverse the link graph from a page |

#### Examples

```json
// Create a typed link
{ "tool": "add_link", "params": { "from": "people/alice", "to": "companies/acme", "link_type": "works_at" } }
```

```json
// Traverse the graph
{ "tool": "traverse_graph", "params": { "slug": "people/alice", "depth": 3, "direction": "both" } }
```

```json
// Get backlinks
{ "tool": "get_backlinks", "params": { "slug": "people/alice" } }
```

### Timeline

| Tool | Description |
|------|-------------|
| `add_timeline_entry` | Add a timeline entry to a page |
| `get_timeline` | Get timeline for a page |

#### Examples

```json
// Add a timeline entry
{ "tool": "add_timeline_entry", "params": { "slug": "people/alice", "date": "2024-01-15", "summary": "Joined Acme Corp" } }
```

```json
// Get timeline
{ "tool": "get_timeline", "params": { "slug": "people/alice" } }
```

### Versioning

| Tool | Description |
|------|-------------|
| `get_versions` | Page version history |
| `revert_version` | Revert a page to a previous version |

#### Examples

```json
// View version history
{ "tool": "get_versions", "params": { "slug": "people/alice" } }
```

```json
// Revert to a specific version
{ "tool": "revert_version", "params": { "slug": "people/alice", "version_id": 3 } }
```

### Raw Data

| Tool | Description |
|------|-------------|
| `put_raw_data` | Store raw API response data for a page |
| `get_raw_data` | Get raw data for a page |

#### Examples

```json
// Store raw data
{ "tool": "put_raw_data", "params": { "slug": "companies/acme", "source": "crustdata", "data": { "founded": "2020" } } }
```

```json
// Get raw data
{ "tool": "get_raw_data", "params": { "slug": "companies/acme", "source": "crustdata" } }
```

### Code Knowledge Graph

| Tool | Description |
|------|-------------|
| `code_def` | Find code symbol definitions |
| `code_refs` | Find code chunks referencing a symbol |
| `search_code_chunks` | Search code chunks by keyword/symbol text |
| `get_callers` | Get callers of a symbol |
| `get_callees` | Get callees of a symbol |
| `get_code_edges_by_chunk` | Get code graph edges for a chunk |
| `reindex_code_page` | Rebuild code chunks and edges for a code page |

#### Examples

```json
// Find symbol definition
{ "tool": "code_def", "params": { "symbol": "parse_config", "lang": "rust" } }
```

```json
// Find symbol references
{ "tool": "code_refs", "params": { "symbol": "SqliteEngine", "lang": "rust" } }
```

```json
// Get callers
{ "tool": "get_callers", "params": { "slug": "src/engine.rs", "symbol": "query" } }
```

```json
// Rebuild code page index
{ "tool": "reindex_code_page", "params": { "slug": "src/engine.rs" } }
```

### File Storage

| Tool | Description |
|------|-------------|
| `file_upload` | Upload a file to storage |
| `file_list` | List stored files |
| `file_url` | Get URL/path for a file |

#### Examples

```json
// Upload a file
{ "tool": "file_upload", "params": { "path": "/path/to/report.pdf", "page_slug": "projects/annual-report" } }
```

```json
// List files
{ "tool": "file_list", "params": { "slug": "projects/annual-report" } }
```

### Import & Sync

| Tool | Description |
|------|-------------|
| `log_ingest` | Log an ingest event |
| `get_ingest_log` | Get recent ingest log |
| `sync_brain` | Sync knowledge base from a Git repo |
| `find_orphans` | Find orphan pages with no inbound links |

#### Examples

```json
// Sync from Git repo
{ "tool": "sync_brain", "params": { "repo_path": "/path/to/repo", "force_full": false } }
```

```json
// Find orphan pages
{ "tool": "find_orphans", "params": { "include_pseudo": false } }
```

```json
// Get ingest log
{ "tool": "get_ingest_log", "params": { "limit": 10 } }
```

### Health & Stats

| Tool | Description |
|------|-------------|
| `get_stats` | Knowledge base statistics (page count, chunk count, etc.) |
| `get_health` | Health dashboard (embedding coverage, orphan pages, etc.) |

#### Examples

```json
// Get statistics
{ "tool": "get_stats", "params": {} }
```

```json
// Get health status
{ "tool": "get_health", "params": {} }
```

### KB Subsystem

| Tool | Description |
|------|-------------|
| `kb_list_libraries` | List all knowledge libraries (with document and chunk counts) |
| `kb_create_library` | Create a knowledge library (with semantic chunking/RAPTOR/chunking params config) |
| `kb_update_library` | Update library configuration |
| `kb_delete_library` | Delete a knowledge library |
| `kb_upload_document` | Upload a document for processing |
| `kb_get_document_status` | Get document processing status |
| `kb_retry_document` | Retry processing a failed document |
| `kb_cancel_document_job` | Cancel a document processing job |
| `kb_delete_document` | Delete a document from a library |
| `kb_list_documents` | List documents in a library |
| `kb_search` | Cross-library hybrid search (vector + keyword + RRF fusion) |
| `kb_create_folder` | Create a folder in a library |
| `kb_purge_document` | Permanently delete a document (requires confirmation) |
| `kb_check_index_health` | Check knowledge library index health |
| `kb_repair_index` | Repair knowledge library index |
| `kb_backup` | Backup knowledge library to file |
| `kb_restore` | Restore knowledge library from backup file |
| `kb_add_eval_query` | Add a search evaluation query |
| `kb_add_search_feedback` | Add search result feedback rating |

#### Examples

```json
// List knowledge libraries
{ "tool": "kb_list_libraries", "params": {} }
```

```json
// Create a knowledge library
{ "tool": "kb_create_library", "params": { "name": "Project Docs", "raptor_enabled": true, "embedding_model": "text-embedding-3-large" } }
```

```json
// Upload a document
{ "tool": "kb_upload_document", "params": { "library_id": 1, "file_path": "/path/to/doc.pdf" } }
```

```json
// KB search
{ "tool": "kb_search", "params": { "query": "deployment process", "library_ids": [1], "top_k": 10, "profile": "accurate" } }
```

```json
// Backup and restore
{ "tool": "kb_backup", "params": { "output": "/path/to/backup" } }
```

### Single-Entry Multi-Projection Fusion (Artifact)

| Tool | Description |
|------|-------------|
| `upload_source` | Upload a source file (unified entry point), auto-create Artifact, KB projection, shadow page, and file attachment |
| `memory_query` | Unified memory query, searches both gbrain curated knowledge and KB document evidence, 4 strategies auto-selected |
| `artifact_list` | List all artifacts |
| `artifact_get` | Get artifact details (supports ID or UID) |
| `artifact_delete` | Soft delete artifact (marks all projections as stale) |
| `artifact_health` | Check artifact projection consistency and health |
| `get_provenance` | Get provenance records for a brain page (trace where facts came from) |

#### Examples

```json
// Upload a source file
{ "tool": "upload_source", "params": { "path": "/path/to/report.pdf", "intent": "document", "library_id": 1 } }
```

```json
// Unified memory query
{ "tool": "memory_query", "params": { "query": "Alice's project history", "strategy": "evidence_first", "limit": 10 } }
```

```json
// Get provenance records
{ "tool": "get_provenance", "params": { "brain_slug": "people/alice" } }
```

```json
// Check artifact health
{ "tool": "artifact_health", "params": {} }
```

### Candidate Changes & Promotion

| Tool | Description |
|------|-------------|
| `promotion_list_candidates` | List promotion candidates (suggested changes extracted from KB evidence) |
| `promotion_get_candidate` | Get candidate details |
| `promotion_accept_candidate` | Accept a candidate |
| `promotion_reject_candidate` | Reject a candidate (with reason parameter for rejection explanation) |
| `promotion_apply_candidate` | Apply an accepted candidate to gbrain |
| `promotion_batch_apply` | Batch apply pending promotion candidates, optionally filtered by artifact and risk level |
| `promotion_rollback_candidate` | Rollback an applied candidate, undo shadow page updates and mark provenance as stale |

#### Examples

```json
// List pending candidates
{ "tool": "promotion_list_candidates", "params": { "status": "pending", "limit": 20 } }
```

```json
// Accept a candidate
{ "tool": "promotion_accept_candidate", "params": { "candidate_id": 42, "reviewer": "alice", "notes": "Information is accurate" } }
```

```json
// Reject a candidate (with reason)
{ "tool": "promotion_reject_candidate", "params": { "candidate_id": 43, "reason": "Outdated information" } }
```

```json
// Batch apply low-risk candidates (preview)
{ "tool": "promotion_batch_apply", "params": { "risk": "low", "dry_run": true } }
```

```json
// Rollback a candidate
{ "tool": "promotion_rollback_candidate", "params": { "candidate_id": 42 } }
```

### Projection Management

| Tool | Description |
|------|-------------|
| `gc_orphan_projections` | Garbage collect orphaned/superseded projections |
| `projection_supersede` | Supersede an old projection with a new one (version chain) |
| `projection_history` | Query projection version chain history |

#### Examples

```json
// Supersede an old projection
{ "tool": "projection_supersede", "params": { "old_proj_id": 101, "new_proj_id": 202 } }
```

```json
// Query projection version chain
{ "tool": "projection_history", "params": { "projection_key": "kb_doc:42", "artifact_id": 7, "limit": 10 } }
```

```json
// Garbage collect orphan projections (preview)
{ "tool": "gc_orphan_projections", "params": { "stale_days": 30, "dry_run": true } }
```

---

## MCP Tool Parameters

### `query`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | Yes | Search query |
| `limit` | integer | No | Max results (default 20) |
| `offset` | integer | No | Pagination offset |
| `expand` | boolean | No | Enable multi-query expansion (default true) |
| `detail` | string | No | `low` / `medium` / `high` (default medium) |
| `lang` | string | No | Filter code retrieval by programming language |
| `symbol_kind` | string | No | Filter code retrieval by symbol type |
| `near_symbol` | string | No | Anchor symbol for two-stage code graph retrieval |
| `walk_depth` | integer | No | Code graph neighbor walk depth (0-2) |
| `include_meta` | boolean | No | Return `{results, meta}` with vector/expansion details |

### `put_page`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `slug` | string | Yes | Page slug |
| `content` | string | Yes | Full Markdown (with YAML frontmatter) |

### `traverse_graph`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `slug` | string | Yes | Starting page slug |
| `depth` | integer | No | Max traversal depth (default 5, max 10) |
| `link_type` | string | No | Filter by link type |
| `direction` | string | No | `in` / `out` / `both` (default out) |

### `find_by_title_fuzzy`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | Yes | Title to match |
| `dir_prefix` | string | No | Constrain slug prefix |
| `min_similarity` | number | No | Similarity threshold 0.0–1.0 (default 0.55) |
| `limit` | integer | No | Max results (default 10) |

### `sync_brain`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `repo_path` | string | Yes | Git repo path |
| `force_full` | boolean | No | Force full sync (default false) |

### `kb_create_library`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | string | Yes | Library name |
| `semantic_segmentation_enabled` | boolean | No | Enable semantic chunking |
| `raptor_enabled` | boolean | No | Enable RAPTOR summarization tree |
| `raptor_llm_base_url` | string | No | RAPTOR LLM base URL override |
| `raptor_llm_secret_ref` | string | No | RAPTOR LLM API key environment variable name |
| `raptor_llm_model` | string | No | RAPTOR LLM model name |
| `chunk_size` | integer | No | Chunk size in characters |
| `chunk_overlap` | integer | No | Chunk overlap in characters |
| `batch_max_documents` | integer | No | Max documents per batch |
| `batch_max_chunks` | integer | No | Max chunks per batch |
| `embedding_provider` | string | No | Embedding provider name |
| `embedding_model` | string | No | Embedding model name |
| `embedding_dimensions` | integer | No | Embedding vector dimensions |
| `search_profile` | string | No | Search profile name |
| `rerank_enabled` | boolean | No | Enable reranking |
| `rerank_provider` | string | No | Rerank provider name |
| `summary_enabled` | boolean | No | Enable summarization |
| `external_embedding_allowed` | boolean | No | Allow external embedding calls |
| `external_rerank_allowed` | boolean | No | Allow external rerank calls |
| `external_summary_allowed` | boolean | No | Allow external summary calls |
| `external_ocr_allowed` | boolean | No | Allow external OCR calls |
| `redaction_enabled` | boolean | No | Enable redaction of sensitive content |

### `kb_search`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | Yes | Search query |
| `library_ids` | integer[] | No | Constrain search to specific library IDs (empty = all) |
| `level` | integer | No | RAPTOR tree level filter |
| `top_k` | integer | No | Max results (default 10, max 50) |
| `profile` | string | No | Search profile: fast/balanced/accurate/file_lookup/table |
| `debug` | boolean | No | Enable debug mode (returns planner/rerank/fallback info) |
| `include_context` | boolean | No | Include context before/after matched nodes |
| `context_before` | integer | No | Characters of context before match (default 200) |
| `context_after` | integer | No | Characters of context after match (default 200) |
| `include_highlights` | boolean | No | Return highlight character ranges |
| `group_by_document` | boolean | No | Group results by document |
| `folder_id` | integer | No | Filter to folder |
| `embedding_dimensions` | integer | No | Override embedding dimensions for query vector |
| `embedding_index_id` | integer | No | Specific embedding index ID to use for query vector |

### `kb_list_documents`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `library_id` | integer | Yes | Library ID |
| `folder_id` | integer | No | Filter documents by folder |
| `limit` | integer | No | Max results (default 50) |
| `offset` | integer | No | Pagination offset |

### `upload_source`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | string | Yes | Local file path |
| `intent` | string | No | Upload intent: auto/document/attachment/memory/promote (default auto) |
| `library_id` | integer | No | KB library ID |
| `target_slug` | string | No | Target gbrain page slug (for promotion) |
| `page_slug` | string | No | Target page slug (for file attachment) |
| `folder_id` | integer | No | KB folder ID |
| `promotion` | string | No | Promotion policy: none/shadow/candidate/auto-low-risk |
| `dry_run` | boolean | No | Return routing plan only, don't execute |

### `memory_query`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | Yes | Query text |
| `strategy` | string | No | Query strategy: brain_first/evidence_first/provenance/timeline_first (default brain_first) |
| `limit` | integer | No | Maximum results |
| `filter_slug` | string | No | Filter by slug (applies to all strategies) |
| `include_evidence` | boolean | No | Include KB evidence results |
| `include_provenance` | boolean | No | Include provenance records |

### `promotion_list_candidates`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `status` | string | No | Filter by status: pending/accepted/rejected/applied/rolled_back/stale/superseded |
| `candidate_type` | string | No | Filter by type: document_summary/entity_mention/link_suggestion/timeline_event/fact_claim/page_create/page_update |
| `target_slug` | string | No | Filter by target slug |
| `limit` | integer | No | Maximum results |

### `promotion_batch_apply`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `artifact_id` | integer | No | Filter by Artifact ID |
| `risk` | string | No | Filter by risk level: low/medium/high |
| `dry_run` | boolean | No | Preview only, don't actually apply |

---

## Environment Variables

> **API Compatibility Note**: This project only supports OpenAI-compatible API formats (`/embeddings`, `/chat/completions`, `/audio/transcriptions`). Anthropic/Claude API is not supported. By setting `*_BASE_URL`, you can connect to any OpenAI-compatible service (DeepSeek, Zhipu, DashScope, Ollama, etc.).

### API Key Fallback Chain

Each module's API key falls back in this priority order:

```
Embeddings:  GBRAIN_OPENAI_API_KEY
Expansion:   GBRAIN_EXPANSION_API_KEY → GBRAIN_OPENAI_API_KEY
LLM Chunker: GBRAIN_CHUNKER_API_KEY → GBRAIN_OPENAI_API_KEY
KB RAPTOR:   GBRAIN_KB_RAPTOR_API_KEY → GBRAIN_EXPANSION_API_KEY → GBRAIN_OPENAI_API_KEY
```

Setting only `GBRAIN_OPENAI_API_KEY` enables all AI features. Override per-module for different models/providers.

### Base Configuration

| Variable | Description | Default |
|----------|-------------|---------|
| `GBRAIN_DIR` | Data storage root directory | `~/.gbrain` |
| `GBRAIN_DB_PATH` | Database file path | `$GBRAIN_DIR/brain.db` |
| `GBRAIN_ARTIFACT_STORAGE_DIR` | Artifact original file storage directory | `$GBRAIN_DIR/artifacts` |

### Embeddings

| Variable | Description | Default |
|----------|-------------|---------|
| `GBRAIN_OPENAI_API_KEY` | OpenAI API key (for embeddings; also fallback for other modules) | — |
| `GBRAIN_OPENAI_BASE_URL` | OpenAI-compatible base URL (also fallback for other modules) | — |
| `GBRAIN_EMBEDDING_MODEL` | Embedding model name | `text-embedding-3-large` |
| `GBRAIN_EMBEDDING_DIMENSIONS` | Embedding vector dimensions | `1536` |

### Query Expansion

| Variable | Description | Default |
|----------|-------------|---------|
| `GBRAIN_EXPANSION_API_KEY` | Query expansion LLM API key | Falls back to `GBRAIN_OPENAI_API_KEY` |
| `GBRAIN_EXPANSION_BASE_URL` | Query expansion LLM base URL | Falls back to `GBRAIN_OPENAI_BASE_URL` |
| `GBRAIN_EXPANSION_MODEL` | Query expansion model | `gpt-4o-mini` |

### LLM Chunking

| Variable | Description | Default |
|----------|-------------|---------|
| `GBRAIN_CHUNKER_API_KEY` | LLM chunking API key | Falls back to `GBRAIN_OPENAI_API_KEY` |
| `GBRAIN_CHUNKER_BASE_URL` | LLM chunking base URL | Falls back to `GBRAIN_OPENAI_BASE_URL` |
| `GBRAIN_CHUNKER_MODEL` | LLM chunking model | `gpt-4o-mini` |

### Audio Transcription

| Variable | Description | Default |
|----------|-------------|---------|
| `GBRAIN_TRANSCRIPTION_PROVIDER` | Transcription service provider (`groq` / `openai`) | `groq` |
| `GBRAIN_TRANSCRIPTION_GROQ_API_KEY` | Groq transcription API key | — |
| `GBRAIN_TRANSCRIPTION_GROQ_BASE_URL` | Groq transcription base URL | — |
| `GBRAIN_TRANSCRIPTION_OPENAI_API_KEY` | OpenAI transcription API key | — |
| `GBRAIN_TRANSCRIPTION_OPENAI_BASE_URL` | OpenAI transcription base URL | — |

### KB Subsystem

| Variable | Description | Default |
|----------|-------------|---------|
| `GBRAIN_KB_ENABLED` | Enable KB subsystem | `true` |
| `GBRAIN_KB_RAPTOR_API_KEY` | KB RAPTOR LLM API key | Falls back to `GBRAIN_EXPANSION_API_KEY` |
| `GBRAIN_KB_RAPTOR_BASE_URL` | KB RAPTOR LLM base URL | Falls back to `GBRAIN_EXPANSION_BASE_URL` |
| `GBRAIN_KB_RAPTOR_MODEL` | KB RAPTOR LLM model | `gpt-4o-mini` |
| `GBRAIN_KB_MAX_FILE_SIZE_MB` | KB max file size (MB) | `50` |
| `GBRAIN_KB_ALLOWED_EXTENSIONS` | KB allowed file extensions (comma-separated) | `pdf,docx,xlsx,csv,html,htm,txt,md` |
| `GBRAIN_KB_STORAGE_DIR` | KB file storage directory | — |

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
| `GBRAIN_POST_WRITE_LINT` | Run lint after write | `false` |

### Debug

| Variable | Description | Default |
|----------|-------------|---------|
| `GBRAIN_SEARCH_DEBUG` | Enable search debug logging (set to `1` or `true`) | — |
| `GBRAIN_PROGRESS_MODE` | Progress display mode (`human` / `json` / `quiet`) | Auto-detected |
| `GBRAIN_PROGRESS_JSON` | Set to `"1"` to enable JSON progress mode | — |

---

## Writer Modes

gbrain provides three write strategies, controlled via the `writer_mode` parameter of `put_page`:

| Mode | Description |
|------|-------------|
| `Strict` | Full validation — requires frontmatter, rejects empty content, checks link reference validity |
| `Lint` | Zero-LLM quality checks — runs 6 rules, auto-fixes where possible |
| `Off` | Free write — skips all validation, writes directly |

### Lint Rules

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

Page deletion follows a soft-delete mechanism to prevent accidental data loss:

```
Active page ──delete──→ Soft-deleted (still in storage, not queryable)
                          │
                          ├──restore──→ Restored to active page
                          │
                          └──purge-deleted──→ Permanently deleted (storage freed)
```

- `gbrain delete <slug>` — Soft-delete; page is marked deleted but data is retained
- `gbrain restore <slug>` — Restore a soft-deleted page
- `gbrain purge-deleted --older-than-hours 168` — Permanently purge soft-deleted pages older than 7 days

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

When importing code files via `gbrain import`, Tree-sitter performs AST chunking and regex extracts symbol definitions, references, and call graphs. Query via `gbrain code` commands or MCP tools.

---

## Testing

```bash
cargo test                    # All tests
cargo test --test engine_test # Engine integration tests
cargo test --test search_test # Search integration tests
cargo test --test fuzzy_test  # Fuzzy matching tests
cargo test --test dedup_test  # Deduplication tests
cargo clippy                  # Lint
```

Tests use in-memory SQLite (`:memory:`) — no extra configuration needed.

---

## Architecture

Three-layer design:

1. **Engine Layer** — `BrainEngine` trait → `SqliteEngine` (SQLite + FTS5 + sqlite-vec). Synchronous, direct database operations.

2. **Operations Layer** — Business logic: auto-chunking, tag extraction, link inference, safety validation, batch operations.

3. **Interface Layer** — CLI + MCP server. CLI uses `remote=false`; MCP sets `remote=true` for untrusted callers.

### Search Pipeline

9-step hybrid search pipeline (+ two-stage code graph expansion + dedup):

1. FTS5 BM25 keyword search (weights: title 10x, compiled_truth 5x, timeline 2x)
2. sqlite-vec cosine similarity
3. Fallback to expanded OR query when vector results < 3
4. RRF fusion (k=60) with multi-list support
5. compiled_truth weighted boost
6. Backlink boost
7. Recency boost (time decay)
8. Intent type boost (entity/time/event)
9. 6-layer dedup (slug top-3 → cross-source dedup → text similarity → type diversity → per-page cap → compiled_truth guarantee)

### KB Subsystem Architecture

Async five-stage document processing pipeline:

1. **Parse** — Document parsers (Markdown / PDF / DOCX / XLSX / CSV / HTML / plaintext / code)
2. **Split** — Recursive splitter / Semantic splitter (Savitzky-Golay smoothing + chunk_overlap overlap), switchable via `semantic_enabled` flag
3. **Embed** — Vector embedding generation and persistence
4. **RAPTOR** — Recursive summarization tree (K-Means++ clustering + LLM summarization, three-level fallback chain: library config → `GBRAIN_EXPANSION_*` → `GBRAIN_CHUNKER_*`)
5. **Persist** — Transaction-protected node/vector writes

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
      +-- KB Document Projection -> Document processing pipeline (parse->split->embed->RAPTOR->persist)
      +-- Shadow Page Projection -> Shadow page (extract content -> generate wiki page)
      +-- Promotion Candidate Projection -> Candidate changes (entity mentions/link suggestions/timeline events/fact claims)
      +-- File Attachment Projection -> File attachment (simple file reference)
      +-- Brain Link Projection -> Auto links
      +-- Brain Timeline Projection -> Auto timeline
```

**Core Concepts:**

- **Artifact** — Uploaded original file, with state (active/deleted/purged), source type (upload/sync/link/mcp), upload intent (auto/document/attachment/memory/promote)
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

---

## Documentation

- [TS vs Rust Comparison Report](./docs/compare_report_en.md) / [中文](./docs/compare_report.md) — Comprehensive comparison of TypeScript and Rust versions (code scale, database, search, MCP, security, etc.)
- [TS vs Rust Module-Level Detail](./docs/module_detail_en.md) / [中文](./docs/module_detail.md) — Module-by-module comparison (engine layer, operations layer, search, chunking, enrichment, validators, etc.)

---

## License

MIT License
