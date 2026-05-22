---
name: reports
version: 1.0.0
description: |
  Save and load timestamped report pages. Keyword routing for fast lookup.
  External jobs, agents, or users can write reports into gbrain and query them by keyword.
triggers:
  - "save report"
  - "load latest report"
  - "what's the latest briefing"
  - "show me the pulse"
tools:
  - artifact_query  # 统一查询接口
  - artifact_get    # 获取知识源详情
  - artifact_put    # 统一写入接口
mutating: true
---

# Reports Skill

## Contract

This skill guarantees:
- Reports saved as timestamped gbrain page slugs with frontmatter
- Keyword routing: query → report category mapping
- Latest report can be found by querying a category and comparing report timestamps
- Reports are searchable via `gbrain query`

## Phases

1. **Save report.** Write with `artifact_put` / `gbrain put` to slug
   `reports/{category}/{YYYY-MM-DD-HHMM}` with frontmatter:
   ```yaml
   ---
   title: {report title}
   type: report
   category: {category name}
   date: {YYYY-MM-DD}
   time: {HH:MM PT}
   ---
   ```
2. **Load latest.** Given a category, query matching report pages and compare
   frontmatter timestamps.
3. **Keyword routing.** Map common queries to report categories:
   - "email" / "inbox" → ea-inbox-sweep
   - "social" / "mentions" → social-mentions
   - "briefing" / "morning" → morning-briefing
   - "meeting" → meeting-sync
   - Custom mappings configurable

## Output Format

Saved: `reports/{category}/{YYYY-MM-DD-HHMM}`
Loaded: full report content with metadata.

## Anti-Patterns

- Saving reports without frontmatter (makes them unsearchable)
- Using inconsistent category names across runs
- Loading all reports when only the latest is needed
- Not routing by keyword (forcing exact category name)
