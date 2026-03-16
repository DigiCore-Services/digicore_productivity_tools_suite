# Git Organization & Deployment Plan

This plan outlines the logical grouping and commitment of the recent KMS and OCR improvements, followed by a clean push to the remote repository.

## Grouped Changes

### 1. Adaptive OCR & Clipboard Metadata
- **Modified Files**: 
    - [sqlite_clipboard_history_adapter.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/src/adapters/clipboard/sqlite_clipboard_history_adapter.rs)
    - [types.ts](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/types.ts)
    - [bindings.ts](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/bindings.ts)
- **Description**: Introduces `extraction_adaptive_table_cross_factor` and other adaptive OCR parameters to the configuration and database layers. Enhances clipboard history querying to handle these new metadata fields.

### 2. KMS Search Modality & Navigation Polish
- **Modified Files**: 
    - [kms_repository.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/kms_repository.rs)
    - [api.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/api.rs)
    - [KmsApp.tsx](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/KmsApp.tsx)
- **Description**: Finalizes the `SearchResult` modality logic. Ensures search results strictly distinguish between "image" matches and "text" (OCR) matches. Refines the frontend `handleNavigateToResult` to trigger the correct modal (Image Viewer vs Text Modal) based on the hit's modality.

### 3. KMS Editor & UI Consistency
- **Modified Files**: 
    - [KmsEditor.tsx](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/components/kms/KmsEditor.tsx)
- **Description**: Polishes the KMS Editor's backlink and diagram rendering to ensure UI consistency with the new navigation patterns.

## Deployment Steps

1. **Commit Group 1**: `feat(ocr): introduce adaptive extraction parameters and metadata expansion`
2. **Commit Group 2**: `feat(kms): refine search modality and navigation logic`
3. **Commit Group 3**: `feat(ui): polish KMS editor backlink and diagram rendering`
4. **Push**: Execute `git push origin feature/ui-decoupling-phase-0-1` to synchronize with the remote repository.

## Verification Plan

### Automated
- **History Audit**: `git log -n 5 --format="%B"` to verify comprehensive commit descriptions.
- **Build Verification**: Run `cargo check` and `npm run build` (if requested) to ensure no regressions were introduced during the organization.

### Manual Verification
- **Search Modality Test**: Perform a search that matches both an image (visual) and its OCR text. Verify two distinct results appear with correct icons.
- **Navigation Test**: Click an "IMAGE" result and verify it opens the Image Viewer. Click a "TEXT" result and verify it opens the Text Modal.
