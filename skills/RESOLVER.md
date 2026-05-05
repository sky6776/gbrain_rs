# gbrain_rs Skill Resolver

This resolver only lists skills that match the current gbrain_rs CLI/MCP surface.
It intentionally omits original gbrain platform skills that require Minions,
skillpack/skillify, Supabase/PGLite setup, OpenClaw runtime hooks, publishing,
cron scheduling, GStack PDF generation, or external research connectors.

## Core Brain Operations

| Trigger | Skill |
|---------|-------|
| Any brain read/write/lookup/citation workflow | `skills/brain-ops/SKILL.md` |
| "What do we know about", "tell me about", "search for", "who is", "background on", "notes on" | `skills/query/SKILL.md` |
| "Who knows who", "relationship between", "connections", "graph query" | `skills/query/SKILL.md` |
| Creating or enriching a person/company/concept/source page | `skills/enrich/SKILL.md` |
| Where should this page go? Filing rules | `skills/repo-architecture/SKILL.md` |
| Fix citations, citation audit, check citation format | `skills/citation-fixer/SKILL.md` |

## Ingestion

| Trigger | Skill |
|---------|-------|
| Generic "ingest this" | `skills/ingest/SKILL.md` |
| User shares a link, article, tweet, note, or idea | `skills/idea-ingest/SKILL.md` |
| Meeting transcript or meeting notes | `skills/meeting-ingestion/SKILL.md` |
| Migrate from Obsidian, Notion export, Logseq, Markdown, CSV, JSON, or Roam-like notes | `skills/migrate/SKILL.md` |

## Operations

| Trigger | Skill |
|---------|-------|
| Add, complete, defer, remove, or review tasks | `skills/daily-task-manager/SKILL.md` |
| Save or load reports | `skills/reports/SKILL.md` |

## Conventions

These apply to all retained brain-writing skills:

- `skills/conventions/quality.md` - citations, backlinks, and notability gate.
- `skills/conventions/brain-first.md` - check gbrain_rs before external sources.
- `skills/_brain-filing-rules.md` - where pages should be filed.
- `skills/_output-rules.md` - output quality standards.

## Removed From Original gbrain

The copied original skillpack included many skills that currently do not apply
to gbrain_rs. They were removed because they route to missing tools or platform
subsystems: `skillify`, `skillpack-check`, `minion-orchestrator`,
`cron-scheduler`, `publish`, `setup`, `frontmatter-guard`, `maintain`,
`smoke-test`, `testing`, `signal-detector`, `webhook-transforms`,
`cross-modal-review`, `brain-pdf`, `book-mirror`, `media-ingest`,
`voice-note-ingest`, `perplexity-research`, `academic-verify`,
`archive-crawler`, `article-enrichment`, `strategic-reading`,
`concept-synthesis`, `data-research`, `daily-task-prep`, `briefing`,
`soul-audit`, `skill-creator`, `install`, and `migrations`.
