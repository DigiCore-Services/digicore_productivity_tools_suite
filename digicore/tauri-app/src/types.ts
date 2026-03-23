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
