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
| `artifact_put` | Write manual memory and non-document knowledge |
| `artifact_upload` | Upload user-provided documents/files as knowledge sources |
| `artifact_query` | Unified knowledge query with source tracing (memory + evidence + timeline) |
| `artifact_list` | List all knowledge sources |
| `artifact_get` | Get knowledge source details |
| `artifact_review_list` | List suggested changes |
| `artifact_review_apply` | Apply a suggested change |
| `artifact_review_rollback` | Undo an applied change |
| `artifact_delete` | Soft-delete a knowledge source |
| `artifact_restore` | Restore a deleted knowledge source |
| `artifact_health` | Check knowledge source consistency |

**已移除的内部工具（不再可用，请使用 Artifact Facade 替代）:**

| 旧工具 | 替代方案 |
|--------|----------|
| `query` | `artifact_query` |
| `get_page` | `artifact_get` |
| `put_page` | `artifact_put` |
| `add_timeline_entry` | `artifact_put` with a timeline section in the page content |
| `add_link` | `artifact_put` content/frontmatter link reconciliation |
| `get_links` | 暂未暴露（mode=graph 尚未实现） |
| `get_backlinks` | 暂未暴露（mode=graph 尚未实现） |
| `get_timeline` | `artifact_query` (mode=timeline) |
| `resolve_slugs` | `artifact_query` (模糊匹配内置) |
| `traverse_graph` | 暂未暴露（mode=graph 尚未实现） |
| `code_def` | 暂未暴露为 MCP；仅 Rust `Operations` 内部 API 可用 |
| `code_refs` | 暂未暴露为 MCP；仅 Rust `Operations` 内部 API 可用 |
| `get_callers` | 暂未暴露为 MCP；仅 Rust `Operations` 内部 API 可用 |

Tool names vary by transport. MCP uses short names; CLI commands are usually
`gbrain <command>`. Use whichever your environment provides.

## The Lookup Chain (MANDATORY ORDER)

1. **`artifact_query`** first — 统一知识查询（memory + evidence + timeline + 来源追溯）
2. **`artifact_query`** with `mode=memory` for brain-first search — 仅精选知识
3. **`artifact_query`** with `mode=evidence` for document-heavy queries — KB 证据优先
4. **Code graph:** MCP 只能做通用 `artifact_query` 搜索；专用符号/调用图 API 仅在 Rust `Operations` 内部可用
5. **External APIs only after steps 1-4 return nothing useful**

Never skip to external APIs without completing steps 1-4. The brain has
thousands of pages. The answer is almost always there.

## Rules

- **Score > 0.5 = use it.** Don't reach for external APIs when the brain answered.
- **User's direct statements are highest-authority data.** The brain captures
  what the user said in meetings, conversations, and notes. External sources
  are supplementary.
- **Tool choice:** user-uploaded documents/files go straight to `artifact_upload`; non-document knowledge and curated memory go through `artifact_put`.
- **After brain page writes:** chunks and links are refreshed by the page write path. Timeline rows are only created from supported timeline content; there is no standalone public timeline write tool.
- **Every brain page reference in output** should use a clickable link format
  appropriate to the deployment (GitHub URL, local path, or slug).
- **Never use `memory_search` for entity lookups.** Memory tools search
  session notes (MEMORY.md), not the brain knowledge graph. Use
  `artifact_query` for entity lookups.

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
