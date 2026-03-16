# Phase 78b: KMS Hybrid Search Plan

## Goal Description
The objective is to implement a robust, industry-standard **Hybrid Search** mechanism for the KMS application that combines exact/partial keyword matching (Full-Text Search) with conceptual semantic matching (Vector Search). We will accomplish this by adding SQLite FTS5 for text search and combining its results with `sqlite-vec` embeddings using Reciprocal Rank Fusion (RRF) in Rust.

## Proposed Changes

### Database Adjustments
#### [MODIFY] lib.rs
- Add **Migration 6** to `tauri-plugin-sql` setup.
- Create an FTS5 virtual table `kms_notes_fts` indexing the `title` and `content_preview` columns.
- Create `AFTER INSERT`, `AFTER UPDATE`, and `AFTER DELETE` triggers on the original `kms_notes` table to ensure the FTS index stays perfectly synchronized without manual intervention.

### Backend Search Refinement
#### [MODIFY] kms_repository.rs
- Refactor `semantic_search` to perform a genuine hybrid query when the modality is "text":
  1. Execute the vector search via `sqlite-vec` to get a list ranked by cosine distance.
  2. Execute an FTS5 `MATCH` query on the `kms_notes_fts` table, using `bm25()` for lexical scoring, to get a list ranked by relevance.
  3. Merge both ranked lists in Rust using **Reciprocal Rank Fusion (RRF)**:
     `RRF_Score = (1 / (K + rank_vector)) + (1 / (K + rank_fts))` (Standard K=60).
  4. Scale the final score to a `0..1` similarity metric and convert it into a "distance" value (`1 - similarity`) to remain seamlessly compatible with the frontend's expected formatting.

#### [MODIFY] api.rs
- Ensure vector generation seamlessly continues to work with the hybrid query function in `kms_repository`.

## Verification Plan

### Automated Tests
- Pre-flight `cargo check` to guarantee the Rust logic builds cleanly with no borrow warnings or typing errors in `kms_repository.rs`.

### Manual Verification
1. Launch the `tauri dev` environment. Migrations will run instantly to create the FTS table and index existing notes (since the trigger fires on insert/update). *We may need to run a manual script or command to populate the FTS table with any existing pre-Migration-6 notes.*
2. Enter the Knowledge Search view in the KMS App.
3. Test **Lexical matching**: Search for an exact jargon term or weirdly spelled name that exists in a note (e.g. "AHK_AutoHotKey"). It should jump to the top due to the FTS BM25 score.
4. Test **Semantic matching**: Search for a concept loosely related to the note (e.g. "automation tools"). It should still surface the note based on the vector score.
5. In dark mode, verify that the search input background explicitly uses `bg-dc-bg-secondary` and text is legible.
