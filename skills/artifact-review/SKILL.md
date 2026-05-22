---
name: artifact-review
version: 2.0.0
description: |
  Artifact-first unified knowledge workflow. Write memories, upload files,
  query knowledge with source tracing, review suggested changes, and manage
  artifacts through the unified artifact_* facade.
triggers:
  - "write memory"
  - "upload file"
  - "query knowledge"
  - "review changes"
  - "suggested changes"
  - "artifact"
  - "provenance"
  - "source tracing"
tools:
  - artifact_put
  - artifact_upload
  - artifact_query
  - artifact_list
  - artifact_get
  - artifact_delete
  - artifact_detach
  - artifact_restore
  - artifact_reprocess
  - artifact_health
  - artifact_review_list
  - artifact_review_get
  - artifact_review_apply
  - artifact_review_reject
  - artifact_review_rollback
mutating: true
writes_pages: true
writes_to:
  - people/
  - companies/
  - concepts/
---

# Artifact Review Skill — Unified Knowledge Operations

The artifact facade provides a single entry point for all knowledge operations:
writing memories, uploading files, querying knowledge, reviewing suggested changes,
and managing the lifecycle of knowledge sources.

## Contract

This skill guarantees:
- All knowledge operations go through the `artifact_*` facade (no direct KB/gbrain/promotion access)
- User-provided documents or files are ingested with `artifact_upload`; do not read a document and pass its contents to `artifact_put`
- Non-document knowledge, manual notes, and agent-authored memory updates are written with `artifact_put`
- Suggested changes are reviewed before application
- Every applied change is reversible via rollback
- Source tracing (provenance) connects every brain fact back to its source artifact
- Intent-based routing controls how knowledge enters the system

## Architecture Overview

```
Manual Memory → artifact_put → Artifact
                                    ├─ Brain page projection (stable page)
                                    ├─ KB projection (optional, searchable)
                                    └─ Provenance (source tracing)

Upload File → artifact_upload → Artifact
                                    ├─ KB projection (chunked, embedded, searchable)
                                    ├─ Shadow page (draft)
                                    └─ File attachment (stored file)

KB evidence → suggested changes → review → apply/reject → brain page update
                                                          └─ rollback (undo)
```

## Phases

### Phase 1: Write Memory or Upload File

**Manual memory / non-document knowledge** — `artifact_put` with slug, content, and intent:

| Intent | Use for | What happens |
|--------|---------|--------------|
| `memory` | Human notes, structured facts, non-document knowledge | Creates stable brain page + optional KB projection |
| `evidence` | Research evidence only | Creates KB projection only, no brain page |
| `promote` | Knowledge to review before publishing | Creates shadow page + KB + review candidates |

`artifact_put` may read only small UTF-8 text files, but if the user uploaded or provided a document path, prefer `artifact_upload`.

**Document/file upload** — `artifact_upload` with path and intent:

| Intent | Use for | What happens |
|--------|---------|--------------|
| `auto` | Default — system decides | Routes based on file type |
| `evidence` / `document` | Research papers, reports, user-uploaded docs | KB projection + shadow page + candidate promotion |
| `attachment` | Files to attach to existing pages | File attachment linked to `page_slug` |
| `memory` | Personal notes, meeting notes | Shadow page + KB projection |
| `promote` | Files with entity information | KB projection + review candidates |

Use `dry_run=true` to preview the routing plan without committing.

### Phase 2: Query Knowledge

`artifact_query` is the unified query interface combining brain curated knowledge
with KB document evidence and source tracing:

| Mode | Use for |
|------|---------|
| `auto` / `memory` | General questions — brain pages first |
| `evidence` | Document-heavy questions — KB evidence first |
| `timeline` | Chronological questions — timeline entries |

Set `include_sources=true` to trace each result back to its source artifact.

### Phase 3: Review Suggested Changes

After upload, KB evidence extraction generates suggested changes:

1. **List changes** — `artifact_review_list` with status/target filters.
2. **Get change details** — `artifact_review_get` for full content and risk assessment.
3. **Apply or reject** — `artifact_review_apply` or `artifact_review_reject`.

### Phase 4: Artifact Management

1. **List artifacts** — `artifact_list` with pagination.
2. **Get artifact details** — `artifact_get` by ID or UID, with optional `include_sources` and `include_projections`.
3. **Delete artifact** — `artifact_delete` soft-deletes, marking all projections and occurrences as stale/deleted.
   Use `dry_run=true` for impact preview.
4. **Restore artifact** — `artifact_restore` reverses a soft-delete.
5. **Detach** — `artifact_detach` removes association between artifact and a specific page.
6. **Reprocess** — `artifact_reprocess` rebuilds all projections.
7. **Check health** — `artifact_health` verifies projection consistency.

## Anti-Patterns

- **Using internal tools (`kb_*`, `promotion_*`) instead of `artifact_*`.** The facade is the only public interface.
- **Auto-applying all suggested changes without review.** High-risk changes always need human review.
- **Ignoring rollback.** Applied changes can be undone. Don't re-create pages when rollback exists.
- **Not checking artifact health.** Stale projections mean knowledge is no longer searchable.
- **Skipping source tracing.** Every fact should be traceable via `include_sources=true`.

## Tools Used

- `artifact_put` — write manual memory
- `artifact_upload` — upload file as knowledge source
- `artifact_query` — unified knowledge query with source tracing
- `artifact_list` — list all knowledge sources
- `artifact_get` — get knowledge source details with sources/projections
- `artifact_delete` — soft-delete knowledge source (dry-run for preview)
- `artifact_detach` — remove association with a page
- `artifact_restore` — restore deleted knowledge source
- `artifact_reprocess` — rebuild projections
- `artifact_health` — check knowledge source consistency
- `artifact_review_list` — list suggested changes
- `artifact_review_get` — get suggested change details
- `artifact_review_apply` — apply a suggested change
- `artifact_review_reject` — reject a suggested change
- `artifact_review_rollback` — undo an applied change
