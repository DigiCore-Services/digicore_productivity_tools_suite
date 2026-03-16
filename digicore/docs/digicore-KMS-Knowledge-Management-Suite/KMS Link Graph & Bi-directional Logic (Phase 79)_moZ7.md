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
    - [x] Verify "Saving..." no longer hangs UI
