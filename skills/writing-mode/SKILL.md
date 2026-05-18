---
name: writing-mode
version: 1.0.0
description: |
  Writing quality control. Choose writing mode (Strict/Lint/Off), run lint checks,
  and auto-fix quality issues in brain pages.
triggers:
  - "lint"
  - "quality check"
  - "writing mode"
  - "check page quality"
  - "fix lint issues"
tools:
  - artifact_put    # 统一写入接口
  - artifact_query  # 统一查询接口
internal_tools:
  - put_page       # 旧页面写入
  - get_page       # 旧页面获取
  - list_pages     # 旧列表接口
  - query          # 旧查询接口
optional_internal_tools: true
mutating: true
writes_pages: true
writes_to:
  - any page slug
---

# Writing Mode Skill — Quality Control for Brain Pages

Control the quality of content written to brain pages through three writing modes
and a zero-LLM lint system that detects and fixes common issues.

## Contract

This skill guarantees:
- Writing mode is chosen appropriately for the content source
- Lint checks are run when quality verification is needed
- Auto-fix is used cautiously — only for safe, deterministic fixes
- Dry-run is used before applying fixes to verify what will change

## Writing Modes

gbrain supports three writing modes via `put_page`:

| Mode | When to use | Behavior |
|------|-------------|----------|
| `Strict` | AI-generated content, external data | Requires frontmatter, rejects empty content, validates link references |
| `Lint` | Most writes (default for MCP) | Zero-LLM quality check with 6 rules, auto-fixes safe issues |
| `Off` | Bulk imports, raw data dumps | No validation — write directly, fastest mode |

**Mode selection guide:**
- Human-authored notes → `Lint` (catch mistakes without blocking)
- AI agent writes → `Strict` (enforce structure and prevent AI slop)
- Bulk import from external source → `Off` (speed over quality)
- Uncertain → `Lint` (safe default)

## Lint Rules

The lint system checks 6 rules without any LLM calls:

| Rule | What it detects | Auto-fixable? |
|------|-----------------|---------------|
| LLM preamble | "Here is...", "Sure, I'll..." AI intro text | Yes — strips preamble |
| Placeholder dates | `YYYY-MM-DD`, `TBD`, `FIXME` unfilled dates | No — requires human input |
| Missing frontmatter | Pages without YAML `---` header | Yes — inserts minimal frontmatter |
| Broken references | Wikilinks `[[slug]]` pointing to non-existent pages | No — requires page creation or link fix |
| Empty sections | Headers with no content below them | No — requires content |
| Unclosed code fences | ``` blocks without closing ``` | Yes — adds closing fence |

## Phases

### Phase 1: Choose Writing Mode

Before writing content, select the appropriate mode:

1. **Assess content source** — is it human-written, AI-generated, or bulk import?
2. **Select mode** — Strict for AI, Lint for human, Off for bulk.
3. **Write with mode** — include `writer_mode` in `put_page` call (MCP only).

### Phase 2: Lint Check

Run lint to verify page quality:

**CLI:** `gbrain lint [slug]` — check specific page or all pages
**CLI:** `gbrain lint --fix` — auto-fix safe issues
**CLI:** `gbrain lint --dry-run` — preview fixes without applying

Lint output shows each rule violation with:
- Rule name and severity
- Location in the page content
- Whether it's auto-fixable
- Suggested fix (if applicable)

### Phase 3: Fix Issues

1. **Auto-fix safe issues** — `gbrain lint --fix` handles preamble, frontmatter, code fences.
2. **Manual fix unsafe issues** — placeholder dates, broken references, empty sections
   require human judgment or additional information.
3. **Verify fixes** — run `gbrain lint` again after fixing to confirm all issues resolved.

### Phase 4: Post-write Lint (Optional)

Enable automatic lint after every write:

**CLI:** `gbrain config set post_write_lint true`

This runs lint in `Lint` mode after every `put_page` call. Useful for quality
assurance in production environments.

## Anti-Patterns

- **Using `Off` mode for AI-generated content.** AI content frequently contains
  preamble text and placeholder dates that `Lint` would catch.
- **Auto-fixing without dry-run.** Always preview fixes with `--dry-run` first.
- **Ignoring broken reference warnings.** A wikilink to a non-existent page
  creates a dead end in the knowledge graph.
- **Running lint on every write in bulk import.** Use `Off` mode for bulk imports
  and run lint separately afterward.

## CLI Commands

```bash
# Lint all pages
gbrain lint

# Lint a specific page
gbrain lint people/alice

# Auto-fix safe issues
gbrain lint --fix

# Preview fixes without applying
gbrain lint --dry-run

# Preview fixes for a specific page
gbrain lint people/alice --fix --dry-run

# Enable post-write lint
gbrain config set post_write_lint true
```

## Tools Used

- `put_page` — write pages with writer_mode control
- `get_page` — read pages to verify lint fixes
- `list_pages` — identify pages needing lint
- `query` — find pages with quality issues
