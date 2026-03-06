//! Windows window focus helpers for restoring focus before expansion.
//!
//! Uses AttachThreadInput workaround for Electron apps (VS Code, Cursor, Antigravity)
//! where plain SetForegroundWindow is blocked by Windows focus-stealing prevention.

#![cfg(target_os = "windows")]

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GetAncestor, GetClassNameW, GetForegroundWindow, GetTopWindow, GetWindow,
    GetWindowThreadProcessId, GetWindowTextW, IsWindowVisible, SetForegroundWindow, ShowWindow,
    GA_ROOT, GA_ROOTOWNER, GW_HWNDPREV, GW_HWNDNEXT, SW_SHOW,
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

fn is_excluded_window_class(class_name: &str) -> bool {
    matches!(
        class_name,
        "Shell_TrayWnd"
            | "NotifyIconOverflowWindow"
            | "#32768"
            | "Progman"
            | "WorkerW"
            | "ConsoleWindowClass"
            | "tray_icon_app"
            | "TopLevelWindowForOverflowXamlIsland"
            | "ThumbnailDeviceHelperWnd"
            | "XamlExplorerHostIslandWindow_WASDK"
    )
}

fn get_window_class_name(hwnd: HWND) -> Option<String> {
    let mut buf = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, &mut buf) };
    if len <= 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buf[..len as usize]))
}

fn get_window_process_id(hwnd: HWND) -> Option<u32> {
    let mut pid = 0u32;
    unsafe {
        let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
    }
    if pid == 0 {
        None
    } else {
        Some(pid)
    }
}

fn get_window_title(hwnd: HWND) -> Option<String> {
    let mut buf = [0u16; 512];
    let len = unsafe { GetWindowTextW(hwnd, &mut buf) };
    if len <= 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buf[..len as usize]))
}

pub fn describe_hwnd(hwnd_raw: isize) -> String {
    if hwnd_raw == 0 {
        return "hwnd=0".to_string();
    }
    let hwnd = HWND(hwnd_raw as *mut _);
    let class_name = get_window_class_name(hwnd).unwrap_or_else(|| "?".to_string());
    let pid = get_window_process_id(hwnd)
        .map(|p| p.to_string())
        .unwrap_or_else(|| "?".to_string());
    let title = get_window_title(hwnd).unwrap_or_default();
    format!(
        "hwnd=0x{:X} class={} pid={} title=\"{}\"",
        hwnd_raw as usize, class_name, pid, title
    )
}

pub fn describe_foreground_window() -> String {
    if let Some(hwnd) = get_foreground_hwnd() {
        describe_hwnd(hwnd)
    } else {
        "foreground=<none>".to_string()
    }
}

fn foreground_external_hwnd_once() -> Option<isize> {
    let hwnd_raw = normalize_candidate_hwnd(get_foreground_hwnd()?)?;
    if !is_external_window_hwnd(hwnd_raw) {
        return None;
    }
    Some(hwnd_raw)
}

/// Strict capture: only accept the current foreground window if it is an external app window.
/// Does not walk z-order and is safe for "remember target" operations.
pub fn capture_strict_external_foreground_hwnd() -> Option<isize> {
    foreground_external_hwnd_once()
}

/// Returns true when hwnd is suitable as a restore-and-paste target.
pub fn is_valid_external_hwnd(hwnd_raw: isize) -> bool {
    let Some(normalized) = normalize_candidate_hwnd(hwnd_raw) else {
        return false;
    };
    is_external_window_hwnd(normalized)
}

fn is_external_window_hwnd(hwnd_raw: isize) -> bool {
    let hwnd = HWND(hwnd_raw as *mut _);
    if hwnd.0.is_null() {
        return false;
    }
    if unsafe { !IsWindowVisible(hwnd).as_bool() } {
        return false;
    }
    let Some(pid) = get_window_process_id(hwnd) else {
        return false;
    };
    if pid == std::process::id() {
        return false;
    }
    if let Some(class_name) = get_window_class_name(hwnd) {
        if is_excluded_window_class(class_name.as_str()) {
            return false;
        }
    }
    true
}

fn external_window_from_z_order(start_raw: isize, max_scan: usize) -> Option<isize> {
    let mut current = HWND(start_raw as *mut _);
    for _ in 0..max_scan {
        current = match unsafe { GetWindow(current, GW_HWNDPREV) } {
            Ok(h) => h,
            Err(_) => break,
        };
        if current.0.is_null() {
            break;
        }
        let raw = current.0 as isize;
        let Some(raw) = normalize_candidate_hwnd(raw) else {
            continue;
        };
        if is_external_window_hwnd(raw) {
            return Some(raw);
        }
    }
    None
}

fn top_external_window(max_scan: usize) -> Option<isize> {
    let mut current = match unsafe { GetTopWindow(None) } {
        Ok(h) => h,
        Err(_) => return None,
    };
    if current.0.is_null() {
        return None;
    }
    for _ in 0..max_scan {
        let raw = current.0 as isize;
        if let Some(raw) = normalize_candidate_hwnd(raw) {
            if is_external_window_hwnd(raw) {
                return Some(raw);
            }
        }
        current = match unsafe { GetWindow(current, GW_HWNDNEXT) } {
            Ok(h) => h,
            Err(_) => break,
        };
        if current.0.is_null() {
            break;
        }
    }
    None
}

fn normalize_candidate_hwnd(hwnd_raw: isize) -> Option<isize> {
    let hwnd = HWND(hwnd_raw as *mut _);
    if hwnd.0.is_null() {
        return None;
    }
    let root_owner = unsafe { GetAncestor(hwnd, GA_ROOTOWNER) };
    if !root_owner.0.is_null() {
        return Some(root_owner.0 as isize);
    }
    let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
    if !root.0.is_null() {
        return Some(root.0 as isize);
    }
    Some(hwnd_raw)
}

/// Capture a foreground window that is likely the user's external target app.
/// Retries briefly to avoid storing transient tray/menu windows.
pub fn capture_recent_external_foreground_hwnd(max_wait_ms: u64) -> Option<isize> {
    if let Some(hwnd) = foreground_external_hwnd_once() {
        return Some(hwnd);
    }
    if let Some(hwnd) = top_external_window(64) {
        return Some(hwnd);
    }
    if let Some(fg_raw) = get_foreground_hwnd() {
        if let Some(hwnd) = external_window_from_z_order(fg_raw, 24) {
            return Some(hwnd);
        }
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(max_wait_ms);
    while std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(25));
        if let Some(hwnd) = foreground_external_hwnd_once() {
            return Some(hwnd);
        }
        if let Some(hwnd) = top_external_window(64) {
            return Some(hwnd);
        }
        if let Some(fg_raw) = get_foreground_hwnd() {
            if let Some(hwnd) = external_window_from_z_order(fg_raw, 24) {
                return Some(hwnd);
            }
        }
    }
    None
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

    #[test]
    fn test_excluded_window_classes() {
        assert!(is_excluded_window_class("Shell_TrayWnd"));
        assert!(is_excluded_window_class("#32768"));
        assert!(is_excluded_window_class("NotifyIconOverflowWindow"));
        assert!(is_excluded_window_class("tray_icon_app"));
        assert!(is_excluded_window_class("TopLevelWindowForOverflowXamlIsland"));
        assert!(is_excluded_window_class("ThumbnailDeviceHelperWnd"));
        assert!(is_excluded_window_class("XamlExplorerHostIslandWindow_WASDK"));
        assert!(!is_excluded_window_class("Chrome_WidgetWin_1"));
    }
}
