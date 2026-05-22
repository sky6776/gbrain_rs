---
name: brain-ops
version: 1.0.0
description: |
  Brain knowledge base operations. The core read/write cycle: brain-first lookup,
  read-enrich-write loop, source attribution, ambient enrichment, back-linking.
  Read this before any brain interaction.
triggers:
  - any brain read/write/lookup/citation
tools:
  - artifact_query  # 统一查询接口
  - artifact_put    # 统一写入接口
  - artifact_upload # 统一上传接口
  - artifact_list   # 列表接口
  - artifact_get    # 获取详情
  - artifact_delete # 删除接口
  - artifact_restore # 恢复接口
  - artifact_review_list # 建议变更列表
  - artifact_review_apply # 应用变更
  - artifact_review_rollback # 回滚变更
mutating: true
writes_pages: true
writes_to:
  - people/
  - companies/
  - deals/
  - concepts/
  - meetings/
---

# Brain Operations — The Ambient Context Layer

The brain is not an archive. It is a live context membrane that every interaction
flows through in both directions.

> **Convention:** See `skills/conventions/brain-first.md` for the 5-step lookup protocol.
> **Convention:** See `skills/conventions/quality.md` for citation and back-link rules.

## Contract

This skill guarantees:
- Brain is checked BEFORE any external API call (brain-first lookup)
- Every inbound signal triggers the READ → ENRICH → WRITE loop
- Every outbound response checks brain for relevant context
- Source attribution on every fact written (inline `[Source: ...]` citations)
- User's direct statements are highest-authority data
- Back-links maintained on every brain write (Iron Law)

## Iron Law: Back-Linking (MANDATORY)

Every mention of a person or company with a brain page MUST create a back-link
FROM that entity's page TO the page mentioning them. An unlinked mention is a
broken brain. See `skills/conventions/quality.md` for format.

## Phases

### Phase 1: Brain-First Lookup (MANDATORY)

Before using ANY external API to research a person, company, or topic:

1. `gbrain query "name"` — unified knowledge query (memory + evidence + timeline)
2. `gbrain query "natural question about name" --mode memory` — brain-first search
3. `gbrain get <uid>` — read full artifact detail if needed
4. Check backlinks: who references this entity?
5. Check timeline: recent events involving this entity

The brain almost always has something. External APIs fill gaps, not start from scratch.

### Phase 2: On Every Inbound Signal (READ → ENRICH → WRITE)

Every message, meeting, email, or conversation that references a person or company:

1. **Detect entities** — people, companies, deals mentioned
2. **Load brain pages** — read existing pages for context before responding
3. **Identify new information** — what does this signal tell us that the page doesn't know?
4. **Write it back** — update the brain page with new info + timeline entry + source citation
5. **Create if missing** — if notable and no page exists, create via enrich skill

**User's direct statements are the highest-value data source.** Write them to brain
pages immediately with attribution `[Source: User, YYYY-MM-DD]`.

### Phase 2.5: Structured Graph Updates (automatic)

`artifact_put` with `intent=memory` writes stable brain pages. The underlying
page write path reconciles links from explicit markdown links, wikilinks, bare
slug references, and supported frontmatter fields. Stale markdown/frontmatter
links are removed during that reconciliation.

- No manual `add_link` calls needed for ordinary knowledge writes via artifact_put.
- Inferred link types: `attended` (meeting -> person), `works_at`, `invested_in`,
  `founded`, `advises`, `source` (frontmatter), `mentions` (default).
- MCP does not expose graph traversal yet; relationship questions should use
  `artifact_query` with `mode=memory`/`evidence` and `filter_slug`.
- Timeline reads use `artifact_query` with `mode=timeline`. To write timeline
  data, include a timeline section in the `artifact_put` content; there is no
  standalone public timeline tool.

### Phase 3: On Every Outbound Response (READ → PULL → RESPOND)

Before answering any question about a person, company, or topic:

1. **Check the brain** — read relevant pages
2. **Pull context** — use compiled truth + recent timeline
3. **Respond with context** — the brain makes every answer better

Don't answer from general knowledge when a brain page exists.

### Phase 4: Ambient Enrichment

This is not a special mode. This is the default. Everything the user says is an
ingest event.

- Person mentioned → check brain, create/enrich if needed (spawn background)
- Company mentioned → same
- Link shared → ingest it (delegate to idea-ingest)
- Data shared → delegate to appropriate skill

**Rules:**
- Never interrupt the conversation to do enrichment
- Spawn sub-agents for anything that would slow down the response
- Never announce "I'm enriching the brain" — just do it silently

## Output Format

No separate output. Brain-ops is an always-on behavior layer, not a report generator.
The output is updated brain pages and enriched responses.

## Citation Format in gbrain_rs

Artifact 统一接口在 `include_sources=true` 时返回 `sources[].artifact_uid`，
引用时使用 `[Source: artifact_uid]` 格式。通过 `artifact_query` 的
`include_sources=true` 参数获取完整来源追溯链。

## Anti-Patterns

- Answering questions about people/companies without checking the brain first
- Using external APIs before checking the brain
- Writing facts without inline `[Source: ...]` citations
- Blocking the response to do enrichment
- Overwriting user's direct statements with lower-authority sources
- Creating brain pages for non-notable entities

## Tools Used

- `artifact_query` — unified knowledge query (memory + evidence + timeline; graph mode is not implemented)
- `artifact_put` — write to long-term memory (unified entry point)
- `artifact_upload` — upload file as knowledge source
- `artifact_list` — list all knowledge sources
- `artifact_get` — get knowledge source details
- `artifact_delete` — soft-delete knowledge source
- `artifact_restore` — restore soft-deleted knowledge source
- `artifact_review_list` — list suggested changes
- `artifact_review_apply` — apply a suggested change
- `artifact_review_rollback` — undo an applied change
