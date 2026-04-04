//! Helper module for settings bundle import/apply orchestration.

use crate::settings_bundle_model::{
    normalize_settings_group, normalized_selected_groups, settings_bundle_schema_supported,
    SETTINGS_BUNDLE_SCHEMA_V1, SETTINGS_BUNDLE_SCHEMA_V1_1, SETTINGS_GROUP_APPEARANCE,
    SETTINGS_GROUP_CLIPBOARD_HISTORY, SETTINGS_GROUP_COPY_TO_CLIPBOARD, SETTINGS_GROUP_CORE,
    SETTINGS_GROUP_DISCOVERY, SETTINGS_GROUP_GHOST_FOLLOWER, SETTINGS_GROUP_GHOST_SUGGESTOR,
    SETTINGS_GROUP_KMS_GRAPH, SETTINGS_GROUP_SCRIPT_RUNTIME, SETTINGS_GROUP_SYNC,
    SETTINGS_GROUP_TEMPLATES,
};

use crate::appearance_enforcement::{
    enforce_appearance_transparency_rules, save_appearance_rules, sort_appearance_rules_deterministic,
};
use crate::clipboard_text_persistence::{load_copy_to_clipboard_config, save_copy_to_clipboard_config};

use super::*;

pub(crate) async fn import_settings_bundle_from_file(
    host: ApiImpl,
    path: String,
    selected_groups: Vec<String>,
) -> Result<SettingsImportResultDto, String> {
    let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let root: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let schema = root
        .get("schema_version")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if !settings_bundle_schema_supported(schema) {
        let msg = format!(
            "Unsupported schema_version '{schema}'. Expected '{}' or '{}'.",
            SETTINGS_BUNDLE_SCHEMA_V1, SETTINGS_BUNDLE_SCHEMA_V1_1
        );
        diag_log("error", format!("[SettingsImport] {msg}"));
        return Err(msg);
    }
    let groups_obj = root
        .get("groups")
        .and_then(|v| v.as_object())
        .ok_or_else(|| "Invalid settings bundle: missing groups object.".to_string())?;
    let mut result = SettingsImportResultDto {
        applied_groups: Vec::new(),
        skipped_groups: Vec::new(),
        warnings: Vec::new(),
        updated_keys: 0,
        appearance_rules_applied: 0,
        theme: None,
        autostart_enabled: None,
    };
    let selected = if selected_groups.is_empty() {
        groups_obj
            .keys()
            .filter_map(|k| normalize_settings_group(k).map(str::to_string))
            .collect::<Vec<String>>()
    } else {
        normalized_selected_groups(&selected_groups)
    };

    for group in selected {
        let Some(value) = groups_obj.get(&group) else {
            result.skipped_groups.push(group.clone());
            result
                .warnings
                .push(format!("Group '{group}' not present in bundle."));
            continue;
        };
        let obj = match value.as_object() {
            Some(v) => v,
            None => {
                result.skipped_groups.push(group.clone());
                result
                    .warnings
                    .push(format!("Group '{group}' has invalid payload type."));
                continue;
            }
        };

        match group.as_str() {
            SETTINGS_GROUP_TEMPLATES => {
                super::config_ipc_service::update_config(
                    host.clone(),
                    ConfigUpdateDto {
                        expansion_paused: None,
                        template_date_format: obj
                            .get("template_date_format")
                            .and_then(|v| v.as_str())
                            .map(str::to_string),
                        template_time_format: obj
                            .get("template_time_format")
                            .and_then(|v| v.as_str())
                            .map(str::to_string),
                        sync_url: None,
                        discovery_enabled: None,
                        discovery_threshold: None,
                        discovery_lookback: None,
                        discovery_min_len: None,
                        discovery_max_len: None,
                        discovery_excluded_apps: None,
                        discovery_excluded_window_titles: None,
                        ghost_suggestor_enabled: None,
                        ghost_suggestor_debounce_ms: None,
                        ghost_suggestor_display_secs: None,
                        ghost_suggestor_snooze_duration_mins: None,
                        ghost_suggestor_offset_x: None,
                        ghost_suggestor_offset_y: None,
                        ghost_follower_enabled: None,
                        ghost_follower_edge_right: None,
                        ghost_follower_monitor_anchor: None,
                        ghost_follower_search: None,
                        ghost_follower_hover_preview: None,
                        ghost_follower_collapse_delay_secs: None,
                        ghost_follower_opacity: None,
                        clip_history_max_depth: None,
                        script_library_run_allowlist: None,
                        ..Default::default()
                    },
                )
                .await?;
                result.updated_keys = result.updated_keys.saturating_add(2);
            }
            SETTINGS_GROUP_SYNC => {
                super::config_ipc_service::update_config(
                    host.clone(),
                    ConfigUpdateDto {
                        expansion_paused: None,
                        template_date_format: None,
                        template_time_format: None,
                        sync_url: obj
                            .get("sync_url")
                            .and_then(|v| v.as_str())
                            .map(str::to_string),
                        discovery_enabled: None,
                        discovery_threshold: None,
                        discovery_lookback: None,
                        discovery_min_len: None,
                        discovery_max_len: None,
                        discovery_excluded_apps: None,
                        discovery_excluded_window_titles: None,
                        ghost_suggestor_enabled: None,
                        ghost_suggestor_debounce_ms: None,
                        ghost_suggestor_display_secs: None,
                        ghost_suggestor_snooze_duration_mins: None,
                        ghost_suggestor_offset_x: None,
                        ghost_suggestor_offset_y: None,
                        ghost_follower_enabled: None,
                        ghost_follower_edge_right: None,
                        ghost_follower_monitor_anchor: None,
                        ghost_follower_search: None,
                        ghost_follower_hover_preview: None,
                        ghost_follower_collapse_delay_secs: None,
                        ghost_follower_opacity: None,
                        clip_history_max_depth: None,
                        script_library_run_allowlist: None,
                        ..Default::default()
                    },
                )
                .await?;
                result.updated_keys = result.updated_keys.saturating_add(1);
            }
            SETTINGS_GROUP_KMS_GRAPH => {
                let vault_json = obj.get("kms_graph_vault_overrides").map(|v| v.to_string());
                super::config_ipc_service::update_config(
                    host.clone(),
                    ConfigUpdateDto {
                        kms_graph_k_means_max_k: obj
                            .get("kms_graph_k_means_max_k")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32),
                        kms_graph_k_means_iterations: obj
                            .get("kms_graph_k_means_iterations")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32),
                        kms_graph_ai_beam_max_nodes: obj
                            .get("kms_graph_ai_beam_max_nodes")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32),
                        kms_graph_ai_beam_similarity_threshold: obj
                            .get("kms_graph_ai_beam_similarity_threshold")
                            .and_then(|v| v.as_f64())
                            .map(|n| n as f32),
                        kms_graph_ai_beam_max_edges: obj
                            .get("kms_graph_ai_beam_max_edges")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32),
                        kms_graph_enable_ai_beams: obj
                            .get("kms_graph_enable_ai_beams")
                            .and_then(|v| v.as_bool()),
                        kms_graph_enable_semantic_clustering: obj
                            .get("kms_graph_enable_semantic_clustering")
                            .and_then(|v| v.as_bool()),
                        kms_graph_enable_leiden_communities: obj
                            .get("kms_graph_enable_leiden_communities")
                            .and_then(|v| v.as_bool()),
                        kms_graph_semantic_max_notes: obj
                            .get("kms_graph_semantic_max_notes")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32),
                        kms_graph_warn_note_threshold: obj
                            .get("kms_graph_warn_note_threshold")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32),
                        kms_graph_beam_max_pair_checks: obj
                            .get("kms_graph_beam_max_pair_checks")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32),
                        kms_graph_enable_semantic_knn_edges: obj
                            .get("kms_graph_enable_semantic_knn_edges")
                            .and_then(|v| v.as_bool()),
                        kms_graph_semantic_knn_per_note: obj
                            .get("kms_graph_semantic_knn_per_note")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32),
                        kms_graph_semantic_knn_min_similarity: obj
                            .get("kms_graph_semantic_knn_min_similarity")
                            .and_then(|v| v.as_f64())
                            .map(|n| n as f32),
                        kms_graph_semantic_knn_max_edges: obj
                            .get("kms_graph_semantic_knn_max_edges")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32),
                        kms_graph_semantic_knn_max_pair_checks: obj
                            .get("kms_graph_semantic_knn_max_pair_checks")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32),
                        kms_graph_auto_paging_enabled: obj
                            .get("kms_graph_auto_paging_enabled")
                            .and_then(|v| v.as_bool()),
                        kms_graph_auto_paging_note_threshold: obj
                            .get("kms_graph_auto_paging_note_threshold")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32),
                        kms_graph_vault_overrides_json: vault_json,
                        kms_graph_bloom_enabled: obj
                            .get("kms_graph_bloom_enabled")
                            .and_then(|v| v.as_bool()),
                        kms_graph_bloom_strength: obj
                            .get("kms_graph_bloom_strength")
                            .and_then(|v| v.as_f64())
                            .map(|n| n as f32),
                        kms_graph_bloom_radius: obj
                            .get("kms_graph_bloom_radius")
                            .and_then(|v| v.as_f64())
                            .map(|n| n as f32),
                        kms_graph_bloom_threshold: obj
                            .get("kms_graph_bloom_threshold")
                            .and_then(|v| v.as_f64())
                            .map(|n| n as f32),
                        kms_graph_hex_cell_radius: obj
                            .get("kms_graph_hex_cell_radius")
                            .and_then(|v| v.as_f64())
                            .map(|n| n as f32),
                        kms_graph_hex_layer_opacity: obj
                            .get("kms_graph_hex_layer_opacity")
                            .and_then(|v| v.as_f64())
                            .map(|n| n as f32),
                        kms_graph_hex_stroke_width: obj
                            .get("kms_graph_hex_stroke_width")
                            .and_then(|v| v.as_f64())
                            .map(|n| n as f32),
                        kms_graph_hex_stroke_opacity: obj
                            .get("kms_graph_hex_stroke_opacity")
                            .and_then(|v| v.as_f64())
                            .map(|n| n as f32),
                        kms_graph_pagerank_iterations: obj
                            .get("kms_graph_pagerank_iterations")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32),
                        kms_graph_pagerank_local_iterations: obj
                            .get("kms_graph_pagerank_local_iterations")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32),
                        kms_graph_pagerank_damping: obj
                            .get("kms_graph_pagerank_damping")
                            .and_then(|v| v.as_f64())
                            .map(|n| n as f32),
                        kms_graph_pagerank_scope: obj
                            .get("kms_graph_pagerank_scope")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        kms_graph_background_wiki_pagerank_enabled: obj
                            .get("kms_graph_background_wiki_pagerank_enabled")
                            .and_then(|v| v.as_bool()),
                        kms_graph_temporal_window_enabled: obj
                            .get("kms_graph_temporal_window_enabled")
                            .and_then(|v| v.as_bool()),
                        kms_graph_temporal_default_days: obj
                            .get("kms_graph_temporal_default_days")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32),
                        kms_graph_temporal_include_notes_without_mtime: obj
                            .get("kms_graph_temporal_include_notes_without_mtime")
                            .and_then(|v| v.as_bool()),
                        kms_graph_temporal_edge_recency_enabled: obj
                            .get("kms_graph_temporal_edge_recency_enabled")
                            .and_then(|v| v.as_bool()),
                        kms_graph_temporal_edge_recency_strength: obj
                            .get("kms_graph_temporal_edge_recency_strength")
                            .and_then(|v| v.as_f64())
                            .map(|n| n as f32),
                        kms_graph_temporal_edge_recency_half_life_days: obj
                            .get("kms_graph_temporal_edge_recency_half_life_days")
                            .and_then(|v| v.as_f64())
                            .map(|n| n as f32),
                        kms_search_min_similarity: obj
                            .get("kms_search_min_similarity")
                            .and_then(|v| v.as_f64())
                            .map(|n| n as f32),
                        kms_search_include_embedding_diagnostics: obj
                            .get("kms_search_include_embedding_diagnostics")
                            .and_then(|v| v.as_bool()),
                        kms_search_default_mode: obj
                            .get("kms_search_default_mode")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        kms_search_default_limit: obj
                            .get("kms_search_default_limit")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32),
                        kms_embedding_model_id: obj
                            .get("kms_embedding_model_id")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        kms_embedding_batch_notes_per_tick: obj
                            .get("kms_embedding_batch_notes_per_tick")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32),
                        kms_graph_sprite_label_max_dpr_scale: obj
                            .get("kms_graph_sprite_label_max_dpr_scale")
                            .and_then(|v| v.as_f64())
                            .map(|n| n as f32),
                        kms_graph_sprite_label_min_res_scale: obj
                            .get("kms_graph_sprite_label_min_res_scale")
                            .and_then(|v| v.as_f64())
                            .map(|n| n as f32),
                        kms_graph_webworker_layout_threshold: obj
                            .get("kms_graph_webworker_layout_threshold")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32),
                        kms_graph_webworker_layout_max_ticks: obj
                            .get("kms_graph_webworker_layout_max_ticks")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32),
                        kms_graph_webworker_layout_alpha_min: obj
                            .get("kms_graph_webworker_layout_alpha_min")
                            .and_then(|v| v.as_f64())
                            .map(|n| n as f32),
                        ..Default::default()
                    },
                )
                .await?;
                result.updated_keys = result.updated_keys.saturating_add(obj.len() as u32);
            }
            SETTINGS_GROUP_DISCOVERY
            | SETTINGS_GROUP_GHOST_SUGGESTOR
            | SETTINGS_GROUP_GHOST_FOLLOWER
            | SETTINGS_GROUP_CLIPBOARD_HISTORY
            | SETTINGS_GROUP_COPY_TO_CLIPBOARD
            | SETTINGS_GROUP_CORE
            | SETTINGS_GROUP_SCRIPT_RUNTIME => {
                let cfg = ConfigUpdateDto {
                    expansion_paused: obj.get("expansion_paused").and_then(|v| v.as_bool()),
                    template_date_format: None,
                    template_time_format: None,
                    sync_url: None,
                    discovery_enabled: obj.get("discovery_enabled").and_then(|v| v.as_bool()),
                    discovery_threshold: obj
                        .get("discovery_threshold")
                        .and_then(|v| v.as_u64())
                        .map(|n| n as u32),
                    discovery_lookback: obj
                        .get("discovery_lookback")
                        .and_then(|v| v.as_u64())
                        .map(|n| n as u32),
                    discovery_min_len: obj
                        .get("discovery_min_len")
                        .and_then(|v| v.as_u64())
                        .map(|n| n as u32),
                    discovery_max_len: obj
                        .get("discovery_max_len")
                        .and_then(|v| v.as_u64())
                        .map(|n| n as u32),
                    discovery_excluded_apps: obj
                        .get("discovery_excluded_apps")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    discovery_excluded_window_titles: obj
                        .get("discovery_excluded_window_titles")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    ghost_suggestor_enabled: obj
                        .get("ghost_suggestor_enabled")
                        .and_then(|v| v.as_bool()),
                    ghost_suggestor_debounce_ms: obj
                        .get("ghost_suggestor_debounce_ms")
                        .and_then(|v| v.as_u64())
                        .map(|n| n as u32),
                    ghost_suggestor_display_secs: obj
                        .get("ghost_suggestor_display_secs")
                        .and_then(|v| v.as_u64())
                        .map(|n| n as u32),
                    ghost_suggestor_snooze_duration_mins: obj
                        .get("ghost_suggestor_snooze_duration_mins")
                        .and_then(|v| v.as_u64())
                        .map(|n| n as u32),
                    ghost_suggestor_offset_x: obj
                        .get("ghost_suggestor_offset_x")
                        .and_then(|v| v.as_i64())
                        .map(|n| n as i32),
                    ghost_suggestor_offset_y: obj
                        .get("ghost_suggestor_offset_y")
                        .and_then(|v| v.as_i64())
                        .map(|n| n as i32),
                    ghost_follower_enabled: obj
                        .get("ghost_follower_enabled")
                        .and_then(|v| v.as_bool()),
                    ghost_follower_edge_right: obj
                        .get("ghost_follower_edge_right")
                        .and_then(|v| v.as_bool()),
                    ghost_follower_monitor_anchor: obj
                        .get("ghost_follower_monitor_anchor")
                        .and_then(|v| v.as_u64())
                        .map(|n| n as u32),
                    ghost_follower_search: None,
                    ghost_follower_hover_preview: obj
                        .get("ghost_follower_hover_preview")
                        .and_then(|v| v.as_bool()),
                    ghost_follower_collapse_delay_secs: obj
                        .get("ghost_follower_collapse_delay_secs")
                        .and_then(|v| v.as_u64())
                        .map(|n| n as u32),
                    ghost_follower_opacity: obj
                        .get("ghost_follower_opacity")
                        .and_then(|v| v.as_u64())
                        .map(|n| n as u32),
                    ghost_follower_mode: obj
                        .get("ghost_follower_mode")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    ghost_follower_expand_trigger: obj
                        .get("ghost_follower_expand_trigger")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    ghost_follower_expand_delay_ms: obj
                        .get("ghost_follower_expand_delay_ms")
                        .and_then(|v| v.as_u64())
                        .map(|n| n as u32),
                    ghost_follower_clipboard_depth: obj
                        .get("ghost_follower_clipboard_depth")
                        .and_then(|v| v.as_u64())
                        .map(|n| n as u32),
                    clip_history_max_depth: obj
                        .get("clip_history_max_depth")
                        .and_then(|v| v.as_u64())
                        .map(|n| n as u32),
                    script_library_run_disabled: obj
                        .get("script_library_run_disabled")
                        .and_then(|v| v.as_bool()),
                    script_library_run_allowlist: obj
                        .get("script_library_run_allowlist")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    ..Default::default()
                };
                super::config_ipc_service::update_config(host.clone(), cfg).await?;
                if group == SETTINGS_GROUP_CLIPBOARD_HISTORY {
                    let mut copy_cfg = {
                        let storage = JsonFileStorageAdapter::load();
                        load_copy_to_clipboard_config(
                            &storage,
                            obj.get("clip_history_max_depth")
                                .and_then(|v| v.as_u64())
                                .map(|n| n as u32)
                                .unwrap_or(20),
                        )
                    };
                    if let Some(v) = obj.get("copy_to_clipboard_enabled").and_then(|v| v.as_bool()) {
                        copy_cfg.enabled = v;
                    }
                    if let Some(v) = obj
                        .get("copy_to_clipboard_min_log_length")
                        .and_then(|v| v.as_u64())
                        .map(|n| n as u32)
                    {
                        copy_cfg.min_log_length = v;
                    }
                    if let Some(v) = obj.get("copy_to_clipboard_mask_cc").and_then(|v| v.as_bool()) {
                        copy_cfg.mask_cc = v;
                    }
                    if let Some(v) = obj.get("copy_to_clipboard_mask_ssn").and_then(|v| v.as_bool()) {
                        copy_cfg.mask_ssn = v;
                    }
                    if let Some(v) = obj.get("copy_to_clipboard_mask_email").and_then(|v| v.as_bool()) {
                        copy_cfg.mask_email = v;
                    }
                    if let Some(v) = obj
                        .get("copy_to_clipboard_blacklist_processes")
                        .and_then(|v| v.as_str())
                    {
                        copy_cfg.blacklist_processes = v.to_string();
                    }
                    if let Some(v) = obj
                        .get("copy_to_clipboard_json_output_enabled")
                        .and_then(|v| v.as_bool())
                    {
                        copy_cfg.json_output_enabled = v;
                    }
                    if let Some(v) = obj
                        .get("copy_to_clipboard_json_output_dir")
                        .and_then(|v| v.as_str())
                    {
                        copy_cfg.json_output_dir = v.to_string();
                    }
                    if let Some(v) = obj
                        .get("copy_to_clipboard_image_storage_dir")
                        .and_then(|v| v.as_str())
                    {
                        copy_cfg.image_storage_dir = v.to_string();
                    }
                    save_copy_to_clipboard_config(&copy_cfg)?;
                }
                if group == SETTINGS_GROUP_COPY_TO_CLIPBOARD {
                    let current_depth = {
                        let guard = host.state.lock().map_err(|e| e.to_string())?;
                        guard.clip_history_max_depth as u32
                    };
                    let mut copy_cfg = {
                        let storage = JsonFileStorageAdapter::load();
                        load_copy_to_clipboard_config(&storage, current_depth)
                    };
                    if let Some(v) = obj.get("copy_to_clipboard_enabled").and_then(|v| v.as_bool()) {
                        copy_cfg.enabled = v;
                    }
                    if let Some(v) = obj
                        .get("copy_to_clipboard_min_log_length")
                        .and_then(|v| v.as_u64())
                        .map(|n| n as u32)
                    {
                        copy_cfg.min_log_length = v;
                    }
                    if let Some(v) = obj.get("copy_to_clipboard_mask_cc").and_then(|v| v.as_bool()) {
                        copy_cfg.mask_cc = v;
                    }
                    if let Some(v) = obj.get("copy_to_clipboard_mask_ssn").and_then(|v| v.as_bool()) {
                        copy_cfg.mask_ssn = v;
                    }
                    if let Some(v) = obj.get("copy_to_clipboard_mask_email").and_then(|v| v.as_bool()) {
                        copy_cfg.mask_email = v;
                    }
                    if let Some(v) = obj
                        .get("copy_to_clipboard_blacklist_processes")
                        .and_then(|v| v.as_str())
                    {
                        copy_cfg.blacklist_processes = v.to_string();
                    }
                    if let Some(v) = obj
                        .get("copy_to_clipboard_json_output_enabled")
                        .and_then(|v| v.as_bool())
                    {
                        copy_cfg.json_output_enabled = v;
                    }
                    if let Some(v) = obj
                        .get("copy_to_clipboard_json_output_dir")
                        .and_then(|v| v.as_str())
                    {
                        copy_cfg.json_output_dir = v.to_string();
                    }
                    if let Some(v) = obj
                        .get("copy_to_clipboard_image_storage_dir")
                        .and_then(|v| v.as_str())
                    {
                        copy_cfg.image_storage_dir = v.to_string();
                    }
                    if let Some(v) = obj
                        .get("copy_to_clipboard_max_history_entries")
                        .or_else(|| obj.get("clip_history_max_depth"))
                        .and_then(|v| v.as_u64())
                        .map(|n| n as u32)
                    {
                        copy_cfg.max_history_entries = v;
                    }
                    save_copy_to_clipboard_config(&copy_cfg)?;
                    super::config_ipc_service::update_config(
                        host.clone(),
                        ConfigUpdateDto {
                            expansion_paused: None,
                            template_date_format: None,
                            template_time_format: None,
                            sync_url: None,
                            discovery_enabled: None,
                            discovery_threshold: None,
                            discovery_lookback: None,
                            discovery_min_len: None,
                            discovery_max_len: None,
                            discovery_excluded_apps: None,
                            discovery_excluded_window_titles: None,
                            ghost_suggestor_enabled: None,
                            ghost_suggestor_debounce_ms: None,
                            ghost_suggestor_display_secs: None,
                            ghost_suggestor_snooze_duration_mins: None,
                            ghost_suggestor_offset_x: None,
                            ghost_suggestor_offset_y: None,
                            ghost_follower_enabled: None,
                            ghost_follower_edge_right: None,
                            ghost_follower_monitor_anchor: None,
                            ghost_follower_search: None,
                            ghost_follower_hover_preview: None,
                            ghost_follower_collapse_delay_secs: None,
                            ghost_follower_opacity: None,
                            clip_history_max_depth: Some(copy_cfg.max_history_entries),
                            script_library_run_allowlist: None,
                            ..Default::default()
                        },
                    )
                    .await?;
                }
                if group == SETTINGS_GROUP_CORE {
                    result.theme = obj.get("theme").and_then(|v| v.as_str()).map(str::to_string);
                    result.autostart_enabled =
                        obj.get("autostart_enabled").and_then(|v| v.as_bool());
                }
                result.updated_keys = result.updated_keys.saturating_add(obj.len() as u32);
            }
            SETTINGS_GROUP_APPEARANCE => {
                let rules_value = obj.get("rules").and_then(|v| v.as_array());
                let Some(rules_arr) = rules_value else {
                    result.skipped_groups.push(group.clone());
                    result
                        .warnings
                        .push("Appearance group missing 'rules' array.".to_string());
                    continue;
                };
                let mut rules = Vec::<AppearanceTransparencyRuleDto>::new();
                for r in rules_arr {
                    let Some(ro) = r.as_object() else {
                        continue;
                    };
                    let mut app = ro
                        .get("app_process")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .trim()
                        .to_ascii_lowercase();
                    if app.is_empty() {
                        continue;
                    }
                    if !app.ends_with(".exe") {
                        app.push_str(".exe");
                    }
                    let opacity = ro
                        .get("opacity")
                        .and_then(|v| v.as_u64())
                        .map(|n| n as u32)
                        .unwrap_or(255)
                        .clamp(20, 255);
                    let enabled = ro.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
                    rules.push(AppearanceTransparencyRuleDto {
                        app_process: app,
                        opacity,
                        enabled,
                    });
                }
                sort_appearance_rules_deterministic(&mut rules);
                save_appearance_rules(&rules)?;
                enforce_appearance_transparency_rules();
                result.appearance_rules_applied = rules.len() as u32;
                result.updated_keys = result.updated_keys.saturating_add(rules.len() as u32);
            }
            _ => {
                result.skipped_groups.push(group.clone());
                result.warnings.push(format!("Unsupported group '{group}'."));
                continue;
            }
        }

        result.applied_groups.push(group.clone());
        diag_log("info", format!("[SettingsImport] Applied group '{group}'"));
    }

    super::config_ipc_service::save_settings(host.clone()).await?;
    diag_log(
        "info",
        format!(
            "[SettingsImport] Completed: applied={} skipped={} warnings={}",
            result.applied_groups.len(),
            result.skipped_groups.len(),
            result.warnings.len()
        ),
    );
    Ok(result)
}

