---
name: delete-restore
version: 1.0.0
description: |
  Soft-delete lifecycle management. Delete pages safely, restore accidentally
  deleted pages, and permanently purge old deleted content.
triggers:
  - "delete page"
  - "restore page"
  - "purge deleted"
  - "recover page"
  - "undo delete"
tools:
  - delete_page
  - get_page
  - list_pages
  - get_stats
mutating: true
writes_pages: false
---

# Delete-Restore Skill — Soft-Delete Lifecycle

Manage the safe deletion lifecycle for brain pages. Pages are never immediately
destroyed — they enter a soft-delete state where data is preserved but hidden
from search and queries.

## Lifecycle

```
Active page ──delete──→ Soft-deleted (hidden, data preserved)
                          │
                          ├──restore──→ Active (fully visible again)
                          │
                          └──purge-deleted──→ Permanently destroyed (irreversible)
```

## Contract

This skill guarantees:
- Deletion requires explicit confirmation (`confirm=true` for MCP, `--force` for CLI)
- Soft-deleted pages are recoverable via restore
- Purge operations have a time threshold to prevent accidental permanent deletion
- Stats reflect the count of soft-deleted pages for monitoring

## Phases

### Phase 1: Safe Deletion

Delete a page with confirmation:

**MCP:** `delete_page` with `slug` and `confirm=true`
**CLI:** `gbrain delete <slug> --force`

Without confirmation, the operation is rejected. This prevents accidental bulk
deletion by AI agents.

After deletion:
- Page is hidden from `query`, `list_pages`, and `get_page` results
- Links and timeline entries referencing the page remain intact
- Raw data and file attachments are preserved
- The page can be fully restored

### Phase 2: Restore

Recover a soft-deleted page:

**CLI:** `gbrain restore <slug>`

After restore:
- Page returns to active state, visible in all queries
- All previous content, links, tags, and timeline entries are intact
- Embeddings may need regeneration if they became stale during deletion

### Phase 3: Permanent Purge

Permanently destroy soft-deleted pages older than a threshold:

**CLI:** `gbrain purge-deleted --older-than-hours <N>`

- Default threshold: 72 hours (3 days)
- Recommended for maintenance: 168 hours (7 days)
- This operation is IRREVERSIBLE — all data is destroyed
- Use `--dry-run` to preview what would be purged before committing

### Phase 4: Monitoring

Check how many soft-deleted pages exist:

**MCP:** `get_stats` — returns `deleted_page_count`
**CLI:** `gbrain stats` — shows deleted page count in summary

Regular monitoring prevents deleted pages from accumulating indefinitely.

## Anti-Patterns

- **Deleting without confirmation.** Always use `confirm=true` (MCP) or `--force` (CLI).
  Unconfirmed deletes are rejected by the engine.
- **Purging immediately after deletion.** Keep a grace period (minimum 72 hours)
  to allow for accidental deletion recovery.
- **Not monitoring deleted page count.** Accumulated soft-deleted pages consume
  storage and should be purged periodically.
- **Using purge-deleted with a very low threshold.** `--older-than-hours 0` would
  destroy all deleted pages immediately, eliminating the safety net.

## CLI Commands

```bash
# Soft-delete a page (requires --force)
gbrain delete people/alice --force

# Restore a deleted page
gbrain restore people/alice

# Preview what would be permanently purged
gbrain purge-deleted --older-than-hours 168 --dry-run

# Permanently purge pages deleted more than 7 days ago
gbrain purge-deleted --older-than-hours 168

# Check deleted page count
gbrain stats
```

## Tools Used

- `delete_page` — soft-delete a page (requires confirm=true)
- `get_page` — verify page state after restore
- `list_pages` — confirm page visibility
- `get_stats` — monitor deleted page count
