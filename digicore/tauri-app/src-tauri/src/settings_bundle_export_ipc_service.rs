//! Helper module for settings bundle export orchestration.

use crate::settings_bundle_model::{
    normalized_selected_groups, SETTINGS_BUNDLE_SCHEMA_V1_1, SETTINGS_GROUP_APPEARANCE,
    SETTINGS_GROUP_CLIPBOARD_HISTORY, SETTINGS_GROUP_COPY_TO_CLIPBOARD, SETTINGS_GROUP_CORE,
    SETTINGS_GROUP_DISCOVERY, SETTINGS_GROUP_GHOST_FOLLOWER, SETTINGS_GROUP_GHOST_SUGGESTOR,
    SETTINGS_GROUP_KMS_GRAPH, SETTINGS_GROUP_SCRIPT_RUNTIME, SETTINGS_GROUP_SYNC,
    SETTINGS_GROUP_TEMPLATES,
};

use crate::appearance_enforcement::{load_appearance_rules, sort_appearance_rules_deterministic};
use crate::clipboard_text_persistence::load_copy_to_clipboard_config;

use super::*;

pub(crate) async fn export_settings_bundle_to_file(
    host: ApiImpl,
    path: String,
    selected_groups: Vec<String>,
    theme: Option<String>,
    autostart_enabled: Option<bool>,
) -> Result<u32, String> {
    let groups = normalized_selected_groups(&selected_groups);
    if groups.is_empty() {
        return Err("No valid settings groups selected for export.".to_string());
    }
    let guard = host.state.lock().map_err(|e| e.to_string())?;
    let mut groups_obj = serde_json::Map::new();

    for group in &groups {
        match group.as_str() {
            SETTINGS_GROUP_TEMPLATES => {
                groups_obj.insert(
                    group.clone(),
                    serde_json::json!({
                        "template_date_format": guard.template_date_format,
                        "template_time_format": guard.template_time_format
                    }),
                );
            }
            SETTINGS_GROUP_SYNC => {
                groups_obj.insert(
                    group.clone(),
                    serde_json::json!({
                        "sync_url": guard.sync_url
                    }),
                );
            }
            SETTINGS_GROUP_DISCOVERY => {
                groups_obj.insert(
                    group.clone(),
                    serde_json::json!({
                        "discovery_enabled": guard.discovery_enabled,
                        "discovery_threshold": guard.discovery_threshold,
                        "discovery_lookback": guard.discovery_lookback,
                        "discovery_min_len": guard.discovery_min_len,
                        "discovery_max_len": guard.discovery_max_len,
                        "discovery_excluded_apps": guard.discovery_excluded_apps,
                        "discovery_excluded_window_titles": guard.discovery_excluded_window_titles
                    }),
                );
            }
            SETTINGS_GROUP_GHOST_SUGGESTOR => {
                groups_obj.insert(
                    group.clone(),
                    serde_json::json!({
                        "ghost_suggestor_enabled": guard.ghost_suggestor_enabled,
                        "ghost_suggestor_debounce_ms": guard.ghost_suggestor_debounce_ms,
                        "ghost_suggestor_display_secs": guard.ghost_suggestor_display_secs,
                        "ghost_suggestor_snooze_duration_mins": guard.ghost_suggestor_snooze_duration_mins,
                        "ghost_suggestor_offset_x": guard.ghost_suggestor_offset_x,
                        "ghost_suggestor_offset_y": guard.ghost_suggestor_offset_y
                    }),
                );
            }
            SETTINGS_GROUP_GHOST_FOLLOWER => {
                groups_obj.insert(
                    group.clone(),
                    serde_json::json!({
                        "ghost_follower_enabled": guard.ghost_follower.config.enabled,
                        "ghost_follower_edge_right": guard.ghost_follower.config.edge == FollowerEdge::Right,
                        "ghost_follower_monitor_anchor": match guard.ghost_follower.config.monitor_anchor {
                            MonitorAnchor::Secondary => 1,
                            MonitorAnchor::Current => 2,
                            _ => 0,
                        },
                        "ghost_follower_hover_preview": guard.ghost_follower.config.hover_preview,
                        "ghost_follower_collapse_delay_secs": guard.ghost_follower.config.collapse_delay_secs,
                        "ghost_follower_opacity": guard.ghost_follower.config.opacity,
                        "ghost_follower_mode": format!("{:?}", guard.ghost_follower.config.mode),
                        "ghost_follower_expand_trigger": format!("{:?}", guard.ghost_follower.config.expand_trigger),
                        "ghost_follower_expand_delay_ms": guard.ghost_follower.config.expand_delay_ms,
                        "ghost_follower_clipboard_depth": guard.ghost_follower.config.clipboard_depth
                    }),
                );
            }
            SETTINGS_GROUP_CLIPBOARD_HISTORY => {
                groups_obj.insert(
                    group.clone(),
                    serde_json::json!({
                        "clip_history_max_depth": guard.clip_history_max_depth
                    }),
                );
            }
            SETTINGS_GROUP_COPY_TO_CLIPBOARD => {
                let storage = JsonFileStorageAdapter::load();
                let copy_cfg =
                    load_copy_to_clipboard_config(&storage, guard.clip_history_max_depth as u32);
                groups_obj.insert(
                    group.clone(),
                    serde_json::json!({
                        "copy_to_clipboard_enabled": copy_cfg.enabled,
                        "copy_to_clipboard_min_log_length": copy_cfg.min_log_length,
                        "copy_to_clipboard_mask_cc": copy_cfg.mask_cc,
                        "copy_to_clipboard_mask_ssn": copy_cfg.mask_ssn,
                        "copy_to_clipboard_mask_email": copy_cfg.mask_email,
                        "copy_to_clipboard_blacklist_processes": copy_cfg.blacklist_processes,
                        "copy_to_clipboard_json_output_enabled": copy_cfg.json_output_enabled,
                        "copy_to_clipboard_json_output_dir": copy_cfg.json_output_dir,
                        "copy_to_clipboard_image_storage_dir": copy_cfg.image_storage_dir,
                        "copy_to_clipboard_max_history_entries": copy_cfg.max_history_entries
                    }),
                );
            }
            SETTINGS_GROUP_CORE => {
                groups_obj.insert(
                    group.clone(),
                    serde_json::json!({
                        "expansion_paused": guard.expansion_paused,
                        "theme": theme,
                        "autostart_enabled": autostart_enabled
                    }),
                );
            }
            SETTINGS_GROUP_SCRIPT_RUNTIME => {
                groups_obj.insert(
                    group.clone(),
                    serde_json::json!({
                        "script_library_run_disabled": guard.script_library_run_disabled,
                        "script_library_run_allowlist": guard.script_library_run_allowlist
                    }),
                );
            }
            SETTINGS_GROUP_APPEARANCE => {
                let storage = JsonFileStorageAdapter::load();
                let mut rules = load_appearance_rules(&storage);
                sort_appearance_rules_deterministic(&mut rules);
                groups_obj.insert(group.clone(), serde_json::json!({ "rules": rules }));
            }
            SETTINGS_GROUP_KMS_GRAPH => {
                let vault_ov: serde_json::Value =
                    serde_json::from_str(&guard.kms_graph_vault_overrides_json)
                        .unwrap_or_else(|_| serde_json::json!({}));
                groups_obj.insert(
                    group.clone(),
                    serde_json::json!({
                        "kms_graph_k_means_max_k": guard.kms_graph_k_means_max_k,
                        "kms_graph_k_means_iterations": guard.kms_graph_k_means_iterations,
                        "kms_graph_ai_beam_max_nodes": guard.kms_graph_ai_beam_max_nodes,
                        "kms_graph_ai_beam_similarity_threshold": guard.kms_graph_ai_beam_similarity_threshold,
                        "kms_graph_ai_beam_max_edges": guard.kms_graph_ai_beam_max_edges,
                        "kms_graph_enable_ai_beams": guard.kms_graph_enable_ai_beams,
                        "kms_graph_enable_semantic_clustering": guard.kms_graph_enable_semantic_clustering,
                        "kms_graph_enable_leiden_communities": guard.kms_graph_enable_leiden_communities,
                        "kms_graph_semantic_max_notes": guard.kms_graph_semantic_max_notes,
                        "kms_graph_warn_note_threshold": guard.kms_graph_warn_note_threshold,
                        "kms_graph_beam_max_pair_checks": guard.kms_graph_beam_max_pair_checks,
                        "kms_graph_enable_semantic_knn_edges": guard.kms_graph_enable_semantic_knn_edges,
                        "kms_graph_semantic_knn_per_note": guard.kms_graph_semantic_knn_per_note,
                        "kms_graph_semantic_knn_min_similarity": guard.kms_graph_semantic_knn_min_similarity,
                        "kms_graph_semantic_knn_max_edges": guard.kms_graph_semantic_knn_max_edges,
                        "kms_graph_semantic_knn_max_pair_checks": guard.kms_graph_semantic_knn_max_pair_checks,
                        "kms_graph_auto_paging_enabled": guard.kms_graph_auto_paging_enabled,
                        "kms_graph_auto_paging_note_threshold": guard.kms_graph_auto_paging_note_threshold,
                        "kms_graph_vault_overrides": vault_ov,
                        "kms_graph_pagerank_iterations": guard.kms_graph_pagerank_iterations,
                        "kms_graph_pagerank_local_iterations": guard.kms_graph_pagerank_local_iterations,
                        "kms_graph_pagerank_damping": guard.kms_graph_pagerank_damping,
                        "kms_graph_pagerank_scope": guard.kms_graph_pagerank_scope,
                        "kms_graph_background_wiki_pagerank_enabled": guard.kms_graph_background_wiki_pagerank_enabled,
                        "kms_graph_temporal_window_enabled": guard.kms_graph_temporal_window_enabled,
                        "kms_graph_temporal_default_days": guard.kms_graph_temporal_default_days,
                        "kms_graph_temporal_include_notes_without_mtime": guard.kms_graph_temporal_include_notes_without_mtime,
                        "kms_graph_temporal_edge_recency_enabled": guard.kms_graph_temporal_edge_recency_enabled,
                        "kms_graph_temporal_edge_recency_strength": guard.kms_graph_temporal_edge_recency_strength,
                        "kms_graph_temporal_edge_recency_half_life_days": guard.kms_graph_temporal_edge_recency_half_life_days,
                        "kms_search_min_similarity": guard.kms_search_min_similarity,
                        "kms_search_include_embedding_diagnostics": guard.kms_search_include_embedding_diagnostics,
                        "kms_search_default_mode": guard.kms_search_default_mode,
                        "kms_search_default_limit": guard.kms_search_default_limit,
                        "kms_embedding_model_id": guard.kms_embedding_model_id,
                        "kms_embedding_batch_notes_per_tick": guard.kms_embedding_batch_notes_per_tick,
                        "kms_embedding_chunk_enabled": guard.kms_embedding_chunk_enabled,
                        "kms_embedding_chunk_max_chars": guard.kms_embedding_chunk_max_chars,
                        "kms_embedding_chunk_overlap_chars": guard.kms_embedding_chunk_overlap_chars,
                        "kms_graph_sprite_label_max_dpr_scale": guard.kms_graph_sprite_label_max_dpr_scale,
                        "kms_graph_sprite_label_min_res_scale": guard.kms_graph_sprite_label_min_res_scale,
                        "kms_graph_webworker_layout_threshold": guard.kms_graph_webworker_layout_threshold,
                        "kms_graph_webworker_layout_max_ticks": guard.kms_graph_webworker_layout_max_ticks,
                        "kms_graph_webworker_layout_alpha_min": guard.kms_graph_webworker_layout_alpha_min,
                    }),
                );
            }
            _ => {}
        }
    }

    let payload = serde_json::json!({
        "schema_version": SETTINGS_BUNDLE_SCHEMA_V1_1,
        "exported_at_utc": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs().to_string())
            .unwrap_or_else(|_| "0".to_string()),
        "app": {
            "name": "DigiCore Text Expander",
            "format": "settings-bundle"
        },
        "selected_groups": groups,
        "groups": groups_obj
    });

    let serialized = serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?;
    std::fs::write(&path, serialized).map_err(|e| e.to_string())?;
    diag_log("info", format!("[SettingsExport] Wrote settings bundle to {path}"));
    Ok(payload["selected_groups"]
        .as_array()
        .map(|a| a.len() as u32)
        .unwrap_or(0))
}

