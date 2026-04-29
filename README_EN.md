# gbrain-rs

English | [中文](./README.md)

**Personal Knowledge Brain Engine** — Rust implementation of [gbrain](https://github.com/garrytan/gbrain). A zero-config embedded knowledge base with hybrid search, knowledge graph, and MCP agent integration. Built with SQLite + sqlite-vec + FTS5.

> Original TypeScript version by [Garry Tan](https://github.com/garrytan). This is the Rust port. This project is built using **Vibe coding**.

---

## Features

- **Hybrid Search** — BM25 keyword + vector cosine similarity + fuzzy trigram, fused via Reciprocal Rank Fusion with multi-query expansion
- **Knowledge Graph** — Wiki-link extraction, typed links, graph traversal, backlink symmetry validation
- **MCP Server** — Full Model Context Protocol (JSON-RPC 2.0) server for AI agent integration
- **Zero Config** — Embedded SQLite, no external services required (embeddings optional)
- **Tiered Enrichment** — Automatic entity detection and promotion (mention → stub → enriched)
- **Version History** — Full page versioning with revert capability
- **Autopilot** — Self-maintaining daemon for embedding stale content and integrity checks
- **Security** — Path traversal protection, slug validation, input sanitization for remote callers

---

## Build & Install

```bash
cargo build --release          # Build
cargo install --path .         # Install to ~/.cargo/bin/
gbrain install                 # Install to ~/.gbrain/bin/
```

Optional feature:

```bash
cargo build --features file-server   # With axum file server
```

---

## CLI Commands

### Global Options

| Flag | Description |
|------|-------------|
| `--db <PATH>` | Database path |
| `--json` | Output as JSON |
| `--dry-run` | Preview without committing |

### Core

| Command | Description |
|---------|-------------|
| `gbrain init` | Initialize a new brain |
| `gbrain get <slug>` | Read a page by slug |
| `gbrain put <slug> --title <TITLE> [--content <TEXT> \| --file <PATH>]` | Create or update a page |
| `gbrain delete <slug> [--force]` | Delete a page |
| `gbrain list [--page-type <TYPE>] [--limit <N>]` | List pages with filters |
| `gbrain query <query> [--limit <N>]` | Hybrid search (alias: `ask`) |

### Search & Graph

| Command | Description |
|---------|-------------|
| `gbrain resolve <partial>` | Fuzzy-resolve a partial slug |
| `gbrain graph <slug> [--depth <N>]` | Traverse knowledge graph from a page |
| `gbrain graph-query <from> [--to <slug>] [--depth <N>] [--link-type <TYPE>]` | Query graph between pages |

### Backlinks

| Command | Description |
|---------|-------------|
| `gbrain backlinks list <slug>` | List backlinks for a page |
| `gbrain backlinks check [slug]` | Check for missing backlinks |
| `gbrain backlinks fix [slug]` | Fix missing backlinks |

### Data Management

| Command | Description |
|---------|-------------|
| `gbrain embed [slugs...] [--batch-size <N>]` | Generate embeddings for chunks |
| `gbrain import <dir> [--embed] [--auto-link]` | Import markdown files |
| `gbrain export [slugs...] [--dir <DIR>] [--page-type <TYPE>]` | Export pages to markdown |
| `gbrain extract [--mode links\|timeline\|all]` | Batch extract links/timeline |
| `gbrain lint [slug] [--fix] [--dry-run]` | Zero-LLM quality check (6 rules) |

### File Storage

| Command | Description |
|---------|-------------|
| `gbrain file upload <path> [--page <slug>]` | Upload a file |
| `gbrain file list [slug]` | List stored files |
| `gbrain file sync <dir>` | Sync directory to storage |
| `gbrain file verify` | Verify all file records |
| `gbrain file url <storage-path>` | Get local path/URL for a file |

### Health & Maintenance

| Command | Description |
|---------|-------------|
| `gbrain stats` | Brain statistics |
| `gbrain health` | Health dashboard |
| `gbrain doctor [--fast]` | Comprehensive diagnosis |
| `gbrain integrity` | Check data integrity |
| `gbrain orphans` | Detect orphan pages |
| `gbrain autopilot [--once] [--interval <SECS>]` | Self-maintaining daemon |

### Config & Other

| Command | Description |
|---------|-------------|
| `gbrain config show` | Show all config values |
| `gbrain config get <key>` | Get a config value |
| `gbrain config set <key> <value>` | Set a config value |
| `gbrain report --report-type <TYPE> [--title <TITLE>] [--content <TEXT>]` | Generate a brain report |
| `gbrain ingest-log [--limit <N>]` | View ingest log entries |
| `gbrain tools-json` | Output MCP tool definitions as JSON |
| `gbrain mcp` | Run as MCP stdio server |

---

## MCP Tools

gbrain provides 28 MCP tools for AI agent integration via JSON-RPC 2.0 over stdio.

### Search

| Tool | Description |
|------|-------------|
| `query` | Hybrid search (vector + keyword + expansion) with detail levels |
| `search` | Full-text search |
| `find_by_title_fuzzy` | Fuzzy search by title using trigram similarity |
| `resolve_slugs` | Fuzzy-resolve a partial slug to matching pages |

### Page CRUD

| Tool | Description |
|------|-------------|
| `get_page` | Read a page (supports fuzzy matching) |
| `put_page` | Write/update a page with markdown + frontmatter |
| `delete_page` | Delete a page |
| `list_pages` | List pages with type/tag/limit filters |
| `get_chunks` | Get content chunks for a page |

### Tags

| Tool | Description |
|------|-------------|
| `add_tag` | Add tag to a page |
| `remove_tag` | Remove tag from a page |
| `get_tags` | List tags for a page |

### Links & Graph

| Tool | Description |
|------|-------------|
| `add_link` | Create a typed link between pages |
| `remove_link` | Remove a link between pages |
| `get_links` | List outgoing links from a page |
| `get_backlinks` | List incoming links to a page |
| `traverse_graph` | Traverse the link graph from a page |

### Timeline

| Tool | Description |
|------|-------------|
| `add_timeline_entry` | Add a timeline event to a page |
| `get_timeline` | Get timeline entries for a page |

### Versioning

| Tool | Description |
|------|-------------|
| `get_versions` | Page version history |
| `revert_version` | Revert page to a previous version |

### Raw Data

| Tool | Description |
|------|-------------|
| `put_raw_data` | Store raw API response data for a page |
| `get_raw_data` | Retrieve raw data for a page |

### Ingest & Sync

| Tool | Description |
|------|-------------|
| `log_ingest` | Log an ingestion event |
| `get_ingest_log` | Get recent ingestion log entries |
| `sync_brain` | Sync brain from a Git repository |

### Health & Stats

| Tool | Description |
|------|-------------|
| `get_stats` | Brain statistics (page count, chunk count, etc.) |
| `get_health` | Health dashboard (embed coverage, orphans, etc.) |

---

## MCP Tool Parameters

### `query`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | yes | Search query |
| `limit` | integer | no | Max results (default 20) |
| `offset` | integer | no | Pagination offset |
| `expand` | boolean | no | Enable multi-query expansion (default true) |
| `detail` | string | no | `low` / `medium` / `high` (default medium) |

### `put_page`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `slug` | string | yes | Page slug |
| `content` | string | yes | Full markdown with YAML frontmatter |

### `traverse_graph`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `slug` | string | yes | Starting page slug |
| `depth` | integer | no | Max traversal depth (default 5, max 10) |
| `link_type` | string | no | Filter by link type |
| `direction` | string | no | `in` / `out` / `both` (default out) |

### `find_by_title_fuzzy`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | yes | Title to match |
| `dir_prefix` | string | no | Constrain to slug prefix |
| `min_similarity` | number | no | Threshold 0.0–1.0 (default 0.55) |
| `limit` | integer | no | Max results (default 10) |

### `sync_brain`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `repo_path` | string | yes | Path to Git repository |
| `force_full` | boolean | no | Force full sync (default false) |

---

## Environment Variables

> **API Compatibility**: This project only supports OpenAI-compatible API formats (`/embeddings`, `/chat/completions`, `/audio/transcriptions`). Anthropic/Claude API is not supported. By setting `*_BASE_URL`, you can connect to any OpenAI-compatible service (DeepSeek, Zhipu, DashScope, Ollama, etc.).

### Core

| Variable | Description | Default |
|----------|-------------|---------|
| `GBRAIN_DIR` | Data storage root | `~/.gbrain` |
| `GBRAIN_DB_PATH` | Database file path | `$GBRAIN_DIR/brain.db` |

### Embeddings

| Variable | Description | Default |
|----------|-------------|---------|
| `GBRAIN_OPENAI_API_KEY` | OpenAI API key (embeddings; also fallback for other modules) | — |
| `GBRAIN_OPENAI_BASE_URL` | OpenAI-compatible base URL (also fallback for other modules) | — |
| `GBRAIN_EMBEDDING_MODEL` | Embedding model name | `text-embedding-3-large` |
| `GBRAIN_EMBEDDING_DIMENSIONS` | Embedding vector dimensions | `1536` |

### Query Expansion

| Variable | Description | Default |
|----------|-------------|---------|
| `GBRAIN_EXPANSION_API_KEY` | Query expansion LLM API key | Falls back to `GBRAIN_OPENAI_API_KEY` |
| `GBRAIN_EXPANSION_BASE_URL` | Query expansion LLM base URL | Falls back to `GBRAIN_OPENAI_BASE_URL` |
| `GBRAIN_EXPANSION_MODEL` | Query expansion model | `gpt-4o-mini` |

### LLM Chunker

| Variable | Description | Default |
|----------|-------------|---------|
| `GBRAIN_CHUNKER_API_KEY` | LLM chunker API key | Falls back to `GBRAIN_OPENAI_API_KEY` |
| `GBRAIN_CHUNKER_BASE_URL` | LLM chunker base URL | Falls back to `GBRAIN_OPENAI_BASE_URL` |
| `GBRAIN_CHUNKER_MODEL` | LLM chunker model | `gpt-4o-mini` |

### Transcription

| Variable | Description | Default |
|----------|-------------|---------|
| `GBRAIN_TRANSCRIPTION_PROVIDER` | Transcription provider (`groq` / `openai`) | `groq` |
| `GBRAIN_TRANSCRIPTION_GROQ_API_KEY` | Groq transcription API key | — |
| `GBRAIN_TRANSCRIPTION_GROQ_BASE_URL` | Groq transcription base URL | — |
| `GBRAIN_TRANSCRIPTION_OPENAI_API_KEY` | OpenAI transcription API key | — |
| `GBRAIN_TRANSCRIPTION_OPENAI_BASE_URL` | OpenAI transcription base URL | — |

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
| `GBRAIN_PROGRESS_MODE` | Progress display mode (`human` / `json` / `quiet`) | Auto-detect |
| `GBRAIN_PROGRESS_JSON` | Set to `"1"` to enable JSON progress mode | — |

---

## Testing

```bash
cargo test                    # All tests
cargo test --test engine_test # Engine integration tests
cargo test --test search_test # Search integration tests
cargo clippy                  # Lint
```

Tests use in-memory SQLite (`:memory:`) — no setup required.

---

## Architecture

Three-layer design:

1. **Engine Layer** — `BrainEngine` trait → `SqliteEngine` (SQLite + FTS5 + sqlite-vec). Sync, direct DB operations.

2. **Operations Layer** — Business logic: auto-chunking, tag extraction, link inference, security validation, batch operations.

3. **Interface Layer** — CLI + MCP server. CLI uses `remote=false`; MCP sets `remote=true` for untrusted callers.

### Search Pipeline

9-step hybrid search pipeline:

1. FTS5 BM25 keyword search (weighted: title 10x, compiled_truth 5x, timeline 2x)
2. sqlite-vec cosine similarity
3. Fallback broadened OR query when vector returns <3 results
4. RRF fusion (k=60) with multi-list support
5. Compiled truth boost
6. Backlink boost
7. Recency boost (time-decay)
8. Intent-type boost (entity/time/event)
9. 4-layer dedup (slug → compiled_truth priority → score sort → truncate)

---

## Documentation

- [TS vs Rust Comparison Report](./docs/compare_report_en.md) / [中文](./docs/compare_report.md) — Comprehensive comparison of TypeScript and Rust versions (code size, database, search, MCP, security, etc.)
- [TS vs Rust Module-Level Comparison](./docs/module_detail_en.md) / [中文](./docs/module_detail.md) — Per-module comparison (engine layer, operations, search, chunker, enrichment, validators, etc.)

---

## License

MIT License
