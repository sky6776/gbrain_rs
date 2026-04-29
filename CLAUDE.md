# gbrain-core

Personal knowledge brain engine — Rust implementation using SQLite + sqlite-vec + FTS5. Provides zero-config embedded knowledge base with hybrid search, knowledge graph, and MCP agent integration.

## Project Structure

```text
src/
├── lib.rs              # Library entry, public re-exports
├── types.rs            # Core types (Page, Chunk, Link, SearchResult, etc.)
├── error.rs            # Error types (GBrainError, OperationError)
├── engine.rs           # BrainEngine trait (55 methods)
├── sqlite_engine.rs    # SqliteEngine implementation (SQLite + FTS5 + sqlite-vec)
├── schema.rs           # SQLite DDL (tables, indexes, triggers, migrations V1-V7)
├── config.rs           # Config loading (env vars + config.json)
├── operations.rs       # Operations business logic layer
├── autopilot.rs        # Periodic maintenance orchestrator (embedding stale content, integrity checks)
├── enrichment.rs       # Enrichment pipeline with tiered entity detection (Tier1/2/3)
├── validators.rs       # Content validators (back-links, citation, source, symmetry)
├── writer.rs           # Brain writer with strict/lint/off validation modes
├── resolver.rs         # Slug resolution registry with fuzzy/keyword fallback
├── scaffold.rs         # Deterministic citation builders (tweets, emails — no LLM-hallucinated URLs)
├── completeness.rs     # Page quality scoring across 7 weighted rubrics
├── lint.rs             # Zero-LLM quality checker (6 rules: preamble, placeholder-date, etc.)
├── sync.rs             # Git-based markdown import with manifest tracking
├── jobs.rs             # SQLite-backed persistent job queue with priority/retry
├── budget.rs           # Daily API spend cap with atomic reserve/commit/rollback
├── backoff.rs          # Exponential-backoff-with-jitter retry wrapper
├── fail_improve.rs     # Deterministic-first/LLM-fallback failure analysis loop (JSONL)
├── progress.rs         # Structured progress reporter (auto/human/json/quiet modes + ETA)
├── logging.rs          # Logging init (configurable level, file rotation, console)
├── markdown.rs         # Frontmatter parsing + body splitting
├── link_extraction.rs  # Entity reference extraction + auto-link inference
├── embedding.rs        # OpenAI embedding client (batch, retry, backoff)
├── transcription.rs    # Audio transcription (Groq Whisper / OpenAI Whisper)
├── security.rs         # Security validation (path, slug, filename)
├── file_storage.rs     # File storage (upload, list, URL)
├── chunker/
│   ├── mod.rs          # Chunker module entry
│   ├── recursive.rs    # Recursive text chunker (default, deterministic)
│   ├── semantic.rs     # Semantic chunker (sentence-embedding similarity + Savitzky-Golay)
│   └── llm.rs         # LLM-guided semantic chunker
├── search/
│   ├── mod.rs          # Search entry point
│   ├── keyword.rs      # FTS5 BM25 keyword search (weighted columns)
│   ├── vector.rs       # sqlite-vec cosine similarity search
│   ├── hybrid.rs       # RRF fusion + multi-list support + fallback + boosting
│   ├── intent.rs       # Query intent classification (entity/time/event/general)
│   ├── dedup.rs        # 4-layer dedup + compiled_truth guarantee
│   ├── expansion.rs    # Query expansion + injection defense
│   ├── fuzzy.rs        # Character-trigram Jaccard similarity (pg_trgm equivalent)
│   └── eval.rs         # Search quality eval framework (P@k, R@k, MRR, nDCG@k)
├── mcp/
│   ├── mod.rs          # MCP stdio server (JSON-RPC 2.0)
│   └── tool_defs.rs    # MCP tool definitions
└── bin/
    └── gbrain.rs       # CLI entry point
tests/
├── engine_test.rs      # Engine CRUD integration tests
├── search_test.rs      # Search integration tests
├── dedup_test.rs       # Dedup logic tests
└── fuzzy_test.rs       # Fuzzy search integration tests
```

## Build & Run

```bash
cargo build                  # Debug build
cargo build --release       # Release build (binary at ./target/release/gbrain)
cargo build --features file-server  # With optional axum file server
```

## Test

```bash
cargo test                  # All tests (unit + integration)
cargo test --test engine_test   # Engine integration tests only
cargo test --test search_test   # Search integration tests only
cargo test --test dedup_test    # Dedup tests only
cargo test --test fuzzy_test    # Fuzzy search tests only
cargo clippy                # Lint
```

Tests use in-memory SQLite (`:memory:`) — no test database setup needed.

## Architecture

Three-layer design:

1. **Engine layer** — `BrainEngine` trait (`engine.rs`, 55 methods) → `SqliteEngine` impl (`sqlite_engine.rs`). Sync, direct SQLite operations. NOT dyn-compatible; use concrete `SqliteEngine`.
2. **Operations layer** — `Operations` struct (`operations.rs`). Business logic: auto-chunking, tag extraction, link inference, security validation, batch operations. Both CLI and MCP dispatch through this. Configurable via `Operations::with_config()` for auto_link/auto_timeline flags.
3. **Interface layer** — CLI (`bin/gbrain.rs`) + MCP server (`mcp/`). CLI uses `OpContext.remote=false`; MCP sets `remote=true` for untrusted callers.

### Search Pipeline

9-step pipeline:

1. FTS5 BM25 keyword search (weighted: title 10x, compiled_truth 5x, timeline 2x)
2. sqlite-vec cosine similarity (requires embedding)
3. Fallback broadened OR query when vector returns <3 results
4. RRF fusion (k=60) with multi-list support for query expansion
5. Compiled truth boost (conditional on detail level)
6. Backlink boost
7. Recency boost (time-decay: `1 / (1 + days_since_update / half_life)`)
8. Intent-type boost (entity intent → entity pages; time/event → pages with timeline)
9. 4-layer dedup: slug dedup → compiled_truth priority → score sort → truncate

### Enrichment Pipeline

Tiered entity detection and auto-promotion:
- **Tier 3**: Mentioned entity — stub page created with backlink

- **Tier 2**: Multiple mentions — auto-promoted with timeline entry

- **Tier 1**: Fully enriched — complete page with sources/citations

### Autopilot

Periodic maintenance orchestrator (`autopilot.rs`): embeds stale content, runs integrity checks, reports health. Designed for scheduled/cron execution.

### Writer & Validation

Three strictness modes (`writer.rs`):

- **Strict**: Block writes on validation errors
- **Lint**: Log warnings but proceed
- **Off**: No validation

Validators (`validators.rs`): back-link checks, citation format, source references, link symmetry, separator rules.

### Job Queue & Budget

- `jobs.rs`: SQLite-backed persistent job queue with priority ordering, retry/max-attempts, status transitions
- `budget.rs`: Daily API spend cap using atomic reserve/commit/rollback with TTL cleanup

### Security Boundary

- `OpContext.remote` distinguishes local (CLI) vs remote (MCP) callers
- Remote callers: path traversal blocked, file uploads confined to working directory
- Slug validation: allowed prefixes only, lowercase alphanumeric + hyphens
- Filename validation: extension whitelist, no path separators

## Chunking

Three strategies available:
- **Recursive** (`chunker/recursive.rs`) — Fast, deterministic, splits by paragraph and character count. Default.
- **Semantic** (`chunker/semantic.rs`) — Sentence-embedding similarity with Savitzky-Golay smoothing to detect topic boundaries.
- **LLM-guided** (`chunker/llm.rs`) — Uses OpenAI-compatible LLM to identify natural section boundaries. Configured via `GBRAIN_CHUNKER_*` env vars, falls back to `GBRAIN_OPENAI_*`.

## Transcription

Audio transcription via Groq Whisper (default, fast) or OpenAI Whisper (fallback). Large files (>25MB) segmented via ffmpeg. Provider selected via `GBRAIN_TRANSCRIPTION_PROVIDER` env var.

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `GBRAIN_DIR` | Data storage root | `~/.gbrain` |
| `GBRAIN_DB_PATH` | Database file path | `$GBRAIN_DIR/brain.db` |
| `GBRAIN_OPENAI_API_KEY` | OpenAI API key (embeddings) | — |
| `GBRAIN_OPENAI_BASE_URL` | OpenAI-compatible base URL | — |
| `GBRAIN_EMBEDDING_MODEL` | Embedding model name | `text-embedding-3-large` |
| `GBRAIN_EMBEDDING_DIMENSIONS` | Embedding dimensions | `1536` |
| `GBRAIN_EXPANSION_API_KEY` | Query expansion API key | falls back to `GBRAIN_OPENAI_API_KEY` |
| `GBRAIN_EXPANSION_BASE_URL` | Query expansion base URL | falls back to `GBRAIN_OPENAI_BASE_URL` |
| `GBRAIN_EXPANSION_MODEL` | Query expansion model | `gpt-4o-mini` |
| `GBRAIN_CHUNKER_API_KEY` | LLM chunker API key | falls back to `GBRAIN_OPENAI_API_KEY` |
| `GBRAIN_CHUNKER_BASE_URL` | LLM chunker base URL | falls back to `GBRAIN_OPENAI_BASE_URL` |
| `GBRAIN_CHUNKER_MODEL` | LLM chunker model | `gpt-4o-mini` |
| `GBRAIN_TRANSCRIPTION_PROVIDER` | Transcription provider | `groq` |
| `GBRAIN_TRANSCRIPTION_GROQ_API_KEY` | Groq API key for transcription | — |
| `GBRAIN_TRANSCRIPTION_GROQ_BASE_URL` | Groq base URL for transcription | — |
| `GBRAIN_TRANSCRIPTION_OPENAI_API_KEY` | OpenAI API key for transcription | — |
| `GBRAIN_TRANSCRIPTION_OPENAI_BASE_URL` | OpenAI base URL for transcription | — |
| `GBRAIN_LOG_LEVEL` | Log level | `info` |
| `GBRAIN_LOG_TO_FILE` | Enable file logging | `true` |
| `GBRAIN_LOG_FILE_PATH` | Log file path | `$GBRAIN_DIR/logs/gbrain.log` |
| `GBRAIN_LOG_TO_CONSOLE` | Enable console logging | `true` |
| `GBRAIN_AUTO_LINK` | Auto-extract links on put_page | `true` |
| `GBRAIN_AUTO_TIMELINE` | Auto-extract timeline on put_page | `true` |
| `GBRAIN_POST_WRITE_LINT` | Run lint after write | `false` |
| `GBRAIN_SEARCH_DEBUG` | Enable search debug logging | `false` |

## Key Conventions

- Engine is **sync**; async only at CLI/MCP boundary via `spawn_blocking`
- Error types: `GBrainError` for engine-level, `OperationError` for business logic
- `Result<T>` = `std::result::Result<T, GBrainError>`
- Slug format: `prefix/name` (prefix must be in allowlist) or just `name`
- All database mutations go through `SqliteEngine`; `Operations` adds orchestration
- MCP tools map 1:1 to `Operations` methods with `remote: true`
- `serde` for serialization; `serde_yaml` for frontmatter; `serde_json` throughout
- Schema version tracked in `schema_version` table; migrations use `INSERT OR IGNORE`
- Batch operations available: `batch_put_pages`, `batch_add_links`
- Deterministic-first, LLM-fallback: scaffold builders, lint rules, and fail_improve all prefer deterministic paths

## Database Tables

pages, chunks, vec_chunks (sqlite-vec virtual), pages_fts (FTS5, auto-synced via triggers), links, tags, timeline, raw_data, page_versions, config, ingest_log, files, schema_version (migration tracking), jobs (persistent job queue)

Schema version: 8 (migrations V1-V8; V8 adds title + page_type columns to page_versions; V7 adds FTS5 rebuild + timeline UNIQUE constraint)

## Dependencies

- `rusqlite` (bundled) — SQLite with FTS5 + sqlite-vec
- `clap` (derive) — CLI argument parsing
- `reqwest` (rustls-tls, multipart) — HTTP client for embeddings + transcription
- `serde` + `serde_json` + `serde_yaml` — serialization
- `thiserror` v2 — error derivation
- `tokio` — async runtime (CLI/MCP only)
- `sha2` + `infer` — hashing and MIME detection
- `chrono` — timestamps
- `regex` — text processing
- `unicode-normalization` — text normalization
- `tracing` + `tracing-subscriber` + `tracing-appender` — structured logging with file rotation
- `dirs` — home directory resolution
- Optional: `axum` + `tower-http` (file-server feature)

## CLI Commands

- `gbrain put` — Create/update a page
- `gbrain get` — Retrieve a page
- `gbrain delete` — Delete a page
- `gbrain list` — List pages with filters
- `gbrain search` — Hybrid search
- `gbrain lint` — Zero-LLM quality check (6 rules)
- `gbrain extract` — Extract links/timeline/all from pages
- `gbrain install` — Install shell completions
- `gbrain stats` — Brain statistics
- `gbrain health` — Health check
