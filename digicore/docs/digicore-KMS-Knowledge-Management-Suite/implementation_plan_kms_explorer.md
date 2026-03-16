# Implementation Plan: KMS Knowledge Hub Hierarchical Explorer

This plan details the transformation of the KMS Explorer from a flat list to a hierarchical tree structure.

## User Review Required
> [!IMPORTANT]
> The implementation assumes folders in the Explorer strictly mirror filesystem directories, following Obsidian/Notion patterns.
> [!WARNING]
> Folder rename operations will require recursive path updates for all child notes and refactoring of backlinks throughout the vault.

## Proposed Changes

### [KMS Backend]
#### [MODIFY] [api.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/api.rs)
- Add `KmsFileSystemItemDto` struct.
- Implement `kms_get_vault_structure` command.

#### [MODIFY] [kms_repository.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/kms_repository.rs)
- Add helper methods for directory-aware operations.

### [Frontend]
#### [MODIFY] [KmsApp.tsx](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/KmsApp.tsx)
- Replace flat list with `FileExplorer` component.

#### [NEW] [FileExplorer.tsx](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/components/kms/FileExplorer.tsx)
- Recursive tree component for managing hierarchical views.

## Verification Plan

### Automated Tests
- Run `cargo test` (after adding unit tests for tree builder logic).
- Run `npm test` (if we add frontend tree transformation tests).

### Manual Verification
1. Verify deep nesting (3+ levels) renders correctly.
2. Verify "New Folder" and "New Note in Folder" persist correctly on disk.
3. Verify Folder Rename updates all nested note paths in the DB.
4. Verify "Semantic Search" still opens notes regardless of depth.

**Detailed plan is available at**: [KMS_Explorer_Enhancement_Plan.md](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/docs/digicore-KMS-Knowledge-Management-Suite/KMS_Explorer_Enhancement_Plan.md)
