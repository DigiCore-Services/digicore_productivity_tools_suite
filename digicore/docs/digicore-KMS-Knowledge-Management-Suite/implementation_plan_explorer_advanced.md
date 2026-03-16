# Implementation Plan - Advanced KMS Explorer Features (Phase 86)

Enhancing the KMS Explorer with robust folder management, organization via Drag & Drop, and improved UI flexibility with a resizable sidebar.

## User Review Required

> [!IMPORTANT]
> **Mandatory Confirmations**: Per user requirement, EVERY significant action (Individual Rename, Bulk Rename, Recursive Delete, and Drag & Drop Move) MUST include a confirmation message before the operation is executed.

## Proposed Changes

### Backend Implementation (`api.rs`)

#### [MODIFY] [api.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/api.rs)
- Implement `kms_rename_folder`:
    - Rename directory on disk using `std::fs::rename`.
    - Bulk update database records for all notes where `path` starts with the old folder path.
    - Update `kms_vector_map` and `kms_links` for all moved items.
- Implement `kms_delete_folder`:
    - Recursive cleanup of database entries for all files in the folder (logic similar to `kms_delete_note`).
    - Recursive deletion of the directory from the file system using `std::fs::remove_dir_all`.
- Implement `kms_move_item`:
    - Generalizes `kms_rename_note` and `kms_rename_folder` to support moving items to different locations.
    - Logic: Calculate new path based on target parent, then perform rename.

---

### Frontend Implementation

#### [MODIFY] [KmsApp.tsx](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/KmsApp.tsx)
- **Resizable Sidebar**:
    - Add a `sidebarWidth` state (default 280px) in `KmsApp.tsx`.
    - Create a transparent 4px "resize handle" div on the right edge of the sidebar.
    - Implement `mousedown` on handle, `mousemove` and `mouseup` on `window` for smooth dragging.
    - Persist to `localStorage.setItem("kms-sidebar-width", width)`.
- **Handlers**:
    - Add `handleDeleteFolder` with `window.confirm` check per critical requirement.
    - Update `FileExplorer` to trigger `kms_delete_folder`.

#### [MODIFY] [FileExplorer.tsx](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/components/kms/FileExplorer.tsx)
- **Drag & Drop**:
    - Update `FileExplorer` to use native HTML5 Draggable API.
    - Notes and Folders are draggable.
    - Folders are drop targets.
    - On Drop: Call `kms_move_item` with `item.path` and `targetFolder.path`.
- **Improved Context Menu**:
    - Add "Rename" and "Delete" actions to folder context menus.
    - Ensure correct handling of `item_type`.

## Verification Plan

### Automated Tests
- Run `cargo check` for backend changes.
- Manual verification of path updates in database after folder move/rename.

### Manual Verification
1. **Sidebar Resize**: Drag the boundary, refresh app, and verify width is restored.
2. **Recursive Delete**: Create a nested folder with notes, delete the parent, verify disk and DB are clean.
3. **Bulk Rename**: Rename a notebook containing linked notes, verify links still work.
4. **Drag & Drop**: Move a note from one folder to another, verify filesystem update.
