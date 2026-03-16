# Hardening Unsafe Win32 Integrations & Diagnostic Improving

This plan aims to address the `STATUS_ACCESS_VIOLATION` (0xc0000005) crash by hardening several `unsafe` blocks identified as high-risk and adding detailed diagnostic logging around these areas to pinpoint the exact failure point if the crash persists.

## User Review Required

> [!IMPORTANT]
> The changes involve low-level Windows API interactions. While they are intended to be safer, they are difficult to verify without direct reproduction of the crash on the user's specific environment.

## Proposed Changes

### [digicore-text-expander]
Hardening the low-level keyboard hook and caret position retrieval.

#### [MODIFY] [windows_keyboard.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/src/platform/windows_keyboard.rs)
- Add a null check for `lparam.0` in `hook_proc` before dereferencing it as `KBDLLHOOKSTRUCT`.

#### [MODIFY] [windows_caret.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/src/platform/windows_caret.rs)
- Add more robust error handling around `GetGUIThreadInfo` and ensure its targeting is logged if it fails.

---

### [tauri-app]
Hardening the window transparency enforcement logic and improving background thread logging.

#### [MODIFY] [api.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/api.rs)
- **Hardening `enum_apply_transparency`**:
    - Add checks for window validity (`IsWindow`) before interacting.
    - Add descriptive logging if `GetLayeredWindowAttributes` or `SetLayeredWindowAttributes` fails.
    - Wrap the `lparam` dereference in a more cautious way (though already relatively safe).
- **Improving `enforce_appearance_transparency_rules`**:
    - Add a `diag_log` call at the start and end of the enforcement cycle to track timing and potential stalls.
    - Reduce frequency of `sysinfo` refresh if possible, or at least log when it begins.

#### [MODIFY] [lib.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/lib.rs)
- Add an error handler for the background thread that calls `enforce_appearance_transparency_rules` to prevent silent thread death or unhandled issues that might lead to instability.

## Verification Plan

### Automated Tests
- Run existing `commands_tests.rs` to ensure no regression in core logic:
  ```powershell
  cd tauri-app/src-tauri
  cargo test --test commands_tests
  ```

### Manual Verification
- **Transparency stress test**: Manually add a few transparency rules for common apps (e.g., `notepad.exe`, `cursor.exe`) via the UI and verify that they are applied correctly without crashing.
- **Hook stress test**: Rapidly type and use hotkeys to verify that the keyboard hook remains stable.
- **Diagnostic log check**: Open the application's "Diagnostic Logs" (via the UI) and verify that the new enforcement cycle logs are appearing every 3 seconds.
