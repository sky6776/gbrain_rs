---
name: citation-fixer
version: 1.2.0
description: |
  Audit and fix citation format across brain pages. Ensures every fact has
  an inline [Source: ...] citation matching the standard format. Uses the
  artifact facade only; broken external references are flagged unless a
  deterministic source URL is already available.
triggers:
  - "fix citations"
  - "fix broken citations"
  - "citation audit"
  - "check citations"
  - "citation fixer"
tools:
  - artifact_query  # 统一查询接口
  - artifact_get    # 获取知识源详情/内容
  - artifact_list   # 列出知识源
  - artifact_put    # 统一写入接口
mutating: true
---

# Citation Fixer Skill

> **Convention:** see [conventions/quality.md](../conventions/quality.md) for
> the canonical citation format every fix should match.
>
> **Output rule:** all links MUST be deterministic (built from API data,
> not composed by LLM). See [_output-rules.md](../_output-rules.md).

## Contract

This skill guarantees:

- The requested page set is scanned for citation compliance.
- Missing citations are flagged with specific location.
- Malformed citations are fixed to match the standard format.
- Broken tweet/post references without URLs are flagged for review. Do not
  invent URLs or assume an X/Twitter API integration exists.
- Results reported with counts (scanned, fixed, remaining).

## Phases

1. **Scan relevant artifacts/pages.** Use `artifact_query` to find candidates,
   `artifact_list` for bounded sweeps, and `artifact_get` with content when the
   transport supports it.
2. **Identify issues:**
   - Facts without any citation
   - Citations missing date
   - Citations missing source type
   - Citations with wrong format
   - Tweet/post references without deterministic URLs
3. **Fix format issues.** Rewrite malformed citations to match
   `conventions/quality.md`.
4. **Flag unresolved external references.** Only patch a URL when the source
   artifact or user-provided evidence contains the exact URL.
5. **Report results.** Count: pages scanned, citations found, issues
   fixed, unresolved external references, remaining gaps.

## Broken External Reference Pipeline

For each broken tweet/post/reference, follow this chain. The current
gbrain_rs skill surface does not include an X/Twitter API connector, so this
workflow is conservative.

### Step 1: Identify broken references

Scan the page for patterns that indicate tweet references without URLs:

- Contains words like `tweeted`, `posted`, `said on X`, `RT`, `retweet`,
  `X post`
- Contains quoted text that looks like a tweet (short, punchy, often
  starts with a quote)
- Has `[Source: ... X/Twitter ...]` without an `x.com` URL
- References engagement metrics (likes, impressions) without a link

### Step 2: Extract searchable content

From each broken reference, extract:

- The **handle** (if mentioned: `@<username>`)
- The **quoted text** (if available)
- The **approximate date** (often present in surrounding timeline entries)

### Step 3: Check existing evidence

Use `artifact_query` with `include_sources=true` and inspect related
artifacts. If the exact URL is present in the source artifact, it can be used.
If not, flag the reference as unresolved.

### Step 4: Patch the brain page

Replace the broken citation with a proper one:

**Before:**

```
"<quote fragment>" [Source: <some hand-wavy attribution>]
```

**After:**

```
"<verified quote>" [Source: [X/<handle>, YYYY-MM-DD](https://x.com/<handle>/status/<tweet_id>)]
```

## Batch mode

When sweeping many pages:

### Find candidate pages

Use bounded `artifact_query` searches such as `tweet`, `posted`, `said on X`,
or `Source: X`, then inspect likely artifacts/pages. Avoid filesystem scans as
the source of truth; gbrain_rs stores brain content in SQLite/artifact storage.

### Priority order

1. Recently created / updated pages — fresh broken refs are easiest to
   resolve while context is fresh.
2. High-traffic pages (frequent reads / writes from other skills).
3. Everything else — bulk cleanup over time.

### Batch safety

- Target small batches first (10-20 candidates).
- Patch only deterministic format issues or URLs already present in evidence.
- Use `artifact_put` with `dry_run=true` where practical before updating pages.

## Output format

```
Citation Audit Report
=====================
Pages scanned:        N
Citations found:      N
Issues fixed:         N
External links resolved: N
Unresolved external refs: N
Remaining gaps:       N (pages with uncitable facts)
```

## Anti-Patterns

- Inventing citations for facts that have no source. Flag them.
- Removing facts that lack citations. Flag them; don't delete.
- Fixing citations without reading the full page context.
- Batch-fixing without checking quality on a sample first.
- Composing tweet URLs by guessing the tweet id. Deterministic links only.

## Integration

This skill can be called:

- **Manually** — "fix citations on this page"
- **By other skills** — `enrich`, `idea-ingest`, or `meeting-ingestion` can call citation-fixer
  before commit to validate output

## Metrics

If running manually as a repeated batch, track state in a small JSON file under
`~/.gbrain/citation-fixer-state.json`:

```json
{
  "last_run": "2026-04-15T...",
  "pages_scanned": 0,
  "citations_fixed": 0,
  "external_links_resolved": 0,
  "citations_unresolvable": 0,
  "pages_remaining": 1424
}
```


## Output Format

The skill's output shape is documented inline in the body sections above (see "Output", "Brain page format", or equivalent). The literal section header here exists for the conformance test (`test/skills-conformance.test.ts`).
