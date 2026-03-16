# Deletion Robustness for All Entities

Confirm and enhance the "deletion robustness" logic to ensure that when any entity (Snippet, Clipboard Entry, Note) is deleted, all its associated data—including vector embeddings, search index metadata, and links—is correctly removed.

## Proposed Changes

### [KMS Repository]
Introduce utility functions for bulk and comprehensive entity cleanup in the semantic index.

#### [MODIFY] [kms_repository.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/kms_repository.rs)
- Add `delete_embeddings_for_entity(entity_type: &str, entity_id: &str)`: Deletes ALL modalities (text, image) for a given entity.
- Add `delete_all_embeddings_for_type(entity_type: &str)`: Deletes all embeddings and mappings for a specific category (e.g., "clipboard").
- Add `delete_embeddings_for_ids(entity_type: &str, entity_ids: &[String])`: Bulk deletes embeddings for a list of IDs.

### [Clipboard Repository]
Update the repository to return information about deleted rows so they can be cleaned up in the search index.

#### [MODIFY] [clipboard_repository.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/clipboard_repository.rs)
- Modify `trim_to_depth(max_depth: u32)` to return `Result<Vec<u32>, String>` containing the IDs of deleted entries instead of just the count.

### [API / IPC Layer]
Update the high-level commands to trigger the search index cleanup.

#### [MODIFY] [api.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/api.rs)
- `delete_clip_entry`: Call `kms_repository::delete_embeddings_for_entity("clipboard", &id)`.
- `delete_clip_entry_by_id`: Call `kms_repository::delete_embeddings_for_entity("clipboard", &id)`.
- `clear_clipboard_history`: Call `kms_repository::delete_all_embeddings_for_type("clipboard")`.
- `update_config`: (Clip history depth check) Capture deleted IDs from `trim_to_depth` and call `kms_repository::delete_embeddings_for_ids`.
- `sync_current_clipboard_image_to_sqlite`: Same as above for automatic trimming.

## Verification Plan

### Automated Tests
- No existing tests specifically cover vector index cleanup for clipboard.
- I will verify the logic by checking the `kms_vector_map` table counts before and after deletion using the sqlite tool or log output.

### Manual Verification
1. **Clipboard Deletion**:
   - Copy a string and an image to the clipboard so they are indexed.
   - Verify they appear in semantic search results.
   - Delete the entries via the UI.
   - Verify they no longer appear in semantic search results and their records in `kms_vector_map` are gone.
2. **Clipboard Clear**:
   - Fill history with indexed items.
   - "Clear History" in settings.
   - Verify `kms_vector_map` has 0 entries for `entity_type = 'clipboard'`.
3. **Trimming**:
   - Set max history to 5.
   - Add 10 items.
   - Verify settings and search only show the latest 5.
