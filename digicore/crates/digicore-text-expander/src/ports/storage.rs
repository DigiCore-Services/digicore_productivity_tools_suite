//! StoragePort - framework-agnostic key-value persistence for user preferences.
//!
//! Part of Phase 0/1 UI decoupling. Enables swapping GUI frameworks (egui, Tauri,
//! Iced, Tauri) without changing persistence logic.
//!
//! Implementations:
//! - EframeStorageAdapter: wraps eframe::Storage (egui)
//! - JsonFileStorageAdapter: JSON file in config dir (Tauri, Iced, etc.)

/// Storage keys for Text Expander user preferences.
/// Used by adapters to read/write from backing store.
pub mod keys {
    pub const LIBRARY_PATH: &str = "library_path";
    pub const SYNC_URL: &str = "sync_url";
    pub const TEMPLATE_DATE_FORMAT: &str = "template_date_format";
    pub const TEMPLATE_TIME_FORMAT: &str = "template_time_format";
    pub const SCRIPT_LIBRARY_RUN_DISABLED: &str = "script_library_run_disabled";
    pub const SCRIPT_LIBRARY_RUN_ALLOWLIST: &str = "script_library_run_allowlist";
    pub const GHOST_SUGGESTOR_DISPLAY_SECS: &str = "ghost_suggestor_display_secs";
    pub const GHOST_SUGGESTOR_SNOOZE_DURATION_MINS: &str = "ghost_suggestor_snooze_duration_mins";
    pub const GHOST_SUGGESTOR_ENABLED: &str = "ghost_suggestor_enabled";
    pub const GHOST_SUGGESTOR_DEBOUNCE_MS: &str = "ghost_suggestor_debounce_ms";
    pub const GHOST_SUGGESTOR_OFFSET_X: &str = "ghost_suggestor_offset_x";
    pub const GHOST_SUGGESTOR_OFFSET_Y: &str = "ghost_suggestor_offset_y";
    pub const DISCOVERY_ENABLED: &str = "discovery_enabled";
    pub const DISCOVERY_THRESHOLD: &str = "discovery_threshold";
    pub const DISCOVERY_LOOKBACK: &str = "discovery_lookback";
    pub const DISCOVERY_MIN_LEN: &str = "discovery_min_len";
    pub const DISCOVERY_MAX_LEN: &str = "discovery_max_len";
    pub const DISCOVERY_EXCLUDED_APPS: &str = "discovery_excluded_apps";
    pub const DISCOVERY_EXCLUDED_WINDOW_TITLES: &str = "discovery_excluded_window_titles";
    pub const GHOST_FOLLOWER_ENABLED: &str = "ghost_follower_enabled";
    pub const GHOST_FOLLOWER_EDGE_RIGHT: &str = "ghost_follower_edge_right";
    pub const GHOST_FOLLOWER_MONITOR_ANCHOR: &str = "ghost_follower_monitor_anchor";
    pub const GHOST_FOLLOWER_HOVER_PREVIEW: &str = "ghost_follower_hover_preview";
    pub const GHOST_FOLLOWER_COLLAPSE_DELAY_SECS: &str = "ghost_follower_collapse_delay_secs";
    pub const GHOST_FOLLOWER_OPACITY: &str = "ghost_follower_opacity";
    pub const GHOST_FOLLOWER_POSITION_X: &str = "ghost_follower_position_x";
    pub const GHOST_FOLLOWER_POSITION_Y: &str = "ghost_follower_position_y";
    pub const CLIP_HISTORY_MAX_DEPTH: &str = "clip_history_max_depth";
    pub const COPY_TO_CLIPBOARD_ENABLED: &str = "copy_to_clipboard_enabled";
    pub const COPY_TO_CLIPBOARD_IMAGE_ENABLED: &str = "copy_to_clipboard_image_enabled";
    pub const COPY_TO_CLIPBOARD_MIN_LOG_LENGTH: &str = "copy_to_clipboard_min_log_length";
    pub const COPY_TO_CLIPBOARD_MASK_CC: &str = "copy_to_clipboard_mask_cc";
    pub const COPY_TO_CLIPBOARD_MASK_SSN: &str = "copy_to_clipboard_mask_ssn";
    pub const COPY_TO_CLIPBOARD_MASK_EMAIL: &str = "copy_to_clipboard_mask_email";
    pub const COPY_TO_CLIPBOARD_BLACKLIST_PROCESSES: &str = "copy_to_clipboard_blacklist_processes";
    pub const COPY_TO_CLIPBOARD_JSON_OUTPUT_ENABLED: &str = "copy_to_clipboard_json_output_enabled";
    pub const COPY_TO_CLIPBOARD_JSON_OUTPUT_DIR: &str = "copy_to_clipboard_json_output_dir";
    pub const COPY_TO_CLIPBOARD_IMAGE_STORAGE_DIR: &str = "copy_to_clipboard_image_storage_dir";
    pub const COPY_TO_CLIPBOARD_OCR_ENABLED: &str = "copy_to_clipboard_ocr_enabled";
    pub const EXPANSION_PAUSED: &str = "expansion_paused";
    /// Tauri UI: last active tab index (0-3).
    pub const UI_LAST_TAB: &str = "ui_last_tab";
    /// Tauri UI: column order for Library table (comma-separated: Profile,Category,Trigger,Content Preview,AppLock,Options,Last Modified).
    pub const UI_COLUMN_ORDER: &str = "ui_column_order";
    /// Tauri UI: Appearance tab transparency rules as JSON array.
    pub const APPEARANCE_TRANSPARENCY_RULES_JSON: &str = "appearance_transparency_rules_json";

    // Corpus Generation Keys
    pub const CORPUS_ENABLED: &str = "corpus_enabled";
    pub const CORPUS_OUTPUT_DIR: &str = "corpus_output_dir";
    pub const CORPUS_SNAPSHOT_DIR: &str = "corpus_snapshot_dir";
    pub const CORPUS_SHORTCUT_MODIFIERS: &str = "corpus_shortcut_modifiers";
    pub const CORPUS_SHORTCUT_KEY: &str = "corpus_shortcut_key";

    // Extraction Config Keys
    pub const EXTRACTION_ROW_OVERLAP_TOLERANCE: &str = "extraction_row_overlap_tolerance";
    pub const EXTRACTION_CLUSTER_THRESHOLD_FACTOR: &str = "extraction_cluster_threshold_factor";
    pub const EXTRACTION_ZONE_PROXIMITY: &str = "extraction_zone_proximity";
    pub const EXTRACTION_CROSS_ZONE_GAP_FACTOR: &str = "extraction_cross_zone_gap_factor";
    pub const EXTRACTION_SAME_ZONE_GAP_FACTOR: &str = "extraction_same_zone_gap_factor";
    pub const EXTRACTION_SIGNIFICANT_GAP_GATE: &str = "extraction_significant_gap_gate";
    pub const EXTRACTION_CHAR_WIDTH_FACTOR: &str = "extraction_char_width_factor";
    pub const EXTRACTION_BRIDGED_THRESHOLD: &str = "extraction_bridged_threshold";
    pub const EXTRACTION_WORD_SPACING_FACTOR: &str = "extraction_word_spacing_factor";

    // Extraction Tables Keys
    pub const EXTRACTION_FOOTER_TRIGGERS: &str = "extraction_footer_triggers";
    pub const EXTRACTION_TABLE_MIN_CONTIGUOUS_ROWS: &str = "extraction_table_min_contiguous_rows";
    pub const EXTRACTION_TABLE_MIN_AVG_SEGMENTS: &str = "extraction_table_min_avg_segments";

    // Extraction Adaptive Overrides
    pub const EXTRACTION_ADAPTIVE_PLAINTEXT_CLUSTER_FACTOR: &str = "extraction_adaptive_plaintext_cluster_factor";
    pub const EXTRACTION_ADAPTIVE_PLAINTEXT_GAP_GATE: &str = "extraction_adaptive_plaintext_gap_gate";
    pub const EXTRACTION_ADAPTIVE_TABLE_CLUSTER_FACTOR: &str = "extraction_adaptive_table_cluster_factor";
    pub const EXTRACTION_ADAPTIVE_TABLE_GAP_GATE: &str = "extraction_adaptive_table_gap_gate";
    pub const EXTRACTION_ADAPTIVE_COLUMN_CLUSTER_FACTOR: &str = "extraction_adaptive_column_cluster_factor";
    pub const EXTRACTION_ADAPTIVE_COLUMN_GAP_GATE: &str = "extraction_adaptive_column_gap_gate";
    pub const EXTRACTION_ADAPTIVE_PLAINTEXT_CROSS_FACTOR: &str = "extraction_adaptive_plaintext_cross_factor";
    pub const EXTRACTION_ADAPTIVE_TABLE_CROSS_FACTOR: &str = "extraction_adaptive_table_cross_factor";
    pub const EXTRACTION_ADAPTIVE_COLUMN_CROSS_FACTOR: &str = "extraction_adaptive_column_cross_factor";

    // Extraction Refinement
    pub const EXTRACTION_REFINEMENT_ENTROPY_THRESHOLD: &str = "extraction_refinement_entropy_threshold";
    pub const EXTRACTION_REFINEMENT_CLUSTER_THRESHOLD_MODIFIER: &str = "extraction_refinement_cluster_threshold_modifier";
    pub const EXTRACTION_REFINEMENT_CROSS_ZONE_GAP_MODIFIER: &str = "extraction_refinement_cross_zone_gap_modifier";

    // Extraction Classifier
    pub const EXTRACTION_CLASSIFIER_GUTTER_WEIGHT: &str = "extraction_classifier_gutter_weight";
    pub const EXTRACTION_CLASSIFIER_DENSITY_WEIGHT: &str = "extraction_classifier_density_weight";
    pub const EXTRACTION_CLASSIFIER_MULTICOLUMN_DENSITY_MAX: &str = "extraction_classifier_multicolumn_density_max";
    pub const EXTRACTION_CLASSIFIER_TABLE_DENSITY_MIN: &str = "extraction_classifier_table_density_min";
    pub const EXTRACTION_CLASSIFIER_TABLE_ENTROPY_MIN: &str = "extraction_classifier_table_entropy_min";

    // Extraction Columns
    pub const EXTRACTION_COLUMNS_MIN_CONTIGUOUS_ROWS: &str = "extraction_columns_min_contiguous_rows";
    pub const EXTRACTION_COLUMNS_GUTTER_GAP_FACTOR: &str = "extraction_columns_gutter_gap_factor";
    pub const EXTRACTION_COLUMNS_GUTTER_VOID_TOLERANCE: &str = "extraction_columns_gutter_void_tolerance";
    pub const EXTRACTION_COLUMNS_EDGE_MARGIN_TOLERANCE: &str = "extraction_columns_edge_margin_tolerance";

    // Extraction Headers
    pub const EXTRACTION_HEADERS_MAX_WIDTH_RATIO: &str = "extraction_headers_max_width_ratio";
    pub const EXTRACTION_HEADERS_CENTERED_TOLERANCE: &str = "extraction_headers_centered_tolerance";
    pub const EXTRACTION_HEADERS_H1_SIZE_MULTIPLIER: &str = "extraction_headers_h1_size_multiplier";
    pub const EXTRACTION_HEADERS_H2_SIZE_MULTIPLIER: &str = "extraction_headers_h2_size_multiplier";
    pub const EXTRACTION_HEADERS_H3_SIZE_MULTIPLIER: &str = "extraction_headers_h3_size_multiplier";

    // Extraction Scoring
    pub const EXTRACTION_SCORING_JITTER_PENALTY_WEIGHT: &str = "extraction_scoring_jitter_penalty_weight";
    pub const EXTRACTION_SCORING_SIZE_PENALTY_WEIGHT: &str = "extraction_scoring_size_penalty_weight";
    pub const EXTRACTION_SCORING_LOW_CONFIDENCE_THRESHOLD: &str = "extraction_scoring_low_confidence_threshold";
}

/// Port for key-value persistence (user preferences, window state).
///
/// Framework-agnostic: egui uses eframe::Storage; Tauri/Iced use JSON file.
/// Adapters load from backing store at init and persist on save.
pub trait StoragePort: Send + Sync {
    /// Get a value by key. Returns None if key is absent.
    fn get(&self, key: &str) -> Option<String>;

    /// Set a value for key. Persisted when adapter's persist method is called.
    fn set(&mut self, key: &str, value: &str);
}
