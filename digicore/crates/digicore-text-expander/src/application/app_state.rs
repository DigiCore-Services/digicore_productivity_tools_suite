//! AppState - framework-agnostic application state.
//!
//! Phase 0/1: Extracted from TextExpanderApp. Used by all UI adapters (egui, Tauri, Iced).
//! No egui/eframe dependency.

use crate::application::template_processor;
use crate::services::sync_service::SyncResult;
use digicore_core::domain::Snippet;
use std::collections::HashMap;
use std::sync::mpsc;

/// Active tab index.
#[derive(Clone, Copy, PartialEq)]
pub enum Tab {
    Library = 0,
    Configuration = 1,
    ClipboardHistory = 2,
    ScriptLibrary = 3,
}

/// View Full Content modal source - determines which action buttons to show.
#[derive(Clone)]
pub enum ClipViewContent {
    /// From Clipboard History - show "Promote to Snippet" and "Close"
    ClipboardHistory { content: String },
    /// From Snippet Library - show "Edit Snippet" and "Close" (already a snippet)
    SnippetLibrary {
        category: String,
        snippet_idx: usize,
        trigger: String,
        content: String,
        options: String,
        snippet_category: String,
        profile: String,
        app_lock: String,
        pinned: bool,
    },
}

/// Add new snippet or edit existing.
#[derive(Clone)]
pub enum SnippetEditorMode {
    Add { category: String },
    Edit { category: String, snippet_idx: usize },
    /// Promote from clipboard - uses Edit Snippet modal with "Promote to Snippet" title.
    Promote { category: String },
}

/// State for in-window variable input when Preview Expansion has interactive vars.
#[derive(Clone)]
pub struct SnippetTestVarState {
    pub content: String,
    pub vars: Vec<template_processor::InteractiveVar>,
    pub values: HashMap<String, String>,
    pub choice_indices: HashMap<String, usize>,
    pub checkbox_checked: HashMap<String, bool>,
}

/// Framework-agnostic application state.
/// Used by all UI adapters (egui, Tauri, Iced).
pub struct AppState {
    // Library
    pub library_path: String,
    pub library: HashMap<String, Vec<Snippet>>,
    pub categories: Vec<String>,
    pub selected_category: Option<usize>,
    pub status: String,
    pub active_tab: Tab,

    // Sync
    pub sync_url: String,
    pub sync_password: String,
    pub sync_status: SyncResult,
    pub sync_rx: Option<mpsc::Receiver<(SyncResult, bool)>>,
    pub startup_sync_done: bool,

    // Expansion
    pub expansion_paused: bool,

    // Auto-load library once on startup (when path exists)
    pub initial_load_attempted: bool,

    // Ensure window shown on first frame (override persistence-restored minimized state)
    pub window_visibility_ensured: bool,

    // Ensure Ghost Follower viewport visible on first show
    pub ghost_follower_visibility_ensured: bool,

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
    pub ghost_suggestor_snooze_duration_mins: u64,
    pub ghost_suggestor_offset_x: i32,
    pub ghost_suggestor_offset_y: i32,

    // Ghost Follower (F48-F59)
    pub ghost_follower_enabled: bool,
    pub ghost_follower_edge_right: bool,
    pub ghost_follower_monitor_anchor: usize,
    pub ghost_follower_search: String,
    pub ghost_follower_hover_preview: bool,
    pub ghost_follower_collapse_delay_secs: u64,
    /// Opacity 10-100 (percent). 100 = fully opaque.
    pub ghost_follower_opacity: u32,
    /// Saved window position (x, y) when user drags. None = use edge/monitor.
    pub ghost_follower_position: Option<(i32, i32)>,
    pub clip_history_max_depth: usize,

    // Templates (F16-F20)
    pub template_date_format: String,
    pub template_time_format: String,

    // Snippet Editor (Add/Edit/Promote)
    pub snippet_editor_mode: Option<SnippetEditorMode>,
    pub snippet_editor_trigger: String,
    pub snippet_editor_content: String,
    pub snippet_editor_options: String,
    pub snippet_editor_category: String,
    pub snippet_editor_profile: String,
    pub snippet_editor_app_lock: String,
    pub snippet_editor_pinned: bool,
    pub snippet_editor_save_clicked: bool,
    pub snippet_editor_modal_open: bool,
    pub snippet_editor_template_selected: usize,

    // Delete confirmation
    pub snippet_delete_confirm: Option<(String, usize)>,
    pub snippet_delete_dialog_open: bool,

    // Snippet Library search
    pub library_search: String,

    // View Full Content modal
    pub clip_view_content: Option<ClipViewContent>,
    pub clip_delete_confirm: Option<usize>,
    pub clip_delete_dialog_open: bool,
    pub clip_clear_confirm_open: bool,

    // Script Library tab (F86)
    pub script_library_run_disabled: bool,
    pub script_library_run_allowlist: String,
    pub script_library_js_content: String,
    pub script_library_loaded: bool,
    pub script_library_py_content: String,
    pub script_library_lua_content: String,

    // Preview Expansion
    pub snippet_test_result: Option<String>,
    pub snippet_test_var_pending: Option<SnippetTestVarState>,
    pub snippet_test_result_modal_open: bool,
    pub snippet_test_var_modal_open: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            library_path: String::new(),
            library: HashMap::new(),
            categories: Vec::new(),
            selected_category: None,
            status: "Ready".to_string(),
            active_tab: Tab::Library,
            sync_url: String::new(),
            sync_password: String::new(),
            sync_status: SyncResult::Idle,
            sync_rx: None,
            startup_sync_done: false,
            expansion_paused: false,
            initial_load_attempted: false,
            window_visibility_ensured: false,
            ghost_follower_visibility_ensured: false,
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
            ghost_suggestor_snooze_duration_mins: 5,
            ghost_suggestor_offset_x: 0,
            ghost_suggestor_offset_y: 20,
            ghost_follower_enabled: true,
            ghost_follower_edge_right: true,
            ghost_follower_monitor_anchor: 0,
            ghost_follower_search: String::new(),
            ghost_follower_hover_preview: true,
            ghost_follower_collapse_delay_secs: 5,
            ghost_follower_opacity: 100,
            ghost_follower_position: None,
            clip_history_max_depth: 20,
            template_date_format: "%Y-%m-%d".to_string(),
            template_time_format: "%H:%M".to_string(),
            snippet_editor_mode: None,
            snippet_editor_trigger: String::new(),
            snippet_editor_content: String::new(),
            snippet_editor_options: "*:".to_string(),
            snippet_editor_category: String::new(),
            snippet_editor_profile: "Work".to_string(),
            snippet_editor_app_lock: String::new(),
            snippet_editor_pinned: false,
            snippet_editor_save_clicked: false,
            snippet_editor_modal_open: false,
            snippet_editor_template_selected: 0,
            snippet_delete_confirm: None,
            snippet_delete_dialog_open: false,
            library_search: String::new(),
            clip_view_content: None,
            clip_delete_confirm: None,
            clip_delete_dialog_open: false,
            clip_clear_confirm_open: false,
            script_library_run_disabled: false,
            script_library_run_allowlist: String::new(),
            script_library_js_content: String::new(),
            script_library_loaded: false,
            script_library_py_content: String::new(),
            script_library_lua_content: String::new(),
            snippet_test_result: None,
            snippet_test_var_pending: None,
            snippet_test_result_modal_open: false,
            snippet_test_var_modal_open: false,
        }
    }
}

impl AppState {
    /// Create with default values. Caller loads from StoragePort and populates.
    pub fn new() -> Self {
        Self::default()
    }

    /// Try to load library from disk into this state. Returns number of categories on success.
    /// Used by Tauri and other UI adapters that need to load without TextExpanderApp.
    pub fn try_load_library(&mut self) -> anyhow::Result<usize> {
        use digicore_core::adapters::persistence::JsonLibraryAdapter;
        use digicore_core::domain::ports::SnippetRepository;
        use std::path::Path;

        if self.library_path.is_empty() {
            return Ok(0);
        }
        let path = Path::new(&self.library_path);
        let repo = JsonLibraryAdapter;
        let library = repo.load(path)?;
        self.library = library;
        self.normalize_library_by_snippet_category();
        self.selected_category = None;
        self.initial_load_attempted = true;
        Ok(self.categories.len())
    }

    /// Try to save library to disk. Uses in-memory library; if empty, loads from path first to avoid overwriting with empty.
    /// Used by Tauri and other UI adapters.
    pub fn try_save_library(&mut self) -> anyhow::Result<()> {
        use digicore_core::adapters::persistence::JsonLibraryAdapter;
        use digicore_core::domain::ports::SnippetRepository;
        use std::path::Path;

        if self.library_path.is_empty() {
            return Err(anyhow::anyhow!("Library path is empty"));
        }
        let path = Path::new(&self.library_path);
        let repo = JsonLibraryAdapter;
        let library = if self.library.is_empty() {
            repo.load(path)?
        } else {
            self.library.clone()
        };
        repo.save(path, &library)?;
        Ok(())
    }

    /// Add a snippet to the library. Category is created if missing. Sets last_modified.
    /// Call try_save_library to persist.
    pub fn add_snippet(&mut self, category: &str, snippet: &Snippet) {
        let cat = if category.trim().is_empty() {
            "General".to_string()
        } else {
            category.trim().to_string()
        };
        let mut snip = snippet.clone();
        snip.last_modified = chrono::Utc::now()
            .format("%Y%m%d%H%M%S%.3f")
            .to_string()
            .replace('.', "");
        if snip.category.trim().is_empty() {
            snip.category = cat.clone();
        }
        self.library.entry(cat.clone()).or_default().push(snip);
        if !self.categories.contains(&cat) {
            self.categories.push(cat);
            self.categories.sort();
        }
    }

    /// Update a snippet at the given category and index. May move to new category if snippet.category differs.
    /// Sets last_modified. Call try_save_library to persist.
    pub fn update_snippet(
        &mut self,
        old_category: &str,
        snippet_idx: usize,
        snippet: &Snippet,
    ) -> anyhow::Result<()> {
        let snips = self
            .library
            .get_mut(old_category)
            .ok_or_else(|| anyhow::anyhow!("Category not found: {}", old_category))?;
        if snippet_idx >= snips.len() {
            return Err(anyhow::anyhow!("Snippet index out of range: {}", snippet_idx));
        }
        let mut new_snip = snippet.clone();
        new_snip.category = if snippet.category.trim().is_empty() {
            old_category.to_string()
        } else {
            snippet.category.trim().to_string()
        };
        new_snip.last_modified = chrono::Utc::now()
            .format("%Y%m%d%H%M%S%.3f")
            .to_string()
            .replace('.', "");
        snips.remove(snippet_idx);
        if snips.is_empty() {
            self.library.remove(old_category);
        }
        let cat = new_snip.category.clone();
        self.library.entry(cat.clone()).or_default().push(new_snip);
        if !self.categories.contains(&cat) {
            self.categories.push(cat);
            self.categories.sort();
        } else {
            self.normalize_library_by_snippet_category();
        }
        Ok(())
    }

    /// Delete a snippet at the given category and index. Removes category if empty.
    /// Call try_save_library to persist.
    pub fn delete_snippet(&mut self, category: &str, snippet_idx: usize) -> anyhow::Result<()> {
        let snips = self
            .library
            .get_mut(category)
            .ok_or_else(|| anyhow::anyhow!("Category not found: {}", category))?;
        if snippet_idx >= snips.len() {
            return Err(anyhow::anyhow!("Snippet index out of range: {}", snippet_idx));
        }
        snips.remove(snippet_idx);
        if snips.is_empty() {
            self.library.remove(category);
        }
        self.categories = self.library.keys().cloned().collect();
        self.categories.sort();
        Ok(())
    }

    /// Re-group library by each snippet's `category` field (not JSON structure keys).
    fn normalize_library_by_snippet_category(&mut self) {
        let mut regrouped: HashMap<String, Vec<Snippet>> = HashMap::new();
        for snippets in self.library.values() {
            for snip in snippets {
                let cat = if snip.category.trim().is_empty() {
                    "General".to_string()
                } else {
                    snip.category.trim().to_string()
                };
                regrouped.entry(cat).or_default().push(snip.clone());
            }
        }
        self.library = regrouped;
        self.categories = self.library.keys().cloned().collect();
        self.categories.sort();
    }
}
