# Brain-First Lookup Convention

**Read this before doing ANY entity/person/company/fact lookup.**

Sub-agents and fresh sessions inherit gbrain tools but not the knowledge of
when and how to use them. This file is that knowledge.

## Available GBrain Tools

Your tool inventory includes these through gbrain_rs MCP, or equivalent CLI
commands:

**Artifact Facade (default — use these first):**

| Tool | Use for |
|------|---------|
| `artifact_put` | Write manual memory / promote shadow pages (unified entry point) |
| `artifact_upload` | Upload a file as knowledge source |
| `artifact_query` | Unified knowledge query with source tracing (memory + evidence + timeline) |
| `artifact_list` | List all knowledge sources |
| `artifact_get` | Get knowledge source details |
| `artifact_review_list` | List suggested changes |
| `artifact_review_apply` | Apply a suggested change |
| `artifact_review_rollback` | Undo an applied change |
| `artifact_delete` | Soft-delete a knowledge source |
| `artifact_restore` | Restore a deleted knowledge source |
| `artifact_health` | Check knowledge source consistency |

**Admin-Tools (legacy — require `admin-tools` feature):**

| Tool | Use for |
|------|---------|
| `query` | Hybrid search (keyword + vector + expansion), best quality when embeddings are available |
| `get_page` | Direct page read when you know the slug |
| `put_page` | Create or update a brain page |
| `add_timeline_entry` | Add a dated event |
| `add_link` | Add a relationship edge |
| `get_links` | Outgoing links from a page |
| `get_backlinks` | Who references this entity |
| `get_timeline` | Dated events for an entity |
| `resolve_slugs` | Fuzzy slug resolution |
| `traverse_graph` | Walk the relationship graph |
| `code_def` | Find code symbol definitions |
| `code_refs` | Find code symbol references |
| `get_callers` | Who calls this function |

Tool names vary by transport. MCP uses short names; CLI commands are usually
`gbrain <command>`. Use whichever your environment provides.

## The Lookup Chain (MANDATORY ORDER)

1. **`artifact_query`** first — unified knowledge query (memory + evidence + timeline + source tracing)
2. **`artifact_query`** with `mode=memory` for brain-first search — curated knowledge only
3. **`artifact_query`** with `mode=evidence` for document-heavy queries — KB evidence first
4. **`query`** (admin-tools) for hybrid search when you need keyword+vector+expansion
5. **`get_page`** (admin-tools) if you found a slug — read the full compiled truth
6. **`code_def`** / **`code_refs`** for code symbol lookups — precise graph queries
7. **External APIs only after steps 1-3 return nothing useful**

Never skip to external APIs without completing steps 1-3. The brain has
thousands of pages. The answer is almost always there.

## Rules

- **Score > 0.5 = use it.** Don't reach for external APIs when the brain answered.
- **User's direct statements are highest-authority data.** The brain captures
  what the user said in meetings, conversations, and notes. External sources
  are supplementary.
- **After bulk brain page writes:** refresh extracted graph/timeline data with
  `gbrain extract --mode all` (admin-tools), or call MCP `sync_brain` when syncing a directory (admin-tools).
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
