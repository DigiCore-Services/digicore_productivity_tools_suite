//! DigiCore Text Expander - Tauri backend.
//!
//! Invokes digicore-text-expander library. Tauri commands provide load/save/get_app_state
//! for the web frontend.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use digicore_core::domain::Snippet;
use digicore_text_expander::adapters::storage::JsonFileStorageAdapter;
use digicore_text_expander::application::app_state::AppState;
use digicore_text_expander::application::clipboard_history::{self, ClipboardHistoryConfig};
use digicore_text_expander::application::ghost_follower;
use digicore_text_expander::application::ghost_suggestor;
use digicore_text_expander::application::scripting::{get_scripting_config, set_global_library};
use digicore_text_expander::application::template_processor::{self, InteractiveVarType};
use digicore_text_expander::application::variable_input;
use digicore_text_expander::application::expansion_diagnostics;
use digicore_text_expander::application::expansion_stats;
use digicore_text_expander::drivers::hotstring::{
    start_listener, sync_ghost_config, update_library, GhostConfig,
};
use digicore_text_expander::platform::windows_caret;
#[cfg(target_os = "windows")]
use digicore_text_expander::platform::windows_monitor;
use digicore_text_expander::ports::{storage_keys, StoragePort};
use digicore_text_expander::services::sync_service::SyncResult;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem};
use tauri::{Emitter, Manager, State};

/// Application state held by Tauri. Wraps AppState in Mutex for thread-safe access.
pub struct TauriAppState(pub Mutex<AppState>);

/// Serializable view of AppState for frontend. Excludes mpsc::Receiver and other non-serializable fields.
#[derive(serde::Serialize)]
pub struct AppStateDto {
    pub library_path: String,
    pub library: HashMap<String, Vec<Snippet>>,
    pub categories: Vec<String>,
    pub selected_category: Option<usize>,
    pub status: String,
    pub sync_url: String,
    pub sync_status: String,
    pub expansion_paused: bool,
    pub template_date_format: String,
    pub template_time_format: String,
    pub discovery_enabled: bool,
    pub discovery_threshold: u32,
    pub discovery_lookback: u32,
    pub discovery_min_len: usize,
    pub discovery_max_len: usize,
    pub discovery_excluded_apps: String,
    pub discovery_excluded_window_titles: String,
    pub ghost_suggestor_enabled: bool,
    pub ghost_suggestor_debounce_ms: u64,
    pub ghost_suggestor_display_secs: u64,
    pub ghost_suggestor_offset_x: i32,
    pub ghost_suggestor_offset_y: i32,
    pub ghost_follower_enabled: bool,
    pub ghost_follower_edge_right: bool,
    pub ghost_follower_monitor_anchor: usize,
    pub ghost_follower_search: String,
    pub ghost_follower_hover_preview: bool,
    pub ghost_follower_collapse_delay_secs: u64,
    pub clip_history_max_depth: usize,
    pub script_library_run_disabled: bool,
    pub script_library_run_allowlist: String,
}

fn sync_result_to_string(r: &SyncResult) -> String {
    match r {
        SyncResult::Idle => "idle".to_string(),
        SyncResult::InProgress => "in_progress".to_string(),
        SyncResult::Success(msg) => format!("success:{}", msg),
        SyncResult::Error(msg) => format!("error:{}", msg),
    }
}

fn app_state_to_dto(state: &AppState) -> AppStateDto {
    AppStateDto {
        library_path: state.library_path.clone(),
        library: state.library.clone(),
        categories: state.categories.clone(),
        selected_category: state.selected_category,
        status: state.status.clone(),
        sync_url: state.sync_url.clone(),
        sync_status: sync_result_to_string(&state.sync_status),
        expansion_paused: state.expansion_paused,
        template_date_format: state.template_date_format.clone(),
        template_time_format: state.template_time_format.clone(),
        discovery_enabled: state.discovery_enabled,
        discovery_threshold: state.discovery_threshold,
        discovery_lookback: state.discovery_lookback,
        discovery_min_len: state.discovery_min_len,
        discovery_max_len: state.discovery_max_len,
        discovery_excluded_apps: state.discovery_excluded_apps.clone(),
        discovery_excluded_window_titles: state.discovery_excluded_window_titles.clone(),
        ghost_suggestor_enabled: state.ghost_suggestor_enabled,
        ghost_suggestor_debounce_ms: state.ghost_suggestor_debounce_ms,
        ghost_suggestor_display_secs: state.ghost_suggestor_display_secs,
        ghost_suggestor_offset_x: state.ghost_suggestor_offset_x,
        ghost_suggestor_offset_y: state.ghost_suggestor_offset_y,
        ghost_follower_enabled: state.ghost_follower_enabled,
        ghost_follower_edge_right: state.ghost_follower_edge_right,
        ghost_follower_monitor_anchor: state.ghost_follower_monitor_anchor,
        ghost_follower_search: state.ghost_follower_search.clone(),
        ghost_follower_hover_preview: state.ghost_follower_hover_preview,
        ghost_follower_collapse_delay_secs: state.ghost_follower_collapse_delay_secs,
        clip_history_max_depth: state.clip_history_max_depth,
        script_library_run_disabled: state.script_library_run_disabled,
        script_library_run_allowlist: state.script_library_run_allowlist.clone(),
    }
}

/// Initialize AppState from JsonFileStorageAdapter (same keys as egui).
fn init_app_state_from_storage() -> AppState {
    let storage = JsonFileStorageAdapter::load();
    let library_path = storage.get(storage_keys::LIBRARY_PATH).unwrap_or_else(|| {
        dirs::config_dir()
            .map(|p: std::path::PathBuf| p.join("DigiCore").join("text_expansion_library.json"))
            .and_then(|p| p.to_str().map(String::from))
            .unwrap_or_else(|| "text_expansion_library.json".to_string())
    });
    let sync_url = storage.get(storage_keys::SYNC_URL).unwrap_or_default();
    let template_date_format = storage
        .get(storage_keys::TEMPLATE_DATE_FORMAT)
        .unwrap_or_else(|| "%Y-%m-%d".to_string());
    let template_time_format = storage
        .get(storage_keys::TEMPLATE_TIME_FORMAT)
        .unwrap_or_else(|| "%H:%M".to_string());
    let (run_disabled, run_allowlist) = storage
        .get(storage_keys::SCRIPT_LIBRARY_RUN_DISABLED)
        .map(|v| {
            (
                v == "true",
                storage
                    .get(storage_keys::SCRIPT_LIBRARY_RUN_ALLOWLIST)
                    .unwrap_or_default(),
            )
        })
        .unwrap_or((false, String::new()));
    let ghost_suggestor_display_secs = storage
        .get(storage_keys::GHOST_SUGGESTOR_DISPLAY_SECS)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10u64);
    let clip_history_max_depth = storage
        .get(storage_keys::CLIP_HISTORY_MAX_DEPTH)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20usize)
        .clamp(5, 100);
    let expansion_paused = storage
        .get(storage_keys::EXPANSION_PAUSED)
        .map(|v| v == "true")
        .unwrap_or(false);

    let mut state = AppState::new();
    state.library_path = library_path;
    state.sync_url = sync_url;
    state.template_date_format = template_date_format;
    state.template_time_format = template_time_format;
    state.ghost_suggestor_display_secs = ghost_suggestor_display_secs;
    state.script_library_run_disabled = run_disabled;
    state.script_library_run_allowlist = run_allowlist;
    state.clip_history_max_depth = clip_history_max_depth;
    state.expansion_paused = expansion_paused;
    state
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut app_state = init_app_state_from_storage();
    if !app_state.library_path.is_empty() {
        let _ = app_state.try_load_library();
    }
    let _ = start_listener(app_state.library.clone());
    sync_ghost_config(GhostConfig {
        suggestor_enabled: app_state.ghost_suggestor_enabled,
        suggestor_debounce_ms: app_state.ghost_suggestor_debounce_ms,
        suggestor_display_secs: app_state.ghost_suggestor_display_secs,
        suggestor_offset_x: app_state.ghost_suggestor_offset_x,
        suggestor_offset_y: app_state.ghost_suggestor_offset_y,
        follower_enabled: app_state.ghost_follower_enabled,
        follower_edge_right: app_state.ghost_follower_edge_right,
        follower_monitor_anchor: app_state.ghost_follower_monitor_anchor,
        follower_search: app_state.ghost_follower_search.clone(),
        follower_hover_preview: app_state.ghost_follower_hover_preview,
        follower_collapse_delay_secs: app_state.ghost_follower_collapse_delay_secs,
    });
    clipboard_history::update_config(ClipboardHistoryConfig {
        enabled: true,
        max_depth: app_state.clip_history_max_depth,
    });
    tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.set_focus();
                let _ = win.show();
                let _ = win.unminimize();
            }
            let _ = app.emit("secondary-instance-args", args);
        }))
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_shortcuts(["F11"])
                .expect("F11 shortcut")
                .with_handler(|app, shortcut, _event| {
                    if shortcut.to_string() != "F11" {
                        return;
                    }
                    if variable_input::has_viewport_modal() {
                        return;
                    }
                    if let Some(pending) = variable_input::take_pending_expansion() {
                        let vars = template_processor::collect_interactive_vars(&pending.content);
                        if vars.is_empty() {
                            return;
                        }
                        let mut values = HashMap::new();
                        let mut choice_indices = HashMap::new();
                        let mut checkbox_checked = HashMap::new();
                        for v in &vars {
                            values.insert(v.tag.clone(), String::new());
                            if matches!(v.var_type, InteractiveVarType::Choice)
                                && !v.options.is_empty()
                            {
                                choice_indices.insert(v.tag.clone(), 0);
                                values.insert(v.tag.clone(), v.options[0].clone());
                            }
                            if matches!(v.var_type, InteractiveVarType::Checkbox) {
                                checkbox_checked.insert(v.tag.clone(), false);
                            }
                        }
                        variable_input::set_viewport_modal(variable_input::ViewportModalState {
                            content: pending.content,
                            vars,
                            values,
                            choice_indices,
                            checkbox_checked,
                            target_hwnd: pending.target_hwnd,
                            response_tx: pending.response_tx,
                        });
                        let _ = app.emit("show-variable-input", ());
                    }
                })
                .build(),
        )
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("digicore-text-expander".to_string()),
                    }),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Webview),
                ])
                .build(),
        )
        .setup(|app| {
            #[cfg(desktop)]
            let _ = app.handle().plugin(tauri_plugin_updater::Builder::new().build());
            #[cfg(target_os = "windows")]
            {
                use tauri::window::{Effect, EffectsBuilder};
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.set_effects(
                        EffectsBuilder::new()
                            .effect(Effect::Mica)
                            .build(),
                    );
                }
            }
            #[cfg(any(windows, target_os = "linux"))]
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                let _ = app.handle().deep_link().register_all();
            }
            let args: Vec<String> = std::env::args().collect();
            let _ = app.emit("initial-cli-args", args);
            let handle = app.handle().clone();
            let handle_cb = handle.clone();
            let cb = std::sync::Arc::new(move || {
                let _ = handle_cb.emit("ghost-suggestor-update", ());
            });
            ghost_suggestor::set_on_change_callback(Some(cb));

            // Tray menu: Show, Pause, Add Snippet, Quit
            if let Some(tray) = app.tray_by_id("default") {
                let show_i = MenuItem::with_id(&handle, "show", "Show", true, None::<&str>);
                let pause_i = MenuItem::with_id(&handle, "pause", "Pause expansion", true, None::<&str>);
                let add_i = MenuItem::with_id(&handle, "add_snippet", "Add Snippet", true, None::<&str>);
                let quit_i = MenuItem::with_id(&handle, "quit", "Quit", true, None::<&str>);
                if let (Ok(show), Ok(pause), Ok(add), Ok(quit)) = (show_i, pause_i, add_i, quit_i) {
                    let items: Vec<&dyn tauri::menu::IsMenuItem<_>> = vec![&show, &pause, &add, &quit];
                    if let Ok(m) = Menu::with_items(&handle, &items) {
                        let _ = tray.set_menu(Some(m));
                        let _ = tray.set_show_menu_on_left_click(true);
                    }
                }
            }

            app.on_menu_event(move |app_handle, event| {
                match event.id.as_ref() {
                    "show" => {
                        if let Some(win) = app_handle.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                            let _ = win.unminimize();
                        }
                    }
                    "pause" => {
                        use digicore_text_expander::application::expansion_engine::{is_expansion_paused, set_expansion_paused};
                        set_expansion_paused(!is_expansion_paused());
                        let _ = app_handle.emit("ghost-follower-update", ());
                    }
                    "add_snippet" => {
                        let _ = app_handle.emit("tray-add-snippet", ());
                        if let Some(win) = app_handle.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                    "quit" => {
                        app_handle.exit(0);
                    }
                    _ => {}
                }
            });

            Ok(())
        })
        .manage(TauriAppState(Mutex::new(app_state)))
        .invoke_handler(tauri::generate_handler![
            greet,
            get_app_state,
            load_library,
            save_library,
            set_library_path,
            save_settings,
            get_ui_prefs,
            save_ui_prefs,
            add_snippet,
            update_snippet,
            delete_snippet,
            update_config,
            get_clipboard_entries,
            delete_clip_entry,
            clear_clipboard_history,
            copy_to_clipboard,
            get_script_library_js,
            save_script_library_js,
            get_ghost_suggestor_state,
            ghost_suggestor_accept,
            ghost_suggestor_dismiss,
            ghost_suggestor_create_snippet,
            ghost_suggestor_cycle_forward,
            get_ghost_follower_state,
            ghost_follower_insert,
            ghost_follower_set_search,
            get_pending_variable_input,
            submit_variable_input,
            cancel_variable_input,
            get_expansion_stats,
            reset_expansion_stats,
            get_diagnostic_logs,
            clear_diagnostic_logs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! DigiCore Text Expander backend ready.", name)
}

/// Get current application state (serializable view for frontend).
#[tauri::command]
fn get_app_state(state: State<TauriAppState>) -> Result<AppStateDto, String> {
    let guard = state.0.lock().map_err(|e| e.to_string())?;
    Ok(app_state_to_dto(&guard))
}

/// Load library from disk. Uses current library_path from state.
/// Returns number of categories on success. Updates hotstring listener.
#[tauri::command]
fn load_library(state: State<TauriAppState>, app: tauri::AppHandle) -> Result<usize, String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    let count = guard.try_load_library().map_err(|e| e.to_string())?;
    update_library(guard.library.clone());
    let _ = app.emit("ghost-follower-update", ());
    Ok(count)
}

/// Save library to disk. Uses current library_path and in-memory library.
#[tauri::command]
fn save_library(state: State<TauriAppState>) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    guard.try_save_library().map_err(|e| e.to_string())
}

/// Set library path. Does not load; call load_library after.
#[tauri::command]
fn set_library_path(state: State<TauriAppState>, path: String) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    guard.library_path = path;
    Ok(())
}

/// Persist current settings (library_path, etc.) to storage. Call after set_library_path to remember on next launch.
#[tauri::command]
fn save_settings(state: State<TauriAppState>) -> Result<(), String> {
    let guard = state.0.lock().map_err(|e| e.to_string())?;
    let mut storage = JsonFileStorageAdapter::load();
    storage.set(storage_keys::LIBRARY_PATH, &guard.library_path);
    storage.set(storage_keys::SYNC_URL, &guard.sync_url);
    storage.set(
        storage_keys::TEMPLATE_DATE_FORMAT,
        &guard.template_date_format,
    );
    storage.set(
        storage_keys::TEMPLATE_TIME_FORMAT,
        &guard.template_time_format,
    );
    storage.set(
        storage_keys::SCRIPT_LIBRARY_RUN_DISABLED,
        &guard.script_library_run_disabled.to_string(),
    );
    storage.set(
        storage_keys::SCRIPT_LIBRARY_RUN_ALLOWLIST,
        &guard.script_library_run_allowlist,
    );
    storage.set(
        storage_keys::GHOST_SUGGESTOR_DISPLAY_SECS,
        &guard.ghost_suggestor_display_secs.to_string(),
    );
    storage.set(
        storage_keys::CLIP_HISTORY_MAX_DEPTH,
        &guard.clip_history_max_depth.to_string(),
    );
    storage.set(
        storage_keys::EXPANSION_PAUSED,
        &guard.expansion_paused.to_string(),
    );
    storage.persist().map_err(|e| e.to_string())
}

/// UI preferences DTO for frontend.
#[derive(serde::Serialize)]
pub struct UiPrefsDto {
    pub last_tab: usize,
    pub column_order: Vec<String>,
}

/// Get UI preferences (last tab, column order). Persisted across sessions.
#[tauri::command]
fn get_ui_prefs() -> Result<UiPrefsDto, String> {
    let storage = JsonFileStorageAdapter::load();
    let last_tab = storage
        .get(storage_keys::UI_LAST_TAB)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0usize);
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

/// Save UI preferences (last tab, column order). Persisted across sessions.
#[tauri::command]
fn save_ui_prefs(last_tab: usize, column_order: Vec<String>) -> Result<(), String> {
    let mut storage = JsonFileStorageAdapter::load();
    storage.set(storage_keys::UI_LAST_TAB, &last_tab.to_string());
    storage.set(storage_keys::UI_COLUMN_ORDER, &column_order.join(","));
    storage.persist().map_err(|e| e.to_string())
}

/// Add a snippet to the library. Call save_library to persist.
#[tauri::command]
fn add_snippet(
    state: State<TauriAppState>,
    app: tauri::AppHandle,
    category: String,
    snippet: Snippet,
) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    guard.add_snippet(&category, &snippet);
    update_library(guard.library.clone());
    let _ = app.emit("ghost-follower-update", ());
    Ok(())
}

/// Update a snippet at category and index. Call save_library to persist.
#[tauri::command]
fn update_snippet(
    state: State<TauriAppState>,
    app: tauri::AppHandle,
    category: String,
    snippet_idx: usize,
    snippet: Snippet,
) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    guard
        .update_snippet(&category, snippet_idx, &snippet)
        .map_err(|e| e.to_string())?;
    update_library(guard.library.clone());
    let _ = app.emit("ghost-follower-update", ());
    Ok(())
}

/// Delete a snippet at category and index. Call save_library to persist.
#[tauri::command]
fn delete_snippet(
    state: State<TauriAppState>,
    app: tauri::AppHandle,
    category: String,
    snippet_idx: usize,
) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    guard
        .delete_snippet(&category, snippet_idx)
        .map_err(|e| e.to_string())?;
    update_library(guard.library.clone());
    let _ = app.emit("ghost-follower-update", ());
    Ok(())
}

/// Config update DTO. All fields optional; only provided fields are updated.
#[derive(serde::Deserialize)]
pub struct ConfigUpdateDto {
    pub expansion_paused: Option<bool>,
    pub template_date_format: Option<String>,
    pub template_time_format: Option<String>,
    pub sync_url: Option<String>,
    pub discovery_enabled: Option<bool>,
    pub discovery_threshold: Option<u32>,
    pub discovery_lookback: Option<u32>,
    pub discovery_min_len: Option<usize>,
    pub discovery_max_len: Option<usize>,
    pub discovery_excluded_apps: Option<String>,
    pub discovery_excluded_window_titles: Option<String>,
    pub ghost_suggestor_enabled: Option<bool>,
    pub ghost_suggestor_debounce_ms: Option<u64>,
    pub ghost_suggestor_display_secs: Option<u64>,
    pub ghost_suggestor_offset_x: Option<i32>,
    pub ghost_suggestor_offset_y: Option<i32>,
    pub ghost_follower_enabled: Option<bool>,
    pub ghost_follower_edge_right: Option<bool>,
    pub ghost_follower_monitor_anchor: Option<usize>,
    pub ghost_follower_search: Option<String>,
    pub ghost_follower_hover_preview: Option<bool>,
    pub ghost_follower_collapse_delay_secs: Option<u64>,
    pub clip_history_max_depth: Option<usize>,
    pub script_library_run_disabled: Option<bool>,
    pub script_library_run_allowlist: Option<String>,
}

/// Update configuration. Call save_settings to persist.
#[tauri::command]
fn update_config(
    state: State<TauriAppState>,
    app: tauri::AppHandle,
    config: ConfigUpdateDto,
) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
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
        guard.discovery_min_len = v;
    }
    if let Some(v) = config.discovery_max_len {
        guard.discovery_max_len = v;
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
        guard.ghost_suggestor_debounce_ms = v;
    }
    if let Some(v) = config.ghost_suggestor_display_secs {
        guard.ghost_suggestor_display_secs = v;
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
        guard.ghost_follower_monitor_anchor = v;
    }
    if let Some(ref v) = config.ghost_follower_search {
        guard.ghost_follower_search = v.clone();
    }
    if let Some(v) = config.ghost_follower_hover_preview {
        guard.ghost_follower_hover_preview = v;
    }
    if let Some(v) = config.ghost_follower_collapse_delay_secs {
        guard.ghost_follower_collapse_delay_secs = v;
    }
    if let Some(v) = config.clip_history_max_depth {
        let depth = v.clamp(5, 100);
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
    sync_ghost_config(GhostConfig {
        suggestor_enabled: guard.ghost_suggestor_enabled,
        suggestor_debounce_ms: guard.ghost_suggestor_debounce_ms,
        suggestor_display_secs: guard.ghost_suggestor_display_secs,
        suggestor_offset_x: guard.ghost_suggestor_offset_x,
        suggestor_offset_y: guard.ghost_suggestor_offset_y,
        follower_enabled: guard.ghost_follower_enabled,
        follower_edge_right: guard.ghost_follower_edge_right,
        follower_monitor_anchor: guard.ghost_follower_monitor_anchor,
        follower_search: guard.ghost_follower_search.clone(),
        follower_hover_preview: guard.ghost_follower_hover_preview,
        follower_collapse_delay_secs: guard.ghost_follower_collapse_delay_secs,
    });
    let _ = app.emit("ghost-follower-update", ());
    Ok(())
}

#[derive(serde::Serialize)]
pub struct SuggestionDto {
    pub trigger: String,
    pub content_preview: String,
    pub category: String,
}

/// Ghost Suggestor state for overlay.
#[derive(serde::Serialize)]
pub struct GhostSuggestorStateDto {
    pub has_suggestions: bool,
    pub suggestions: Vec<SuggestionDto>,
    pub selected_index: usize,
    pub position: Option<(i32, i32)>,
}

#[tauri::command]
fn get_ghost_suggestor_state() -> Result<GhostSuggestorStateDto, String> {
    let suggestions = ghost_suggestor::get_suggestions();
    let selected = ghost_suggestor::get_selected_index();
    #[cfg(target_os = "windows")]
    let position = {
        let pos = windows_caret::get_caret_screen_position();
        let cfg = ghost_suggestor::get_config();
        pos.map(|(x, y)| (x + cfg.offset_x, y + cfg.offset_y))
    };
    #[cfg(not(target_os = "windows"))]
    let position: Option<(i32, i32)> = None;
    Ok(GhostSuggestorStateDto {
        has_suggestions: !suggestions.is_empty(),
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
        selected_index: selected,
        position,
    })
}

#[tauri::command]
fn ghost_suggestor_accept() -> Result<Option<(String, String)>, String> {
    Ok(ghost_suggestor::accept_selected())
}

#[tauri::command]
fn ghost_suggestor_dismiss() -> Result<(), String> {
    ghost_suggestor::dismiss();
    Ok(())
}

#[tauri::command]
fn ghost_suggestor_create_snippet() -> Result<Option<(String, String)>, String> {
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

#[tauri::command]
fn ghost_suggestor_cycle_forward() -> Result<usize, String> {
    Ok(ghost_suggestor::cycle_selection_forward())
}

#[derive(serde::Serialize)]
pub struct PinnedSnippetDto {
    pub trigger: String,
    pub content: String,
    pub content_preview: String,
    pub category: String,
}

#[derive(serde::Serialize)]
pub struct GhostFollowerStateDto {
    pub enabled: bool,
    pub pinned: Vec<PinnedSnippetDto>,
    pub search_filter: String,
    /// Position (x, y) for edge-anchored window. None on non-Windows.
    pub position: Option<(i32, i32)>,
}

#[tauri::command]
fn get_ghost_follower_state(
    search_filter: Option<String>,
) -> Result<GhostFollowerStateDto, String> {
    let filter = search_filter.as_deref().unwrap_or("");
    let pinned = ghost_follower::get_pinned_snippets(filter);
    let cfg = ghost_follower::get_config();
    let enabled = ghost_follower::is_enabled();

    #[cfg(target_os = "windows")]
    let position = {
        let work = match cfg.monitor_anchor {
            ghost_follower::MonitorAnchor::Primary => {
                windows_monitor::get_primary_monitor_work_area()
            }
            ghost_follower::MonitorAnchor::Secondary => {
                windows_monitor::get_secondary_monitor_work_area()
                    .unwrap_or_else(windows_monitor::get_primary_monitor_work_area)
            }
            ghost_follower::MonitorAnchor::Current => {
                windows_monitor::get_current_monitor_work_area()
            }
        };
        let (x, _y) = match cfg.edge {
            ghost_follower::FollowerEdge::Right => (work.right - 280, work.top + 20),
            ghost_follower::FollowerEdge::Left => (work.left, work.top + 20),
        };
        Some((x, work.top + 20))
    };
    #[cfg(not(target_os = "windows"))]
    let position: Option<(i32, i32)> = None;

    Ok(GhostFollowerStateDto {
        enabled,
        pinned: pinned
            .into_iter()
            .map(|(s, cat)| PinnedSnippetDto {
                trigger: s.trigger.clone(),
                content: s.content.clone(),
                content_preview: if s.content.len() > 40 {
                    format!("{}...", &s.content[..40])
                } else {
                    s.content.clone()
                },
                category: cat,
            })
            .collect(),
        search_filter: ghost_follower::get_search_filter(),
        position,
    })
}

#[tauri::command]
fn ghost_follower_insert(_trigger: String, content: String) -> Result<(), String> {
    digicore_text_expander::drivers::hotstring::request_expansion(content);
    Ok(())
}

#[tauri::command]
fn ghost_follower_set_search(filter: String, app: tauri::AppHandle) -> Result<(), String> {
    ghost_follower::set_search_filter(&filter);
    let _ = app.emit("ghost-follower-update", ());
    Ok(())
}

#[derive(serde::Serialize)]
pub struct InteractiveVarDto {
    pub tag: String,
    pub label: String,
    pub var_type: String,
    pub options: Vec<String>,
}

#[derive(serde::Serialize)]
pub struct PendingVariableInputDto {
    pub content: String,
    pub vars: Vec<InteractiveVarDto>,
    pub values: HashMap<String, String>,
    pub choice_indices: HashMap<String, usize>,
    pub checkbox_checked: HashMap<String, bool>,
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

#[tauri::command]
fn get_pending_variable_input() -> Result<Option<PendingVariableInputDto>, String> {
    if let Some((content, vars, values, choice_indices, checkbox_checked)) =
        variable_input::get_viewport_modal_display()
    {
        Ok(Some(PendingVariableInputDto {
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
            choice_indices,
            checkbox_checked,
        }))
    } else {
        Ok(None)
    }
}

#[tauri::command]
fn submit_variable_input(values: HashMap<String, String>) -> Result<(), String> {
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

#[tauri::command]
fn cancel_variable_input() -> Result<(), String> {
    if let Some(state) = variable_input::take_viewport_modal() {
        if let Some(ref tx) = state.response_tx {
            let _ = tx.send((None, None));
        }
    }
    Ok(())
}

/// Expansion stats DTO for Analytics dashboard.
#[derive(serde::Serialize)]
pub struct ExpansionStatsDto {
    pub total_expansions: u64,
    pub total_chars_saved: u64,
    pub estimated_time_saved_secs: f64,
    pub top_triggers: Vec<(String, u64)>,
}

/// Get expansion statistics for Analytics dashboard.
#[tauri::command]
fn get_expansion_stats() -> Result<ExpansionStatsDto, String> {
    let stats = expansion_stats::get_stats();
    Ok(ExpansionStatsDto {
        total_expansions: stats.total_expansions,
        total_chars_saved: stats.total_chars_saved,
        estimated_time_saved_secs: stats.estimated_time_saved_secs(),
        top_triggers: stats.top_triggers(10),
    })
}

/// Reset expansion statistics.
#[tauri::command]
fn reset_expansion_stats() -> Result<(), String> {
    expansion_stats::reset_stats();
    Ok(())
}

/// Diagnostic entry DTO for Log tab.
#[derive(serde::Serialize)]
pub struct DiagnosticEntryDto {
    pub timestamp_ms: u64,
    pub level: String,
    pub message: String,
}

/// Get expansion diagnostic logs for Log tab.
#[tauri::command]
fn get_diagnostic_logs() -> Result<Vec<DiagnosticEntryDto>, String> {
    let entries = expansion_diagnostics::get_recent();
    Ok(entries
        .into_iter()
        .map(|e| DiagnosticEntryDto {
            timestamp_ms: e.timestamp_ms,
            level: e.level,
            message: e.message,
        })
        .collect())
}

/// Clear expansion diagnostic logs.
#[tauri::command]
fn clear_diagnostic_logs() -> Result<(), String> {
    expansion_diagnostics::clear();
    Ok(())
}

/// Clipboard entry DTO (Instant not serializable).
#[derive(serde::Serialize)]
pub struct ClipEntryDto {
    pub content: String,
    pub process_name: String,
    pub window_title: String,
    pub length: usize,
}

/// Get clipboard history entries (most recent first).
#[tauri::command]
fn get_clipboard_entries() -> Result<Vec<ClipEntryDto>, String> {
    let entries = clipboard_history::get_entries();
    Ok(entries
        .into_iter()
        .map(|e| ClipEntryDto {
            content: e.content.clone(),
            process_name: e.process_name,
            window_title: e.window_title,
            length: e.content.len(),
        })
        .collect())
}

/// Delete clipboard entry at index.
#[tauri::command]
fn delete_clip_entry(index: usize) -> Result<(), String> {
    clipboard_history::delete_entry_at(index);
    Ok(())
}

/// Clear all clipboard history.
#[tauri::command]
fn clear_clipboard_history() -> Result<(), String> {
    clipboard_history::clear_all();
    Ok(())
}

/// Copy text to system clipboard.
#[tauri::command]
fn copy_to_clipboard(text: String) -> Result<(), String> {
    arboard::Clipboard::new()
        .map_err(|e| e.to_string())?
        .set_text(&text)
        .map_err(|e| e.to_string())
}

/// Get global JavaScript library content.
#[tauri::command]
fn get_script_library_js() -> Result<String, String> {
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
 * Define reusable JavaScript functions for use in any {js:...} tag.
 */
function greet(name) { return "Hello, " + name + "!"; }
function getTimeGreeting() {
  var hour = new Date().getHours();
  if (hour < 12) return "Good Morning";
  if (hour < 18) return "Good Afternoon";
  return "Good Evening";
}
"#
        .to_string()
    }))
}

/// Save global JavaScript library and reload.
#[tauri::command]
fn save_script_library_js(content: String) -> Result<(), String> {
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
