# Unified Multi-Modal Semantic Search (Phase 78)

Implement a truly unified semantic search that indexes not just KMS Notes, but also Snippet Library content and Clipboard History (including images via CLIP embeddings). The design follows **Hexagonal Architecture** and **SOLID** principles to ensure future extensibility for media attachments, PDFs, audio, and video.

## Architectural Strategy
- **Indexing Service (Domain/Service Layer)**: A centralized `KmsIndexingService` that manages the orchestration of indexing tasks.
- **Indexable Provider (Trait/Interface)**: Define a `SemanticIndexProvider` trait that different components (Notes, Snippets, Clipboard, Media) implement.
- **Adapter-Based Storage**: The `kms_repository` acts as the persistence adapter for vector storage.

---

### Backend (Rust)

#### [NEW] [indexing_service.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/indexing_service.rs)
- **`SemanticIndexProvider` Trait**:
    ```rust
    pub trait SemanticIndexProvider: Send + Sync {
        fn provider_id(&self) -> &str;
        async fn index_all(&self, app: &tauri::AppHandle) -> Result<usize, String>;
        async fn index_item(&self, app: &tauri::AppHandle, entity_id: &str) -> Result<(), String>;
    }
    ```
- **Implementations**:
    - `NoteIndexProvider`: Current note-sync logic refactored into this trait.
    - `SnippetIndexProvider`: New logic for indexing snippets from `AppState`.
    - `ClipboardIndexProvider`: Logic for indexing text and images from clipboard history.
- **`KmsIndexingService`**: Registry of providers that handles "Reindex All" or specific type reindexing.

#### [MODIFY] [api.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/api.rs)
- **`kms_reindex_all`**: Delegate to `KmsIndexingService::index_all_providers()`.
- **`kms_reindex_type(type: String)` [NEW]**: Reindex specific categories (e.g., "notes", "snippets", "clipboard", "images").
- **`kms_get_indexing_status` [NEW]**: Return counts and status for each provider.

#### [MODIFY] [kms_repository.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/kms_repository.rs)
- Ensure generic support for `entity_type` and `modality` in search and storage.

---

### Frontend (React)

#### [MODIFY] [ConfigTab.tsx](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/components/ConfigTab.tsx)
- Add a new "Semantic Search" sub-tab in the configuration view.
- **Dynamic Reindex Grid**: Render reindex buttons based on a list of available types (future-proofed for Audio/Video/PDF).
- Buttons for: "All", "Notes", "Snippets", "Clipboard", "Media/Images".
- Display per-type index counts and last-indexed timestamps.

#### [MODIFY] [KmsApp.tsx](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/KmsApp.tsx)
- **`handleNavigateToResult`**:
    - Handle `entity_type === "snippet"`: Show snippet details or provide a "Copy to Clipboard" action.
    - Handle `entity_type === "clipboard"`: Show content/image and provide "Copy" action.

---

## Verification Plan

### Automated Tests
- Run `cargo test` in `src-tauri` to ensure no regressions in existing search logic.
- Potential new test in `kms_repository.rs` to verify mixed-type results.

### Manual Verification
1.  **Snippet Search**:
    - Create a unique snippet with a specific keyword.
    - Run "Reindex Snippets" from the Config tab.
    - Search for the keyword in the KMS Search view and verify the snippet appears as a result.
2.  **Clipboard Image Search**:
    - Copy an image (e.g., a photo of a cat).
    - Ensure it's saved in clipboard history.
    - Run "Reindex Clipboard".
    - Search for "cat" in KMS Search and verify the clipboard entry appears.
3.  **Cross-Platform Search**:
    - Verify that the "Similar Content" sidebar in the Note Editor also shows relevant snippets or clipboard items related to the current note content.
