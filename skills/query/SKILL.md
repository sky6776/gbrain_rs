---
name: query
version: 1.0.0
description: |
  Answer questions using the brain's knowledge with 3-layer search, synthesis,
  and citation propagation. Use when the user asks a question, wants a lookup,
  or needs information from the brain.
triggers:
  - "what do we know about"
  - "tell me about"
  - "who is"
  - "what happened"
  - "search for"
  - "look up"
  - "background on"
  - "notes on"
  - "who knows who"
  - "relationship between"
  - "connections"
  - "graph query"
tools:
  - artifact_query  # 统一查询接口
  - artifact_get    # 获取知识源详情
mutating: false
---

# Query Skill

Answer questions using the brain's knowledge with 3-layer search and synthesis.

## Contract

This skill guarantees:
- Every answer is grounded in brain content (no hallucination)
- Every claim has a citation tracing back to a specific page slug
- Gaps are flagged explicitly ("the brain doesn't have information on X")
- Source precedence is respected (user statements > compiled truth > timeline > external)
- Conflicting sources are noted with both citations

## Phases

1. **Decompose the question** into search strategies:
   - Keyword search for specific names, dates, terms
   - Semantic query for conceptual questions
   - Filtered memory/evidence/timeline searches for relational questions
2. **Execute searches:**
   - Hybrid search gbrain for semantic+keyword with expansion (query)
   - Unified artifact query for cross-subsystem search (artifact_query)
   - Use `filter_slug` when narrowing to a known entity or page
3. **Read top results.** Read the top 3-5 pages from gbrain to get full context.
4. **Synthesize answer** with citations. Every claim traces back to a specific page slug.
5. **Flag gaps.** If the brain doesn't have info, say "the brain doesn't have information on X" rather than hallucinating.

## Anti-Patterns

- Answering from general knowledge when the brain has relevant content
- Hallucinating facts not in the brain
- Silently picking one source when sources conflict
- Loading full pages when search chunks are sufficient
- Ignoring source precedence (user statements are highest authority)

## Output Format

Answers should include:
- Direct response to the question
- Citations: "According to [Source: people/jane-doe, compiled truth]..."
- Gap flags: "The brain doesn't have information on X"
- Conflict notes when sources disagree

## Quality Rules

- Never hallucinate. Only answer from brain content.
- Cite sources: "According to concepts/do-things-that-dont-scale..."
- Flag stale results: if a search result shows [STALE], note that the info may be outdated
- For "who" questions, search mentions, wikilinks, and cited source context
  because dedicated backlink traversal is not exposed through the artifact facade
- For "what happened" questions, use timeline entries
- For "what do we know" questions, read compiled_truth directly

## Token-Budget Awareness

Search returns **chunks**, not full pages. Read the excerpts first before deciding
whether to load a full page.

- `gbrain query` searches both gbrain curated knowledge and KB document evidence with source tracing.
  These are often enough to answer the question directly.
- Only use `gbrain get <uid>` to load the full artifact detail when a search result confirms
  the page is relevant and you need more context.
- **"Tell me about X"** -- get the full detail (the user wants the complete picture).
- **"Did anyone mention Y?"** -- search results are enough (the user wants a yes/no with evidence).

### Source precedence

When multiple sources provide conflicting information, follow this precedence:

1. **User's direct statements** (highest authority -- what the user told you directly)
2. **Compiled truth** (the brain's synthesized, cited understanding)
3. **Timeline entries** (raw evidence, reverse-chronological)
4. **External sources** (web search, API enrichment -- lowest authority)

When sources conflict, note the contradiction with both citations. Don't silently
pick one.

## Citation in Answers

When referencing brain pages in your answer, propagate inline citations:
- Cite the page: "According to [Source: people/jane-doe, compiled truth]..."
- When brain pages have inline `[Source: ...]` citations, propagate them so
  the user can trace facts to their origin
- When you synthesize across multiple pages, cite all sources

## Graph Traversal

> **注意**: `mode=graph` 尚未在 artifact_query 中实现，图谱遍历暂不可用。
> 当前可用 mode: `auto`/`memory`/`evidence`/`timeline`。

For relationship questions, use memory or evidence mode with `filter_slug` to
narrow results to related entities:

- `gbrain query "<topic>" --mode memory --filter <slug>` — narrow to specific entity's connections
- MCP: `artifact_query` with `mode: "memory"` or `mode: "evidence"` and optional `filter_slug`

Dedicated graph/backlink traversal is not exposed through the artifact facade;
do not promise complete relationship expansion from search alone.

## Search Quality Awareness

If search results seem off (wrong results, missing known pages, irrelevant hits):
- Run `gbrain health` to check knowledge source consistency
- Check embedding coverage -- partial embeddings degrade hybrid search
- Compare hybrid search with `gbrain query "name"`
  for the same query to isolate whether the issue is embedding-related
- Report search quality issues via the review system (`gbrain review list`)

## Tools Used

- Hybrid search gbrain (artifact_query)
- Unified artifact query (artifact_query)
- Read artifact detail (artifact_get)
- ~~List pages in gbrain with filters~~ (list_pages — legacy, 已移除)
- ~~Check backlinks in gbrain~~ (get_backlinks — legacy, 已移除)
- ~~Traverse the link graph in gbrain~~ (traverse_graph — legacy, 已移除)
- ~~View timeline entries in gbrain~~ (get_timeline — legacy, 已移除)
