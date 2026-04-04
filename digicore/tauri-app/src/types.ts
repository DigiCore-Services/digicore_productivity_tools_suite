export interface Snippet {
  trigger: string;
  trigger_type: 'suffix' | 'regex';
  content: string;
  htmlContent: string | null;
  rtfContent: string | null;
  options: string;
  category: string;
  profile: string;
  appLock: string;
  pinned: string;
  case_adaptive: boolean;
  case_sensitive: boolean;
  smart_suffix: boolean;
  is_sensitive: boolean;
  lastModified: string;
}

export interface AppState {
  library_path: string;
  kms_vault_path: string;
  library: Record<string, Snippet[]>;
  categories: string[];
  selected_category?: number;
  status: string;
  sync_url: string;
  sync_status: string;
  expansion_log_path: string;
  expansion_paused: boolean;
  template_date_format: string;
  template_time_format: string;
  discovery_enabled: boolean;
  discovery_threshold: number;
  discovery_lookback: number;
  discovery_min_len: number;
  discovery_max_len: number;
  discovery_excluded_apps: string;
  discovery_excluded_window_titles: string;
  ghost_suggestor_enabled: boolean;
  ghost_suggestor_debounce_ms: number;
  ghost_suggestor_display_secs: number;
  ghost_suggestor_snooze_duration_mins: number;
  ghost_suggestor_offset_x: number;
  ghost_suggestor_offset_y: number;
  ghost_follower_enabled: boolean;
  ghost_follower_edge_right: boolean;
  ghost_follower_monitor_anchor: number;
  ghost_follower_search: string;
  ghost_follower_hover_preview: boolean;
  ghost_follower_collapse_delay_secs: number;
  ghost_follower_mode: string;
  ghost_follower_expand_trigger: string;
  ghost_follower_expand_delay_ms: number;
  ghost_follower_clipboard_depth: number;
  ghost_follower_opacity: number;
  clip_history_max_depth: number;
  script_library_run_disabled: boolean;
  script_library_run_allowlist: string;

  corpus_enabled: boolean;
  corpus_output_dir: string;
  corpus_snapshot_dir: string;
  corpus_shortcut_modifiers: number;
  corpus_shortcut_key: number;

  extraction_row_overlap_tolerance: number;
  extraction_cluster_threshold_factor: number;
  extraction_zone_proximity: number;
  extraction_cross_zone_gap_factor: number;
  extraction_same_zone_gap_factor: number;
  extraction_significant_gap_gate: number;
  extraction_char_width_factor: number;
  extraction_bridged_threshold: number;
  extraction_word_spacing_factor: number;

  extraction_footer_triggers: string;
  extraction_table_min_contiguous_rows: number;
  extraction_table_min_avg_segments: number;
  extraction_layout_row_lookback: number;
  extraction_layout_table_break_threshold: number;
  extraction_layout_paragraph_break_threshold: number;
  extraction_layout_max_space_clamp: number;
  extraction_tables_column_jitter_tolerance: number;
  extraction_tables_merge_y_gap_max: number;
  extraction_tables_merge_y_gap_min: number;

  extraction_adaptive_plaintext_cluster_factor: number;
  extraction_adaptive_plaintext_gap_gate: number;
  extraction_adaptive_table_cluster_factor: number;
  extraction_adaptive_table_gap_gate: number;
  extraction_adaptive_column_cluster_factor: number;
  extraction_adaptive_column_gap_gate: number;
  extraction_adaptive_plaintext_cross_factor: number;
  extraction_adaptive_table_cross_factor: number;
  extraction_adaptive_column_cross_factor: number;

  extraction_refinement_entropy_threshold: number;
  extraction_refinement_cluster_threshold_modifier: number;
  extraction_refinement_cross_zone_gap_modifier: number;

  extraction_classifier_gutter_weight: number;
  extraction_classifier_density_weight: number;
  extraction_classifier_multicolumn_density_max: number;
  extraction_classifier_table_density_min: number;
  extraction_classifier_table_entropy_min: number;

  extraction_columns_min_contiguous_rows: number;
  extraction_columns_gutter_gap_factor: number;
  extraction_columns_gutter_void_tolerance: number;
  extraction_columns_edge_margin_tolerance: number;

  extraction_headers_max_width_ratio: number;
  extraction_headers_centered_tolerance: number;
  extraction_headers_h1_size_multiplier: number;
  extraction_headers_h2_size_multiplier: number;
  extraction_headers_h3_size_multiplier: number;

  extraction_scoring_jitter_penalty_weight: number;
  extraction_scoring_size_penalty_weight: number;
  extraction_scoring_low_confidence_threshold: number;

  kms_graph_k_means_max_k: number;
  kms_graph_k_means_iterations: number;
  kms_graph_ai_beam_max_nodes: number;
  kms_graph_ai_beam_similarity_threshold: number;
  kms_graph_ai_beam_max_edges: number;
  kms_graph_enable_ai_beams: boolean;
  kms_graph_enable_semantic_clustering: boolean;
  kms_graph_enable_leiden_communities: boolean;
  kms_graph_semantic_max_notes: number;
  kms_graph_warn_note_threshold: number;
  kms_graph_beam_max_pair_checks: number;
  kms_graph_enable_semantic_knn_edges: boolean;
  kms_graph_semantic_knn_per_note: number;
  kms_graph_semantic_knn_min_similarity: number;
  kms_graph_semantic_knn_max_edges: number;
  kms_graph_semantic_knn_max_pair_checks: number;
  kms_graph_auto_paging_enabled: boolean;
  kms_graph_auto_paging_note_threshold: number;
  kms_graph_vault_overrides_json: string;

  kms_graph_bloom_enabled: boolean;
  kms_graph_bloom_strength: number;
  kms_graph_bloom_radius: number;
  kms_graph_bloom_threshold: number;
  kms_graph_hex_cell_radius: number;
  kms_graph_hex_layer_opacity: number;
  kms_graph_hex_stroke_width: number;
  kms_graph_hex_stroke_opacity: number;

  kms_graph_pagerank_iterations: number;
  kms_graph_pagerank_local_iterations: number;
  kms_graph_pagerank_damping: number;
  kms_graph_pagerank_scope: string;
  kms_graph_background_wiki_pagerank_enabled: boolean;

  kms_graph_temporal_window_enabled: boolean;
  kms_graph_temporal_default_days: number;
  kms_graph_temporal_include_notes_without_mtime: boolean;
  kms_graph_temporal_edge_recency_enabled: boolean;
  kms_graph_temporal_edge_recency_strength: number;
  kms_graph_temporal_edge_recency_half_life_days: number;
  kms_search_min_similarity: number;
  kms_search_include_embedding_diagnostics: boolean;
  kms_search_default_mode: string;
  kms_search_default_limit: number;

  kms_embedding_model_id: string;
  kms_embedding_batch_notes_per_tick: number;
  kms_embedding_chunk_enabled: boolean;
  kms_embedding_chunk_max_chars: number;
  kms_embedding_chunk_overlap_chars: number;

  kms_graph_sprite_label_max_dpr_scale: number;
  kms_graph_sprite_label_min_res_scale: number;
  kms_graph_webworker_layout_threshold: number;
  kms_graph_webworker_layout_max_ticks: number;
  kms_graph_webworker_layout_alpha_min: number;
}

export interface ClipEntry {
  id: number;
  content: string;
  html_content?: string | null;
  rtf_content?: string | null;
  process_name: string;
  window_title: string;
  length: number;
  word_count: number;
  created_at: string;
  entry_type: string;
  mime_type?: string | null;
  image_path?: string | null;
  thumb_path?: string | null;
  image_width?: number | null;
  image_height?: number | null;
  image_bytes?: number | null;
  parent_id?: number | null;
  metadata?: string | null;
}

export interface InteractiveVarDto {
  tag: string;
  label: string;
  var_type: string;
  options: string[];
}

export interface PendingVariableInput {
  content: string;
  vars: InteractiveVarDto[];
  values: Record<string, string>;
  choice_indices: Record<string, number>;
  checkbox_checked: Record<string, boolean>;
}
