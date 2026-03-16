# Phase 90: Unified KMS Search Accuracy

Improve search relevance by indexing all entity types (Notes, Snippets, Clipboard) in Full Text Search (FTS5) and providing user-selectable search modes.

## User Review Required

> [!IMPORTANT]
> - New Unified FTS Table: Snippets and clipboard history will now be searchable via keywords, not just conceptually (semantic).
> - Ranking Logic: Switching from strict `AND` to `OR` for multi-word queries to prevent 0-result cases on broad natural language queries.
> - Search Modes: Introducing a UI control to switch between **Hybrid** (default), **Semantic** (conceptual), and **Keyword** (exact word match).

## Proposed Changes

### Database Migration
#### [MODIFY] [lib.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/lib.rs)
- **Add Migration 8**:
```sql
-- Create a unified FTS table that doesn't depend on a single content table
CREATE VIRTUAL TABLE kms_unified_fts USING fts5(
    entity_type UNINDEXED,
    entity_id UNINDEXED,
    title,
    content,
    tokenize='porter'
);

-- Trigger to sync notes to the unified FTS table
CREATE TRIGGER IF NOT EXISTS kms_notes_sync_fts AFTER INSERT ON kms_notes
BEGIN
    INSERT INTO kms_unified_fts (entity_type, entity_id, title, content)
    VALUES ('note', new.path, new.title, new.content_preview);
END;

-- (+ UPDATE and DELETE triggers)
```

### Backend: Indexing Service
#### [MODIFY] [indexing_service.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/indexing_service.rs)
- Update `SnippetIndexProvider` and `ClipboardIndexProvider` to manually upsert into `kms_unified_fts` when indexing.
- Update `NoteIndexProvider` to backfill existing notes into `kms_unified_fts`.

### Backend: Repository Layer
#### [MODIFY] [kms_repository.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/kms_repository.rs)
- Rename/Refactor `semantic_search` to `kms_search` supporting a `mode` parameter.
- **Improved FTS Query**: Split query into words and use `OR` for broader recall, or a mix of `AND` for adjacent words and `OR` for others.
- **RRF Weighting**: Ensure FTS matches give a strong enough signal to "rescue" keyword-relevant results that semantic search might bury.

### API Layer
#### [MODIFY] [api.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/api.rs)
- Update `kms_search_semantic` to accept `search_mode: Option<String>`.
- Pass this mode to `kms_repository`.

### Frontend
#### [MODIFY] [KmsApp.tsx](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/KmsApp.tsx)
- Add a segmented control or dropdown next to the search bar for: `Hybrid`, `Semantic`, `Keyword`.
- Bind to a local state `searchMode`.
- **Note**: Display "Match %" more prominently for top results.

## Verification Plan

### Automated Verification
- No automated search tests currently exist. I will create a temporary console-based verification script in Rust or simply rely on manual verification via the UI as it's the most direct way to check "relevance feel".

### Manual Verification
1.  **Baseline Query**: Search for "document about Architecture".
    - **Current**: Note ranks #4 or lower.
    - **Expected**: Note ranks #1 (due to keyword "Architecture" in title/content).
2.  **Snippet Keyword**: Search for a unique word in a snippet (e.g., "/zsep"). Ensure it ranks high in **Keyword** and **Hybrid** modes.
3.  **Broad Concept**: Search for "how to generate images". Ensure relevant snippets/notes appear even if literal words are missing (**Semantic** mode).
4.  **Mode Toggle**: Verify that toggling to **Semantic** removes keyword-only "noise" and toggling to **Keyword** removes conceptual but word-unrelated results.
