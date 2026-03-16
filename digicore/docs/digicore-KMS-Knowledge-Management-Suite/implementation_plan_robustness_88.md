# Implementation Plan - KMS Robustness & Architectural Refactoring (Phase 88)

Establish a robust foundation for the KMS Knowledge Hub by implementing structured error handling, persistent diagnostic logging, and strict separation of concerns using Hexagonal architecture.

## User Review Required

> [!NOTE]
> **Architectural Shift**: I am moving significant logic (path normalization, backlink updates, recursive folder logic) from the `api.rs` (IPC adapter) into a new `kms_service.rs` (Application Service). This adheres to Hexagonal architecture by keeping the IPC layer thin and the domain/application layer testable.
> **Diagnostic Persistence**: A new `kms_logs` table will be added to your SQLite vault. This will store synchronization events and errors, allowing us to build a "Sync History" view later.

## Proposed Changes

### Domain & Error Handling
#### [NEW] [kms_error.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/kms_error.rs)
- Define `KmsError` enum using `thiserror`.
- Categories: `Io`, `Database`, `Encoding`, `NotFound`, `Validation`, `Security`.

### Application Services
#### [NEW] [kms_service.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/kms_service.rs)
- Extract "Application Logic" from `api.rs`.
- Handle high-level orchestration:
    - `rename_note`: File rename + Backlink refactoring + DB update.
    - `move_item`: Validation + FS Move + DB Bulk Update.
    - `sync_vault`: Filesystem crawl + DB reconciliation.

#### [NEW] [kms_diagnostic_service.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/kms_diagnostic_service.rs)
- Interface for persistent logging.
- `fn log_sync_event(level: LogLevel, message: String, details: Option<String>)`.

### Persistence Layer
#### [MODIFY] [kms_repository.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/kms_repository.rs)
- Add `kms_logs` table to the schema.
- Add `insert_log` function.
- Add `clear_logs` and `list_logs` functions.

### API Adapter
#### [MODIFY] [api.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/api.rs)
- Refactor KMS commands to delegate to `kms_service.rs`.
- Standardize error mapping (e.g., `.map_err(|e| e.to_string())` after internal `KmsError` handling).

### Frontend UI
#### [MODIFY] [KmsApp.tsx](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/KmsApp.tsx)
- Add a "Sync Status" indicator (icon/text) showing the result of the last sync or any active errors.
- Add a "View Logs" button (potentially in a modal) to display structured diagnostic info.

---

## Verification Plan

### Automated Tests
- **Unit (Rust)**:
    - Create `kms_service_tests.rs` to verify path normalization and link graph logic in isolation.
    - `cargo test --test kms_service_tests`
- **Integration**:
    - Verify that `kms_get_logs` returns logs inserted during a mock sync operation.

### Manual Verification
1.  **Log Persistence**: Trigger a rename operation and verify (using `sqlite3` or a UI log viewer) that an entry appeared in `kms_logs`.
2.  **Error Handling**: Attempt to move a file to a read-only directory or a non-existent parent; verify the UI shows a detailed, categorized error message.
3.  **UI Feedback**: Verify the "Sync Status" indicator updates correctly after a manual vault refresh.
