# KMS Vault Auto-Synchronization Plan

The user's existing notes are not showing up in the UI even after a database repair because the app currently only indexes files created through the GUI. This plan adds a robust synchronization process that reconciles the local filesystem with the database on every launch.

## Proposed Changes

### [Backend] KMS Layer
#### [MODIFY] [kms_repository.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/kms_repository.rs)
- Ensure `upsert_note` handles metadata updates correctly even if the file was modified outside the app.

#### [MODIFY] [api.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/api.rs)
- **New helper `sync_vault_files_to_db`**: Scans the `vault_path/notes` directory and performs a three-way reconciliation:
    1. **Discovered Files**: Files found on disk that aren't in the DB are inserted.
    2. **AI Indexing**: Automatically generate semantic embeddings and refresh FTS5 entries for every new or updated file discovery.
    3. **Stale Records**: Records in the DB whose files no longer exist on disk are deleted.
    4. **Metadata Sync**: Existing records are updated to match the current filename (title).
- **Update `kms_initialize`**: Call `sync_vault_files_to_db` immediately after initializing the repository.
- **Update `kms_search_semantic`**: Ensure the distance scores and entity paths are accurately mapped to the discovered files.

## Verification Plan

### Automated Tests
- No existing automated tests cover the KMS sync layer. I will verify via logs and manual checks.

### Manual Verification
1. **Manual File Import**:
    - Manually copy a `.md` file into the `DigiCore Notes/notes` directory using Windows Explorer.
    - Launch the app.
    - Verify the note appears in the "Explorer" sidebar automatically.
2. **Search Reconciliation**:
    - Search for a keyword found in an imported note.
    - Verify the result displays the correct filename.
    - Click the search result and verify the note opens in the editor.
3. **Ghost Record Cleanup**:
    - Delete a note file from the disk while the app is closed.
    - Launch the app.
    - Verify the note is gone from the sidebar.
