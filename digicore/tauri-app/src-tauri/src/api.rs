//! TauRPC API - type-safe IPC procedures for DigiCore Text Expander.

use crate::{
    app_state_to_dto, AppearanceTransparencyRuleDto, ConfigUpdateDto, ExpansionStatsDto, GhostFollowerStateDto,
    GhostSuggestorStateDto, SettingsBundlePreviewDto, SettingsImportResultDto, UiPrefsDto,
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
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager};
#[cfg(target_os = "windows")]
use windows::core::BOOL;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetLayeredWindowAttributes, GetWindowLongW, GetWindowThreadProcessId, IsWindowVisible,
    SetLayeredWindowAttributes, SetWindowLongW, GWL_EXSTYLE, LWA_ALPHA, WS_EX_LAYERED,
};

use crate::{
    ClipEntryDto, DiagnosticEntryDto, InteractiveVarDto, PinnedSnippetDto,
    PendingVariableInputDto as PendingVarDto, SuggestionDto,
};

fn load_appearance_rules(storage: &JsonFileStorageAdapter) -> Vec<AppearanceTransparencyRuleDto> {
    storage
        .get(storage_keys::APPEARANCE_TRANSPARENCY_RULES_JSON)
        .and_then(|s| serde_json::from_str::<Vec<AppearanceTransparencyRuleDto>>(&s).ok())
        .unwrap_or_default()
}

fn normalize_process_key(name: &str) -> String {
    name.trim()
        .to_ascii_lowercase()
        .trim_end_matches(".exe")
        .to_string()
}

fn sort_appearance_rules_deterministic(rules: &mut [AppearanceTransparencyRuleDto]) {
    rules.sort_by(|a, b| {
        b.enabled
            .cmp(&a.enabled)
            .then_with(|| normalize_process_key(&a.app_process).cmp(&normalize_process_key(&b.app_process)))
            .then_with(|| a.app_process.to_ascii_lowercase().cmp(&b.app_process.to_ascii_lowercase()))
            .then_with(|| a.opacity.cmp(&b.opacity))
    });
}

fn effective_rules_for_enforcement(mut rules: Vec<AppearanceTransparencyRuleDto>) -> Vec<AppearanceTransparencyRuleDto> {
    sort_appearance_rules_deterministic(&mut rules);
    let mut seen = HashSet::new();
    let mut effective = Vec::new();
    for rule in rules {
        let key = normalize_process_key(&rule.app_process);
        if key.is_empty() || !seen.insert(key) {
            continue;
        }
        if rule.enabled {
            effective.push(rule);
        }
    }
    effective
}

fn save_appearance_rules(rules: &[AppearanceTransparencyRuleDto]) -> Result<(), String> {
    let mut storage = JsonFileStorageAdapter::load();
    let serialized = serde_json::to_string(rules).map_err(|e| e.to_string())?;
    storage.set(storage_keys::APPEARANCE_TRANSPARENCY_RULES_JSON, &serialized);
    storage
        .persist_if_safe()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

const SETTINGS_GROUP_TEMPLATES: &str = "templates";
const SETTINGS_GROUP_SYNC: &str = "sync";
const SETTINGS_GROUP_DISCOVERY: &str = "discovery";
const SETTINGS_GROUP_GHOST_SUGGESTOR: &str = "ghost_suggestor";
const SETTINGS_GROUP_GHOST_FOLLOWER: &str = "ghost_follower";
const SETTINGS_GROUP_CLIPBOARD_HISTORY: &str = "clipboard_history";
const SETTINGS_GROUP_CORE: &str = "core";
const SETTINGS_GROUP_SCRIPT_RUNTIME: &str = "script_runtime";
const SETTINGS_GROUP_APPEARANCE: &str = "appearance";

fn all_settings_groups() -> Vec<&'static str> {
    vec![
        SETTINGS_GROUP_TEMPLATES,
        SETTINGS_GROUP_SYNC,
        SETTINGS_GROUP_DISCOVERY,
        SETTINGS_GROUP_GHOST_SUGGESTOR,
        SETTINGS_GROUP_GHOST_FOLLOWER,
        SETTINGS_GROUP_CLIPBOARD_HISTORY,
        SETTINGS_GROUP_CORE,
        SETTINGS_GROUP_SCRIPT_RUNTIME,
        SETTINGS_GROUP_APPEARANCE,
    ]
}

fn normalize_settings_group(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "templates" => Some(SETTINGS_GROUP_TEMPLATES),
        "sync" => Some(SETTINGS_GROUP_SYNC),
        "discovery" => Some(SETTINGS_GROUP_DISCOVERY),
        "ghost_suggestor" | "ghost-suggestor" | "ghost suggestor" => Some(SETTINGS_GROUP_GHOST_SUGGESTOR),
        "ghost_follower" | "ghost-follower" | "ghost follower" => Some(SETTINGS_GROUP_GHOST_FOLLOWER),
        "clipboard_history" | "clipboard-history" | "clipboard history" => Some(SETTINGS_GROUP_CLIPBOARD_HISTORY),
        "core" => Some(SETTINGS_GROUP_CORE),
        "script_runtime" | "script-runtime" | "script runtime" => Some(SETTINGS_GROUP_SCRIPT_RUNTIME),
        "appearance" => Some(SETTINGS_GROUP_APPEARANCE),
        _ => None,
    }
}

fn normalized_selected_groups(groups: &[String]) -> Vec<String> {
    let mut out = Vec::<String>::new();
    if groups.is_empty() {
        return all_settings_groups().into_iter().map(str::to_string).collect();
    }
    for g in groups {
        if let Some(n) = normalize_settings_group(g) {
            if !out.iter().any(|v| v == n) {
                out.push(n.to_string());
            }
        }
    }
    out
}

fn diag_log(level: &str, message: impl Into<String>) {
    let msg = message.into();
    expansion_diagnostics::push(level, msg.clone());
    match level {
        "error" => log::error!("{msg}"),
        "warn" => log::warn!("{msg}"),
        _ => log::info!("{msg}"),
    }
}

pub(crate) fn enforce_appearance_transparency_rules() {
    let storage = JsonFileStorageAdapter::load();
    let rules = load_appearance_rules(&storage);
    for rule in effective_rules_for_enforcement(rules) {
        let _ = apply_process_transparency(
            &rule.app_process,
            Some(rule.opacity.clamp(20, 255) as u8),
        );
    }
}

#[cfg(target_os = "windows")]
fn transparency_cache() -> &'static Mutex<HashMap<isize, u8>> {
    static CACHE: OnceLock<Mutex<HashMap<isize, u8>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(target_os = "windows")]
struct TransparencyApplyContext {
    target_pids: std::collections::HashSet<u32>,
    alpha: Option<u8>,
    applied: u32,
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn enum_apply_transparency(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = &mut *(lparam.0 as *mut TransparencyApplyContext);
    let hwnd_key = hwnd.0 as isize;
    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }
    let mut pid = 0u32;
    let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == 0 || !ctx.target_pids.contains(&pid) {
        return BOOL(1);
    }
    let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
    if let Some(alpha) = ctx.alpha {
        if let Ok(cache) = transparency_cache().lock() {
            if cache.get(&hwnd_key).copied() == Some(alpha) {
                return BOOL(1);
            }
        }
        let mut next_style = ex_style;
        if (next_style & WS_EX_LAYERED.0 as i32) == 0 {
            next_style |= WS_EX_LAYERED.0 as i32;
            let _ = SetWindowLongW(hwnd, GWL_EXSTYLE, next_style);
        }
        let mut current_color_key = COLORREF(0);
        let mut current_alpha: u8 = 0;
        let mut current_flags = windows::Win32::UI::WindowsAndMessaging::LAYERED_WINDOW_ATTRIBUTES_FLAGS(0);
        let has_current_alpha = GetLayeredWindowAttributes(
            hwnd,
            Some(&mut current_color_key),
            Some(&mut current_alpha),
            Some(&mut current_flags),
        )
        .is_ok()
            && (current_flags.0 & LWA_ALPHA.0) != 0;
        if !has_current_alpha || current_alpha != alpha {
            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
        }
        if let Ok(mut cache) = transparency_cache().lock() {
            cache.insert(hwnd_key, alpha);
        }
    } else if (ex_style & WS_EX_LAYERED.0 as i32) != 0 {
        let _ = SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style & !(WS_EX_LAYERED.0 as i32));
        if let Ok(mut cache) = transparency_cache().lock() {
            cache.remove(&hwnd_key);
        }
    }
    ctx.applied = ctx.applied.saturating_add(1);
    BOOL(1)
}

#[cfg(target_os = "windows")]
fn process_name_matches(target: &str, name: &str) -> bool {
    let t = normalize_process_key(target);
    let n = normalize_process_key(name);
    !t.is_empty() && t == n
}

#[cfg(target_os = "windows")]
fn apply_process_transparency(app_process: &str, alpha: Option<u8>) -> Result<u32, String> {
    use sysinfo::{ProcessesToUpdate, System};
    let target = app_process.trim();
    if target.is_empty() {
        return Ok(0);
    }
    let mut sys = System::new_all();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let target_pids: std::collections::HashSet<u32> = sys
        .processes()
        .iter()
        .filter_map(|(pid, process)| {
            let name = process.name().to_string_lossy();
            if process_name_matches(target, &name) {
                Some(pid.as_u32())
            } else {
                None
            }
        })
        .collect();

    if target_pids.is_empty() {
        return Ok(0);
    }

    let mut ctx = TransparencyApplyContext {
        target_pids,
        alpha,
        applied: 0,
    };
    unsafe {
        let _ = EnumWindows(
            Some(enum_apply_transparency),
            LPARAM((&mut ctx as *mut TransparencyApplyContext) as isize),
        );
    }
    Ok(ctx.applied)
}

#[cfg(not(target_os = "windows"))]
fn apply_process_transparency(_app_process: &str, _alpha: Option<u8>) -> Result<u32, String> {
    Ok(0)
}

#[cfg(target_os = "windows")]
fn get_running_process_names() -> Vec<String> {
    use std::collections::BTreeSet;
    use sysinfo::{ProcessesToUpdate, System};

    let mut sys = System::new_all();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let mut names = BTreeSet::new();
    for process in sys.processes().values() {
        let mut name = process.name().to_string_lossy().trim().to_ascii_lowercase();
        if name.is_empty() {
            continue;
        }
        if !name.ends_with(".exe") {
            name.push_str(".exe");
        }
        names.insert(name);
    }
    names.into_iter().collect()
}

#[cfg(not(target_os = "windows"))]
fn get_running_process_names() -> Vec<String> {
    Vec::new()
}

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
    async fn get_appearance_transparency_rules() -> Result<Vec<AppearanceTransparencyRuleDto>, String>;
    async fn get_running_process_names() -> Result<Vec<String>, String>;
    async fn export_settings_bundle_to_file(
        path: String,
        selected_groups: Vec<String>,
        theme: Option<String>,
        autostart_enabled: Option<bool>,
    ) -> Result<u32, String>;
    async fn preview_settings_bundle_from_file(path: String) -> Result<SettingsBundlePreviewDto, String>;
    async fn import_settings_bundle_from_file(
        path: String,
        selected_groups: Vec<String>,
    ) -> Result<SettingsImportResultDto, String>;
    async fn save_appearance_transparency_rule(app_process: String, opacity: u32, enabled: bool) -> Result<(), String>;
    async fn delete_appearance_transparency_rule(app_process: String) -> Result<(), String>;
    async fn apply_appearance_transparency_now(app_process: String, opacity: u32) -> Result<u32, String>;
    async fn restore_appearance_defaults() -> Result<u32, String>;
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

    async fn get_appearance_transparency_rules(self) -> Result<Vec<AppearanceTransparencyRuleDto>, String> {
        let storage = JsonFileStorageAdapter::load();
        let mut rules = load_appearance_rules(&storage);
        for rule in effective_rules_for_enforcement(rules.clone()) {
            let _ = apply_process_transparency(
                &rule.app_process,
                Some(rule.opacity.clamp(20, 255) as u8),
            );
        }
        sort_appearance_rules_deterministic(&mut rules);
        Ok(rules)
    }

    async fn get_running_process_names(self) -> Result<Vec<String>, String> {
        Ok(get_running_process_names())
    }

    async fn export_settings_bundle_to_file(
        self,
        path: String,
        selected_groups: Vec<String>,
        theme: Option<String>,
        autostart_enabled: Option<bool>,
    ) -> Result<u32, String> {
        let groups = normalized_selected_groups(&selected_groups);
        if groups.is_empty() {
            return Err("No valid settings groups selected for export.".to_string());
        }
        let guard = self.state.lock().map_err(|e| e.to_string())?;
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
                            "ghost_follower_enabled": guard.ghost_follower_enabled,
                            "ghost_follower_edge_right": guard.ghost_follower_edge_right,
                            "ghost_follower_monitor_anchor": guard.ghost_follower_monitor_anchor,
                            "ghost_follower_hover_preview": guard.ghost_follower_hover_preview,
                            "ghost_follower_collapse_delay_secs": guard.ghost_follower_collapse_delay_secs,
                            "ghost_follower_opacity": guard.ghost_follower_opacity
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
                _ => {}
            }
        }

        let payload = serde_json::json!({
            "schema_version": "1.0.0",
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
        Ok(payload["selected_groups"].as_array().map(|a| a.len() as u32).unwrap_or(0))
    }

    async fn preview_settings_bundle_from_file(
        self,
        path: String,
    ) -> Result<SettingsBundlePreviewDto, String> {
        let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let root: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        let schema = root
            .get("schema_version")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let mut warnings = Vec::new();
        let mut available_groups = Vec::new();
        let mut valid = true;

        if schema != "1.0.0" {
            valid = false;
            warnings.push(format!(
                "Unsupported schema_version '{schema}'. Expected '1.0.0'."
            ));
        }

        match root.get("groups").and_then(|v| v.as_object()) {
            Some(groups_obj) => {
                for key in groups_obj.keys() {
                    available_groups.push(key.clone());
                    if normalize_settings_group(key).is_none() {
                        warnings.push(format!("Unknown group '{key}' will be ignored."));
                    }
                }
            }
            None => {
                valid = false;
                warnings.push("Missing or invalid 'groups' object.".to_string());
            }
        }

        if warnings.is_empty() {
            diag_log(
                "info",
                format!(
                    "[SettingsImportPreview] OK path={} groups={}",
                    path,
                    available_groups.len()
                ),
            );
        } else {
            diag_log(
                "warn",
                format!(
                    "[SettingsImportPreview] path={} warnings={}",
                    path,
                    warnings.join("; ")
                ),
            );
        }

        Ok(SettingsBundlePreviewDto {
            path,
            schema_version: schema,
            available_groups,
            warnings,
            valid,
        })
    }

    async fn import_settings_bundle_from_file(
        self,
        path: String,
        selected_groups: Vec<String>,
    ) -> Result<SettingsImportResultDto, String> {
        let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let root: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        let schema = root
            .get("schema_version")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if schema != "1.0.0" {
            let msg = format!("Unsupported schema_version '{schema}'. Expected '1.0.0'.");
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
                result.warnings.push(format!("Group '{group}' not present in bundle."));
                continue;
            };
            let obj = match value.as_object() {
                Some(v) => v,
                None => {
                    result.skipped_groups.push(group.clone());
                    result.warnings.push(format!("Group '{group}' has invalid payload type."));
                    continue;
                }
            };

            match group.as_str() {
                SETTINGS_GROUP_TEMPLATES => {
                    self.clone()
                        .update_config(ConfigUpdateDto {
                            expansion_paused: None,
                            template_date_format: obj.get("template_date_format").and_then(|v| v.as_str()).map(str::to_string),
                            template_time_format: obj.get("template_time_format").and_then(|v| v.as_str()).map(str::to_string),
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
                            script_library_run_disabled: None,
                            script_library_run_allowlist: None,
                        })
                        .await?;
                    result.updated_keys = result.updated_keys.saturating_add(2);
                }
                SETTINGS_GROUP_SYNC => {
                    self.clone()
                        .update_config(ConfigUpdateDto {
                            expansion_paused: None,
                            template_date_format: None,
                            template_time_format: None,
                            sync_url: obj.get("sync_url").and_then(|v| v.as_str()).map(str::to_string),
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
                            script_library_run_disabled: None,
                            script_library_run_allowlist: None,
                        })
                        .await?;
                    result.updated_keys = result.updated_keys.saturating_add(1);
                }
                SETTINGS_GROUP_DISCOVERY | SETTINGS_GROUP_GHOST_SUGGESTOR | SETTINGS_GROUP_GHOST_FOLLOWER
                | SETTINGS_GROUP_CLIPBOARD_HISTORY | SETTINGS_GROUP_CORE | SETTINGS_GROUP_SCRIPT_RUNTIME => {
                    let cfg = ConfigUpdateDto {
                        expansion_paused: obj.get("expansion_paused").and_then(|v| v.as_bool()),
                        template_date_format: None,
                        template_time_format: None,
                        sync_url: None,
                        discovery_enabled: obj.get("discovery_enabled").and_then(|v| v.as_bool()),
                        discovery_threshold: obj.get("discovery_threshold").and_then(|v| v.as_u64()).map(|n| n as u32),
                        discovery_lookback: obj.get("discovery_lookback").and_then(|v| v.as_u64()).map(|n| n as u32),
                        discovery_min_len: obj.get("discovery_min_len").and_then(|v| v.as_u64()).map(|n| n as u32),
                        discovery_max_len: obj.get("discovery_max_len").and_then(|v| v.as_u64()).map(|n| n as u32),
                        discovery_excluded_apps: obj.get("discovery_excluded_apps").and_then(|v| v.as_str()).map(str::to_string),
                        discovery_excluded_window_titles: obj
                            .get("discovery_excluded_window_titles")
                            .and_then(|v| v.as_str())
                            .map(str::to_string),
                        ghost_suggestor_enabled: obj.get("ghost_suggestor_enabled").and_then(|v| v.as_bool()),
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
                        ghost_follower_enabled: obj.get("ghost_follower_enabled").and_then(|v| v.as_bool()),
                        ghost_follower_edge_right: obj.get("ghost_follower_edge_right").and_then(|v| v.as_bool()),
                        ghost_follower_monitor_anchor: obj
                            .get("ghost_follower_monitor_anchor")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32),
                        ghost_follower_search: None,
                        ghost_follower_hover_preview: obj.get("ghost_follower_hover_preview").and_then(|v| v.as_bool()),
                        ghost_follower_collapse_delay_secs: obj
                            .get("ghost_follower_collapse_delay_secs")
                            .and_then(|v| v.as_u64())
                            .map(|n| n as u32),
                        ghost_follower_opacity: obj
                            .get("ghost_follower_opacity")
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
                    };
                    self.clone().update_config(cfg).await?;
                    if group == SETTINGS_GROUP_CORE {
                        result.theme = obj.get("theme").and_then(|v| v.as_str()).map(str::to_string);
                        result.autostart_enabled = obj.get("autostart_enabled").and_then(|v| v.as_bool());
                    }
                    result.updated_keys = result.updated_keys.saturating_add(obj.len() as u32);
                }
                SETTINGS_GROUP_APPEARANCE => {
                    let rules_value = obj.get("rules").and_then(|v| v.as_array());
                    let Some(rules_arr) = rules_value else {
                        result.skipped_groups.push(group.clone());
                        result.warnings.push("Appearance group missing 'rules' array.".to_string());
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

        self.clone().save_settings().await?;
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

    async fn save_appearance_transparency_rule(
        self,
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

    async fn delete_appearance_transparency_rule(self, app_process: String) -> Result<(), String> {
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

    async fn apply_appearance_transparency_now(
        self,
        app_process: String,
        opacity: u32,
    ) -> Result<u32, String> {
        let alpha = opacity.clamp(20, 255) as u8;
        apply_process_transparency(&app_process, Some(alpha))
    }

    async fn restore_appearance_defaults(self) -> Result<u32, String> {
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

#[cfg(test)]
mod tests {
    use super::{
        effective_rules_for_enforcement, normalize_process_key, normalized_selected_groups,
        sort_appearance_rules_deterministic,
    };
    #[cfg(target_os = "windows")]
    use super::process_name_matches;
    use crate::AppearanceTransparencyRuleDto;

    #[cfg(target_os = "windows")]
    #[test]
    fn process_name_matches_exact_and_case_insensitive() {
        assert!(process_name_matches("cursor.exe", "Cursor.EXE"));
        assert!(process_name_matches("CURSOR.EXE", "cursor.exe"));
        assert!(process_name_matches("cursor", "cursor"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn process_name_matches_with_optional_exe_suffix() {
        assert!(process_name_matches("cursor", "cursor.exe"));
        assert!(process_name_matches("cursor.exe", "cursor"));
        assert!(!process_name_matches("cursor", "code.exe"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn process_name_matches_rejects_empty_values() {
        assert!(!process_name_matches("", "cursor.exe"));
        assert!(!process_name_matches("cursor.exe", ""));
        assert!(!process_name_matches("   ", "cursor.exe"));
    }

    #[test]
    fn normalize_process_key_handles_exe_suffix_and_case() {
        assert_eq!(normalize_process_key("Cursor.EXE"), "cursor");
        assert_eq!(normalize_process_key("cursor"), "cursor");
        assert_eq!(normalize_process_key("  Code.ExE  "), "code");
    }

    #[test]
    fn appearance_rules_sort_is_deterministic() {
        let mut rules = vec![
            AppearanceTransparencyRuleDto {
                app_process: "zeta.exe".to_string(),
                opacity: 200,
                enabled: true,
            },
            AppearanceTransparencyRuleDto {
                app_process: "cursor".to_string(),
                opacity: 180,
                enabled: false,
            },
            AppearanceTransparencyRuleDto {
                app_process: "alpha.exe".to_string(),
                opacity: 140,
                enabled: true,
            },
        ];
        sort_appearance_rules_deterministic(&mut rules);
        assert_eq!(rules[0].app_process, "alpha.exe");
        assert_eq!(rules[1].app_process, "zeta.exe");
        assert_eq!(rules[2].app_process, "cursor");
    }

    #[test]
    fn selected_groups_defaults_to_all_when_empty() {
        let groups = normalized_selected_groups(&[]);
        assert!(groups.contains(&"templates".to_string()));
        assert!(groups.contains(&"appearance".to_string()));
        assert!(groups.len() >= 9);
    }

    #[test]
    fn selected_groups_normalizes_aliases_and_deduplicates() {
        let input = vec![
            "Ghost Follower".to_string(),
            "ghost-follower".to_string(),
            "appearance".to_string(),
            "invalid".to_string(),
        ];
        let groups = normalized_selected_groups(&input);
        assert_eq!(groups, vec!["ghost_follower".to_string(), "appearance".to_string()]);
    }

    #[test]
    fn appearance_integration_save_restart_reenforce_is_deterministic() {
        let saved_rules = vec![
            AppearanceTransparencyRuleDto {
                app_process: "cursor".to_string(),
                opacity: 200,
                enabled: true,
            },
            AppearanceTransparencyRuleDto {
                app_process: "cursor.exe".to_string(),
                opacity: 120,
                enabled: true,
            },
            AppearanceTransparencyRuleDto {
                app_process: "code.exe".to_string(),
                opacity: 180,
                enabled: true,
            },
        ];

        // Simulate startup/enforcement after restart by recomputing effective rules.
        let first_start = effective_rules_for_enforcement(saved_rules.clone());
        let second_start = effective_rules_for_enforcement(saved_rules);
        assert_eq!(first_start.len(), 2);
        assert_eq!(second_start.len(), 2);
        assert_eq!(first_start[0].app_process, second_start[0].app_process);
        assert_eq!(first_start[0].opacity, second_start[0].opacity);
        assert_eq!(first_start[1].app_process, second_start[1].app_process);
        assert_eq!(first_start[1].opacity, second_start[1].opacity);
    }

    #[test]
    fn appearance_stress_many_rules_remains_stable() {
        let mut rules = Vec::new();
        for i in 0..5000u32 {
            let base = format!("app{}", i % 300);
            rules.push(AppearanceTransparencyRuleDto {
                app_process: if i % 2 == 0 { base.clone() } else { format!("{base}.exe") },
                opacity: 20 + (i % 236),
                enabled: i % 5 != 0,
            });
        }

        let first = effective_rules_for_enforcement(rules.clone());
        let second = effective_rules_for_enforcement(rules);
        assert_eq!(first.len(), second.len());
        for idx in 0..first.len() {
            assert_eq!(first[idx].app_process, second[idx].app_process);
            assert_eq!(first[idx].opacity, second[idx].opacity);
            assert!(first[idx].enabled);
        }
        assert!(first.len() <= 300);
    }
}
