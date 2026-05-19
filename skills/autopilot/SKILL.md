---
name: autopilot
version: 1.0.0
description: |
  Health diagnostics and knowledge maintenance. Check brain health via artifact_health,
  review and apply suggested changes, and monitor knowledge base consistency.
triggers:
  - "autopilot"
  - "self-maintain"
  - "health check"
  - "doctor"
  - "integrity"
  - "brain health"
  - "diagnose"
tools:
  - artifact_health       # 健康检查接口
  - artifact_review_list  # 列出建议变更
  - artifact_review_get   # 查看建议变更详情
  - artifact_review_apply # 应用建议变更
  - artifact_review_reject # 拒绝建议变更
  - artifact_list         # 列出知识源
mutating: true
writes_pages: false
---

# Autopilot Skill — Health Diagnostics and Knowledge Maintenance

Keep the brain healthy through health checks and the review/apply cycle for
suggested changes.

## Contract

This skill guarantees:
- Health diagnostics identify issues in knowledge source consistency
- Suggested changes are reviewed and applied systematically
- Maintenance operations are safe and reversible (reject / rollback)
- Dry-run previews are used before applying changes

## Components

### Health Dashboard

Quick overview of brain health:

**MCP:** `artifact_health` — knowledge source consistency, stale projections, orphan count
**CLI:** `gbrain health` — formatted health dashboard

Key metrics:
- Total artifacts and their status distribution (active/deleted/purged)
- Stale projection count (projections needing re-processing)
- Orphan occurrence count (occurrences with no valid artifact)
- Consistency violations

### Suggested Changes (Review System)

The review system surfaces improvement candidates from the promotion pipeline:

**CLI:** `gbrain review list` — list all pending suggestions
**CLI:** `gbrain review list --status pending` — filter by status
**CLI:** `gbrain review show <id>` — view suggestion details
**CLI:** `gbrain review apply <id>` — apply a suggestion
**CLI:** `gbrain review reject <id> --reason "..."` — reject with reason
**CLI:** `gbrain review rollback <id>` — rollback an applied change

### Knowledge Source Management

Routine maintenance operations:

**CLI:** `gbrain list` — list all knowledge sources
**CLI:** `gbrain get <id>` — get artifact detail with projections
**CLI:** `gbrain reprocess <id>` — re-process stale projections
**CLI:** `gbrain delete <id> --dry-run` — preview soft-delete impact
**CLI:** `gbrain restore <id>` — restore soft-deleted artifacts

## Phases

### Phase 1: Quick Health Check

Before any maintenance, assess current state:

1. `gbrain health` — get the health dashboard
2. Review: are there stale projections? consistency issues? orphan records?
3. Prioritize: stale projections > consistency issues > orphan records

### Phase 2: Review Suggested Changes

If health check reveals suggestions:

1. `gbrain review list` — list all suggestions
2. `gbrain review show <id>` — examine each suggestion's details and risk level
3. Prioritize: low-risk suggestions first, review high-risk carefully

### Phase 3: Apply or Reject

Process suggestions systematically:

1. `gbrain review apply <id>` — apply accepted suggestions
2. `gbrain review reject <id> --reason "..."` — reject unwanted ones
3. Verify: `gbrain health` after each batch
4. Rollback if needed: `gbrain review rollback <id>`

### Phase 4: Stale Content Refresh

For stale projections identified by health check:

1. `gbrain list` — identify artifacts with stale projections
2. `gbrain reprocess <id> --dry-run` — preview re-processing
3. `gbrain reprocess <id>` — re-process to regenerate projections
4. `gbrain health` — verify consistency restored

## Anti-Patterns

- **Applying all suggestions blindly.** Review each suggestion's risk level and
  target page before applying. High-risk suggestions may need human judgment.
- **Ignoring stale projections.** Stale projections mean search results are
  out of date with the source content.
- **Not running health check regularly.** Periodic health checks catch issues
  before they accumulate.
- **Skipping dry-run for reprocess.** Always preview with `--dry-run` first.

## CLI Commands

```bash
# Quick health check
gbrain health

# List all knowledge sources
gbrain list

# List pending suggestions
gbrain review list --status pending

# View suggestion details
gbrain review show 1

# Apply a suggestion
gbrain review apply 1

# Reject with reason
gbrain review reject 2 --reason "信息已过时"

# Rollback an applied change
gbrain review rollback 1

# Preview reprocess
gbrain reprocess <id> --dry-run

# Reprocess stale projections
gbrain reprocess <id>

# Restore soft-deleted artifact
gbrain restore <id>
```

## Tools Used

- `artifact_health` — knowledge source health dashboard
- `artifact_review_list` — list suggested changes
- `artifact_review_get` — view suggestion details
- `artifact_review_apply` — apply suggestion
- `artifact_review_reject` — reject suggestion
- `artifact_list` — list all artifacts
