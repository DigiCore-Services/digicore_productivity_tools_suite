# Plan: Fix Duplicate Clipboard Capture

Resolve the issue of redundant JSON files and database entries created by the clipboard capture logic.

## Proposed Changes

### [Component Name] Tauri Backend (`tauri-app/src-tauri`)

#### [MODIFY] [api.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/api.rs)
- Remove `sync_runtime_clipboard_entries_to_sqlite()` calls from `get_clipboard_entries` and `search_clipboard_entries`. The real-time observer already ensures these are up-to-date.
- Change `sync_runtime_clipboard_entries_to_sqlite` visibility to `pub(crate)` so it can be called from `lib.rs`.
- Add a check in `sync_runtime_clipboard_entries_to_sqlite` to avoid re-syncing if the DB already has the new entries (though removing the redundant calls is the primary fix).

#### [MODIFY] [lib.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/lib.rs)
- Call `crate::api::sync_runtime_clipboard_entries_to_sqlite()` exactly once after starting the clipboard listener during setup. This ensures the startup "seeded" clipboard content is persisted without causing a loop during regular operation.

## Verification Plan

### Automated Tests
- No new automated tests are required as this is an architectural fix for redundant calls.
- Verify existing tests pass to ensure no regressions in clipboard persistence.

### Manual Verification
1. **Clear History**: Use the "Clear History" feature in the UI.
2. **Monitor Logs**: Verify that copying something once creates exactly one `[Clipboard][capture.accepted]` log entry and one JSON file.
3. **Open History**: Open the clipboard history UI multiple times; verify that NO new JSON files are created just by viewing the history.
4. **Search**: Perform a clipboard search; verify no duplicate entries are created.
