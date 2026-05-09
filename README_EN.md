# gbrain-rs

中文 | [English](./README_EN.md)

**Personal Knowledge Brain Engine** — Rust port of [gbrain](https://github.com/garrytan/gbrain), with added KB subsystem (async document processing pipeline + RAPTOR recursive summarization tree), full Chinese NLP support (jieba tokenization + pinyin + FTS5 query rewriting), soft-delete lifecycle (restore/purge-deleted), time-decay search, and more. Built on SQLite + sqlite-vec + FTS5 with a zero-config embedded architecture — ready to use out of the box.

> The original TypeScript version was developed by [Garry Tan](https://github.com/garrytan). Built with **Vibe coding**.

---

## Features

- **Hybrid Search** — BM25 keywords + vector cosine similarity + fuzzy trigrams, merged via Reciprocal Rank Fusion (RRF), with multi-query expansion
- **Knowledge Graph** — Wiki-link extraction, typed links, graph traversal, backlink symmetry verification
- **KB Subsystem** — Async five-stage document processing pipeline (parse → split → embed → RAPTOR → persist), RAPTOR recursive summarization tree, document upload and processing, multi-format parsers (Markdown/PDF/DOCX/XLSX/CSV/HTML/plaintext/code), semantic chunking (Savitzky-Golay smoothing + chunk_overlap overlap)
- **Chinese NLP** — jieba tokenization + pinyin + prefix wildcards, FTS5 query auto-rewriting, Chinese punctuation sentence-breaking and token counting, pre-tokenized column auto-sync
- **MCP Server** — Full Model Context Protocol (JSON-RPC 2.0) server with 51 tools for AI agent integration
- **Zero Config** — Embedded SQLite, no external services required (embeddings optional)
- **Layered Enrichment** — Automatic entity detection and promotion (mention → stub → enriched)
- **Version History** — Full page versioning with rollback
- **Autopilot** — Self-maintenance daemon that auto-embeds stale content and runs integrity checks
- **Safety Guards** — Path traversal protection, slug validation, remote-call input sanitization, parameterized queries against SQL injection

---

## Build & Install

```bash
cargo build --release          # Build
cargo install --path .         # Install to ~/.cargo/bin/
gbrain install                 # Install to ~/.gbrain/bin/
```

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

### Search & Graph

| Command | Description |
|---------|-------------|
| `gbrain resolve <partial>` | Fuzzy-resolve a partial slug |
| `gbrain graph <slug> [--depth <N>]` | Traverse the knowledge graph from a page |
| `gbrain graph-query <from> [--to <slug>] [--depth <N>] [--link-type <TYPE>]` | Query graph relationships between pages |
| `gbrain code search/def/refs/callers/callees/edges` | Code chunk, symbol definition/reference, and call graph queries |

### Backlinks

| Command | Description |
|---------|-------------|
| `gbrain backlinks list <slug>` | List backlinks for a page |
| `gbrain backlinks check [slug]` | Check for missing backlinks |
| `gbrain backlinks fix [slug]` | Fix missing backlinks |

### Data Management

| Command | Description |
|---------|-------------|
| `gbrain embed [slugs...] [--batch-size <N>]` | Generate and persist embeddings for stale chunks |
| `gbrain import <dir> [--embed] [--auto-link]` | Import Markdown and supported code files; skips when frontmatter slug mismatches path |
| `gbrain export [slugs...] [--dir <DIR>] [--page-type <TYPE>]` | Export pages as Markdown |
| `gbrain extract [--mode links\|timeline\|all]` | Batch extract links/timeline |
| `gbrain lint [slug] [--fix] [--dry-run]` | Zero-LLM quality checks (6 rules) |

### File Storage

| Command | Description |
|---------|-------------|
| `gbrain file upload <path> [--page <slug>]` | Upload a file |
| `gbrain file list [slug]` | List stored files |
| `gbrain file sync <dir>` | Sync a directory to storage |
| `gbrain file verify` | Verify all file records |
| `gbrain file url <storage-path>` | Get local path/URL for a file |

### Health & Maintenance

| Command | Description |
|---------|-------------|
| `gbrain stats` | Knowledge base statistics |
| `gbrain health` | Health dashboard |
| `gbrain doctor [--fast]` | Comprehensive diagnostics |
| `gbrain integrity` | Check data integrity |
| `gbrain orphans` | Detect orphan pages |
| `gbrain autopilot [--once] [--interval <SECS>]` | Self-maintenance daemon |

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

---

## MCP Tools

gbrain provides 51 MCP tools for AI agent integration via JSON-RPC 2.0 over stdio.

### Search

| Tool | Description |
|------|-------------|
| `query` | Hybrid search (vector + keyword + expansion), with detail levels, code filtering, two-stage retrieval, and search metadata |
| `search` | Full-text search (vector + keyword + RRF fusion), with code filtering |
| `find_by_title_fuzzy` | Fuzzy title search based on trigram similarity |
| `resolve_slugs` | Fuzzy-resolve partial slugs to matching pages |

### Page CRUD

| Tool | Description |
|------|-------------|
| `get_page` | Read a page (supports fuzzy matching) |
| `put_page` | Write/update a page (Markdown + frontmatter) |
| `delete_page` | Soft-delete a page |
| `list_pages` | List pages (filter by type/tag/limit) |
| `get_chunks` | Get content chunks for a page |

### Tags

| Tool | Description |
|------|-------------|
| `add_tag` | Add a tag to a page |
| `remove_tag` | Remove a tag from a page |
| `get_tags` | List tags for a page |

### Links & Graph

| Tool | Description |
|------|-------------|
| `add_link` | Create a typed link between pages |
| `remove_link` | Remove a link between pages |
| `get_links` | List outbound links for a page |
| `get_backlinks` | List inbound links for a page |
| `traverse_graph` | Traverse the link graph from a page |

### Timeline

| Tool | Description |
|------|-------------|
| `add_timeline_entry` | Add a timeline entry to a page |
| `get_timeline` | Get timeline for a page |

### Versioning

| Tool | Description |
|------|-------------|
| `get_versions` | Page version history |
| `revert_version` | Revert a page to a previous version |

### Raw Data

| Tool | Description |
|------|-------------|
| `put_raw_data` | Store raw API response data for a page |
| `get_raw_data` | Get raw data for a page |

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

### File Storage

| Tool | Description |
|------|-------------|
| `file_upload` | Upload a file to storage |
| `file_list` | List stored files |
| `file_url` | Get URL/path for a file |

### Import & Sync

| Tool | Description |
|------|-------------|
| `log_ingest` | Log an ingest event |
| `get_ingest_log` | Get recent ingest log |
| `sync_brain` | Sync knowledge base from a Git repo |
| `find_orphans` | Find orphan pages with no inbound links |

### Health & Stats

| Tool | Description |
|------|-------------|
| `get_stats` | Knowledge base statistics (page count, chunk count, etc.) |
| `get_health` | Health dashboard (embedding coverage, orphan pages, etc.) |

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

### `kb_search`

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | Yes | Search query |
| `library_ids` | integer[] | No | Constrain search to specific library IDs (empty = all) |
| `level` | integer | No | RAPTOR tree level filter |
| `top_k` | integer | No | Max results (default 10, max 50) |

---

## Environment Variables

> **API Compatibility Note**: This project only supports OpenAI-compatible API formats (`/embeddings`, `/chat/completions`, `/audio/transcriptions`). Anthropic/Claude API is not supported. By setting `*_BASE_URL`, you can connect to any OpenAI-compatible service (DeepSeek, Zhipu, DashScope, Ollama, etc.).

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

## Testing

```bash
cargo test                    # All tests
cargo test --test engine_test # Engine integration tests
cargo test --test search_test # Search integration tests
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

9-step hybrid search pipeline:

1. FTS5 BM25 keyword search (weights: title 10x, compiled_truth 5x, timeline 2x)
2. sqlite-vec cosine similarity
3. Fallback to expanded OR query when vector results < 3
4. RRF fusion (k=60) with multi-list support
5. compiled_truth weighted boost
6. Backlink boost
7. Recency boost (time decay)
8. Intent type boost (entity/time/event)
9. 4-layer dedup (slug → compiled_truth priority → score sort → truncation)

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

---

## Documentation

- [TS vs Rust Comparison Report](./docs/compare_report_en.md) / [中文](./docs/compare_report.md) — Comprehensive comparison of TypeScript and Rust versions (code scale, database, search, MCP, security, etc.)
- [TS vs Rust Module-Level Detail](./docs/module_detail_en.md) / [中文](./docs/module_detail.md) — Module-by-module comparison (engine layer, operations layer, search, chunking, enrichment, validators, etc.)

---

## License

MIT License
