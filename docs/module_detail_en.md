# gbrain TS vs Rust — Module-Level Detailed Comparison

English | [中文版](./module_detail.md)

**Date**: 2026-05-04

---

## 1. Engine Layer

### TS: BrainEngine Interface (engine.ts, 1,721 lines)

```typescript
// Dual-engine implementation:
// - PGLiteEngine: Embedded WASM Postgres (browser/local)
// - PostgresEngine: Remote Postgres (production)

// Key unique methods:
searchKeywordChunks(query, opts): Promise<CodeChunkResult[]>
countStaleChunks(): Promise<number>
listStaleChunks(limit): Promise<StaleChunkRow[]>
addCodeEdges(edges): Promise<number>
deleteCodeEdgesForChunks(chunkIds): Promise<number>
getCallersOf(slug, symbol): Promise<CodeEdgeResult[]>
getCalleesOf(slug, symbol): Promise<CodeEdgeResult[]>
getEdgesByChunk(chunkId): Promise<CodeEdgeResult[]>
withReservedConnection<T>(fn): Promise<T>  // advisory lock
```

### Rust: BrainEngine trait (engine.rs, 59 methods)

```rust
// Single-engine implementation: SqliteEngine (SQLite + FTS5 + sqlite-vec)
// NOT dyn-compatible — use concrete type

// Key unique methods:
detect_dead_links(slug): Result<Vec<String>>
file_url_by_storage_path(storage_path): Result<Option<String>>
file_verify(file_id): Result<bool>
add_timeline_multi_batch(entries): Result<usize>
restore_page(slug): Result<bool>
purge_deleted_pages(older_than_hours): Result<Vec<String>>
count_stale_chunks(): Result<usize>
list_stale_chunks(limit): Result<Vec<StaleChunk>>
```

### Difference Analysis

| Aspect | TS | Rust |
|--------|----|----|
| Engine count | 2 (PGLite + Postgres) | 1 (SQLite) |
| dyn-compatible | Yes (interface) | No (trait) |
| Connection management | Connection pool + advisory lock | Single connection |
| Code edges | Yes | No |
| Stale chunks | Yes | Yes |
| Multi-source | Yes (source_id) | No |

---

## 2. Operations Layer

### TS: Operations (operations.ts, 2,501 lines)

```typescript
// Key unique methods:
extractAndEnrich(slug, content): Promise<EnrichResult>  // batch + throttling
reindexCodePage(slug): Promise<void>  // code page re-indexing
reconcileLinks(slug): Promise<void>  // batch recalculate links
checkBacklinks(slug): Promise<BacklinkCheckResult>  // find missing backlinks
publishPage(slug, opts): Promise<PublishResult>  // HTML export + encryption
```

### Rust: Operations (operations.rs, ~1,200 lines)

```rust
// Core methods complete: put_page, get_page, delete_page, search, query embedding, file_upload, etc.
// Missing: extractAndEnrich, reindexCodePage, reconcileLinks, checkBacklinks, publishPage
```

---

## 3. Search Module

### TS: search/ (7 files, ~2,300 lines)

```
search/
├── hybrid.ts          (721 lines) RRF fusion + source-boost + hard-exclude
├── sql-ranking.ts     (415 lines) Postgres SQL ranking functions
├── source-boost.ts    (349 lines) Slug prefix weighting
├── two-pass.ts        (325 lines) Cathedral II two-pass retrieval
├── intent.ts          (276 lines) Query intent classification
├── expansion.ts       (236 lines) Query expansion
└── dedup.ts           (215 lines) 4-layer dedup
```

### Rust: search/ (8 files, ~2,200 lines)

```
search/
├── hybrid.rs          (689 lines) RRF fusion + time decay + cosine rescoring
├── keyword.rs         (68 lines)  FTS5 BM25 keyword search
├── vector.rs          (242 lines) sqlite-vec cosine similarity
├── intent.rs          (155 lines) Query intent classification
├── expansion.rs       (236 lines) Query expansion + injection defense
├── dedup.rs           (164 lines) 4-layer dedup
├── fuzzy.rs           (317 lines) Character trigram Jaccard fuzzy search
└── eval.rs            (313 lines) Search quality evaluation framework
```

### Key Differences

| Aspect | TS | Rust |
|--------|----|----|
| SQL ranking | sql-ranking.ts (415 lines, Postgres-specific) | keyword.rs (68 lines, FTS5-specific) |
| Source boost | Yes (349 lines) | Yes (slug-prefix weighting) |
| Hard exclude | Yes (SQL NOT LIKE) | Yes (include/exclude slug-prefix filters) |
| Two-pass retrieval | Yes (325 lines) | No |
| Fuzzy search | pg_trgm (Postgres extension) | Custom implementation (317 lines) |
| Evaluation framework | No | Yes (313 lines, P@k/R@k/MRR/nDCG@k) |
| Time decay | No | Yes (in hybrid.rs) |
| Injection defense | N/A (parameterized queries) | Yes (escape_fts_term) |

---

## 4. Chunker Module

### TS: chunkers/ (5 files, ~1,890 lines)

```
chunkers/
├── recursive.ts       (211 lines) Recursive text chunking
├── semantic.ts        (340 lines) Semantic chunking
├── llm.ts             (163 lines) LLM-guided chunking
├── code.ts            (1,050 lines) tree-sitter code chunking
└── edge-extractor.ts  (178 lines) Code edge extraction
```

### Rust: chunker/ (3 files, ~1,361 lines + lightweight fenced-code extraction in operations.rs)

```
chunker/
├── mod.rs             (35 lines)  Module entry
├── recursive.rs       (366 lines) Recursive text chunking
├── semantic.rs        (719 lines) Semantic chunking
└── llm.rs             (276 lines) LLM-guided chunking
```

### Key Differences

| Aspect | TS | Rust |
|--------|----|----|
| Code chunking | Yes (tree-sitter, 1,050 lines) | Partial: Markdown fenced-code chunks |
| Edge extraction | Yes (178 lines) | No |
| Qualified names | Yes (109 lines) | No |
| Semantic chunking | 340 lines | 719 lines (more detailed) |
| Recursive chunking | 211 lines | 366 lines (more detailed) |

---

## 5. Enrichment Module

### TS: enrichment/ (3 files, ~1,870 lines)

```
enrichment/
├── enrichment.ts      (1,059 lines) Enrichment pipeline
├── budget.ts          (478 lines)   API budget management
└── completeness.ts    (333 lines)   Completeness scoring
```

### Rust: enrichment.rs (472 lines, single file)

```rust
// Contains: Tier classification, entity detection, auto-enrichment, tag/link suggestions
// Separated into: completeness.rs (313 lines), budget.rs (422 lines)
```

### Key Differences

| Aspect | TS | Rust |
|--------|----|----|
| extractAndEnrich | Yes (batch + throttling) | No |
| Budget implementation | SELECT FOR UPDATE + TTL | TxGuard RAII + reserve/commit |
| Completeness scoring | 333 lines | 313 lines (essentially same) |
| Tier classification | Yes | Yes (Tier2 reachability fixed) |

---

## 6. Output/Validator Module

### TS: output/validators/ (4 files, ~590 lines)

```
output/validators/
├── back-link.ts       (150 lines) Back-link validation
├── citation.ts        (180 lines) Citation format validation
├── link.ts            (150 lines) Link validation
└── triple-hr.ts       (110 lines) Triple-hr validation
```

### Rust: validators.rs (429 lines, single file)

```rust
// Contains: back-link validation, link validation, source citation validation, link symmetry validation
// Missing: citation.ts, triple-hr.ts
```

---

## 7. Storage Module

### TS: storage/ (3 files, ~1,560 lines)

```
storage/
├── file-storage.ts    (620 lines) File storage abstraction
├── s3-storage.ts      (530 lines) S3-compatible storage
└── supabase-storage.ts (410 lines) Supabase storage + TUS upload
```

### Rust: file_storage.rs (331 lines, single file)

```rust
// Local filesystem storage only
// Missing: S3, Supabase
// Unique: axum file server (optional feature)
```

---

## 8. Minions Module

### TS: minions/ (11 files, ~4,478 lines)

```
minions/
├── queue.ts           (1,281 lines) Job queue
├── worker.ts          (513 lines)   Worker process pool
├── supervisor.ts      (630 lines)   Process management + auto-restart
├── subagent.ts        (710 lines)   LLM-in-loop tool calling
├── shell-handler.ts   (321 lines)   Shell command execution
├── subagent-aggregator.ts (169 lines) Subagent aggregation
├── plugin-loader.ts   (235 lines)   Plugin loading
├── rate-leases.ts     (152 lines)   Rate leases
├── quiet-hours.ts     (94 lines)    Quiet hours
├── transcript.ts      (229 lines)   Execution transcript
└── audit/             (3 files)     Audit handlers
```

### Rust: jobs.rs (422 lines, single file)

```rust
// Basic job queue: submit, get, list, cancel, retry, complete, fail
// Missing: Worker, Supervisor, Subagent, Shell, Plugin, RateLease, QuietHours, Transcript, Audit
```

---

## 9. MCP Module

### TS: mcp/ (4 files, ~686 lines)

```
mcp/
├── server.ts          (289 lines) HTTP + stdio dual transport
├── dispatch.ts        (215 lines) Tool dispatch
├── tool-defs.ts       (132 lines) Tool definitions
└── rate-limit.ts      (50 lines)  Rate limiting
```

### Rust: mcp/ (2 files, ~1,133 lines)

```
mcp/
├── mod.rs             (816 lines) stdio JSON-RPC server
└── tool_defs.rs       (317 lines) Tool definitions
```

### Key Differences

| Aspect | TS | Rust |
|--------|----|----|
| Transport | HTTP + stdio | stdio only |
| Authentication | Bearer token | None |
| Rate limiting | IP + Token dual-layer | None |
| CORS | Yes | None |
| Audit logging | Yes | None |
| Structured errors | OperationError(code/message/suggestion) | GBrainError enum |
| Parameter validation | Type checking | Basic validation |
| Tool count | 37 | 32 |

---

## 10. Resolver Module

### TS: resolver/ (4 files, ~1,095 lines)

```
resolver/
├── interface.ts       (158 lines) Resolver interface definition
├── registry.ts        (151 lines) Resolver registry
├── url-reachable.ts   (332 lines) URL reachability detection
└── handle-to-tweet.ts (428 lines) X/Twitter tweet parsing
```

### Rust: Completely missing

The Resolver module has no corresponding implementation in Rust. This includes:
- Dead link detection
- X/Twitter API integration
- Extensible resolver framework

---

## 11. Skillify/Skillpack Module

### TS: skillify/ (4 files, ~1,013 lines)

```
skillify/
├── skillify.ts        (310 lines) Skill scaffolding/generation
├── skillify-check.ts  (360 lines) Skill validation
├── skillpack.ts       (440 lines) Skill pack management
└── skillpack-check.ts (228 lines) Skill pack health check
```

### Rust: Completely missing

The Skill system has no corresponding implementation in Rust.

---

## 12. Scaffold Module

### TS: scaffold.ts (236 lines)

```typescript
// Deterministic citation builders:
// - tweetCitation(url): Tweet citation
// - emailCitation(subject, from, to, date): Email citation
// - meetingCitation(title, date, attendees): Meeting citation
```

### Rust: scaffold.rs (154 lines)

```rust
// Essentially same implementation
// tweet_citation, email_citation, meeting_citation
// Fixed: markdown injection in email subject
```

---

## 13. Sync Module

### TS: sync.ts (~500 lines)

```typescript
// Git sync + manifest tracking
// Supports multiple Git URL formats
```

### Rust: sync.rs (839 lines)

```rust
// More detailed implementation
// Includes: validate_git_url (SSH injection protection), acknowledge_failures (atomic writes)
// Fixed: colon-less git@ URL bypass
```

---

## 14. Configuration Module

### TS: config.ts (~300 lines)

```typescript
interface BrainConfig {
  // Database
  databaseUrl?: string;
  pgliteDataDir?: string;

  // OpenAI
  openaiApiKey?: string;
  openaiBaseUrl?: string;
  embeddingModel?: string;
  embeddingDimensions?: number;

  // Search
  searchBoosts?: Record<string, number>;  // source-boost config
  hardExclude?: string[];                  // exclude prefixes

  // Storage
  storageBackend?: 'local' | 's3' | 'supabase';
  s3Config?: S3Config;
  supabaseConfig?: SupabaseConfig;

  // MCP
  mcpPort?: number;
  mcpAuthToken?: string;

  // Minions
  minionConcurrency?: number;
  minionQuietHours?: { start: string; end: string };

  // Other
  autoLink?: boolean;
  autoTimeline?: boolean;
  postWriteLint?: boolean;
}
```

### Rust: config.rs (~200 lines)

```rust
pub struct Config {
    // Database
    pub db_path: String,
    pub gbrain_dir: String,

    // OpenAI
    pub openai_api_key: Option<String>,
    pub openai_base_url: Option<String>,
    pub embedding_model: String,
    pub embedding_dimensions: usize,

    // Chunking
    pub chunk_size: usize,
    pub chunk_overlap: usize,

    // Transcription
    pub transcription_provider: String,
    pub transcription_groq_api_key: Option<String>,
    pub transcription_openai_api_key: Option<String>,

    // Other
    pub auto_link: bool,
    pub auto_timeline: bool,
    pub post_write_lint: bool,
    pub search_debug: bool,

    // Logging
    pub log_level: String,
    pub log_to_file: bool,
    pub log_file_path: String,
    pub log_to_console: bool,
}
```

### Differences

| Config field | TS | Rust |
|-------------|----|----|
| databaseUrl | Yes | No (uses db_path) |
| storageBackend | Yes (local/s3/supabase) | No (local only) |
| searchBoosts | Yes | Partial: built-in slug-prefix boosts |
| hardExclude | Yes | No |
| mcpPort | Yes | No |
| mcpAuthToken | Yes | No |
| minionConcurrency | Yes | No |
| minionQuietHours | Yes | No |
| chunk_size/overlap | No | Yes |
| transcription | No | Yes |
| log config | No (uses Bun built-in) | Yes |
| search_debug | No | Yes |

---

## 15. Error Handling Comparison

### TS: OperationError

```typescript
class OperationError extends Error {
  code: string;        // 'NOT_FOUND', 'INVALID_INPUT', 'SECURITY', etc.
  message: string;
  suggestion?: string;  // Fix suggestion
  docs?: string;        // Documentation link
}
```

### Rust: GBrainError

```rust
enum GBrainError {
    NotFound(String),
    InvalidInput(String),
    Security(String),
    Database(String),
    Io(std::io::Error),
    // ... 12 variants
}
```

### Differences

| Aspect | TS | Rust |
|--------|----|----|
| Structured error codes | Yes (code field) | No (enum variants) |
| Fix suggestions | Yes (suggestion field) | No |
| Documentation links | Yes (docs field) | No |
| Error chaining | Yes (cause) | Yes (thiserror source) |
| MCP error format | Yes (JSON structured) | No (string) |

---

## 16. Testing Comparison

### TS Testing

- Uses Bun built-in test runner
- Tests inline in source files or separate test files
- Integration tests require PGLite instance
- Search quality evaluation (eval command)

### Rust Testing

- 222 lib unit tests (#[cfg(test)] mod tests)
- 4 dedup integration tests
- 17 engine integration tests
- 16 fuzzy integration tests
- 3 search integration tests
- 262 total tests, all passing
- Uses :memory: SQLite, zero configuration

### Differences

| Aspect | TS | Rust |
|--------|----|----|
| Test framework | Bun test | cargo test |
| Test count | Unknown (inline) | 262 |
| Integration tests | PGLite instance | :memory: SQLite |
| Evaluation framework | Yes (eval command) | Yes (eval.rs) |
| Coverage | Unknown | Not measured |
