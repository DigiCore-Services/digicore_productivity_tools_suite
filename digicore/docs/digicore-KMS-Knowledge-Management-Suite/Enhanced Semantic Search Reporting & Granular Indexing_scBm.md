# Enhanced Semantic Search Reporting & Granular Indexing

Provide detailed visibility into the indexing status of your knowledge base (Notes, Snippets, Clipboard) and add controls to resolve failures.

## 1. KMS Storage & Data Management

### Location
The "KMS" data is stored in the unified application database:
- **Exact Path**: `%APPDATA%\com.digicore.text-expander\digicore.db`
- **Engine**: SQLite with `sqlite-vec` (vector extension) and `FTS5` (full-text search).

### Tables
| Table Name | Purpose |
|------------|---------|
| `kms_notes` | Metadata, titles, and `sync_status` for local markdown notes. |
| `kms_vector_map` | Central registry mapping all entities (Notes, Snippets, etc.) to their vector IDs. |
| `kms_embeddings_text` | **Virtual (sqlite-vec)**: Stores high-dimensional text embeddings. |
| `kms_embeddings_image` | **Virtual (sqlite-vec)**: Stores high-dimensional image embeddings. |
| `kms_notes_fts` | **Virtual (FTS5)**: Enables rapid full-text search across note contents. |
| `kms_index_status` | **[NEW]**: Tracks indexing status (success/failed/pending) and error messages for all types. |

---

## 2. Proposed Technical Changes

### Backend (Rust)

#### [NEW] Database Table
Create `kms_index_status` to track granular status for non-note entities (Snippets, Clipboard) which don't have dedicated metadata tables.
```sql
CREATE TABLE IF NOT EXISTS kms_index_status (
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    status TEXT NOT NULL, -- 'indexed', 'failed', 'pending'
    error TEXT,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (entity_type, entity_id)
);
```

#### [MODIFY] [api.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/api.rs)
- Update `IndexingStatusDto` to include `indexed`, `failed`, `total`, and `last_error`.
- Enhance `kms_get_indexing_status` to query both `kms_notes` and `kms_index_status`.
- Add `kms_retry_failed(provider_id: String)` command.

#### [MODIFY] [indexing_service.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/indexing_service.rs)
- Update `SemanticIndexProvider` trait to report indexing results.
- Update `SnippetIndexProvider` and `ClipboardIndexProvider` to record failures in `kms_index_status`.

### Frontend (React/Tauri)

#### [MODIFY] [ConfigTab.tsx](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/components/ConfigTab.tsx)
- **Enhanced Status Grid**: Show `Indexed`, `Failed`, and `Total` for each category (Notes, Snippets, Clipboard).
- **Failure Drill-down**:
    - Add a "View Failures" button for each category that opens a modal or expandable list.
    - List specific items (e.g., Note title/path, Snippet trigger, Clipboard timestamp) that failed.
    - Show the specific error message for each failure (e.g., "Timeout", "OOM", "Invalid format").
- **Granular Controls**:
    - **"Retry Failed"**: A single button to re-run indexing only for items marked as `failed`.
    - **"Individual Retry"**: Button next to each item in the Failure List.
    - **"Global Reindex"**: (Existing) Wipes and restarts everything.
- **Success View**: Optionally allow viewing all `indexed` items to verify what's currently in the vector cache.

---

## 3. Verification Plan

### Automated Tests
- `cargo check` for API consistency.
- New unit tests for `kms_repository::get_detailed_status`.

### Manual Verification
- Deliberately cause a failure (e.g., mock an embedding service timeout).
- Verify the "Failed" count increases in the Semantic Search tab.
- Click "Retry Failed" and verify it attempts to re-index only the failed items.
