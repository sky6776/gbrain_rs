---
name: autopilot
version: 1.0.0
description: |
  Self-maintenance daemon and health diagnostics. Run autopilot for automated
  embedding and integrity checks, diagnose issues with doctor, and monitor
  brain health.
triggers:
  - "autopilot"
  - "self-maintain"
  - "health check"
  - "doctor"
  - "integrity"
  - "brain health"
  - "diagnose"
tools:
  - artifact_health # 健康检查接口
internal_tools:
  - get_stats      # 旧统计接口
  - get_health     # 旧健康接口
optional_internal_tools: true
mutating: true
writes_pages: false
---

# Autopilot Skill — Self-Maintenance and Health Diagnostics

Keep the brain healthy through automated maintenance and diagnostic tools.

## Contract

This skill guarantees:
- Autopilot runs embedding generation and integrity checks automatically
- Health diagnostics identify issues before they impact search quality
- Doctor provides comprehensive diagnosis with actionable recommendations
- Maintenance operations are safe and reversible (except purge)

## Components

### Autopilot Daemon

The autopilot daemon is a self-maintaining process that:

1. **Embeds stale content** — generates embeddings for chunks that lack vectors
2. **Checks integrity** — verifies data consistency across tables
3. **Reports health** — logs health metrics for monitoring

**CLI:** `gbrain autopilot` — run continuously (default interval: 3600 seconds)
**CLI:** `gbrain autopilot --once` — run one cycle and exit
**CLI:** `gbrain autopilot --interval 600` — run every 10 minutes

### Health Dashboard

Quick overview of brain health:

**MCP:** `get_health` — embedding coverage, stale pages, orphan count
**CLI:** `gbrain health` — formatted health dashboard

Key metrics:
- Embedding coverage (% of chunks with vectors)
- Stale page count (pages needing re-embedding)
- Orphan page count (pages with no inbound links)
- Soft-deleted page count

### Doctor

Comprehensive diagnostic with recommendations:

**CLI:** `gbrain doctor` — full diagnosis (may take time)
**CLI:** `gbrain doctor --fast` — skip expensive checks

Doctor checks:
- Database integrity (table consistency, foreign keys)
- Embedding coverage and staleness
- Orphan pages and broken links
- FTS5 index health
- Schema version compatibility

### Integrity Check

Focused data integrity verification:

**CLI:** `gbrain integrity` — check data consistency

Verifies:
- All pages have corresponding chunks
- All chunks have valid page references
- No orphaned embeddings (vectors without chunks)
- Link table consistency

### Orphan Detection

Find pages with no inbound connections:

**CLI:** `gbrain orphans` — list orphan pages

Orphan pages are:
- Not referenced by any other page
- Not discoverable through graph traversal
- Candidates for enrichment or linking

## Phases

### Phase 1: Quick Health Check

Before any maintenance, assess current state:

1. `gbrain health` — get the health dashboard
2. `gbrain stats` — get page/chunk counts
3. Review: are there stale embeddings? orphan pages? integrity issues?

### Phase 2: Diagnosis

If health check reveals issues:

1. `gbrain doctor` — comprehensive diagnosis
2. Review recommendations from doctor output
3. Prioritize: embedding staleness > integrity issues > orphan pages

### Phase 3: Automated Maintenance

Run autopilot for automated fixes:

1. `gbrain autopilot --once` — single maintenance cycle
2. For ongoing maintenance: `gbrain autopilot --interval 600`
3. Monitor: `gbrain health` after each cycle

### Phase 4: Manual Intervention

For issues autopilot can't fix:

1. `gbrain embed` — manually embed specific pages
2. `gbrain orphans` — identify and enrich orphan pages
3. `gbrain integrity` — verify data consistency
4. `gbrain lint` — check page quality

## Anti-Patterns

- **Running autopilot without checking health first.** Always assess before maintaining.
- **Setting autopilot interval too low.** Below 60 seconds wastes API calls on
  embedding generation when nothing has changed.
- **Ignoring doctor recommendations.** Doctor identifies specific issues — address them.
- **Not monitoring embedding coverage.** Low coverage means search quality is degraded.

## CLI Commands

```bash
# Quick health check
gbrain health

# Comprehensive diagnosis
gbrain doctor

# Fast diagnosis (skip expensive checks)
gbrain doctor --fast

# Check data integrity
gbrain integrity

# Find orphan pages
gbrain orphans

# Run one maintenance cycle
gbrain autopilot --once

# Continuous maintenance (every 10 minutes)
gbrain autopilot --interval 600

# Manual embedding for stale content
gbrain embed

# Embed specific pages
gbrain embed people/alice companies/acme --batch-size 10

# Brain statistics
gbrain stats
```

## Tools Used

- `get_stats` — brain statistics
- `get_health` — health dashboard
