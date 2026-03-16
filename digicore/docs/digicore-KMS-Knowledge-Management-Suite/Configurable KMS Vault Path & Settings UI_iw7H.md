# Configurable KMS Vault Path & Settings UI

Enable users to customize their KMS vault location and fix the "Vault Settings" button in the UI.

> [!IMPORTANT]
> - **Hierarchical Support**: The app now supports nested folders (Notebooks). You can organize notes into sub-directories, and they will be discovered and indexed correctly.
> - **Relative Pathing**: Database records will now use paths relative to your vault. This makes your vault portable—you can move the entire folder to a different drive, and your favorites and links will "just work."
> - **Physical Migration**: Added a feature to physically move all `.md` files, sub-folders, and `/attachments/` to the new target.
> - **Discovery & Merging**: If the new target already contains Notes/Folders, they will be automatically discovered, merged, and AI-indexed.
> - **Metadata Persistence**: Existing records are updated to reference new relative paths.
> - **Collision Risk**: If files with the same name exist in the destination, the migration will offer to skip or overwrite.

## Proposed Changes

### [digicore-text-expander]
#### [MODIFY] [storage.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/src/ports/storage.rs)
- Add `KMS_VAULT_PATH` constant to `keys` module.

#### [MODIFY] [app_state.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/src/application/app_state.rs)
- Add `kms_vault_path: String` to `AppState` struct.

---

### [tauri-app]
#### [MODIFY] [lib.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/lib.rs)
- Update `AppStateDto` and `app_state_to_dto` to include `kms_vault_path`.
- Update `init_app_state_from_storage` to load the vault path from storage keys, defaulting to `Documents/DigiCore Notes` if empty.

#### [MODIFY] [api.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/api.rs)
- Refactor `kms_initialize` to use `state.kms_vault_path`.
- Update `kms_save_note` and other KMS methods to use the path from state.
- **Enhanced `kms_set_vault_path`**: 
    - Accepts `migrate: bool` flag.
    - If `true`, physically moves **entire directory structure** (recursive) to the new target.
    - Updates `kms_notes`, `kms_links`, and `kms_vector_map` table paths to use **Relative Paths** from the new vault root.
    - **Immediate Recursive Sync**: Automatically triggers a deep scan of the new path to discover and index all files in all sub-folders.
- Update `sync_vault_files_to_db` to be **Recursive**: Scans all sub-directories for `.md` files.
- Refactor all KMS commands to resolve relative DB paths to absolute FS paths using the configured vault path.

#### [MODIFY] [KmsApp.tsx](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/KmsApp.tsx)
- Integrate `VaultSettingsModal`.
- Update the "Vault Settings" button to open the modal.

#### [NEW] [VaultSettingsModal.tsx](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/components/kms/VaultSettingsModal.tsx)
- A modal dialog that displays the current vault path.
- "Change Path" triggers a folder picker.
- **Migration Dialogue**: Prompt the user: "Migrate existing notes and attachments to new location?"
- Status indicator for migration progress.

## Verification Plan

### Automated Tests
- N/A (UI-driven flow)

### Manual Verification
1.  Open KMS and verify the default vault path is displayed.
2.  Click "Vault Settings" and verify the modal opens.
3.  Click "Change Path", select a new folder, and verify the UI updates.
4.  Verify that notes in the new folder are successfully discovered and displayed.
5.  Verify that search works for notes in the new location.
