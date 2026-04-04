//! Bounded inbound service for ghost and appearance RPC orchestration.

use std::collections::HashSet;

use crate::appearance_enforcement::{
    apply_process_transparency, effective_rules_for_enforcement, load_appearance_rules,
    normalize_process_key, save_appearance_rules, sort_appearance_rules_deterministic,
};
use super::*;

pub(crate) async fn get_appearance_transparency_rules(
    _host: ApiImpl,
) -> Result<Vec<AppearanceTransparencyRuleDto>, String> {
    let storage = JsonFileStorageAdapter::load();
    let mut rules = load_appearance_rules(&storage);
    for rule in effective_rules_for_enforcement(rules.clone()) {
        let _ = apply_process_transparency(&rule.app_process, Some(rule.opacity.clamp(20, 255) as u8));
    }
    sort_appearance_rules_deterministic(&mut rules);
    Ok(rules)
}

pub(crate) async fn save_appearance_transparency_rule(
    _host: ApiImpl,
    app_process: String,
    opacity: u32,
    enabled: bool,
) -> Result<(), String> {
    let mut app = app_process.trim().to_ascii_lowercase();
    if app.is_empty() {
        return Err("App process is required".to_string());
    }
    if !app.ends_with(".exe") {
        app.push_str(".exe");
    }
    let app_for_apply = app.clone();
    let opacity = opacity.clamp(20, 255);
    let storage = JsonFileStorageAdapter::load();
    let mut rules = load_appearance_rules(&storage);
    if let Some(existing) = rules
        .iter_mut()
        .find(|r| normalize_process_key(&r.app_process) == normalize_process_key(&app))
    {
        existing.opacity = opacity;
        existing.enabled = enabled;
        existing.app_process = app;
    } else {
        rules.push(AppearanceTransparencyRuleDto {
            app_process: app,
            opacity,
            enabled,
        });
    }
    sort_appearance_rules_deterministic(&mut rules);
    save_appearance_rules(&rules)?;
    if enabled {
        let _ = apply_process_transparency(&app_for_apply, Some(opacity as u8));
    }
    Ok(())
}

pub(crate) async fn delete_appearance_transparency_rule(
    _host: ApiImpl,
    app_process: String,
) -> Result<(), String> {
    let app = normalize_process_key(&app_process);
    if app.is_empty() {
        return Ok(());
    }
    let storage = JsonFileStorageAdapter::load();
    let mut rules = load_appearance_rules(&storage);
    rules.retain(|r| normalize_process_key(&r.app_process) != app);
    save_appearance_rules(&rules)?;
    let _ = apply_process_transparency(&format!("{app}.exe"), None);
    Ok(())
}

pub(crate) async fn apply_appearance_transparency_now(
    _host: ApiImpl,
    app_process: String,
    opacity: u32,
) -> Result<u32, String> {
    let alpha = opacity.clamp(20, 255) as u8;
    apply_process_transparency(&app_process, Some(alpha))
}

pub(crate) async fn restore_appearance_defaults(_host: ApiImpl) -> Result<u32, String> {
    let storage = JsonFileStorageAdapter::load();
    let rules = load_appearance_rules(&storage);
    save_appearance_rules(&[])?;

    let mut seen = HashSet::new();
    let mut cleared_windows = 0u32;
    for rule in rules {
        let key = normalize_process_key(&rule.app_process);
        if key.is_empty() || !seen.insert(key.clone()) {
            continue;
        }
        let app_name = format!("{key}.exe");
        let count = apply_process_transparency(&app_name, None).unwrap_or(0);
        cleared_windows = cleared_windows.saturating_add(count);
    }
    Ok(cleared_windows)
}

pub(crate) async fn get_ghost_suggestor_state(_host: ApiImpl) -> Result<GhostSuggestorStateDto, String> {
    let suggestions = ghost_suggestor::get_suggestions();
    let selected = ghost_suggestor::get_selected_index();
    let has_suggestions = !suggestions.is_empty();
    let first_trigger = suggestions.first().map(|s| s.snippet.trigger.len()).unwrap_or(0);
    log::debug!(
        "[GhostSuggestor] get_ghost_suggestor_state: suggestions={} has_suggestions={} selected={} first_trigger_len={}",
        suggestions.len(),
        has_suggestions,
        selected,
        first_trigger
    );
    let should_auto_hide = ghost_suggestor::should_auto_hide();
    let should_passthrough = should_auto_hide || !has_suggestions;
    if should_auto_hide && has_suggestions {
        ghost_suggestor::dismiss();
    }
    let suggestions = ghost_suggestor::get_suggestions();
    let has_suggestions = !suggestions.is_empty();
    #[cfg(target_os = "windows")]
    let position = {
        let pos = digicore_text_expander::platform::windows_caret::get_caret_screen_position();
        let cfg = ghost_suggestor::get_config();
        let raw = pos.map(|(x, y)| (x + cfg.offset_x, y + cfg.offset_y));
        raw.map(|(x, y)| {
            digicore_text_expander::platform::windows_monitor::clamp_position_to_work_area(
                x, y, 320, 260,
            )
        })
    };
    #[cfg(not(target_os = "windows"))]
    let position: Option<(i32, i32)> = None;
    log::debug!(
        "[GhostSuggestor] get_ghost_suggestor_state: returning has_suggestions={} position={:?}",
        has_suggestions,
        position
    );
    Ok(GhostSuggestorStateDto {
        has_suggestions,
        suggestions: suggestions
            .into_iter()
            .map(|s| SuggestionDto {
                trigger: s.snippet.trigger,
                content_preview: if s.snippet.content.len() > 40 {
                    format!("{}...", &s.snippet.content[..40])
                } else {
                    s.snippet.content
                },
                category: s.category,
            })
            .collect(),
        selected_index: selected as u32,
        position,
        should_passthrough,
    })
}

pub(crate) async fn ghost_suggestor_accept(_host: ApiImpl) -> Result<Option<(String, String)>, String> {
    Ok(ghost_suggestor::accept_selected())
}

pub(crate) async fn ghost_suggestor_snooze(_host: ApiImpl) -> Result<(), String> {
    ghost_suggestor::snooze();
    Ok(())
}

pub(crate) async fn ghost_suggestor_dismiss(_host: ApiImpl) -> Result<(), String> {
    ghost_suggestor::dismiss();
    Ok(())
}

pub(crate) async fn ghost_suggestor_ignore(_host: ApiImpl, phrase: String) -> Result<(), String> {
    discovery::add_ignored_phrase(&phrase);
    ghost_suggestor::dismiss();
    Ok(())
}

pub(crate) async fn ghost_suggestor_create_snippet(
    _host: ApiImpl,
) -> Result<Option<(String, String)>, String> {
    let suggestions = ghost_suggestor::get_suggestions();
    let idx = ghost_suggestor::get_selected_index().min(suggestions.len().saturating_sub(1));
    if let Some(s) = suggestions.get(idx) {
        ghost_suggestor::request_create_snippet(s.snippet.trigger.clone(), s.snippet.content.clone());
        ghost_suggestor::dismiss();
        Ok(Some((s.snippet.trigger.clone(), s.snippet.content.clone())))
    } else {
        Ok(None)
    }
}

pub(crate) async fn ghost_suggestor_cycle_forward(_host: ApiImpl) -> Result<u32, String> {
    Ok(ghost_suggestor::cycle_selection_forward() as u32)
}

pub(crate) async fn get_ghost_follower_state(
    host: ApiImpl,
    search_filter: Option<String>,
) -> Result<GhostFollowerStateDto, String> {
    let filter = search_filter.as_deref().unwrap_or("");
    let guard = host.state.lock().map_err(|e| e.to_string())?;
    let gf_state = &guard.ghost_follower;
    let pinned = ghost_follower::get_pinned_snippets(gf_state, filter);
    let cfg = gf_state.config.clone();
    let enabled = cfg.enabled;
    log::debug!(
        "[GhostFollower] get_ghost_follower_state: enabled={}, pinned_count={}, filter_len={}",
        enabled,
        pinned.len(),
        filter.len()
    );

    #[cfg(target_os = "windows")]
    let (position, saved_position) = {
        let saved = guard.ghost_follower.config.position;
        let use_saved = saved.map_or(false, |(x, y)| {
            x >= -20000 && x <= 20000 && y >= -20000 && y <= 20000
        });
        if use_saved {
            (saved, true)
        } else {
            let work = match cfg.monitor_anchor {
                ghost_follower::MonitorAnchor::Primary => {
                    digicore_text_expander::platform::windows_monitor::get_primary_monitor_work_area()
                }
                ghost_follower::MonitorAnchor::Secondary => {
                    digicore_text_expander::platform::windows_monitor::get_secondary_monitor_work_area()
                        .unwrap_or_else(
                            digicore_text_expander::platform::windows_monitor::get_primary_monitor_work_area,
                        )
                }
                ghost_follower::MonitorAnchor::Current => {
                    digicore_text_expander::platform::windows_monitor::get_current_monitor_work_area()
                }
            };
            let (x, _y) = match cfg.edge {
                ghost_follower::FollowerEdge::Right => (work.right - 280, work.top + 20),
                ghost_follower::FollowerEdge::Left => (work.left, work.top + 20),
            };
            (Some((x, work.top + 20)), false)
        }
    };
    #[cfg(not(target_os = "windows"))]
    let (position, saved_position): (Option<(i32, i32)>, bool) = (None, false);

    let ghost_follower_opacity = gf_state.config.opacity;
    let _ghost_follower_position = gf_state.config.position;

    let collapse_delay = cfg.collapse_delay_secs as u32;
    let should_collapse = gf_state.should_collapse();

    let opacity = (ghost_follower_opacity as f64 / 100.0).clamp(0.1, 1.0);

    Ok(GhostFollowerStateDto {
        enabled,
        mode: format!("{:?}", gf_state.config.mode),
        expand_trigger: format!("{:?}", gf_state.config.expand_trigger),
        expand_delay_ms: gf_state.config.expand_delay_ms as u32,
        clipboard_depth: gf_state.config.clipboard_depth as u32,
        pinned: pinned
            .into_iter()
            .map(|(s, cat, idx)| PinnedSnippetDto {
                trigger: s.trigger.clone(),
                content: s.content.clone(),
                content_preview: if s.content.len() > 40 {
                    format!("{}...", &s.content[..40])
                } else {
                    s.content.clone()
                },
                category: cat,
                snippet_idx: idx as u32,
            })
            .collect(),
        search_filter: gf_state.search_filter.clone(),
        position,
        edge_right: cfg.edge == FollowerEdge::Right,
        monitor_primary: cfg.monitor_anchor == MonitorAnchor::Primary,
        clip_history_max_depth: guard.clip_history_max_depth as u32,
        should_collapse,
        collapse_delay_secs: collapse_delay,
        opacity,
        saved_position,
    })
}

pub(crate) async fn ghost_follower_insert(
    _host: ApiImpl,
    _trigger: String,
    content: String,
) -> Result<(), String> {
    log::debug!(
        "[QuickSearchInsert] ghost_follower_insert invoked content_len={}",
        content.len()
    );
    digicore_text_expander::drivers::hotstring::request_expansion_from_ghost_follower(content);
    Ok(())
}

pub(crate) async fn bring_main_window_to_foreground(host: ApiImpl) -> Result<(), String> {
    let app = get_app(&host.app_handle);
    bring_main_to_foreground_above_ghost_follower(&app);
    Ok(())
}

pub(crate) async fn ghost_follower_restore_always_on_top(host: ApiImpl) -> Result<(), String> {
    if let Some(ghost) = get_app(&host.app_handle).get_webview_window("ghost-follower") {
        let _ = ghost.set_always_on_top(true);
    }
    Ok(())
}

pub(crate) async fn ghost_follower_capture_target_window(_host: ApiImpl) -> Result<(), String> {
    log::debug!("[QuickSearchInsert] ghost_follower_capture_target_window invoked");
    ghost_follower::capture_target_window_global();
    Ok(())
}

pub(crate) async fn ghost_follower_touch(host: ApiImpl) -> Result<(), String> {
    if let Ok(mut guard) = host.state.lock() {
        guard.ghost_follower.touch();
    }
    Ok(())
}

pub(crate) async fn ghost_follower_set_collapsed(
    host: ApiImpl,
    collapsed: bool,
) -> Result<(), String> {
    if let Ok(mut guard) = host.state.lock() {
        guard.ghost_follower.collapsed = collapsed;
    }
    Ok(())
}

pub(crate) async fn ghost_follower_set_size(
    host: ApiImpl,
    width: f64,
    height: f64,
) -> Result<(), String> {
    use tauri::LogicalSize;
    if let Some(win) = get_app(&host.app_handle).get_webview_window("ghost-follower") {
        let _ = win.set_size(LogicalSize::new(width, height));
    }
    Ok(())
}

pub(crate) async fn ghost_follower_set_opacity(
    host: ApiImpl,
    opacity_pct: u32,
) -> Result<(), String> {
    let val = opacity_pct.clamp(10, 100);
    if let Ok(mut guard) = host.state.lock() {
        guard.ghost_follower.config.opacity = val;
    }
    let _ = get_app(&host.app_handle).emit("ghost-follower-update", ());
    Ok(())
}

pub(crate) async fn ghost_follower_save_position(
    host: ApiImpl,
    x: i32,
    y: i32,
) -> Result<(), String> {
    let sane = x >= -20000 && x <= 20000 && y >= -20000 && y <= 20000;
    if !sane {
        return Ok(());
    }
    if let Ok(mut guard) = host.state.lock() {
        guard.ghost_follower.config.position = Some((x, y));
    }
    let mut storage = JsonFileStorageAdapter::load();
    storage.set(storage_keys::GHOST_FOLLOWER_POSITION_X, &x.to_string());
    storage.set(storage_keys::GHOST_FOLLOWER_POSITION_Y, &y.to_string());
    let _ = storage.persist_if_safe().map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) async fn ghost_follower_hide(host: ApiImpl) -> Result<(), String> {
    if let Some(win) = get_app(&host.app_handle).get_webview_window("ghost-follower") {
        let _ = win.hide();
    }
    Ok(())
}

pub(crate) async fn ghost_follower_set_search(host: ApiImpl, filter: String) -> Result<(), String> {
    if let Ok(mut guard) = host.state.lock() {
        guard.ghost_follower.search_filter = filter;
    }
    let _ = get_app(&host.app_handle).emit("ghost-follower-update", ());
    Ok(())
}

pub(crate) async fn ghost_follower_request_view_full(
    host: ApiImpl,
    content: String,
) -> Result<(), String> {
    let app = get_app(&host.app_handle);
    bring_main_to_foreground_above_ghost_follower(&app);
    let _ = app.emit("ghost-follower-view-full", content);
    Ok(())
}

pub(crate) async fn ghost_follower_request_edit(
    host: ApiImpl,
    category: String,
    snippet_idx: u32,
) -> Result<(), String> {
    let app = get_app(&host.app_handle);
    bring_main_to_foreground_above_ghost_follower(&app);
    let _ = app.emit(
        "ghost-follower-edit",
        serde_json::json!({ "category": category, "snippetIdx": snippet_idx as usize }),
    );
    Ok(())
}

pub(crate) async fn ghost_follower_request_promote(
    host: ApiImpl,
    content: String,
    trigger: String,
) -> Result<(), String> {
    let app = get_app(&host.app_handle);
    bring_main_to_foreground_above_ghost_follower(&app);
    let _ = app.emit(
        "ghost-follower-promote",
        serde_json::json!({ "content": content, "trigger": trigger }),
    );
    Ok(())
}

pub(crate) async fn ghost_follower_toggle_pin(
    host: ApiImpl,
    category: String,
    snippet_idx: u32,
) -> Result<(), String> {
    let mut guard = host.state.lock().map_err(|e| e.to_string())?;
    let snippets = guard
        .library
        .get_mut(&category)
        .ok_or_else(|| "Category not found".to_string())?;
    let s = snippets
        .get_mut(snippet_idx as usize)
        .ok_or_else(|| "Snippet not found".to_string())?;
    let new_pinned = if s.pinned.eq_ignore_ascii_case("true") {
        "false"
    } else {
        "true"
    };
    s.pinned = new_pinned.to_string();
    guard.try_save_library().map_err(|e| e.to_string())?;
    let library_clone = guard.library.clone();
    guard.ghost_follower.update_library(&library_clone);
    update_library(library_clone);
    let _ = get_app(&host.app_handle).emit("ghost-follower-update", ());
    Ok(())
}

