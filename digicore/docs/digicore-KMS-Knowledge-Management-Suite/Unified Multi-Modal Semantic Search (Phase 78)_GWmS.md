# Unified Multi-Modal Semantic Search (Phase 78)

Implement a truly unified semantic search that indexes not just KMS Notes, but also Snippet Library content and Clipboard History (including images via CLIP embeddings). The design follows **Hexagonal Architecture** and **SOLID** principles to ensure future extensibility for media attachments, PDFs, audio, and video.

## Architectural Strategy
- **Indexing Service (Domain/Service Layer)**: A centralized `KmsIndexingService` that manages the orchestration of indexing tasks.
- **Indexable Provider (Trait/Interface)**: Define a `SemanticIndexProvider` trait that different components (Notes, Snippets, Clipboard, Media) implement.
- **Adapter-Based Storage**: The `kms_repository` acts as the persistence adapter for vector storage.

---

## Proposed Changes

### Backend (Rust)

#### [x] [indexing_service.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/indexing_service.rs)
- **`SemanticIndexProvider` Trait**: Abstract interface for data sources.
- **Implementations**: `NoteIndexProvider`, `SnippetIndexProvider`, `ClipboardIndexProvider`.
- **`KmsIndexingService`**: Orchestrator for global and partial reindexing.

#### [/] [api.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/api.rs)
- [x] **`kms_reindex_all`**: Delegates to `KmsIndexingService`.
- [x] **`kms_reindex_type`**: Targeted indexing for specific providers.
- [x] **`kms_get_indexing_status`**: Provides counts per index type.
- [ ] **Verification**: Ensure all commands are correctly exposed via TauRPC.

#### [x] [kms_repository.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/kms_repository.rs)
- Verified `upsert_embedding` handles multi-modal data and entity mapping.

---

### Frontend (React)

#### [/] [ConfigTab.tsx](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/components/ConfigTab.tsx)
- [x] Add "Semantic Search" tab to configuration menu.
- [x] Implement reindex buttons for Notes, Snippets, and Clipboard.
- [x] Add real-time indexing status display.
- [x] Support global "Reindex All" action.

#### [ ] [KmsApp.tsx](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/KmsApp.tsx)
- **Result Rendering**: Update search UI to distinguish between Notes, Snippets, and Clipboard items (iconography).
- **Actions**: Add specific actions for non-note results (e.g., "Copy Snippet", "View Clipboard Image").

---

## Verification Plan

### Automated Tests
- [ ] Run `cargo check` to verify Rust compilation and TauRPC bindings.
- [ ] Implement unit tests for `ClipboardIndexProvider` logic.

### Manual Verification
1.  **Reindex Control**: Go to Settings > Semantic Search and trigger a "Reindex Snippets" task. Verify the count updates.
2.  **Multimodal Search**: Copy an image, reindex clipboard, and search for visual content descriptions.
3.  **Note Sync**: Verify that standard note indexing still works after refactoring.
