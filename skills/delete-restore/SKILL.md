---
name: delete-restore
version: 1.0.0
description: |
  Soft-delete lifecycle management for artifacts. Delete knowledge sources safely,
  restore accidentally deleted ones, and manage the artifact lifecycle.
triggers:
  - "delete artifact"
  - "restore artifact"
  - "detach artifact"
  - "recover artifact"
  - "undo delete"
tools:
  - artifact_delete
  - artifact_restore
  - artifact_detach
  - artifact_health
  - artifact_list
  - artifact_get
mutating: true
writes_pages: false
---

# Delete-Restore Skill — Artifact Lifecycle Management

Manage the safe deletion lifecycle for knowledge sources (artifacts). Artifacts are
never immediately destroyed — they enter a soft-delete state where data is preserved
but hidden from search and queries.

## Lifecycle

```
Active artifact ──delete──→ Soft-deleted (hidden, data preserved)
                               │
                               ├──restore──→ Active (fully visible again)
                               │
                               └──detach──→ Remove association with a specific page
```

## Contract

This skill guarantees:
- Delete marks all projections, occurrences, KB documents, and provenance as stale/deleted
- Restore reverses a soft-delete, reactivating artifact-deleted items only
- Detach removes association between artifact and a specific page without deleting the artifact
- Dry-run preview is available for both delete and detach operations
- Health check verifies projection consistency

## Phases

### Phase 1: Safe Deletion

Delete an artifact by ID or UID:

**MCP:** `artifact_delete` with `id_or_uid`
**CLI:** `gbrain artifact delete <id-or-uid>`

Use `dry_run=true` to preview impact before committing:

**MCP:** `artifact_delete` with `id_or_uid` and `dry_run=true`
**CLI:** `gbrain artifact delete <id-or-uid> --dry-run`

After deletion:
- All projections (KB, brain, shadow, file) marked as stale
- All occurrences marked as deleted
- Associated KB documents soft-deleted
- Provenance records marked as stale
- Artifact itself marked as deleted

### Phase 2: Restore

Recover a soft-deleted artifact:

**MCP:** `artifact_restore` with `id_or_uid`
**CLI:** `gbrain artifact restore <id-or-uid>`

After restore:
- Artifact returns to active state
- Artifact-deleted occurrences and projections reactivated
- Associated KB documents restored and re-queued
- Provenance records reactivated

### Phase 3: Detach

Remove association between an artifact and a specific page:

**MCP:** `artifact_detach` with `id_or_uid` and `from_slug`
**CLI:** `gbrain artifact detach <id-or-uid> --from <slug>`

Detach only removes the specific occurrence/projection linking the artifact
to that page. The artifact itself remains active.

### Phase 4: Health Check

Verify projection consistency:

**MCP:** `artifact_health`
**CLI:** `gbrain artifact health`

Checks for stale projections, missing KB documents, stuck processing jobs,
and orphan provenance records.

## Anti-Patterns

- **Deleting without dry-run preview.** Always preview impact first.
- **Restoring when only specific items need recovery.** Use detach instead of
  delete+restore for partial disassociation.
- **Not checking artifact health.** Stale projections mean knowledge is
  no longer searchable.
- **Using internal kb_* or promotion_* tools instead of artifact_* facade.**

## Tools Used

- `artifact_delete` — soft-delete artifact (dry-run for preview)
- `artifact_restore` — restore deleted artifact
- `artifact_detach` — remove association with a page
- `artifact_health` — check projection consistency
- `artifact_list` — list all artifacts
- `artifact_get` — get artifact details