//! Smart-Discovery (F60-F69): Phrase harvesting from typing.
//!
//! Analyzes typed text for repeated phrases and suggests them as snippets.
//! Configurable threshold, lookback, min/max phrase length, exclusions.

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn debug_log(msg: &str) {
    if std::env::var("DIGICORE_DEBUG").as_deref() != Ok("1") {
        return;
    }
    let path = std::env::temp_dir().join("digicore_debug.log");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{}", msg);
    }
}

/// Discovery configuration (F61-F64).
#[derive(Clone, Debug)]
pub struct DiscoveryConfig {
    /// Minimum repeats to suggest (default 2).
    pub threshold: u32,
    /// Lookback window in minutes (default 60).
    pub lookback_minutes: u32,
    /// Minimum phrase length in chars (default 3).
    pub min_phrase_len: usize,
    /// Maximum phrase length in chars (default 50).
    pub max_phrase_len: usize,
    /// Excluded app process names (e.g. "chrome.exe").
    pub excluded_apps: Vec<String>,
    /// Excluded window title substrings.
    pub excluded_window_titles: Vec<String>,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            threshold: 2,
            lookback_minutes: 60,
            min_phrase_len: 3,
            max_phrase_len: 50,
            excluded_apps: vec![],
            excluded_window_titles: vec![],
        }
    }
}

/// A harvested phrase with timestamp.
#[derive(Clone, Debug)]
struct HarvestEntry {
    phrase: String,
    timestamp: Instant,
}

/// Discovery state - harvest buffer and phrase counts.
struct DiscoveryState {
    config: DiscoveryConfig,
    /// Current typing buffer (until word boundary).
    buffer: String,
    /// Harvested phrases with timestamps (for lookback).
    harvest_log: Vec<HarvestEntry>,
    /// Process name for exclusion check.
    current_process: String,
    /// Window title for exclusion check.
    current_title: String,
}

static DISCOVERY_STATE: Mutex<Option<Arc<Mutex<DiscoveryState>>>> = Mutex::new(None);
static DISCOVERY_ENABLED: AtomicBool = AtomicBool::new(false);

/// Start discovery with given config. Call before or after hotstring starts.
pub fn start(config: DiscoveryConfig) {
    DISCOVERY_ENABLED.store(true, Ordering::SeqCst);
    *DISCOVERY_STATE.lock().unwrap() = Some(Arc::new(Mutex::new(DiscoveryState {
        config,
        buffer: String::new(),
        harvest_log: Vec::new(),
        current_process: String::new(),
        current_title: String::new(),
    })));
}

/// Stop discovery.
pub fn stop() {
    DISCOVERY_ENABLED.store(false, Ordering::SeqCst);
    *DISCOVERY_STATE.lock().unwrap() = None;
    *LAST_SUGGESTION.lock().unwrap() = None;
}

/// Update current window context (call from hotstring when we have context).
pub fn set_window_context(process_name: &str, title: &str) {
    if let Ok(guard) = DISCOVERY_STATE.lock() {
        if let Some(ref state) = *guard {
            if let Ok(mut s) = state.lock() {
                s.current_process = process_name.to_lowercase();
                s.current_title = title.to_lowercase();
            }
        }
    }
}

/// Process a key event. Call from hotstring's on_key.
pub fn on_key(vk_code: u16, ch: Option<char>) {
    if !DISCOVERY_ENABLED.load(Ordering::SeqCst) {
        return;
    }

    let guard = match DISCOVERY_STATE.lock() {
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

    const VK_BACK: u16 = 0x08;
    const VK_SPACE: u16 = 0x20;
    const VK_RETURN: u16 = 0x0D;
    const VK_TAB: u16 = 0x09;

    if vk_code == VK_BACK {
        s.buffer.pop();
        return;
    }

    if vk_code == VK_SPACE {
        s.buffer.push(' ');
        if s.buffer.len() > 256 {
            s.buffer.remove(0);
        }
        return;
    }

    if vk_code == VK_RETURN || vk_code == VK_TAB {
        debug_log(&format!("discovery: enter/tab - harvesting phrase buffer={:?}", s.buffer));
        harvest_and_analyze(&mut s);
        s.buffer.clear();
        return;
    }

    if let Some(c) = ch.filter(|c| !c.is_control()) {
        s.buffer.push(c);
        if s.buffer.len() > 256 {
            s.buffer.remove(0);
        }
    }
}

fn harvest_and_analyze(s: &mut DiscoveryState) {
    let phrase = s.buffer.trim().to_string();
    debug_log(&format!(
        "discovery: harvest_and_analyze buffer={:?} phrase={:?} len={} min={} max={}",
        s.buffer, phrase, phrase.len(), s.config.min_phrase_len, s.config.max_phrase_len
    ));
    if phrase.len() < s.config.min_phrase_len || phrase.len() > s.config.max_phrase_len {
        debug_log(&format!("discovery: skip phrase (len out of range)"));
        return;
    }

    if is_excluded(s) {
        debug_log(&format!(
            "discovery: skip phrase (excluded) process={} title={}",
            s.current_process, s.current_title
        ));
        return;
    }

    let now = Instant::now();
    let lookback = Duration::from_secs(s.config.lookback_minutes as u64 * 60);

    s.harvest_log.push(HarvestEntry {
        phrase: phrase.clone(),
        timestamp: now,
    });

    s.harvest_log.retain(|e| now.duration_since(e.timestamp) <= lookback);

    let count = s
        .harvest_log
        .iter()
        .filter(|e| e.phrase == phrase)
        .count() as u32;

    debug_log(&format!(
        "discovery: phrase={:?} count={} threshold={}",
        phrase, count, s.config.threshold
    ));

    if count >= s.config.threshold {
        debug_log(&format!("discovery: SUGGEST phrase={:?} count={}", phrase, count));
        suggest_phrase(&phrase, count);
    }
}

fn is_excluded(s: &DiscoveryState) -> bool {
    for app in &s.config.excluded_apps {
        if s.current_process.contains(&app.to_lowercase()) {
            return true;
        }
    }
    for title in &s.config.excluded_window_titles {
        if s.current_title.contains(&title.to_lowercase()) {
            return true;
        }
    }
    false
}

/// Called when we have a suggestion. Override via set_suggestion_callback.
static SUGGESTION_CALLBACK: Mutex<Option<Box<dyn Fn(&str, u32) + Send>>> = Mutex::new(None);

static LAST_SUGGESTION: Mutex<Option<(String, u32)>> = Mutex::new(None);

/// Set callback for toast/notification when phrase is suggested.
pub fn set_suggestion_callback<F>(f: F)
where
    F: Fn(&str, u32) + Send + 'static,
{
    *SUGGESTION_CALLBACK.lock().unwrap() = Some(Box::new(f));
}

fn suggest_phrase(phrase: &str, count: u32) {
    *LAST_SUGGESTION.lock().unwrap() = Some((phrase.to_string(), count));
    show_toast(phrase, count);
    if let Ok(cb) = SUGGESTION_CALLBACK.lock() {
        if let Some(ref f) = *cb {
            f(phrase, count);
        }
    }
}

/// Show Windows toast in lower-right corner (AHK-style pop-up).
fn show_toast(phrase: &str, count: u32) {
    use winrt_toast_reborn::{Text, Toast, ToastManager};
    let body = format!("Add \"{}\" as snippet? (typed {}x)", phrase, count);
    debug_log(&format!("discovery: show_toast phrase={:?}", phrase));
    let manager = ToastManager::new(ToastManager::POWERSHELL_AUM_ID);
    let mut toast = Toast::new();
    toast
        .text1("DigiCore Discovery")
        .text2(Text::new(&body));
    match manager.show(&toast) {
        Ok(()) => debug_log("discovery: toast shown OK"),
        Err(e) => debug_log(&format!("discovery: toast FAILED: {:?}", e)),
    }
}

/// Take the last suggestion for UI display. Returns None if none pending.
pub fn take_suggestion() -> Option<(String, u32)> {
    std::mem::take(&mut *LAST_SUGGESTION.lock().unwrap())
}

/// Check if discovery is enabled.
pub fn is_enabled() -> bool {
    DISCOVERY_ENABLED.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_config_default() {
        let config = DiscoveryConfig::default();
        assert_eq!(config.threshold, 2);
        assert_eq!(config.lookback_minutes, 60);
        assert_eq!(config.min_phrase_len, 3);
        assert_eq!(config.max_phrase_len, 50);
    }

    #[test]
    #[serial]
    fn test_start_stop() {
        stop();
        assert!(!is_enabled());
        start(DiscoveryConfig::default());
        assert!(is_enabled());
        stop();
        assert!(!is_enabled());
    }

    #[test]
    #[serial]
    fn test_harvest_suggests_on_repeat() {
        stop();
        start(DiscoveryConfig {
            threshold: 2,
            lookback_minutes: 60,
            min_phrase_len: 3,
            max_phrase_len: 50,
            excluded_apps: vec![],
            excluded_window_titles: vec![],
        });
        set_window_context("notepad.exe", "Test");

        let simulate_phrase = |phrase: &str| {
            for c in phrase.chars() {
                if c == ' ' {
                    on_key(0x20, Some(' '));
                } else {
                    on_key(0, Some(c));
                }
            }
        };
        let enter = || on_key(0x0D, None);

        simulate_phrase("hey");
        enter();
        assert!(take_suggestion().is_none());

        simulate_phrase("hey");
        enter();
        let s = take_suggestion();
        assert!(s.is_some());
        let (phrase, count) = s.unwrap();
        assert_eq!(phrase, "hey");
        assert_eq!(count, 2);

        stop();
    }

    #[test]
    #[serial]
    fn test_harvest_full_phrase_on_enter() {
        stop();
        start(DiscoveryConfig {
            threshold: 2,
            lookback_minutes: 60,
            min_phrase_len: 5,
            max_phrase_len: 50,
            excluded_apps: vec![],
            excluded_window_titles: vec![],
        });
        set_window_context("notepad.exe", "Test");

        let simulate_phrase = |phrase: &str| {
            for c in phrase.chars() {
                if c == ' ' {
                    on_key(0x20, Some(' '));
                } else {
                    on_key(0, Some(c));
                }
            }
        };
        let enter = || on_key(0x0D, None);

        simulate_phrase("hello how are you");
        enter();
        assert!(take_suggestion().is_none());

        simulate_phrase("hello how are you");
        enter();
        let s = take_suggestion();
        assert!(s.is_some());
        let (phrase, count) = s.unwrap();
        assert_eq!(phrase, "hello how are you");
        assert_eq!(count, 2);

        stop();
    }
}
