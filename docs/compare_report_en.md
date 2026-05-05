# gbrain TypeScript vs gbrain-rs Rust Comparison Report

English | [中文版](./compare_report.md)

**Date**: 2026-05-04
**TS Version**: gbrain v0.22.8 (Bun runtime)
**Rust Version**: gbrain-rs (native, SQLite + sqlite-vec + FTS5)

---

## 1. Code Size Comparison

| Metric | TypeScript (gbrain) | Rust (gbrain-rs) | Ratio |
|--------|---------------------|-------------------|-------|
| Core source lines | ~30,890 (src/core/) | ~19,922 (src/) | 1.55x |
| CLI command lines | ~16,690 (src/commands/) | ~1,050 (src/bin/gbrain.rs) | 15.9x |
| MCP server lines | ~686 (src/mcp/) | ~1,133 (src/mcp/) | 0.61x |
| Test code lines | (inline tests) | ~783 (tests/) | — |
| **Total source** | **~50,500** | **~20,705** | **2.44x** |
| Source file count | ~97 (core/) + 49 (commands/) | ~44 | 3.3x |

The Rust codebase is approximately 41% the size of the TS version. Main reasons:
- Rust lacks 49 CLI command files (TS has a complete command system)
- Rust lacks the Minions subsystem (4,478 lines)
- Rust lacks the Resolver subsystem (1,095 lines)
- Rust lacks the Skillify/Skillpack system (1,013 lines)
- Rust now has tree-sitter multi-language code chunk indexing, qualified symbols, code_edges/unresolved symbol edges, definition/reference lookup, and Cathedral II two-pass code graph retrieval.

---

## 2. Database Engine Comparison

| Feature | TypeScript | Rust |
|---------|-----------|------|
| **Primary database** | PostgreSQL / PGLite (WASM) | SQLite |
| **Full-text search** | tsvector + ts_rank | FTS5 + BM25 |
| **Vector search** | pgvector (cosine distance) | sqlite-vec (cosine similarity) |
| **Fuzzy search** | pg_trgm (trigram) | Custom character trigram Jaccard |
| **Connection pooling** | postgres.js built-in | Single connection (SQLite doesn't need pooling) |
| **Transaction support** | Postgres transactions + advisory lock | BEGIN IMMEDIATE + COMMIT/ROLLBACK |
| **Schema version** | V1-V29 | V1-V11 |
| **Multi-source support** | source_id composite key (v0.18+) | No |
| **PGLite** | Embedded WASM Postgres | No (uses SQLite directly) |
| **PgBouncer** | Auto-detect compatibility | Not applicable |

**Key differences**:
- TS uses the Postgres ecosystem (pgvector, pg_trgm, tsvector), Rust uses the SQLite ecosystem (sqlite-vec, FTS5)
- TS supports multi-source brains (source_id composite key), Rust does not
- TS schema migrations reach V29, Rust is currently V11
- TS has dual engines (PGLite for embedded, remote Postgres for production), Rust only has SQLite

---

## 3. BrainEngine Interface Comparison

### Method Count

| Category | TS Methods | Rust Methods | Difference |
|----------|-----------|-------------|------------|
| Lifecycle | 4 | 4 | Same |
| Page CRUD | 6 | 8 | Rust adds soft-delete restore/purge |
| Search | 4 | 4 | Same at interface level; Rust uses deterministic code chunk search |
| Chunking | 6 | 7 | Rust now has countStaleChunks/listStaleChunks equivalents |
| Links | 9 | 9 | Same |
| Tags | 3 | 3 | Same |
| Timeline | 3 | 4 | Rust has add_timeline_multi_batch |
| Raw data | 2 | 2 | Same |
| Versions | 3 | 3 | Same |
| Stats/Health | 2 | 2 | Same |
| Orphans/Dead links | 1 | 2 | Rust has detect_dead_links |
| Config | 2 | 2 | Same |
| Files | 4 | 5 | Rust has file_url_by_storage_path, file_verify |
| Code edges | 5 | 4 | Rust has add/delete/query edge APIs; TS still has broader code graph workflows |
| Other | 5 | 5 | Roughly same |
| **Total** | **~59** | **~65** | Rust added soft-delete, stale chunk, code chunk search, and code edge APIs |

### TS-Only Methods

| Method | Description |
|--------|-------------|
| `findOrphanPages()` | Find orphan pages (Rust has detect_orphans) |
| `addCodeEdges()` | Add code call edges (v0.20 Cathedral II) |
| `deleteCodeEdgesForChunks()` | Delete code edges |
| `getCallersOf()` | Query who calls a symbol |
| `getCalleesOf()` | Query what a symbol calls |
| `getEdgesByChunk()` | Query code edges for a chunk |
| `withReservedConnection()` | Dedicated connection (advisory lock) |

---

## 4. Type System Comparison

### PageType Variants

| PageType | TS | Rust | Status |
|----------|----|----|--------|
| person | Yes | Yes | Same |
| company | Yes | Yes | Same |
| deal | Yes | Yes | Same |
| yc | Yes | Yes | Same |
| civic | Yes | Yes | Same |
| project | Yes | Yes | Same |
| concept | Yes | Yes | Same |
| source | Yes | Yes | Same |
| media | Yes | Yes | Same |
| writing | Yes | Yes | Same |
| analysis | Yes | Yes | Same |
| guide | Yes | Yes | Same |
| hardware | Yes | Yes | Same |
| architecture | Yes | Yes | Same |
| meeting | Yes | Yes | Same |
| note | Yes | Yes | Same |
| **email** | Yes | Yes | Same |
| **slack** | Yes | Yes | Same |
| **calendar-event** | Yes | Yes | Same |
| **code** | Yes | Yes | Same |

Rust now includes the workflow PageType variants added in TS v0.18+: email, slack, calendar-event, and code.

### Chunk Differences

| Feature | TS | Rust |
|---------|----|----|
| chunk_source | compiled_truth / timeline / **fenced_code** | compiled_truth / timeline / **fenced_code** |
| Code metadata | Yes (language, symbol_name, symbol_type, start_line, end_line, parent_symbol_path, doc_comment, symbol_name_qualified) | Partial: language, symbol_name, symbol_type, start_line, end_line |
| embedding field | Yes (Float32Array) | Yes on `ChunkInput`; stored in `chunk_embeddings` fallback table |
| model field | Yes | Yes |
| embedded_at | Yes | Yes |
| StaleChunkRow | Yes | Yes (`StaleChunk`) |

### SearchOpts Differences

| Feature | TS | Rust |
|---------|----|----|
| exclude_slug_prefixes | Yes | Yes |
| include_slug_prefixes | Yes | Yes |
| language | Yes (v0.20) | No |
| symbolKind | Yes (v0.20) | No |
| nearSymbol | Yes (v0.20) | No |
| walkDepth | Yes (v0.20) | No |
| sourceId | Yes (v0.18) | No |
| expanded_queries | No | Yes |
| expanded_embeddings | No | Yes |
| dedup_opts | No | Yes |

### Other Type Differences

| Type | TS | Rust | Difference |
|------|----|----|------------|
| PageKind | Yes (markdown/code) | No | Rust has no PageKind |
| CodeEdgeInput | Yes | Yes | Rust has basic intra-page calls/references |
| CodeEdgeResult | Yes | Yes (`CodeEdge`) | Rust has basic callers/callees |
| Link.direction | No | Yes | Rust has LinkDirection |
| LinkBatchInput.direction | No | Yes | Rust has direction field |
| PutPageResult | Yes | Yes | Same |
| ListPageEntry | Yes | Yes | Same |

---

## 5. Search Pipeline Comparison

### Pipeline Steps

| Step | TS | Rust | Difference |
|------|----|----|------------|
| 1. Keyword search | tsvector + ts_rank + **source-boost** + **hard-exclude** | FTS5 BM25 weighted + source boost + hard exclude | Different SQL engine, similar controls |
| 2. Vector search | pgvector cosine distance | sqlite-vec cosine similarity + `chunk_embeddings` fallback | Different engines, same algorithm |
| 3. Fallback broadening | Yes | Yes | Same |
| 4. RRF fusion | Yes (k=60) | Yes (k=60) | Same |
| 5. compiled_truth boost | Yes (2.0x) | Yes | Weights may differ |
| 6. Cosine rescoring | Yes (0.7*rrf + 0.3*cosine) | Yes | Same |
| 7. Backlink boost | Yes (1 + 0.05*ln(1+count)) | Yes | Same |
| 8. Time decay | No (not explicitly in hybrid.ts) | Yes | **Rust only** |
| 9. Intent-type boost | Yes | Yes | Same |
| 10. Two-pass retrieval | Yes (v0.20 Cathedral II) | No | **TS only** |
| 11. 4-layer dedup | Yes | Yes | Same |
| 12. source-boost | Yes (weighted by slug prefix) | No | **TS only** |
| 13. hard-exclude | Yes (excluded by slug prefix) | No | **TS only** |

### TS-Only Search Features

| Feature | File | Description |
|---------|------|-------------|
| source-boost | search/source-boost.ts | Implemented in Rust as slug-prefix result weighting |
| hard-exclude | search/sql-ranking.ts | Implemented in Rust via include/exclude slug-prefix filters |
| two-pass retrieval | search/two-pass.ts | Not yet implemented in Rust; code_edges are available for callers/callees |
| searchKeywordChunks | engine.ts | Code chunk search |

### Rust-Only Search Features

| Feature | File | Description |
|---------|------|-------------|
| Time decay boost | search/hybrid.rs | 1/(1 + days/half_life) time decay |
| expanded_queries/embeddings | types.rs SearchOpts | Pre-computed expanded queries and embeddings |
| dedup_opts | types.rs SearchOpts | Customizable dedup options |

---

## 6. MCP Tools Comparison

### Tool List

| Tool | TS | Rust | Difference |
|------|----|----|------------|
| get_page | Yes | Yes | Same |
| put_page | Yes | Yes | Same |
| delete_page | Yes | Yes | Rust is soft-delete by default |
| list_pages | Yes | Yes | Same |
| search | Yes | Yes | Same |
| query | Yes | Yes | Same |
| add_tag | Yes | Yes | Same |
| remove_tag | Yes | Yes | Same |
| get_tags | Yes | Yes | Same |
| add_link | Yes | Yes | Same |
| remove_link | Yes | Yes | Same |
| get_links | Yes | Yes | Same |
| get_backlinks | Yes | Yes | Same |
| traverse_graph | Yes | Yes | Same |
| add_timeline_entry | Yes | Yes | Same |
| get_timeline | Yes | Yes | Same |
| get_stats | Yes | Yes | Same |
| get_health | Yes | Yes | Same |
| get_versions | Yes | Yes | Same |
| revert_version | Yes | Yes | Same |
| sync_brain | Yes | Yes | Same |
| put_raw_data | Yes | Yes | Same |
| get_raw_data | Yes | Yes | Same |
| resolve_slugs | Yes | Yes | Same |
| get_chunks | Yes | Yes | Same |
| count_stale_chunks | Yes | Yes | Same |
| list_stale_chunks | Yes | Yes | Same |
| log_ingest | Yes | Yes | Same |
| get_ingest_log | Yes | Yes | Same |
| file_list | Yes | Yes | Same |
| file_upload | Yes | Yes | Same |
| file_url | Yes | Yes | Same |
| submit_job | Yes | Yes | Same |
| get_job | Yes | Yes | Same |
| list_jobs | Yes | Yes | Same |
| cancel_job | Yes | Yes | Same |
| retry_job | Yes | Yes | Same |
| get_job_progress | Yes | Yes | Same |
| pause_job | Yes | No | **Missing in Rust** |
| resume_job | Yes | No | **Missing in Rust** |
| replay_job | Yes | No | **Missing in Rust** |
| send_job_message | Yes | No | **Missing in Rust** |
| find_orphans | Yes | Yes | Same |

### TS-Only MCP Features

| Feature | Description |
|---------|-------------|
| HTTP transport | Bearer token auth + IP/Token dual-layer rate limiting + CORS + request body size limit |
| Rate limiting | Token bucket algorithm, LRU bounded map, anti-bucket-reset attack protection |
| Structured errors | OperationError with code/message/suggestion/docs fields |
| Parameter validation | Type checking (string/number/boolean/object/array) |

Rust MCP only supports stdio transport, with no HTTP transport, no rate limiting, and no structured error codes.

---

## 7. Chunkers Comparison

| Chunker | TS | Rust | Difference |
|---------|----|----|------------|
| Recursive | Yes (211 lines) | Yes (366 lines) | Rust implementation more detailed |
| Semantic | Yes (340 lines) | Yes (719 lines) | Rust implementation more detailed |
| LLM-guided | Yes (163 lines) | Yes (276 lines) | Same |
| **Code parser** | Yes (tree-sitter, 1,050 lines) | Yes (deterministic parser for Rust/TS/JS/Python/Go/Java/C-style declarations) | Different implementation |
| **Edge extractor** | Yes (178 lines) | Yes (intra-page calls/references) | Rust is simpler |
| **Qualified names** | Yes (109 lines) | Yes (container.method style) | Rust is simpler |

Rust now has deterministic symbol chunk extraction, qualified names, `chunks_fts`, and `code_edges`; it still lacks tree-sitter-grade AST precision and Cathedral II two-pass retrieval.

---

## 8. Enrichment Pipeline Comparison

| Feature | TS | Rust | Difference |
|---------|----|----|------------|
| Entity detection | Yes (regex + company suffixes) | Yes (regex + company suffixes) | Same |
| Tier classification | Yes (Tier1/2/3) | Yes (Tier1/2/3) | Same |
| Completeness scoring | Yes (7 rubrics, by entity type) | Yes (7 rubrics, by entity type) | Same |
| Budget management | Yes (SELECT FOR UPDATE, TTL, midnight rollover) | Yes (TxGuard RAII, reserve/commit/rollback) | Different implementation, same semantics |
| Auto-enrichment | Yes (stub creation + backlinks) | Yes (stub creation + backlinks) | Same |
| extractAndEnrich | Yes (batch + throttling) | No | **Missing in Rust** |

---

## 9. Validators Comparison

| Validator | TS | Rust | Difference |
|-----------|----|----|------------|
| Back-link validation | Yes (back-link.ts) | Yes (validators.rs) | Same |
| Citation validation | Yes (citation.ts, 180 lines) | No | **Missing in Rust** |
| Link validation | Yes (link.ts, 150 lines) | Yes (validators.rs) | Same |
| Triple-hr validation | Yes (triple-hr.ts) | No | **Missing in Rust** |
| BrainWriter | Yes (writer.ts, 330 lines) | Yes (writer.rs, 429 lines) | Same |
| Scaffold | Yes (scaffold.ts, 236 lines) | Yes (scaffold.rs, 154 lines) | Same |
| SlugRegistry | Yes (slug-registry.ts) | Yes (resolver.rs) | Same |
| Post-write lint | Yes (post-write.ts) | Yes (config flag) | Same |

---

## 10. Storage Backend Comparison

| Backend | TS | Rust | Difference |
|---------|----|----|------------|
| Local filesystem | Yes | Yes | Same |
| **S3-compatible** | Yes (AWS S3, R2, MinIO) | No | **Missing in Rust** |
| **Supabase Storage** | Yes (with TUS resumable upload) | No | **Missing in Rust** |
| File server | No | Yes (axum, optional feature) | **Rust only** |

---

## 11. Minions Subsystem Comparison

| Feature | TS | Rust | Difference |
|---------|----|----|------------|
| **Job queue** | Yes (MinionQueue, 1,281 lines) | Yes (jobs.rs, 422 lines) | TS more complex |
| **Worker** | Yes (MinionWorker, 513 lines) | No | **Missing in Rust** |
| **Supervisor** | Yes (process management, 630 lines) | No | **Missing in Rust** |
| **Subagent** | Yes (LLM-in-loop, 710 lines) | No | **Missing in Rust** |
| **Shell handler** | Yes (321 lines) | No | **Missing in Rust** |
| **Subagent aggregator** | Yes (169 lines) | No | **Missing in Rust** |
| **Plugin loader** | Yes (235 lines) | No | **Missing in Rust** |
| **Rate leases** | Yes (152 lines) | No | **Missing in Rust** |
| **Quiet hours** | Yes (94 lines) | No | **Missing in Rust** |
| **Transcript** | Yes (229 lines) | No | **Missing in Rust** |
| **Audit handlers** | Yes (3 files) | No | **Missing in Rust** |

Rust has a basic job queue (submit/get/list/cancel/retry), but lacks the entire Minions runtime: Worker process pool, Supervisor process management, Subagent LLM loop, Shell execution, Plugin system, etc.

---

## 12. Resolver Subsystem Comparison

| Feature | TS | Rust | Difference |
|---------|----|----|------------|
| **Resolver interface** | Yes (interface.ts, 158 lines) | No | **Missing in Rust** |
| **Resolver registry** | Yes (registry.ts, 151 lines) | No | **Missing in Rust** |
| **URL reachability detection** | Yes (url-reachable.ts, 332 lines) | No | **Missing in Rust** |
| **X API tweet parsing** | Yes (handle-to-tweet.ts, 428 lines) | No | **Missing in Rust** |

The entire Resolver subsystem is missing in Rust. This includes dead link detection, X/Twitter API integration, etc.

---

## 13. CLI Commands Comparison

### Commands Implemented in Rust

| Command | TS | Rust |
|---------|----|----|
| get | Yes | Yes |
| put | Yes | Yes |
| delete | Yes | Yes |
| restore | Partial/varies | Yes |
| purge-deleted | Partial/varies | Yes |
| list | Yes | Yes |
| search | Yes | Yes |
| query/ask | Yes | Yes |
| tag/untag | Yes | Yes |
| link/unlink | Yes | Yes |
| backlinks | Yes | Yes |
| timeline | Yes | Yes |
| stats | Yes | Yes |
| health | Yes | Yes |
| history | Yes | Yes |
| revert | Yes | Yes |
| lint | Yes | Yes |
| extract | Yes | Yes |
| embed | Yes | Yes |
| sync | Yes | Yes |
| serve | Yes | Yes |
| import | Yes | Yes |
| files | Yes | Yes |
| jobs | Yes | Yes |
| autopilot | Yes | Yes |

### Commands in TS but Missing in Rust

| Command | Lines | Description |
|---------|-------|-------------|
| init | 382 | Create brain (PGLite/Supabase/URL) |
| doctor | 1,050 | Health check (resolver, skills, pgvector, RLS, embeddings) |
| upgrade | 259 | Self-update |
| check-update | 179 | Check for new version |
| integrations | 1,005 | Manage integration recipes |
| auth | 262 | HTTP MCP token management |
| apply-migrations | 420 | Schema migration orchestration |
| config | 50 | Show/get/set brain config |
| migrate | 305 | Cross-engine migration |
| features | 305 | Scan usage + recommend unused features |
| export | 56 | Export brain to markdown |
| agent | 333 | Subagent runtime |
| agent-logs | 185 | Agent log viewer |
| dream | 209 | One-shot nightly maintenance |
| code-def | 136 | Find symbol definition |
| code-refs | 133 | Find symbol references |
| code-callers | 74 | Find callers |
| code-callees | 80 | Find callees |
| reindex-code | 324 | Code page re-indexing |
| reconcile-links | 177 | Batch recalculate links |
| eval | 344 | Search quality evaluation |
| routing-eval | 209 | Routing quality evaluation |
| skillify | 310 | Skill scaffolding/generation |
| skillify-check | 360 | Skill validation |
| skillpack | 440 | Skill pack management |
| skillpack-check | 228 | Skill pack health check |
| publish | 378 | Shareable HTML export (with AES-256 encryption) |
| report | 82 | Save timestamped report |
| sources | 372 | Multi-source brain management |
| resolvers | 195 | Resolver configuration |
| frontmatter | 299 | Frontmatter operations |
| frontmatter-install-hook | 216 | Git hook installation |
| check-backlinks | 274 | Find/fix missing backlinks |
| orphans | 241 | Find orphan pages |
| integrity | 762 | Brain integrity check |
| check-resolvable | 315 | Verify skill tree reachability |
| repair-jsonb | 169 | Fix corrupted JSONB columns |
| graph-query | 118 | Graph query |

---

## 14. Security Boundary Comparison

| Feature | TS | Rust | Difference |
|---------|----|----|------------|
| remote flag | Yes (OpContext.remote) | Yes (OpContext.remote) | Same |
| Path traversal protection | Yes | Yes | Same |
| Symlink rejection | Yes | Yes | Same |
| Slug validation | Yes | Yes | Same |
| Filename validation | Yes | Yes | Same |
| **HTTP Bearer token** | Yes | No | **Missing in Rust** |
| **IP rate limiting** | Yes | No | **Missing in Rust** |
| **Token rate limiting** | Yes | No | **Missing in Rust** |
| **CORS** | Yes (deny by default) | No | **Missing in Rust** |
| **Request body size limit** | Yes (1MB) | Yes (1MB) | Same |
| **Audit logging** | Yes (mcp_request_log) | No | **Missing in Rust** |
| **SSH injection protection** | Yes | Yes | Same |
| **FTS5 injection protection** | N/A (uses tsvector) | Yes | Rust only |
| **Content size limit** | Yes | Yes | Same |

---

## 15. Dependencies Comparison

### TS Core Dependencies

| Dependency | Purpose | Rust Equivalent |
|------------|---------|-----------------|
| @electric-sql/pglite | Embedded Postgres | rusqlite (SQLite) |
| postgres | Postgres client | Not applicable |
| pgvector | Vector search | sqlite-vec |
| openai | OpenAI API | reqwest (manual implementation) |
| @anthropic-ai/sdk | Anthropic API | Not applicable |
| @aws-sdk/client-s3 | S3 storage | Not applicable |
| gray-matter | Frontmatter parsing | serde_yaml |
| marked | Markdown rendering | Not applicable |
| @dqbd/tiktoken | Token counting | Not applicable |
| web-tree-sitter | Code parsing | Not applicable |
| @modelcontextprotocol/sdk | MCP SDK | Manual JSON-RPC implementation |

### Rust Core Dependencies

| Dependency | Purpose | TS Equivalent |
|------------|---------|---------------|
| rusqlite (bundled) | SQLite + FTS5 + sqlite-vec | pglite/postgres |
| clap (derive) | CLI argument parsing | Manual arg parsing |
| reqwest (rustls-tls) | HTTP client | fetch/openai sdk |
| serde + serde_json + serde_yaml | Serialization | JSON.parse/YAML.parse |
| thiserror v2 | Error derivation | Custom Error classes |
| tokio | Async runtime | Bun (native async) |
| sha2 + infer | Hashing + MIME detection | crypto/mime |
| chrono | Timestamps | Date |
| regex | Regex | RegExp |
| unicode-normalization | Text normalization | Not applicable |
| tracing | Structured logging | console |
| Optional: axum + tower-http | File server | Not applicable |

---

## 16. Architecture Differences Summary

### Shared Three-Layer Architecture

Both versions follow the same three-layer design:

```
Engine Layer (BrainEngine interface/trait)
    |
Operations Layer (business logic)
    |
Interface Layer (CLI + MCP)
```

### Key Architecture Differences

| Aspect | TS | Rust |
|--------|----|----|
| **Runtime** | Bun (JS/TS) | Native compiled |
| **Async model** | Fully async/await | Engine sync, CLI/MCP boundary uses spawn_blocking |
| **Database** | Postgres dual-engine (PGLite + remote) | Single-engine SQLite |
| **Type system** | TypeScript interfaces | Rust trait (NOT dyn-compatible) |
| **Error handling** | OperationError class + structured error codes | GBrainError enum + thiserror |
| **Configuration** | config.json + env | env + config.json |
| **Transactions** | Postgres transactions + advisory lock | BEGIN IMMEDIATE + COMMIT/ROLLBACK |
| **Deployment** | Bun compiled single binary | cargo build single binary |

---

## 17. Feature Completeness Score

| Module | Completeness | Notes |
|--------|-------------|--------|
| Core engine | 88% | Missing code edges and multi-source; stale chunk APIs now implemented |
| Search pipeline | 80% | Missing source-boost, hard-exclude, two-pass |
| MCP tools | 90% | Missing pause/resume/replay/send_message |
| Chunkers/code index | 78% | Deterministic code chunks and code_edges implemented; tree-sitter precision and two-pass retrieval still missing |
| Enrichment pipeline | 90% | Missing batch extractAndEnrich |
| Validators | 70% | Missing citation, triple-hr validators |
| Storage backends | 40% | Missing S3, Supabase; has unique axum file server |
| Minions/Jobs | 30% | Basic queue exists, entire runtime missing |
| Resolver | 0% | Completely missing |
| CLI commands | 40% | Core commands present, 25+ advanced commands missing |
| Security | 75% | Missing HTTP auth, rate limiting, CORS, audit logging |
| **Overall** | **~65%** | Core functionality complete, advanced features largely missing |

---

## 18. Rust-Only Advantages

| Advantage | Description |
|-----------|-------------|
| Native performance | No GC pauses, no WASM overhead, compiler optimizations |
| Memory safety | Compile-time guarantees, no null/undefined runtime errors |
| Zero configuration | SQLite requires no Postgres server |
| Small binary | Single-file deployment, no Bun/node runtime dependency |
| Time decay search | Rust-only search result time decay boost |
| axum file server | Optional embedded HTTP file server |
| FTS5 BM25 weighting | More fine-grained full-text search weight control |
| Compile-time type checking | Stronger type safety guarantees |

---

## 19. TS-Only Advantages

| Advantage | Description |
|-----------|-------------|
| Multi-engine support | PGLite (local) + Postgres (remote) dual engines |
| Code indexing | tree-sitter code parsing, symbol definition/references/callers/callees |
| Cathedral II | Two-pass structural retrieval, code edge graph traversal |
| Subagent runtime | LLM-in-loop tool calling, token tracking |
| Resolver system | Dead link detection, X API integration, extensible resolver framework |
| Skill system | Skill generation, validation, packaging, installation |
| HTTP MCP | Bearer auth, dual-layer rate limiting, CORS, audit logging |
| S3/Supabase storage | Cloud storage backends, TUS resumable upload |
| Multi-source brain | source_id composite key, cross-source dedup |
| Publish/Export | HTML export, AES-256 encryption, skill packaging |
| Integration system | Integration recipe management |
| Process management | Supervisor process monitoring, auto-restart |

---

## 20. Recommended Rust Features to Prioritize

Sorted by impact:

| Priority | Feature | Estimated Effort | Rationale |
|----------|---------|-----------------|-----------|
| P0 | HTTP MCP transport + Bearer auth | 2-3 days | Security-critical, required for remote access |
| P0 | Rate limiting | 1 day | DoS/abuse prevention |
| P1 | Citation validator | 0.5 day | Data quality assurance |
| P1 | PageType extension (email/slack/calendar-event/code) | Done | Type completeness |
| P1 | source-boost + hard-exclude search | Done | Search quality improvement |
| P2 | Multi-source brain (source_id) | 2 days | Multi-repo scenarios |
| P2 | S3 storage backend | 1-2 days | Cloud deployment needs |
| P2 | Subagent runtime | 3-5 days | Automated workflows |
| P3 | Deterministic code chunker + code_edges | Done | Code indexing needs |
| P3 | Resolver system | 2-3 days | Dead link detection, external APIs |
| P3 | Skill system | 3-5 days | Skill ecosystem |
| P4 | Supervisor process management | 2 days | Production stability |
| P4 | Audit logging | 1 day | Observability |
