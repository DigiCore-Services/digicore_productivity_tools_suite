# Implementation Plan - Skill Hub Foundation (Backend)

This plan covers the initial backend implementation of the Agent Skills standard, specifically the data models and metadata extraction logic.

- [x] Phase 7: Sync Engine & Watcher
- [x] Phase 8: Frontend - Skill Management UI

### Phase 6: Skill Hub Foundation (Backend) [COMPLETED]

- **[NEW] [skill.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/digicore-core/src/domain/entities/skill.rs)**: `Skill` and `SkillMetadata` structs with progressive disclosure extraction logic. [DONE]
- **[MODIFY] [kms_repository.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/kms_repository.rs)**: Implement `KmsSkillRepository` for SQLite-backed skill storage. [DONE]
- **[MODIFY] [lib.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/lib.rs)**: Add Migration 14 for `kms_skills` table and FTS5 triggers. [DONE]
- **[MODIFY] [indexing_service.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/indexing_service.rs)**: Register `SkillIndexProvider` for semantic search. [DONE]
- **[MODIFY] [api.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/api.rs)**: Expose `kms_list_skills`, `kms_save_skill`, etc., to the frontend. [DONE]

### [Test Suite]
#### [NEW] [skill_tests.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-core/tests/skill_tests.rs)
- Unit tests for metadata parsing.
- Edge cases: Empty frontmatter, invalid YAML, max length violations.
- Negative cases: reserved words ("anthropic", "claude") in names.

## Verification Plan

### Automated Tests
- Run `cargo test -p digicore-core --test skill_tests` to verify the metadata extractor.
- Ensure all "Level 1" metadata (name, description) is correctly extracted from sample `SKILL.md` files.

### Manual Verification
- Not applicable for this purely logical/backend phase.
