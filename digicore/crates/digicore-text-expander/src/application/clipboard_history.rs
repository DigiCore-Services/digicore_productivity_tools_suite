//! Clipboard History (F38-F42).
//!
//! Real-time clipboard monitoring, configurable depth, metadata, promote to snippet, dedup.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use digicore_core::domain::entities::clipboard_entry::ClipEntry;
use digicore_core::domain::ports::clipboard_repository::ClipboardRepository;

/// Clipboard history configuration (F39).
#[derive(Clone, Debug)]
pub struct ClipboardHistoryConfig {
    pub enabled: bool,
    pub max_depth: usize,
}

impl Default for ClipboardHistoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_depth: 200,
        }
    }
}

struct ClipboardHistoryState {
    config: ClipboardHistoryConfig,
    entries: Vec<ClipEntry>,
    last_content: Option<String>,
    repo: Option<Arc<dyn ClipboardRepository>>,
}

static CLIP_STATE: Mutex<Option<Arc<Mutex<ClipboardHistoryState>>>> = Mutex::new(None);
static CLIP_ENABLED: AtomicBool = AtomicBool::new(false);
static CLIP_THREAD: Mutex<Option<thread::JoinHandle<()>>> = Mutex::new(None);
type EntryObserver = Arc<dyn Fn(&ClipEntry) + Send + Sync>;
static ENTRY_OBSERVER: Mutex<Option<EntryObserver>> = Mutex::new(None);

pub fn set_entry_observer(observer: Option<EntryObserver>) {
    if let Ok(mut guard) = ENTRY_OBSERVER.lock() {
        *guard = observer;
    }
}

/// Start clipboard history monitoring (F38).
/// On Windows: uses WM_CLIPBOARDUPDATE listener (AHK parity - captures App/Window Title).
/// On other platforms: uses poll loop.
/// Seeds with current clipboard content on startup so existing content is visible.
pub fn start(config: ClipboardHistoryConfig, repo: Option<Arc<dyn ClipboardRepository>>) {
    CLIP_ENABLED.store(config.enabled, Ordering::SeqCst);
    let mut entries = Vec::new();
    let mut last_content = None;

    // Load from repository if available
    if let Some(ref r) = repo {
        match r.load_last_n(config.max_depth) {
            Ok(loaded) => {
                entries = loaded;
                if let Some(first) = entries.first() {
                    last_content = Some(first.content.clone());
                }
                log::info!("[ClipboardHistory] loaded {} entries from persistence", entries.len());
            }
            Err(e) => log::error!("[ClipboardHistory] failed to load from persistence: {}", e),
        }
    }

    #[cfg(not(test))]
    if config.enabled && entries.is_empty() {
        // Seed from current clipboard if empty
        for attempt in 0..2 {
            if attempt > 0 {
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                match clipboard.get_text() {
                    Ok(text) => {
                        if !text.is_empty() && !text.chars().all(|c| c.is_whitespace()) {
                            let entry = ClipEntry::new(
                                text.clone(),
                                None,
                                None,
                                String::new(),
                                String::new(),
                            );
                            if let Some(ref r) = repo {
                                let _ = r.save(&entry);
                            }
                            last_content = Some(text);
                            entries.push(entry);
                            break;
                        }
                    }
                    Err(_) => {}
                }
            }
        }
    }

    *CLIP_STATE.lock().unwrap() = Some(Arc::new(Mutex::new(ClipboardHistoryState {
        config: config.clone(),
        entries,
        last_content,
        repo,
    })));

    if config.enabled {
        #[cfg(target_os = "windows")]
        {
            if let Err(_) = crate::platform::windows_clipboard_listener::start_clipboard_listener(
                move |entry| {
                    if !is_suppressed() {
                        add_entry(entry);
                    }
                },
            ) {
                // Fallback to poll loop if listener fails
                let state = CLIP_STATE.lock().unwrap().clone().unwrap();
                let handle = thread::spawn(move || run_poll_loop(state));
                *CLIP_THREAD.lock().unwrap() = Some(handle);
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let state = CLIP_STATE.lock().unwrap().clone().unwrap();
            let handle = thread::spawn(move || run_poll_loop(state));
            *CLIP_THREAD.lock().unwrap() = Some(handle);
        }
    }
}

/// Stop clipboard history.
pub fn stop() {
    CLIP_ENABLED.store(false, Ordering::SeqCst);
    if let Ok(mut h) = CLIP_THREAD.lock() {
        if let Some(handle) = h.take() {
            let _ = handle.join();
        }
    }
    *CLIP_STATE.lock().unwrap() = None;
}

fn run_poll_loop(state: Arc<Mutex<ClipboardHistoryState>>) {
    let mut clipboard = match arboard::Clipboard::new() {
        Ok(c) => c,
        Err(_) => return,
    };

    while CLIP_ENABLED.load(Ordering::SeqCst) {
        if is_suppressed() {
            thread::sleep(Duration::from_millis(100));
            continue;
        }
        
        // Handle images in poll loop
        if let Ok(img) = clipboard.get_image() {
            let mut guard = match state.lock() {
                Ok(g) => g,
                Err(_) => break,
            };
            // Simplistic image dedup: check bytes len and dimensions
            let is_dup = guard.entries.first().map_or(false, |e| {
                e.entry_type == "image" && e.image_bytes == Some(img.bytes.len() as i64) && e.image_width == Some(img.width as i32)
            });

            if !is_dup {
                let (html, rtf) = get_rich_formats();
                #[cfg(target_os = "windows")]
                let (process_name, window_title) = crate::platform::windows_clipboard_listener::get_foreground_window_context_internal();
                #[cfg(not(target_os = "windows"))]
                let (process_name, window_title) = (String::new(), String::new());

                let mut entry = ClipEntry::new("[Image]".to_string(), html, rtf, process_name, window_title);
                entry.entry_type = "image".to_string();
                entry.image_width = Some(img.width as i32);
                entry.image_height = Some(img.height as i32);
                entry.image_bytes = Some(img.bytes.len() as i64);
                
                // Save image (Dry logic for now, similar to windows_listener)
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

                add_entry_inner(&mut guard, entry);
            }
        } else if let Ok(text) = clipboard.get_text() {
            if !text.is_empty() && !text.chars().all(|c| c.is_whitespace()) {
                let mut guard = match state.lock() {
                    Ok(g) => g,
                    Err(_) => break,
                };
                if guard.config.enabled {
                    if !is_fuzzy_duplicate(guard.last_content.as_deref().unwrap_or(""), &text) {
                        guard.last_content = Some(text.clone());
                        let (html, rtf) = get_rich_formats();
                        #[cfg(target_os = "windows")]
                        let (process_name, window_title) = crate::platform::windows_clipboard_listener::get_foreground_window_context_internal();
                        #[cfg(not(target_os = "windows"))]
                        let (process_name, window_title) = (String::new(), String::new());

                        let entry = ClipEntry::new(text, html, rtf, process_name, window_title);
                        add_entry_inner(&mut guard, entry);
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(1000));
    }
}

fn add_entry_inner(
    state: &mut ClipboardHistoryState,
    entry: ClipEntry,
) {
    if let Some(first) = state.entries.first() {
        if is_fuzzy_duplicate(&first.content, &entry.content) {
            return;
        }
    }

    // Save to repository
    if let Some(ref r) = state.repo {
        if let Err(e) = r.save(&entry) {
            log::error!("[ClipboardHistory] failed to persist entry: {}", e);
        }
    }

    state.entries.insert(0, entry);
    
    if let Ok(guard) = ENTRY_OBSERVER.lock() {
        if let Some(cb) = guard.as_ref() {
            if let Some(inserted) = state.entries.first() {
                cb(inserted);
            }
        }
    }

    while state.entries.len() > state.config.max_depth {
        state.entries.pop();
    }
}

/// Add entry directly.
pub fn add_entry(entry: ClipEntry) {
    if !CLIP_ENABLED.load(Ordering::SeqCst) {
        return;
    }
    let guard = match CLIP_STATE.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    let state = match guard.as_ref() {
        Some(s) => s.clone(),
        None => return,
    };
    drop(guard);

    let mut s = match state.lock() {
        Ok(s) => s,
        Err(_) => return,
    };
    
    if !is_fuzzy_duplicate(s.last_content.as_deref().unwrap_or(""), &entry.content) {
        s.last_content = Some(entry.content.clone());
        add_entry_inner(&mut s, entry);
    }
}

fn is_fuzzy_duplicate(a: &str, b: &str) -> bool {
    if a == "[Image]" || b == "[Image]" { return false; }
    if a == b { return true; }
    if a.is_empty() || b.is_empty() { return false; }
    
    // Quick check: if lengths are wildly different, not a fuzzy duplicate
    let len_a = a.len();
    let len_b = b.len();
    let max_len = len_a.max(len_b);
    let diff = (len_a as isize - len_b as isize).abs();
    
    if diff > (max_len as f32 * 0.1) as isize {
        return false;
    }

    let dist = strsim::levenshtein(a, b);
    let similarity = 1.0 - (dist as f32 / max_len as f32);
    similarity > 0.92 // 92% similarity threshold
}

#[cfg(target_os = "windows")]
pub(crate) fn get_rich_formats() -> (Option<String>, Option<String>) {
    use windows::Win32::System::DataExchange::{OpenClipboard, CloseClipboard, GetClipboardData, RegisterClipboardFormatW};
    use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
    use windows::Win32::Foundation::HGLOBAL;
    use windows::core::PCWSTR;

    unsafe {
        if OpenClipboard(None).is_err() {
            return (None, None);
        }
        
        let html_name: Vec<u16> = "HTML Format\0".encode_utf16().collect();
        let format_html = RegisterClipboardFormatW(PCWSTR(html_name.as_ptr()));
        
        let rtf_name: Vec<u16> = "Rich Text Format\0".encode_utf16().collect();
        let format_rtf = RegisterClipboardFormatW(PCWSTR(rtf_name.as_ptr()));
        
        let mut html = None;
        let mut rtf = None;
        
        if format_html != 0 {
            if let Ok(h) = GetClipboardData(format_html) {
                let h_glob = HGLOBAL(h.0);
                let ptr = GlobalLock(h_glob);
                if !ptr.is_null() {
                    // C-style string reading (HTML Format is UTF-8)
                    let bytes = std::ffi::CStr::from_ptr(ptr as *const i8).to_bytes();
                    html = Some(String::from_utf8_lossy(bytes).into_owned());
                    let _ = GlobalUnlock(h_glob);
                }
            }
        }
        
        if format_rtf != 0 {
            if let Ok(h) = GetClipboardData(format_rtf) {
                let h_glob = HGLOBAL(h.0);
                let ptr = GlobalLock(h_glob);
                if !ptr.is_null() {
                    let bytes = std::ffi::CStr::from_ptr(ptr as *const i8).to_bytes();
                    rtf = Some(String::from_utf8_lossy(bytes).into_owned());
                    let _ = GlobalUnlock(h_glob);
                }
            }
        }
        
        let _ = CloseClipboard();
        (html, rtf)
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn get_rich_formats() -> (Option<String>, Option<String>) {
    (None, None)
}

/// Get clipboard history entries (most recent first).
pub fn get_entries() -> Vec<ClipEntry> {
    let guard = match CLIP_STATE.lock() {
        Ok(g) => g,
        Err(_) => return vec![],
    };
    let state = match guard.as_ref() {
        Some(s) => s.clone(),
        None => return vec![],
    };
    drop(guard);

    let s = match state.lock() {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    s.entries.clone()
}

/// Update config (e.g. max_depth).
pub fn update_config(config: ClipboardHistoryConfig) {
    CLIP_ENABLED.store(config.enabled, Ordering::SeqCst);
    if let Ok(guard) = CLIP_STATE.lock() {
        if let Some(ref state) = *guard {
            if let Ok(mut s) = state.lock() {
                s.config = config;
                while s.entries.len() > s.config.max_depth {
                    s.entries.pop();
                }
            }
        }
    }
}

/// Check if clipboard history is enabled.
pub fn is_enabled() -> bool {
    CLIP_ENABLED.load(Ordering::SeqCst)
}

/// Take pending promote request (F41). Returns content to add as snippet.
pub fn take_promote_pending() -> Option<String> {
    PROMOTE_PENDING.lock().ok().and_then(|mut g| g.take())
}

static PROMOTE_PENDING: Mutex<Option<String>> = Mutex::new(None);
static SUPPRESS_UNTIL: Mutex<Option<Instant>> = Mutex::new(None);

/// Suppress adding to history (e.g. during our own paste). Call before expansion.
pub fn suppress_for_duration(duration: Duration) {
    if let Ok(mut g) = SUPPRESS_UNTIL.lock() {
        *g = Some(Instant::now() + duration);
    }
}

fn is_suppressed() -> bool {
    SUPPRESS_UNTIL
        .lock()
        .ok()
        .and_then(|g| *g)
        .map(|until| Instant::now() < until)
        .unwrap_or(false)
}

/// Request promote to snippet (F41). Pre-fills for Snippet Editor.
pub fn request_promote(content: String) {
    if let Ok(mut g) = PROMOTE_PENDING.lock() {
        *g = Some(content);
    }
}

/// Remove entry at index (0-based; matches get_entries() order).
pub fn delete_entry_at(index: usize) {
    if let Ok(guard) = CLIP_STATE.lock() {
        if let Some(ref state_arc) = *guard {
            if let Ok(mut s) = state_arc.lock() {
                if index < s.entries.len() {
                    let entry = s.entries.remove(index);
                    if let Some(ref r) = s.repo {
                        let _ = r.delete_at(entry.timestamp);
                    }
                }
            }
        }
    }
}

/// Clear all clipboard history entries.
pub fn clear_all() {
    if let Ok(guard) = CLIP_STATE.lock() {
        if let Some(ref state_arc) = *guard {
            if let Ok(mut s) = state_arc.lock() {
                if let Some(ref r) = s.repo {
                    let _ = r.clear_all();
                }
                s.entries.clear();
                s.last_content = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_config_default() {
        let config = ClipboardHistoryConfig::default();
        assert!(config.enabled);
        assert_eq!(config.max_depth, 200);
    }

    #[test]
    #[serial]
    fn test_start_stop() {
        stop();
        assert!(!is_enabled());
        start(ClipboardHistoryConfig::default(), None);
        assert!(is_enabled());
        stop();
        assert!(!is_enabled());
    }

    #[test]
    #[serial]
    fn test_add_entry_dedup() {
        stop();
        start(ClipboardHistoryConfig {
            enabled: true,
            max_depth: 5,
        }, None);
        add_entry(ClipEntry::new("hello".to_string(), None, None, "notepad.exe".to_string(), "Test".to_string()));
        add_entry(ClipEntry::new("hello".to_string(), None, None, "notepad.exe".to_string(), "Test".to_string()));
        let entries = get_entries();
        assert_eq!(entries.len(), 1);
        stop();
    }

    #[test]
    #[serial]
    fn test_add_entry_max_depth() {
        stop();
        start(ClipboardHistoryConfig {
            enabled: true,
            max_depth: 3,
        }, None);
        add_entry(ClipEntry::new("a".to_string(), None, None, "app".to_string(), "win".to_string()));
        add_entry(ClipEntry::new("b".to_string(), None, None, "app".to_string(), "win".to_string()));
        add_entry(ClipEntry::new("c".to_string(), None, None, "app".to_string(), "win".to_string()));
        add_entry(ClipEntry::new("d".to_string(), None, None, "app".to_string(), "win".to_string()));
        let entries = get_entries();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].content, "d");
        assert_eq!(entries[2].content, "b");
        stop();
    }

    #[test]
    #[serial]
    fn test_take_promote_pending_none() {
        stop();
        start(ClipboardHistoryConfig::default(), None);
        let result = take_promote_pending();
        assert!(result.is_none());
        stop();
    }

    #[test]
    #[serial]
    fn test_request_take_promote() {
        stop();
        start(ClipboardHistoryConfig::default(), None);
        request_promote("snippet content".to_string());
        let result = take_promote_pending();
        assert_eq!(result, Some("snippet content".to_string()));
        let again = take_promote_pending();
        assert!(again.is_none());
        stop();
    }

    #[test]
    #[serial]
    fn test_add_entry_when_disabled() {
        stop();
        start(ClipboardHistoryConfig {
            enabled: false,
            max_depth: 5,
        }, None);
        add_entry(ClipEntry::new("hello".to_string(), None, None, "notepad.exe".to_string(), "Test".to_string()));
        let entries = get_entries();
        assert!(entries.is_empty());
        stop();
    }

    #[test]
    #[serial]
    fn test_get_entries_when_stopped() {
        stop();
        let entries = get_entries();
        assert!(entries.is_empty());
    }

    #[test]
    #[serial]
    fn test_delete_entry_at() {
        stop();
        start(ClipboardHistoryConfig {
            enabled: true,
            max_depth: 10,
        }, None);
        add_entry(ClipEntry::new("a".to_string(), None, None, "app".to_string(), "win".to_string()));
        add_entry(ClipEntry::new("b".to_string(), None, None, "app".to_string(), "win".to_string()));
        add_entry(ClipEntry::new("c".to_string(), None, None, "app".to_string(), "win".to_string()));
        let entries = get_entries();
        assert_eq!(entries.len(), 3);

        delete_entry_at(1);
        let entries = get_entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].content, "c");
        assert_eq!(entries[1].content, "a");

        delete_entry_at(0);
        let entries = get_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].content, "a");

        delete_entry_at(0);
        let entries = get_entries();
        assert!(entries.is_empty());

        stop();
    }

    #[test]
    #[serial]
    fn test_delete_entry_at_out_of_bounds() {
        stop();
        start(ClipboardHistoryConfig {
            enabled: true,
            max_depth: 5,
        }, None);
        add_entry(ClipEntry::new("x".to_string(), None, None, "app".to_string(), "win".to_string()));
        delete_entry_at(99);
        let entries = get_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].content, "x");
        stop();
    }

    #[test]
    #[serial]
    fn test_clear_all() {
        stop();
        start(ClipboardHistoryConfig {
            enabled: true,
            max_depth: 10,
        }, None);
        add_entry(ClipEntry::new("a".to_string(), None, None, "app".to_string(), "win".to_string()));
        add_entry(ClipEntry::new("b".to_string(), None, None, "app".to_string(), "win".to_string()));
        add_entry(ClipEntry::new("c".to_string(), None, None, "app".to_string(), "win".to_string()));
        let entries = get_entries();
        assert_eq!(entries.len(), 3);

        clear_all();
        let entries = get_entries();
        assert!(entries.is_empty());

        add_entry(ClipEntry::new("new".to_string(), None, None, "app".to_string(), "win".to_string()));
        let entries = get_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].content, "new");

        stop();
    }

    #[test]
    #[serial]
    fn test_add_entry_with_metadata() {
        stop();
        start(ClipboardHistoryConfig {
            enabled: true,
            max_depth: 10,
        }, None);
        add_entry(ClipEntry::new("content from notepad".to_string(), None, None, "notepad.exe".to_string(), "Untitled - Notepad".to_string()));
        add_entry(ClipEntry::new("content from terminal".to_string(), None, None, "WindowsTerminal.exe".to_string(), "PowerShell".to_string()));
        let entries = get_entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].content, "content from terminal");
        assert_eq!(entries[0].process_name, "WindowsTerminal.exe");
        assert_eq!(entries[0].window_title, "PowerShell");
        assert_eq!(entries[1].content, "content from notepad");
        assert_eq!(entries[1].process_name, "notepad.exe");
        assert_eq!(entries[1].window_title, "Untitled - Notepad");
        stop();
    }

    #[test]
    #[serial]
    fn test_update_config_max_depth_trims() {
        stop();
        start(ClipboardHistoryConfig {
            enabled: true,
            max_depth: 10,
        }, None);
        add_entry(ClipEntry::new("a".to_string(), None, None, "app".to_string(), "win".to_string()));
        add_entry(ClipEntry::new("b".to_string(), None, None, "app".to_string(), "win".to_string()));
        add_entry(ClipEntry::new("c".to_string(), None, None, "app".to_string(), "win".to_string()));
        add_entry(ClipEntry::new("d".to_string(), None, None, "app".to_string(), "win".to_string()));
        add_entry(ClipEntry::new("e".to_string(), None, None, "app".to_string(), "win".to_string()));
        let entries = get_entries();
        assert_eq!(entries.len(), 5);

        update_config(ClipboardHistoryConfig {
            enabled: true,
            max_depth: 2,
        });
        let entries = get_entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].content, "e");
        assert_eq!(entries[1].content, "d");
        stop();
    }

    #[test]
    #[serial]
    fn test_update_config_disabled() {
        stop();
        start(ClipboardHistoryConfig {
            enabled: true,
            max_depth: 5,
        }, None);
        assert!(is_enabled());
        update_config(ClipboardHistoryConfig {
            enabled: false,
            max_depth: 5,
        });
        assert!(!is_enabled());
        add_entry(ClipEntry::new("should not add".to_string(), None, None, "app".to_string(), "win".to_string()));
        let entries = get_entries();
        assert!(entries.is_empty());
        stop();
    }

    #[test]
    #[serial]
    fn test_suppress_for_duration_no_panic() {
        stop();
        start(ClipboardHistoryConfig {
            enabled: true,
            max_depth: 5,
        }, None);
        suppress_for_duration(Duration::from_millis(10));
        add_entry(ClipEntry::new("entry".to_string(), None, None, "app".to_string(), "win".to_string()));
        let entries = get_entries();
        assert_eq!(entries.len(), 1);
        stop();
    }
}
