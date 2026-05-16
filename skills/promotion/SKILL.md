---
name: promotion
version: 1.0.0
description: |
  Single-entry multi-projection fusion workflow. Upload source files, review
  promotion candidates extracted from KB evidence, accept/reject/apply/rollback
  changes, manage artifacts and projection version chains, and trace provenance.
triggers:
  - "upload source"
  - "review candidates"
  - "promotion"
  - "apply candidate"
  - "rollback candidate"
  - "artifact"
  - "provenance"
  - "projection history"
tools:
  - upload_source
  - memory_query
  - promotion_list_candidates
  - promotion_get_candidate
  - promotion_accept_candidate
  - promotion_reject_candidate
  - promotion_apply_candidate
  - promotion_rollback_candidate
  - promotion_batch_apply
  - artifact_list
  - artifact_get
  - artifact_delete
  - artifact_health
  - get_provenance
  - gc_orphan_projections
  - projection_supersede
  - projection_history
mutating: true
writes_pages: true
writes_to:
  - people/
  - companies/
  - concepts/
---

# Promotion Skill — Single-Entry Multi-Projection Fusion

The promotion system connects uploaded source files to curated brain knowledge
through a structured review workflow. Source files become Artifacts with multiple
projections (KB document, shadow page, file attachment). Evidence extracted from
KB projections generates promotion candidates that a human or agent reviews before
applying to brain pages.

## Contract

This skill guarantees:
- Source files are uploaded via the unified entry point (`upload_source`) with explicit intent
- Promotion candidates are reviewed before application (never auto-applied without review)
- Every applied change is reversible via rollback
- Provenance traces every brain fact back to its source artifact
- Projection version chains are maintained for auditability

## Architecture Overview

```
Source File → upload_source → Artifact
                                  ├─ KB projection (chunked, embedded, searchable)
                                  ├─ Shadow page (gbrain draft)
                                  └─ File attachment (stored file)

KB evidence → promotion candidates → review → accept/reject → apply → brain page update
                                                                  └─ rollback (undo)
```

## Phases

### Phase 1: Upload Source

Use `upload_source` as the single entry point. Choose intent based on the file:

| Intent | Use for | What happens |
|--------|---------|--------------|
| `auto` | Default — system decides | Routes based on file type and content |
| `document` | Research papers, reports | Creates KB projection for searchability |
| `attachment` | Files to attach to existing pages | Creates file attachment linked to `page_slug` |
| `memory` | Personal notes, meeting notes | Creates shadow page + KB projection |
| `promote` | Files with entity information | Creates KB projection + promotion candidates targeting `target_slug` |

**Promotion policies** control how aggressively candidates are generated:

| Policy | Behavior |
|--------|----------|
| `none` | No promotion candidates generated |
| `shadow` | Creates shadow page only (draft, not applied) |
| `candidate` | Creates promotion candidates for review |
| `auto-low-risk` | Auto-applies low-risk candidates, queues others for review |

Use `dry_run=true` to preview the routing plan without committing.

### Phase 2: Review Candidates

After upload, KB evidence extraction generates promotion candidates:

1. **List candidates** — `promotion_list_candidates` with status/type/slug filters.
   Candidate types: `document_summary`, `entity_mention`, `link_suggestion`,
   `timeline_event`, `fact_claim`, `page_create`, `page_update`.
2. **Get candidate details** — `promotion_get_candidate` for full content and risk assessment.
3. **Accept or reject** — `promotion_accept_candidate` (with optional reviewer/notes)
   or `promotion_reject_candidate` (with optional reason).

### Phase 3: Apply Changes

1. **Apply single candidate** — `promotion_apply_candidate` writes the accepted change
   to the target brain page.
2. **Batch apply** — `promotion_batch_apply` with risk level filter (`low`/`medium`/`high`)
   and optional `artifact_id` scope. Use `dry_run=true` to preview.
3. **Rollback** — `promotion_rollback_candidate` undoes an applied change, reverts
   shadow page updates, and marks provenance as stale.

### Phase 4: Artifact Management

1. **List artifacts** — `artifact_list` with pagination.
2. **Get artifact details** — `artifact_get` by ID or UID (e.g., `art_ab12cd34ef56`).
3. **Delete artifact** — `artifact_delete` soft-deletes, marking all projections as stale.
4. **Check health** — `artifact_health` verifies projection consistency.

### Phase 5: Projection Version Chains

Projections form version chains when superseded:

1. **Supersede** — `projection_supersede` replaces an old projection with a new one,
   maintaining the version chain.
2. **Query history** — `projection_history` by `projection_key` (e.g., `kb_doc:42`),
   optionally filtered by `artifact_id` or `projection_type`.
3. **Garbage collect** — `gc_orphan_projections` cleans stale/superseded projections
   older than `stale_days` (default 30). Use `dry_run=true` to preview.

### Phase 6: Provenance

`get_provenance` traces every fact on a brain page back to its source:
- Which artifact contributed the fact
- Which promotion candidate applied it
- When it was applied and by whom

## Cross-reference with memory_query

`memory_query` is the unified query interface that combines brain curated knowledge
with KB document evidence. Strategies:

| Strategy | Use for |
|----------|---------|
| `brain_first` | General questions — brain pages first, KB evidence as supplement |
| `evidence_first` | Document-heavy questions — KB evidence first, brain for context |
| `provenance` | Source tracing — where did this fact come from? |
| `timeline_first` | Chronological questions — timeline entries first |

Use `filter_slug` to scope results to a specific entity page.

## Anti-Patterns

- **Auto-applying all candidates without review.** Even `auto-low-risk` should be
  monitored. High-risk candidates (entity mentions, fact claims) always need human review.
- **Uploading with `intent=auto` when you know the intent.** Explicit intent produces
  better routing.
- **Ignoring rollback.** Applied candidates can be undone. Don't re-create pages
  when rollback exists.
- **Not checking artifact health.** Stale projections mean the artifact's KB data
  is no longer searchable.
- **Skipping provenance.** Every applied fact should be traceable. If provenance
  records are missing, the knowledge graph loses auditability.

## Tools Used

- `upload_source` — unified upload entry point
- `memory_query` — cross-subsystem query
- `promotion_list_candidates` — list candidates
- `promotion_get_candidate` — get candidate details
- `promotion_accept_candidate` — accept a candidate
- `promotion_reject_candidate` — reject a candidate
- `promotion_apply_candidate` — apply a candidate to brain
- `promotion_rollback_candidate` — undo an applied candidate
- `promotion_batch_apply` — batch apply with risk filter
- `artifact_list` — list all artifacts
- `artifact_get` — get artifact details
- `artifact_delete` — soft-delete artifact
- `artifact_health` — check projection consistency
- `get_provenance` — trace fact origins
- `gc_orphan_projections` — clean stale projections
- `projection_supersede` — replace old projection with new
- `projection_history` — query version chain history