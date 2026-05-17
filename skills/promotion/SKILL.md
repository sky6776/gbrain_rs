---
name: promotion
version: 1.0.0
description: |
  **DEPRECATED** — Use the artifact-review skill instead. Promotion candidates
  are now exposed as "suggested changes" via artifact_review_* tools.
  This skill is kept for backward reference only.
triggers:
  - "promotion"
  - "suggested changes"
  - "review changes"
tools:
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

# Promotion Skill — DEPRECATED

> **This skill is deprecated.** Use the **artifact-review** skill instead.
> Promotion candidates are now exposed as "suggested changes" via the
> `artifact_review_*` tools. The `upload_source` and `memory_query` tools
> have been replaced by `artifact_upload` and `artifact_query` respectively.

## Migration Guide

| Old Tool | New Tool |
|----------|----------|
| `upload_source` | `artifact_upload` |
| `memory_query` | `artifact_query` |
| `promotion_list_candidates` | `artifact_review_list` |
| `promotion_get_candidate` | `artifact_review_get` |
| `promotion_apply_candidate` | `artifact_review_apply` |
| `promotion_reject_candidate` | `artifact_review_reject` |
| `promotion_rollback_candidate` | `artifact_review_rollback` |
| `promotion_batch_apply` | Use `artifact_review_list` + loop `artifact_review_apply` |
| `artifact_list` | `artifact_list` (unchanged) |
| `artifact_get` | `artifact_get` (unchanged) |
| `artifact_delete` | `artifact_delete` (unchanged) |
| `artifact_health` | `artifact_health` (unchanged) |
| `get_provenance` | `artifact_get` with `include_sources=true` |
| `gc_orphan_projections` | Internal only (requires `expose_internal_tools=true`) |
| `projection_supersede` | Internal only (requires `expose_internal_tools=true`) |
| `projection_history` | Internal only (requires `expose_internal_tools=true`) |

See `skills/artifact-review/SKILL.md` for the current workflow.