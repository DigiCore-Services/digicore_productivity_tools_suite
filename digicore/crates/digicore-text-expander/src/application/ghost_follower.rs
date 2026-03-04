//! Ghost Follower (F48-F59): Edge ribbon.
//!
//! Edge-anchored ribbon showing pinned snippets. Double-click to insert.
//! WS_EX_NOACTIVATE to avoid stealing focus.

use digicore_core::domain::Snippet;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Edge to anchor the ribbon (F48).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FollowerEdge {
    Left,
    Right,
}

impl Default for FollowerEdge {
    fn default() -> Self {
        Self::Right
    }
}

/// Monitor to anchor the ribbon (F49: multi-monitor).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MonitorAnchor {
    /// Primary monitor.
    Primary,
    /// Secondary (non-primary) monitor.
    Secondary,
    /// Monitor containing the cursor.
    Current,
}

impl Default for MonitorAnchor {
    fn default() -> Self {
        Self::Primary
    }
}

/// Ghost Follower configuration.
#[derive(Clone, Debug)]
pub struct GhostFollowerConfig {
    /// Enable/disable.
    pub enabled: bool,
    /// Edge to anchor (F48).
    pub edge: FollowerEdge,
    /// Monitor to anchor (F49).
    pub monitor_anchor: MonitorAnchor,
    /// Search filter (F51).
    pub search_filter: String,
    /// F53: Show full content on hover.
    pub hover_preview: bool,
    /// F54-F55: Collapse to pill after delay (seconds). 0 = disabled.
    pub collapse_delay_secs: u64,
}

impl Default for GhostFollowerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            edge: FollowerEdge::Right,
            monitor_anchor: MonitorAnchor::Primary,
            search_filter: String::new(),
            hover_preview: true,
            collapse_delay_secs: 5,
        }
    }
}

/// Ghost Follower state.
struct FollowerState {
    config: GhostFollowerConfig,
    library: HashMap<String, Vec<Snippet>>,
    /// Pinned snippets only (F50). (snippet, category, snippet_idx)
    pinned: Vec<(Snippet, String, usize)>,
}

static FOLLOWER_STATE: Mutex<Option<Arc<Mutex<FollowerState>>>> = Mutex::new(None);
static FOLLOWER_ENABLED: AtomicBool = AtomicBool::new(false);
static FOLLOWER_SEARCH: Mutex<String> = Mutex::new(String::new());
static FOLLOWER_COLLAPSED: Mutex<bool> = Mutex::new(false);
static FOLLOWER_LAST_ACTIVE: Mutex<Option<std::time::Instant>> = Mutex::new(None);

/// Start Ghost Follower with config and library.
pub fn start(config: GhostFollowerConfig, library: HashMap<String, Vec<Snippet>>) {
    FOLLOWER_ENABLED.store(config.enabled, Ordering::SeqCst);
    let pinned = collect_pinned(&library);
    *FOLLOWER_STATE.lock().unwrap() = Some(Arc::new(Mutex::new(FollowerState {
        config,
        library,
        pinned,
    })));
}

/// Stop Ghost Follower.
pub fn stop() {
    FOLLOWER_ENABLED.store(false, Ordering::SeqCst);
    *FOLLOWER_STATE.lock().unwrap() = None;
}

/// Update library and recompute pinned list.
pub fn update_library(library: HashMap<String, Vec<Snippet>>) {
    if let Ok(guard) = FOLLOWER_STATE.lock() {
        if let Some(ref state) = *guard {
            if let Ok(mut s) = state.lock() {
                s.library = library;
                s.pinned = collect_pinned(&s.library);
            }
        }
    }
}

/// Update config.
pub fn update_config(config: GhostFollowerConfig) {
    FOLLOWER_ENABLED.store(config.enabled, Ordering::SeqCst);
    if let Ok(guard) = FOLLOWER_STATE.lock() {
        if let Some(ref state) = *guard {
            if let Ok(mut s) = state.lock() {
                s.config = config;
                s.pinned = collect_pinned(&s.library);
            }
        }
    }
}

/// Get config for UI (hover_preview, collapse_delay).
pub fn get_config() -> GhostFollowerConfig {
    let guard = match FOLLOWER_STATE.lock() {
        Ok(g) => g,
        Err(_) => return GhostFollowerConfig::default(),
    };
    let state = match guard.as_ref() {
        Some(s) => s.clone(),
        None => return GhostFollowerConfig::default(),
    };
    drop(guard);
    state.lock().map(|s| s.config.clone()).unwrap_or_default()
}

fn collect_pinned(library: &HashMap<String, Vec<Snippet>>) -> Vec<(Snippet, String, usize)> {
    let mut result = Vec::new();
    for (category, snippets) in library {
        for (idx, snip) in snippets.iter().enumerate() {
            if snip.is_pinned() {
                result.push((snip.clone(), category.clone(), idx));
            }
        }
    }
    result.sort_by(|a, b| a.0.trigger.cmp(&b.0.trigger));
    result
}

/// Get clipboard history entries for display (F50).
pub fn get_clipboard_entries() -> Vec<super::clipboard_history::ClipEntry> {
    super::clipboard_history::get_entries()
}

/// Get pinned snippets for display, filtered by search (F50, F51).
/// Returns (Snippet, category, snippet_idx).
pub fn get_pinned_snippets(filter: &str) -> Vec<(Snippet, String, usize)> {
    let guard = match FOLLOWER_STATE.lock() {
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

    let filter_lower = filter.to_lowercase();
    if filter_lower.is_empty() {
        return s.pinned.clone();
    }
    s.pinned
        .iter()
        .filter(|(snip, cat, _)| {
            snip.trigger.to_lowercase().contains(&filter_lower)
                || snip.content.to_lowercase().contains(&filter_lower)
                || cat.to_lowercase().contains(&filter_lower)
        })
        .cloned()
        .collect()
}

/// Check if Ghost Follower is enabled.
pub fn is_enabled() -> bool {
    FOLLOWER_ENABLED.load(Ordering::SeqCst)
}

/// Get current search filter (F51).
pub fn get_search_filter() -> String {
    FOLLOWER_SEARCH
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default()
}

/// Set search filter (F51).
pub fn set_search_filter(filter: &str) {
    if let Ok(mut s) = FOLLOWER_SEARCH.lock() {
        *s = filter.to_string();
    }
}

/// F54: Check if ribbon is collapsed.
pub fn is_collapsed() -> bool {
    FOLLOWER_COLLAPSED.lock().map(|g| *g).unwrap_or(false)
}

/// F54: Set collapsed state.
pub fn set_collapsed(collapsed: bool) {
    if let Ok(mut g) = FOLLOWER_COLLAPSED.lock() {
        *g = collapsed;
    }
}

/// F55: Notify user activity (reset collapse timer).
pub fn touch() {
    if let Ok(mut g) = FOLLOWER_LAST_ACTIVE.lock() {
        *g = Some(std::time::Instant::now());
    }
}

/// F55: Check if should collapse (no activity for delay).
pub fn should_collapse(delay_secs: u64) -> bool {
    if delay_secs == 0 {
        return false;
    }
    let last = FOLLOWER_LAST_ACTIVE.lock().ok().and_then(|g| *g);
    let Some(last) = last else {
        touch();
        return false;
    };
    last.elapsed() >= std::time::Duration::from_secs(delay_secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use digicore_core::domain::Snippet;
    use serial_test::serial;
    use std::collections::HashMap;

    #[test]
    fn test_config_default() {
        let config = GhostFollowerConfig::default();
        assert!(config.enabled);
        assert_eq!(config.edge, FollowerEdge::Right);
    }

    #[test]
    #[serial]
    fn test_start_stop() {
        stop();
        assert!(!is_enabled());
        let mut library = HashMap::new();
        let mut snip = Snippet::new("sig", "Best regards");
        snip.pinned = "true".to_string();
        library.insert("Cat".to_string(), vec![snip]);
        start(GhostFollowerConfig::default(), library);
        assert!(is_enabled());
        let pinned = get_pinned_snippets("");
        assert_eq!(pinned.len(), 1);
        assert_eq!(pinned[0].0.trigger, "sig");
        stop();
        assert!(!is_enabled());
    }

    #[test]
    #[serial]
    fn test_search_filter() {
        stop();
        let mut library = HashMap::new();
        let mut snip = Snippet::new("sig", "Best regards");
        snip.pinned = "true".to_string();
        library.insert("Cat".to_string(), vec![snip]);
        start(GhostFollowerConfig::default(), library);
        set_search_filter("sig");
        let pinned = get_pinned_snippets(&get_search_filter());
        assert_eq!(pinned.len(), 1);
        set_search_filter("xyz");
        let pinned = get_pinned_snippets(&get_search_filter());
        assert_eq!(pinned.len(), 0);
        stop();
    }
}
