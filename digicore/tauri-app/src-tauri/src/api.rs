//! TauRPC API - type-safe IPC procedures for DigiCore Text Expander.

use crate::{
    app_state_to_dto, ConfigUpdateDto, ExpansionStatsDto, GhostFollowerStateDto,
    GhostSuggestorStateDto, UiPrefsDto,
};
use digicore_core::domain::Snippet;
use digicore_text_expander::adapters::storage::JsonFileStorageAdapter;
use digicore_text_expander::application::clipboard_history::{self, ClipboardHistoryConfig};
use digicore_text_expander::application::expansion_diagnostics;
use digicore_text_expander::application::expansion_stats;
use digicore_text_expander::application::discovery;
use digicore_text_expander::application::ghost_follower;
use digicore_text_expander::application::ghost_suggestor;
use digicore_text_expander::application::scripting::{get_scripting_config, set_global_library};
use digicore_text_expander::application::template_processor::{self, InteractiveVarType};
use digicore_text_expander::application::variable_input;
use digicore_text_expander::drivers::hotstring::{
    sync_discovery_config, sync_ghost_config, update_library, GhostConfig,
};
use digicore_text_expander::application::app_state::AppState;
use digicore_text_expander::ports::{storage_keys, StoragePort};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    ClipEntryDto, DiagnosticEntryDto, InteractiveVarDto, PinnedSnippetDto,
    PendingVariableInputDto as PendingVarDto, SuggestionDto,
};

/// Persists app state to JSON storage. Used by save_settings and save_all_on_exit.
fn persist_settings_to_storage(state: &AppState) -> Result<(), String> {
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
        &state.ghost_follower_enabled.to_string(),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_EDGE_RIGHT,
        &state.ghost_follower_edge_right.to_string(),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_MONITOR_ANCHOR,
        &state.ghost_follower_monitor_anchor.to_string(),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_HOVER_PREVIEW,
        &state.ghost_follower_hover_preview.to_string(),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_COLLAPSE_DELAY_SECS,
        &state.ghost_follower_collapse_delay_secs.to_string(),
    );
    storage.set(
        storage_keys::GHOST_FOLLOWER_OPACITY,
        &state.ghost_follower_opacity.to_string(),
    );
    if let Some((px, py)) = state.ghost_follower_position {
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
    storage.persist().map_err(|e| e.to_string())
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

// Export to frontend src/ (outside src-tauri) to avoid watcher rebuild loop
#[taurpc::procedures(export_to = "../src/bindings.ts")]
pub trait Api {
    async fn greet(name: String) -> String;
    async fn get_app_state() -> Result<crate::AppStateDto, String>;
    async fn load_library() -> Result<u32, String>;
    async fn save_library() -> Result<(), String>;
    async fn set_library_path(path: String) -> Result<(), String>;
    async fn save_settings() -> Result<(), String>;
    async fn get_ui_prefs() -> Result<UiPrefsDto, String>;
    async fn save_ui_prefs(last_tab: u32, column_order: Vec<String>) -> Result<(), String>;
    async fn add_snippet(category: String, snippet: Snippet) -> Result<(), String>;
    async fn update_snippet(
        category: String,
        snippet_idx: u32,
        snippet: Snippet,
    ) -> Result<(), String>;
    async fn delete_snippet(category: String, snippet_idx: u32) -> Result<(), String>;
    async fn update_config(config: ConfigUpdateDto) -> Result<(), String>;
    async fn get_clipboard_entries() -> Result<Vec<ClipEntryDto>, String>;
    async fn delete_clip_entry(index: u32) -> Result<(), String>;
    async fn clear_clipboard_history() -> Result<(), String>;
    async fn copy_to_clipboard(text: String) -> Result<(), String>;
    async fn get_script_library_js() -> Result<String, String>;
    async fn save_script_library_js(content: String) -> Result<(), String>;
    async fn get_ghost_suggestor_state() -> Result<GhostSuggestorStateDto, String>;
    async fn ghost_suggestor_accept() -> Result<Option<(String, String)>, String>;
    async fn ghost_suggestor_snooze() -> Result<(), String>;
    async fn ghost_suggestor_dismiss() -> Result<(), String>;
    async fn ghost_suggestor_ignore(phrase: String) -> Result<(), String>;
    async fn ghost_suggestor_create_snippet() -> Result<Option<(String, String)>, String>;
    async fn ghost_suggestor_cycle_forward() -> Result<u32, String>;
    async fn get_ghost_follower_state(search_filter: Option<String>)
        -> Result<GhostFollowerStateDto, String>;
    async fn ghost_follower_insert(trigger: String, content: String) -> Result<(), String>;
    async fn ghost_follower_set_search(filter: String) -> Result<(), String>;
    async fn bring_main_window_to_foreground() -> Result<(), String>;
    async fn ghost_follower_restore_always_on_top() -> Result<(), String>;
    async fn ghost_follower_capture_target_window() -> Result<(), String>;
    async fn ghost_follower_touch() -> Result<(), String>;
    async fn ghost_follower_set_collapsed(collapsed: bool) -> Result<(), String>;
    async fn ghost_follower_set_size(width: f64, height: f64) -> Result<(), String>;
    async fn ghost_follower_set_opacity(opacity_pct: u32) -> Result<(), String>;
    async fn ghost_follower_save_position(x: i32, y: i32) -> Result<(), String>;
    async fn ghost_follower_hide() -> Result<(), String>;
    async fn ghost_follower_request_view_full(content: String) -> Result<(), String>;
    async fn ghost_follower_request_edit(category: String, snippet_idx: u32) -> Result<(), String>;
    async fn ghost_follower_request_promote(content: String, trigger: String) -> Result<(), String>;
    async fn ghost_follower_toggle_pin(category: String, snippet_idx: u32) -> Result<(), String>;
    async fn get_pending_variable_input() -> Result<Option<PendingVarDto>, String>;
    async fn submit_variable_input(values: HashMap<String, String>) -> Result<(), String>;
    async fn cancel_variable_input() -> Result<(), String>;
    async fn get_expansion_stats() -> Result<ExpansionStatsDto, String>;
    async fn reset_expansion_stats() -> Result<(), String>;
    async fn get_diagnostic_logs() -> Result<Vec<DiagnosticEntryDto>, String>;
    async fn clear_diagnostic_logs() -> Result<(), String>;
}

#[derive(Clone)]
pub struct ApiImpl {
    pub state: Arc<Mutex<digicore_text_expander::application::app_state::AppState>>,
    pub app_handle: Arc<Mutex<Option<AppHandle>>>,
}

fn get_app(app: &Arc<Mutex<Option<AppHandle>>>) -> AppHandle {
    app.lock()
        .unwrap()
        .clone()
        .expect("AppHandle not yet set (setup not run)")
}

fn bring_main_to_foreground(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

/// Lowers Ghost Follower's always_on_top so the main window can appear above it,
/// then brings the main window to foreground. Call ghost_follower_restore_always_on_top
/// when the modal is closed.
fn bring_main_to_foreground_above_ghost_follower(app: &AppHandle) {
    if let Some(ghost) = app.get_webview_window("ghost-follower") {
        let _ = ghost.set_always_on_top(false);
    }
    bring_main_to_foreground(app);
}

fn var_type_to_string(t: &InteractiveVarType) -> &'static str {
    match t {
        InteractiveVarType::Edit => "edit",
        InteractiveVarType::Choice => "choice",
        InteractiveVarType::Checkbox => "checkbox",
        InteractiveVarType::DatePicker => "date_picker",
        InteractiveVarType::FilePicker => "file_picker",
    }
}

#[taurpc::resolvers]
impl Api for ApiImpl {
    async fn greet(self, name: String) -> String {
        format!("Hello, {}! DigiCore Text Expander backend ready.", name)
    }

    async fn get_app_state(self) -> Result<crate::AppStateDto, String> {
        let guard = self.state.lock().map_err(|e| e.to_string())?;
        Ok(app_state_to_dto(&guard))
    }

    async fn load_library(self) -> Result<u32, String> {
        let mut guard = self.state.lock().map_err(|e| e.to_string())?;
        let count = guard.try_load_library().map_err(|e| e.to_string())? as u32;
        update_library(guard.library.clone());
        let _ = get_app(&self.app_handle).emit("ghost-follower-update", ());
        Ok(count)
    }

    async fn save_library(self) -> Result<(), String> {
        let mut guard = self.state.lock().map_err(|e| e.to_string())?;
        guard.try_save_library().map_err(|e| e.to_string())
    }

    async fn set_library_path(self, path: String) -> Result<(), String> {
        let mut guard = self.state.lock().map_err(|e| e.to_string())?;
        guard.library_path = path;
        Ok(())
    }

    async fn save_settings(self) -> Result<(), String> {
        let guard = self.state.lock().map_err(|e| e.to_string())?;
        persist_settings_to_storage(&*guard)
    }

    async fn get_ui_prefs(self) -> Result<UiPrefsDto, String> {
        let storage = JsonFileStorageAdapter::load();
        let last_tab: u32 = storage
            .get(storage_keys::UI_LAST_TAB)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let column_order: Vec<String> = storage
            .get(storage_keys::UI_COLUMN_ORDER)
            .map(|s| s.split(',').map(String::from).collect())
            .unwrap_or_else(|| {
                vec![
                    "Profile".into(),
                    "Category".into(),
                    "Trigger".into(),
                    "Content Preview".into(),
                    "AppLock".into(),
                    "Options".into(),
                    "Last Modified".into(),
                ]
            });
        Ok(UiPrefsDto {
            last_tab,
            column_order,
        })
    }

    async fn save_ui_prefs(self, last_tab: u32, column_order: Vec<String>) -> Result<(), String> {
        let mut storage = JsonFileStorageAdapter::load();
        storage.set(storage_keys::UI_LAST_TAB, &last_tab.to_string());
        storage.set(storage_keys::UI_COLUMN_ORDER, &column_order.join(","));
        storage.persist().map_err(|e| e.to_string())
    }

    async fn add_snippet(self, category: String, snippet: Snippet) -> Result<(), String> {
        let mut guard = self.state.lock().map_err(|e| e.to_string())?;
        guard.add_snippet(&category, &snippet);
        update_library(guard.library.clone());
        let _ = get_app(&self.app_handle).emit("ghost-follower-update", ());
        Ok(())
    }

    async fn update_snippet(
        self,
        category: String,
        snippet_idx: u32,
        snippet: Snippet,
    ) -> Result<(), String> {
        let mut guard = self.state.lock().map_err(|e| e.to_string())?;
        guard
            .update_snippet(&category, snippet_idx as usize, &snippet)
            .map_err(|e| e.to_string())?;
        update_library(guard.library.clone());
        let _ = get_app(&self.app_handle).emit("ghost-follower-update", ());
        Ok(())
    }

    async fn delete_snippet(self, category: String, snippet_idx: u32) -> Result<(), String> {
        let mut guard = self.state.lock().map_err(|e| e.to_string())?;
        guard
            .delete_snippet(&category, snippet_idx as usize)
            .map_err(|e| e.to_string())?;
        update_library(guard.library.clone());
        let _ = get_app(&self.app_handle).emit("ghost-follower-update", ());
        Ok(())
    }

    async fn update_config(self, config: ConfigUpdateDto) -> Result<(), String> {
        let mut guard = self.state.lock().map_err(|e| e.to_string())?;
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
            guard.ghost_follower_enabled = v;
        }
        if let Some(v) = config.ghost_follower_edge_right {
            guard.ghost_follower_edge_right = v;
        }
        if let Some(v) = config.ghost_follower_monitor_anchor {
            guard.ghost_follower_monitor_anchor = v as usize;
        }
        if let Some(ref v) = config.ghost_follower_search {
            guard.ghost_follower_search = v.clone();
        }
        if let Some(v) = config.ghost_follower_hover_preview {
            guard.ghost_follower_hover_preview = v;
        }
        if let Some(v) = config.ghost_follower_collapse_delay_secs {
            guard.ghost_follower_collapse_delay_secs = v as u64;
        }
        if let Some(v) = config.ghost_follower_opacity {
            guard.ghost_follower_opacity = v.clamp(10, 100);
        }
        if let Some(v) = config.clip_history_max_depth {
            let depth = v.clamp(5, 100) as usize;
            guard.clip_history_max_depth = depth;
            clipboard_history::update_config(ClipboardHistoryConfig {
                enabled: true,
                max_depth: depth,
            });
        }
        if let Some(v) = config.script_library_run_disabled {
            guard.script_library_run_disabled = v;
        }
        if let Some(ref v) = config.script_library_run_allowlist {
            guard.script_library_run_allowlist = v.clone();
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
            follower_enabled: guard.ghost_follower_enabled,
            follower_edge_right: guard.ghost_follower_edge_right,
            follower_monitor_anchor: guard.ghost_follower_monitor_anchor,
            follower_search: guard.ghost_follower_search.clone(),
            follower_hover_preview: guard.ghost_follower_hover_preview,
            follower_collapse_delay_secs: guard.ghost_follower_collapse_delay_secs,
        });
        let _ = get_app(&self.app_handle).emit("ghost-follower-update", ());
        Ok(())
    }

    async fn get_clipboard_entries(self) -> Result<Vec<ClipEntryDto>, String> {
        let entries = clipboard_history::get_entries();
        Ok(entries
            .into_iter()
            .map(|e| ClipEntryDto {
                content: e.content.clone(),
                process_name: e.process_name,
                window_title: e.window_title,
                length: e.content.len() as u32,
            })
            .collect())
    }

    async fn delete_clip_entry(self, index: u32) -> Result<(), String> {
        clipboard_history::delete_entry_at(index as usize);
        Ok(())
    }

    async fn clear_clipboard_history(self) -> Result<(), String> {
        clipboard_history::clear_all();
        Ok(())
    }

    async fn copy_to_clipboard(self, text: String) -> Result<(), String> {
        arboard::Clipboard::new()
            .map_err(|e| e.to_string())?
            .set_text(&text)
            .map_err(|e| e.to_string())
    }

    async fn get_script_library_js(self) -> Result<String, String> {
        let cfg = get_scripting_config();
        let base = dirs::config_dir()
            .unwrap_or_else(|| Path::new(".").into())
            .join("DigiCore");
        let lib_path = if cfg.js.library_paths.is_empty() {
            base.join(&cfg.js.library_path)
        } else {
            base.join(cfg.js.library_paths.first().unwrap_or(&String::new()))
        };
        Ok(std::fs::read_to_string(&lib_path).unwrap_or_else(|_| {
            r#"/**
 * Text Expansion Pro - Global Script Library
 */
function greet(name) { return "Hello, " + name + "!"; }
"#
            .to_string()
        }))
    }

    async fn save_script_library_js(self, content: String) -> Result<(), String> {
        let cfg = get_scripting_config();
        let base = dirs::config_dir()
            .unwrap_or_else(|| Path::new(".").into())
            .join("DigiCore");
        let lib_path = if cfg.js.library_paths.is_empty() {
            base.join(&cfg.js.library_path)
        } else {
            base.join(cfg.js.library_paths.first().unwrap_or(&String::new()))
        };
        if let Some(parent) = lib_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&lib_path, &content).map_err(|e| e.to_string())?;
        set_global_library(content);
        Ok(())
    }

    async fn get_ghost_suggestor_state(self) -> Result<GhostSuggestorStateDto, String> {
        let suggestions = ghost_suggestor::get_suggestions();
        let selected = ghost_suggestor::get_selected_index();
        let has_suggestions = !suggestions.is_empty();
        let first_trigger = suggestions.first().map(|s| s.snippet.trigger.len()).unwrap_or(0);
        log::info!(
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
        log::info!(
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

    async fn ghost_suggestor_accept(self) -> Result<Option<(String, String)>, String> {
        Ok(ghost_suggestor::accept_selected())
    }

    async fn ghost_suggestor_snooze(self) -> Result<(), String> {
        ghost_suggestor::snooze();
        Ok(())
    }

    async fn ghost_suggestor_dismiss(self) -> Result<(), String> {
        ghost_suggestor::dismiss();
        Ok(())
    }

    async fn ghost_suggestor_ignore(self, phrase: String) -> Result<(), String> {
        discovery::add_ignored_phrase(&phrase);
        ghost_suggestor::dismiss();
        Ok(())
    }

    async fn ghost_suggestor_create_snippet(self) -> Result<Option<(String, String)>, String> {
        let suggestions = ghost_suggestor::get_suggestions();
        let idx = ghost_suggestor::get_selected_index().min(suggestions.len().saturating_sub(1));
        if let Some(s) = suggestions.get(idx) {
            ghost_suggestor::request_create_snippet(
                s.snippet.trigger.clone(),
                s.snippet.content.clone(),
            );
            ghost_suggestor::dismiss();
            Ok(Some((s.snippet.trigger.clone(), s.snippet.content.clone())))
        } else {
            Ok(None)
        }
    }

    async fn ghost_suggestor_cycle_forward(self) -> Result<u32, String> {
        Ok(ghost_suggestor::cycle_selection_forward() as u32)
    }

    async fn get_ghost_follower_state(
        self,
        search_filter: Option<String>,
    ) -> Result<GhostFollowerStateDto, String> {
        let filter = search_filter.as_deref().unwrap_or("");
        let pinned = ghost_follower::get_pinned_snippets(filter);
        let cfg = ghost_follower::get_config();
        let enabled = ghost_follower::is_enabled();
        log::info!(
            "[GhostFollower] get_ghost_follower_state: enabled={}, pinned_count={}, filter_len={}",
            enabled,
            pinned.len(),
            filter.len()
        );

        #[cfg(target_os = "windows")]
        let (position, saved_position) = {
            let saved = self
                .state
                .lock()
                .ok()
                .and_then(|g| g.ghost_follower_position);
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
                            .unwrap_or_else(digicore_text_expander::platform::windows_monitor::get_primary_monitor_work_area)
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

        let clip_max = self
            .state
            .lock()
            .map_err(|e| e.to_string())?
            .clip_history_max_depth as u32;

        let collapse_delay = cfg.collapse_delay_secs as u32;
        let should_collapse = ghost_follower::should_collapse(cfg.collapse_delay_secs);

        let opacity = self
            .state
            .lock()
            .map(|g| (g.ghost_follower_opacity as f64 / 100.0).clamp(0.1, 1.0))
            .unwrap_or(1.0);

        Ok(GhostFollowerStateDto {
            enabled,
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
            search_filter: ghost_follower::get_search_filter(),
            position,
            edge_right: cfg.edge == ghost_follower::FollowerEdge::Right,
            monitor_primary: cfg.monitor_anchor == ghost_follower::MonitorAnchor::Primary,
            clip_history_max_depth: clip_max,
            should_collapse,
            collapse_delay_secs: collapse_delay,
            opacity,
            saved_position,
        })
    }

    async fn ghost_follower_insert(self, _trigger: String, content: String) -> Result<(), String> {
        digicore_text_expander::drivers::hotstring::request_expansion_from_ghost_follower(content);
        Ok(())
    }

    async fn bring_main_window_to_foreground(self) -> Result<(), String> {
        let app = get_app(&self.app_handle);
        bring_main_to_foreground_above_ghost_follower(&app);
        Ok(())
    }

    async fn ghost_follower_restore_always_on_top(self) -> Result<(), String> {
        if let Some(ghost) = get_app(&self.app_handle).get_webview_window("ghost-follower") {
            let _ = ghost.set_always_on_top(true);
        }
        Ok(())
    }

    async fn ghost_follower_capture_target_window(self) -> Result<(), String> {
        digicore_text_expander::application::ghost_follower::capture_target_window();
        Ok(())
    }

    async fn ghost_follower_touch(self) -> Result<(), String> {
        ghost_follower::touch();
        Ok(())
    }

    async fn ghost_follower_set_collapsed(self, collapsed: bool) -> Result<(), String> {
        ghost_follower::set_collapsed(collapsed);
        Ok(())
    }

    async fn ghost_follower_set_size(self, width: f64, height: f64) -> Result<(), String> {
        use tauri::LogicalSize;
        if let Some(win) = get_app(&self.app_handle).get_webview_window("ghost-follower") {
            let _ = win.set_size(LogicalSize::new(width, height));
        }
        Ok(())
    }

    async fn ghost_follower_set_opacity(self, opacity_pct: u32) -> Result<(), String> {
        let val = opacity_pct.clamp(10, 100);
        if let Ok(mut guard) = self.state.lock() {
            guard.ghost_follower_opacity = val;
        }
        let _ = get_app(&self.app_handle).emit("ghost-follower-update", ());
        Ok(())
    }

    async fn ghost_follower_save_position(self, x: i32, y: i32) -> Result<(), String> {
        let sane = x >= -20000 && x <= 20000 && y >= -20000 && y <= 20000;
        if !sane {
            return Ok(());
        }
        if let Ok(mut guard) = self.state.lock() {
            guard.ghost_follower_position = Some((x, y));
        }
        let mut storage = JsonFileStorageAdapter::load();
        storage.set(storage_keys::GHOST_FOLLOWER_POSITION_X, &x.to_string());
        storage.set(storage_keys::GHOST_FOLLOWER_POSITION_Y, &y.to_string());
        let _ = storage.persist_if_safe().map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn ghost_follower_hide(self) -> Result<(), String> {
        if let Some(win) = get_app(&self.app_handle).get_webview_window("ghost-follower") {
            let _ = win.hide();
        }
        Ok(())
    }

    async fn ghost_follower_set_search(self, filter: String) -> Result<(), String> {
        ghost_follower::set_search_filter(&filter);
        let _ = get_app(&self.app_handle).emit("ghost-follower-update", ());
        Ok(())
    }

    async fn ghost_follower_request_view_full(self, content: String) -> Result<(), String> {
        let app = get_app(&self.app_handle);
        bring_main_to_foreground_above_ghost_follower(&app);
        let _ = app.emit("ghost-follower-view-full", content);
        Ok(())
    }

    async fn ghost_follower_request_edit(
        self,
        category: String,
        snippet_idx: u32,
    ) -> Result<(), String> {
        let app = get_app(&self.app_handle);
        bring_main_to_foreground_above_ghost_follower(&app);
        let _ = app.emit(
            "ghost-follower-edit",
            serde_json::json!({ "category": category, "snippetIdx": snippet_idx as usize }),
        );
        Ok(())
    }

    async fn ghost_follower_request_promote(
        self,
        content: String,
        trigger: String,
    ) -> Result<(), String> {
        let app = get_app(&self.app_handle);
        bring_main_to_foreground_above_ghost_follower(&app);
        let _ = app.emit(
            "ghost-follower-promote",
            serde_json::json!({ "content": content, "trigger": trigger }),
        );
        Ok(())
    }

    async fn ghost_follower_toggle_pin(
        self,
        category: String,
        snippet_idx: u32,
    ) -> Result<(), String> {
        let mut guard = self.state.lock().map_err(|e| e.to_string())?;
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
        update_library(guard.library.clone());
        let _ = get_app(&self.app_handle).emit("ghost-follower-update", ());
        Ok(())
    }

    async fn get_pending_variable_input(self) -> Result<Option<PendingVarDto>, String> {
        if let Some((content, vars, values, choice_indices, checkbox_checked)) =
            variable_input::get_viewport_modal_display()
        {
            Ok(Some(PendingVarDto {
                content,
                vars: vars
                    .iter()
                    .map(|v| InteractiveVarDto {
                        tag: v.tag.clone(),
                        label: v.label.clone(),
                        var_type: var_type_to_string(&v.var_type).to_string(),
                        options: v.options.clone(),
                    })
                    .collect(),
                values,
                choice_indices: choice_indices
                    .into_iter()
                    .map(|(k, v)| (k, v as u32))
                    .collect(),
                checkbox_checked,
            }))
        } else {
            Ok(None)
        }
    }

    async fn submit_variable_input(self, values: HashMap<String, String>) -> Result<(), String> {
        if let Some(state) = variable_input::take_viewport_modal() {
            let clip_history: Vec<String> = clipboard_history::get_entries()
                .iter()
                .map(|e| e.content.clone())
                .collect();
            let processed = template_processor::process_with_user_vars(
                &state.content,
                None,
                &clip_history,
                Some(&values),
            );
            let hwnd = state.target_hwnd;
            if let Some(ref tx) = state.response_tx {
                let _ = tx.send((Some(processed), hwnd));
            } else {
                digicore_text_expander::drivers::hotstring::request_expansion(processed);
            }
        }
        Ok(())
    }

    async fn cancel_variable_input(self) -> Result<(), String> {
        if let Some(state) = variable_input::take_viewport_modal() {
            if let Some(ref tx) = state.response_tx {
                let _ = tx.send((None, None));
            }
        }
        Ok(())
    }

    async fn get_expansion_stats(self) -> Result<ExpansionStatsDto, String> {
        let stats = expansion_stats::get_stats();
        Ok(ExpansionStatsDto {
            total_expansions: stats.total_expansions as u32,
            total_chars_saved: stats.total_chars_saved as u32,
            estimated_time_saved_secs: stats.estimated_time_saved_secs(),
            top_triggers: stats
                .top_triggers(10)
                .into_iter()
                .map(|(s, c)| (s, c as u32))
                .collect(),
        })
    }

    async fn reset_expansion_stats(self) -> Result<(), String> {
        expansion_stats::reset_stats();
        Ok(())
    }

    async fn get_diagnostic_logs(self) -> Result<Vec<DiagnosticEntryDto>, String> {
        let entries = expansion_diagnostics::get_recent();
        Ok(entries
            .into_iter()
            .map(|e| DiagnosticEntryDto {
                timestamp_ms: e.timestamp_ms as u32,
                level: e.level,
                message: e.message,
            })
            .collect())
    }

    async fn clear_diagnostic_logs(self) -> Result<(), String> {
        expansion_diagnostics::clear();
        Ok(())
    }
}
