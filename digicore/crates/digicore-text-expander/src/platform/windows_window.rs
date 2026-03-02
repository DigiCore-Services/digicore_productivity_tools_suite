//! Windows window focus helpers for restoring focus before expansion.

#![cfg(target_os = "windows")]

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, SetForegroundWindow};

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
/// Call this before do_expand when the user completed the variable input modal.
pub fn restore_foreground_window(hwnd: isize) {
    if hwnd == 0 {
        return;
    }
    unsafe {
        let h = HWND(hwnd as *mut _);
        if !h.0.is_null() {
            let _ = SetForegroundWindow(h);
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
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
