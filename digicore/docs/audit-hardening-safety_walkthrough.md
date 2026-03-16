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

### Manual Verification Steps (Recommended for User)
1. **Check Logs**: Open the application's "Diagnostic Logs" and verify that enforcement cycle entries (e.g., `[Appearance] Enforcement cycle completed`) appear every 3 seconds.
2. **Stress Test**: Open multiple windows and apply transparency rules. The application should remain stable.
3. **Keyboard Interaction**: Rapidly use hotkeys and type; the keyboard hook has been hardened against null pointers.

> [!NOTE]
> If the crash persists, the new diagnostic logs will help pinpoint the exact function and state at the time of failure.
