//! Application configuration - Configuration-first design.
//!
//! All configurable values in one place. Loaded from storage/file at startup.
//! Follows Hexagonal: config is a value object; persistence is via a port/adapter.

use serde::{Deserialize, Serialize};

/// Application configuration. Single source of truth for all configurable settings.
/// Load from storage (eframe) or JSON file; persist on change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    // Library
    pub library_path: String,

    // Sync
    pub sync_url: String,
    pub sync_password: String,

    // Templates (F16-F20)
    pub template_date_format: String,
    pub template_time_format: String,

    // Discovery (F60-F69)
    pub discovery_enabled: bool,
    pub discovery_threshold: u32,
    pub discovery_lookback: u32,
    pub discovery_min_len: usize,
    pub discovery_max_len: usize,
    pub discovery_excluded_apps: String,
    pub discovery_excluded_window_titles: String,

    // Ghost Suggestor (F43-F47)
    pub ghost_suggestor_enabled: bool,
    pub ghost_suggestor_debounce_ms: u64,
    pub ghost_suggestor_display_secs: u64,
    pub ghost_suggestor_offset_x: i32,
    pub ghost_suggestor_offset_y: i32,

    // Ghost Follower (F48-F59)
    pub ghost_follower_enabled: bool,
    pub ghost_follower_edge_right: bool,
    pub ghost_follower_monitor_anchor: usize,
    pub ghost_follower_search: String,
    pub ghost_follower_hover_preview: bool,
    pub ghost_follower_collapse_delay_secs: u64,

    // Clipboard
    pub clip_history_max_depth: usize,

    // Script Library (F86)
    pub script_library_run_disabled: bool,
    pub script_library_run_allowlist: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            library_path: String::new(),
            sync_url: String::new(),
            sync_password: String::new(),
            template_date_format: "%Y-%m-%d".to_string(),
            template_time_format: "%H:%M".to_string(),
            discovery_enabled: false,
            discovery_threshold: 2,
            discovery_lookback: 60,
            discovery_min_len: 3,
            discovery_max_len: 50,
            discovery_excluded_apps: String::new(),
            discovery_excluded_window_titles: String::new(),
            ghost_suggestor_enabled: true,
            ghost_suggestor_debounce_ms: 50,
            ghost_suggestor_display_secs: 10,
            ghost_suggestor_offset_x: 0,
            ghost_suggestor_offset_y: 20,
            ghost_follower_enabled: true,
            ghost_follower_edge_right: true,
            ghost_follower_monitor_anchor: 0,
            ghost_follower_search: String::new(),
            ghost_follower_hover_preview: true,
            ghost_follower_collapse_delay_secs: 5,
            clip_history_max_depth: 20,
            script_library_run_disabled: false,
            script_library_run_allowlist: String::new(),
        }
    }
}
