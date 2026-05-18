---
name: migrate
description: Universal migration from Obsidian, Notion, Logseq, markdown, CSV, JSON, Roam
triggers:
  - "migrate from"
  - "import from obsidian"
  - "import from notion"
tools:
  - artifact_put    # 统一写入接口
  - artifact_query  # 统一查询接口
  - artifact_upload # 统一上传接口
  - artifact_health # 健康检查接口
internal_tools:
  - put_page       # 旧页面写入
  - get_page       # 旧页面获取
  - query          # 旧查询接口
  - add_link       # 旧链接接口
  - add_tag        # 旧标签接口
  - get_stats      # 旧统计接口
  - get_health     # 旧健康接口
  - sync_brain     # 旧同步接口
optional_internal_tools: true
mutating: true
---

# Migrate Skill

Universal migration from any wiki, note tool, or brain system into GBrain.

## Contract

- Source data is never modified or deleted; migration is additive only.
- Every migrated page is verified round-trip: written to gbrain, read back, spot-checked.
- Cross-references from the source system (wikilinks, block refs, tags) are converted to gbrain equivalents.
- Migration is tested on a sample (5-10 files) before bulk execution.
- Post-migration health check confirms page count, link integrity, and embedding coverage.

## Supported Sources

| Source | Format | Strategy |
|--------|--------|----------|
| Obsidian | Markdown + `[[wikilinks]]` | Direct import, convert wikilinks to gbrain links |
| Notion | Exported markdown or CSV | Parse Notion's export structure |
| Logseq | Markdown with `((block refs))` | Convert block refs to page links |
| Plain markdown | Any .md directory | Import directory into gbrain directly |
| CSV | Tabular data | Map columns to frontmatter fields |
| JSON | Structured data | Map keys to page fields |
| Roam | JSON export | Convert block structure to pages |

## Phases

1. **Assess the source.** What format? How many files? What structure?
2. **Plan the mapping.** How do source fields map to gbrain fields (type, title, tags, compiled_truth, timeline)?
3. **Test with a sample.** Import 5-10 files, verify by reading them back from gbrain and exporting.
4. **Bulk import.** Import the full directory into gbrain.
5. **Verify.** Check gbrain health and statistics, spot-check pages.
6. **Build links.** Extract cross-references from content and create typed links in gbrain.

## Obsidian Migration

1. Import the vault directory into gbrain (Obsidian vaults are markdown directories)
2. Wire the graph with native wikilink support (v0.12.1+):

   ```bash
   gbrain extract --mode links
   ```

   (admin-tools)

   `extract links` natively parses `[[relative/path]]` and `[[relative/path|Display Text]]`
   alongside standard `[text](page.md)` markdown syntax. Ancestor-search resolution handles
   wiki KBs where authors omit one or more leading `../` prefixes. The `.md` suffix is
   inferred automatically for wikilinks.

Obsidian-specific:
- Tags (`#tag`) become gbrain tags
- Frontmatter properties map to YAML frontmatter stored by gbrain_rs
- Attachments (images, PDFs) are noted but handled separately via file storage

## Notion Migration

1. Export from Notion: Settings > Export > Markdown & CSV
2. Notion exports nested directories with UUIDs in filenames
3. Strip UUIDs from filenames for clean slugs
4. Map Notion's database properties to frontmatter
5. Import the cleaned directory into gbrain

## CSV Migration

For tabular data (e.g., CRM exports, contact lists):
1. For each row in the CSV, create a page with column values as frontmatter
2. Use a designated column as the slug (e.g., name)
3. Use another column as compiled_truth (e.g., notes)
4. Store each page in gbrain

## Verification

After any migration:
1. Check gbrain statistics to verify page count matches source
2. Check gbrain health for orphans and missing embeddings
3. Export pages from gbrain for round-trip verification
4. Spot-check 5-10 pages by reading them from gbrain
5. Test search: query gbrain for "someone you know is in the data"

## Anti-Patterns

- **Bulk import without sample test.** Never import the full dataset before verifying with 5-10 files. The cost of cleaning up hundreds of bad pages is enormous.
- **Destroying source data.** Migration is additive. Never modify, move, or delete the source files.
- **Ignoring cross-references.** Wikilinks, block refs, and tags from the source system must be converted to gbrain equivalents. Dropping them loses the knowledge graph.
- **Skipping verification.** A migration without post-import health check, page count comparison, and spot-check reads is incomplete.

## Output Format

```
MIGRATION REPORT -- [source] -> GBrain
=======================================

Source: [format] ([file count] files, [size])
Mapping: [field mapping summary]

Sample Test (N files):
- Imported: N/N
- Round-trip verified: N/N
- Cross-refs converted: N

Bulk Import:
- Total imported: N
- Skipped (duplicates/errors): N
- Links created: N
- Tags migrated: N

Verification:
- Page count match: [yes/no]
- Health check: [pass/fail]
- Search test: [query] -> [result count] hits
```

## Tools Used

- Write pages to gbrain (artifact_put — unified entry point)
- Read pages from gbrain (artifact_query, artifact_get)
- Upload files as knowledge sources (artifact_upload)
- Link entities in gbrain (add_link — admin-tools)
- Tag pages in gbrain (add_tag — admin-tools)
- Get gbrain statistics (get_stats — admin-tools)
- Check gbrain health (artifact_health)
- Search gbrain (artifact_query)
