//! Windows-specific fix for Ghost Suggestor click-through.
//!
//! Tauri's set_ignore_cursor_events can fail to properly disable click-through on Windows.
//! This module uses raw Win32 API to forcibly remove WS_EX_TRANSPARENT from the window.

#![cfg(windows)]

use windows::core::PCWSTR;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_TRANSPARENT,
};

const GHOST_SUGGESTOR_TITLE: &str = "Ghost Suggestor";

/// Remove WS_EX_TRANSPARENT from the Ghost Suggestor window so it captures mouse clicks.
/// Call this when the overlay should be interactive (has suggestions, not auto-hidden).
pub fn ensure_ghost_suggestor_captures_clicks() {
    unsafe {
        let title: Vec<u16> = GHOST_SUGGESTOR_TITLE
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let Ok(hwnd) = FindWindowW(PCWSTR::null(), PCWSTR::from_raw(title.as_ptr())) else {
            return;
        };
        if hwnd.is_invalid() || hwnd.0.is_null() {
            return;
        }
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        if ex_style == 0 {
            return;
        }
        let new_style = ex_style & !(WS_EX_TRANSPARENT.0 as isize);
        if new_style != ex_style {
            let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style);
        }
    }
}
