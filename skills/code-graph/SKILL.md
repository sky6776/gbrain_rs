---
name: code-graph
version: 1.0.0
description: |
  Code knowledge graph operations. Find symbol definitions, trace references,
  explore call graphs, search code chunks, and reindex code pages.
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
  - artifact_query  # 统一查询接口
internal_tools:
  - code_def       # 代码定义查找
  - code_refs      # 代码引用查找
  - search_code_chunks # 代码块搜索
  - get_callers    # 代码调用者查找
  - get_callees    # 代码被调用者查找
  - get_code_edges_by_chunk # 代码边查询
  - reindex_code_page # 代码页重索引
optional_internal_tools: true
mutating: true
writes_pages: false
---

# Code Graph Skill

Navigate and query the code knowledge graph — a structured index of code symbols,
their definitions, references, and call relationships.

## Contract

This skill guarantees:
- Symbol lookups use the code graph before falling back to text search
- Call chains are traced through the graph, not guessed from names
- Code pages are reindexed when their content changes significantly
- Search results include language and symbol kind context

## When to Use

Use this skill when:
- Finding where a function/type/constant is defined
- Tracing who calls a function (callers) or what a function calls (callees)
- Searching for code by keyword or symbol text
- Understanding call relationships between modules
- Reindexing a code page after significant changes

Do NOT use this skill for:
- General brain page queries (use `query` skill)
- Document/evidence search (use `artifact_query` with mode=evidence)
- Entity lookups (use brain-ops lookup chain)

## Phases

### Phase 1: Symbol Definition Lookup

Use `code_def` to find where a symbol is defined:

1. Provide the symbol name (qualified or local) and optionally filter by language.
2. Results include the defining chunk, file path, and surrounding context.
3. For ambiguous symbols (same name in multiple files), filter by `lang` or use
   the qualified name.

### Phase 2: Reference Tracing

Use `code_refs` to find all code chunks that reference a symbol:

1. Provide the symbol name and optional language filter.
2. Results show every chunk that mentions the symbol — imports, calls, type annotations.
3. Combine with `code_def` to build a complete picture: definition + all usages.

### Phase 3: Call Graph Exploration

Use `get_callers` and `get_callees` for directed call graph traversal:

- `get_callers` — "who calls this function?" (incoming edges)
- `get_callees` — "what does this function call?" (outgoing edges)
- Both require a `slug` (the code page) and `symbol` name.
- Use these to trace multi-hop call chains by iterating through results.

### Phase 4: Code Chunk Search

Use `search_code_chunks` for keyword-based code search:

1. Provide a query string (keyword, symbol fragment, or concept).
2. Optionally filter by `lang` or `symbol_kind`.
3. Results return matching chunks with language context.

### Phase 5: Edge Inspection

Use `get_code_edges_by_chunk` to see all call graph edges attached to a specific chunk:

1. Provide the `chunk_id` from a previous search or definition result.
2. Returns all incoming and outgoing call edges for that chunk.

### Phase 6: Reindexing

Use `reindex_code_page` when a code page's content has changed significantly:

1. Provide the `slug` of the code page.
2. Rebuilds all code chunks and code edges for that page.
3. Necessary after major refactors, function renames, or structural changes.

## Integration with Brain Query

The `query` tool supports code-aware retrieval via `lang`, `symbol_kind`, `near_symbol`,
and `walk_depth` parameters. For combined semantic + code graph search:

- `query` with `lang="rust"` — filter brain + code results by language
- `query` with `near_symbol="parse_config"` — anchor two-pass retrieval near a symbol
- `query` with `walk_depth=1` — include one-hop code graph neighbors in results

Use `code_def`/`code_refs`/`get_callers` for precise graph queries. Use `query`
with code params for broad semantic + graph hybrid search.

## Anti-Patterns

- **Guessing call relationships from function names.** Use `get_callers`/`get_callees`
  to trace actual edges, not name patterns.
- **Searching code with `query` alone for symbol lookups.** `code_def` is more precise
  for finding definitions.
- **Not reindexing after refactors.** Stale code edges produce wrong call chains.
- **Using `search_code_chunks` when you know the exact symbol.** `code_def` and
  `code_refs` are more targeted.

## Tools Used

- `code_def` — find symbol definitions
- `code_refs` — find symbol references
- `search_code_chunks` — keyword search in code
- `get_callers` — incoming call edges
- `get_callees` — outgoing call edges
- `get_code_edges_by_chunk` — all edges for a chunk
- `reindex_code_page` — rebuild code index for a page