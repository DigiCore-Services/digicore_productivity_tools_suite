//! Persist `AppState` fields to JSON storage (settings file).

use std::sync::{Arc, Mutex};

use digicore_text_expander::adapters::storage::JsonFileStorageAdapter;
use digicore_text_expander::application::app_state::AppState;
use digicore_text_expander::ports::{storage_keys, StoragePort};

/// Persists app state to JSON storage. Used by save_settings and save_all_on_exit.
pub(crate) fn persist_settings_to_storage(state: &AppState) -> Result<(), String> {
    let mut storage = JsonFileStorageAdapter::load();
    storage.set(storage_keys::LIBRARY_PATH, &state.library_path);
    storage.set(storage_keys::SYNC_URL, &state.sync_url);
    storage.set(
        storage_keys::TEMPLATE_DATE_FORMAT,
        &state.template_date_format,
    );
    storage.set(
        storage_keys::TEMPLATE_TIME_FORMAT,
        &state.template_time_format,
    );
    storage.set(
        storage_keys::SCRIPT_LIBRARY_RUN_DISABLED,
        &state.script_library_run_disabled.to_string(),
    );
    storage.set(
        storage_keys::SCRIPT_LIBRARY_RUN_ALLOWLIST,
        &state.script_library_run_allowlist,
    );
    storage.set(
        storage_keys::GHOST_SUGGESTOR_DISPLAY_SECS,
        &state.ghost_suggestor_display_secs.to_string(),
    );
    storage.set(
        storage_keys::GHOST_SUGGESTOR_SNOOZE_DURATION_MINS,
        &state.ghost_suggestor_snooze_duration_mins.to_string(),
    );
    storage.set(
        storage_keys::CLIP_HISTORY_MAX_DEPTH,
        &state.clip_history_max_depth.to_string(),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_ENABLED,
        &state.ghost_follower.config.enabled.to_string(),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_MODE,
        &format!("{:?}", state.ghost_follower.config.mode),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_EDGE_RIGHT,
        &(state.ghost_follower.config.edge == digicore_text_expander::application::ghost_follower::FollowerEdge::Right).to_string(),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_MONITOR_ANCHOR,
        &match state.ghost_follower.config.monitor_anchor {
            digicore_text_expander::application::ghost_follower::MonitorAnchor::Primary => 0u32,
            digicore_text_expander::application::ghost_follower::MonitorAnchor::Secondary => 1u32,
            digicore_text_expander::application::ghost_follower::MonitorAnchor::Current => 2u32,
        }.to_string(),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_EXPAND_TRIGGER,
        &format!("{:?}", state.ghost_follower.config.expand_trigger),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_EXPAND_DELAY_MS,
        &state.ghost_follower.config.expand_delay_ms.to_string(),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_HOVER_PREVIEW,
        &state.ghost_follower.config.hover_preview.to_string(),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_COLLAPSE_DELAY_SECS,
        &state.ghost_follower.config.collapse_delay_secs.to_string(),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_CLIPBOARD_DEPTH,
        &state.ghost_follower.config.clipboard_depth.to_string(),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_OPACITY,
        &state.ghost_follower.config.opacity.to_string(),
    );
    if let Some((px, py)) = state.ghost_follower.config.position {
        storage.set(storage_keys::GHOST_FOLLOWER_POSITION_X, &px.to_string());
        storage.set(storage_keys::GHOST_FOLLOWER_POSITION_Y, &py.to_string());
    }
    storage.set(
        storage_keys::EXPANSION_PAUSED,
        &state.expansion_paused.to_string(),
    );
    storage.set(
        storage_keys::GHOST_SUGGESTOR_ENABLED,
        &state.ghost_suggestor_enabled.to_string(),
    );
    storage.set(
        storage_keys::GHOST_SUGGESTOR_DEBOUNCE_MS,
        &state.ghost_suggestor_debounce_ms.to_string(),
    );
    storage.set(
        storage_keys::GHOST_SUGGESTOR_OFFSET_X,
        &state.ghost_suggestor_offset_x.to_string(),
    );
    storage.set(
        storage_keys::GHOST_SUGGESTOR_OFFSET_Y,
        &state.ghost_suggestor_offset_y.to_string(),
    );
    storage.set(
        storage_keys::DISCOVERY_ENABLED,
        &state.discovery_enabled.to_string(),
    );
    storage.set(
        storage_keys::DISCOVERY_THRESHOLD,
        &state.discovery_threshold.to_string(),
    );
    storage.set(
        storage_keys::DISCOVERY_LOOKBACK,
        &state.discovery_lookback.to_string(),
    );
    storage.set(
        storage_keys::DISCOVERY_MIN_LEN,
        &state.discovery_min_len.to_string(),
    );
    storage.set(
        storage_keys::DISCOVERY_MAX_LEN,
        &state.discovery_max_len.to_string(),
    );
    storage.set(
        storage_keys::DISCOVERY_EXCLUDED_APPS,
        &state.discovery_excluded_apps,
    );
    storage.set(
        storage_keys::DISCOVERY_EXCLUDED_WINDOW_TITLES,
        &state.discovery_excluded_window_titles,
    );
    
    storage.set(storage_keys::CORPUS_ENABLED, &state.corpus_enabled.to_string());
    storage.set(storage_keys::CORPUS_OUTPUT_DIR, &state.corpus_output_dir);
    storage.set(storage_keys::CORPUS_SNAPSHOT_DIR, &state.corpus_snapshot_dir);
    storage.set(storage_keys::CORPUS_SHORTCUT_MODIFIERS, &state.corpus_shortcut_modifiers.to_string());
    storage.set(storage_keys::CORPUS_SHORTCUT_KEY, &state.corpus_shortcut_key.to_string());

    storage.set(storage_keys::EXTRACTION_ROW_OVERLAP_TOLERANCE, &state.extraction_row_overlap_tolerance.to_string());
    storage.set(storage_keys::EXTRACTION_CLUSTER_THRESHOLD_FACTOR, &state.extraction_cluster_threshold_factor.to_string());
    storage.set(storage_keys::EXTRACTION_ZONE_PROXIMITY, &state.extraction_zone_proximity.to_string());
    storage.set(storage_keys::EXTRACTION_CROSS_ZONE_GAP_FACTOR, &state.extraction_cross_zone_gap_factor.to_string());
    storage.set(storage_keys::EXTRACTION_SAME_ZONE_GAP_FACTOR, &state.extraction_same_zone_gap_factor.to_string());
    storage.set(storage_keys::EXTRACTION_SIGNIFICANT_GAP_GATE, &state.extraction_significant_gap_gate.to_string());
    storage.set(storage_keys::EXTRACTION_CHAR_WIDTH_FACTOR, &state.extraction_char_width_factor.to_string());
    storage.set(storage_keys::EXTRACTION_BRIDGED_THRESHOLD, &state.extraction_bridged_threshold.to_string());
    storage.set(storage_keys::EXTRACTION_WORD_SPACING_FACTOR, &state.extraction_word_spacing_factor.to_string());

    storage.set(storage_keys::EXTRACTION_FOOTER_TRIGGERS, &state.extraction_footer_triggers);
    storage.set(storage_keys::EXTRACTION_TABLE_MIN_CONTIGUOUS_ROWS, &state.extraction_table_min_contiguous_rows.to_string());
    storage.set(storage_keys::EXTRACTION_TABLE_MIN_AVG_SEGMENTS, &state.extraction_table_min_avg_segments.to_string());

    storage.set(storage_keys::EXTRACTION_ADAPTIVE_PLAINTEXT_CLUSTER_FACTOR, &state.extraction_adaptive_plaintext_cluster_factor.to_string());
    storage.set(storage_keys::EXTRACTION_ADAPTIVE_PLAINTEXT_GAP_GATE, &state.extraction_adaptive_plaintext_gap_gate.to_string());
    storage.set(storage_keys::EXTRACTION_ADAPTIVE_TABLE_CLUSTER_FACTOR, &state.extraction_adaptive_table_cluster_factor.to_string());
    storage.set(storage_keys::EXTRACTION_ADAPTIVE_TABLE_GAP_GATE, &state.extraction_adaptive_table_gap_gate.to_string());
    storage.set(storage_keys::EXTRACTION_ADAPTIVE_COLUMN_CLUSTER_FACTOR, &state.extraction_adaptive_column_cluster_factor.to_string());
    storage.set(storage_keys::EXTRACTION_ADAPTIVE_COLUMN_GAP_GATE, &state.extraction_adaptive_column_gap_gate.to_string());
    storage.set(storage_keys::EXTRACTION_ADAPTIVE_PLAINTEXT_CROSS_FACTOR, &state.extraction_adaptive_plaintext_cross_factor.to_string());
    storage.set(storage_keys::EXTRACTION_ADAPTIVE_TABLE_CROSS_FACTOR, &state.extraction_adaptive_table_cross_factor.to_string());
    storage.set(storage_keys::EXTRACTION_ADAPTIVE_COLUMN_CROSS_FACTOR, &state.extraction_adaptive_column_cross_factor.to_string());


    storage.set(storage_keys::EXTRACTION_REFINEMENT_ENTROPY_THRESHOLD, &state.extraction_refinement_entropy_threshold.to_string());
    storage.set(storage_keys::EXTRACTION_REFINEMENT_CLUSTER_THRESHOLD_MODIFIER, &state.extraction_refinement_cluster_threshold_modifier.to_string());
    storage.set(storage_keys::EXTRACTION_REFINEMENT_CROSS_ZONE_GAP_MODIFIER, &state.extraction_refinement_cross_zone_gap_modifier.to_string());

    storage.set(storage_keys::EXTRACTION_CLASSIFIER_GUTTER_WEIGHT, &state.extraction_classifier_gutter_weight.to_string());
    storage.set(storage_keys::EXTRACTION_CLASSIFIER_DENSITY_WEIGHT, &state.extraction_classifier_density_weight.to_string());
    storage.set(storage_keys::EXTRACTION_CLASSIFIER_MULTICOLUMN_DENSITY_MAX, &state.extraction_classifier_multicolumn_density_max.to_string());
    storage.set(storage_keys::EXTRACTION_CLASSIFIER_TABLE_DENSITY_MIN, &state.extraction_classifier_table_density_min.to_string());
    storage.set(storage_keys::EXTRACTION_CLASSIFIER_TABLE_ENTROPY_MIN, &state.extraction_classifier_table_entropy_min.to_string());

    storage.set(storage_keys::EXTRACTION_COLUMNS_MIN_CONTIGUOUS_ROWS, &state.extraction_columns_min_contiguous_rows.to_string());
    storage.set(storage_keys::EXTRACTION_COLUMNS_GUTTER_GAP_FACTOR, &state.extraction_columns_gutter_gap_factor.to_string());
    storage.set(storage_keys::EXTRACTION_COLUMNS_GUTTER_VOID_TOLERANCE, &state.extraction_columns_gutter_void_tolerance.to_string());
    storage.set(storage_keys::EXTRACTION_COLUMNS_EDGE_MARGIN_TOLERANCE, &state.extraction_columns_edge_margin_tolerance.to_string());

    storage.set(storage_keys::EXTRACTION_HEADERS_MAX_WIDTH_RATIO, &state.extraction_headers_max_width_ratio.to_string());
    storage.set(storage_keys::EXTRACTION_HEADERS_CENTERED_TOLERANCE, &state.extraction_headers_centered_tolerance.to_string());
    storage.set(storage_keys::EXTRACTION_HEADERS_H1_SIZE_MULTIPLIER, &state.extraction_headers_h1_size_multiplier.to_string());
    storage.set(storage_keys::EXTRACTION_HEADERS_H2_SIZE_MULTIPLIER, &state.extraction_headers_h2_size_multiplier.to_string());
    storage.set(storage_keys::EXTRACTION_HEADERS_H3_SIZE_MULTIPLIER, &state.extraction_headers_h3_size_multiplier.to_string());

    storage.set(storage_keys::EXTRACTION_SCORING_JITTER_PENALTY_WEIGHT, &state.extraction_scoring_jitter_penalty_weight.to_string());
    storage.set(storage_keys::EXTRACTION_SCORING_SIZE_PENALTY_WEIGHT, &state.extraction_scoring_size_penalty_weight.to_string());
    storage.set(storage_keys::EXTRACTION_SCORING_LOW_CONFIDENCE_THRESHOLD, &state.extraction_scoring_low_confidence_threshold.to_string());
    
    storage.set(storage_keys::EXTRACTION_LAYOUT_ROW_LOOKBACK, &state.extraction_layout_row_lookback.to_string());
    storage.set(storage_keys::EXTRACTION_LAYOUT_TABLE_BREAK_THRESHOLD, &state.extraction_layout_table_break_threshold.to_string());
    storage.set(storage_keys::EXTRACTION_LAYOUT_PARAGRAPH_BREAK_THRESHOLD, &state.extraction_layout_paragraph_break_threshold.to_string());
    storage.set(storage_keys::EXTRACTION_LAYOUT_MAX_SPACE_CLAMP, &state.extraction_layout_max_space_clamp.to_string());
    storage.set(storage_keys::EXTRACTION_TABLES_COLUMN_JITTER_TOLERANCE, &state.extraction_tables_column_jitter_tolerance.to_string());
    storage.set(storage_keys::EXTRACTION_TABLES_MERGE_Y_GAP_MAX, &state.extraction_tables_merge_y_gap_max.to_string());
    storage.set(storage_keys::EXTRACTION_TABLES_MERGE_Y_GAP_MIN, &state.extraction_tables_merge_y_gap_min.to_string());

    storage.set(storage_keys::KMS_GRAPH_K_MEANS_MAX_K, &state.kms_graph_k_means_max_k.to_string());
    storage.set(storage_keys::KMS_GRAPH_K_MEANS_ITERATIONS, &state.kms_graph_k_means_iterations.to_string());
    storage.set(storage_keys::KMS_GRAPH_AI_BEAM_MAX_NODES, &state.kms_graph_ai_beam_max_nodes.to_string());
    storage.set(
        storage_keys::KMS_GRAPH_AI_BEAM_SIMILARITY_THRESHOLD,
        &state.kms_graph_ai_beam_similarity_threshold.to_string(),
    );
    storage.set(storage_keys::KMS_GRAPH_AI_BEAM_MAX_EDGES, &state.kms_graph_ai_beam_max_edges.to_string());
    storage.set(storage_keys::KMS_GRAPH_ENABLE_AI_BEAMS, &state.kms_graph_enable_ai_beams.to_string());
    storage.set(
        storage_keys::KMS_GRAPH_ENABLE_SEMANTIC_CLUSTERING,
        &state.kms_graph_enable_semantic_clustering.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_ENABLE_LEIDEN_COMMUNITIES,
        &state.kms_graph_enable_leiden_communities.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_SEMANTIC_MAX_NOTES,
        &state.kms_graph_semantic_max_notes.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_WARN_NOTE_THRESHOLD,
        &state.kms_graph_warn_note_threshold.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_BEAM_MAX_PAIR_CHECKS,
        &state.kms_graph_beam_max_pair_checks.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_ENABLE_SEMANTIC_KNN_EDGES,
        &state.kms_graph_enable_semantic_knn_edges.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_SEMANTIC_KNN_PER_NOTE,
        &state.kms_graph_semantic_knn_per_note.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_SEMANTIC_KNN_MIN_SIMILARITY,
        &state.kms_graph_semantic_knn_min_similarity.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_SEMANTIC_KNN_MAX_EDGES,
        &state.kms_graph_semantic_knn_max_edges.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_SEMANTIC_KNN_MAX_PAIR_CHECKS,
        &state.kms_graph_semantic_knn_max_pair_checks.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_AUTO_PAGING_ENABLED,
        &state.kms_graph_auto_paging_enabled.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_AUTO_PAGING_NOTE_THRESHOLD,
        &state.kms_graph_auto_paging_note_threshold.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_VAULT_OVERRIDES_JSON,
        &state.kms_graph_vault_overrides_json,
    );
    storage.set(
        storage_keys::KMS_GRAPH_BLOOM_ENABLED,
        &state.kms_graph_bloom_enabled.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_BLOOM_STRENGTH,
        &state.kms_graph_bloom_strength.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_BLOOM_RADIUS,
        &state.kms_graph_bloom_radius.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_BLOOM_THRESHOLD,
        &state.kms_graph_bloom_threshold.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_HEX_CELL_RADIUS,
        &state.kms_graph_hex_cell_radius.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_HEX_LAYER_OPACITY,
        &state.kms_graph_hex_layer_opacity.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_HEX_STROKE_WIDTH,
        &state.kms_graph_hex_stroke_width.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_HEX_STROKE_OPACITY,
        &state.kms_graph_hex_stroke_opacity.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_PAGERANK_ITERATIONS,
        &state.kms_graph_pagerank_iterations.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_PAGERANK_LOCAL_ITERATIONS,
        &state.kms_graph_pagerank_local_iterations.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_PAGERANK_DAMPING,
        &state.kms_graph_pagerank_damping.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_PAGERANK_SCOPE,
        &state.kms_graph_pagerank_scope,
    );
    storage.set(
        storage_keys::KMS_GRAPH_BACKGROUND_WIKI_PAGERANK_ENABLED,
        &state.kms_graph_background_wiki_pagerank_enabled.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_TEMPORAL_WINDOW_ENABLED,
        &state.kms_graph_temporal_window_enabled.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_TEMPORAL_DEFAULT_DAYS,
        &state.kms_graph_temporal_default_days.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_TEMPORAL_INCLUDE_NOTES_WITHOUT_MTIME,
        &state.kms_graph_temporal_include_notes_without_mtime.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_TEMPORAL_EDGE_RECENCY_ENABLED,
        &state.kms_graph_temporal_edge_recency_enabled.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_TEMPORAL_EDGE_RECENCY_STRENGTH,
        &state.kms_graph_temporal_edge_recency_strength.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_TEMPORAL_EDGE_RECENCY_HALF_LIFE_DAYS,
        &state.kms_graph_temporal_edge_recency_half_life_days.to_string(),
    );
    storage.set(
        storage_keys::KMS_SEARCH_MIN_SIMILARITY,
        &state.kms_search_min_similarity.to_string(),
    );
    storage.set(
        storage_keys::KMS_SEARCH_INCLUDE_EMBEDDING_DIAGNOSTICS,
        &state.kms_search_include_embedding_diagnostics.to_string(),
    );
    storage.set(
        storage_keys::KMS_SEARCH_DEFAULT_MODE,
        &state.kms_search_default_mode,
    );
    storage.set(
        storage_keys::KMS_SEARCH_DEFAULT_LIMIT,
        &state.kms_search_default_limit.to_string(),
    );
    storage.set(storage_keys::KMS_EMBEDDING_MODEL_ID, &state.kms_embedding_model_id);
    storage.set(
        storage_keys::KMS_EMBEDDING_BATCH_NOTES_PER_TICK,
        &state.kms_embedding_batch_notes_per_tick.to_string(),
    );
    storage.set(
        storage_keys::KMS_EMBEDDING_CHUNK_ENABLED,
        &state.kms_embedding_chunk_enabled.to_string(),
    );
    storage.set(
        storage_keys::KMS_EMBEDDING_CHUNK_MAX_CHARS,
        &state.kms_embedding_chunk_max_chars.to_string(),
    );
    storage.set(
        storage_keys::KMS_EMBEDDING_CHUNK_OVERLAP_CHARS,
        &state.kms_embedding_chunk_overlap_chars.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_SPRITE_LABEL_MAX_DPR_SCALE,
        &state.kms_graph_sprite_label_max_dpr_scale.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_SPRITE_LABEL_MIN_RES_SCALE,
        &state.kms_graph_sprite_label_min_res_scale.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_WEBWORKER_LAYOUT_THRESHOLD,
        &state.kms_graph_webworker_layout_threshold.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_WEBWORKER_LAYOUT_MAX_TICKS,
        &state.kms_graph_webworker_layout_max_ticks.to_string(),
    );
    storage.set(
        storage_keys::KMS_GRAPH_WEBWORKER_LAYOUT_ALPHA_MIN,
        &state.kms_graph_webworker_layout_alpha_min.to_string(),
    );

    storage.persist().map_err(|e| e.to_string())
}

/// Persist only settings from current shared AppState.
pub fn persist_settings_for_state(state: &Arc<Mutex<AppState>>) -> Result<(), String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    persist_settings_to_storage(&*guard)
}

/// Saves settings and library to disk. Call on app exit to persist unsaved changes.
pub fn save_all_on_exit(state: &Arc<Mutex<AppState>>) {
    if let Ok(mut guard) = state.lock() {
        if let Err(e) = persist_settings_to_storage(&*guard) {
            log::warn!("[Exit] persist_settings failed: {}", e);
        }
        if let Err(e) = guard.try_save_library() {
            log::warn!("[Exit] try_save_library failed: {}", e);
        }
    }
}
