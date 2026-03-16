# Phase 78b: KMS Hybrid Search Setup (FTS5 + Vector AI)

I have successfully designed and implemented an industry-standard **Hybrid Search Engine** for our Knowledge Management Suite. Instead of forcing the user to rely entirely on Semantic Vector search (which is great for conceptual recall, but struggles with exact acronyms like `AHK`), we're now fusing Vector search with lexical SQLite FTS5.

## ✨ Key Implementations

### 1. SQLite FTS5 Migration
- Created **Migration 6** inside `tauri-plugin-sql` setup in `lib.rs`.
- Provisioned the `kms_notes_fts` virtual table, mapped directly against the exact `title` and `content_preview` columns of your `kms_notes`.
- Built an **automated trigger pipeline** using `AFTER INSERT`, `UPDATE`, and `DELETE`. The FTS5 text index seamlessly remains in-sync with standard CRUD actions without manual Rust backend logic!

### 2. Reciprocal Rank Fusion (RRF) Algorithm
- Rewrote the `semantic_search` endpoint inside `kms_repository.rs` to intelligently run *both* searching mechanisms on a given text string.
- First grabs context-aware hits from the `sqlite-vec` index and assigns mathematical "ranks".
- Next, issues a high-performance `BM25()` lexical keyword query against `kms_notes_fts` for direct term match hits.
- Evaluates the top hits from both sources using standard **Reciprocal Rank Fusion** (`RRF = 1.0 / (K * rank)` where `K` = 60).
- Standardizes the merged scores into the `distance` (lower is better) expected by the React frontend components.

### 3. Dark Mode CSS Fidelity
- Addressed the white-on-white text readability in the UI. Explicitly bounded the input component with `bg-dc-bg-secondary text-dc-text` in `KmsApp.tsx`. Extraneous visual bugs when switching from Desktop themes are now resolved.

### 4. Async Stability and Search Cancellation
- **Fixed the infinite "Thinking..." hang**: In `api.rs`, the heavy ML embedding generation and synchronous SQLite `FTS5` engine queries were blocking the Tauri `tokio` async workers. Wrapped the search endpoint natively inside `tokio::task::spawn_blocking` to decouple heavy lifting from the main app thread.
- **Frontend Cancellation Integration**: Rebuilt the search handler in `KmsApp.tsx` with a `React.useRef<AbortController>()` pipeline. A "Cancel Search" button instantly aborts the UI loading state and cleanly silences any delayed frontend resolution of the blocked thread.

### 5. Semantic Vector and FTS5 Retroactive Backfill Bug
- **FTS5 Existing Note Fix**: Identified that Migration 6 only added indexing triggers for *new* inserts/updates, leaving existing notes unsearchable. Added **Migration 7** to retroactively backfill the `kms_notes_fts` table with any pre-existing database records.
- **On-Save Vector Generation**: Discovered that clicking "Save" on a note bypassed vector generation entirely! Refactored the `kms_save_note` endpoint in `api.rs` to automatically generate semantic vectors and insert them into the `sqlite-vec` map dynamically on every save.

### 6. Tauri Window Capabilities and Search Engine Syntax
- **"kms" event.listen Authorization**: Identified the root cause of the unexpected Save action crash. The "kms" native window spawned by the frontend lacked Tauri `allow-listen` capabilities to broadcast global theme/save events. Added "kms" to the `capabilities/default.json` manifest.
- **FTS5 bm25() Syntactic Error**: Addressed an edge-case syntax error where calling `bm25()` upon `kms_notes_fts` without its `f` table alias silently failed to return matches. **Status: REVERTED TO FULL NAME (Confirmed Fix).**
- **FTS5 Soft Phrase Matching**: Updated the FTS5 query logic to split user queries by alphanumeric characters and inject `AND` between tokens, rather than strictly injecting exact `MATCH '"phrase"'` quotes which inadvertently prevented multi-word note searches.

## ⚠️ Database Corruption Recovery
If you see "database disk image is malformed" or if KMS tables are missing from your DB Browser:
1. **Click the "Repair KMS Index" button** in the Vault Settings (bottom left of the KMS window).
2. This will surgically drop only the KMS-related tables and reset the migration history for versions 4-7. **Your Snippets and Clipboard history will NOT be affected.**
3. **RESTART the app** (Ctrl-C and run `npm run tauri dev`).
4. On startup, the app will automatically recreate the fresh KMS tables in your existing `digicore.db`.

> [!NOTE]
> If you don't see `kms_notes` in your DB Browser yet, it's likely because the "malformed" error prevented the schema from fully loading, or the migrations are currently stuck. The "Repair" button is designed to force-clean this state.

## 🧪 Verification Plan
The Rust backend is entirely compiling properly and the frontend logic gracefully handles our adjusted schema! Ensure you **RESTART** your Tauri server altogether `(Ctrl-C)` to reload the new JSON capabilities manifest.
