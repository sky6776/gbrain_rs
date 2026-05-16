---
name: ingest
description: Route content to specialized ingestion skills. Detects input type and delegates.
triggers:
  - "ingest this"
  - "save this to brain"
  - "process this meeting"
tools:
  - query
  - get_page
  - put_page
  - add_link
  - add_timeline_entry
  - upload_source
  - memory_query
  - sync_brain
mutating: true
writes_pages: true
writes_to:
  - people/
  - companies/
  - concepts/
  - meetings/
  - sources/
---

# Ingest Skill

Ingest meetings, articles, media, documents, and conversations into the brain.

> **Filing rule:** Read `skills/_brain-filing-rules.md` before creating any new page.

## Contract

- Every fact written to a brain page carries an inline `[Source: ...]` citation with date and provenance.
- Every entity mention creates a back-link from the entity's page to the page mentioning them (Iron Law).
- Raw sources are preserved for provenance with `gbrain file upload` or `put_raw_data`.
- State sections are rewritten with current best understanding, never appended to.
- Notable entities get pages or updates when the current ingestion task mentions them.

> **Convention:** See `skills/conventions/quality.md` for Iron Law back-linking.

Every mention of a person or company with a brain page MUST create a back-link
FROM that entity's page TO the page mentioning them. An unlinked mention is a
broken brain. See `skills/_brain-filing-rules.md` for format.

## Citation Requirements (MANDATORY)

Every fact written to a brain page must carry an inline `[Source: ...]` citation.

- **User's statements:** `[Source: User, {context}, YYYY-MM-DD]`
- **Meeting data:** `[Source: Meeting "{title}", YYYY-MM-DD]`
- **Email/message:** `[Source: email from {name} re: {subject}, YYYY-MM-DD]`
- **Web content:** `[Source: {publication}, {URL}, YYYY-MM-DD]`
- **Social media:** `[Source: X/@handle, YYYY-MM-DD](URL)` (include link)
- **Synthesis:** `[Source: compiled from {sources}]`

## Phases

> **Router note:** This skill is a router. For specialized ingestion, see: idea-ingest and meeting-ingestion.

1. **Parse the source.** Extract people, companies, dates, and events from the input.
2. **For each entity mentioned:**
   - Read the entity's page from gbrain to check if it exists
   - If exists: update compiled_truth (rewrite State section with new info, don't append)
   - If new: check notability gate, then store the page in gbrain with the appropriate type and slug
3. **Append to timeline.** Add a timeline entry in gbrain for each event, with date, summary, and source citation.
4. **Create cross-reference links.** Link entities in gbrain for every entity pair mentioned together, using the appropriate relationship type.
5. **Back-link all entities.** Update EVERY mentioned entity's page with a back-link to this page (Iron Law).
6. **Timeline merge.** The same event appears on ALL mentioned entities' timelines. If Alice met Bob at Acme Corp, the event goes on Alice's page, Bob's page, and Acme Corp's page.

## Entity Detection on Every Message

Production agents should detect entity mentions on EVERY inbound message. This is
the signal detection loop that makes the brain compound over time.

### Protocol

1. **Scan the message** for entity mentions: people, companies, concepts, original
   thinking. Fire on every message (no exceptions unless purely operational).
2. **For each entity detected:**
   - `gbrain query "name"` -- does a page already exist?
   - **If yes:** load context with `gbrain get <slug>`. Use the compiled truth to
     inform your response. Update the page if the message contains new information.
   - **If no:** assess notability (see `skills/_brain-filing-rules.md`). If the entity
     is worth tracking, create a new page with `gbrain put <type/slug>` and populate
     with what you know.
3. **After creating or updating pages:** refresh the index:
   ```bash
   gbrain extract --mode all
   ```
4. **Don't block the conversation.** Entity detection and enrichment should happen
   alongside the response, not before it. The user shouldn't wait for brain writes
   to get an answer.

### What counts as notable

- People the user interacts with or discusses (not random mentions)
- Companies relevant to the user's work or interests
- Concepts or frameworks the user references or creates
- The user's own original thinking (ideas, theses, observations) -- highest value
- See `skills/_brain-filing-rules.md` for the full notability gate

### What to capture from the user's own thinking

Original thinking is the most valuable signal. Capture exact phrasing -- the user's
language IS the insight. Don't paraphrase.

- Novel observations or theses
- Frameworks, mental models, heuristics
- Connections between ideas that others miss
- Contrarian positions with reasoning
- Strong reactions to external stimuli (what triggered it and why)

## Media Workflows

Content the user encounters should be captured in the brain. File by PRIMARY
SUBJECT, not by format (see `skills/_brain-filing-rules.md`).

### Articles & Web Content

**Input:** URL shared by user, or article mentioned in conversation.

**Process:**
1. Fetch content (`web_fetch` or equivalent)
2. Extract: title, author, publication, date, full text
3. Summarize: executive summary + key arguments (not a rehash)
4. Extract entities: people, companies, concepts mentioned
5. **Save raw source** for provenance (see Raw Source Preservation below)
6. Analyze for the user: don't just summarize. What's interesting given what you
   know about them? Flag connections, contradictions, content opportunities.

**Write to:** appropriate directory per filing rules (about a person -> `people/`,
about a company -> `companies/`, reusable framework -> `concepts/`, raw data -> `sources/`)

### PDFs & Documents

**Input:** File path or URL.

**Process:**
1. Extract text with an available local or host tool before writing to gbrain_rs.
2. **Save raw source** for provenance
3. Summarize: executive summary + key sections + notable data
4. Extract entities
5. Cross-reference from entity pages

**Write to:** per filing rules (file by primary subject, not format).

### Meeting Transcripts

**Input:** Transcript from meeting recording service, or manual notes.

**Process:**
1. Pull full transcript (source of truth -- AI summaries are medium-low trust)
2. **Save raw transcript** for provenance
3. Write meeting page with YOUR analysis above the line, raw transcript below
4. **Entity propagation (MANDATORY):** for each attendee and company discussed:
   - Update their brain page State section if new info surfaced
   - Append to their Timeline with link to the meeting page
   - Create page if person/company is notable and has no page yet
5. A meeting is NOT fully ingested until all entity pages are updated

**Write to:** `meetings/YYYY-MM-DD-short-description.md`

**What makes a good meeting page:**
- Reveals the real crux, not a bullet dump
- Connects to existing brain pages (people, companies, deals)
- Flags what changed (status, decisions, new info)
- Names tension or what was left unsaid
- Captures actual dynamic, not performative summary

### Social Media Content

**Input:** Tweet, thread, or social media post.

**Process:**
1. Fetch full content (thread, quote tweets, context)
2. If images present: OCR via vision model for full text extraction
3. Summarize: what's being said, why it matters, who's involved
4. Extract entities and update brain pages
5. Include direct link to the original post (MANDATORY for citations)

**Write to:** `media/x/` for daily aggregation, or entity-specific directories
if the post is primarily about a person/company.

## Raw Source Preservation

Every ingested item must have its raw source preserved for provenance.

**Use `gbrain file upload` for file provenance:**
```bash
gbrain file upload <file> --page <page-slug>
```

- JSON/API payloads can be stored with `put_raw_data`.
- Large cloud-storage routing from original gbrain is not part of gbrain_rs.

**Accessing stored files:**
- `gbrain file list <page-slug>` -- list files attached to a page
- `gbrain file url <storage-path>` -- get the local file path/URL

Use `put_raw_data` in gbrain to store raw API responses and metadata (JSON, not binary).

## Test Before Bulk

When processing multiple items (batch video ingestion, bulk meeting processing, etc.):

1. **Test on 3-5 items first.** Run in test mode if available.
2. **Read the actual output.** Is the quality good? Are titles compelling (not
   "This video discusses...")? Are entities extracted and back-linked? Is the
   format clean?
3. **Fix what's wrong** in the approach/skill, not via one-off patches.
4. **Only then: bulk execute** with throttling, commits every 5-10 items.

The marginal cost of testing 3 items first is near zero. The cost of cleaning
up 100 bad pages is enormous.

## Quality Rules

- Executive summary in compiled_truth must be updated, not just timeline appended
- State section is REWRITTEN, not appended to. Current best understanding only.
- Timeline entries are reverse-chronological (newest first)
- Every person/company mentioned gets a page if notable (see filing rules)
- Link types: knows, works_at, invested_in, founded, met_at, discussed
- Source attribution: every timeline entry includes [Source: ...] citation
- Back-links: every entity mention creates a back-link (Iron Law)
- Filing: file by primary subject, not format or source (see filing rules)

## Anti-Patterns

- **Appending to State sections.** State is rewritten with the current best understanding on every update. Append-only State sections grow stale and contradictory.
- **Ingesting without back-links.** An unlinked mention is a broken brain. Every entity mentioned must have a back-link from their page to the page mentioning them.
- **Skipping raw source preservation.** Every ingested item must have its raw source preserved. A brain page without provenance is unverifiable.
- **Bulk processing without sample test.** Test on 3-5 items first. Fix quality issues in the approach, not via one-off patches.
- **Paraphrasing the user's original thinking.** The user's exact language IS the insight. Capture verbatim phrasing for ideas, theses, and frameworks.

## Output Format

```
INGESTED: [title]
==================

Page: [slug]
Type: [person / company / meeting / media / concept]
Source: [source description]

Entities detected: N
- [entity] -> [created / updated] ([slug])

Back-links created: N
Timeline entries: N
Raw source: [preserved at path / uploaded to cloud]
```

## Tools Used

- Read a page from gbrain (get_page)
- Store/update a page in gbrain (put_page)
- Add a timeline entry in gbrain (add_timeline_entry)
- Link entities in gbrain (add_link)
- List tags for a page (get_tags)
- Tag a page in gbrain (add_tag)
- Store raw data in gbrain (put_raw_data)
- Check backlinks in gbrain (get_backlinks)
