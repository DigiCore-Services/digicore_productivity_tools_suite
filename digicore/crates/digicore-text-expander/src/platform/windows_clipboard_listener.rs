//! Windows clipboard format listener - WM_CLIPBOARDUPDATE.
//!
//! Uses AddClipboardFormatListener with a message-only window to receive
//! clipboard change notifications. At notification time we query the foreground
//! window (source app) for App and Window Title - AHK parity.

use digicore_core::domain::ports::WindowContextPort;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::HBRUSH;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::DataExchange::{AddClipboardFormatListener, RemoveClipboardFormatListener};
use windows::Win32::UI::WindowsAndMessaging::*;

const WM_CLIPBOARDUPDATE: u32 = 0x031D;
const WM_CREATE: u32 = 0x0001;
const WM_DESTROY: u32 = 0x0002;

static LISTENER_ACTIVE: AtomicBool = AtomicBool::new(false);

thread_local! {
    static MSG_WND: std::cell::RefCell<Option<HWND>> = std::cell::RefCell::new(None);
}

/// Start clipboard listener in background thread. On WM_CLIPBOARDUPDATE,
/// reads clipboard, gets foreground window context, and calls on_clip.
/// Runs until process exits or stop requested.
pub fn start_clipboard_listener(
    on_clip: impl Fn(String, String, String) + Send + 'static,
) -> anyhow::Result<()> {
    if LISTENER_ACTIVE.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    let on_clip: std::sync::Arc<std::sync::Mutex<Box<dyn Fn(String, String, String) + Send>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Box::new(on_clip)));

    thread::spawn(move || {
        let _ = run_listener(on_clip);
        LISTENER_ACTIVE.store(false, Ordering::SeqCst);
    });

    Ok(())
}

pub fn is_clipboard_listener_running() -> bool {
    LISTENER_ACTIVE.load(Ordering::SeqCst)
}

fn run_listener(on_clip: std::sync::Arc<std::sync::Mutex<Box<dyn Fn(String, String, String) + Send>>>) -> anyhow::Result<()> {
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class_name = windows::core::w!("DigiCoreClipboardListener");

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: WNDCLASS_STYLES::default(),
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance.into(),
            hIcon: HICON::default(),
            hCursor: HCURSOR::default(),
            hbrBackground: HBRUSH::default(),
            lpszMenuName: windows::core::PCWSTR::null(),
            lpszClassName: class_name,
            hIconSm: HICON::default(),
        };

        let atom = RegisterClassExW(&wc);
        if atom == 0 {
            return Err(anyhow::anyhow!("RegisterClassExW failed"));
        }

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            windows::core::w!(""),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            HMENU::default(),
            instance,
            None,
        )
        .map_err(|e| anyhow::anyhow!("CreateWindowExW: {:?}", e))?;

        LISTENER_DATA.with(|cell| {
            *cell.borrow_mut() = Some(ListenerData {
                on_clip: on_clip.clone(),
            });
        });

        MSG_WND.with(|cell| *cell.borrow_mut() = Some(hwnd));

        let mut msg = MSG::default();
        loop {
            let ret = GetMessageW(&mut msg, HWND::default(), 0, 0);
            if ret.as_bool() {
                if ret.0 == -1 {
                    break;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            } else {
                break;
            }
        }

        LISTENER_DATA.with(|cell| *cell.borrow_mut() = None);
        MSG_WND.with(|cell| *cell.borrow_mut() = None);
        Ok(())
    }
}

struct ListenerData {
    on_clip: std::sync::Arc<std::sync::Mutex<Box<dyn Fn(String, String, String) + Send>>>,
}

thread_local! {
    static LISTENER_DATA: std::cell::RefCell<Option<ListenerData>> = std::cell::RefCell::new(None);
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            if AddClipboardFormatListener(hwnd).is_ok() {
                LRESULT(0)
            } else {
                LRESULT(-1)
            }
        }
        WM_CLIPBOARDUPDATE => {
            on_clipboard_update();
            LRESULT(0)
        }
        WM_DESTROY => {
            let _ = RemoveClipboardFormatListener(hwnd);
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn on_clipboard_update() {
    let mut clipboard = match arboard::Clipboard::new() {
        Ok(c) => c,
        Err(_) => return,
    };

    let text = match clipboard.get_text() {
        Ok(t) => {
            if t.is_empty() || t.chars().all(|c| c.is_whitespace()) {
                None
            } else {
                Some(t)
            }
        }
        Err(_) => None,
    };

    let content = if let Some(t) = text {
        t
    } else if clipboard.get_image().is_ok() {
        "[Image]".to_string()
    } else {
        return;
    };

    let (process_name, window_title) = get_foreground_window_context();
    LISTENER_DATA.with(|cell| {
        if let Some(ref data) = *cell.borrow() {
            if let Ok(cb) = data.on_clip.lock() {
                cb(content, process_name, window_title);
            }
        }
    });
}

fn get_foreground_window_context() -> (String, String) {
    digicore_core::adapters::platform::window::WindowsWindowAdapter::new()
        .get_active()
        .map(|ctx| (ctx.process_name, ctx.title))
        .unwrap_or_default()
}
