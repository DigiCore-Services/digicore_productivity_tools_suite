# KMS Link Graph & Bi-directional Logic (Phase 79)

- [x] Backend: Link Parsing & Storage
    - [x] Implement `upsert_link` and `delete_links_for_source` in `kms_repository.rs`
    - [x] Add regex-based link extraction to `api.rs` indexing logic
    - [x] Implement title-to-path resolution using `kms_notes` table
- [x] Backend: Link Refactoring (Rename Logic)
    - [x] Update `kms_rename_note` to scan and update `[[links]]` in referencing notes
    - [x] Add `update_links_on_path_change` to `kms_repository.rs`
    - [x] Implement safe search-and-replace for Markdown links (supporting aliases)
- [x] Backend: API Extensions
    - [x] Add `KmsLinksDto` to `api.rs`
    - [x] Implement `kms_get_note_links` in `ApiImpl`
- [x] Frontend: Bi-directional View
    - [x] Update `KmsEditor.tsx` to fetch and display backlinks
    - [x] Implement navigation for backlink clicks
- [x] Verification
    - [x] Add unit test for Markdown link parser and refactor logic
    - [x] Manually verify rename refactoring preserves aliases
    - [x] Verify graph table consistency after renames

## Phase 80: Backlink Fixes & Sync Robustness
- [x] Backend: Enhanced Link Extraction
    - [x] Update `extract_links_from_markdown` for `[text](path)`
    - [x] Implement path-based link resolution (relative/anchors)
- [x] Backend: Sync Robustness
    - [x] Add file modification time checks to `sync_vault_files_to_db_internal`
- [x] Performance: Resolve Saving Hangups
    - [x] Offload embedding generation to `spawn_blocking`
- [x] Verification
    - [x] Add tests for link extraction/resolution
    - [x] Manually verify external change detection
    - [ ] Verify "Saving..." no longer hangs UI

## Phase 81: Slash Normalization & Backlink Retrieval Fixes
- [x] Normalize `ApiImpl::get_relative_path` to forward slashes
- [x] Normalize `ApiImpl::resolve_absolute_path` input handling
- [x] Audit `kms_save_note`, `kms_rename_note`, and `kms_delete_note` for slash consistency
- [ ] Verify backlink retrieval in UI after normalization

## Phase 82: Restoring Indexing Controls & Auto-Refresh
- [x] Backend: Robust `kms_get_indexing_status`
    - [x] Handle uninitialized repository without failing RPC
- [x] Frontend: Proactive Status Fetching
    - [x] Fetch indexing status on `semantic_search` tab entry
    - [x] Implement 5s auto-refresh while `semantic_search` tab is active
- [x] Verification
    - [x] Cold start test: Verify boxes appear after delay
    - [x] Auto-refresh test: Verify counts update after saving note

## Phase 83: Startup Reconciliation & Runtime Robustness
- [x] Implement automatic KMS vault sync on application startup in `lib.rs`
- [x] Ensure KMS filesystem watcher starts on boot if vault path exists
- [x] Verify reconciliation of external file changes (add/edit/delete) during app-off time
- [x] Audit `delete_note` logic for case-sensitivity and secondary index cleanup

## Phase 84: Deletion Robustness Audit & Hardening [x]

- [x] Implement bulk deletion methods in `kms_repository.rs`
- [x] Modify `clipboard_repository::trim_to_depth` to return deleted IDs
- [x] Update clipboard deletion commands in `api.rs` to clean up search index
- [x] Verify search index consistency after deletion, clear-all, and trimming operations

- [x] Audit and analyze existing flat Explorer implementation
- [x] Generate standalone implementation plan for hierarchical navigation
- [x] Implement recursive `KmsFileSystemItemDto` and `kms_get_vault_structure` command
- [x] Build hierarchical `FileExplorer` component in React
- [x] Implement folder management actions (New Folder, Rename, Delete)
- [x] Verify hierarchical views and refactoring impact

## Phase 86: Advanced Explorer Management & UX [x]

- [x] Backend: Implement recursive `kms_rename_folder` (bulk path/backlink update)
- [x] Backend: Implement recursive `kms_delete_folder` (cleanup DB & FS)
- [x] Backend: Implement `kms_move_item` for Drag & Drop organization
- [x] Frontend: Implement resizable sidebar with persistence in `KmsApp.tsx`
- [x] Frontend: Implement Drag & Drop (files/folders) in `FileExplorer.tsx`
- [x] Verification: Test bulk renaming and recursive deletion safety

## Phase 87: Mermaid Diagram Rendering [x]

- [x] Research: Identify markdown rendering component and current support
- [x] Planning: Create implementation plan for Mermaid integration
- [x] Execution: Install `mermaid` dependency
- [x] Execution: Implement custom Tiptap Mermaid extension
- [x] Execution: Integrate Mermaid extension into `KmsEditor.tsx`
- [x] Verification: Test rendering of various Mermaid diagram types
## Phase 88: KMS Robustness & Architectural Refactoring [/]

- [x] Planning: Create detailed implementation plan for Phase 88
- [x] Refactor: Implement `KmsError` enum for structured error handling
- [x] Refactor: Extract domain logic from `api.rs` to `kms_service.rs`
- [x] Database: Add `kms_logs` table for diagnostic history
- [x] Service: Implement `KmsDiagnosticService` for persistent logging
- [x] UI: Implement "Sync Status" indicator and log viewer
- [x] Verification: Ensure all KMS operations use structured errors and log to the new system
## Phase 89: Search & Discovery UX Enhancement [x]

- [x] Planning: Create implementation plan for Search UX enhancements
- [x] Backend: Update `SearchResultDto` with `snippet` field in `api.rs` & `bindings.ts`
- [x] Phase 89: KMS Search & Discovery UX
    - [x] Implement backend: contextual search snippets (KmsService::extract_contextual_snippet)
    - [x] Integrate snippet extraction into `kms_search_semantic` in `api.rs`
    - [x] Update frontend: display snippets in search results (KmsApp.tsx)
    - [x] Resolve build errors (thiserror dependency, error mapping)
- [x] Frontend: Implement "Quick Peek" preview on hover/focus
- [x] Verification: Test snippet accuracy and UI rendering
## Phase 90: Search Accuracy & Hybrid Search Improvements [x]
- [x] Research/Audit existing FTS and Hybrid search bottlenecks
- [x] Implementation Plan for Unified FTS and Search Modes
- [x] Implement Unified FTS (index snippets and clipboard in FTS)
- [x] Refine Hybrid Search ranking (soften AND, adjust RRF, boost titles)
- [x] Add Search Mode selection to UI (Hybrid, Semantic, Keyword)
- [x] Verify improved ranking with broad queries

## Phase 92: UI Reorganization - Management Console
- [x] Analyze `KmsApp.tsx` tab implementation
- [x] Move 'Appearance', 'Statistics', and 'Log' into 'Configurations and Settings' sub-tabs
- [x] Reorder main tabs (move 'Configurations and Settings' to last)
- [x] Verify tab navigation and state persistence
- [x] Clean up redundant UI elements

## Phase 91: Text Expansion & Clipboard Feature Audit [x]
- [x] Research current implementation of Text Expansion and Copy-to-Clipboard
- [x] Analyze architecture against Hexagonal/SOLID/SRP principles
- [x] Identify gaps and opportunities for "robust" and "feature-rich" enhancements
- [x] Generate comprehensive Audit & Implementation Plan document
- [x] Present findings to USER for review
## Phase 93: Debugging KMS Search Content Display [/]
- [x] Research: Trace `kms_search_semantic` result metadata for Clipboard/Snippets
- [x] Backend: Ensure `metadata` and `snippet` fields are correctly populated for all types
- [x] Frontend: Fix content parsing in `KmsApp.tsx` and `ViewFull` modal
- [x] Verification: Test search result display and modal content for all entity types
## Phase 94: KMS Image Search Integration [/]
- [x] Backend: Expose `get_clip_entry_by_id` in `api.rs`
- [x] Frontend: Import and integrate `ImageViewerModal` in `KmsApp.tsx`
- [x] Frontend: Update `handleNavigateToResult` to launch image viewer for image results
- [x] Verification: Test clicking image search results launches the image viewer
## Phase 94.1: Image Viewer Resolution Fixes [x]
- [x] Backend: Resolve parent images for `extracted_text` in `get_clip_entry_by_id`
- [x] Frontend: Fetch full entry in `handleNavigateToResult` to ensure correct modal triggers
- [x] Backend: Update indexing service to include correct metadata types
- [x] Backend: Resolve parent images for `extracted_text` in `get_clip_entry_by_id`
- [x] Frontend: Fetch full entry in `handleNavigateToResult` to ensure correct modal triggers
- [x] Backend: Update indexing service to include correct metadata types
## Phase 94.2: Distinct Result Labeling [x]
- [x] Backend: Refactor `search_hybrid` to return hit modality
- [x] Frontend: Display modality badges in search results
- [x] Verification: Test that OCR and Image hits are distinct and clearly labeled

## Phase 95: Git Organization & Deployment [/]
- [x] Staging and committing Group 1: Adaptive OCR & Clipboard Metadata
- [x] Staging and committing Group 2: KMS Search Modality & Navigation Polish
- [x] Staging and committing Group 3: KMS Editor & UI Consistency
- [x] Pushing final changes to remote repository

## Phase 96: Final Assets Deployment [/]
- [/] Staging and committing documentation and test snapshots
- [ ] Pushing final assets to remote repository
