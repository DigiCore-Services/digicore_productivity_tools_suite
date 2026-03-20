//! WindowsRichClipboardAdapter - native Win32 clipboard with HTML and RTF support.

use crate::domain::ports::ClipboardPort;
use anyhow::{Result, anyhow};
use windows::Win32::Foundation::{HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, RegisterClipboardFormatW,
    SetClipboardData,
};
use windows::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
};
use std::ptr;
use windows::core::PCWSTR;

pub struct WindowsRichClipboardAdapter;

impl WindowsRichClipboardAdapter {
    pub fn new() -> Self {
        Self
    }

    fn set_native(&self, format: u32, data: &[u8]) -> Result<()> {
        unsafe {
            let h_mem = GlobalAlloc(GMEM_MOVEABLE, data.len())?;
            if h_mem.is_invalid() {
                return Err(anyhow!("GlobalAlloc failed"));
            }
            let ptr = GlobalLock(h_mem);
            if ptr.is_null() {
                return Err(anyhow!("GlobalLock failed"));
            }
            ptr::copy_nonoverlapping(data.as_ptr(), ptr as *mut u8, data.len());
            let _ = GlobalUnlock(h_mem);

            if SetClipboardData(format, Some(HANDLE(h_mem.0))).is_err() {
                // If it fails, we should free the memory, but if it succeeds, the system owns it.
                return Err(anyhow!("SetClipboardData failed"));
            }
        }
        Ok(())
    }

    /// Wraps HTML content in the mandatory Windows Clipboard HTML Format header.
    fn wrap_html(&self, html: &str) -> String {
        let start_html = 105;
        let start_fragment = 141;
        let fragment = format!("<!--StartFragment-->{}<!--EndFragment-->", html);
        let body = format!("<html><body>{}</body></html>", fragment);
        let end_html = start_html + body.len();
        let end_fragment = start_fragment + html.len();

        format!(
            "Version:0.9\r\nStartHTML:{:010}\r\nEndHTML:{:010}\r\nStartFragment:{:010}\r\nEndFragment:{:010}\r\n{}",
            start_html, end_html, start_fragment, end_fragment, body
        )
    }

    /// Extracts the HTML fragment between <!--StartFragment--> and <!--EndFragment-->.
    fn unwrap_html(&self, raw: &str) -> String {
        let start_tag = "<!--StartFragment-->";
        let end_tag = "<!--EndFragment-->";

        if let Some(start_pos) = raw.find(start_tag) {
            if let Some(end_pos) = raw.find(end_tag) {
                if end_pos > start_pos + start_tag.len() {
                    return raw[start_pos + start_tag.len()..end_pos].to_string();
                }
            }
        }
        raw.to_string()
    }
}

impl ClipboardPort for WindowsRichClipboardAdapter {
    fn get_text(&self) -> Result<String> {
        let (plain, _, _) = self.get_rich_text()?;
        Ok(plain)
    }

    fn set_text(&self, text: &str) -> Result<()> {
        self.set_multi(text, None, None)
    }

    fn set_multi(&self, plain: &str, html: Option<&str>, rtf: Option<&str>) -> Result<()> {
        unsafe {
            if OpenClipboard(None).is_err() {
                return Err(anyhow!("OpenClipboard failed"));
            }
            if EmptyClipboard().is_err() {
                let _ = CloseClipboard();
                return Err(anyhow!("EmptyClipboard failed"));
            }
        }

        let mut res = Ok(());

        // 1. Set Plain Text (CF_UNICODETEXT = 13)
        let utf16: Vec<u16> = plain.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = unsafe { std::slice::from_raw_parts(utf16.as_ptr() as *const u8, utf16.len() * 2) };
        if let Err(e) = self.set_native(13, bytes) {
            res = Err(e);
        }

        // 2. Set HTML (HTML Format)
        if let Some(html_text) = html {
            let name: Vec<u16> = "HTML Format\0".encode_utf16().collect();
            let format_html = unsafe { RegisterClipboardFormatW(PCWSTR(name.as_ptr())) };
            if format_html != 0 {
                let wrapped = self.wrap_html(html_text);
                if let Err(e) = self.set_native(format_html, wrapped.as_bytes()) {
                    res = Err(e);
                }
            }
        }

        // 3. Set RTF (Rich Text Format)
        if let Some(rtf_text) = rtf {
            let name: Vec<u16> = "Rich Text Format\0".encode_utf16().collect();
            let format_rtf = unsafe { RegisterClipboardFormatW(PCWSTR(name.as_ptr())) };
            if format_rtf != 0 {
                if let Err(e) = self.set_native(format_rtf, rtf_text.as_bytes()) {
                    res = Err(e);
                }
            }
        }

        unsafe {
            let _ = CloseClipboard();
        }
        res
    }

    fn get_rich_text(&self) -> Result<(String, Option<String>, Option<String>)> {
        unsafe {
            if OpenClipboard(None).is_err() {
                return Err(anyhow!("OpenClipboard failed"));
            }
        }

        let mut plain = String::new();
        let mut html = None;
        let mut rtf = None;

        unsafe {
            // 1. Plain Text (CF_UNICODETEXT = 13)
            if let Ok(h_mem) = GetClipboardData(13) {
                if !h_mem.is_invalid() {
                    let h_global = HGLOBAL(h_mem.0);
                    let ptr = GlobalLock(h_global);
                    if !ptr.is_null() {
                        let len = GlobalSize(h_global);
                        let slice = std::slice::from_raw_parts(ptr as *const u16, len / 2);
                        let end = slice.iter().position(|&c| c == 0).unwrap_or(slice.len());
                        plain = String::from_utf16_lossy(&slice[..end]);
                        let _ = GlobalUnlock(h_global);
                    }
                }
            }

            // 2. HTML
            let name_html: Vec<u16> = "HTML Format\0".encode_utf16().collect();
            let format_html = RegisterClipboardFormatW(PCWSTR(name_html.as_ptr()));
            if format_html != 0 {
                if let Ok(h_mem) = GetClipboardData(format_html) {
                    if !h_mem.is_invalid() {
                        let h_global = HGLOBAL(h_mem.0);
                        let ptr = GlobalLock(h_global);
                        if !ptr.is_null() {
                            let len = GlobalSize(h_global);
                            let slice = std::slice::from_raw_parts(ptr as *const u8, len);
                            let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
                            let raw_html = String::from_utf8_lossy(&slice[..end]).to_string();
                            html = Some(self.unwrap_html(&raw_html));
                            let _ = GlobalUnlock(h_global);
                        }
                    }
                }
            }

            // 3. RTF
            let name_rtf: Vec<u16> = "Rich Text Format\0".encode_utf16().collect();
            let format_rtf = RegisterClipboardFormatW(PCWSTR(name_rtf.as_ptr()));
            if format_rtf != 0 {
                if let Ok(h_mem) = GetClipboardData(format_rtf) {
                    if !h_mem.is_invalid() {
                        let h_global = HGLOBAL(h_mem.0);
                        let ptr = GlobalLock(h_global);
                        if !ptr.is_null() {
                            let len = GlobalSize(h_global);
                            let slice = std::slice::from_raw_parts(ptr as *const u8, len);
                            let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
                            rtf = Some(String::from_utf8_lossy(&slice[..end]).to_string());
                            let _ = GlobalUnlock(h_global);
                        }
                    }
                }
            }

            let _ = CloseClipboard();
        }

        Ok((plain, html, rtf))
    }
}
