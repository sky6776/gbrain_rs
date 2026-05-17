---
name: kb-ops
version: 1.0.0
description: |
  Knowledge Base (KB) subsystem lifecycle management. **INTERNAL** — requires
  expose_internal_tools=true. For user-facing operations, use the artifact-review
  skill (artifact_upload, artifact_query, etc.) instead.
triggers:
  - "create knowledge base"
  - "upload document to kb"
  - "kb search"
  - "kb health"
  - "kb backup"
  - "kb library"
  - "knowledge base"
tools:
  - kb_list_libraries
  - kb_create_library
  - kb_update_library
  - kb_delete_library
  - kb_upload_document
  - kb_get_document_status
  - kb_retry_document
  - kb_cancel_document_job
  - kb_delete_document
  - kb_list_documents
  - kb_search
  - kb_create_folder
  - kb_purge_document
  - kb_check_index_health
  - kb_repair_index
  - kb_backup
  - kb_restore
  - kb_add_eval_query
  - kb_add_search_feedback
mutating: true
writes_pages: false
---

# KB Operations Skill

Manage the Knowledge Base (KB) subsystem — a separate document processing and
search engine layered on top of the gbrain core.

## Contract

This skill guarantees:
- KB libraries are created with appropriate governance settings before documents are uploaded
- Document processing status is checked after upload before assuming success
- Search uses the correct profile for the task (fast for quick lookups, accurate for research)
- Failed documents are retried or cancelled, never left in limbo
- Index health is monitored and repaired when degraded
- Backups are taken before destructive operations

## KB vs Brain

The brain stores curated, human-authored pages. The KB stores processed documents
(PDFs, DOCX, etc.) with automatic chunking, embedding, and RAPTOR summarization.
Use the brain for entities and relationships; use the KB for document evidence and
raw source material.

## Phases

### Phase 1: Library Setup

Before uploading any document, create a library with appropriate configuration:

1. **Create library** with governance params:
   - `embedding_provider` / `embedding_model` / `embedding_dimensions` — control which model processes chunks
   - `rerank_enabled` / `rerank_provider` — enable second-pass relevance ranking
   - `raptor_enabled` / `raptor_llm_model` — enable tree summarization for deep retrieval
   - `redaction_enabled` — strip sensitive content before indexing
   - `external_*_allowed` — gate external API calls for embedding, rerank, summary, OCR
   - `semantic_segmentation_enabled` — use LLM-based chunking instead of fixed-size splits
   - `chunk_size` / `chunk_overlap` — tune fixed-size chunking parameters

2. **Verify creation** — list libraries to confirm the new library appears.

### Phase 2: Document Processing

1. **Upload document** — `kb_upload_document` with `library_id` and `file_path`.
   Optional: specify `folder_id` for organization.
2. **Check status** — `kb_get_document_status` to verify processing completed.
   Status values: `pending`, `chunking`, `embedding`, `summarizing`, `ready`, `failed`.
3. **Handle failures** — `kb_retry_document` for transient errors, `kb_cancel_document_job`
   for stuck jobs, `kb_delete_document` (with `confirm=true`) for irrecoverable documents.

### Phase 3: Search

KB search uses hybrid vector + keyword + RRF fusion with optional reranking.

**Search profiles** (choose based on task):

| Profile | Use for | Behavior |
|---------|---------|----------|
| `fast` | Quick lookups, autocomplete | Keyword-only, no rerank |
| `balanced` | General search (default) | Vector + keyword + RRF |
| `accurate` | Research, deep analysis | Vector + keyword + rerank + context |
| `file_lookup` | Find a specific document | Filename/title matching |
| `table` | Extract tabular data | Table-aware chunk retrieval |

**Context options** for `accurate` profile:
- `include_context=true` + `context_before/after` — return surrounding text around matches
- `include_highlights=true` — return character ranges for highlighting
- `group_by_document=true` — cluster results by source document
- `embedding_index_id` — use a specific embedding index for cross-model queries

### Phase 4: Maintenance

1. **Health check** — `kb_check_index_health` detects orphan nodes, missing FTS entries,
   embedding mismatches, and split inconsistencies.
2. **Repair** — `kb_repair_index` fixes missing FTS entries.
3. **Backup** — `kb_backup` writes the KB database to an output directory.
4. **Restore** — `kb_restore` reads from a backup directory.
5. **Purge** — `kb_purge_document` permanently destroys a soft-deleted document
   (requires `confirm=true`).

### Phase 5: Quality Evaluation

1. **Add eval queries** — `kb_add_eval_query` with expected document IDs to measure
   search recall and precision.
2. **Submit feedback** — `kb_add_search_feedback` with rating (0-5) for individual
   search results to improve future ranking.

## Anti-Patterns

- **Uploading documents without checking processing status.** A document in `failed`
  state is invisible to search. Always check after upload.
- **Using `fast` profile for research queries.** Fast skips vector search and reranking,
  missing semantically similar results.
- **Creating libraries without governance params.** Default settings may not match
  your embedding provider or security requirements.
- **Skipping health checks.** Orphan embeddings and missing FTS entries silently
  degrade search quality.
- **Not backing up before destructive operations.** `kb_purge_document` is irreversible.

## Tools Used

- `kb_list_libraries` — list all KB libraries
- `kb_create_library` — create a new library with governance config
- `kb_update_library` — update library configuration
- `kb_delete_library` — delete a library (requires confirm)
- `kb_upload_document` — upload a file for processing
- `kb_get_document_status` — check document processing status
- `kb_retry_document` — retry a failed document
- `kb_cancel_document_job` — cancel a stuck job
- `kb_delete_document` — soft-delete a document (requires confirm)
- `kb_list_documents` — list documents in a library
- `kb_search` — hybrid search with profiles and context
- `kb_create_folder` — organize documents in folders
- `kb_purge_document` — permanently destroy a document
- `kb_check_index_health` — diagnose index issues
- `kb_repair_index` — fix missing FTS entries
- `kb_backup` — export KB database
- `kb_restore` — import KB database
- `kb_add_eval_query` — add evaluation queries
- `kb_add_search_feedback` — submit relevance feedback