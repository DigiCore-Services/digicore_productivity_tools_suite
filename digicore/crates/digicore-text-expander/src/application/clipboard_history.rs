//! Clipboard History (F38-F42).
//!
//! Real-time clipboard monitoring, configurable depth, metadata, promote to snippet, dedup.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// A clipboard history entry (F38, F40).
#[derive(Clone, Debug)]
pub struct ClipEntry {
    pub content: String,
    pub process_name: String,
    pub window_title: String,
    pub timestamp: Instant,
}

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
            max_depth: 20,
        }
    }
}

struct ClipboardHistoryState {
    config: ClipboardHistoryConfig,
    entries: Vec<ClipEntry>,
    last_content: Option<String>,
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
pub fn start(config: ClipboardHistoryConfig) {
    CLIP_ENABLED.store(config.enabled, Ordering::SeqCst);
    #[allow(unused_mut)]
    let mut entries = Vec::new();
    #[allow(unused_mut)]
    let mut last_content = None;

    #[cfg(not(test))]
    if config.enabled {
        // Seed from current clipboard. Retry once after short delay (clipboard may not be ready at startup).
        for attempt in 0..2 {
            if attempt > 0 {
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                match clipboard.get_text() {
                    Ok(text) => {
                        if !text.is_empty() && !text.chars().all(|c| c.is_whitespace()) {
                            let len = text.len();
                            entries.push(ClipEntry {
                                content: text.clone(),
                                process_name: String::new(),
                                window_title: String::new(),
                                timestamp: Instant::now(),
                            });
                            last_content = Some(text);
                            log::info!(
                                "[ClipboardHistory] seeded from current clipboard: {} chars (attempt {})",
                                len,
                                attempt + 1
                            );
                            break;
                        } else if attempt == 1 {
                            log::info!("[ClipboardHistory] clipboard empty or whitespace-only after retry");
                        }
                    }
                    Err(e) => {
                        if attempt == 1 {
                            log::warn!("[ClipboardHistory] failed to read clipboard for seed: {}", e);
                        }
                    }
                }
            } else if attempt == 1 {
                log::warn!("[ClipboardHistory] failed to create Clipboard for seed");
            }
        }
    }

    *CLIP_STATE.lock().unwrap() = Some(Arc::new(Mutex::new(ClipboardHistoryState {
        config: config.clone(),
        entries,
        last_content,
    })));

    if config.enabled {
        #[cfg(target_os = "windows")]
        {
            if let Err(_) = crate::platform::windows_clipboard_listener::start_clipboard_listener(
                move |text, process_name, window_title| {
                    if !is_suppressed() {
                        add_entry(text, &process_name, &window_title);
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
        if let Ok(text) = clipboard.get_text() {
            if !text.is_empty() && !text.chars().all(|c| c.is_whitespace()) {
                let mut guard = match state.lock() {
                    Ok(g) => g,
                    Err(_) => break,
                };
                if guard.config.enabled {
                    if guard.last_content.as_ref() != Some(&text) {
                        guard.last_content = Some(text.clone());
                        add_entry_inner(&mut guard, text, String::new(), String::new());
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(500));
    }
}

fn add_entry_inner(
    state: &mut ClipboardHistoryState,
    content: String,
    process_name: String,
    window_title: String,
) {
    if content != "[Image]" && state.entries.first().map(|e| e.content.as_str()) == Some(content.as_str()) {
        return;
    }
    state.entries.insert(
        0,
        ClipEntry {
            content: content.clone(),
            process_name,
            window_title,
            timestamp: Instant::now(),
        },
    );
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

/// Add entry (called when we have window context). F40: app + window title metadata.
pub fn add_entry(content: String, process_name: &str, window_title: &str) {
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
    if content != "[Image]" && s.last_content.as_ref() == Some(&content) {
        return;
    }
    s.last_content = Some(content.clone());
    add_entry_inner(&mut s, content, process_name.to_string(), window_title.to_string());
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
        if let Some(ref state) = *guard {
            if let Ok(mut s) = state.lock() {
                if index < s.entries.len() {
                    s.entries.remove(index);
                }
            }
        }
    }
}

/// Clear all clipboard history entries.
pub fn clear_all() {
    if let Ok(guard) = CLIP_STATE.lock() {
        if let Some(ref state) = *guard {
            if let Ok(mut s) = state.lock() {
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
        assert_eq!(config.max_depth, 20);
    }

    #[test]
    #[serial]
    fn test_start_stop() {
        stop();
        assert!(!is_enabled());
        start(ClipboardHistoryConfig::default());
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
        });
        add_entry("hello".to_string(), "notepad.exe", "Test");
        add_entry("hello".to_string(), "notepad.exe", "Test");
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
        });
        add_entry("a".to_string(), "app", "win");
        add_entry("b".to_string(), "app", "win");
        add_entry("c".to_string(), "app", "win");
        add_entry("d".to_string(), "app", "win");
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
        start(ClipboardHistoryConfig::default());
        let result = take_promote_pending();
        assert!(result.is_none());
        stop();
    }

    #[test]
    #[serial]
    fn test_request_take_promote() {
        stop();
        start(ClipboardHistoryConfig::default());
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
        });
        add_entry("hello".to_string(), "notepad.exe", "Test");
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
        });
        add_entry("a".to_string(), "app", "win");
        add_entry("b".to_string(), "app", "win");
        add_entry("c".to_string(), "app", "win");
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
        });
        add_entry("x".to_string(), "app", "win");
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
        });
        add_entry("a".to_string(), "app", "win");
        add_entry("b".to_string(), "app", "win");
        add_entry("c".to_string(), "app", "win");
        let entries = get_entries();
        assert_eq!(entries.len(), 3);

        clear_all();
        let entries = get_entries();
        assert!(entries.is_empty());

        add_entry("new".to_string(), "app", "win");
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
        });
        add_entry("content from notepad".to_string(), "notepad.exe", "Untitled - Notepad");
        add_entry("content from terminal".to_string(), "WindowsTerminal.exe", "PowerShell");
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
        });
        add_entry("a".to_string(), "app", "win");
        add_entry("b".to_string(), "app", "win");
        add_entry("c".to_string(), "app", "win");
        add_entry("d".to_string(), "app", "win");
        add_entry("e".to_string(), "app", "win");
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
        });
        assert!(is_enabled());
        update_config(ClipboardHistoryConfig {
            enabled: false,
            max_depth: 5,
        });
        assert!(!is_enabled());
        add_entry("should not add".to_string(), "app", "win");
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
        });
        suppress_for_duration(Duration::from_millis(10));
        add_entry("entry".to_string(), "app", "win");
        let entries = get_entries();
        assert_eq!(entries.len(), 1);
        stop();
    }
}
