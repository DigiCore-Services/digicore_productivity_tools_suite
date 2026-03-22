//! WindowsWindowAdapter - implements WindowContextPort for Windows.

use crate::domain::ports::{WindowContext, WindowContextPort};
use anyhow::Result;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId};

/// Windows window context adapter.
pub struct WindowsWindowAdapter;

impl WindowsWindowAdapter {
    pub fn new() -> Self {
        Self
    }

    /// Creates a mock version of the adapter that returns a fixed context.
    pub fn mock(process_name: &str, title: &str) -> MockWindowAdapter {
        MockWindowAdapter {
            process_name: process_name.to_string(),
            title: title.to_string(),
        }
    }
}

/// A mock version of the WindowContextPort for testing.
pub struct MockWindowAdapter {
    pub process_name: String,
    pub title: String,
}

impl WindowContextPort for MockWindowAdapter {
    fn get_active(&self) -> Result<WindowContext> {
        Ok(WindowContext {
            process_name: self.process_name.clone(),
            title: self.title.clone(),
        })
    }
}

impl Default for WindowsWindowAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowContextPort for WindowsWindowAdapter {
    fn get_active(&self) -> Result<WindowContext> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_invalid() {
                return Ok(WindowContext::default());
            }

            let mut title = [0u16; 256];
            let len = GetWindowTextW(hwnd, &mut title) as usize;
            let title_str = String::from_utf16_lossy(if len > 0 { &title[..len] } else { &[] });

            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));

            let process_name = if pid != 0 {
                if let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                    let mut path = [0u16; 260];
                    let len = GetModuleFileNameExW(Some(handle), None, &mut path) as usize;
                    let _ = CloseHandle(handle);
                    let path_str = String::from_utf16_lossy(if len > 0 { &path[..len] } else { &[] });
                    path_str.rsplit('\\').next().unwrap_or("").to_string()
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            Ok(WindowContext {
                process_name,
                title: title_str,
            })
        }
    }
}
