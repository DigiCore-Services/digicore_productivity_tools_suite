//! DigiCore Text Expander - Tauri backend.
//!
//! Invokes digicore-text-expander library. Tauri commands provide load/save/get_app_state
//! for the web frontend.

#![recursion_limit = "256"]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_diagnostics;
mod app_settings_storage;
mod settings_bundle_model;
mod scripting_signer_registry;
mod app_shell;
mod fs_util;
mod kms_ipc_boundary;
mod taurpc_ipc_types;
mod api;
mod appearance_enforcement;
mod clipboard_text_persistence;
mod clipboard_sqlite_sync;
mod clipboard_repository;
mod kms_repository;
mod kms_link_adjacency_cache;
mod kms_graph_service;
mod kms_graph_build_ring;
mod kms_graph_effective_params;
mod kms_note_tags;
mod kms_graph_ports;
mod kms_error;
mod kms_service;
mod kms_diagnostic_service;
mod kms_embed_diagnostic_log;
mod embedding_service;
mod embedding_pipeline;
mod kms_embedding_migrate;
mod indexing_service;
mod skill_sync;
mod kms_sync_service;
mod kms_sync_orchestration;
mod kms_watcher;
mod kms_git_service;

use crate::api::Api;
use crate::app_settings_storage::{persist_settings_for_state, save_all_on_exit};

use digicore_core::domain::Snippet;
use digicore_core::adapters::platform::clipboard_windows::WindowsRichClipboardAdapter;
use digicore_core::domain::ports::ClipboardPort;
use digicore_text_expander::adapters::storage::JsonFileStorageAdapter;
use digicore_text_expander::application::app_state::AppState;
use digicore_text_expander::application::clipboard_history::{self, ClipboardHistoryConfig};
use digicore_text_expander::application::expansion_engine::set_expansion_paused;
use digicore_text_expander::application::ghost_suggestor;
use digicore_text_expander::application::scripting::load_and_apply_script_libraries;
use digicore_text_expander::application::template_processor::{self, InteractiveVarType};
use digicore_text_expander::application::variable_input;
use digicore_text_expander::application::discovery;
use digicore_text_expander::drivers::hotstring::{
    start_listener, sync_discovery_config, sync_ghost_config, GhostConfig,
};
use digicore_text_expander::ports::{storage_keys, StoragePort};
use digicore_text_expander::services::sync_service::SyncResult;
use crate::utils::crypto_adapter::TauriCryptoAdapter;
pub mod utils;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::menu::{Menu, MenuItem};
use tauri::{Emitter, Listener, Manager};
use tauri_plugin_sql::{Migration, MigrationKind};

/// Serializable view of AppState for frontend. Excludes mpsc::Receiver and other non-serializable fields.
#[taurpc::ipc_type]
pub struct AppStateDto {
    pub library_path: String,
    pub expansion_log_path: String,
    pub kms_vault_path: String,
    pub library: HashMap<String, Vec<Snippet>>,
    pub categories: Vec<String>,
    pub selected_category: Option<u32>,
    pub status: String,
    pub sync_url: String,
    pub sync_status: String,
    pub expansion_paused: bool,
    pub dummy_field_for_regeneration: Option<String>, // force-regeneration-1774393600
    pub template_date_format: String,
    pub template_time_format: String,
    pub discovery_enabled: bool,
    pub discovery_threshold: u32,
    pub discovery_lookback: u32,
    pub discovery_min_len: u32,
    pub discovery_max_len: u32,
    pub discovery_excluded_apps: String,
    pub discovery_excluded_window_titles: String,
    pub ghost_suggestor_enabled: bool,
    pub ghost_suggestor_debounce_ms: u32,
    pub ghost_suggestor_display_secs: u32,
    pub ghost_suggestor_snooze_duration_mins: u32,
    pub ghost_suggestor_offset_x: i32,
    pub ghost_suggestor_offset_y: i32,
    pub ghost_follower_enabled: bool,
    pub ghost_follower_edge_right: bool,
    pub ghost_follower_monitor_anchor: u32,
    pub ghost_follower_hover_preview: bool,
    pub ghost_follower_collapse_delay_secs: u32,
    pub ghost_follower_search: String,
    pub ghost_follower_mode: String,
    pub ghost_follower_expand_trigger: String,
    pub ghost_follower_expand_delay_ms: u32,
    pub ghost_follower_clipboard_depth: u32,
    pub ghost_follower_opacity: u32,
    pub clip_history_max_depth: u32,
    pub script_library_run_disabled: bool,
    pub script_library_run_allowlist: String,
    pub snippet_editor_is_sensitive: bool,

    pub corpus_enabled: bool,
    pub corpus_output_dir: String,
    pub corpus_snapshot_dir: String,
    pub corpus_shortcut_modifiers: u32,
    pub corpus_shortcut_key: u32,

    pub extraction_row_overlap_tolerance: f32,
    pub extraction_cluster_threshold_factor: f32,
    pub extraction_zone_proximity: f32,
    pub extraction_cross_zone_gap_factor: f32,
    pub extraction_same_zone_gap_factor: f32,
    pub extraction_significant_gap_gate: f32,
    pub extraction_char_width_factor: f32,
    pub extraction_bridged_threshold: f32,
    pub extraction_word_spacing_factor: f32,

    pub extraction_footer_triggers: String,
    pub extraction_table_min_contiguous_rows: u32,
    pub extraction_table_min_avg_segments: f32,
    pub extraction_layout_row_lookback: u32,
    pub extraction_layout_table_break_threshold: f32,
    pub extraction_layout_paragraph_break_threshold: f32,
    pub extraction_layout_max_space_clamp: u32,
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

    pub extraction_columns_min_contiguous_rows: u32,
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

    pub kms_graph_k_means_max_k: u32,
    pub kms_graph_k_means_iterations: u32,
    pub kms_graph_ai_beam_max_nodes: u32,
    pub kms_graph_ai_beam_similarity_threshold: f32,
    pub kms_graph_ai_beam_max_edges: u32,
    pub kms_graph_enable_ai_beams: bool,
    pub kms_graph_enable_semantic_clustering: bool,
    pub kms_graph_enable_leiden_communities: bool,
    pub kms_graph_semantic_max_notes: u32,
    pub kms_graph_warn_note_threshold: u32,
    pub kms_graph_beam_max_pair_checks: u32,
    pub kms_graph_enable_semantic_knn_edges: bool,
    pub kms_graph_semantic_knn_per_note: u32,
    pub kms_graph_semantic_knn_min_similarity: f32,
    pub kms_graph_semantic_knn_max_edges: u32,
    pub kms_graph_semantic_knn_max_pair_checks: u32,
    pub kms_graph_auto_paging_enabled: bool,
    pub kms_graph_auto_paging_note_threshold: u32,
    pub kms_graph_vault_overrides_json: String,

    pub kms_graph_bloom_enabled: bool,
    pub kms_graph_bloom_strength: f32,
    pub kms_graph_bloom_radius: f32,
    pub kms_graph_bloom_threshold: f32,
    pub kms_graph_hex_cell_radius: f32,
    pub kms_graph_hex_layer_opacity: f32,
    pub kms_graph_hex_stroke_width: f32,
    pub kms_graph_hex_stroke_opacity: f32,

    pub kms_graph_pagerank_iterations: u32,
    pub kms_graph_pagerank_local_iterations: u32,
    pub kms_graph_pagerank_damping: f32,
    pub kms_graph_pagerank_scope: String,
    pub kms_graph_background_wiki_pagerank_enabled: bool,

    pub kms_graph_temporal_window_enabled: bool,
    pub kms_graph_temporal_default_days: u32,
    pub kms_graph_temporal_include_notes_without_mtime: bool,
    pub kms_graph_temporal_edge_recency_enabled: bool,
    pub kms_graph_temporal_edge_recency_strength: f32,
    pub kms_graph_temporal_edge_recency_half_life_days: f32,
    pub kms_search_min_similarity: f32,
    /// When true, semantic search rows include query-embedding timing and effective model id.
    pub kms_search_include_embedding_diagnostics: bool,
    pub kms_search_default_mode: String,
    pub kms_search_default_limit: u32,

    /// Stored KMS text embedding model id (empty = use default fastembed id).
    pub kms_embedding_model_id: String,
    /// Background note re-embed batch size (D6 migration).
    pub kms_embedding_batch_notes_per_tick: u32,
    pub kms_embedding_chunk_enabled: bool,
    pub kms_embedding_chunk_max_chars: u32,
    pub kms_embedding_chunk_overlap_chars: u32,

    pub kms_graph_sprite_label_max_dpr_scale: f32,
    pub kms_graph_sprite_label_min_res_scale: f32,
    pub kms_graph_webworker_layout_threshold: u32,
    pub kms_graph_webworker_layout_max_ticks: u32,
    pub kms_graph_webworker_layout_alpha_min: f32,

    pub snippet_editor_case_sensitive: bool,
}

fn parse_comma_list(s: &str) -> Vec<String> {
    s.split(',')
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
}

fn sync_result_to_string(r: &SyncResult) -> String {
    match r {
        SyncResult::Idle => "idle".to_string(),
        SyncResult::InProgress => "in_progress".to_string(),
        SyncResult::Success(msg) => format!("success:{}", msg),
        SyncResult::Error(msg) => format!("error:{}", msg),
    }
}

fn app_state_to_dto(state: &AppState) -> AppStateDto {
    AppStateDto {
        library_path: state.library_path.clone(),
        expansion_log_path: state.expansion_log_path.clone(),
        kms_vault_path: state.kms_vault_path.clone(),
        library: state.library.clone(),
        categories: state.categories.clone(),
        selected_category: state.selected_category.map(|v| v as u32),
        status: state.status.clone(),
        sync_url: state.sync_url.clone(),
        sync_status: sync_result_to_string(&state.sync_status),
        expansion_paused: state.expansion_paused,
        dummy_field_for_regeneration: None,
        template_date_format: state.template_date_format.clone(),
        template_time_format: state.template_time_format.clone(),
        discovery_enabled: state.discovery_enabled,
        discovery_threshold: state.discovery_threshold,
        discovery_lookback: state.discovery_lookback,
        discovery_min_len: state.discovery_min_len as u32,
        discovery_max_len: state.discovery_max_len as u32,
        discovery_excluded_apps: state.discovery_excluded_apps.clone(),
        discovery_excluded_window_titles: state.discovery_excluded_window_titles.clone(),
        ghost_suggestor_enabled: state.ghost_suggestor_enabled,
        ghost_suggestor_debounce_ms: state.ghost_suggestor_debounce_ms as u32,
        ghost_suggestor_display_secs: state.ghost_suggestor_display_secs as u32,
        ghost_suggestor_snooze_duration_mins: state.ghost_suggestor_snooze_duration_mins as u32,
        ghost_suggestor_offset_x: state.ghost_suggestor_offset_x,
        ghost_suggestor_offset_y: state.ghost_suggestor_offset_y,
        ghost_follower_enabled: state.ghost_follower.config.enabled,
        ghost_follower_edge_right: state.ghost_follower.config.edge == digicore_text_expander::application::ghost_follower::FollowerEdge::Right,
        ghost_follower_monitor_anchor: match state.ghost_follower.config.monitor_anchor {
            digicore_text_expander::application::ghost_follower::MonitorAnchor::Secondary => 1,
            digicore_text_expander::application::ghost_follower::MonitorAnchor::Current => 2,
            _ => 0,
        },
        ghost_follower_mode: format!("{:?}", state.ghost_follower.config.mode),
        ghost_follower_expand_trigger: format!("{:?}", state.ghost_follower.config.expand_trigger),
        ghost_follower_expand_delay_ms: state.ghost_follower.config.expand_delay_ms as u32,
        ghost_follower_clipboard_depth: state.ghost_follower.config.clipboard_depth as u32,
        ghost_follower_opacity: state.ghost_follower.config.opacity,
        ghost_follower_search: state.ghost_follower.search_filter.clone(),
        ghost_follower_hover_preview: state.ghost_follower.config.hover_preview,
        ghost_follower_collapse_delay_secs: state.ghost_follower.config.collapse_delay_secs as u32,
        clip_history_max_depth: state.clip_history_max_depth as u32,
        script_library_run_disabled: state.script_library_run_disabled,
        script_library_run_allowlist: state.script_library_run_allowlist.clone(),
        snippet_editor_is_sensitive: state.snippet_editor_is_sensitive,

        corpus_enabled: state.corpus_enabled,
        corpus_output_dir: state.corpus_output_dir.clone(),
        corpus_snapshot_dir: state.corpus_snapshot_dir.clone(),
        corpus_shortcut_modifiers: state.corpus_shortcut_modifiers as u32,
        corpus_shortcut_key: state.corpus_shortcut_key as u32,

        extraction_row_overlap_tolerance: state.extraction_row_overlap_tolerance,
        extraction_cluster_threshold_factor: state.extraction_cluster_threshold_factor,
        extraction_zone_proximity: state.extraction_zone_proximity,
        extraction_cross_zone_gap_factor: state.extraction_cross_zone_gap_factor,
        extraction_same_zone_gap_factor: state.extraction_same_zone_gap_factor,
        extraction_significant_gap_gate: state.extraction_significant_gap_gate,
        extraction_char_width_factor: state.extraction_char_width_factor,
        extraction_bridged_threshold: state.extraction_bridged_threshold,
        extraction_word_spacing_factor: state.extraction_word_spacing_factor,

        extraction_footer_triggers: state.extraction_footer_triggers.clone(),
        extraction_table_min_contiguous_rows: state.extraction_table_min_contiguous_rows as u32,
        extraction_table_min_avg_segments: state.extraction_table_min_avg_segments,
        extraction_layout_row_lookback: state.extraction_layout_row_lookback as u32,
        extraction_layout_table_break_threshold: state.extraction_layout_table_break_threshold,
        extraction_layout_paragraph_break_threshold: state.extraction_layout_paragraph_break_threshold,
        extraction_layout_max_space_clamp: state.extraction_layout_max_space_clamp as u32,
        extraction_tables_column_jitter_tolerance: state.extraction_tables_column_jitter_tolerance,
        extraction_tables_merge_y_gap_max: state.extraction_tables_merge_y_gap_max,
        extraction_tables_merge_y_gap_min: state.extraction_tables_merge_y_gap_min,

        extraction_adaptive_plaintext_cluster_factor: state.extraction_adaptive_plaintext_cluster_factor,
        extraction_adaptive_plaintext_gap_gate: state.extraction_adaptive_plaintext_gap_gate,
        extraction_adaptive_table_cluster_factor: state.extraction_adaptive_table_cluster_factor,
        extraction_adaptive_table_gap_gate: state.extraction_adaptive_table_gap_gate,
        extraction_adaptive_column_cluster_factor: state.extraction_adaptive_column_cluster_factor,
        extraction_adaptive_column_gap_gate: state.extraction_adaptive_column_gap_gate,
        extraction_adaptive_plaintext_cross_factor: state.extraction_adaptive_plaintext_cross_factor,
        extraction_adaptive_table_cross_factor: state.extraction_adaptive_table_cross_factor,
        extraction_adaptive_column_cross_factor: state.extraction_adaptive_column_cross_factor,

        extraction_refinement_entropy_threshold: state.extraction_refinement_entropy_threshold,
        extraction_refinement_cluster_threshold_modifier: state.extraction_refinement_cluster_threshold_modifier,
        extraction_refinement_cross_zone_gap_modifier: state.extraction_refinement_cross_zone_gap_modifier,

        extraction_classifier_gutter_weight: state.extraction_classifier_gutter_weight,
        extraction_classifier_density_weight: state.extraction_classifier_density_weight,
        extraction_classifier_multicolumn_density_max: state.extraction_classifier_multicolumn_density_max,
        extraction_classifier_table_density_min: state.extraction_classifier_table_density_min,
        extraction_classifier_table_entropy_min: state.extraction_classifier_table_entropy_min,

        extraction_columns_min_contiguous_rows: state.extraction_columns_min_contiguous_rows as u32,
        extraction_columns_gutter_gap_factor: state.extraction_columns_gutter_gap_factor,
        extraction_columns_gutter_void_tolerance: state.extraction_columns_gutter_void_tolerance,
        extraction_columns_edge_margin_tolerance: state.extraction_columns_edge_margin_tolerance,

        extraction_headers_max_width_ratio: state.extraction_headers_max_width_ratio,
        extraction_headers_centered_tolerance: state.extraction_headers_centered_tolerance,
        extraction_headers_h1_size_multiplier: state.extraction_headers_h1_size_multiplier,
        extraction_headers_h2_size_multiplier: state.extraction_headers_h2_size_multiplier,
        extraction_headers_h3_size_multiplier: state.extraction_headers_h3_size_multiplier,

        extraction_scoring_jitter_penalty_weight: state.extraction_scoring_jitter_penalty_weight,
        extraction_scoring_size_penalty_weight: state.extraction_scoring_size_penalty_weight,
        extraction_scoring_low_confidence_threshold: state.extraction_scoring_low_confidence_threshold,

        kms_graph_k_means_max_k: state.kms_graph_k_means_max_k,
        kms_graph_k_means_iterations: state.kms_graph_k_means_iterations,
        kms_graph_ai_beam_max_nodes: state.kms_graph_ai_beam_max_nodes,
        kms_graph_ai_beam_similarity_threshold: state.kms_graph_ai_beam_similarity_threshold,
        kms_graph_ai_beam_max_edges: state.kms_graph_ai_beam_max_edges,
        kms_graph_enable_ai_beams: state.kms_graph_enable_ai_beams,
        kms_graph_enable_semantic_clustering: state.kms_graph_enable_semantic_clustering,
        kms_graph_enable_leiden_communities: state.kms_graph_enable_leiden_communities,
        kms_graph_semantic_max_notes: state.kms_graph_semantic_max_notes,
        kms_graph_warn_note_threshold: state.kms_graph_warn_note_threshold,
        kms_graph_beam_max_pair_checks: state.kms_graph_beam_max_pair_checks,
        kms_graph_enable_semantic_knn_edges: state.kms_graph_enable_semantic_knn_edges,
        kms_graph_semantic_knn_per_note: state.kms_graph_semantic_knn_per_note,
        kms_graph_semantic_knn_min_similarity: state.kms_graph_semantic_knn_min_similarity,
        kms_graph_semantic_knn_max_edges: state.kms_graph_semantic_knn_max_edges,
        kms_graph_semantic_knn_max_pair_checks: state.kms_graph_semantic_knn_max_pair_checks,
        kms_graph_auto_paging_enabled: state.kms_graph_auto_paging_enabled,
        kms_graph_auto_paging_note_threshold: state.kms_graph_auto_paging_note_threshold,
        kms_graph_vault_overrides_json: state.kms_graph_vault_overrides_json.clone(),

        kms_graph_bloom_enabled: state.kms_graph_bloom_enabled,
        kms_graph_bloom_strength: state.kms_graph_bloom_strength,
        kms_graph_bloom_radius: state.kms_graph_bloom_radius,
        kms_graph_bloom_threshold: state.kms_graph_bloom_threshold,
        kms_graph_hex_cell_radius: state.kms_graph_hex_cell_radius,
        kms_graph_hex_layer_opacity: state.kms_graph_hex_layer_opacity,
        kms_graph_hex_stroke_width: state.kms_graph_hex_stroke_width,
        kms_graph_hex_stroke_opacity: state.kms_graph_hex_stroke_opacity,

        kms_graph_pagerank_iterations: state.kms_graph_pagerank_iterations,
        kms_graph_pagerank_local_iterations: state.kms_graph_pagerank_local_iterations,
        kms_graph_pagerank_damping: state.kms_graph_pagerank_damping,
        kms_graph_pagerank_scope: state.kms_graph_pagerank_scope.clone(),
        kms_graph_background_wiki_pagerank_enabled: state.kms_graph_background_wiki_pagerank_enabled,

        kms_graph_temporal_window_enabled: state.kms_graph_temporal_window_enabled,
        kms_graph_temporal_default_days: state.kms_graph_temporal_default_days,
        kms_graph_temporal_include_notes_without_mtime: state.kms_graph_temporal_include_notes_without_mtime,
        kms_graph_temporal_edge_recency_enabled: state.kms_graph_temporal_edge_recency_enabled,
        kms_graph_temporal_edge_recency_strength: state.kms_graph_temporal_edge_recency_strength,
        kms_graph_temporal_edge_recency_half_life_days: state.kms_graph_temporal_edge_recency_half_life_days,
        kms_search_min_similarity: state.kms_search_min_similarity,
        kms_search_include_embedding_diagnostics: state.kms_search_include_embedding_diagnostics,
        kms_search_default_mode: state.kms_search_default_mode.clone(),
        kms_search_default_limit: state.kms_search_default_limit,

        kms_embedding_model_id: state.kms_embedding_model_id.clone(),
        kms_embedding_batch_notes_per_tick: state.kms_embedding_batch_notes_per_tick,
        kms_embedding_chunk_enabled: state.kms_embedding_chunk_enabled,
        kms_embedding_chunk_max_chars: state.kms_embedding_chunk_max_chars,
        kms_embedding_chunk_overlap_chars: state.kms_embedding_chunk_overlap_chars,

        kms_graph_sprite_label_max_dpr_scale: state.kms_graph_sprite_label_max_dpr_scale,
        kms_graph_sprite_label_min_res_scale: state.kms_graph_sprite_label_min_res_scale,
        kms_graph_webworker_layout_threshold: state.kms_graph_webworker_layout_threshold,
        kms_graph_webworker_layout_max_ticks: state.kms_graph_webworker_layout_max_ticks,
        kms_graph_webworker_layout_alpha_min: state.kms_graph_webworker_layout_alpha_min,

        snippet_editor_case_sensitive: state.snippet_editor_case_sensitive,
    }
}

fn tray_pause_menu_label(paused: bool) -> &'static str {
    if paused {
        "Toggle Unpause - Paused"
    } else {
        "Toggle Pause - Running"
    }
}

fn install_tray_menu(handle: &tauri::AppHandle, paused: bool) {
    if let Some(tray) = handle.tray_by_id("default") {
        let console_i =
            MenuItem::with_id(handle, "view_console", "View Management Console", true, None::<&str>);
        let quick_search_i = MenuItem::with_id(
            handle,
            "quick_search",
            "Display Quick Search (Shift+Alt+Space)",
            true,
            None::<&str>,
        );
        let toggle_pause_i =
            MenuItem::with_id(handle, "toggle_pause", tray_pause_menu_label(paused), true, None::<&str>);
        let follower_i =
            MenuItem::with_id(handle, "view_follower", "Display Ghost Follower", true, None::<&str>);
        let quit_i = MenuItem::with_id(handle, "quit", "Exit application", true, None::<&str>);
        if let (Ok(console), Ok(quick_search), Ok(toggle_pause), Ok(follower), Ok(quit)) =
            (console_i, quick_search_i, toggle_pause_i, follower_i, quit_i)
        {
            let items: Vec<&dyn tauri::menu::IsMenuItem<_>> =
                vec![&console, &quick_search, &toggle_pause, &follower, &quit];
            if let Ok(m) = Menu::with_items(handle, &items) {
                let _ = tray.set_menu(Some(m));
                let _ = tray.set_show_menu_on_left_click(false);
            }
        }
    }
}

/// Initialize AppState from JsonFileStorageAdapter (same keys as egui).
fn init_app_state_from_storage() -> AppState {
    let storage = JsonFileStorageAdapter::load();
    let mut state = AppState::default();
    let sync_url = storage.get(storage_keys::SYNC_URL).unwrap_or_default();
    let template_date_format = storage
        .get(storage_keys::TEMPLATE_DATE_FORMAT)
        .unwrap_or_else(|| "%Y-%m-%d".to_string());
    let template_time_format = storage
        .get(storage_keys::TEMPLATE_TIME_FORMAT)
        .unwrap_or_else(|| "%H:%M".to_string());
    let (run_disabled, run_allowlist) = storage
        .get(storage_keys::SCRIPT_LIBRARY_RUN_DISABLED)
        .map(|v| {
            (
                v == "true",
                storage
                    .get(storage_keys::SCRIPT_LIBRARY_RUN_ALLOWLIST)
                    .unwrap_or_default(),
            )
        })
        .unwrap_or((false, String::new()));
    let ghost_suggestor_display_secs = storage
        .get(storage_keys::GHOST_SUGGESTOR_DISPLAY_SECS)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10u64);
    let ghost_suggestor_snooze_duration_mins = storage
        .get(storage_keys::GHOST_SUGGESTOR_SNOOZE_DURATION_MINS)
        .and_then(|s| s.parse().ok())
        .unwrap_or(5u64)
        .clamp(1, 120);
    let clip_history_max_depth = storage
        .get(storage_keys::CLIP_HISTORY_MAX_DEPTH)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20usize);
    let expansion_paused = storage
        .get(storage_keys::EXPANSION_PAUSED)
        .map(|v| v == "true")
        .unwrap_or(false);
    let ghost_suggestor_enabled = storage
        .get(storage_keys::GHOST_SUGGESTOR_ENABLED)
        .map(|v| v == "true")
        .unwrap_or(true);
    let ghost_suggestor_debounce_ms = storage
        .get(storage_keys::GHOST_SUGGESTOR_DEBOUNCE_MS)
        .and_then(|s| s.parse().ok())
        .unwrap_or(50u64);
    let ghost_suggestor_offset_x = storage
        .get(storage_keys::GHOST_SUGGESTOR_OFFSET_X)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let ghost_suggestor_offset_y = storage
        .get(storage_keys::GHOST_SUGGESTOR_OFFSET_Y)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);
    let discovery_enabled = storage
        .get(storage_keys::DISCOVERY_ENABLED)
        .map(|v| v == "true")
        .unwrap_or(false);
    let discovery_threshold = storage
        .get(storage_keys::DISCOVERY_THRESHOLD)
        .and_then(|s| s.parse().ok())
        .unwrap_or(2u32);
    let discovery_lookback = storage
        .get(storage_keys::DISCOVERY_LOOKBACK)
        .and_then(|s| s.parse().ok())
        .unwrap_or(60u32);
    let discovery_min_len = storage
        .get(storage_keys::DISCOVERY_MIN_LEN)
        .and_then(|s| s.parse().ok())
        .unwrap_or(3usize);
    let discovery_max_len = storage
        .get(storage_keys::DISCOVERY_MAX_LEN)
        .and_then(|s| s.parse().ok())
        .unwrap_or(50usize);
    let discovery_excluded_apps = storage
        .get(storage_keys::DISCOVERY_EXCLUDED_APPS)
        .unwrap_or_default();
    let discovery_excluded_window_titles = storage
        .get(storage_keys::DISCOVERY_EXCLUDED_WINDOW_TITLES)
        .unwrap_or_default();

    state.sync_url = sync_url;
    state.template_date_format = template_date_format;
    state.template_time_format = template_time_format;
    state.ghost_suggestor_display_secs = ghost_suggestor_display_secs;
    state.ghost_suggestor_snooze_duration_mins = ghost_suggestor_snooze_duration_mins;
    state.script_library_run_disabled = run_disabled;
    state.script_library_run_allowlist = run_allowlist;
    state.clip_history_max_depth = clip_history_max_depth;
    
    // Initialize Ghost Follower Config
    state.ghost_follower.config = digicore_text_expander::application::ghost_follower::GhostFollowerConfig {
        enabled: storage.get(storage_keys::GHOST_FOLLOWER_ENABLED).map(|v| v == "true").unwrap_or(true),
        mode: match storage.get(storage_keys::GHOST_FOLLOWER_MODE).as_deref() {
            Some("Bubble") => digicore_text_expander::application::ghost_follower::FollowerMode::FloatingBubble,
            _ => digicore_text_expander::application::ghost_follower::FollowerMode::EdgeAnchored,
        },
        edge: if storage.get(storage_keys::GHOST_FOLLOWER_EDGE_RIGHT).map(|v| v == "true").unwrap_or(true) {
            digicore_text_expander::application::ghost_follower::FollowerEdge::Right
        } else {
            digicore_text_expander::application::ghost_follower::FollowerEdge::Left
        },
        monitor_anchor: match storage.get(storage_keys::GHOST_FOLLOWER_MONITOR_ANCHOR).and_then(|s| s.parse().ok()).unwrap_or(0u32) {
            1 => digicore_text_expander::application::ghost_follower::MonitorAnchor::Secondary,
            2 => digicore_text_expander::application::ghost_follower::MonitorAnchor::Current,
            _ => digicore_text_expander::application::ghost_follower::MonitorAnchor::Primary,
        },
        expand_trigger: match storage.get(storage_keys::GHOST_FOLLOWER_EXPAND_TRIGGER).as_deref() {
            Some("Hover") => digicore_text_expander::application::ghost_follower::ExpandTrigger::Hover,
            _ => digicore_text_expander::application::ghost_follower::ExpandTrigger::Click,
        },
        expand_delay_ms: storage.get(storage_keys::GHOST_FOLLOWER_EXPAND_DELAY_MS).and_then(|s| s.parse().ok()).unwrap_or(500u64),
        collapse_delay_secs: storage.get(storage_keys::GHOST_FOLLOWER_COLLAPSE_DELAY_SECS).and_then(|s| s.parse().ok()).unwrap_or(5u64),
        hover_preview: storage.get(storage_keys::GHOST_FOLLOWER_HOVER_PREVIEW).map(|v| v == "true").unwrap_or(true),
        clipboard_depth: storage.get(storage_keys::GHOST_FOLLOWER_CLIPBOARD_DEPTH).and_then(|s| s.parse().ok()).unwrap_or(20usize),
        opacity: storage.get(storage_keys::GHOST_FOLLOWER_OPACITY).and_then(|s| s.parse().ok()).unwrap_or(100u32),
        position: storage.get(storage_keys::GHOST_FOLLOWER_POSITION_X).and_then(|sx| sx.parse().ok()).and_then(|x: i32| {
            storage.get(storage_keys::GHOST_FOLLOWER_POSITION_Y).and_then(|sy| sy.parse().ok()).map(|y: i32| (x, y))
        }),
    };

    state.snippet_editor_is_sensitive = false;
    state.expansion_paused = expansion_paused;
    state.ghost_suggestor_enabled = ghost_suggestor_enabled;
    state.crypto = Some(Box::new(TauriCryptoAdapter));
    state.ghost_suggestor_debounce_ms = ghost_suggestor_debounce_ms;
    state.ghost_suggestor_offset_x = ghost_suggestor_offset_x;
    state.ghost_suggestor_offset_y = ghost_suggestor_offset_y;
    state.discovery_enabled = discovery_enabled;
    state.discovery_threshold = discovery_threshold;
    state.discovery_lookback = discovery_lookback;
    state.discovery_min_len = discovery_min_len;
    state.discovery_max_len = discovery_max_len;
    state.discovery_excluded_apps = discovery_excluded_apps;
    state.discovery_excluded_window_titles = discovery_excluded_window_titles;

    if let Some(v) = storage.get(storage_keys::LIBRARY_PATH) { state.library_path = v.to_string(); }
    if let Some(v) = storage.get(storage_keys::EXPANSION_LOG_PATH) { 
        state.expansion_log_path = v.to_string(); 
        digicore_text_expander::application::expansion_logger::set_log_path(v.to_string());
    }
    if let Some(v) = storage.get(storage_keys::KMS_VAULT_PATH) { state.kms_vault_path = v.to_string(); }

    state.corpus_enabled = storage.get(storage_keys::CORPUS_ENABLED).map(|v| v == "true").unwrap_or(state.corpus_enabled);
    if let Some(v) = storage.get(storage_keys::CORPUS_OUTPUT_DIR) { state.corpus_output_dir = v.to_string(); }
    if let Some(v) = storage.get(storage_keys::CORPUS_SNAPSHOT_DIR) { state.corpus_snapshot_dir = v.to_string(); }
    if let Some(v) = storage.get(storage_keys::CORPUS_SHORTCUT_MODIFIERS).and_then(|s| s.parse().ok()) { state.corpus_shortcut_modifiers = v; }
    if let Some(v) = storage.get(storage_keys::CORPUS_SHORTCUT_KEY).and_then(|s| s.parse().ok()) { state.corpus_shortcut_key = v; }

    // Legacy hotkey upgrade path (from Phase 57 bugfix)
    // Old default was modifiers=0x13 (19) and key=0x53 ('S'). Auto-upgrade to 7 and 'C' (0x43)
    if state.corpus_shortcut_modifiers == 19 {
        state.corpus_shortcut_modifiers = 7; // 1=Ctrl | 2=Alt | 4=Shift
        if state.corpus_shortcut_key == 0x53 {
            state.corpus_shortcut_key = 0x43; // 'C'
        }
    }

    if let Some(v) = storage.get(storage_keys::EXTRACTION_ROW_OVERLAP_TOLERANCE).and_then(|s| s.parse().ok()) { state.extraction_row_overlap_tolerance = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_CLUSTER_THRESHOLD_FACTOR).and_then(|s| s.parse().ok()) { state.extraction_cluster_threshold_factor = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_ZONE_PROXIMITY).and_then(|s| s.parse().ok()) { state.extraction_zone_proximity = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_CROSS_ZONE_GAP_FACTOR).and_then(|s| s.parse().ok()) { state.extraction_cross_zone_gap_factor = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_SAME_ZONE_GAP_FACTOR).and_then(|s| s.parse().ok()) { state.extraction_same_zone_gap_factor = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_SIGNIFICANT_GAP_GATE).and_then(|s| s.parse().ok()) { state.extraction_significant_gap_gate = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_CHAR_WIDTH_FACTOR).and_then(|s| s.parse().ok()) { state.extraction_char_width_factor = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_BRIDGED_THRESHOLD).and_then(|s| s.parse().ok()) { state.extraction_bridged_threshold = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_WORD_SPACING_FACTOR).and_then(|s| s.parse().ok()) { state.extraction_word_spacing_factor = v; }

    if let Some(v) = storage.get(storage_keys::EXTRACTION_FOOTER_TRIGGERS) { state.extraction_footer_triggers = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_TABLE_MIN_CONTIGUOUS_ROWS).and_then(|s| s.parse().ok()) { state.extraction_table_min_contiguous_rows = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_TABLE_MIN_AVG_SEGMENTS).and_then(|s| s.parse().ok()) { state.extraction_table_min_avg_segments = v; }

    if let Some(v) = storage.get(storage_keys::EXTRACTION_ADAPTIVE_PLAINTEXT_CLUSTER_FACTOR).and_then(|s| s.parse().ok()) { state.extraction_adaptive_plaintext_cluster_factor = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_ADAPTIVE_PLAINTEXT_GAP_GATE).and_then(|s| s.parse().ok()) { state.extraction_adaptive_plaintext_gap_gate = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_ADAPTIVE_TABLE_CLUSTER_FACTOR).and_then(|s| s.parse().ok()) { state.extraction_adaptive_table_cluster_factor = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_ADAPTIVE_TABLE_GAP_GATE).and_then(|s| s.parse().ok()) { state.extraction_adaptive_table_gap_gate = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_ADAPTIVE_COLUMN_CLUSTER_FACTOR).and_then(|s| s.parse().ok()) { state.extraction_adaptive_column_cluster_factor = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_ADAPTIVE_COLUMN_GAP_GATE).and_then(|s| s.parse().ok()) { state.extraction_adaptive_column_gap_gate = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_ADAPTIVE_PLAINTEXT_CROSS_FACTOR).and_then(|s| s.parse().ok()) { state.extraction_adaptive_plaintext_cross_factor = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_ADAPTIVE_TABLE_CROSS_FACTOR).and_then(|s| s.parse().ok()) { state.extraction_adaptive_table_cross_factor = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_ADAPTIVE_COLUMN_CROSS_FACTOR).and_then(|s| s.parse().ok()) { state.extraction_adaptive_column_cross_factor = v; }

    if let Some(v) = storage.get(storage_keys::EXTRACTION_REFINEMENT_ENTROPY_THRESHOLD).and_then(|s| s.parse().ok()) { state.extraction_refinement_entropy_threshold = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_REFINEMENT_CLUSTER_THRESHOLD_MODIFIER).and_then(|s| s.parse().ok()) { state.extraction_refinement_cluster_threshold_modifier = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_REFINEMENT_CROSS_ZONE_GAP_MODIFIER).and_then(|s| s.parse().ok()) { state.extraction_refinement_cross_zone_gap_modifier = v; }

    if let Some(v) = storage.get(storage_keys::EXTRACTION_CLASSIFIER_GUTTER_WEIGHT).and_then(|s| s.parse().ok()) { state.extraction_classifier_gutter_weight = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_CLASSIFIER_DENSITY_WEIGHT).and_then(|s| s.parse().ok()) { state.extraction_classifier_density_weight = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_CLASSIFIER_MULTICOLUMN_DENSITY_MAX).and_then(|s| s.parse().ok()) { state.extraction_classifier_multicolumn_density_max = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_CLASSIFIER_TABLE_DENSITY_MIN).and_then(|s| s.parse().ok()) { state.extraction_classifier_table_density_min = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_CLASSIFIER_TABLE_ENTROPY_MIN).and_then(|s| s.parse().ok()) { state.extraction_classifier_table_entropy_min = v; }

    if let Some(v) = storage.get(storage_keys::EXTRACTION_COLUMNS_MIN_CONTIGUOUS_ROWS).and_then(|s| s.parse().ok()) { state.extraction_columns_min_contiguous_rows = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_COLUMNS_GUTTER_GAP_FACTOR).and_then(|s| s.parse().ok()) { state.extraction_columns_gutter_gap_factor = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_COLUMNS_GUTTER_VOID_TOLERANCE).and_then(|s| s.parse().ok()) { state.extraction_columns_gutter_void_tolerance = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_COLUMNS_EDGE_MARGIN_TOLERANCE).and_then(|s| s.parse().ok()) { state.extraction_columns_edge_margin_tolerance = v; }

    if let Some(v) = storage.get(storage_keys::EXTRACTION_HEADERS_MAX_WIDTH_RATIO).and_then(|s| s.parse().ok()) { state.extraction_headers_max_width_ratio = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_HEADERS_CENTERED_TOLERANCE).and_then(|s| s.parse().ok()) { state.extraction_headers_centered_tolerance = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_HEADERS_H1_SIZE_MULTIPLIER).and_then(|s| s.parse().ok()) { state.extraction_headers_h1_size_multiplier = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_HEADERS_H2_SIZE_MULTIPLIER).and_then(|s| s.parse().ok()) { state.extraction_headers_h2_size_multiplier = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_HEADERS_H3_SIZE_MULTIPLIER).and_then(|s| s.parse().ok()) { state.extraction_headers_h3_size_multiplier = v; }

    if let Some(v) = storage.get(storage_keys::EXTRACTION_SCORING_JITTER_PENALTY_WEIGHT).and_then(|s| s.parse().ok()) { state.extraction_scoring_jitter_penalty_weight = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_SCORING_SIZE_PENALTY_WEIGHT).and_then(|s| s.parse().ok()) { state.extraction_scoring_size_penalty_weight = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_SCORING_LOW_CONFIDENCE_THRESHOLD).and_then(|s| s.parse().ok()) { state.extraction_scoring_low_confidence_threshold = v; }
    
    if let Some(v) = storage.get(storage_keys::EXTRACTION_LAYOUT_ROW_LOOKBACK).and_then(|s| s.parse().ok()) { state.extraction_layout_row_lookback = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_LAYOUT_TABLE_BREAK_THRESHOLD).and_then(|s| s.parse().ok()) { state.extraction_layout_table_break_threshold = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_LAYOUT_PARAGRAPH_BREAK_THRESHOLD).and_then(|s| s.parse().ok()) { state.extraction_layout_paragraph_break_threshold = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_LAYOUT_MAX_SPACE_CLAMP).and_then(|s| s.parse().ok()) { state.extraction_layout_max_space_clamp = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_TABLES_COLUMN_JITTER_TOLERANCE).and_then(|s| s.parse().ok()) { state.extraction_tables_column_jitter_tolerance = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_TABLES_MERGE_Y_GAP_MAX).and_then(|s| s.parse().ok()) { state.extraction_tables_merge_y_gap_max = v; }
    if let Some(v) = storage.get(storage_keys::EXTRACTION_TABLES_MERGE_Y_GAP_MIN).and_then(|s| s.parse().ok()) { state.extraction_tables_merge_y_gap_min = v; }

    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_K_MEANS_MAX_K)
        .and_then(|s| s.parse::<u32>().ok())
    {
        state.kms_graph_k_means_max_k = v.max(2);
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_K_MEANS_ITERATIONS)
        .and_then(|s| s.parse::<u32>().ok())
    {
        state.kms_graph_k_means_iterations = v.max(1);
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_AI_BEAM_MAX_NODES)
        .and_then(|s| s.parse::<u32>().ok())
    {
        state.kms_graph_ai_beam_max_nodes = v.max(2);
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_AI_BEAM_SIMILARITY_THRESHOLD)
        .and_then(|s| s.parse::<f32>().ok())
    {
        state.kms_graph_ai_beam_similarity_threshold = v.clamp(0.0, 1.0);
    }
    if let Some(v) = storage.get(storage_keys::KMS_GRAPH_AI_BEAM_MAX_EDGES).and_then(|s| s.parse().ok()) { state.kms_graph_ai_beam_max_edges = v; }
    if let Some(v) = storage.get(storage_keys::KMS_GRAPH_ENABLE_AI_BEAMS) { state.kms_graph_enable_ai_beams = v == "true"; }
    if let Some(v) = storage.get(storage_keys::KMS_GRAPH_ENABLE_SEMANTIC_CLUSTERING) { state.kms_graph_enable_semantic_clustering = v == "true"; }
    if let Some(v) = storage.get(storage_keys::KMS_GRAPH_ENABLE_LEIDEN_COMMUNITIES) { state.kms_graph_enable_leiden_communities = v == "true"; }
    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_SEMANTIC_MAX_NOTES)
        .and_then(|s| s.parse::<u32>().ok())
    {
        state.kms_graph_semantic_max_notes = v;
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_WARN_NOTE_THRESHOLD)
        .and_then(|s| s.parse::<u32>().ok())
    {
        state.kms_graph_warn_note_threshold = v;
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_BEAM_MAX_PAIR_CHECKS)
        .and_then(|s| s.parse::<u32>().ok())
    {
        state.kms_graph_beam_max_pair_checks = v;
    }
    if let Some(v) = storage.get(storage_keys::KMS_GRAPH_ENABLE_SEMANTIC_KNN_EDGES) {
        state.kms_graph_enable_semantic_knn_edges = v == "true";
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_SEMANTIC_KNN_PER_NOTE)
        .and_then(|s| s.parse::<u32>().ok())
    {
        state.kms_graph_semantic_knn_per_note = v.clamp(1, 30);
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_SEMANTIC_KNN_MIN_SIMILARITY)
        .and_then(|s| s.parse::<f32>().ok())
    {
        state.kms_graph_semantic_knn_min_similarity = v.clamp(0.5, 0.999);
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_SEMANTIC_KNN_MAX_EDGES)
        .and_then(|s| s.parse::<u32>().ok())
    {
        state.kms_graph_semantic_knn_max_edges = v.min(500_000);
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_SEMANTIC_KNN_MAX_PAIR_CHECKS)
        .and_then(|s| s.parse::<u32>().ok())
    {
        state.kms_graph_semantic_knn_max_pair_checks = v;
    }
    if let Some(v) = storage.get(storage_keys::KMS_GRAPH_AUTO_PAGING_ENABLED) {
        state.kms_graph_auto_paging_enabled = v == "true";
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_AUTO_PAGING_NOTE_THRESHOLD)
        .and_then(|s| s.parse::<u32>().ok())
    {
        state.kms_graph_auto_paging_note_threshold = v;
    }
    if let Some(s) = storage.get(storage_keys::KMS_GRAPH_VAULT_OVERRIDES_JSON) {
        if !s.trim().is_empty() {
            state.kms_graph_vault_overrides_json = s;
        }
    }
    if let Some(v) = storage.get(storage_keys::KMS_GRAPH_BLOOM_ENABLED) {
        state.kms_graph_bloom_enabled = v == "true";
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_BLOOM_STRENGTH)
        .and_then(|s| s.parse::<f32>().ok())
    {
        state.kms_graph_bloom_strength = v.clamp(0.0, 2.5);
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_BLOOM_RADIUS)
        .and_then(|s| s.parse::<f32>().ok())
    {
        state.kms_graph_bloom_radius = v.clamp(0.0, 1.5);
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_BLOOM_THRESHOLD)
        .and_then(|s| s.parse::<f32>().ok())
    {
        state.kms_graph_bloom_threshold = v.clamp(0.0, 1.0);
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_HEX_CELL_RADIUS)
        .and_then(|s| s.parse::<f32>().ok())
    {
        state.kms_graph_hex_cell_radius = v.clamp(0.5, 8.0);
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_HEX_LAYER_OPACITY)
        .and_then(|s| s.parse::<f32>().ok())
    {
        state.kms_graph_hex_layer_opacity = v.clamp(0.0, 1.0);
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_HEX_STROKE_WIDTH)
        .and_then(|s| s.parse::<f32>().ok())
    {
        state.kms_graph_hex_stroke_width = v.clamp(0.02, 0.5);
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_HEX_STROKE_OPACITY)
        .and_then(|s| s.parse::<f32>().ok())
    {
        state.kms_graph_hex_stroke_opacity = v.clamp(0.0, 1.0);
    }

    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_PAGERANK_ITERATIONS)
        .and_then(|s| s.parse::<u32>().ok())
    {
        state.kms_graph_pagerank_iterations = v.max(4);
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_PAGERANK_LOCAL_ITERATIONS)
        .and_then(|s| s.parse::<u32>().ok())
    {
        state.kms_graph_pagerank_local_iterations = v.max(4);
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_PAGERANK_DAMPING)
        .and_then(|s| s.parse::<f32>().ok())
    {
        state.kms_graph_pagerank_damping = v.clamp(0.5, 0.99);
    }
    if let Some(s) = storage.get(storage_keys::KMS_GRAPH_PAGERANK_SCOPE) {
        let t = s.trim();
        if !t.is_empty() {
            state.kms_graph_pagerank_scope = t.to_string();
        }
    }
    if let Some(v) = storage.get(storage_keys::KMS_GRAPH_BACKGROUND_WIKI_PAGERANK_ENABLED) {
        state.kms_graph_background_wiki_pagerank_enabled = v == "true";
    }
    if let Some(v) = storage.get(storage_keys::KMS_GRAPH_TEMPORAL_WINDOW_ENABLED) {
        state.kms_graph_temporal_window_enabled = v == "true";
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_TEMPORAL_DEFAULT_DAYS)
        .and_then(|s| s.parse::<u32>().ok())
    {
        state.kms_graph_temporal_default_days = v;
    }
    if let Some(v) = storage.get(storage_keys::KMS_GRAPH_TEMPORAL_INCLUDE_NOTES_WITHOUT_MTIME) {
        state.kms_graph_temporal_include_notes_without_mtime = v == "true";
    }
    if let Some(v) = storage.get(storage_keys::KMS_GRAPH_TEMPORAL_EDGE_RECENCY_ENABLED) {
        state.kms_graph_temporal_edge_recency_enabled = v == "true";
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_TEMPORAL_EDGE_RECENCY_STRENGTH)
        .and_then(|s| s.parse::<f32>().ok())
    {
        state.kms_graph_temporal_edge_recency_strength = v.clamp(0.0, 1.0);
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_TEMPORAL_EDGE_RECENCY_HALF_LIFE_DAYS)
        .and_then(|s| s.parse::<f32>().ok())
    {
        state.kms_graph_temporal_edge_recency_half_life_days = v.max(0.1);
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_SEARCH_MIN_SIMILARITY)
        .and_then(|s| s.parse::<f32>().ok())
    {
        state.kms_search_min_similarity = v.clamp(0.0, 1.0);
    }
    if let Some(v) = storage.get(storage_keys::KMS_SEARCH_INCLUDE_EMBEDDING_DIAGNOSTICS) {
        state.kms_search_include_embedding_diagnostics = v == "true";
    }
    if let Some(s) = storage.get(storage_keys::KMS_SEARCH_DEFAULT_MODE) {
        let t = s.trim();
        if t == "Hybrid" || t == "Semantic" || t == "Keyword" {
            state.kms_search_default_mode = t.to_string();
        }
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_SEARCH_DEFAULT_LIMIT)
        .and_then(|s| s.parse::<u32>().ok())
    {
        state.kms_search_default_limit = v.clamp(1, 200);
    }
    if let Some(s) = storage.get(storage_keys::KMS_EMBEDDING_MODEL_ID) {
        state.kms_embedding_model_id = s;
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_EMBEDDING_BATCH_NOTES_PER_TICK)
        .and_then(|s| s.parse::<u32>().ok())
    {
        state.kms_embedding_batch_notes_per_tick = v.clamp(1, 500);
    }
    if let Some(v) = storage.get(storage_keys::KMS_EMBEDDING_CHUNK_ENABLED) {
        state.kms_embedding_chunk_enabled = v == "true";
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_EMBEDDING_CHUNK_MAX_CHARS)
        .and_then(|s| s.parse::<u32>().ok())
    {
        state.kms_embedding_chunk_max_chars = v.clamp(256, 8192);
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_EMBEDDING_CHUNK_OVERLAP_CHARS)
        .and_then(|s| s.parse::<u32>().ok())
    {
        state.kms_embedding_chunk_overlap_chars = v.min(4096);
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_SPRITE_LABEL_MAX_DPR_SCALE)
        .and_then(|s| s.parse::<f32>().ok())
    {
        state.kms_graph_sprite_label_max_dpr_scale = v.clamp(1.0, 8.0);
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_SPRITE_LABEL_MIN_RES_SCALE)
        .and_then(|s| s.parse::<f32>().ok())
    {
        state.kms_graph_sprite_label_min_res_scale = v.clamp(1.0, 4.0);
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_WEBWORKER_LAYOUT_THRESHOLD)
        .and_then(|s| s.parse::<u32>().ok())
    {
        state.kms_graph_webworker_layout_threshold = v.min(500_000);
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_WEBWORKER_LAYOUT_MAX_TICKS)
        .and_then(|s| s.parse::<u32>().ok())
    {
        state.kms_graph_webworker_layout_max_ticks = v.clamp(20, 10_000);
    }
    if let Some(v) = storage
        .get(storage_keys::KMS_GRAPH_WEBWORKER_LAYOUT_ALPHA_MIN)
        .and_then(|s| s.parse::<f32>().ok())
    {
        state.kms_graph_webworker_layout_alpha_min = v.clamp(0.0005, 0.5);
    }

    state
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    std::panic::set_hook(Box::new(|info| {
        let payload = info.payload();
        let msg = if let Some(s) = payload.downcast_ref::<&str>() {
            *s
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.as_str()
        } else {
            "Unknown panic"
        };
        println!("[CRITICAL PANIC] {}: {:?}", msg, info.location());
        log::error!("[PANIC] {}: {:?}", msg, info);
    }));
    let mut app_state = init_app_state_from_storage();
    if !app_state.library_path.is_empty() {
        let _ = app_state.try_load_library();
    }
    use digicore_text_expander::adapters::corpus::{FileSystemCorpusStorageAdapter, OcrBaselineAdapter};
    use digicore_text_expander::application::corpus_generator::CorpusService;
    use digicore_core::domain::value_objects::CorpusConfig;
    let corpus_config = CorpusConfig {
        enabled: app_state.corpus_enabled,
        output_dir: app_state.corpus_output_dir.clone(),
        snapshot_dir: app_state.corpus_snapshot_dir.clone(),
        shortcut_modifiers: app_state.corpus_shortcut_modifiers,
        shortcut_key: app_state.corpus_shortcut_key,
    };
    let corpus_storage = std::sync::Arc::new(FileSystemCorpusStorageAdapter::new(corpus_config.output_dir.clone()));
    let ocr_config = digicore_text_expander::adapters::extraction::RuntimeConfig::load_from_json_adapter(&JsonFileStorageAdapter::load());
    let corpus_baseline = std::sync::Arc::new(OcrBaselineAdapter::new(corpus_config.snapshot_dir.clone(), ocr_config));
    let corpus_service = std::sync::Arc::new(CorpusService::new(corpus_config, corpus_storage, corpus_baseline));

    // Moving heavy initializations out of run() to background spawn in setup()
    // clipboard_repository::init and script loading are deferred.
    let storage_for_clip = JsonFileStorageAdapter::load();
    let copy_text_enabled = storage_for_clip
        .get(storage_keys::COPY_TO_CLIPBOARD_ENABLED)
        .map(|v| v == "true")
        .unwrap_or(true);
    let copy_image_enabled = storage_for_clip
        .get(storage_keys::COPY_TO_CLIPBOARD_IMAGE_ENABLED)
        .map(|v| v == "true")
        .unwrap_or(true);

    clipboard_history::update_config(ClipboardHistoryConfig {
        enabled: copy_text_enabled || copy_image_enabled,
        max_depth: if app_state.clip_history_max_depth == 0 {
            usize::MAX
        } else {
            app_state.clip_history_max_depth
        },
    });
    log::info!(
        "[Startup] sync_discovery_config: enabled={} threshold={}",
        app_state.discovery_enabled,
        app_state.discovery_threshold
    );
    sync_discovery_config(
        app_state.discovery_enabled,
        discovery::DiscoveryConfig {
            threshold: app_state.discovery_threshold,
            lookback_minutes: app_state.discovery_lookback,
            min_phrase_len: app_state.discovery_min_len,
            max_phrase_len: app_state.discovery_max_len,
            excluded_apps: parse_comma_list(&app_state.discovery_excluded_apps),
            excluded_window_titles: parse_comma_list(&app_state.discovery_excluded_window_titles),
        },
    );
    let clipboard: Arc<dyn ClipboardPort> = Arc::new(WindowsRichClipboardAdapter::new());
    let app_handle: Arc<Mutex<Option<tauri::AppHandle>>> = Arc::new(Mutex::new(None));
    let app_handle_for_setup = app_handle.clone();

    let app_state = Arc::new(Mutex::new(app_state));
    let state_for_exit = app_state.clone();
    let state_for_tray = app_state.clone();
    let prevent_default = if cfg!(debug_assertions) {
        tauri_plugin_prevent_default::debug()
    } else {
        tauri_plugin_prevent_default::init()
    };
    // Register sqlite-vec extension globally before tauri-plugin-sql initializes its database pools.
    unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    }

    let indexing_service = Arc::new(indexing_service::KmsIndexingService::new());
    indexing_service.register_provider(Arc::new(indexing_service::NoteIndexProvider));
    indexing_service.register_provider(Arc::new(indexing_service::SnippetIndexProvider));
    indexing_service.register_provider(Arc::new(indexing_service::ClipboardIndexProvider));
    indexing_service.register_provider(Arc::new(indexing_service::SkillIndexProvider));

    // Ensure KMS Git repository is initialized
    if let Err(e) = kms_git_service::KmsGitService::ensure_repo() {
        log::error!("[KMS] Failed to initialize Git repo: {:?}", e);
    }

    tauri::Builder::default()
        .manage(app_state.clone())
        .manage(indexing_service)
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_persisted_scope::init())
        .plugin(tauri_plugin_positioner::init())
        .plugin(prevent_default)
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_sql::Builder::default()
                .add_migrations(
                    "sqlite:digicore.db",
                    vec![
                        Migration {
                            version: 1,
                            description: "create_snippets_schema",
                            sql: r#"
                                CREATE TABLE IF NOT EXISTS categories (
                                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                                    name TEXT NOT NULL UNIQUE
                                );
                                CREATE TABLE IF NOT EXISTS snippets (
                                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                                    category_id INTEGER NOT NULL,
                                    trigger TEXT NOT NULL,
                                    content TEXT NOT NULL,
                                    options TEXT DEFAULT '',
                                    profile TEXT DEFAULT 'Default',
                                    app_lock TEXT DEFAULT '',
                                    pinned TEXT DEFAULT 'false',
                                    last_modified TEXT DEFAULT '',
                                    FOREIGN KEY (category_id) REFERENCES categories(id)
                                );
                                CREATE INDEX IF NOT EXISTS idx_snippets_category ON snippets(category_id);
                                CREATE INDEX IF NOT EXISTS idx_snippets_trigger ON snippets(trigger);
                            "#,
                            kind: MigrationKind::Up,
                        },
                        Migration {
                            version: 2,
                            description: "create_clipboard_history_schema",
                            sql: r#"
                                CREATE TABLE IF NOT EXISTS clipboard_history (
                                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                                    content TEXT NOT NULL,
                                    process_name TEXT NOT NULL DEFAULT '',
                                    window_title TEXT NOT NULL DEFAULT '',
                                    char_count INTEGER NOT NULL DEFAULT 0,
                                    word_count INTEGER NOT NULL DEFAULT 0,
                                    content_hash TEXT NOT NULL DEFAULT '',
                                    created_at_unix_ms INTEGER NOT NULL
                                );
                                CREATE INDEX IF NOT EXISTS idx_clipboard_history_created_at
                                    ON clipboard_history(created_at_unix_ms DESC);
                                CREATE INDEX IF NOT EXISTS idx_clipboard_history_content_hash
                                    ON clipboard_history(content_hash);
                            "#,
                            kind: MigrationKind::Up,
                        },
                        Migration {
                            version: 3,
                            description: "clipboard_history_add_image_support",
                            sql: r#"
                                ALTER TABLE clipboard_history ADD COLUMN entry_type TEXT NOT NULL DEFAULT 'text';
                                ALTER TABLE clipboard_history ADD COLUMN mime_type TEXT;
                                ALTER TABLE clipboard_history ADD COLUMN image_path TEXT;
                                ALTER TABLE clipboard_history ADD COLUMN thumb_path TEXT;
                                ALTER TABLE clipboard_history ADD COLUMN image_width INTEGER;
                                ALTER TABLE clipboard_history ADD COLUMN image_height INTEGER;
                                ALTER TABLE clipboard_history ADD COLUMN image_bytes INTEGER;
                            "#,
                            kind: MigrationKind::Up,
                        },
                        Migration {
                            version: 4,
                            description: "kms_foundation",
                            sql: r#"
                                CREATE TABLE IF NOT EXISTS kms_notes (
                                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                                    path TEXT NOT NULL UNIQUE,
                                    title TEXT NOT NULL,
                                    content_preview TEXT,
                                    last_modified TEXT,
                                    is_favorite INTEGER DEFAULT 0
                                );
                                CREATE TABLE IF NOT EXISTS kms_links (
                                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                                    source_path TEXT NOT NULL,
                                    target_path TEXT NOT NULL,
                                    link_type TEXT DEFAULT 'internal',
                                    context TEXT,
                                    UNIQUE(source_path, target_path)
                                );
                                CREATE TABLE IF NOT EXISTS kms_bookmarks (
                                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                                    url TEXT NOT NULL UNIQUE,
                                    title TEXT,
                                    snapshot_path TEXT,
                                    created_at TEXT
                                );
                                CREATE TABLE IF NOT EXISTS kms_tags (
                                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                                    name TEXT NOT NULL UNIQUE
                                );
                                CREATE TABLE IF NOT EXISTS kms_note_tags (
                                    note_id INTEGER NOT NULL,
                                    tag_id INTEGER NOT NULL,
                                    PRIMARY KEY (note_id, tag_id),
                                    FOREIGN KEY (note_id) REFERENCES kms_notes(id) ON DELETE CASCADE,
                                    FOREIGN KEY (tag_id) REFERENCES kms_tags(id) ON DELETE CASCADE
                                );
                            "#,
                            kind: MigrationKind::Up,
                        },
                        Migration {
                            version: 5,
                            description: "kms_multi_modal_vector_search",
                            sql: r#"
                                -- Virtual tables for vector embeddings (sqlite-vec)
                                -- float32 dimensions for BGE-small (384)
                                CREATE VIRTUAL TABLE IF NOT EXISTS kms_embeddings_text USING vec0(
                                    embedding float[384]
                                );
                                
                                -- float32 dimensions for CLIP-ViT-B-32 (512)
                                CREATE VIRTUAL TABLE IF NOT EXISTS kms_embeddings_image USING vec0(
                                    embedding float[512]
                                );

                                -- Unified mapping table to link vectors to source entities
                                -- Links vec0 rowid to app components
                                CREATE TABLE IF NOT EXISTS kms_vector_map (
                                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                                    vec_id INTEGER NOT NULL,
                                    modality TEXT NOT NULL, -- 'text', 'image'
                                    entity_type TEXT NOT NULL, -- 'note', 'snippet', 'clipboard', 'image_library'
                                    entity_id TEXT NOT NULL,   -- path or numeric ID
                                    content_hash TEXT,
                                    metadata TEXT,             -- JSON string (Window Title, App Name, etc.)
                                    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
                                );
                                
                                -- Index for faster mapping
                                CREATE INDEX IF NOT EXISTS idx_kms_vector_map_entity ON kms_vector_map(entity_type, entity_id);
                            "#,
                            kind: MigrationKind::Up,
                        },
                        Migration {
                            version: 6,
                            description: "kms_fts5_hybrid_search",
                            sql: r#"
                                -- Virtual table for Full Text Search (FTS5)
                                CREATE VIRTUAL TABLE IF NOT EXISTS kms_notes_fts USING fts5(
                                    title,
                                    content_preview,
                                    content='kms_notes',
                                    content_rowid='id'
                                );

                                -- Triggers to keep FTS table synchronized with `kms_notes`
                                CREATE TRIGGER IF NOT EXISTS kms_notes_ai AFTER INSERT ON kms_notes
                                BEGIN
                                    INSERT INTO kms_notes_fts (rowid, title, content_preview)
                                    VALUES (new.id, new.title, new.content_preview);
                                END;

                                CREATE TRIGGER IF NOT EXISTS kms_notes_ad AFTER DELETE ON kms_notes
                                BEGIN
                                    INSERT INTO kms_notes_fts (kms_notes_fts, rowid, title, content_preview)
                                    VALUES ('delete', old.id, old.title, old.content_preview);
                                END;

                                CREATE TRIGGER IF NOT EXISTS kms_notes_au AFTER UPDATE ON kms_notes
                                BEGIN
                                    INSERT INTO kms_notes_fts (kms_notes_fts, rowid, title, content_preview)
                                    VALUES ('delete', old.id, old.title, old.content_preview);
                                    INSERT INTO kms_notes_fts (rowid, title, content_preview)
                                    VALUES (new.id, new.title, new.content_preview);
                                END;
                            "#,
                            kind: MigrationKind::Up,
                        },
                        Migration {
                            version: 7,
                            description: "kms_fts5_backfill_existing",
                            sql: r#"
                                INSERT INTO kms_notes_fts (rowid, title, content_preview)
                                SELECT id, title, content_preview FROM kms_notes
                                WHERE id NOT IN (SELECT rowid FROM kms_notes_fts);
                            "#,
                            kind: MigrationKind::Up,
                        },
                        Migration {
                            version: 8,
                            description: "kms_sync_status_and_errors",
                            sql: r#"
                                ALTER TABLE kms_notes ADD COLUMN sync_status TEXT DEFAULT 'indexed';
                                ALTER TABLE kms_notes ADD COLUMN last_error TEXT;
                            "#,
                            kind: MigrationKind::Up,
                        },
                        Migration {
                            version: 9,
                            description: "kms_granular_index_status",
                            sql: r#"
                                CREATE TABLE IF NOT EXISTS kms_index_status (
                                    entity_type TEXT NOT NULL,
                                    entity_id TEXT NOT NULL,
                                    status TEXT NOT NULL, -- 'indexed', 'failed'
                                    error TEXT,
                                    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                                    PRIMARY KEY (entity_type, entity_id)
                                );
                            "#,
                            kind: MigrationKind::Up,
                        },
                        Migration {
                            version: 10,
                            description: "kms_unified_hybrid_fts",
                            sql: r#"
                                -- Create a unified FTS table for all entity types
                                CREATE VIRTUAL TABLE IF NOT EXISTS kms_unified_fts USING fts5(
                                    entity_type UNINDEXED,
                                    entity_id UNINDEXED,
                                    title,
                                    content,
                                    tokenize='porter'
                                );

                                -- Trigger to sync NEW notes to the unified FTS table
                                CREATE TRIGGER IF NOT EXISTS kms_notes_sync_fts_ai AFTER INSERT ON kms_notes
                                BEGIN
                                    INSERT INTO kms_unified_fts (entity_type, entity_id, title, content)
                                    VALUES ('note', new.path, new.title, new.content_preview);
                                END;

                                -- Trigger to sync UPDATED notes
                                CREATE TRIGGER IF NOT EXISTS kms_notes_sync_fts_au AFTER UPDATE ON kms_notes
                                BEGIN
                                    DELETE FROM kms_unified_fts WHERE entity_type = 'note' AND entity_id = old.path;
                                    INSERT INTO kms_unified_fts (entity_type, entity_id, title, content)
                                    VALUES ('note', new.path, new.title, new.content_preview);
                                END;

                                -- Trigger to sync DELETED notes
                                CREATE TRIGGER IF NOT EXISTS kms_notes_sync_fts_ad AFTER DELETE ON kms_notes
                                BEGIN
                                    DELETE FROM kms_unified_fts WHERE entity_type = 'note' AND entity_id = old.path;
                                END;

                                -- Backfill existing notes into unified FTS
                                INSERT INTO kms_unified_fts (entity_type, entity_id, title, content)
                                SELECT 'note', path, title, content_preview FROM kms_notes;
                            "#,
                            kind: MigrationKind::Up,
                        },
                        Migration {
                            version: 11,
                            description: "snippets_add_trigger_type_and_rich_text",
                            sql: r#"
                                ALTER TABLE snippets ADD COLUMN trigger_type TEXT NOT NULL DEFAULT 'suffix';
                                ALTER TABLE snippets ADD COLUMN html_content TEXT;
                                ALTER TABLE snippets ADD COLUMN rtf_content TEXT;
                            "#,
                            kind: MigrationKind::Up,
                        },
                        Migration {
                            version: 12,
                            description: "clipboard_history_add_rich_text",
                            sql: r#"
                                ALTER TABLE clipboard_history ADD COLUMN html_content TEXT;
                                ALTER TABLE clipboard_history ADD COLUMN rtf_content TEXT;
                            "#,
                            kind: MigrationKind::Up,
                        },
                        Migration {
                            version: 13,
                            description: "add_case_adaptive_to_snippets",
                            sql: "ALTER TABLE snippets ADD COLUMN case_adaptive TEXT DEFAULT 'true';",
                            kind: MigrationKind::Up,
                        },
                        Migration {
                            version: 14,
                            description: "kms_skill_hub_foundation",
                            sql: r#"
                                CREATE TABLE IF NOT EXISTS kms_skills (
                                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                                    name TEXT NOT NULL UNIQUE,
                                    description TEXT NOT NULL,
                                    version TEXT,
                                    path TEXT NOT NULL,
                                    instructions TEXT,
                                    last_modified TEXT,
                                    sync_status TEXT DEFAULT 'idle',
                                    last_error TEXT
                                );

                                -- Sync skills to unified FTS
                                CREATE TRIGGER IF NOT EXISTS kms_skills_sync_fts_ai AFTER INSERT ON kms_skills
                                BEGIN
                                    INSERT INTO kms_unified_fts (entity_type, entity_id, title, content)
                                    VALUES ('skill', new.name, new.name, new.description || ' ' || COALESCE(new.instructions, ''));
                                END;

                                CREATE TRIGGER IF NOT EXISTS kms_skills_sync_fts_au AFTER UPDATE ON kms_skills
                                BEGIN
                                    DELETE FROM kms_unified_fts WHERE entity_type = 'skill' AND entity_id = old.name;
                                    INSERT INTO kms_unified_fts (entity_type, entity_id, title, content)
                                    VALUES ('skill', new.name, new.name, new.description || ' ' || COALESCE(new.instructions, ''));
                                END;

                                CREATE TRIGGER IF NOT EXISTS kms_skills_sync_fts_ad AFTER DELETE ON kms_skills
                                BEGIN
                                    DELETE FROM kms_unified_fts WHERE entity_type = 'skill' AND entity_id = old.name;
                                END;
                            "#,
                            kind: MigrationKind::Up,
                        },
                        Migration {
                            version: 15,
                            description: "kms_cleanup_legacy_fts",
                            sql: r#"
                                -- Remove legacy note-specific FTS table and triggers
                                DROP TRIGGER IF EXISTS kms_notes_ai;
                                DROP TRIGGER IF EXISTS kms_notes_au;
                                DROP TRIGGER IF EXISTS kms_notes_ad;
                                DROP TABLE IF EXISTS kms_notes_fts;
                            "#,
                            kind: MigrationKind::Up,
                        },
                        Migration {
                            version: 16,
                            description: "kms_notes_wiki_pagerank_materialized",
                            sql: r#"
                                ALTER TABLE kms_notes ADD COLUMN wiki_pagerank REAL;
                                CREATE TABLE IF NOT EXISTS kms_graph_meta (
                                    key TEXT PRIMARY KEY NOT NULL,
                                    value TEXT NOT NULL
                                );
                            "#,
                            kind: MigrationKind::Up,
                        },
                        Migration {
                            version: 17,
                            description: "kms_notes_embedding_identity_columns",
                            sql: r#"
                                ALTER TABLE kms_notes ADD COLUMN embedding_model_id TEXT;
                                ALTER TABLE kms_notes ADD COLUMN embedding_policy_sig TEXT;
                            "#,
                            kind: MigrationKind::Up,
                        },
                        Migration {
                            version: 18,
                            description: "kms_ui_state_sidebar_lists",
                            sql: r#"
                                CREATE TABLE IF NOT EXISTS kms_ui_state (
                                    key TEXT PRIMARY KEY NOT NULL,
                                    value TEXT NOT NULL
                                );
                            "#,
                            kind: MigrationKind::Up,
                        },
                    ],
                )
                .build(),
        )
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.emit("window-closed-to-tray", ());
                    let _ = window.hide();
                }
            }
        })
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.set_focus();
                let _ = win.show();
                let _ = win.unminimize();
            }
            let _ = app.emit("secondary-instance-args", args);
        }))
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_shortcuts(["F11", "Shift+Alt+Space"])
                .expect("global shortcuts")
                .with_handler(|app, shortcut, _event| {
                    let s = shortcut.to_string();
                    if s.eq_ignore_ascii_case("Shift+Alt+Space") {
                        digicore_text_expander::application::ghost_follower::capture_target_window_for_quick_search_launch_global();
                        let _ = app.emit("show-quick-search", ());
                        return;
                    }
                    if s == "F11" {
                         // F11 now just brings window to front if not visible, 
                         // or we can just leave it as manual override if needed.
                         // But for auto-popup, we don't need logic here.
                         let _ = app.emit("show-main-window", ());
                    }
                })
                .build(),
        )
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("digicore-text-expander".to_string()),
                    }),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Webview),
                ])
                .build(),
        )
        .setup(move |app| {
            let handle = app.handle().clone();
            *app_handle_for_setup.lock().unwrap() = Some(handle.clone());
            
            // 1. Wire up callbacks IMMEDIATELY (prevents "NO callback set" warnings)
            let handle_suggestor = handle.clone();
            let cb = std::sync::Arc::new(move || {
                log::info!("[GhostSuggestor] on_change: emitting ghost-suggestor-update");
                let _ = handle_suggestor.emit("ghost-suggestor-update", ());
            });
            ghost_suggestor::set_on_change_callback(Some(cb));

            let handle_discovery = handle.clone();
            discovery::set_suggestion_callback(move |phrase, count| {
                ghost_suggestor::set_pending_discovery_for_notification(phrase.to_string(), count);
                let _ = handle_discovery.emit("discovery-suggestion", (phrase.to_string(), count));
            });

            // 2. STAGGERED Background Initialization
            let state_for_init = state_for_tray.clone();
            let corpus_service_for_init = corpus_service.clone();
            let handle_for_init = handle.clone();
            tauri::async_runtime::spawn(async move {
                // T+1s: Database, Conflict Sync, and Script Libraries
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                log::info!("[Startup] Running KMS Boot Sync");
                let _ = kms_sync_service::KmsSyncService::run_boot_sync().await;
                
                log::info!("[Startup] Initializing repositories and scripts");
                spawn_variable_input_poller(handle_for_init.clone());
                load_and_apply_script_libraries();
                let db_path = clipboard_repository::default_db_path();
                if let Err(e) = clipboard_repository::init(db_path.clone()) {
                    log::error!("[Startup][Clipboard][SQLite] failed: {}", e);
                }
                if let Err(e) = kms_repository::init(db_path) {
                    log::error!("[Startup][KMS][SQLite] failed: {}", e);
                } else {
                    let handle_observer = handle_for_init.clone();
                    digicore_text_expander::application::clipboard_history::set_entry_observer(Some(std::sync::Arc::new(
                        move |entry| {
                            if entry.content == "[Image]" {
                                log::debug!("[Clipboard][Observer] Image marker detected");
                                crate::clipboard_sqlite_sync::sync_current_clipboard_image_to_sqlite(entry.process_name.clone(), entry.window_title.clone(), Some(&handle_observer));
                                return;
                            }
                            log::debug!("[Clipboard][Observer] Text entry: '{}'", entry.content);
                            match crate::clipboard_text_persistence::persist_clipboard_entry_with_settings(
                                &entry.content,
                                &entry.process_name,
                                &entry.window_title,
                                entry.file_list.clone(),
                            ) {
                                Ok(Some(id)) => {
                                    digicore_text_expander::application::expansion_diagnostics::push(
                                        "info",
                                        format!("[Clipboard][capture.accepted] id={} app='{}' chars={}", 
                                            id, entry.process_name, entry.content.chars().count()),
                                    );
                                    
                                    // Trigger auto-indexing
                                    let h = handle_observer.clone();
                                    let service = h.state::<Arc<crate::indexing_service::KmsIndexingService>>().inner().clone();
                                    let entity_id = id.to_string();
                                    tauri::async_runtime::spawn(async move {
                                        let _ = service.index_single_item(&h, "clipboard", &entity_id).await;
                                    });
                                }
                                Ok(None) => {
                                    digicore_text_expander::application::expansion_diagnostics::push(
                                        "warn",
                                        format!("[Clipboard][capture.skipped] app='{}'", entry.process_name),
                                    );
                                }
                                Err(err) => {
                                    log::warn!("[ClipboardHistory][SQLite] insert failed: {}", err);
                                    digicore_text_expander::application::expansion_diagnostics::push(
                                        "error",
                                        format!("[Clipboard][persistence.write_err] {}", err),
                                    );
                                }
                            }
                        },
                    )));
                }


                // T+3s: Background Listener and Sync
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                log::info!("[Startup] Starting listener and sync");
                let (lib_cloned, paused_cloned, cfg_cloned) = {
                    let g = state_for_init.lock().unwrap();
                    (
                        g.library.clone(),
                        g.expansion_paused,
                        GhostConfig {
                            suggestor_enabled: g.ghost_suggestor_enabled,
                            suggestor_debounce_ms: g.ghost_suggestor_debounce_ms,
                            suggestor_display_secs: g.ghost_suggestor_display_secs,
                            suggestor_snooze_duration_mins: g.ghost_suggestor_snooze_duration_mins,
                            suggestor_offset_x: g.ghost_suggestor_offset_x,
                            suggestor_offset_y: g.ghost_suggestor_offset_y,
                            follower_enabled: g.ghost_follower.config.enabled,
                            follower_edge_right: g.ghost_follower.config.edge == digicore_text_expander::application::ghost_follower::FollowerEdge::Right,
                            follower_monitor_anchor: match g.ghost_follower.config.monitor_anchor {
                                digicore_text_expander::application::ghost_follower::MonitorAnchor::Secondary => 1,
                                digicore_text_expander::application::ghost_follower::MonitorAnchor::Current => 2,
                                _ => 0,
                            },
                            follower_search: g.ghost_follower.search_filter.clone(),
                            follower_hover_preview: g.ghost_follower.config.hover_preview,
                            follower_collapse_delay_secs: g.ghost_follower.config.collapse_delay_secs,
                        }
                    )
                };
                let _ = start_listener(lib_cloned, Some(corpus_service_for_init), None);
                set_expansion_paused(paused_cloned);
                sync_ghost_config(cfg_cloned);

                // Run sync in its own task so it doesn't block the startup chain
                let handle_sync = handle_for_init.clone();
                tauri::async_runtime::spawn(async move {
                    log::info!("[Startup] Initializing runtime clipboard sync (background)");
                    crate::clipboard_sqlite_sync::sync_runtime_clipboard_entries_to_sqlite(&handle_sync);
                    log::info!("[Startup] Background sync completed");
                });


                // T+4s: KMS Vault Reconciliation & Watcher
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                log::info!("[Startup] Initializing KMS reconciliation sync and watcher");
                let vault_path = {
                    let g = state_for_init.lock().unwrap();
                    g.kms_vault_path.clone()
                };
                if !vault_path.is_empty() {
                    let vault_path_buf = PathBuf::from(&vault_path);
                    if vault_path_buf.exists() {
                        let handle_kms = handle_for_init.clone();
                        let kms_path_clone = vault_path_buf.clone();
                        tauri::async_runtime::spawn(async move {
                            log::info!("[KMS][Startup] Starting vault reconciliation...");
                            let _ = handle_kms.emit("kms-sync-status", "Indexing...");
                            let _ = crate::kms_sync_orchestration::sync_vault_files_to_db_internal(
                                &handle_kms,
                                &kms_path_clone,
                            )
                            .await;
                            let _ = handle_kms.emit("kms-sync-status", "Idle");
                            let _ = handle_kms.emit("kms-sync-complete", ());
                            log::info!("[KMS][Startup] Vault reconciliation completed");
                        });
                        
                        // Start watcher
                        crate::kms_watcher::start_kms_watcher(handle_for_init.clone(), vault_path_buf);
                        log::info!("[KMS][Startup] Filesystem watcher initialized");
                    }
                }

                // T+4.5s: Skill Hub Sync
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                log::info!("[SkillSync][Startup] Initializing Skill Hub sync...");
                let skill_paths = skill_sync::get_default_skill_paths();
                let handle_skills = handle_for_init.clone();
                tauri::async_runtime::spawn(async move {
                    for path in &skill_paths {
                        let _ = skill_sync::sync_skills_dir(&handle_skills, path).await;
                    }
                    skill_sync::start_skill_watcher(handle_skills, skill_paths);
                });


                // T+5s: Appearance enforcement loop
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                log::info!("[Startup] Starting appearance enforcement loop");
                std::thread::spawn(|| {
                    loop {
                        let _ = std::panic::catch_unwind(|| {
                            crate::appearance_enforcement::enforce_appearance_transparency_rules();
                        });
                        std::thread::sleep(std::time::Duration::from_secs(3));
                    }
                });
                log::info!("[Startup] All background tasks staggered and running");
            });

            #[cfg(any(windows, target_os = "linux"))]
            {
                // Defer deep link to avoid potential registry locks on main
                let handle_dl = handle.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    use tauri_plugin_deep_link::DeepLinkExt;
                    let _ = handle_dl.deep_link().register_all();
                });
            }
            
            let args: Vec<String> = std::env::args().collect();
            let _ = app.emit("initial-cli-args", args);

            // Create Ghost Follower, Suggestor, and Quick Search windows from Rust (avoids URL/path issues).
            // Use a thread to avoid Windows deadlocks when creating windows.
            let handle_for_ghost = app.handle().clone();
            std::thread::spawn(move || {
                use tauri::WebviewUrl;
                let _ = (|| -> Result<(), Box<dyn std::error::Error>> {
                    let suggestor = tauri::WebviewWindowBuilder::new(
                        &handle_for_ghost,
                        "ghost-suggestor",
                        WebviewUrl::App("ghost-suggestor.html".into()),
                    )
                    .title("Ghost Suggestor")
                    .inner_size(320.0, 260.0)
                    .decorations(false)
                    .transparent(true)
                    .always_on_top(true)
                    .visible(false)
                    .build()?;
                    log::info!("[GhostSuggestor] window created from Rust");
                    let handle_suggestor = handle_for_ghost.clone();
                    suggestor.once("tauri://created", move |_| {
                        log::info!("[GhostSuggestor] webview ready, can receive ghost-suggestor-update events");
                        let _ = handle_suggestor.emit("ghost-suggestor-update", ());
                    });

                    let follower = tauri::WebviewWindowBuilder::new(
                        &handle_for_ghost,
                        "ghost-follower",
                        WebviewUrl::App("ghost-follower.html".into()),
                    )
                    .title("Ghost Follower")
                    .inner_size(64.0, 36.0)
                    .decorations(false)
                    .resizable(false)
                    .maximizable(false)
                    .minimizable(false)
                    .shadow(false)
                    .transparent(true)
                    .always_on_top(true)
                    .visible(true)
                    .build()?;

                    // Defensive enforcement: keep Ghost Follower borderless even if platform/window-state
                    // attempts to restore framed styles from prior sessions.
                    let _ = follower.set_decorations(false);
                    let _ = follower.set_resizable(false);
                    let _ = follower.set_maximizable(false);
                    let _ = follower.set_minimizable(false);

                    log::info!("[GhostFollower] window created from Rust");
                    let handle_emit = handle_for_ghost.clone();
                    follower.once("tauri://created", move |_| {
                        log::info!("[GhostFollower] webview ready, emitting update");
                        let _ = handle_emit.emit("ghost-follower-update", ());
                    });

                    let quick_search = tauri::WebviewWindowBuilder::new(
                        &handle_for_ghost,
                        "quick-search",
                        WebviewUrl::App("quick-search.html".into()),
                    )
                    .title("Quick Search")
                    .inner_size(520.0, 540.0)
                    .decorations(false)
                    .transparent(false)
                    .always_on_top(true)
                    .shadow(true)
                    .visible(false)
                    .build()?;

                    let _ = quick_search.set_resizable(false);
                    let _ = quick_search.set_maximizable(false);
                    let _ = quick_search.set_minimizable(false);
                    let _ = quick_search.hide();
                    log::info!("[QuickSearch] window created from Rust");
                    let handle_quick = handle_for_ghost.clone();
                    quick_search.once("tauri://created", move |_| {
                        if let Some(win) = handle_quick.get_webview_window("quick-search") {
                            let _ = win.hide();
                        }
                        let _ = handle_quick.emit("quick-search-refresh", ());
                    });

                    let variable_input = tauri::WebviewWindowBuilder::new(
                        &handle_for_ghost,
                        "variable-input",
                         WebviewUrl::App("variable-input.html".into()),
                    )
                    .title("Snippet Input Required")
                    .inner_size(460.0, 520.0)
                    .decorations(false)
                    .transparent(true)
                    .always_on_top(true)
                    .shadow(true)
                    .visible(false)
                    .build()?;

                    let _ = variable_input.set_resizable(false);
                    let _ = variable_input.set_maximizable(false);
                    let _ = variable_input.set_minimizable(false);
                    let _ = variable_input.hide();
                    log::info!("[VariableInput] window created from Rust");

                    Ok(())
                })();
            });

            // Tray menu: View Management Console, Quick Search, Toggle Pause/Unpause, View Ghost Follower, Exit
            let paused = state_for_tray
                .lock()
                .map(|g| g.expansion_paused)
                .unwrap_or(false);
            install_tray_menu(&handle, paused);

            let tray_state_for_menu = state_for_tray.clone();
            app.on_menu_event(move |app_handle, event| match event.id.as_ref() {
                "view_console" => {
                    if let Some(win) = app_handle.get_webview_window("main") {
                        let _ = win.show();
                        let _ = win.set_focus();
                        let _ = win.unminimize();
                    }
                }
                "quick_search" => {
                    if let Ok(mut guard) = tray_state_for_menu.lock() {
                        digicore_text_expander::application::ghost_follower::capture_target_window_for_quick_search_launch(&mut guard.ghost_follower);
                    }
                    if let Some(win) = app_handle.get_webview_window("quick-search") {
                        let _ = win.show();
                        let _ = win.set_focus();
                        let _ = win.unminimize();
                    }
                    let _ = app_handle.emit("quick-search-refresh", ());
                }
                "toggle_pause" => {
                    let paused = if let Ok(mut guard) = tray_state_for_menu.lock() {
                        guard.expansion_paused = !guard.expansion_paused;
                        guard.expansion_paused
                    } else {
                        false
                    };
                    set_expansion_paused(paused);
                    if let Err(e) = persist_settings_for_state(&tray_state_for_menu) {
                        log::warn!("[Tray] persist pause toggle failed: {}", e);
                    }
                    install_tray_menu(app_handle, paused);
                    let _ = app_handle.emit(
                        "tray-expansion-paused-changed",
                        serde_json::json!({ "paused": paused }),
                    );
                }
                "view_follower" => {
                    if let Some(win) = app_handle.get_webview_window("ghost-follower") {
                        let _ = win.show();
                        let _ = win.set_focus();
                        let _ = win.unminimize();
                    }
                }
                "quit" => {
                    app_handle.exit(0);
                }
                _ => {}
            });

            let tray_state_for_listen = state_for_tray.clone();
            app.listen("show-quick-search", move |_event| {
                if let Ok(mut guard) = tray_state_for_listen.lock() {
                    digicore_text_expander::application::ghost_follower::capture_target_window_for_quick_search_launch(&mut guard.ghost_follower);
                }
                if let Some(win) = handle.get_webview_window("quick-search") {
                    let _ = win.show();
                    let _ = win.set_focus();
                    let _ = win.unminimize();
                }
                let _ = handle.emit("quick-search-refresh", ());
            });

            Ok(())
        })
        .invoke_handler(taurpc::create_ipc_handler(
            api::ApiImpl {
                state: app_state,
                app_handle,
                clipboard: clipboard.clone(),
            }
            .into_handler(),
        ))
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(move |_app_handle, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                save_all_on_exit(&state_for_exit);
            }
        });
}

/// UI preferences DTO for frontend.
#[taurpc::ipc_type]
pub struct UiPrefsDto {
    pub last_tab: u32,
    pub column_order: Vec<String>,
}

/// Config update DTO. All fields optional; only provided fields are updated.
#[taurpc::ipc_type]
#[derive(Default)]
pub struct ConfigUpdateDto {
    pub library_path: Option<String>,
    pub expansion_log_path: Option<String>,
    pub kms_vault_path: Option<String>,
    pub expansion_paused: Option<bool>,
    pub template_date_format: Option<String>,
    pub template_time_format: Option<String>,
    pub sync_url: Option<String>,
    pub discovery_enabled: Option<bool>,
    pub discovery_threshold: Option<u32>,
    pub discovery_lookback: Option<u32>,
    pub discovery_min_len: Option<u32>,
    pub discovery_max_len: Option<u32>,
    pub discovery_excluded_apps: Option<String>,
    pub discovery_excluded_window_titles: Option<String>,
    pub ghost_suggestor_enabled: Option<bool>,
    pub ghost_suggestor_debounce_ms: Option<u32>,
    pub ghost_suggestor_display_secs: Option<u32>,
    pub ghost_suggestor_snooze_duration_mins: Option<u32>,
    pub ghost_suggestor_offset_x: Option<i32>,
    pub ghost_suggestor_offset_y: Option<i32>,
    pub ghost_follower_enabled: Option<bool>,
    pub ghost_follower_edge_right: Option<bool>,
    pub ghost_follower_monitor_anchor: Option<u32>,
    pub ghost_follower_search: Option<String>,
    pub ghost_follower_hover_preview: Option<bool>,
    pub ghost_follower_collapse_delay_secs: Option<u32>,
    pub ghost_follower_opacity: Option<u32>,
    pub ghost_follower_mode: Option<String>,
    pub ghost_follower_expand_trigger: Option<String>,
    pub ghost_follower_expand_delay_ms: Option<u32>,
    pub ghost_follower_clipboard_depth: Option<u32>,
    pub clip_history_max_depth: Option<u32>,
    pub script_library_run_disabled: Option<bool>,
    pub script_library_run_allowlist: Option<String>,

    pub corpus_enabled: Option<bool>,
    pub corpus_output_dir: Option<String>,
    pub corpus_snapshot_dir: Option<String>,
    pub corpus_shortcut_modifiers: Option<u32>,
    pub corpus_shortcut_key: Option<u32>,

    pub extraction_row_overlap_tolerance: Option<f32>,
    pub extraction_cluster_threshold_factor: Option<f32>,
    pub extraction_zone_proximity: Option<f32>,
    pub extraction_cross_zone_gap_factor: Option<f32>,
    pub extraction_same_zone_gap_factor: Option<f32>,
    pub extraction_significant_gap_gate: Option<f32>,
    pub extraction_char_width_factor: Option<f32>,
    pub extraction_bridged_threshold: Option<f32>,
    pub extraction_word_spacing_factor: Option<f32>,

    pub extraction_footer_triggers: Option<String>,
    pub extraction_table_min_contiguous_rows: Option<u32>,
    pub extraction_table_min_avg_segments: Option<f32>,

    pub extraction_adaptive_plaintext_cluster_factor: Option<f32>,
    pub extraction_adaptive_plaintext_gap_gate: Option<f32>,
    pub extraction_adaptive_table_cluster_factor: Option<f32>,
    pub extraction_adaptive_table_gap_gate: Option<f32>,
    pub extraction_adaptive_column_cluster_factor: Option<f32>,
    pub extraction_adaptive_column_gap_gate: Option<f32>,
    pub extraction_adaptive_plaintext_cross_factor: Option<f32>,
    pub extraction_adaptive_table_cross_factor: Option<f32>,
    pub extraction_adaptive_column_cross_factor: Option<f32>,

    pub extraction_refinement_entropy_threshold: Option<f32>,
    pub extraction_refinement_cluster_threshold_modifier: Option<f32>,
    pub extraction_refinement_cross_zone_gap_modifier: Option<f32>,

    pub extraction_classifier_gutter_weight: Option<f32>,
    pub extraction_classifier_density_weight: Option<f32>,
    pub extraction_classifier_multicolumn_density_max: Option<f32>,
    pub extraction_classifier_table_density_min: Option<f32>,
    pub extraction_classifier_table_entropy_min: Option<f32>,

    pub extraction_columns_min_contiguous_rows: Option<u32>,
    pub extraction_columns_gutter_gap_factor: Option<f32>,
    pub extraction_columns_gutter_void_tolerance: Option<f32>,
    pub extraction_columns_edge_margin_tolerance: Option<f32>,

    pub extraction_headers_max_width_ratio: Option<f32>,
    pub extraction_headers_centered_tolerance: Option<f32>,
    pub extraction_headers_h1_size_multiplier: Option<f32>,
    pub extraction_headers_h2_size_multiplier: Option<f32>,
    pub extraction_headers_h3_size_multiplier: Option<f32>,

    pub extraction_scoring_jitter_penalty_weight: Option<f32>,
    pub extraction_scoring_size_penalty_weight: Option<f32>,
    pub extraction_scoring_low_confidence_threshold: Option<f32>,

    pub extraction_layout_row_lookback: Option<u32>,
    pub extraction_layout_table_break_threshold: Option<f32>,
    pub extraction_layout_paragraph_break_threshold: Option<f32>,
    pub extraction_layout_max_space_clamp: Option<u32>,
    pub extraction_tables_column_jitter_tolerance: Option<f32>,
    pub extraction_tables_merge_y_gap_max: Option<f32>,
    pub extraction_tables_merge_y_gap_min: Option<f32>,

    pub kms_graph_k_means_max_k: Option<u32>,
    pub kms_graph_k_means_iterations: Option<u32>,
    pub kms_graph_ai_beam_max_nodes: Option<u32>,
    pub kms_graph_ai_beam_similarity_threshold: Option<f32>,
    pub kms_graph_ai_beam_max_edges: Option<u32>,
    pub kms_graph_enable_ai_beams: Option<bool>,
    pub kms_graph_enable_semantic_clustering: Option<bool>,
    pub kms_graph_enable_leiden_communities: Option<bool>,
    pub kms_graph_semantic_max_notes: Option<u32>,
    pub kms_graph_warn_note_threshold: Option<u32>,
    pub kms_graph_beam_max_pair_checks: Option<u32>,
    pub kms_graph_enable_semantic_knn_edges: Option<bool>,
    pub kms_graph_semantic_knn_per_note: Option<u32>,
    pub kms_graph_semantic_knn_min_similarity: Option<f32>,
    pub kms_graph_semantic_knn_max_edges: Option<u32>,
    pub kms_graph_semantic_knn_max_pair_checks: Option<u32>,
    pub kms_graph_auto_paging_enabled: Option<bool>,
    pub kms_graph_auto_paging_note_threshold: Option<u32>,
    pub kms_graph_vault_overrides_json: Option<String>,

    pub kms_graph_bloom_enabled: Option<bool>,
    pub kms_graph_bloom_strength: Option<f32>,
    pub kms_graph_bloom_radius: Option<f32>,
    pub kms_graph_bloom_threshold: Option<f32>,
    pub kms_graph_hex_cell_radius: Option<f32>,
    pub kms_graph_hex_layer_opacity: Option<f32>,
    pub kms_graph_hex_stroke_width: Option<f32>,
    pub kms_graph_hex_stroke_opacity: Option<f32>,

    pub kms_graph_pagerank_iterations: Option<u32>,
    pub kms_graph_pagerank_local_iterations: Option<u32>,
    pub kms_graph_pagerank_damping: Option<f32>,
    pub kms_graph_pagerank_scope: Option<String>,
    pub kms_graph_background_wiki_pagerank_enabled: Option<bool>,

    pub kms_graph_temporal_window_enabled: Option<bool>,
    pub kms_graph_temporal_default_days: Option<u32>,
    pub kms_graph_temporal_include_notes_without_mtime: Option<bool>,
    pub kms_graph_temporal_edge_recency_enabled: Option<bool>,
    pub kms_graph_temporal_edge_recency_strength: Option<f32>,
    pub kms_graph_temporal_edge_recency_half_life_days: Option<f32>,
    pub kms_search_min_similarity: Option<f32>,
    pub kms_search_include_embedding_diagnostics: Option<bool>,
    pub kms_search_default_mode: Option<String>,
    pub kms_search_default_limit: Option<u32>,

    pub kms_embedding_model_id: Option<String>,
    pub kms_embedding_batch_notes_per_tick: Option<u32>,
    pub kms_embedding_chunk_enabled: Option<bool>,
    pub kms_embedding_chunk_max_chars: Option<u32>,
    pub kms_embedding_chunk_overlap_chars: Option<u32>,

    pub kms_graph_sprite_label_max_dpr_scale: Option<f32>,
    pub kms_graph_sprite_label_min_res_scale: Option<f32>,
    pub kms_graph_webworker_layout_threshold: Option<u32>,
    pub kms_graph_webworker_layout_max_ticks: Option<u32>,
    pub kms_graph_webworker_layout_alpha_min: Option<f32>,
}

#[taurpc::ipc_type]
pub struct CopyToClipboardConfigDto {
    pub enabled: bool,
    pub image_capture_enabled: bool,
    pub min_log_length: u32,
    pub mask_cc: bool,
    pub mask_ssn: bool,
    pub mask_email: bool,
    pub blacklist_processes: String,
    pub max_history_entries: u32,
    pub json_output_enabled: bool,
    pub json_output_dir: String,
    pub image_storage_dir: String,
    pub ocr_enabled: bool,
}

#[taurpc::ipc_type]
pub struct CopyToClipboardStatsDto {
    pub total_entries: u32,
}

#[taurpc::ipc_type]
pub struct ScriptingHttpConfigDto {
    pub timeout_secs: u32,
    pub retry_count: u32,
    pub retry_delay_ms: u32,
    pub use_async: bool,
}

#[taurpc::ipc_type]
pub struct ScriptingPyConfigDto {
    pub enabled: bool,
    pub path: String,
    pub library_path: String,
}

#[taurpc::ipc_type]
pub struct ScriptingLuaConfigDto {
    pub enabled: bool,
    pub path: String,
    pub library_path: String,
}

#[taurpc::ipc_type]
pub struct ScriptingDslConfigDto {
    pub enabled: bool,
}

#[taurpc::ipc_type]
pub struct ScriptingEngineConfigDto {
    pub dsl: ScriptingDslConfigDto,
    pub http: ScriptingHttpConfigDto,
    pub py: ScriptingPyConfigDto,
    pub lua: ScriptingLuaConfigDto,
}

#[taurpc::ipc_type]
pub struct AppearanceTransparencyRuleDto {
    pub app_process: String,
    pub opacity: u32,
    pub enabled: bool,
}

#[taurpc::ipc_type]
pub struct SettingsImportResultDto {
    pub applied_groups: Vec<String>,
    pub skipped_groups: Vec<String>,
    pub warnings: Vec<String>,
    pub updated_keys: u32,
    pub appearance_rules_applied: u32,
    pub theme: Option<String>,
    pub autostart_enabled: Option<bool>,
}

#[taurpc::ipc_type]
pub struct SettingsBundlePreviewDto {
    pub path: String,
    pub schema_version: String,
    pub available_groups: Vec<String>,
    pub warnings: Vec<String>,
    pub valid: bool,
}

#[taurpc::ipc_type]
pub struct ScriptingProfilePreviewDto {
    pub path: String,
    pub schema_version: String,
    pub available_groups: Vec<String>,
    pub warnings: Vec<String>,
    pub valid: bool,
    pub signed_bundle: bool,
    pub signature_valid: bool,
    pub migrated_from_schema: Option<String>,
    pub signature_key_id: Option<String>,
    pub signer_fingerprint: Option<String>,
    pub signer_trusted: bool,
}

#[taurpc::ipc_type]
pub struct ScriptingProfileImportResultDto {
    pub applied_groups: Vec<String>,
    pub skipped_groups: Vec<String>,
    pub warnings: Vec<String>,
    pub updated_keys: u32,
    pub schema_version_used: String,
    pub signature_valid: bool,
    pub migrated_from_schema: Option<String>,
    pub signer_fingerprint: Option<String>,
    pub signer_trusted: bool,
}

#[taurpc::ipc_type]
pub struct ScriptingProfileDiffEntryDto {
    pub group: String,
    pub field: String,
    pub current_value: String,
    pub incoming_value: String,
}

#[taurpc::ipc_type]
pub struct ScriptingProfileDryRunDto {
    pub path: String,
    pub selected_groups: Vec<String>,
    pub changed_groups: Vec<String>,
    pub estimated_updates: u32,
    pub warnings: Vec<String>,
    pub diff_entries: Vec<ScriptingProfileDiffEntryDto>,
    pub schema_version_used: String,
    pub signature_valid: bool,
    pub migrated_from_schema: Option<String>,
    pub signer_fingerprint: Option<String>,
    pub signer_trusted: bool,
}

#[taurpc::ipc_type]
pub struct ScriptingSignerRegistryDto {
    pub allow_unknown_signers: bool,
    pub trust_on_first_use: bool,
    pub trusted_fingerprints: Vec<String>,
    pub blocked_fingerprints: Vec<String>,
}

#[taurpc::ipc_type]
pub struct ScriptingDetachedSignatureExportDto {
    pub profile_path: String,
    pub signature_path: String,
    pub key_id: String,
    pub signer_fingerprint: String,
    pub payload_sha256: String,
}

#[cfg(test)]
mod tests {
    use super::tray_pause_menu_label;

    #[test]
    fn tray_pause_label_reflects_state() {
        assert_eq!(tray_pause_menu_label(false), "Toggle Pause - Running");
        assert_eq!(tray_pause_menu_label(true), "Toggle Unpause - Paused");
    }

    #[test]
    fn ipc_dtos_do_not_use_u64_or_u128_fields() {
        let src = include_str!("lib.rs");
        let lines: Vec<&str> = src.lines().collect();
        let mut i = 0usize;
        while i < lines.len() {
            if !lines[i].contains("#[taurpc::ipc_type]") {
                i += 1;
                continue;
            }

            // Move to struct declaration line.
            i += 1;
            while i < lines.len() && !lines[i].contains("pub struct ") {
                i += 1;
            }
            if i >= lines.len() {
                break;
            }
            let struct_line = lines[i].trim();
            let struct_name = struct_line
                .strip_prefix("pub struct ")
                .and_then(|s| s.split_whitespace().next())
                .unwrap_or("<unknown>");

            // Parse struct body by brace depth.
            let mut depth = 0i32;
            let mut started = false;
            while i < lines.len() {
                let line = lines[i];
                for ch in line.chars() {
                    if ch == '{' {
                        depth += 1;
                        started = true;
                    } else if ch == '}' {
                        depth -= 1;
                    }
                }

                if started {
                    let trimmed = line.trim();
                    if trimmed.starts_with("pub ")
                        && (trimmed.contains(": u64")
                            || trimmed.contains(": u128")
                            || trimmed.contains("<u64>")
                            || trimmed.contains("<u128>"))
                    {
                        panic!(
                            "taurpc ipc dto '{}' must not use u64/u128 field types: {}",
                            struct_name, trimmed
                        );
                    }
                    if depth == 0 {
                        i += 1;
                        break;
                    }
                }
                i += 1;
            }
        }
    }

    #[test]
    fn clip_history_depth_supports_unlimited_guard() {
        let lib_src = include_str!("lib.rs");
        assert!(
            !lib_src.contains(&format!("{}.{}", ".clamp", "(5, 5000)")),
            "clip_history_max_depth startup should allow 0=unlimited and high values"
        );
    }
}

#[taurpc::ipc_type]
pub struct SuggestionDto {
    pub trigger: String,
    pub content_preview: String,
    pub category: String,
}

/// Ghost Suggestor state for overlay.
#[taurpc::ipc_type]
pub struct GhostSuggestorStateDto {
    pub has_suggestions: bool,
    pub suggestions: Vec<SuggestionDto>,
    pub selected_index: u32,
    pub position: Option<(i32, i32)>,
    /// When true, enable mouse passthrough (e.g. when fading out per display_duration).
    pub should_passthrough: bool,
}

#[taurpc::ipc_type]
pub struct PinnedSnippetDto {
    pub trigger: String,
    pub content: String,
    pub content_preview: String,
    pub category: String,
    pub snippet_idx: u32,
}

#[taurpc::ipc_type]
pub struct GhostFollowerStateDto {
    pub enabled: bool,
    pub mode: String,
    pub expand_trigger: String,
    pub expand_delay_ms: u32,
    pub clipboard_depth: u32,
    pub pinned: Vec<PinnedSnippetDto>,
    pub search_filter: String,
    /// Position (x, y) for edge-anchored window. None on non-Windows.
    pub position: Option<(i32, i32)>,
    /// True when edge is Right (for positioner TopRight).
    pub edge_right: bool,
    /// True when monitor is Primary (positioner works best for primary).
    pub monitor_primary: bool,
    /// Configured max clipboard history depth (e.g. 100, 20).
    pub clip_history_max_depth: u32,
    /// True when ribbon should auto-collapse (no activity for collapse_delay_secs).
    pub should_collapse: bool,
    /// Seconds of inactivity before auto-collapsing. 0 = disabled.
    pub collapse_delay_secs: u32,
    /// Opacity 0.0-1.0 (10-100% from config).
    pub opacity: f64,
    /// True when position is from user drag (saved); use position directly, skip positioner.
    pub saved_position: bool,
}

#[taurpc::ipc_type]
pub struct InteractiveVarDto {
    pub tag: String,
    pub label: String,
    pub var_type: String,
    pub options: Vec<String>,
}

#[taurpc::ipc_type]
pub struct PendingVariableInputDto {
    pub content: String,
    pub vars: Vec<InteractiveVarDto>,
    pub values: HashMap<String, String>,
    pub choice_indices: HashMap<String, u32>,
    pub checkbox_checked: HashMap<String, bool>,
}

#[taurpc::ipc_type]
pub struct SnippetLogicTestResultDto {
    pub result: String,
    pub requires_input: bool,
    pub vars: Vec<InteractiveVarDto>,
}

/// Expansion stats DTO for Analytics dashboard.
#[taurpc::ipc_type]
pub struct ExpansionStatsDto {
    pub total_expansions: u32,
    pub total_chars_saved: u32,
    pub estimated_time_saved_secs: f64,
    pub top_triggers: Vec<(String, u32)>,
}

/// Diagnostic entry DTO for Log tab.
#[taurpc::ipc_type]
pub struct DiagnosticEntryDto {
    pub timestamp_ms: u32,
    pub level: String,
    pub message: String,
}



/// Clipboard entry DTO (Instant not serializable).
#[taurpc::ipc_type]
pub struct ClipEntryDto {
    pub id: u32,
    pub content: String,
    pub process_name: String,
    pub window_title: String,
    pub length: u32,
    pub word_count: u32,
    pub created_at: String,
    pub entry_type: String,
    pub mime_type: Option<String>,
    pub image_path: Option<String>,
    pub thumb_path: Option<String>,
    pub image_width: Option<u32>,
    pub image_height: Option<u32>,
    pub image_bytes: Option<u32>,
    pub parent_id: Option<u32>,
    pub metadata: Option<String>,
    pub file_list: Option<Vec<String>>,
}
fn spawn_variable_input_poller(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            if variable_input::has_viewport_modal() {
                continue;
            }
            if let Some(pending) = variable_input::take_pending_expansion() {
                let vars = template_processor::collect_interactive_vars(&pending.content);
                if vars.is_empty() {
                    continue;
                }
                let mut values = HashMap::new();
                let mut choice_indices = HashMap::new();
                let mut checkbox_checked = HashMap::new();
                for v in &vars {
                    values.insert(v.tag.clone(), String::new());
                    if matches!(v.var_type, InteractiveVarType::Choice)
                        && !v.options.is_empty()
                    {
                        choice_indices.insert(v.tag.clone(), 0);
                        values.insert(v.tag.clone(), v.options[0].clone());
                    }
                    if matches!(v.var_type, InteractiveVarType::Checkbox) {
                        checkbox_checked.insert(v.tag.clone(), false);
                    }
                }
                variable_input::set_viewport_modal(variable_input::ViewportModalState {
                    content: pending.content,
                    vars,
                    values,
                    choice_indices,
                    checkbox_checked,
                    target_hwnd: pending.target_hwnd,
                    response_tx: pending.response_tx,
                });
                if let Some(win) = app.get_webview_window("variable-input") {
                    let _ = win.show();
                    let _ = win.unminimize();
                    let _ = win.set_focus();
                    let _ = win.set_always_on_top(true);
                    let _ = win.center();
                }
                let _ = app.emit("variable-input-refresh", ());
            }
        }
    });
}
