//! AppState - framework-agnostic application state.
//!
//! Phase 0/1: Extracted from TextExpanderApp. Used by all UI adapters (egui, Tauri, Iced).
//! No egui/eframe dependency.

use crate::application::template_processor;
use crate::services::sync_service::SyncResult;
use digicore_core::domain::Snippet;
use std::collections::HashMap;
use std::sync::mpsc;

fn default_kms_graph_pagerank_scope() -> String {
    "auto".to_string()
}

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
        case_sensitive: bool,
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
    pub expansion_log_path: String,
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
    pub ghost_follower: crate::application::ghost_follower::GhostFollowerState,
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

    /// KMS Knowledge Graph: max k for k-means (cap on sqrt(n) heuristic).
    pub kms_graph_k_means_max_k: u32,
    pub kms_graph_k_means_iterations: u32,
    pub kms_graph_ai_beam_max_nodes: u32,
    pub kms_graph_ai_beam_similarity_threshold: f32,
    pub kms_graph_ai_beam_max_edges: u32,
    pub kms_graph_enable_ai_beams: bool,
    pub kms_graph_enable_semantic_clustering: bool,
    /// Prototype kNN + Leiden community layer (default on for smoke testing; toggled in Settings).
    pub kms_graph_enable_leiden_communities: bool,
    /// 0 = no cap (always run semantics when enabled).
    pub kms_graph_semantic_max_notes: u32,
    /// 0 = never show large-vault warning.
    pub kms_graph_warn_note_threshold: u32,
    /// 0 = unlimited pair checks for beam search.
    pub kms_graph_beam_max_pair_checks: u32,
    /// Embedding k-nearest-neighbor edges (`semantic_knn` in DTO); PageRank still uses wiki links only.
    pub kms_graph_enable_semantic_knn_edges: bool,
    pub kms_graph_semantic_knn_per_note: u32,
    pub kms_graph_semantic_knn_min_similarity: f32,
    pub kms_graph_semantic_knn_max_edges: u32,
    /// 0 = unlimited cosine comparisons while building kNN edges (risky on huge vaults).
    pub kms_graph_semantic_knn_max_pair_checks: u32,
    /// When true, KMS graph view may default to paged mode when note count >= threshold.
    pub kms_graph_auto_paging_enabled: bool,
    /// Indexed note count at or above which auto-paged mode applies (if auto_paging_enabled).
    pub kms_graph_auto_paging_note_threshold: u32,
    /// JSON map: vault_graph_settings_key -> partial kms_graph_* overrides.
    pub kms_graph_vault_overrides_json: String,

    /// Screen-space bloom on 3D KMS graphs (UnrealBloomPass).
    pub kms_graph_bloom_enabled: bool,
    pub kms_graph_bloom_strength: f32,
    pub kms_graph_bloom_radius: f32,
    pub kms_graph_bloom_threshold: f32,
    /// Hex constellation backdrop tuning (2D/3D graph chrome).
    pub kms_graph_hex_cell_radius: f32,
    pub kms_graph_hex_layer_opacity: f32,
    pub kms_graph_hex_stroke_width: f32,
    pub kms_graph_hex_stroke_opacity: f32,

    /// Undirected PageRank iterations for the global full-graph build.
    pub kms_graph_pagerank_iterations: u32,
    /// Undirected PageRank iterations for the local neighborhood graph.
    pub kms_graph_pagerank_local_iterations: u32,
    /// PageRank damping factor (typical 0.85).
    pub kms_graph_pagerank_damping: f32,
    /// Global graph: `auto` (paged => page subgraph, else full vault), `full_vault`, `page_subgraph`, `off`.
    pub kms_graph_pagerank_scope: String,
    /// When true, after bulk vault sync the app may run a background job to refresh materialized wiki PageRank (`wiki_pagerank`). Independent of `pagerank_scope` (scope still controls in-request PR).
    pub kms_graph_background_wiki_pagerank_enabled: bool,

    /// Seq 12 Option A: enable default time window (`temporal_default_days`) on graph builds.
    pub kms_graph_temporal_window_enabled: bool,
    /// Default window length in days ending at UTC now when Option A is enabled (0 = no default until RPC sends bounds).
    pub kms_graph_temporal_default_days: u32,
    pub kms_graph_temporal_include_notes_without_mtime: bool,
    /// Seq 12 Option B: expose `edge_recency` on wiki edges in graph DTOs.
    pub kms_graph_temporal_edge_recency_enabled: bool,
    pub kms_graph_temporal_edge_recency_strength: f32,
    pub kms_graph_temporal_edge_recency_half_life_days: f32,

    /// Hybrid/semantic search: drop vector hits below this cosine similarity (0 = disabled).
    pub kms_search_min_similarity: f32,
    /// When true, `kms_search_semantic` includes per-row query embedding timing and effective model id.
    pub kms_search_include_embedding_diagnostics: bool,
    /// Default search mode for KMS Explorer (`Hybrid`, `Semantic`, or `Keyword`).
    pub kms_search_default_mode: String,
    /// Default result limit for KMS hybrid/semantic search (clamped when saving).
    pub kms_search_default_limit: u32,

    /// KMS note text embedding model id (empty string = default fastembed model id).
    pub kms_embedding_model_id: String,
    /// D6: how many notes to re-embed per migration tick (backpressure).
    pub kms_embedding_batch_notes_per_tick: u32,
    /// When true, long note/query text is split into overlapping character chunks; chunk vectors are mean-pooled and L2-normalized into one stored vector per note.
    pub kms_embedding_chunk_enabled: bool,
    /// Maximum characters per chunk (clamped at runtime, typically 256-8192).
    pub kms_embedding_chunk_max_chars: u32,
    /// Overlap between consecutive chunks (clamped to at most half of max chunk size).
    pub kms_embedding_chunk_overlap_chars: u32,

    /// three-spritetext label sharpness: upper cap on devicePixelRatio multiplier (3D graphs).
    pub kms_graph_sprite_label_max_dpr_scale: f32,
    /// Minimum texture scale vs default canvas (helps 1x displays when zooming in).
    pub kms_graph_sprite_label_min_res_scale: f32,
    /// 2D graph: run initial d3-force layout in a WebWorker when node count is at or above this value. 0 = always main thread.
    pub kms_graph_webworker_layout_threshold: u32,
    /// WebWorker simulation: maximum tick budget (capped vs a scaled minimum from node count on the client).
    pub kms_graph_webworker_layout_max_ticks: u32,
    /// WebWorker simulation: stop when alpha falls below this (d3-force alphaMin).
    pub kms_graph_webworker_layout_alpha_min: f32,

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
    pub snippet_editor_case_adaptive: bool,
    pub snippet_editor_case_sensitive: bool,
    pub snippet_editor_smart_suffix: bool,
    pub snippet_editor_is_sensitive: bool,

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

    // Ports
    pub crypto: Option<Box<dyn crate::ports::CryptoPort>>,
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
            expansion_log_path: String::new(),
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
            ghost_follower: crate::application::ghost_follower::GhostFollowerState::new(
                crate::application::ghost_follower::GhostFollowerConfig::default(),
                &HashMap::new(),
            ),
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

            kms_graph_k_means_max_k: 10,
            kms_graph_k_means_iterations: 15,
            kms_graph_ai_beam_max_nodes: 400,
            kms_graph_ai_beam_similarity_threshold: 0.90,
            kms_graph_ai_beam_max_edges: 20,
            kms_graph_enable_ai_beams: true,
            kms_graph_enable_semantic_clustering: true,
            kms_graph_enable_leiden_communities: true,
            kms_graph_semantic_max_notes: 2500,
            kms_graph_warn_note_threshold: 1500,
            kms_graph_beam_max_pair_checks: 200_000,
            kms_graph_enable_semantic_knn_edges: true,
            kms_graph_semantic_knn_per_note: 5,
            kms_graph_semantic_knn_min_similarity: 0.82,
            kms_graph_semantic_knn_max_edges: 8000,
            kms_graph_semantic_knn_max_pair_checks: 400_000,
            kms_graph_auto_paging_enabled: true,
            kms_graph_auto_paging_note_threshold: 2000,
            kms_graph_vault_overrides_json: "{}".to_string(),

            kms_graph_bloom_enabled: true,
            kms_graph_bloom_strength: 0.48,
            kms_graph_bloom_radius: 0.4,
            kms_graph_bloom_threshold: 0.22,
            kms_graph_hex_cell_radius: 2.35,
            kms_graph_hex_layer_opacity: 0.22,
            kms_graph_hex_stroke_width: 0.11,
            kms_graph_hex_stroke_opacity: 0.38,

            kms_graph_pagerank_iterations: 48,
            kms_graph_pagerank_local_iterations: 32,
            kms_graph_pagerank_damping: 0.85,
            kms_graph_pagerank_scope: default_kms_graph_pagerank_scope(),
            kms_graph_background_wiki_pagerank_enabled: true,

            kms_graph_temporal_window_enabled: false,
            kms_graph_temporal_default_days: 0,
            kms_graph_temporal_include_notes_without_mtime: true,
            kms_graph_temporal_edge_recency_enabled: false,
            kms_graph_temporal_edge_recency_strength: 1.0,
            kms_graph_temporal_edge_recency_half_life_days: 30.0,

            kms_search_min_similarity: 0.0,
            kms_search_include_embedding_diagnostics: true,
            kms_search_default_mode: "Hybrid".to_string(),
            kms_search_default_limit: 20,

            kms_embedding_model_id: String::new(),
            kms_embedding_batch_notes_per_tick: 8,
            kms_embedding_chunk_enabled: false,
            kms_embedding_chunk_max_chars: 2048,
            kms_embedding_chunk_overlap_chars: 128,

            kms_graph_sprite_label_max_dpr_scale: 2.5,
            kms_graph_sprite_label_min_res_scale: 1.25,
            kms_graph_webworker_layout_threshold: 800,
            kms_graph_webworker_layout_max_ticks: 450,
            kms_graph_webworker_layout_alpha_min: 0.02,

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
            snippet_editor_case_adaptive: true,
            snippet_editor_case_sensitive: false,
            snippet_editor_smart_suffix: true,
            snippet_editor_is_sensitive: false,
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
            crypto: None,
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
        let mut library = repo.load(path)?;

        // Decrypt sensitive snippets if crypto port available
        if let Some(ref crypto) = self.crypto {
            for snippets in library.values_mut() {
                for snip in snippets {
                    if snip.is_sensitive {
                        if let Some(decrypted) = crypto.decrypt_local(&snip.content) {
                            snip.content = decrypted;
                        }
                    }
                }
            }
        }

        self.library = library;
        self.normalize_library_by_snippet_category();
        self.ghost_follower.update_library(&self.library);
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
        let mut library = if self.library.is_empty() {
            repo.load(path)?
        } else {
            self.library.clone()
        };

        // Encrypt sensitive snippets if crypto port available
        if let Some(ref crypto) = self.crypto {
            for snippets in library.values_mut() {
                for snip in snippets {
                    if snip.is_sensitive && !snip.content.starts_with("ENC:") {
                        if let Ok(encrypted) = crypto.encrypt_local(&snip.content) {
                            snip.content = encrypted;
                        }
                    }
                }
            }
        }

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
        self.ghost_follower.update_library(&self.library);
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
        self.ghost_follower.update_library(&self.library);
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
        self.ghost_follower.update_library(&self.library);
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
        self.ghost_follower.update_library(&self.library);
    }
}
