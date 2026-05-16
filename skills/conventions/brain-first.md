# Brain-First Lookup Convention

**Read this before doing ANY entity/person/company/fact lookup.**

Sub-agents and fresh sessions inherit gbrain tools but not the knowledge of
when and how to use them. This file is that knowledge.

## Available GBrain Tools

Your tool inventory includes these through gbrain_rs MCP, or equivalent CLI
commands:

| Tool | Use for |
|------|---------|
| `query` | Hybrid search (keyword + vector + expansion), best quality when embeddings are available |
| `get_page` | Direct page read when you know the slug |
| `get_links` | Outgoing links from a page |
| `get_backlinks` | Who references this entity |
| `get_timeline` | Dated events for an entity |
| `resolve_slugs` | Fuzzy slug resolution |
| `traverse_graph` | Walk the relationship graph |
| `put_page` | Create or update a brain page |
| `add_timeline_entry` | Add a dated event |
| `add_link` | Add a relationship edge |
| `upload_source` | Upload a source file (unified entry point) |
| `memory_query` | Unified memory query across gbrain + KB |
| `kb_search` | Search across KB libraries with profiles |
| `kb_list_libraries` | List KB libraries and their stats |
| `code_def` | Find code symbol definitions |
| `code_refs` | Find code symbol references |
| `get_callers` | Who calls this function |
| `get_callees` | What this function calls |
| `artifact_get` | Get artifact details |
| `get_provenance` | Trace fact origins on a page |

Tool names vary by transport. MCP uses short names; CLI commands are usually
`gbrain <command>`. Use whichever your environment provides.

## The Lookup Chain (MANDATORY ORDER)

1. **`query`** first — hybrid search (keyword + vector + expansion), zero API cost when embeddings are local
2. **`query`** with `expand=false` for keyword-only mode — fast, no LLM expansion
3. **`get_page`** if you found a slug — read the full compiled truth
4. **`memory_query`** for cross-subsystem search — brain + KB evidence in one call
5. **`kb_search`** for document-heavy queries — search KB libraries with profile selection
6. **`code_def`** / **`code_refs`** for code symbol lookups — precise graph queries
7. **External APIs only after steps 1-6 return nothing useful**

Never skip to external APIs without completing steps 1-2. The brain has
thousands of pages. The answer is almost always there.

## Rules

- **Score > 0.5 = use it.** Don't reach for external APIs when the brain answered.
- **User's direct statements are highest-authority data.** The brain captures
  what the user said in meetings, conversations, and notes. External sources
  are supplementary.
- **After bulk brain page writes:** refresh extracted graph/timeline data with
  `gbrain extract --mode all`, or call MCP `sync_brain` when syncing a directory.
- **Every brain page reference in output** should use a clickable link format
  appropriate to the deployment (GitHub URL, local path, or slug).
- **Never use `memory_search` for entity lookups.** Memory tools search
  session notes (MEMORY.md), not the brain knowledge graph. Use
  `query` for entity lookups.

## Entity Page Conventions

Standard directory structure:

| Directory | Type | Example |
|-----------|------|---------|
| `people/` | person | `people/paul-graham.md` |
| `companies/` | company | `companies/stripe.md` |
| `deals/` | deal | `deals/stripe-series-c.md` |
| `meetings/` | meeting | `meetings/2026-04-23-weekly-sync.md` |
| `projects/` | project | `projects/gbrain.md` |
| `yc/` | yc | `yc/batch-w26.md` |

When creating new pages, include proper frontmatter with `type`, `title`,
and `tags` fields.
