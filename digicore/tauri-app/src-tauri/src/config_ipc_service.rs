//! Bounded inbound service for config-oriented RPC orchestration.

use digicore_text_expander::application::clipboard_history::{self, ClipboardHistoryConfig};

use crate::app_settings_storage::persist_settings_to_storage;
use crate::clipboard_text_persistence::{load_copy_to_clipboard_config, save_copy_to_clipboard_config};

use super::*;

pub(crate) async fn save_settings(host: ApiImpl) -> Result<(), String> {
    let guard = host.state.lock().map_err(|e| e.to_string())?;
    persist_settings_to_storage(&*guard)
}

pub(crate) async fn update_config(host: ApiImpl, config: ConfigUpdateDto) -> Result<(), String> {
    let mut guard = host.state.lock().map_err(|e| e.to_string())?;
    let prev_pagerank_settings = (
        guard.kms_graph_pagerank_iterations,
        guard.kms_graph_pagerank_damping,
        guard.kms_graph_pagerank_scope.clone(),
    );
    let prev_embed_norm =
        embedding_service::normalized_embedding_model_id(&guard.kms_embedding_model_id);
    let prev_vault_buf = PathBuf::from(guard.kms_vault_path.clone());
    let prev_chunk_effective =
        crate::kms_graph_effective_params::effective_kms_embedding_chunk_config(
            &*guard,
            &prev_vault_buf,
        );
    if let Some(v) = config.expansion_paused {
        guard.expansion_paused = v;
    }
    if let Some(ref v) = config.template_date_format {
        guard.template_date_format = v.clone();
    }
    if let Some(ref v) = config.template_time_format {
        guard.template_time_format = v.clone();
    }
    if let Some(ref v) = config.sync_url {
        guard.sync_url = v.clone();
    }
    if let Some(ref v) = config.expansion_log_path {
        guard.expansion_log_path = v.clone();
        digicore_text_expander::application::expansion_logger::set_log_path(v.clone());
    }
    if let Some(v) = config.discovery_enabled {
        guard.discovery_enabled = v;
    }
    if let Some(v) = config.discovery_threshold {
        guard.discovery_threshold = v;
    }
    if let Some(v) = config.discovery_lookback {
        guard.discovery_lookback = v;
    }
    if let Some(v) = config.discovery_min_len {
        guard.discovery_min_len = v as usize;
    }
    if let Some(v) = config.discovery_max_len {
        guard.discovery_max_len = v as usize;
    }
    if let Some(ref v) = config.discovery_excluded_apps {
        guard.discovery_excluded_apps = v.clone();
    }
    if let Some(ref v) = config.discovery_excluded_window_titles {
        guard.discovery_excluded_window_titles = v.clone();
    }
    if let Some(v) = config.ghost_suggestor_enabled {
        guard.ghost_suggestor_enabled = v;
    }
    if let Some(v) = config.ghost_suggestor_debounce_ms {
        guard.ghost_suggestor_debounce_ms = v as u64;
    }
    if let Some(v) = config.ghost_suggestor_display_secs {
        guard.ghost_suggestor_display_secs = v as u64;
    }
    if let Some(v) = config.ghost_suggestor_snooze_duration_mins {
        guard.ghost_suggestor_snooze_duration_mins = v.clamp(1, 120) as u64;
    }
    if let Some(v) = config.ghost_suggestor_offset_x {
        guard.ghost_suggestor_offset_x = v;
    }
    if let Some(v) = config.ghost_suggestor_offset_y {
        guard.ghost_suggestor_offset_y = v;
    }
    if let Some(v) = config.ghost_follower_enabled {
        guard.ghost_follower.config.enabled = v;
    }
    if let Some(v) = config.ghost_follower_edge_right {
        guard.ghost_follower.config.edge = if v { FollowerEdge::Right } else { FollowerEdge::Left };
    }
    if let Some(v) = config.ghost_follower_monitor_anchor {
        guard.ghost_follower.config.monitor_anchor = match v {
            1 => MonitorAnchor::Secondary,
            2 => MonitorAnchor::Current,
            _ => MonitorAnchor::Primary,
        };
    }
    if let Some(ref v) = config.ghost_follower_search {
        guard.ghost_follower.search_filter = v.clone();
    }
    if let Some(v) = config.ghost_follower_hover_preview {
        guard.ghost_follower.config.hover_preview = v;
    }
    if let Some(v) = config.ghost_follower_collapse_delay_secs {
        guard.ghost_follower.config.collapse_delay_secs = v as u64;
    }
    if let Some(v) = config.ghost_follower_opacity {
        guard.ghost_follower.config.opacity = v.clamp(10, 100);
    }
    if let Some(ref v) = config.ghost_follower_mode {
        if v == "Bubble" || v == "FloatingBubble" {
            guard.ghost_follower.config.mode = FollowerMode::FloatingBubble;
        } else {
            guard.ghost_follower.config.mode = FollowerMode::EdgeAnchored;
        }
    }
    if let Some(ref v) = config.ghost_follower_expand_trigger {
        if v == "Hover" {
            guard.ghost_follower.config.expand_trigger = ExpandTrigger::Hover;
        } else {
            guard.ghost_follower.config.expand_trigger = ExpandTrigger::Click;
        }
    }
    if let Some(v) = config.ghost_follower_expand_delay_ms {
        guard.ghost_follower.config.expand_delay_ms = v as u64;
    }
    if let Some(v) = config.ghost_follower_clipboard_depth {
        guard.ghost_follower.config.clipboard_depth = v as usize;
    }
    if let Some(v) = config.clip_history_max_depth {
        let depth = v as usize;
        guard.clip_history_max_depth = depth;
        clipboard_history::update_config(ClipboardHistoryConfig {
            enabled: true,
            max_depth: if depth == 0 { usize::MAX } else { depth },
        });
        let storage = JsonFileStorageAdapter::load();
        let mut copy_cfg = load_copy_to_clipboard_config(&storage, depth as u32);
        copy_cfg.max_history_entries = depth as u32;
        let _ = save_copy_to_clipboard_config(&copy_cfg);
        if depth > 0 {
            if let Ok(deleted_ids) = clipboard_repository::trim_to_depth(depth as u32) {
                for id in deleted_ids {
                    let _ = kms_repository::delete_embeddings_for_entity("clipboard", &id.to_string());
                }
            }
        }
    }
    if let Some(v) = config.script_library_run_disabled {
        guard.script_library_run_disabled = v;
    }
    if let Some(ref v) = config.script_library_run_allowlist {
        guard.script_library_run_allowlist = v.clone();
    }

    if let Some(v) = config.corpus_enabled { guard.corpus_enabled = v; }
    if let Some(ref v) = config.corpus_output_dir { guard.corpus_output_dir = v.clone(); }
    if let Some(ref v) = config.corpus_snapshot_dir { guard.corpus_snapshot_dir = v.clone(); }
    if let Some(v) = config.corpus_shortcut_modifiers { guard.corpus_shortcut_modifiers = v as u16; }
    if let Some(v) = config.corpus_shortcut_key { guard.corpus_shortcut_key = v as u16; }

    if let Some(v) = config.extraction_row_overlap_tolerance { guard.extraction_row_overlap_tolerance = v; }
    if let Some(v) = config.extraction_cluster_threshold_factor { guard.extraction_cluster_threshold_factor = v; }
    if let Some(v) = config.extraction_zone_proximity { guard.extraction_zone_proximity = v; }
    if let Some(v) = config.extraction_cross_zone_gap_factor { guard.extraction_cross_zone_gap_factor = v; }
    if let Some(v) = config.extraction_same_zone_gap_factor { guard.extraction_same_zone_gap_factor = v; }
    if let Some(v) = config.extraction_significant_gap_gate { guard.extraction_significant_gap_gate = v; }
    if let Some(v) = config.extraction_char_width_factor { guard.extraction_char_width_factor = v; }
    if let Some(v) = config.extraction_bridged_threshold { guard.extraction_bridged_threshold = v; }
    if let Some(v) = config.extraction_word_spacing_factor { guard.extraction_word_spacing_factor = v; }

    if let Some(ref v) = config.extraction_footer_triggers { guard.extraction_footer_triggers = v.clone(); }
    if let Some(v) = config.extraction_table_min_contiguous_rows { guard.extraction_table_min_contiguous_rows = v as usize; }
    if let Some(v) = config.extraction_table_min_avg_segments { guard.extraction_table_min_avg_segments = v; }

    if let Some(v) = config.extraction_adaptive_plaintext_cluster_factor { guard.extraction_adaptive_plaintext_cluster_factor = v; }
    if let Some(v) = config.extraction_adaptive_plaintext_gap_gate { guard.extraction_adaptive_plaintext_gap_gate = v; }
    if let Some(v) = config.extraction_adaptive_table_cluster_factor { guard.extraction_adaptive_table_cluster_factor = v; }
    if let Some(v) = config.extraction_adaptive_table_gap_gate { guard.extraction_adaptive_table_gap_gate = v; }
    if let Some(v) = config.extraction_adaptive_column_cluster_factor { guard.extraction_adaptive_column_cluster_factor = v; }
    if let Some(v) = config.extraction_adaptive_column_gap_gate { guard.extraction_adaptive_column_gap_gate = v; }
    if let Some(v) = config.extraction_adaptive_plaintext_cross_factor { guard.extraction_adaptive_plaintext_cross_factor = v; }
    if let Some(v) = config.extraction_adaptive_table_cross_factor { guard.extraction_adaptive_table_cross_factor = v; }
    if let Some(v) = config.extraction_adaptive_column_cross_factor { guard.extraction_adaptive_column_cross_factor = v; }

    if let Some(v) = config.extraction_refinement_entropy_threshold { guard.extraction_refinement_entropy_threshold = v; }
    if let Some(v) = config.extraction_refinement_cluster_threshold_modifier { guard.extraction_refinement_cluster_threshold_modifier = v; }
    if let Some(v) = config.extraction_refinement_cross_zone_gap_modifier { guard.extraction_refinement_cross_zone_gap_modifier = v; }

    if let Some(v) = config.extraction_classifier_gutter_weight { guard.extraction_classifier_gutter_weight = v; }
    if let Some(v) = config.extraction_classifier_density_weight { guard.extraction_classifier_density_weight = v; }
    if let Some(v) = config.extraction_classifier_multicolumn_density_max { guard.extraction_classifier_multicolumn_density_max = v; }
    if let Some(v) = config.extraction_classifier_table_density_min { guard.extraction_classifier_table_density_min = v; }
    if let Some(v) = config.extraction_classifier_table_entropy_min { guard.extraction_classifier_table_entropy_min = v; }

    if let Some(v) = config.extraction_columns_min_contiguous_rows { guard.extraction_columns_min_contiguous_rows = v as usize; }
    if let Some(v) = config.extraction_columns_gutter_gap_factor { guard.extraction_columns_gutter_gap_factor = v; }
    if let Some(v) = config.extraction_columns_gutter_void_tolerance { guard.extraction_columns_gutter_void_tolerance = v; }
    if let Some(v) = config.extraction_columns_edge_margin_tolerance { guard.extraction_columns_edge_margin_tolerance = v; }

    if let Some(v) = config.extraction_headers_max_width_ratio { guard.extraction_headers_max_width_ratio = v; }
    if let Some(v) = config.extraction_headers_centered_tolerance { guard.extraction_headers_centered_tolerance = v; }
    if let Some(v) = config.extraction_headers_h1_size_multiplier { guard.extraction_headers_h1_size_multiplier = v; }
    if let Some(v) = config.extraction_headers_h2_size_multiplier { guard.extraction_headers_h2_size_multiplier = v; }
    if let Some(v) = config.extraction_headers_h3_size_multiplier { guard.extraction_headers_h3_size_multiplier = v; }

    if let Some(v) = config.extraction_scoring_jitter_penalty_weight { guard.extraction_scoring_jitter_penalty_weight = v; }
    if let Some(v) = config.extraction_scoring_size_penalty_weight { guard.extraction_scoring_size_penalty_weight = v; }
    if let Some(v) = config.extraction_scoring_low_confidence_threshold { guard.extraction_scoring_low_confidence_threshold = v; }

    if let Some(v) = config.extraction_layout_row_lookback { guard.extraction_layout_row_lookback = v as usize; }
    if let Some(v) = config.extraction_layout_table_break_threshold { guard.extraction_layout_table_break_threshold = v; }
    if let Some(v) = config.extraction_layout_paragraph_break_threshold { guard.extraction_layout_paragraph_break_threshold = v; }
    if let Some(v) = config.extraction_layout_max_space_clamp { guard.extraction_layout_max_space_clamp = v as usize; }
    if let Some(v) = config.extraction_tables_column_jitter_tolerance { guard.extraction_tables_column_jitter_tolerance = v; }
    if let Some(v) = config.extraction_tables_merge_y_gap_max { guard.extraction_tables_merge_y_gap_max = v; }
    if let Some(v) = config.extraction_tables_merge_y_gap_min { guard.extraction_tables_merge_y_gap_min = v; }

    if let Some(v) = config.kms_graph_k_means_max_k {
        guard.kms_graph_k_means_max_k = v.max(2);
    }
    if let Some(v) = config.kms_graph_k_means_iterations {
        guard.kms_graph_k_means_iterations = v.max(1);
    }
    if let Some(v) = config.kms_graph_ai_beam_max_nodes {
        guard.kms_graph_ai_beam_max_nodes = v.max(2);
    }
    if let Some(v) = config.kms_graph_ai_beam_similarity_threshold {
        guard.kms_graph_ai_beam_similarity_threshold = v.clamp(0.0, 1.0);
    }
    if let Some(v) = config.kms_graph_ai_beam_max_edges {
        guard.kms_graph_ai_beam_max_edges = v;
    }
    if let Some(v) = config.kms_graph_enable_ai_beams {
        guard.kms_graph_enable_ai_beams = v;
    }
    if let Some(v) = config.kms_graph_enable_semantic_clustering {
        guard.kms_graph_enable_semantic_clustering = v;
    }
    if let Some(v) = config.kms_graph_enable_leiden_communities {
        guard.kms_graph_enable_leiden_communities = v;
    }
    if let Some(v) = config.kms_graph_semantic_max_notes {
        guard.kms_graph_semantic_max_notes = v;
    }
    if let Some(v) = config.kms_graph_warn_note_threshold {
        guard.kms_graph_warn_note_threshold = v;
    }
    if let Some(v) = config.kms_graph_beam_max_pair_checks {
        guard.kms_graph_beam_max_pair_checks = v;
    }
    if let Some(v) = config.kms_graph_enable_semantic_knn_edges {
        guard.kms_graph_enable_semantic_knn_edges = v;
    }
    if let Some(v) = config.kms_graph_semantic_knn_per_note {
        guard.kms_graph_semantic_knn_per_note = v.clamp(1, 30);
    }
    if let Some(v) = config.kms_graph_semantic_knn_min_similarity {
        guard.kms_graph_semantic_knn_min_similarity = v.clamp(0.5, 0.999);
    }
    if let Some(v) = config.kms_graph_semantic_knn_max_edges {
        guard.kms_graph_semantic_knn_max_edges = v.min(500_000);
    }
    if let Some(v) = config.kms_graph_semantic_knn_max_pair_checks {
        guard.kms_graph_semantic_knn_max_pair_checks = v;
    }
    if let Some(v) = config.kms_graph_auto_paging_enabled {
        guard.kms_graph_auto_paging_enabled = v;
    }
    if let Some(v) = config.kms_graph_auto_paging_note_threshold {
        guard.kms_graph_auto_paging_note_threshold = v;
    }
    if let Some(ref s) = config.kms_graph_vault_overrides_json {
        if serde_json::from_str::<serde_json::Value>(s).is_ok() {
            guard.kms_graph_vault_overrides_json = s.clone();
        }
    }
    if let Some(v) = config.kms_graph_bloom_enabled {
        guard.kms_graph_bloom_enabled = v;
    }
    if let Some(v) = config.kms_graph_bloom_strength {
        guard.kms_graph_bloom_strength = v.clamp(0.0, 2.5);
    }
    if let Some(v) = config.kms_graph_bloom_radius {
        guard.kms_graph_bloom_radius = v.clamp(0.0, 1.5);
    }
    if let Some(v) = config.kms_graph_bloom_threshold {
        guard.kms_graph_bloom_threshold = v.clamp(0.0, 1.0);
    }
    if let Some(v) = config.kms_graph_hex_cell_radius {
        guard.kms_graph_hex_cell_radius = v.clamp(0.5, 8.0);
    }
    if let Some(v) = config.kms_graph_hex_layer_opacity {
        guard.kms_graph_hex_layer_opacity = v.clamp(0.0, 1.0);
    }
    if let Some(v) = config.kms_graph_hex_stroke_width {
        guard.kms_graph_hex_stroke_width = v.clamp(0.02, 0.5);
    }
    if let Some(v) = config.kms_graph_hex_stroke_opacity {
        guard.kms_graph_hex_stroke_opacity = v.clamp(0.0, 1.0);
    }
    if let Some(v) = config.kms_graph_pagerank_iterations {
        guard.kms_graph_pagerank_iterations = v.max(4);
    }
    if let Some(v) = config.kms_graph_pagerank_local_iterations {
        guard.kms_graph_pagerank_local_iterations = v.max(4);
    }
    if let Some(v) = config.kms_graph_pagerank_damping {
        guard.kms_graph_pagerank_damping = v.clamp(0.5, 0.99);
    }
    if let Some(ref s) = config.kms_graph_pagerank_scope {
        let t = s.trim();
        if !t.is_empty() {
            guard.kms_graph_pagerank_scope = t.to_string();
        }
    }
    if let Some(v) = config.kms_graph_background_wiki_pagerank_enabled {
        guard.kms_graph_background_wiki_pagerank_enabled = v;
    }
    if let Some(v) = config.kms_graph_sprite_label_max_dpr_scale {
        guard.kms_graph_sprite_label_max_dpr_scale = v.clamp(1.0, 8.0);
    }
    if let Some(v) = config.kms_graph_sprite_label_min_res_scale {
        guard.kms_graph_sprite_label_min_res_scale = v.clamp(1.0, 4.0);
    }
    if let Some(v) = config.kms_graph_webworker_layout_threshold {
        guard.kms_graph_webworker_layout_threshold = v.min(500_000);
    }
    if let Some(v) = config.kms_graph_webworker_layout_max_ticks {
        guard.kms_graph_webworker_layout_max_ticks = v.clamp(20, 10_000);
    }
    if let Some(v) = config.kms_graph_webworker_layout_alpha_min {
        guard.kms_graph_webworker_layout_alpha_min = v.clamp(0.0005, 0.5);
    }
    if let Some(v) = config.kms_graph_temporal_window_enabled {
        guard.kms_graph_temporal_window_enabled = v;
    }
    if let Some(v) = config.kms_graph_temporal_default_days {
        guard.kms_graph_temporal_default_days = v;
    }
    if let Some(v) = config.kms_graph_temporal_include_notes_without_mtime {
        guard.kms_graph_temporal_include_notes_without_mtime = v;
    }
    if let Some(v) = config.kms_graph_temporal_edge_recency_enabled {
        guard.kms_graph_temporal_edge_recency_enabled = v;
    }
    if let Some(v) = config.kms_graph_temporal_edge_recency_strength {
        guard.kms_graph_temporal_edge_recency_strength = v.clamp(0.0, 1.0);
    }
    if let Some(v) = config.kms_graph_temporal_edge_recency_half_life_days {
        guard.kms_graph_temporal_edge_recency_half_life_days = v.max(0.1);
    }
    if let Some(v) = config.kms_search_min_similarity {
        guard.kms_search_min_similarity = v.clamp(0.0, 1.0);
    }
    if let Some(v) = config.kms_search_include_embedding_diagnostics {
        guard.kms_search_include_embedding_diagnostics = v;
    }
    if let Some(ref v) = config.kms_search_default_mode {
        let t = v.trim();
        if t == "Hybrid" || t == "Semantic" || t == "Keyword" {
            guard.kms_search_default_mode = t.to_string();
        }
    }
    if let Some(v) = config.kms_search_default_limit {
        guard.kms_search_default_limit = v.clamp(1, 200);
    }
    if let Some(ref v) = config.kms_embedding_model_id {
        guard.kms_embedding_model_id = v.trim().to_string();
    }
    if let Some(v) = config.kms_embedding_batch_notes_per_tick {
        guard.kms_embedding_batch_notes_per_tick = v.clamp(1, 500);
    }
    if let Some(v) = config.kms_embedding_chunk_enabled {
        guard.kms_embedding_chunk_enabled = v;
    }
    if let Some(v) = config.kms_embedding_chunk_max_chars {
        guard.kms_embedding_chunk_max_chars = v.clamp(256, 8192);
    }
    if let Some(v) = config.kms_embedding_chunk_overlap_chars {
        guard.kms_embedding_chunk_overlap_chars = v.min(4096);
    }
    {
        let max_o = guard.kms_embedding_chunk_max_chars / 2;
        guard.kms_embedding_chunk_overlap_chars = guard.kms_embedding_chunk_overlap_chars.min(max_o);
    }

    sync_discovery_config(
        guard.discovery_enabled,
        discovery::DiscoveryConfig {
            threshold: guard.discovery_threshold,
            lookback_minutes: guard.discovery_lookback,
            min_phrase_len: guard.discovery_min_len,
            max_phrase_len: guard.discovery_max_len,
            excluded_apps: guard
                .discovery_excluded_apps
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            excluded_window_titles: guard
                .discovery_excluded_window_titles
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        },
    );
    sync_ghost_config(GhostConfig {
        suggestor_enabled: guard.ghost_suggestor_enabled,
        suggestor_debounce_ms: guard.ghost_suggestor_debounce_ms,
        suggestor_display_secs: guard.ghost_suggestor_display_secs,
        suggestor_snooze_duration_mins: guard.ghost_suggestor_snooze_duration_mins,
        suggestor_offset_x: guard.ghost_suggestor_offset_x,
        suggestor_offset_y: guard.ghost_suggestor_offset_y,
        follower_enabled: guard.ghost_follower.config.enabled,
        follower_edge_right: guard.ghost_follower.config.edge == FollowerEdge::Right,
        follower_monitor_anchor: match guard.ghost_follower.config.monitor_anchor {
            MonitorAnchor::Secondary => 1,
            MonitorAnchor::Current => 2,
            _ => 0,
        },
        follower_search: guard.ghost_follower.search_filter.clone(),
        follower_hover_preview: guard.ghost_follower.config.hover_preview,
        follower_collapse_delay_secs: guard.ghost_follower.config.collapse_delay_secs,
    });
    set_expansion_paused(guard.expansion_paused);
    {
        use digicore_text_expander::adapters::corpus::{FileSystemCorpusStorageAdapter, OcrBaselineAdapter};
        use digicore_text_expander::application::corpus_generator::CorpusService;
        use digicore_core::domain::value_objects::CorpusConfig;
        let corpus_config = CorpusConfig {
            enabled: guard.corpus_enabled,
            output_dir: guard.corpus_output_dir.clone(),
            snapshot_dir: guard.corpus_snapshot_dir.clone(),
            shortcut_modifiers: guard.corpus_shortcut_modifiers,
            shortcut_key: guard.corpus_shortcut_key,
        };
        let corpus_storage = std::sync::Arc::new(FileSystemCorpusStorageAdapter::new(corpus_config.output_dir.clone()));
        let ocr_config = digicore_text_expander::adapters::extraction::RuntimeConfig::load_from_json_adapter(&JsonFileStorageAdapter::load());
        let corpus_baseline = std::sync::Arc::new(OcrBaselineAdapter::new(corpus_config.snapshot_dir.clone(), ocr_config));
        let corpus_service = std::sync::Arc::new(CorpusService::new(corpus_config, corpus_storage, corpus_baseline));
        digicore_text_expander::drivers::hotstring::update_corpus_service(Some(corpus_service));
    }
    let _ = get_app(&host.app_handle).emit("ghost-follower-update", ());

    let next_pagerank_settings = (
        guard.kms_graph_pagerank_iterations,
        guard.kms_graph_pagerank_damping,
        guard.kms_graph_pagerank_scope.clone(),
    );
    let vault_buf = PathBuf::from(guard.kms_vault_path.clone());
    let pagerank_settings_changed = prev_pagerank_settings != next_pagerank_settings;

    let next_embed_norm =
        embedding_service::normalized_embedding_model_id(&guard.kms_embedding_model_id);
    let embedding_model_changed = prev_embed_norm != next_embed_norm;
    let embed_chunk_cfg =
        crate::kms_graph_effective_params::effective_kms_embedding_chunk_config(&*guard, &vault_buf);
    let chunk_effective_changed =
        prev_chunk_effective.clamped() != embed_chunk_cfg.clamped();
    let batch_notes = guard.kms_embedding_batch_notes_per_tick.max(1);
    let target_model_for_migration = next_embed_norm.clone();

    persist_settings_to_storage(&guard)?;
    drop(guard);

    if pagerank_settings_changed && !vault_buf.as_os_str().is_empty() {
        crate::kms_sync_orchestration::schedule_debounced_background_wiki_pagerank_on_settings(
            &get_app(&host.app_handle),
            vault_buf.clone(),
        );
    }

    if (embedding_model_changed || chunk_effective_changed)
        && !vault_buf.as_os_str().is_empty()
        && vault_buf.exists()
    {
        let app_h = get_app(&host.app_handle);
        crate::kms_embedding_migrate::spawn_note_embedding_migration(
            &app_h,
            vault_buf,
            target_model_for_migration,
            batch_notes,
            embed_chunk_cfg,
            false,
        );
    }

    let _ = get_app(&host.app_handle).emit("digicore-app-state-changed", ());

    Ok(())
}
