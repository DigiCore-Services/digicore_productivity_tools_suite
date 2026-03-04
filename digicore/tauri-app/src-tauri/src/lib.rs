//! DigiCore Text Expander - Tauri backend.
//!
//! Invokes digicore-text-expander library. Tauri commands provide load/save/get_app_state
//! for the web frontend.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;

use crate::api::Api;
use digicore_core::domain::Snippet;
use digicore_text_expander::adapters::storage::JsonFileStorageAdapter;
use digicore_text_expander::application::app_state::AppState;
use digicore_text_expander::application::clipboard_history::{self, ClipboardHistoryConfig};
use digicore_text_expander::application::ghost_suggestor;
use digicore_text_expander::application::template_processor::{self, InteractiveVarType};
use digicore_text_expander::application::variable_input;
use digicore_text_expander::drivers::hotstring::{
    start_listener, sync_ghost_config, GhostConfig,
};
use digicore_text_expander::ports::{storage_keys, StoragePort};
use digicore_text_expander::services::sync_service::SyncResult;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::menu::{Menu, MenuItem};
use tauri::{Emitter, Manager};
use tauri_plugin_sql::{Migration, MigrationKind};

/// Serializable view of AppState for frontend. Excludes mpsc::Receiver and other non-serializable fields.
#[taurpc::ipc_type]
pub struct AppStateDto {
    pub library_path: String,
    pub library: HashMap<String, Vec<Snippet>>,
    pub categories: Vec<String>,
    pub selected_category: Option<u32>,
    pub status: String,
    pub sync_url: String,
    pub sync_status: String,
    pub expansion_paused: bool,
    pub template_date_format: String,
    pub template_time_format: String,
    pub discovery_enabled: bool,
    pub discovery_threshold: u32,
    pub discovery_lookback: u32,
    pub discovery_min_len: u32,
    pub discovery_max_len: u32,
    pub discovery_excluded_apps: String,
    pub discovery_excluded_window_titles: String,
    pub ghost_suggestor_enabled: bool,
    pub ghost_suggestor_debounce_ms: u32,
    pub ghost_suggestor_display_secs: u32,
    pub ghost_suggestor_offset_x: i32,
    pub ghost_suggestor_offset_y: i32,
    pub ghost_follower_enabled: bool,
    pub ghost_follower_edge_right: bool,
    pub ghost_follower_monitor_anchor: u32,
    pub ghost_follower_search: String,
    pub ghost_follower_hover_preview: bool,
    pub ghost_follower_collapse_delay_secs: u32,
    pub clip_history_max_depth: u32,
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
        selected_category: state.selected_category.map(|v| v as u32),
        status: state.status.clone(),
        sync_url: state.sync_url.clone(),
        sync_status: sync_result_to_string(&state.sync_status),
        expansion_paused: state.expansion_paused,
        template_date_format: state.template_date_format.clone(),
        template_time_format: state.template_time_format.clone(),
        discovery_enabled: state.discovery_enabled,
        discovery_threshold: state.discovery_threshold,
        discovery_lookback: state.discovery_lookback,
        discovery_min_len: state.discovery_min_len as u32,
        discovery_max_len: state.discovery_max_len as u32,
        discovery_excluded_apps: state.discovery_excluded_apps.clone(),
        discovery_excluded_window_titles: state.discovery_excluded_window_titles.clone(),
        ghost_suggestor_enabled: state.ghost_suggestor_enabled,
        ghost_suggestor_debounce_ms: state.ghost_suggestor_debounce_ms as u32,
        ghost_suggestor_display_secs: state.ghost_suggestor_display_secs as u32,
        ghost_suggestor_offset_x: state.ghost_suggestor_offset_x,
        ghost_suggestor_offset_y: state.ghost_suggestor_offset_y,
        ghost_follower_enabled: state.ghost_follower_enabled,
        ghost_follower_edge_right: state.ghost_follower_edge_right,
        ghost_follower_monitor_anchor: state.ghost_follower_monitor_anchor as u32,
        ghost_follower_search: state.ghost_follower_search.clone(),
        ghost_follower_hover_preview: state.ghost_follower_hover_preview,
        ghost_follower_collapse_delay_secs: state.ghost_follower_collapse_delay_secs as u32,
        clip_history_max_depth: state.clip_history_max_depth as u32,
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
    let app_handle: Arc<Mutex<Option<tauri::AppHandle>>> = Arc::new(Mutex::new(None));
    let app_handle_for_setup = app_handle.clone();
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
    let prevent_default = if cfg!(debug_assertions) {
        tauri_plugin_prevent_default::debug()
    } else {
        tauri_plugin_prevent_default::init()
    };
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_persisted_scope::init())
        .plugin(tauri_plugin_positioner::init())
        .plugin(prevent_default)
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_sql::Builder::default()
                .add_migrations(
                    "sqlite:digicore.db",
                    vec![
                        Migration {
                            version: 1,
                            description: "create_snippets_schema",
                            sql: r#"
                                CREATE TABLE IF NOT EXISTS categories (
                                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                                    name TEXT NOT NULL UNIQUE
                                );
                                CREATE TABLE IF NOT EXISTS snippets (
                                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                                    category_id INTEGER NOT NULL,
                                    trigger TEXT NOT NULL,
                                    content TEXT NOT NULL,
                                    options TEXT DEFAULT '',
                                    profile TEXT DEFAULT 'Default',
                                    app_lock TEXT DEFAULT '',
                                    pinned TEXT DEFAULT 'false',
                                    last_modified TEXT DEFAULT '',
                                    FOREIGN KEY (category_id) REFERENCES categories(id)
                                );
                                CREATE INDEX IF NOT EXISTS idx_snippets_category ON snippets(category_id);
                                CREATE INDEX IF NOT EXISTS idx_snippets_trigger ON snippets(trigger);
                            "#,
                            kind: MigrationKind::Up,
                        },
                    ],
                )
                .build(),
        )
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
                .with_shortcuts(["F11", "Shift+Alt+Space"])
                .expect("global shortcuts")
                .with_handler(|app, shortcut, _event| {
                    let s = shortcut.to_string();
                    if s.eq_ignore_ascii_case("Shift+Alt+Space") {
                        let _ = app.emit("show-command-palette", ());
                        return;
                    }
                    if s != "F11" {
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
        .setup(move |app| {
            *app_handle_for_setup.lock().unwrap() = Some(app.handle().clone());
            #[cfg(desktop)]
            let _ = app
                .handle()
                .plugin(tauri_plugin_updater::Builder::new().build());
            #[cfg(target_os = "windows")]
            {
                use tauri::window::{Effect, EffectsBuilder};
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.set_effects(EffectsBuilder::new().effect(Effect::Mica).build());
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
                let pause_i =
                    MenuItem::with_id(&handle, "pause", "Pause expansion", true, None::<&str>);
                let add_i =
                    MenuItem::with_id(&handle, "add_snippet", "Add Snippet", true, None::<&str>);
                let quit_i = MenuItem::with_id(&handle, "quit", "Quit", true, None::<&str>);
                if let (Ok(show), Ok(pause), Ok(add), Ok(quit)) = (show_i, pause_i, add_i, quit_i) {
                    let items: Vec<&dyn tauri::menu::IsMenuItem<_>> =
                        vec![&show, &pause, &add, &quit];
                    if let Ok(m) = Menu::with_items(&handle, &items) {
                        let _ = tray.set_menu(Some(m));
                        let _ = tray.set_show_menu_on_left_click(true);
                    }
                }
            }

            app.on_menu_event(move |app_handle, event| match event.id.as_ref() {
                "show" => {
                    if let Some(win) = app_handle.get_webview_window("main") {
                        let _ = win.show();
                        let _ = win.set_focus();
                        let _ = win.unminimize();
                    }
                }
                "pause" => {
                    use digicore_text_expander::application::expansion_engine::{
                        is_expansion_paused, set_expansion_paused,
                    };
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
            });

            Ok(())
        })
        .invoke_handler(taurpc::create_ipc_handler(
            api::ApiImpl {
                state: Arc::new(Mutex::new(app_state)),
                app_handle,
            }
            .into_handler(),
        ))
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// UI preferences DTO for frontend.
#[taurpc::ipc_type]
pub struct UiPrefsDto {
    pub last_tab: u32,
    pub column_order: Vec<String>,
}

/// Config update DTO. All fields optional; only provided fields are updated.
#[taurpc::ipc_type]
pub struct ConfigUpdateDto {
    pub expansion_paused: Option<bool>,
    pub template_date_format: Option<String>,
    pub template_time_format: Option<String>,
    pub sync_url: Option<String>,
    pub discovery_enabled: Option<bool>,
    pub discovery_threshold: Option<u32>,
    pub discovery_lookback: Option<u32>,
    pub discovery_min_len: Option<u32>,
    pub discovery_max_len: Option<u32>,
    pub discovery_excluded_apps: Option<String>,
    pub discovery_excluded_window_titles: Option<String>,
    pub ghost_suggestor_enabled: Option<bool>,
    pub ghost_suggestor_debounce_ms: Option<u32>,
    pub ghost_suggestor_display_secs: Option<u32>,
    pub ghost_suggestor_offset_x: Option<i32>,
    pub ghost_suggestor_offset_y: Option<i32>,
    pub ghost_follower_enabled: Option<bool>,
    pub ghost_follower_edge_right: Option<bool>,
    pub ghost_follower_monitor_anchor: Option<u32>,
    pub ghost_follower_search: Option<String>,
    pub ghost_follower_hover_preview: Option<bool>,
    pub ghost_follower_collapse_delay_secs: Option<u32>,
    pub clip_history_max_depth: Option<u32>,
    pub script_library_run_disabled: Option<bool>,
    pub script_library_run_allowlist: Option<String>,
}

#[taurpc::ipc_type]
pub struct SuggestionDto {
    pub trigger: String,
    pub content_preview: String,
    pub category: String,
}

/// Ghost Suggestor state for overlay.
#[taurpc::ipc_type]
pub struct GhostSuggestorStateDto {
    pub has_suggestions: bool,
    pub suggestions: Vec<SuggestionDto>,
    pub selected_index: u32,
    pub position: Option<(i32, i32)>,
    /// When true, enable mouse passthrough (e.g. when fading out per display_duration).
    pub should_passthrough: bool,
}

#[taurpc::ipc_type]
pub struct PinnedSnippetDto {
    pub trigger: String,
    pub content: String,
    pub content_preview: String,
    pub category: String,
    pub snippet_idx: u32,
}

#[taurpc::ipc_type]
pub struct GhostFollowerStateDto {
    pub enabled: bool,
    pub pinned: Vec<PinnedSnippetDto>,
    pub search_filter: String,
    /// Position (x, y) for edge-anchored window. None on non-Windows.
    pub position: Option<(i32, i32)>,
    /// True when edge is Right (for positioner TopRight).
    pub edge_right: bool,
    /// True when monitor is Primary (positioner works best for primary).
    pub monitor_primary: bool,
}

#[taurpc::ipc_type]
pub struct InteractiveVarDto {
    pub tag: String,
    pub label: String,
    pub var_type: String,
    pub options: Vec<String>,
}

#[taurpc::ipc_type]
pub struct PendingVariableInputDto {
    pub content: String,
    pub vars: Vec<InteractiveVarDto>,
    pub values: HashMap<String, String>,
    pub choice_indices: HashMap<String, u32>,
    pub checkbox_checked: HashMap<String, bool>,
}

/// Expansion stats DTO for Analytics dashboard.
#[taurpc::ipc_type]
pub struct ExpansionStatsDto {
    pub total_expansions: u32,
    pub total_chars_saved: u32,
    pub estimated_time_saved_secs: f64,
    pub top_triggers: Vec<(String, u32)>,
}

/// Diagnostic entry DTO for Log tab.
#[taurpc::ipc_type]
pub struct DiagnosticEntryDto {
    pub timestamp_ms: u32,
    pub level: String,
    pub message: String,
}

/// Clipboard entry DTO (Instant not serializable).
#[taurpc::ipc_type]
pub struct ClipEntryDto {
    pub content: String,
    pub process_name: String,
    pub window_title: String,
    pub length: u32,
}
