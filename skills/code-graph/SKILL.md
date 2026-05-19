---
name: code-graph
version: 1.1.0
description: |
  Code knowledge graph operations. Find symbol definitions, trace references,
  explore call graphs, and search code chunks. Code graph features (symbol
  definitions, references, call relationships) are built into the KB document
  processing pipeline and activated automatically during document upload.
  The unified artifact_query facade (mode=graph) for code graph queries is
  planned but not yet implemented.
triggers:
  - "find definition"
  - "code definition"
  - "who calls"
  - "callers of"
  - "callees of"
  - "code references"
  - "search code"
  - "code graph"
  - "reindex code"
tools:
  - artifact_query  # 统一查询接口（当前 code graph 查询仅此入口可用）
mutating: true
writes_pages: false
---

# Code Graph Skill

Navigate and query the code knowledge graph — a structured index of code symbols,
their definitions, references, and call relationships built via Tree-sitter AST
chunking + regex symbol indexing.

## Contract

This skill guarantees:
- Code graph features are built into the KB document processing pipeline — code files 
  uploaded via `artifact_upload` are automatically AST-chunked and symbol-indexed
- `artifact_query` with `filter_slug=<code_page>` can narrow results to specific 
  code pages (general search, not dedicated graph traversal)
- For programmatic access, the `gbrain_core` library exposes internal engine-level 
  functions (`code_def`, `code_refs`, `get_callers_of`, `get_callees_of`) via the 
  `Operations` struct — these are not MCP tools but can be used in Rust code
- `artifact_query mode=graph` is planned for the artifact facade (see §8.2 
  of design doc) but NOT yet implemented — graph-specific queries are not yet 
  available through MCP

## When to Use

Use this skill when:
- Understanding what code graph capabilities exist in gbrain
- Uploading code files to be indexed by the KB pipeline
- Narrowing artifact_query results to code-related pages via `filter_slug`
- Planning to use the Rust library API for code graph operations

Do NOT use this skill for:
- General brain page queries (use brain-ops skill directly)
- Document/evidence search (use `artifact_query` with mode=evidence)
- Entity lookups (use brain-ops lookup chain)

## Current State (v1.1)

### Available through MCP facade (artifact_query):
- `artifact_query` with `filter_slug=<code_page>` — narrow to code pages
- `artifact_query` with `include_sources=true` — show source tracing for code results

### Available through library API (not MCP):
The `gbrain_core::Operations` struct exposes:
- `find_code_definitions(symbol, language)` — find where a symbol is defined
- `get_callers_of(slug, symbol)` — find callers of a symbol (in-edges)
- `get_callees_of(slug, symbol)` — find callees of a symbol (out-edges)
- `get_edges_by_chunk(chunk_id)` — view all edges for a specific chunk

### Pipeline integration:
Code knowledge graph features are built into the KB document processing pipeline:
1. **AST chunking** — Tree-sitter parses code into structural chunks
2. **Symbol indexing** — regex extracts symbol definitions, references, and call edges
3. **Two-pass search expansion** — hybrid search pipeline optionally expands results 
   through code edges (walk_depth: 0-2) for structural context

### Planned:
- `artifact_query mode=graph` — unified artifact facade entry for code graph queries
  (design doc §8.2)

## Phases

### Phase 1: Upload code files for indexing

Upload code files via `artifact_upload` — the KB pipeline automatically:
1. Detects the language via file extension
2. Runs Tree-sitter AST chunking
3. Extracts symbols and call edges via regex
4. Builds the code graph index during document processing

```jsonc
// Upload a Rust source file
{ "tool": "artifact_upload", "params": { "path": "/path/to/lib.rs", "intent": "auto" } }
```

### Phase 2: Search code content (general search)

Use `artifact_query` to search code-related content:

```jsonc
// Search for a symbol or keyword in code pages
{ "tool": "artifact_query", "params": { "query": "handle_request", "mode": "memory" } }

// Narrow to a specific code page
{ "tool": "artifact_query", "params": { "query": "async fn", "filter_slug": "src/lib", "mode": "memory" } }
```

### Phase 3: Library-level code graph operations

For Rust crate consumers using `gbrain_core` directly:

```rust
let ops = Operations::new(&engine, ctx, config);
let definitions = ops.find_code_definitions("handle_request", Some("rust"))?;
let callers = ops.get_callers_of("src/lib", "handle_request")?;
let callees = ops.get_callees_of("src/lib", "handle_request")?;
```

## Supported Languages

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

## Anti-Patterns

- **Expecting `artifact_query mode=graph` to work.** This mode is planned but not yet implemented. Use general search with `filter_slug` for now.
- **Guessing call relationships from function names.** Use the library API (`get_callers_of`/`get_callees_of`) for actual edge traversal in Rust code.
- **Uploading code without Tree-sitter support.** Only the 8 listed languages are supported for AST chunking; unsupported languages fall back to generic text chunking.
- **Not reindexing after refactoring.** Code edges can become stale after refactoring; re-upload the updated files.

## Tools Used

- `artifact_query` — unified knowledge query (current code graph entry through artifact facade)
- `artifact_upload` — upload code files to trigger KB pipeline indexing

Internal library API (Rust only, not MCP):
- `Operations::find_code_definitions` — symbol definition lookup
- `Operations::get_callers_of` — caller traversal (in-edges)
- `Operations::get_callees_of` — callee traversal (out-edges)
- `Operations::get_edges_by_chunk` — chunk-level edge inspection
