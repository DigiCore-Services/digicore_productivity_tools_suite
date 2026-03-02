//! Ghost Suggestor (F43-F47): Predictive overlay.
//!
//! Shows snippet suggestions as user types. Tab to accept, Ctrl+Tab to cycle.
//! Debounced suggestions, configurable offset, enable/disable.

use digicore_core::domain::Snippet;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Ghost Suggestor configuration (F46, F47).
#[derive(Clone, Debug)]
pub struct GhostSuggestorConfig {
    /// Enable/disable suggestor (F47).
    pub enabled: bool,
    /// Debounce delay in ms (F44, default 50).
    pub debounce_ms: u64,
    /// Offset X from caret (F46).
    pub offset_x: i32,
    /// Offset Y from caret (F46).
    pub offset_y: i32,
}

impl Default for GhostSuggestorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            debounce_ms: 50,
            offset_x: 0,
            offset_y: 20,
        }
    }
}

/// A suggestion entry (snippet + category).
#[derive(Clone, Debug)]
pub struct Suggestion {
    pub snippet: Snippet,
    pub category: String,
}

/// Ghost Suggestor state.
struct SuggestorState {
    config: GhostSuggestorConfig,
    library: HashMap<String, Vec<Snippet>>,
    buffer: String,
    process_name: String,
    suggestions: Vec<Suggestion>,
    selected_index: usize,
    last_buffer_change: Instant,
    debounce_timer: Option<Instant>,
}

static SUGGESTOR_STATE: Mutex<Option<Arc<Mutex<SuggestorState>>>> = Mutex::new(None);
static SUGGESTOR_ENABLED: AtomicBool = AtomicBool::new(false);

/// Start Ghost Suggestor with config.
pub fn start(config: GhostSuggestorConfig, library: HashMap<String, Vec<Snippet>>) {
    SUGGESTOR_ENABLED.store(config.enabled, Ordering::SeqCst);
    *SUGGESTOR_STATE.lock().unwrap() = Some(Arc::new(Mutex::new(SuggestorState {
        config,
        library,
        buffer: String::new(),
        process_name: String::new(),
        suggestions: Vec::new(),
        selected_index: 0,
        last_buffer_change: Instant::now(),
        debounce_timer: None,
    })));
}

/// Stop Ghost Suggestor.
pub fn stop() {
    SUGGESTOR_ENABLED.store(false, Ordering::SeqCst);
    *SUGGESTOR_STATE.lock().unwrap() = None;
}

/// Update library (when user loads/saves).
pub fn update_library(library: HashMap<String, Vec<Snippet>>) {
    if let Ok(guard) = SUGGESTOR_STATE.lock() {
        if let Some(ref state) = *guard {
            if let Ok(mut s) = state.lock() {
                s.library = library;
                recompute_suggestions(&mut s);
            }
        }
    }
}

/// Get current config (for overlay positioning).
pub fn get_config() -> GhostSuggestorConfig {
    let guard = match SUGGESTOR_STATE.lock() {
        Ok(g) => g,
        Err(_) => return GhostSuggestorConfig::default(),
    };
    let state = match guard.as_ref() {
        Some(s) => s.clone(),
        None => return GhostSuggestorConfig::default(),
    };
    drop(guard);
    state.lock().map(|s| s.config.clone()).unwrap_or_default()
}

/// Update config (e.g. enable/disable, debounce).
pub fn update_config(config: GhostSuggestorConfig) {
    SUGGESTOR_ENABLED.store(config.enabled, Ordering::SeqCst);
    if let Ok(guard) = SUGGESTOR_STATE.lock() {
        if let Some(ref state) = *guard {
            if let Ok(mut s) = state.lock() {
                s.config = config;
            }
        }
    }
}

/// Notify buffer changed (call from hotstring). Triggers debounced suggestion update.
pub fn on_buffer_changed(buffer: &str, process_name: &str) {
    if !SUGGESTOR_ENABLED.load(Ordering::SeqCst) {
        return;
    }

    let guard = match SUGGESTOR_STATE.lock() {
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

    s.buffer = buffer.to_string();
    s.process_name = process_name.to_string();
    s.last_buffer_change = Instant::now();
    s.debounce_timer = Some(Instant::now());
}

/// Check if debounce has elapsed and recompute suggestions. Call periodically from hotstring.
/// Returns true if suggestions changed.
pub fn tick_debounce() -> bool {
    if !SUGGESTOR_ENABLED.load(Ordering::SeqCst) {
        return false;
    }

    let guard = match SUGGESTOR_STATE.lock() {
        Ok(g) => g,
        Err(_) => return false,
    };
    let state = match guard.as_ref() {
        Some(s) => s.clone(),
        None => return false,
    };
    drop(guard);

    let mut s = match state.lock() {
        Ok(s) => s,
        Err(_) => return false,
    };

    let Some(timer) = s.debounce_timer else {
        return false;
    };
    let elapsed = timer.elapsed();
    let debounce = Duration::from_millis(s.config.debounce_ms);
    if elapsed < debounce {
        return false;
    }

    s.debounce_timer = None;
    let had_suggestions = !s.suggestions.is_empty();
    recompute_suggestions(&mut s);
    had_suggestions != !s.suggestions.is_empty()
}

fn recompute_suggestions(s: &mut SuggestorState) {
    s.suggestions.clear();
    s.selected_index = 0;

    let buf = s.buffer.trim();
    if buf.is_empty() {
        return;
    }

    let process = s.process_name.to_lowercase();

    for (category, snippets) in &s.library {
        for snip in snippets {
            // Prefix match: trigger starts with buffer (case-insensitive)
            if snip.trigger.len() >= buf.len()
                && snip.trigger[..buf.len()].eq_ignore_ascii_case(buf)
            {
                if !snip.app_lock.is_empty() {
                    let allowed: Vec<&str> = snip.app_lock.split(',').map(|s| s.trim()).collect();
                    if !allowed.is_empty()
                        && !allowed
                            .iter()
                            .any(|a| process.contains(&a.to_lowercase()))
                    {
                        continue;
                    }
                }
                s.suggestions.push(Suggestion {
                    snippet: snip.clone(),
                    category: category.clone(),
                });
            }
        }
    }

    // Sort: pinned first, then by trigger length (shorter = more specific)
    s.suggestions.sort_by(|a, b| {
        let a_pin = a.snippet.is_pinned();
        let b_pin = b.snippet.is_pinned();
        match (a_pin, b_pin) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.snippet.trigger.len().cmp(&b.snippet.trigger.len()),
        }
    });
}

/// Get current suggestions (for overlay display).
pub fn get_suggestions() -> Vec<Suggestion> {
    let guard = match SUGGESTOR_STATE.lock() {
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
    s.suggestions.clone()
}

/// Get selected index.
pub fn get_selected_index() -> usize {
    let guard = match SUGGESTOR_STATE.lock() {
        Ok(g) => g,
        Err(_) => return 0,
    };
    let state = match guard.as_ref() {
        Some(s) => s.clone(),
        None => return 0,
    };
    drop(guard);

    let s = match state.lock() {
        Ok(s) => s,
        Err(_) => return 0,
    };
    s.selected_index.min(s.suggestions.len().saturating_sub(1))
}

/// Cycle selection (Ctrl+Tab). Returns new index.
pub fn cycle_selection_forward() -> usize {
    let guard = match SUGGESTOR_STATE.lock() {
        Ok(g) => g,
        Err(_) => return 0,
    };
    let state = match guard.as_ref() {
        Some(s) => s.clone(),
        None => return 0,
    };
    drop(guard);

    let mut s = match state.lock() {
        Ok(s) => s,
        Err(_) => return 0,
    };

    if s.suggestions.is_empty() {
        return 0;
    }
    s.selected_index = (s.selected_index + 1) % s.suggestions.len();
    s.selected_index
}

/// Cycle selection backward (Ctrl+Shift+Tab). Returns new index.
pub fn cycle_selection_backward() -> usize {
    let guard = match SUGGESTOR_STATE.lock() {
        Ok(g) => g,
        Err(_) => return 0,
    };
    let state = match guard.as_ref() {
        Some(s) => s.clone(),
        None => return 0,
    };
    drop(guard);

    let mut s = match state.lock() {
        Ok(s) => s,
        Err(_) => return 0,
    };

    if s.suggestions.is_empty() {
        return 0;
    }
    s.selected_index = s
        .selected_index
        .checked_sub(1)
        .unwrap_or(s.suggestions.len() - 1);
    s.selected_index
}

/// Accept selected suggestion. Returns (trigger, content) if any, and clears buffer.
pub fn accept_selected() -> Option<(String, String)> {
    let guard = match SUGGESTOR_STATE.lock() {
        Ok(g) => g,
        Err(_) => return None,
    };
    let state = match guard.as_ref() {
        Some(s) => s.clone(),
        None => return None,
    };
    drop(guard);

    let mut s = match state.lock() {
        Ok(s) => s,
        Err(_) => return None,
    };

    let idx = s.selected_index.min(s.suggestions.len().saturating_sub(1));
    let suggestion = s.suggestions.get(idx).cloned()?;
    s.buffer.clear();
    s.suggestions.clear();
    s.selected_index = 0;

    Some((suggestion.snippet.trigger, suggestion.snippet.content))
}

/// Clear buffer and suggestions (e.g. on Escape or when overlay dismissed).
pub fn dismiss() {
    if let Ok(guard) = SUGGESTOR_STATE.lock() {
        if let Some(ref state) = *guard {
            if let Ok(mut s) = state.lock() {
                s.buffer.clear();
                s.suggestions.clear();
                s.selected_index = 0;
                s.debounce_timer = None;
            }
        }
    }
}

/// Check if suggestor has active suggestions (Tab should be consumed).
pub fn has_suggestions() -> bool {
    !get_suggestions().is_empty()
}

/// Check if suggestor is enabled.
pub fn is_enabled() -> bool {
    SUGGESTOR_ENABLED.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;
    use digicore_core::domain::Snippet;
    use serial_test::serial;
    use std::collections::HashMap;

    #[test]
    fn test_config_default() {
        let config = GhostSuggestorConfig::default();
        assert!(config.enabled);
        assert_eq!(config.debounce_ms, 50);
    }

    #[test]
    #[serial]
    fn test_start_stop() {
        stop();
        assert!(!is_enabled());
        let mut library = HashMap::new();
        library.insert("Cat".to_string(), vec![Snippet::new("sig", "Best regards")]);
        start(GhostSuggestorConfig::default(), library);
        assert!(is_enabled());
        stop();
        assert!(!is_enabled());
    }

    #[test]
    #[serial]
    fn test_has_suggestions_after_buffer() {
        stop();
        let mut library = HashMap::new();
        library.insert(
            "Cat".to_string(),
            vec![Snippet::new("dyf", "Did you find"), Snippet::new("dy", "Yesterday")],
        );
        start(GhostSuggestorConfig { debounce_ms: 0, ..Default::default() }, library);
        on_buffer_changed("dy", "notepad.exe");
        std::thread::sleep(std::time::Duration::from_millis(10));
        let _ = tick_debounce();
        assert!(has_suggestions());
        let suggestions = get_suggestions();
        assert!(suggestions.len() >= 1);
        let accepted = accept_selected();
        assert!(accepted.is_some());
        assert!(!has_suggestions());
        stop();
    }
}
