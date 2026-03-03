export interface Snippet {
  trigger: string;
  content: string;
  options?: string;
  category?: string;
  profile?: string;
  app_lock?: string;
  pinned?: string;
  last_modified?: string;
}

export interface AppState {
  library_path: string;
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
  ghost_suggestor_offset_x: number;
  ghost_suggestor_offset_y: number;
  ghost_follower_enabled: boolean;
  ghost_follower_edge_right: boolean;
  ghost_follower_monitor_anchor: number;
  ghost_follower_search: string;
  ghost_follower_hover_preview: boolean;
  ghost_follower_collapse_delay_secs: number;
  clip_history_max_depth: number;
  script_library_run_disabled: boolean;
  script_library_run_allowlist: string;
}

export interface ClipEntry {
  content: string;
  process_name: string;
  window_title: string;
  length?: number;
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
