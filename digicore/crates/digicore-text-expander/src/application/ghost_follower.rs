//! Ghost Follower (F48-F59): Edge ribbon.
//!
//! Edge-anchored ribbon showing pinned snippets. Double-click to insert.
//! WS_EX_NOACTIVATE to avoid stealing focus.

use digicore_core::domain::Snippet;
use std::collections::HashMap;
use std::sync::Mutex;
use once_cell::sync::Lazy;

static GLOBAL_STATE: Lazy<Mutex<Option<GhostFollowerState>>> = Lazy::new(|| Mutex::new(None));

/// Edge to anchor the ribbon (F48).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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

/// Feature Mode: Edge-Anchored vs Floating Bubble.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FollowerMode {
    EdgeAnchored,
    FloatingBubble,
}

impl Default for FollowerMode {
    fn default() -> Self {
        Self::EdgeAnchored
    }
}

/// Expansion Trigger: Click vs Hover.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ExpandTrigger {
    Click,
    Hover,
}

impl Default for ExpandTrigger {
    fn default() -> Self {
        Self::Click
    }
}

/// Ghost Follower configuration.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct GhostFollowerConfig {
    /// Enable/disable.
    pub enabled: bool,
    /// Edge to anchor (F48).
    pub edge: FollowerEdge,
    /// Monitor to anchor (F49).
    pub monitor_anchor: MonitorAnchor,
    /// Feature Mode (Edge vs Bubble).
    pub mode: FollowerMode,
    /// Expansion Trigger (Click vs Hover).
    pub expand_trigger: ExpandTrigger,
    /// Delay before auto-expanding on hover (ms).
    pub expand_delay_ms: u64,
    /// F54-F55: Collapse to pill after delay (seconds). 0 = disabled.
    pub collapse_delay_secs: u64,
    /// F53: Show full content on hover.
    pub hover_preview: bool,
    /// Maximum clipboard entries to display in follower.
    pub clipboard_depth: usize,
    /// Opacity (0-100).
    pub opacity: u32,
    /// Saved window position.
    pub position: Option<(i32, i32)>,
}

impl Default for GhostFollowerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            edge: FollowerEdge::Right,
            monitor_anchor: MonitorAnchor::Primary,
            mode: FollowerMode::EdgeAnchored,
            expand_trigger: ExpandTrigger::Click,
            expand_delay_ms: 500,
            collapse_delay_secs: 5,
            hover_preview: true,
            clipboard_depth: 20,
            opacity: 100,
            position: None,
        }
    }
}

/// Ghost Follower runtime state.
#[derive(Clone, Debug)]
pub struct GhostFollowerState {
    pub config: GhostFollowerConfig,
    pub collapsed: bool,
    pub search_filter: String,
    pub last_active: Option<std::time::Instant>,
    pub last_target_hwnd: Option<isize>,
    /// Pinned snippets cache. (snippet, category, snippet_idx)
    pub pinned: Vec<(Snippet, String, usize)>,
}

impl GhostFollowerState {
    pub fn new(config: GhostFollowerConfig, library: &HashMap<String, Vec<Snippet>>) -> Self {
        let pinned = collect_pinned(library);
        Self {
            config,
            collapsed: true,
            search_filter: String::new(),
            last_active: None,
            last_target_hwnd: None,
            pinned,
        }
    }

    pub fn update_library(&mut self, library: &HashMap<String, Vec<Snippet>>) {
        self.pinned = collect_pinned(library);
    }

    pub fn update_config(&mut self, config: GhostFollowerConfig) {
        self.config = config;
    }

    pub fn touch(&mut self) {
        self.last_active = Some(std::time::Instant::now());
    }

    pub fn should_collapse(&self) -> bool {
        let delay = self.config.collapse_delay_secs;
        if delay == 0 || self.collapsed {
            return false;
        }
        let Some(last) = self.last_active else {
            return false;
        };
        last.elapsed() >= std::time::Duration::from_secs(delay)
    }
}

/// Global entry points for drivers (hotstring.rs) that don't have access to AppState.
/// These synchronize with the state managed by the host app.

pub fn start(config: GhostFollowerConfig, library: HashMap<String, Vec<Snippet>>) {
    let mut guard = GLOBAL_STATE.lock().unwrap();
    *guard = Some(GhostFollowerState::new(config, &library));
}

pub fn update_library(library: HashMap<String, Vec<Snippet>>) {
    if let Ok(mut guard) = GLOBAL_STATE.lock() {
        if let Some(state) = guard.as_mut() {
            state.update_library(&library);
        }
    }
}

pub fn update_config(config: GhostFollowerConfig) {
    if let Ok(mut guard) = GLOBAL_STATE.lock() {
        if let Some(state) = guard.as_mut() {
            state.update_config(config);
        }
    }
}

/// Capture the current foreground window as the insert target.
#[cfg(target_os = "windows")]
pub fn capture_target_window(state: &mut GhostFollowerState) {
    let fg = crate::platform::windows_window::describe_foreground_window();
    if let Some(hwnd) = crate::platform::windows_window::capture_strict_external_foreground_hwnd() {
        state.last_target_hwnd = Some(hwnd);
        let captured = crate::platform::windows_window::describe_hwnd(hwnd);
        log::debug!(
            "[GhostFollowerTarget] capture_target_window: foreground={} captured={}",
            fg, captured
        );
    } else {
        log::info!(
            "[GhostFollowerTarget] capture_target_window: foreground={} captured=<none> (strict)",
            fg
        );
    }
}

/// Global wrapper for capture_target_window.
pub fn capture_target_window_global() {
    if let Ok(mut guard) = GLOBAL_STATE.lock() {
        if let Some(state) = guard.as_mut() {
            capture_target_window(state);
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn capture_target_window(_state: &mut GhostFollowerState) {}

/// Capture target window for tray/quick-search launch where foreground is often tray UI.
#[cfg(target_os = "windows")]
pub fn capture_target_window_for_quick_search_launch(state: &mut GhostFollowerState) {
    let fg = crate::platform::windows_window::describe_foreground_window();
    if let Some(hwnd) =
        crate::platform::windows_window::capture_recent_external_foreground_hwnd(1500)
    {
        state.last_target_hwnd = Some(hwnd);
        let captured = crate::platform::windows_window::describe_hwnd(hwnd);
        log::debug!(
            "[QuickSearchTarget] capture_for_launch: foreground={} captured={}",
            fg, captured
        );
    } else {
        log::info!(
            "[QuickSearchTarget] capture_for_launch: foreground={} captured=<none>",
            fg
        );
    }
}

#[cfg(not(target_os = "windows"))]
pub fn capture_target_window_for_quick_search_launch(_state: &mut GhostFollowerState) {}

/// Global wrapper for capture_target_window_for_quick_search_launch.
pub fn capture_target_window_for_quick_search_launch_global() {
    if let Ok(mut guard) = GLOBAL_STATE.lock() {
        if let Some(state) = guard.as_mut() {
            capture_target_window_for_quick_search_launch(state);
        }
    }
}

/// Take the stored target window for insert. Returns None if not captured.
pub fn take_target_hwnd_global() -> Option<isize> {
    if let Ok(mut guard) = GLOBAL_STATE.lock() {
        if let Some(ref mut state) = *guard {
            return state.last_target_hwnd.take();
        }
    }
    None
}

/// Take the stored target window for insert from instance state.
pub fn take_target_hwnd(state: &mut GhostFollowerState) -> Option<isize> {
    state.last_target_hwnd.take()
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
pub fn get_clipboard_entries() -> Vec<digicore_core::domain::entities::clipboard_entry::ClipEntry> {
    crate::application::clipboard_history::get_entries()
}

/// Get pinned snippets for display, filtered by search (F50, F51).
/// Returns (Snippet, category, snippet_idx).
pub fn get_pinned_snippets(state: &GhostFollowerState, filter: &str) -> Vec<(Snippet, String, usize)> {
    let filter_lower = filter.to_lowercase();
    if filter_lower.is_empty() {
        return state.pinned.clone();
    }
    state.pinned
        .iter()
        .filter(|(snip, cat, _)| {
            snip.trigger.to_lowercase().contains(&filter_lower)
                || snip.content.to_lowercase().contains(&filter_lower)
                || cat.to_lowercase().contains(&filter_lower)
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use digicore_core::domain::Snippet;
    use std::collections::HashMap;

    #[test]
    fn test_config_default() {
        let config = GhostFollowerConfig::default();
        assert!(config.enabled);
        assert_eq!(config.edge, FollowerEdge::Right);
    }

    #[test]
    fn test_state_lifecycle() {
        let mut library = HashMap::new();
        let mut snip = Snippet::new("sig", "Best regards");
        snip.pinned = "true".to_string();
        library.insert("Cat".to_string(), vec![snip]);
        
        let mut state = GhostFollowerState::new(GhostFollowerConfig::default(), &library);
        assert!(state.config.enabled);
        let pinned = get_pinned_snippets(&state, "");
        assert_eq!(pinned.len(), 1);
        assert_eq!(pinned[0].0.trigger, "sig");
    }

    #[test]
    fn test_search_filter() {
        let mut library = HashMap::new();
        let mut snip = Snippet::new("sig", "Best regards");
        snip.pinned = "true".to_string();
        library.insert("Cat".to_string(), vec![snip]);
        
        let state = GhostFollowerState::new(GhostFollowerConfig::default(), &library);
        let pinned = get_pinned_snippets(&state, "sig");
        assert_eq!(pinned.len(), 1);
        let pinned = get_pinned_snippets(&state, "xyz");
        assert_eq!(pinned.len(), 0);
    }
}
