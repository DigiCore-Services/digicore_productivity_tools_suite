# Implementation Plan - Phase 66: Image Library & Layout OCR Viewer

Add a dedicated "Image Library" tab to the Tauri application that allows users to browse captured images in a gallery format with pagination, and view images with an interactive OCR layout overlay.

## Proposed Changes

### [Backend - Rust]

#### [MODIFY] [clipboard_repository.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/clipboard_repository.rs)
- **Migration**: Add Migration #4 to add `metadata` TEXT column to `clipboard_history`.
- **Insert**: Update `insert_extracted_text_entry` to persist the `metadata` JSON.
- **Query**: Add `list_image_entries(page, page_size, search)` that returns a paged list of image entries.

#### [MODIFY] [api.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/api.rs)
- **TauRPC**: Add `get_image_gallery(page: u32, page_size: u32, search: Option<String>) -> Result<(Vec<ClipEntryDto>, u32), String>`.
- **TauRPC**: Add `get_image_ocr_details(id: u32) -> Result<serde_json::Value, String>`.
- Update `persist_clipboard_entry_with_settings` (or rather the OCR observer logic) to ensure metadata is passed through.

### [Frontend - TypeScript/React]

#### [NEW] [ImageLibraryTab.tsx](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/components/ImageLibraryTab.tsx)
- Grid layout for thumbnails.
- Pagination controls (10/25/50 per page, page navigation).
- Context menu integration (reuse `ClipboardTab` actions).

#### [NEW] [ImageViewerModal.tsx](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/components/modals/ImageViewerModal.tsx)
- Full-screen image view.
- Navigation (Next/Prev) based on current gallery state.
- "OCR Overlay" toggle: Renders bounding boxes and text from metadata using absolute positioning over the image.
- "Properties" panel: Shows app name, window title, dimensions, etc.

#### [MODIFY] [MainLayout.tsx](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/App.tsx) (or wherever tabs are managed)
- Add "Image Library" to the main navigation.

## Verification Plan

### Automated Tests
- `cargo test --package digicore-text-expander` to ensure migrations and repository queries work.
- Add a new integration test in `api.rs` for `get_image_gallery`.

### Manual Verification
1. Capture several images to the clipboard.
2. Open "Image Library".
3. Verify pagination works (switching between 10 and 25 items).
4. Right-click an image and verify "Save As" and "Copy" work.
5. Click an image to open the viewer.
6. Toggle "OCR Overlay" and verify text alignment matches the image content.
7. Navigate through images using Prev/Next buttons in the viewer.
