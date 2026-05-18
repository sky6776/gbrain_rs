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

# 3. Write to long-term memory
gbrain artifact put people/alice --title "Alice" --content "An engineer skilled in Rust and systems programming"

# 4. Query knowledge
gbrain artifact query "who is Alice"

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
- **MCP Server** — Full Model Context Protocol (JSON-RPC 2.0) server, exposing Artifact facade tools by default; full tool set available with internal tools enabled
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

### Core (Default CLI — Artifact Unified Knowledge Operations)

| Command | Description |
|---------|-------------|
| `gbrain init` | Initialize a new knowledge base |
| `gbrain artifact put <slug> [--title <TITLE>] [--content <TEXT> \| --file <PATH>] [--intent <INTENT>] [--dry-run] [--force]` | Write to long-term memory (intent: memory/evidence/promote) |
| `gbrain artifact upload <path> [--intent <INTENT>] [--target <SLUG>]` | Upload file as knowledge source |
| `gbrain artifact query <query> [--mode <MODE>] [--limit <N>] [--filter <SLUG>] [--include-sources]` | Unified knowledge query (mode: auto/memory/evidence/timeline/graph) |
| `gbrain artifact list [--limit <N>]` | List all Artifacts |
| `gbrain artifact get <id_or_uid>` | Get Artifact detail |
| `gbrain artifact delete <id_or_uid>` | Soft-delete an Artifact |
| `gbrain artifact detach <id_or_uid> --from <slug>` | Detach Artifact from page |
| `gbrain artifact restore <id_or_uid>` | Restore a deleted Artifact |
| `gbrain artifact reprocess <id_or_uid>` | Reprocess Artifact projections |
| `gbrain artifact review list [--status <STATUS>]` | List suggested changes |
| `gbrain artifact review apply <change_id>` | Apply a suggested change |
| `gbrain artifact review reject <change_id>` | Reject a suggested change |
| `gbrain artifact review rollback <change_id>` | Rollback an applied suggested change |
| `gbrain serve` | Run as MCP stdio server |
| `gbrain tools-json` | Output MCP tool definitions as JSON |
| `gbrain config show/get/set` | Configuration management |
| `gbrain doctor [--fast]` | Comprehensive diagnostics |
| `gbrain health` | Health dashboard |

#### Examples

```bash
# Initialize a knowledge base
gbrain init

# Write to long-term memory (default intent: memory)
gbrain artifact put people/alice --title "Alice" --content "An engineer skilled in Rust and systems programming"

# Query knowledge
gbrain artifact query "who is Alice"

# Upload document with auto-routing
gbrain artifact upload report.pdf --intent document

# Preview write routing (dry-run)
gbrain artifact put people/bob --content "Product manager" --dry-run

# Force overwrite a human-modified page
gbrain artifact put people/alice --content "Updated content" --force

# List suggested changes
gbrain artifact review list --status pending

# Apply a suggested change
gbrain artifact review apply 1

# Start MCP server
gbrain serve
```

### Page Operations (Requires admin-tools feature)

> The following commands require `--features admin-tools` to compile. Regular users should use `gbrain artifact put/query` instead.

| Command | Description |
|---------|-------------|
| `gbrain get <slug>` | Read a page by slug |
| `gbrain put <slug> --title <TITLE> [--content <TEXT> \| --file <PATH>] [--page-type <TYPE>]` | Create or update a page |
| `gbrain delete <slug> [--force]` | Soft-delete a page |
| `gbrain restore <slug>` | Restore a soft-deleted page |
| `gbrain purge-deleted [--older-than-hours <N>]` | Permanently clean up old soft-deleted pages |
| `gbrain list [--page-type <TYPE>] [--limit <N>]` | List pages (filterable) |
| `gbrain query <query> [--limit <N>] [--detail <LEVEL>] [--lang <LANG>] [--symbol-kind <KIND>] [--near-symbol <SYMBOL>] [--walk-depth <DEPTH>] [--expand]` | Hybrid search (alias: `ask`), with code filtering and two-stage retrieval |

#### Examples

```bash
# Create a page
gbrain put people/alice --title "Alice" --content "An engineer skilled in Rust and systems programming"

# Search
gbrain query "Rust async programming" --limit 5 --lang rust

# Restore a mistakenly deleted page
gbrain restore people/alice

# Update page content
gbrain put people/alice --title "Alice" --content "A senior engineer skilled in Rust and systems programming, currently at Acme Corp"

# Create page from file
gbrain put projects/alpha --title "Project Alpha" --file ./notes/alpha.md

# Create page with type
gbrain put people/bob --title "Bob" --content "Product manager" --page-type person

# Search code-related content
gbrain query "async runtime" --lang rust --limit 10

# Search with detail level
gbrain query "Acme Corp" --detail high --limit 5

# Two-stage code graph retrieval
gbrain query "handle_request" --near-symbol "HttpHandler" --walk-depth 1

# Keyword search (no expansion)
gbrain query "Rust" --limit 5 --expand false

# List pages by type
gbrain list --page-type person --limit 20

# Permanently purge pages deleted more than 7 days ago
gbrain purge-deleted --older-than-hours 168
```

### Search & Graph (Requires admin-tools feature)

> The following commands require `--features admin-tools` to compile.

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

# Query graph relationship path
gbrain graph-query people/alice --to projects/alpha --depth 2

# Query by link type
gbrain graph-query people/alice --link-type works_at --depth 1

# Find code symbol references
gbrain code refs --symbol "SqliteEngine" --lang rust

# Search code chunks
gbrain code search "async fn handle" --lang rust

# View callers
gbrain code callers --slug src/engine.rs --symbol "query"

# View callees
gbrain code callees --slug src/engine.rs --symbol "query"

# Reindex code page
gbrain code reindex src/engine.rs
```

### Backlinks (Requires admin-tools feature)

> The following commands require `--features admin-tools` to compile.

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

### Data Management (Requires admin-tools feature)

> The following commands require `--features admin-tools` to compile.

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

# Export specific pages
gbrain export people/alice companies/acme --dir ./backup

# Export by type
gbrain export --dir ./people-backup --page-type person

# Extract links only
gbrain extract --mode links

# Extract timeline only
gbrain extract --mode timeline

# Check page quality (no fix)
gbrain lint people/alice

# Preview lint fixes
gbrain lint --fix --dry-run
```

### File Storage (Requires admin-tools feature)

> The following commands require `--features admin-tools` to compile.

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

# Sync directory to storage
gbrain file sync ./documents

# Verify all file records
gbrain file verify

# List all files
gbrain file list
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

# View health dashboard
gbrain health

# Check data integrity
gbrain integrity

# Detect orphan pages
gbrain orphans

# Generate embeddings for specific pages
gbrain embed people/alice companies/acme --batch-size 10
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

# Get single config value
gbrain config get embedding_model

# Enable post-write lint
gbrain config set post_write_lint true

# View ingest log
gbrain ingest-log --limit 10

# Output MCP tool definitions
gbrain tools-json
```

### KB Subsystem (Internal/Advanced — requires expose_internal_tools=true)

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

### Single-Entry Multi-Projection Fusion (Artifact — Legacy Entry, DEPRECATED, use artifact_* instead)

> **DEPRECATED**: The CLI commands below have been replaced by the unified `gbrain artifact ...` commands.
> New users should use `gbrain artifact put/upload/query/list/get/delete/detach/restore`.
> Legacy commands require the `admin-tools` feature.

| Command | Description | Replacement |
|---------|-------------|-------------|
| `gbrain upload <path> [--intent <INTENT>] ...` | Upload a source file (legacy entry) | `gbrain artifact upload` |
| `gbrain memory-query <query> ...` | Unified memory query (legacy entry) | `gbrain artifact query` |
| `gbrain artifact list [--limit <N>] [--offset <N>]` | List all artifacts | `gbrain artifact list` (unchanged) |
| `gbrain artifact get <id_or_uid>` | Get artifact details | `gbrain artifact get` (unchanged) |
| `gbrain artifact delete <artifact_id>` | Soft delete artifact | `gbrain artifact delete` (unchanged) |
| `gbrain artifact health` | Check artifact health | `gbrain artifact health` (unchanged) |

#### Examples

```bash
# Upload a document for KB processing
gbrain upload report.pdf --intent document --library-id 1

# Upload file attachment
gbrain upload photo.jpg --intent attachment --page people/alice

# Upload to KB specific folder
gbrain upload report.pdf --intent document --library-id 1 --folder-id 5

# Unified memory query — search KB evidence first
gbrain memory-query "deployment process" --strategy evidence_first --limit 5

# Unified memory query — trace fact origins
gbrain memory-query "Alice's position" --strategy provenance --include-provenance

# Unified memory query — sort by timeline
gbrain memory-query "recent project updates" --strategy timeline_first

# List all artifacts
gbrain artifact list --limit 20

# Check artifact health
gbrain artifact health

# Soft delete artifact
gbrain artifact delete 42
```

### Candidate Changes & Promotion (Internal/Advanced — requires expose_internal_tools=true)

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

#### Examples

```bash
# List pending candidates
gbrain promotion list --status pending

# Get candidate details
gbrain promotion get 42

# Reject a candidate
gbrain promotion reject 43 --reviewer bob --notes "Outdated information"

# Apply an accepted candidate
gbrain promotion apply 42

# Auto-apply low-risk candidates
gbrain promotion auto-apply 7

# List applied candidates
gbrain promotion list --status applied
```

### Projection Management (Internal/Advanced — requires expose_internal_tools=true)

| Command | Description |
|---------|-------------|
| `gbrain projection supersede <old_proj_id> <new_proj_id>` | Supersede an old projection with a new one (version chain) |
| `gbrain projection history <projection_key> [--artifact-id <ID>] [--projection-type <TYPE>] [--limit <N>]` | Query projection version chain history |
| `gbrain gc-orphan-projections [--stale-days <N>] [--dry-run]` | Garbage collect orphaned/superseded projections |

---

## CLI Command Parameters

### `gbrain artifact put`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `slug` | string | Yes | Target page slug (e.g., people/alice) |
| `--title` | string | No | Page title (optional, inferred from slug by default) |
| `--content` | string | No | Direct text content (alternative to --file) |
| `--file` | path | No | Read content from file (alternative to --content) |
| `--intent` | string | No | Intent: memory / evidence / promote (default memory) |
| `--force` | flag | No | Force overwrite of human-edited pages |
| `--dry-run` | flag | No | Return routing plan only, don't write |

### `gbrain artifact upload`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | path | Yes | File path |
| `--intent` | string | No | Upload intent: auto/document/attachment/memory/promote (default auto) |
| `--target` | string | No | Target gbrain page slug (for promotion) |
| `--page` | string | No | Target page slug (for file attachment) |
| `--library-id` | integer | No | KB library ID |
| `--folder-id` | integer | No | KB folder ID |
| `--promotion` | string | No | Promotion policy: none/shadow/candidate/auto-low-risk |
| `--dry-run` | flag | No | Return routing plan only, don't execute |

### `gbrain artifact query`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | Yes | Query text |
| `--mode` | string | No | Query mode: auto/memory/evidence/timeline/graph (default auto) |
| `--limit` | integer | No | Max results (default 20) |
| `--filter` | string | No | Filter by slug |
| `--include-sources` | flag | No | Include source tracing |

### `gbrain artifact list`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `--limit` | integer | No | Max results |

### `gbrain artifact get`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id_or_uid` | string | Yes | Artifact ID or UID |

### `gbrain artifact delete`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id_or_uid` | string | Yes | Artifact ID or UID |
| `--dry-run` | flag | No | Preview impact, don't delete |

### `gbrain artifact restore`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id_or_uid` | string | Yes | Artifact ID or UID |

### `gbrain artifact review list`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `--status` | string | No | Filter by status (pending/applied/rejected) |

### `gbrain artifact review apply`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `change_id` | string | Yes | Suggested change ID |

### `gbrain artifact review reject`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `change_id` | string | Yes | Suggested change ID |

### `gbrain artifact review rollback`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `change_id` | string | Yes | Suggested change ID |

### Admin/Legacy CLI Parameters (requires admin-tools feature)

> The following commands require `--features admin-tools` to compile. Regular users should use `gbrain artifact put/query` instead.

### `gbrain put` [ADMIN]

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `slug` | string | Yes | Page slug (e.g., people/alice) |
| `--title` | string | Yes | Page title |
| `--content` | string | No | Page content (Markdown) |
| `--file` | path | No | Read content from file |
| `--page-type` | string | No | Page type (e.g., person, company) |

### `gbrain query` [ADMIN]

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | Yes | Search query |
| `--limit` | integer | No | Max results (default 20) |
| `--detail` | string | No | Result detail level: low/medium/high (default medium) |
| `--lang` | string | No | Filter code retrieval by programming language |
| `--symbol-kind` | string | No | Filter code retrieval by symbol type |
| `--near-symbol` | string | No | Anchor symbol for two-stage code graph retrieval |
| `--walk-depth` | integer | No | Code graph neighbor walk depth (0-2, default 0) |
| `--expand` | flag | No | Enable LLM query expansion |

### `gbrain upload` [ADMIN]

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | path | Yes | File path |
| `--intent` | string | No | Upload intent: auto/document/attachment/memory/promote (default auto) |
| `--library-id` | integer | No | KB library ID |
| `--target` | string | No | Target gbrain page slug (for promotion) |
| `--page` | string | No | Target page slug (for file attachment) |
| `--folder-id` | integer | No | KB folder ID |
| `--promotion` | string | No | Promotion policy: none/shadow/candidate/auto-low-risk |
| `--dry-run` | flag | No | Return routing plan only, don't execute |
| `--json` | flag | No | Output as JSON |

### `gbrain memory-query` [ADMIN]

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | Yes | Query text |
| `--strategy` | string | No | Query strategy: brain_first/evidence_first/provenance/timeline_first (default brain_first) |
| `--limit` | integer | No | Max results (default 10) |
| `--filter-slug` | string | No | Filter by slug |
| `--include-evidence` | flag | No | Include KB evidence (default true) |
| `--include-provenance` | flag | No | Include provenance records (default false) |
| `--json` | flag | No | Output as JSON |

### `gbrain embed`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `slugs` | string[] | No | Specific page slugs (empty = all stale content) |
| `--batch-size` | integer | No | Embedding API batch size (default 20) |

### `gbrain import`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `dir` | string | Yes | Directory to scan for .md files |
| `--embed` | flag | No | Generate embeddings for imported content |
| `--auto-link` | flag | No | Auto-link imported pages to existing pages |

### `gbrain export`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `slugs` | string[] | No | Specific page slugs to export |
| `--dir` | string | No | Output directory |
| `--page-type` | string | No | Filter by page type |

### `gbrain autopilot`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `--once` | flag | No | Run once and exit |
| `--interval` | integer | No | Cycle interval in seconds (default 3600) |

### `gbrain purge-deleted`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `--older-than-hours` | integer | No | Purge pages deleted more than N hours ago (default 72) |

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

gbrain exposes only Artifact unified knowledge operation facade tools (`artifact_*`) by default, via JSON-RPC 2.0 over stdio.

Set `GBRAIN_EXPOSE_INTERNAL_TOOLS=true` or use `--expose-internal-tools` to expose internal tools (`kb_*`, `promotion_*`, `projection_*`, etc.).

### Artifact Unified Knowledge Operations (default)

| Tool | Description |
|------|-------------|
| `artifact_put` | Write manual memory (slug + content + intent) |
| `artifact_upload` | Upload file as knowledge source (PDF/DOCX/MD etc.) |
| `artifact_query` | Unified knowledge query (memory/evidence/timeline/graph modes) |
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

#### Examples

```json
// Write manual memory
{ "tool": "artifact_put", "params": { "slug": "rust-async", "content": "Rust async programming uses async/await syntax...", "intent": "memory" } }
```

```json
// Unified knowledge query with source tracing
{ "tool": "artifact_query", "params": { "query": "Rust async programming", "mode": "auto", "include_sources": true } }
```

```json
// Get knowledge source details
{ "tool": "artifact_get", "params": { "id_or_uid": "art_abc123", "include_sources": true, "include_projections": true } }
```

```json
// Upload file
{ "tool": "artifact_upload", "params": { "path": "/path/to/doc.pdf", "intent": "memory", "library_id": 1 } }
```

```json
// List suggested changes
{ "tool": "artifact_review_list", "params": { "status": "pending" } }
```

### Internal Tools (requires expose_internal_tools=true)

The following tools are only available when `GBRAIN_EXPOSE_INTERNAL_TOOLS=true`:

- **gbrain page operations**: `query`, `get_page`, `put_page`, `delete_page`, `list_pages`, `add_tag`, `remove_link`, `get_links`, `traverse_graph`, `add_timeline_entry`, `get_timeline`, `get_stats`, `get_health`, `get_versions`, `resolve_slugs`, `find_by_title_fuzzy`, `get_chunks`
- **KB subsystem**: `kb_list_libraries`, `kb_upload_document`, `kb_search`, `kb_get_document_status`, `kb_delete_document`, `kb_purge_document`, `kb_check_index_health`, `kb_backup`, `kb_restore`
- **Internal knowledge operations**: `upload_source`, `memory_query`, `get_provenance`
- **Review/projection internals**: `promotion_list_candidates`, `promotion_apply_candidate`, `promotion_rollback_candidate`, `gc_orphan_projections`, `projection_supersede`, `projection_history`

### Search (internal)

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

```json
// Search with metadata
{ "tool": "query", "params": { "query": "Rust async", "limit": 5, "include_meta": true } }
```

```json
// Two-stage code graph retrieval
{ "tool": "query", "params": { "query": "handle_request", "near_symbol": "HttpHandler", "walk_depth": 1 } }
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

```json
// Read page with fuzzy matching
{ "tool": "get_page", "params": { "slug": "ali", "fuzzy": true } }
```

```json
// Get page content chunks
{ "tool": "get_chunks", "params": { "slug": "people/alice" } }
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

```json
// Remove a tag
{ "tool": "remove_tag", "params": { "slug": "people/alice", "tag": "engineer" } }
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

```json
// Remove a link
{ "tool": "remove_link", "params": { "from": "people/alice", "to": "companies/acme", "link_type": "works_at" } }
```

```json
// Traverse graph by link type
{ "tool": "traverse_graph", "params": { "slug": "people/alice", "depth": 3, "link_type": "works_at", "direction": "out" } }
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

```json
// Revert to a specific version (creates a new version record)
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

```json
// Search code chunks
{ "tool": "search_code_chunks", "params": { "query": "async fn handle", "lang": "rust" } }
```

```json
// Get code edges
{ "tool": "get_code_edges_by_chunk", "params": { "chunk_id": 42 } }
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

### KB Subsystem (Internal/Advanced — requires expose_internal_tools=true)

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

### Single-Entry Multi-Projection Fusion (Artifact — Legacy Entry, DEPRECATED, use artifact_* instead)

> **DEPRECATED**: The tools below have been replaced by the unified `artifact_*` facade tools.
> New users should use the "Artifact Unified Knowledge Operations" section above.
> Legacy tools are only available when `GBRAIN_EXPOSE_INTERNAL_TOOLS=true`.

| Tool | Description | Replacement |
|------|-------------|-------------|
| `upload_source` | Upload a source file (legacy entry) | `artifact_upload` |
| `memory_query` | Unified memory query (legacy entry) | `artifact_query` |
| `artifact_list` | List all artifacts | `artifact_list` (unchanged) |
| `artifact_get` | Get artifact details | `artifact_get` (unchanged) |
| `artifact_delete` | Soft delete artifact | `artifact_delete` (unchanged) |
| `artifact_health` | Check artifact health | `artifact_health` (unchanged) |
| `get_provenance` | Get provenance records | `artifact_get` + `include_sources=true` |

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

```json
// Upload file attachment
{ "tool": "upload_source", "params": { "path": "/path/to/photo.jpg", "intent": "attachment", "page_slug": "people/alice" } }
```

```json
// Preview upload routing
{ "tool": "upload_source", "params": { "path": "/path/to/report.pdf", "intent": "auto", "dry_run": true } }
```

```json
// Memory query — trace origins
{ "tool": "memory_query", "params": { "query": "Alice's position", "strategy": "provenance", "include_provenance": true } }
```

### Candidate Changes & Promotion (Internal/Advanced — requires expose_internal_tools=true)

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

### Projection Management (Internal/Advanced — requires expose_internal_tools=true)

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

### `artifact_put`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `slug` | string | Yes | Target page slug (e.g., people/alice) |
| `content` | string | No | Direct text content (alternative to file) |
| `file` | string | No | Local file path (alternative to content) |
| `title` | string | No | Page title (optional, inferred from slug by default) |
| `intent` | string | No | Intent: memory / evidence / promote (default memory) |
| `force` | boolean | No | Force overwrite of human-edited pages (default false, returns resolution=conflict on conflict) |
| `dry_run` | boolean | No | Return routing plan only, don't write |

### `artifact_upload`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | string | Yes | Local file path |
| `intent` | string | No | Upload intent: auto / evidence / memory / attachment / promote (default auto) |
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
| `mode` | string | No | Query mode: auto / memory / evidence / timeline / graph (default auto) |
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

### `artifact_review_rollback`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `change_id` | integer | Yes | Change ID |

---

## Internal Tools Parameters (requires expose_internal_tools=true)

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

### `get_page`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `slug` | string | Yes | Page slug |
| `fuzzy` | boolean | No | Enable fuzzy slug resolution (default false) |

### `add_link`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `from` | string | Yes | Source page slug |
| `to` | string | Yes | Target page slug |
| `link_type` | string | No | Link type (e.g., works_at, invested_in) |
| `context` | string | No | Context description for the link |

### `list_pages`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `type` | string | No | Filter by page type |
| `tag` | string | No | Filter by tag |
| `limit` | integer | No | Max results (default 50) |

### `delete_page`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `slug` | string | Yes | Page slug |
| `confirm` | boolean | Yes | Must be true to confirm deletion |

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

### `add_tag`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `slug` | string | Yes | Page slug |
| `tag` | string | Yes | Tag name |

### `remove_tag`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `slug` | string | Yes | Page slug |
| `tag` | string | Yes | Tag name |

### `add_timeline_entry`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `slug` | string | Yes | Page slug |
| `date` | string | Yes | Date (YYYY-MM-DD) |
| `summary` | string | Yes | Event summary |

### `remove_link`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `from` | string | Yes | Source page slug |
| `to` | string | Yes | Target page slug |
| `link_type` | string | No | Link type to remove |

### `revert_version`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `slug` | string | Yes | Page slug |
| `version_id` | integer | Yes | Version ID to revert to |

### `put_raw_data`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `slug` | string | Yes | Page slug |
| `source` | string | Yes | Data source identifier |
| `data` | object | Yes | Raw data object |

### `get_raw_data`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `slug` | string | Yes | Page slug |
| `source` | string | No | Filter by source |

### `resolve_slugs`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `partial` | string | Yes | Partial slug |

### `log_ingest`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `source_type` | string | Yes | Source type (e.g., git, import, api) |
| `source_ref` | string | Yes | Source reference |
| `pages_updated` | string[] | Yes | List of updated page slugs |
| `summary` | string | Yes | Ingestion summary |

### `kb_update_library`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `library_id` | integer | Yes | Library ID to update |
| `name` | string | No | New library name |
| `semantic_segmentation_enabled` | boolean | No | Enable semantic chunking |
| `raptor_enabled` | boolean | No | Enable RAPTOR tree summarization |
| `chunk_size` | integer | No | Chunk size in characters |
| `chunk_overlap` | integer | No | Chunk overlap in characters |
| `embedding_provider` | string | No | Embedding provider name |
| `embedding_model` | string | No | Embedding model name |
| `embedding_dimensions` | integer | No | Embedding vector dimensions |
| `search_profile` | string | No | Search profile name |
| `rerank_enabled` | boolean | No | Enable reranking |
| `rerank_provider` | string | No | Rerank provider name |
| `summary_enabled` | boolean | No | Enable summarization |
| `redaction_enabled` | boolean | No | Enable sensitive content redaction |

### `kb_delete_library`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `library_id` | integer | Yes | Library ID to delete |
| `confirm` | boolean | Yes | Must be true to confirm deletion |

### `kb_upload_document`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `library_id` | integer | Yes | Target library ID |
| `file_path` | string | Yes | Local file path |
| `folder_id` | integer | No | Folder ID |

### `kb_get_document_status`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `document_id` | integer | Yes | Document ID |

### `kb_retry_document`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `document_id` | integer | Yes | Document ID to retry |

### `kb_cancel_document_job`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `document_id` | integer | Yes | Document ID to cancel |

### `kb_delete_document`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `document_id` | integer | Yes | Document ID to delete |
| `confirm` | boolean | Yes | Must be true to confirm deletion |

### `kb_purge_document`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `document_id` | integer | Yes | Document ID to permanently destroy |
| `confirm` | boolean | Yes | Must be true to confirm permanent destruction |

### `kb_create_folder`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `library_id` | integer | Yes | Library ID |
| `name` | string | Yes | Folder name |
| `parent_id` | integer | No | Parent folder ID (null = root) |

### `kb_add_eval_query`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `library_id` | integer | Yes | Library ID |
| `query` | string | Yes | Evaluation query text |
| `query_type` | string | No | Query type classification |
| `expected_document_ids` | string | No | Comma-separated expected document IDs |

### `kb_add_search_feedback`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `search_log_id` | integer | No | Search log entry ID |
| `document_id` | integer | No | Document ID rated |
| `node_id` | integer | No | Node ID rated |
| `rating` | integer | Yes | Relevance rating 0-5 |
| `comment` | string | No | Feedback comment |

### `code_def`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `symbol` | string | Yes | Qualified or local symbol name |
| `lang` | string | No | Filter by programming language |
| `limit` | integer | No | Max results (default 20) |

### `code_refs`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `symbol` | string | Yes | Qualified or local symbol name |
| `lang` | string | No | Filter by programming language |
| `limit` | integer | No | Max results (default 20) |

### `search_code_chunks`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | Yes | Code search query |
| `limit` | integer | No | Max results (default 20) |
| `lang` | string | No | Filter by programming language |
| `symbol_kind` | string | No | Filter by symbol kind |

### `get_callers`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `slug` | string | Yes | Code page slug |
| `symbol` | string | Yes | Qualified or local symbol name |

### `get_callees`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `slug` | string | Yes | Code page slug |
| `symbol` | string | Yes | Qualified or local symbol name |

### `get_code_edges_by_chunk`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `chunk_id` | integer | Yes | Code chunk ID |

### `reindex_code_page`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `slug` | string | Yes | Code page slug |

### `file_upload`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | string | Yes | Local file path |
| `page_slug` | string | No | Associated page slug |

### `promotion_get_candidate`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `candidate_id` | integer | Yes | Candidate ID |

### `promotion_accept_candidate`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `candidate_id` | integer | Yes | Candidate ID |
| `reviewer` | string | No | Reviewer name |
| `notes` | string | No | Review notes |

### `promotion_reject_candidate`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `candidate_id` | integer | Yes | Candidate ID |
| `reviewer` | string | No | Reviewer name |
| `reason` | string | No | Rejection reason |

### `promotion_apply_candidate`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `candidate_id` | integer | Yes | Candidate ID |

### `promotion_rollback_candidate`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `candidate_id` | integer | Yes | Candidate ID to rollback |

### `gc_orphan_projections`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `stale_days` | integer | No | Purge projections orphaned for more than N days (default 30) |
| `dry_run` | boolean | No | Preview only, don't actually purge |

### `projection_supersede`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `old_proj_id` | integer | Yes | Old projection ID |
| `new_proj_id` | integer | Yes | New projection ID |

### `projection_history`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `projection_key` | string | Yes | Projection key |
| `artifact_id` | integer | No | Filter by Artifact ID |
| `projection_type` | string | No | Filter by projection type |
| `limit` | integer | No | Max records (default 20) |

### `get_provenance`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `brain_slug` | string | Yes | Brain page slug |

---

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

### KB Subsystem (Internal/Advanced — requires expose_internal_tools=true)

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

### KB Subsystem (Internal/Advanced — requires expose_internal_tools=true) Architecture

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
