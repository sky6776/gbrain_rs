---
name: reports
version: 1.0.0
description: |
  Save and load timestamped reports. Keyword routing for fast lookup. Cron jobs
  save output as reports; the agent or user queries them by keyword.
triggers:
  - "save report"
  - "load latest report"
  - "what's the latest briefing"
  - "show me the pulse"
tools:
  - artifact_query  # 统一查询接口
  - artifact_put    # 统一写入接口
internal_tools:
  - get_page       # 旧页面获取
  - put_page       # 旧页面写入
  - query          # 旧查询接口
optional_internal_tools: true
mutating: true
---

# Reports Skill

## Contract

This skill guarantees:
- Reports saved with timestamped filenames and frontmatter
- Keyword routing: query → report category mapping
- Latest report loadable by category name
- Reports are searchable via `gbrain artifact query`

## Phases

1. **Save report.** Write to `reports/{category}/{YYYY-MM-DD-HHMM}.md` with frontmatter:
   ```yaml
   ---
   title: {report title}
   type: report
   category: {category name}
   date: {YYYY-MM-DD}
   time: {HH:MM PT}
   ---
   ```
2. **Load latest.** Given a category, find the most recent report file.
3. **Keyword routing.** Map common queries to report categories:
   - "email" / "inbox" → ea-inbox-sweep
   - "social" / "mentions" → social-mentions
   - "briefing" / "morning" → morning-briefing
   - "meeting" → meeting-sync
   - Custom mappings configurable

## Output Format

Saved: `reports/{category}/{YYYY-MM-DD-HHMM}.md`
Loaded: full report content with metadata.

## Anti-Patterns

- Saving reports without frontmatter (makes them unsearchable)
- Using inconsistent category names across runs
- Loading all reports when only the latest is needed
- Not routing by keyword (forcing exact category name)
