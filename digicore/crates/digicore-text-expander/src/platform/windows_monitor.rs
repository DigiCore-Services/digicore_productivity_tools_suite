//! Windows monitor work area helpers for Ghost Follower viewport placement.
//!
//! Uses MonitorFromPoint, GetMonitorInfoW, EnumDisplayMonitors to resolve
//! Primary, Secondary, and Current (cursor) monitor work areas.

#![cfg(target_os = "windows")]

use std::mem;
use windows::Win32::Foundation::{BOOL, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, MonitorFromPoint, HMONITOR, MONITORINFO,
    MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY,
};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

/// MONITORINFOF_PRIMARY = 1
const MONITORINFOF_PRIMARY: u32 = 1;

/// Work area in screen coordinates (left, top, right, bottom).
#[derive(Debug, Clone, Copy)]
pub struct WorkArea {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl WorkArea {
    pub fn width(&self) -> i32 {
        self.right - self.left
    }
    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }
}

/// Get work area of the primary monitor.
/// Falls back to (0, 0, 1920, 1080) if API fails.
pub fn get_primary_monitor_work_area() -> WorkArea {
    unsafe {
        let pt = POINT { x: 0, y: 0 };
        let hmon = MonitorFromPoint(pt, MONITOR_DEFAULTTOPRIMARY);
        get_monitor_work_area(hmon)
    }
}

/// Get work area of the first non-primary monitor (secondary).
/// Returns None if only one monitor or API fails.
pub fn get_secondary_monitor_work_area() -> Option<WorkArea> {
    unsafe {
        let mut monitors: Vec<HMONITOR> = Vec::new();
        let ptr = &mut monitors as *mut Vec<HMONITOR> as isize;

        let result = EnumDisplayMonitors(
            None,
            None,
            Some(enum_monitors_callback),
            LPARAM(ptr),
        );
        if !result.as_bool() {
            return None;
        }

        for hmon in monitors {
            let mut info = MONITORINFO {
                cbSize: mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if GetMonitorInfoW(hmon, &mut info).as_bool() {
                if (info.dwFlags & MONITORINFOF_PRIMARY) == 0 {
                    return Some(rect_to_work_area(info.rcWork));
                }
            }
        }
        None
    }
}

/// Get work area of the monitor containing the cursor.
/// Falls back to primary if API fails.
pub fn get_current_monitor_work_area() -> WorkArea {
    unsafe {
        let mut pt = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut pt).is_err() {
            return get_primary_monitor_work_area();
        }
        let hmon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
        get_monitor_work_area(hmon)
    }
}

unsafe fn get_monitor_work_area(hmon: HMONITOR) -> WorkArea {
    let mut info = MONITORINFO {
        cbSize: mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if GetMonitorInfoW(hmon, &mut info).as_bool() {
        rect_to_work_area(info.rcWork)
    } else {
        WorkArea {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        }
    }
}

fn rect_to_work_area(rc: RECT) -> WorkArea {
    WorkArea {
        left: rc.left,
        top: rc.top,
        right: rc.right,
        bottom: rc.bottom,
    }
}

unsafe extern "system" fn enum_monitors_callback(
    hmonitor: HMONITOR,
    _hdc: windows::Win32::Graphics::Gdi::HDC,
    _lprc: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let vec = &mut *(lparam.0 as *mut Vec<HMONITOR>);
    vec.push(hmonitor);
    BOOL::from(true)
}
