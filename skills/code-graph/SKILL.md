---
name: code-graph
version: 1.1.0
description: |
  Code graph capability notes for gbrain_rs. Dedicated symbol definition,
  reference, caller, and callee APIs exist inside the Rust Operations layer,
  but are not exposed through MCP. The public MCP facade can only use
  artifact_query for general code-related search; mode=graph is not implemented.
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
  - artifact_query  # 统一查询接口（当前 MCP 仅支持通用搜索）
mutating: true
writes_pages: false
---

# Code Graph Skill

Understand the current code graph surface in gbrain_rs. Dedicated graph
operations exist in the Rust library layer, while MCP currently exposes only
general `artifact_query` search.

## Contract

This skill guarantees:
- MCP callers do not pretend dedicated code graph traversal is available.
- `artifact_query` with `filter_slug=<code_page>` can narrow results to code-related pages, but this is general search.
- Code files uploaded with `artifact_upload --intent auto` are not indexed as code graph documents by default; auto routing treats code files as an external code import/sync concern.
- Programmatic Rust callers can use `Operations::find_code_definitions`, `find_code_references`, `get_callers_of`, `get_callees_of`, and `get_edges_by_chunk`.
- `artifact_query mode=graph` is planned but NOT implemented.

## When to Use

Use this skill when:
- Understanding what code graph capabilities exist in gbrain
- Checking what code graph features are actually exposed
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
- `find_code_definitions(symbol, language, limit)` — find where a symbol is defined
- `find_code_references(symbol, language, limit)` — find code chunks referencing a symbol
- `get_callers_of(slug, symbol)` — find callers of a symbol (in-edges)
- `get_callees_of(slug, symbol)` — find callees of a symbol (out-edges)
- `get_edges_by_chunk(chunk_id)` — view all edges for a specific chunk

### Pipeline integration:
Code indexing runs when a page is written as `PageType::Code` (for example a
slug under `code/`) or when fenced code blocks are present in a page:
1. **AST chunking** — Tree-sitter parses supported code pages into structural chunks.
2. **Symbol indexing** — regex extracts symbol definitions and local call edges.
3. **Search indexing** — chunks can be found by general hybrid/keyword search.

This is not the same as document upload. `artifact_upload` is for documents and
attachments; code file upload is not a public code graph import path today.

### Planned:
- `artifact_query mode=graph` — unified artifact facade entry for code graph queries
  (design doc §8.2)

## Phases

### Phase 1: Confirm the access path

For MCP callers, use general search only. Do not call `artifact_query` with
`mode=graph`, and do not use `artifact_upload` as a code graph importer.

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
let ops = Operations::with_config(&engine, ctx, config);
let definitions = ops.find_code_definitions("handle_request", Some("rust"), 10)?;
let references = ops.find_code_references("handle_request", Some("rust"), 20)?;
let callers = ops.get_callers_of("code/lib", "handle_request")?;
let callees = ops.get_callees_of("code/lib", "handle_request")?;
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
- **Uploading code with `artifact_upload` and expecting graph indexing.** Auto upload does not create code graph projections for code files.
- **Not reindexing after refactoring.** Code edges can become stale after refactoring; re-write or reindex the affected code page through the Rust API.

## Tools Used

- `artifact_query` — unified knowledge query (current code graph entry through artifact facade)

Internal library API (Rust only, not MCP):
- `Operations::find_code_definitions` — symbol definition lookup
- `Operations::find_code_references` — symbol reference lookup
- `Operations::get_callers_of` — caller traversal (in-edges)
- `Operations::get_callees_of` — callee traversal (out-edges)
- `Operations::get_edges_by_chunk` — chunk-level edge inspection
