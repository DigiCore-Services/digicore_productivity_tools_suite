//! Windows clipboard format listener - WM_CLIPBOARDUPDATE.
//!
//! Uses AddClipboardFormatListener with a message-only window to receive
//! clipboard change notifications. At notification time we query the foreground
//! window (source app) for App and Window Title - AHK parity.

use digicore_core::domain::ports::WindowContextPort;
use digicore_core::domain::entities::clipboard_entry::ClipEntry;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{thread, time::Duration};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::HBRUSH;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::DataExchange::{AddClipboardFormatListener, RemoveClipboardFormatListener, IsClipboardFormatAvailable, OpenClipboard, CloseClipboard, GetClipboardData};
const CF_HDROP: u32 = 15;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};

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
    on_clip: impl Fn(ClipEntry) + Send + 'static,
) -> anyhow::Result<()> {
    if LISTENER_ACTIVE.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    let on_clip: std::sync::Arc<std::sync::Mutex<Box<dyn Fn(ClipEntry) + Send>>> =
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

fn run_listener(on_clip: std::sync::Arc<std::sync::Mutex<Box<dyn Fn(ClipEntry) + Send>>>) -> anyhow::Result<()> {
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
            Some(HWND_MESSAGE),
            Some(HMENU::default()),
            Some(instance.into()),
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
            let ret = GetMessageW(&mut msg, None, 0, 0);
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
    on_clip: std::sync::Arc<std::sync::Mutex<Box<dyn Fn(ClipEntry) + Send>>>,
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
    log::debug!("[ClipboardListener] WM_CLIPBOARDUPDATE received");

    let mut clipboard = match arboard::Clipboard::new() {
        Ok(c) => c,
        Err(e) => {
            log::warn!("[ClipboardListener] Failed to initialize arboard: {}", e);
            thread::sleep(Duration::from_millis(100));
            match arboard::Clipboard::new() {
                Ok(c) => c,
                Err(e2) => {
                    log::error!("[ClipboardListener] Final failure to initialize arboard: {}", e2);
                    return;
                }
            }
        }
    };

    let (process_name, window_title) = get_foreground_window_context_internal();
    let (html_content, rtf_content) = crate::application::clipboard_history::get_rich_formats();

    let entry = if let Ok(img) = clipboard.get_image() {
        log::info!("[ClipboardListener] Image detected ({}x{})", img.width, img.height);
        let mut entry = ClipEntry::new(
            "[Image]".to_string(),
            html_content,
            rtf_content,
            process_name,
            window_title,
        );
        entry.entry_type = "image".to_string();
        entry.image_width = Some(img.width as i32);
        entry.image_height = Some(img.height as i32);
        entry.image_bytes = Some(img.bytes.len() as i64);

        // Save image to file
        let images_dir = crate::ports::data_path_resolver::DataPathResolver::clipboard_images_dir();
        let _ = std::fs::create_dir_all(&images_dir);
        let file_id = uuid::Uuid::new_v4().to_string();
        let file_path = images_dir.join(format!("{}.png", file_id));
        if let Ok(mut png_buf) = std::fs::File::create(&file_path) {
            use image::ImageEncoder;
            let _ = image::codecs::png::PngEncoder::new(&mut png_buf)
                .write_image(&img.bytes, img.width as u32, img.height as u32, image::ColorType::Rgba8.into());
            entry.image_path = Some(file_path.to_string_lossy().to_string());
            
            // Generate thumbnail
            let thumb_path = images_dir.join(format!("{}_thumb.png", file_id));
            if let Some(rgba_img) = image::ImageBuffer::<image::Rgba<u8>, Vec<u8>>::from_raw(img.width as u32, img.height as u32, img.bytes.to_vec()) {
                let dynamic_img = image::DynamicImage::ImageRgba8(rgba_img);
                let thumb = dynamic_img.thumbnail(200, 200);
                let _ = thumb.save(&thumb_path);
                entry.thumb_path = Some(thumb_path.to_string_lossy().to_string());
            }
        }
        entry
    } else if let Ok(text) = clipboard.get_text() {
        if text.is_empty() || text.chars().all(|c| c.is_whitespace()) {
            return;
        }
        ClipEntry::new(text, html_content, rtf_content, process_name, window_title)
    } else if unsafe { IsClipboardFormatAvailable(CF_HDROP).is_ok() } {
        let (files, content) = get_file_list_and_content();
        if files.is_empty() {
            return;
        }
        let mut entry = ClipEntry::new(content, html_content, rtf_content, process_name, window_title);
        entry.entry_type = "file_list".to_string();
        entry.file_list = Some(files);
        entry
    } else {
        log::debug!("[ClipboardListener] No supported content on clipboard");
        return;
    };

    LISTENER_DATA.with(|cell| {
        if let Some(ref data) = *cell.borrow() {
            if let Ok(cb) = data.on_clip.lock() {
                cb(entry);
            }
        }
    });
}

pub(crate) fn get_foreground_window_context_internal() -> (String, String) {
    digicore_core::adapters::platform::window::WindowsWindowAdapter::new()
        .get_active()
        .map(|ctx| (ctx.process_name, ctx.title))
        .unwrap_or_default()
}

fn get_file_list_and_content() -> (Vec<String>, String) {
    let mut files = Vec::new();
    unsafe {
        if OpenClipboard(None).is_err() {
            return (files, String::new());
        }
        
        let h_data = GetClipboardData(CF_HDROP);
        if let Ok(h_drop) = h_data {
            let h_drop = HDROP(h_drop.0);
            let count = DragQueryFileW(h_drop, 0xFFFFFFFF, None);
            for i in 0..count {
                let len = DragQueryFileW(h_drop, i, None);
                let mut buffer = vec![0u16; (len + 1) as usize];
                DragQueryFileW(h_drop, i, Some(&mut buffer));
                if let Ok(path) = String::from_utf16(&buffer[..len as usize]) {
                    files.push(path);
                }
            }
        }
        let _ = CloseClipboard();
    }
    
    let content = if files.is_empty() {
        String::new()
    } else if files.len() == 1 {
        files[0].clone()
    } else {
        format!("[{} files]", files.len())
    };
    
    (files, content)
}
