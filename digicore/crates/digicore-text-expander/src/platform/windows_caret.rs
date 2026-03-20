//! Windows caret position for Ghost Suggestor overlay (F46).
//!
//! Uses GetGUIThreadInfo to get caret position, MapWindowPoints for screen coords.
//! Falls back to GetCursorPos when caret is not available.

use windows::Win32::Foundation::POINT;
use windows::Win32::Graphics::Gdi::MapWindowPoints;
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetGUIThreadInfo, GUITHREADINFO};

/// Get caret position in screen coordinates (F46).
/// Returns (x, y) or None if caret/cursor position unavailable.
pub fn get_caret_screen_position() -> Option<(i32, i32)> {
    unsafe {
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        if GetGUIThreadInfo(0, &mut info).is_err() {
            return get_cursor_fallback();
        }
        if info.hwndCaret.is_invalid() {
            return get_cursor_fallback();
        }
        let mut pts = [
            POINT {
                x: info.rcCaret.left,
                y: info.rcCaret.top,
            },
            POINT {
                x: info.rcCaret.right,
                y: info.rcCaret.bottom,
            },
        ];
        if MapWindowPoints(Some(info.hwndCaret), None, &mut pts) == 0 {
            return get_cursor_fallback();
        }
        Some((pts[0].x, pts[0].y))
    }
}

fn get_cursor_fallback() -> Option<(i32, i32)> {
    unsafe {
        let mut pt = POINT::default();
        if GetCursorPos(&mut pt).is_ok() {
            Some((pt.x, pt.y))
        } else {
            None
        }
    }
}
