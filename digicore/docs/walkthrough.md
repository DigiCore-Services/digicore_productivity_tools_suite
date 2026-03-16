# Walkthrough - Resolving STATUS_ACCESS_VIOLATION (0xc0000005)

I have implemented several hardening measures and improved diagnostic logging to resolve the `STATUS_ACCESS_VIOLATION` crash in the Tauri application. These changes target the most likely sources of memory access errors—low-level Windows API interactions and periodic background tasks.

## Changes Made

### 1. Hardened Keyboard Hook Procedure
In `windows_keyboard.rs`, I added a null check for `lparam` before dereferencing it. This prevents a potential null pointer dereference in the low-level keyboard hook, which is a common cause of 0xc0000005 errors.

### 2. Improved Window Transparency Enforcement
In `api.rs`, I added:
- **Null Checks & Window Validity**: The `enum_apply_transparency` callback now verifies that both the `lparam` pointer and the `HWND` are valid before proceeding.
- **Timing & Status Logs**: Added logging to track the start and end of each transparency enforcement cycle, ensuring visibility into its behavior.

### 3. Background Thread Stability
In `lib.rs`, the periodic transparency enforcement loop is now wrapped in `std::panic::catch_unwind`. This prevents the entire application from crashing if a transient error occurs during enforcement and ensures the panic is logged.

### 4. Global Panic Hook
Added a global panic hook in `lib.rs` that logs any unhandled panics. This will provide immediate visibility into the cause of any future crashes directly in the application logs.

## Verification Results

### Automated Tests
Successfully ran existing automated tests in `tauri-app/src-tauri` using `cargo test --test commands_tests`. All tests passed, confirming core logic remains intact.


## Resolving Duplicate Clipboard Capture

I have resolved the issue where multiple JSON files were being created for a single clipboard copy event.

### 1. Removed Redundant Sync Calls
In `api.rs`, I removed calls to `sync_runtime_clipboard_entries_to_sqlite()` from the `get_clipboard_entries` and `search_clipboard_entries` commands. These calls were redundant because the real-time clipboard observer already persists new entries as they occur. Triggering a full buffer sync every time the history was viewed caused multiple re-persistence cycles.

### 2. Implemented Single Startup Sync
In `lib.rs`, I added a single call to the sync function during application setup. This ensures that the initial clipboard content (captured during "seeding" at startup) is persisted to the database and JSON storage exactly once, without interfering with the regular observer-based capture.

## Verification Results

### Automated Tests
Successfully ran the automated test suite (`cargo test --test commands_tests`) to ensure that removing the redundant sync calls did not break the existing clipboard persistence logic. All tests passed.


## Diagnosing DB Persistence Failure

I have added targeted diagnostic logging to trace exactly how and where the clipboard database is being updated.

### 1. Database Initialization Trace
In `clipboard_repository.rs`, the `init` function now logs the absolute path of the SQLite database file it opens.
- **Log entry**: `[ClipboardRepository] Opening database at: <path>`

### 2. Insertion Flow Tracking
I've added logging to the following points in the persistence flow:
- **Clipboard Repository**: Logs when an insertion is attempted (including the content hash) and when it successfully completes (including the generated Row ID).
    - `[ClipboardRepository] Inserting text entry, hash: <hash>`
    - `[ClipboardRepository] Text insertion successful, row_id: <id>`
- **API Wrapper**: Logs the result of the `insert_entry` call to confirm whether it returned `true` (success/new entry) or `false` (duplicate/skipped).
    - `[Clipboard] clipboard_repository::insert_entry returned <bool>`

## Troubleshooting Steps for the User

1. **Start the application with Info logging**:
   ```powershell
   cd tauri-app; $env:RUST_LOG="info"; npm run tauri dev
   ```
2. **Observe the Initialization**: Look for the `Opening database at:` log entry to verify the location of the `digicore.db` file.
3. **Trigger a Copy**: Copy text and look for the `Inserting text entry` and `Text insertion successful` entries.
4. **Verify the Record**: If the logs show `Text insertion successful, row_id: <id>`, but the record is missing from the table when you check via a SQLite tool, this indicates a transaction/committal issue or a structural mismatch in the DB file.
