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
    pub library: HashMap<String, Vec<Snippet>>,
    pub library_path: String,
    pub kms_vault_path: String,
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
    pub ghost_follower_opacity: u32,
    /// Saved window position (x, y) when user drags. None = use edge/monitor.
    pub ghost_follower_position: Option<(i32, i32)>,
    pub clip_history_max_depth: usize,

    // Corpus Generation (F55)
    pub corpus_enabled: bool,
    pub corpus_output_dir: String,
    pub corpus_snapshot_dir: String,
    pub corpus_shortcut_modifiers: u16,
    pub corpus_shortcut_key: u16,

    // Extraction Config (F55)
    pub extraction_row_overlap_tolerance: f32,
    pub extraction_cluster_threshold_factor: f32,
    pub extraction_zone_proximity: f32,
    pub extraction_cross_zone_gap_factor: f32,
    pub extraction_same_zone_gap_factor: f32,
    pub extraction_significant_gap_gate: f32,
    pub extraction_char_width_factor: f32,
    pub extraction_bridged_threshold: f32,
    pub extraction_word_spacing_factor: f32,
    pub extraction_layout_row_lookback: usize,
    pub extraction_layout_table_break_threshold: f32,
    pub extraction_layout_paragraph_break_threshold: f32,
    pub extraction_layout_max_space_clamp: usize,

    pub extraction_footer_triggers: String,
    pub extraction_table_min_contiguous_rows: usize,
    pub extraction_table_min_avg_segments: f32,
    pub extraction_tables_column_jitter_tolerance: f32,
    pub extraction_tables_merge_y_gap_max: f32,
    pub extraction_tables_merge_y_gap_min: f32,

    pub extraction_adaptive_plaintext_cluster_factor: f32,
    pub extraction_adaptive_plaintext_gap_gate: f32,
    pub extraction_adaptive_table_cluster_factor: f32,
    pub extraction_adaptive_table_gap_gate: f32,
    pub extraction_adaptive_column_cluster_factor: f32,
    pub extraction_adaptive_column_gap_gate: f32,
    pub extraction_adaptive_plaintext_cross_factor: f32,
    pub extraction_adaptive_table_cross_factor: f32,
    pub extraction_adaptive_column_cross_factor: f32,

    pub extraction_refinement_entropy_threshold: f32,
    pub extraction_refinement_cluster_threshold_modifier: f32,
    pub extraction_refinement_cross_zone_gap_modifier: f32,

    pub extraction_classifier_gutter_weight: f32,
    pub extraction_classifier_density_weight: f32,
    pub extraction_classifier_multicolumn_density_max: f32,
    pub extraction_classifier_table_density_min: f32,
    pub extraction_classifier_table_entropy_min: f32,

    pub extraction_columns_min_contiguous_rows: usize,
    pub extraction_columns_gutter_gap_factor: f32,
    pub extraction_columns_gutter_void_tolerance: f32,
    pub extraction_columns_edge_margin_tolerance: f32,

    pub extraction_headers_max_width_ratio: f32,
    pub extraction_headers_centered_tolerance: f32,
    pub extraction_headers_h1_size_multiplier: f32,
    pub extraction_headers_h2_size_multiplier: f32,
    pub extraction_headers_h3_size_multiplier: f32,

    pub extraction_scoring_jitter_penalty_weight: f32,
    pub extraction_scoring_size_penalty_weight: f32,
    pub extraction_scoring_low_confidence_threshold: f32,

    // Templates (F16-F20)
    pub template_date_format: String,
    pub template_time_format: String,

    // Snippet Editor (Add/Edit/Promote)
    pub snippet_editor_mode: Option<SnippetEditorMode>,
    pub snippet_editor_trigger: String,
    pub snippet_editor_content: String,
    pub snippet_editor_trigger_type: String,
    pub snippet_editor_html_content: Option<String>,
    pub snippet_editor_rtf_content: Option<String>,
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
            library: HashMap::new(),
            library_path: String::new(),
            kms_vault_path: String::new(),
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

            // Corpus
            corpus_enabled: true,
            corpus_output_dir: "docs/sample-ocr-images".to_string(),
            corpus_snapshot_dir: "crates/digicore-text-expander/tests/snapshots".to_string(),
            corpus_shortcut_modifiers: 1 | 2 | 4, // 1=Ctrl, 2=Alt, 4=Shift
            corpus_shortcut_key: 0x43, // 'C'

            // Extraction Config Defaults
            extraction_row_overlap_tolerance: 0.6,
            extraction_cluster_threshold_factor: 0.45,
            extraction_zone_proximity: 15.0,
            extraction_cross_zone_gap_factor: 0.25,
            extraction_same_zone_gap_factor: 0.8,
            extraction_significant_gap_gate: 0.8,
            extraction_char_width_factor: 0.45,
            extraction_bridged_threshold: 0.4,
            extraction_word_spacing_factor: 0.2,
            extraction_layout_row_lookback: 5,
            extraction_layout_table_break_threshold: 3.0,
            extraction_layout_paragraph_break_threshold: 3.0,
            extraction_layout_max_space_clamp: 6,

            extraction_footer_triggers: "total,sum,subtotal,balance".to_string(),
            extraction_table_min_contiguous_rows: 4,
            extraction_table_min_avg_segments: 3.1,
            extraction_tables_column_jitter_tolerance: 20.0,
            extraction_tables_merge_y_gap_max: 100.0,
            extraction_tables_merge_y_gap_min: 40.0,

            extraction_adaptive_plaintext_cluster_factor: 1.1,
            extraction_adaptive_plaintext_gap_gate: 0.5,
            extraction_adaptive_table_cluster_factor: 0.45,
            extraction_adaptive_table_gap_gate: 1.2,
            extraction_adaptive_column_cluster_factor: 0.45,
            extraction_adaptive_column_gap_gate: 0.8,
            extraction_adaptive_plaintext_cross_factor: 1.0,
            extraction_adaptive_table_cross_factor: 0.25,
            extraction_adaptive_column_cross_factor: 0.8,

            extraction_refinement_entropy_threshold: 50.0,
            extraction_refinement_cluster_threshold_modifier: 0.8,
            extraction_refinement_cross_zone_gap_modifier: 1.2,

            extraction_classifier_gutter_weight: 15.0,
            extraction_classifier_density_weight: 10.0,
            extraction_classifier_multicolumn_density_max: 0.4,
            extraction_classifier_table_density_min: 1.0,
            extraction_classifier_table_entropy_min: 40.0,

            extraction_columns_min_contiguous_rows: 3,
            extraction_columns_gutter_gap_factor: 5.0,
            extraction_columns_gutter_void_tolerance: 0.7,
            extraction_columns_edge_margin_tolerance: 30.0,

            extraction_headers_max_width_ratio: 0.75,
            extraction_headers_centered_tolerance: 0.12,
            extraction_headers_h1_size_multiplier: 1.6,
            extraction_headers_h2_size_multiplier: 1.3,
            extraction_headers_h3_size_multiplier: 1.2,

            extraction_scoring_jitter_penalty_weight: 0.4,
            extraction_scoring_size_penalty_weight: 0.1,
            extraction_scoring_low_confidence_threshold: 0.6,

            template_date_format: "%Y-%m-%d".to_string(),
            template_time_format: "%H:%M".to_string(),
            snippet_editor_mode: None,
            snippet_editor_trigger: String::new(),
            snippet_editor_content: String::new(),
            snippet_editor_trigger_type: "suffix".to_string(),
            snippet_editor_html_content: None,
            snippet_editor_rtf_content: None,
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
