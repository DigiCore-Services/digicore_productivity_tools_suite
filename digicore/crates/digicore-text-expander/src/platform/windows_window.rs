//! Windows window focus helpers for restoring focus before expansion.
//!
//! Uses AttachThreadInput workaround for Electron apps (VS Code, Cursor, Antigravity)
//! where plain SetForegroundWindow is blocked by Windows focus-stealing prevention.

#![cfg(target_os = "windows")]

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow,
    ShowWindow, SW_SHOW,
};

/// Get the foreground window handle (the window that has focus).
/// Returns None if invalid.
pub fn get_foreground_hwnd() -> Option<isize> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() || hwnd.0.is_null() {
            None
        } else {
            Some(hwnd.0 as isize)
        }
    }
}

/// Restore focus to the given window before pasting expansion.
/// Uses AttachThreadInput workaround so SetForegroundWindow succeeds with Electron
/// apps (VS Code, Cursor, Antigravity) where Windows blocks focus-stealing.
pub fn restore_foreground_window(hwnd: isize) {
    if hwnd == 0 {
        return;
    }
    unsafe {
        let h = HWND(hwnd as *mut _);
        if h.0.is_null() {
            return;
        }
        let fore = GetForegroundWindow();
        let fore_thread = if fore.is_invalid() || fore.0.is_null() {
            None
        } else {
            Some(GetWindowThreadProcessId(fore, None))
        };
        let app_thread = GetCurrentThreadId();
        let attached = fore_thread.map_or(false, |ft| {
            ft != app_thread && AttachThreadInput(ft, app_thread, true).as_bool()
        });
        let _ = BringWindowToTop(h);
        let _ = ShowWindow(h, SW_SHOW);
        let _ = SetForegroundWindow(h);
        if attached {
            if let Some(ft) = fore_thread {
                let _ = AttachThreadInput(ft, app_thread, false);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_foreground_hwnd_does_not_panic() {
        let result = get_foreground_hwnd();
        assert!(result.is_none() || result.is_some());
    }

    #[test]
    fn test_restore_foreground_window_zero_noop() {
        restore_foreground_window(0);
    }
}
