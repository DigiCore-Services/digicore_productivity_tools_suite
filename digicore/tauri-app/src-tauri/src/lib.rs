//! DigiCore Text Expander - Tauri backend.
//!
//! Invokes digicore-text-expander library. Tauri commands provide load/save/get_app_state
//! for the web frontend.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod clipboard_repository;

use crate::api::{save_all_on_exit, Api};
use digicore_core::domain::Snippet;
use digicore_text_expander::adapters::storage::JsonFileStorageAdapter;
use digicore_text_expander::application::app_state::AppState;
use digicore_text_expander::application::clipboard_history::{self, ClipboardHistoryConfig};
use digicore_text_expander::application::expansion_engine::set_expansion_paused;
use digicore_text_expander::application::ghost_suggestor;
use digicore_text_expander::application::scripting::load_and_apply_script_libraries;
use digicore_text_expander::application::template_processor::{self, InteractiveVarType};
use digicore_text_expander::application::variable_input;
use digicore_text_expander::application::discovery;
use digicore_text_expander::drivers::hotstring::{
    start_listener, sync_discovery_config, sync_ghost_config, GhostConfig,
};
use digicore_text_expander::ports::{storage_keys, StoragePort};
use digicore_text_expander::services::sync_service::SyncResult;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::menu::{Menu, MenuItem};
use tauri::{Emitter, Listener, Manager};
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
    pub ghost_suggestor_snooze_duration_mins: u32,
    pub ghost_suggestor_offset_x: i32,
    pub ghost_suggestor_offset_y: i32,
    pub ghost_follower_enabled: bool,
    pub ghost_follower_edge_right: bool,
    pub ghost_follower_monitor_anchor: u32,
    pub ghost_follower_search: String,
    pub ghost_follower_hover_preview: bool,
    pub ghost_follower_collapse_delay_secs: u32,
    pub ghost_follower_opacity: u32,
    pub clip_history_max_depth: u32,
    pub script_library_run_disabled: bool,
    pub script_library_run_allowlist: String,
}

fn parse_comma_list(s: &str) -> Vec<String> {
    s.split(',')
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
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
        ghost_suggestor_snooze_duration_mins: state.ghost_suggestor_snooze_duration_mins as u32,
        ghost_suggestor_offset_x: state.ghost_suggestor_offset_x,
        ghost_suggestor_offset_y: state.ghost_suggestor_offset_y,
        ghost_follower_enabled: state.ghost_follower_enabled,
        ghost_follower_edge_right: state.ghost_follower_edge_right,
        ghost_follower_monitor_anchor: state.ghost_follower_monitor_anchor as u32,
        ghost_follower_search: state.ghost_follower_search.clone(),
        ghost_follower_hover_preview: state.ghost_follower_hover_preview,
        ghost_follower_collapse_delay_secs: state.ghost_follower_collapse_delay_secs as u32,
        ghost_follower_opacity: state.ghost_follower_opacity,
        clip_history_max_depth: state.clip_history_max_depth as u32,
        script_library_run_disabled: state.script_library_run_disabled,
        script_library_run_allowlist: state.script_library_run_allowlist.clone(),
    }
}

fn tray_pause_menu_label(paused: bool) -> &'static str {
    if paused {
        "Toggle Unpause - Paused"
    } else {
        "Toggle Pause - Running"
    }
}

fn install_tray_menu(handle: &tauri::AppHandle, paused: bool) {
    if let Some(tray) = handle.tray_by_id("default") {
        let console_i =
            MenuItem::with_id(handle, "view_console", "View Management Console", true, None::<&str>);
        let quick_search_i = MenuItem::with_id(
            handle,
            "quick_search",
            "Display Quick Search (Shift+Alt+Space)",
            true,
            None::<&str>,
        );
        let toggle_pause_i =
            MenuItem::with_id(handle, "toggle_pause", tray_pause_menu_label(paused), true, None::<&str>);
        let follower_i =
            MenuItem::with_id(handle, "view_follower", "Display Ghost Follower", true, None::<&str>);
        let quit_i = MenuItem::with_id(handle, "quit", "Exit application", true, None::<&str>);
        if let (Ok(console), Ok(quick_search), Ok(toggle_pause), Ok(follower), Ok(quit)) =
            (console_i, quick_search_i, toggle_pause_i, follower_i, quit_i)
        {
            let items: Vec<&dyn tauri::menu::IsMenuItem<_>> =
                vec![&console, &quick_search, &toggle_pause, &follower, &quit];
            if let Ok(m) = Menu::with_items(handle, &items) {
                let _ = tray.set_menu(Some(m));
                let _ = tray.set_show_menu_on_left_click(false);
            }
        }
    }
}

/// Initialize AppState from JsonFileStorageAdapter (same keys as egui).
fn init_app_state_from_storage() -> AppState {
    let storage = JsonFileStorageAdapter::load();
    let library_path = storage.get(storage_keys::LIBRARY_PATH).unwrap_or_else(|| {
        digicore_text_expander::ports::data_path_resolver::DataPathResolver::script_library_path()
            .to_string_lossy()
            .to_string()
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
    let ghost_suggestor_snooze_duration_mins = storage
        .get(storage_keys::GHOST_SUGGESTOR_SNOOZE_DURATION_MINS)
        .and_then(|s| s.parse().ok())
        .unwrap_or(5u64)
        .clamp(1, 120);
    let clip_history_max_depth = storage
        .get(storage_keys::CLIP_HISTORY_MAX_DEPTH)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20usize);
    let ghost_follower_enabled = storage
        .get(storage_keys::GHOST_FOLLOWER_ENABLED)
        .map(|v| v == "true")
        .unwrap_or(true);
    let ghost_follower_edge_right = storage
        .get(storage_keys::GHOST_FOLLOWER_EDGE_RIGHT)
        .map(|v| v == "true")
        .unwrap_or(true);
    let ghost_follower_monitor_anchor = storage
        .get(storage_keys::GHOST_FOLLOWER_MONITOR_ANCHOR)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0u32)
        .min(2);
    let ghost_follower_hover_preview = storage
        .get(storage_keys::GHOST_FOLLOWER_HOVER_PREVIEW)
        .map(|v| v == "true")
        .unwrap_or(true);
    let ghost_follower_collapse_delay_secs = storage
        .get(storage_keys::GHOST_FOLLOWER_COLLAPSE_DELAY_SECS)
        .and_then(|s| s.parse().ok())
        .unwrap_or(5u64)
        .min(60);
    let ghost_follower_opacity = storage
        .get(storage_keys::GHOST_FOLLOWER_OPACITY)
        .and_then(|s| s.parse().ok())
        .unwrap_or(100u32)
        .clamp(10, 100);
    let ghost_follower_position = storage
        .get(storage_keys::GHOST_FOLLOWER_POSITION_X)
        .and_then(|sx| sx.parse().ok())
        .and_then(|x: i32| {
            storage
                .get(storage_keys::GHOST_FOLLOWER_POSITION_Y)
                .and_then(|sy| sy.parse().ok())
                .map(|y: i32| (x, y))
        });
    let expansion_paused = storage
        .get(storage_keys::EXPANSION_PAUSED)
        .map(|v| v == "true")
        .unwrap_or(false);
    let ghost_suggestor_enabled = storage
        .get(storage_keys::GHOST_SUGGESTOR_ENABLED)
        .map(|v| v == "true")
        .unwrap_or(true);
    let ghost_suggestor_debounce_ms = storage
        .get(storage_keys::GHOST_SUGGESTOR_DEBOUNCE_MS)
        .and_then(|s| s.parse().ok())
        .unwrap_or(50u64);
    let ghost_suggestor_offset_x = storage
        .get(storage_keys::GHOST_SUGGESTOR_OFFSET_X)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let ghost_suggestor_offset_y = storage
        .get(storage_keys::GHOST_SUGGESTOR_OFFSET_Y)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);
    let discovery_enabled = storage
        .get(storage_keys::DISCOVERY_ENABLED)
        .map(|v| v == "true")
        .unwrap_or(false);
    let discovery_threshold = storage
        .get(storage_keys::DISCOVERY_THRESHOLD)
        .and_then(|s| s.parse().ok())
        .unwrap_or(2u32);
    let discovery_lookback = storage
        .get(storage_keys::DISCOVERY_LOOKBACK)
        .and_then(|s| s.parse().ok())
        .unwrap_or(60u32);
    let discovery_min_len = storage
        .get(storage_keys::DISCOVERY_MIN_LEN)
        .and_then(|s| s.parse().ok())
        .unwrap_or(3usize);
    let discovery_max_len = storage
        .get(storage_keys::DISCOVERY_MAX_LEN)
        .and_then(|s| s.parse().ok())
        .unwrap_or(50usize);
    let discovery_excluded_apps = storage
        .get(storage_keys::DISCOVERY_EXCLUDED_APPS)
        .unwrap_or_default();
    let discovery_excluded_window_titles = storage
        .get(storage_keys::DISCOVERY_EXCLUDED_WINDOW_TITLES)
        .unwrap_or_default();

    let mut state = AppState::new();
    state.library_path = library_path;
    state.sync_url = sync_url;
    state.template_date_format = template_date_format;
    state.template_time_format = template_time_format;
    state.ghost_suggestor_display_secs = ghost_suggestor_display_secs;
    state.ghost_suggestor_snooze_duration_mins = ghost_suggestor_snooze_duration_mins;
    state.script_library_run_disabled = run_disabled;
    state.script_library_run_allowlist = run_allowlist;
    state.clip_history_max_depth = clip_history_max_depth;
    state.ghost_follower_enabled = ghost_follower_enabled;
    state.ghost_follower_edge_right = ghost_follower_edge_right;
    state.ghost_follower_monitor_anchor = ghost_follower_monitor_anchor as usize;
    state.ghost_follower_hover_preview = ghost_follower_hover_preview;
    state.ghost_follower_collapse_delay_secs = ghost_follower_collapse_delay_secs;
    state.ghost_follower_opacity = ghost_follower_opacity;
    state.ghost_follower_position = ghost_follower_position;
    state.expansion_paused = expansion_paused;
    state.ghost_suggestor_enabled = ghost_suggestor_enabled;
    state.ghost_suggestor_debounce_ms = ghost_suggestor_debounce_ms;
    state.ghost_suggestor_offset_x = ghost_suggestor_offset_x;
    state.ghost_suggestor_offset_y = ghost_suggestor_offset_y;
    state.discovery_enabled = discovery_enabled;
    state.discovery_threshold = discovery_threshold;
    state.discovery_lookback = discovery_lookback;
    state.discovery_min_len = discovery_min_len;
    state.discovery_max_len = discovery_max_len;
    state.discovery_excluded_apps = discovery_excluded_apps;
    state.discovery_excluded_window_titles = discovery_excluded_window_titles;
    state
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    std::panic::set_hook(Box::new(|info| {
        log::error!("[PANIC] Process panicked: {:?}", info);
    }));
    let mut app_state = init_app_state_from_storage();
    // Ensure {js:...} has global library functions available at startup.
    load_and_apply_script_libraries();
    let app_handle: Arc<Mutex<Option<tauri::AppHandle>>> = Arc::new(Mutex::new(None));
    let app_handle_for_setup = app_handle.clone();
    if !app_state.library_path.is_empty() {
        let _ = app_state.try_load_library();
    }
    if let Err(e) = clipboard_repository::init(clipboard_repository::default_db_path()) {
        log::error!("[ClipboardHistory][SQLite] initialization failed: {}", e);
    } else {
        digicore_text_expander::application::clipboard_history::set_entry_observer(Some(Arc::new(
            move |entry| {
                if entry.content == "[Image]" {
                    crate::api::sync_current_clipboard_image_to_sqlite(entry.process_name.clone(), entry.window_title.clone());
                    return;
                }
                match crate::api::persist_clipboard_entry_with_settings(
                    &entry.content,
                    &entry.process_name,
                    &entry.window_title,
                ) {
                    Ok(true) => {
                        digicore_text_expander::application::expansion_diagnostics::push(
                            "info",
                            format!(
                                "[Clipboard][capture.accepted] app='{}' chars={}",
                                entry.process_name,
                                entry.content.chars().count()
                            ),
                        );
                    }
                    Ok(false) => {
                        digicore_text_expander::application::expansion_diagnostics::push(
                            "warn",
                            format!(
                                "[Clipboard][capture.skipped] app='{}'",
                                entry.process_name
                            ),
                        );
                    }
                    Err(err) => {
                        log::warn!("[ClipboardHistory][SQLite] insert failed: {}", err);
                        digicore_text_expander::application::expansion_diagnostics::push(
                            "error",
                            format!("[Clipboard][persistence.write_err] {}", err),
                        );
                    }
                }
            },
        )));
    }
    let _ = start_listener(app_state.library.clone());
    // Initial sync to persist seeded clipboard entry from startup
    crate::api::sync_runtime_clipboard_entries_to_sqlite();
    set_expansion_paused(app_state.expansion_paused);
    sync_ghost_config(GhostConfig {
        suggestor_enabled: app_state.ghost_suggestor_enabled,
        suggestor_debounce_ms: app_state.ghost_suggestor_debounce_ms,
        suggestor_display_secs: app_state.ghost_suggestor_display_secs,
        suggestor_snooze_duration_mins: app_state.ghost_suggestor_snooze_duration_mins,
        suggestor_offset_x: app_state.ghost_suggestor_offset_x,
        suggestor_offset_y: app_state.ghost_suggestor_offset_y,
        follower_enabled: app_state.ghost_follower_enabled,
        follower_edge_right: app_state.ghost_follower_edge_right,
        follower_monitor_anchor: app_state.ghost_follower_monitor_anchor,
        follower_search: app_state.ghost_follower_search.clone(),
        follower_hover_preview: app_state.ghost_follower_hover_preview,
        follower_collapse_delay_secs: app_state.ghost_follower_collapse_delay_secs,
    });
    let storage_for_clip = JsonFileStorageAdapter::load();
    let copy_enabled = storage_for_clip
        .get(storage_keys::COPY_TO_CLIPBOARD_ENABLED)
        .map(|v| v == "true")
        .unwrap_or(true);
    clipboard_history::update_config(ClipboardHistoryConfig {
        enabled: copy_enabled,
        max_depth: if app_state.clip_history_max_depth == 0 {
            usize::MAX
        } else {
            app_state.clip_history_max_depth
        },
    });
    log::info!(
        "[Startup] sync_discovery_config: enabled={} threshold={}",
        app_state.discovery_enabled,
        app_state.discovery_threshold
    );
    sync_discovery_config(
        app_state.discovery_enabled,
        discovery::DiscoveryConfig {
            threshold: app_state.discovery_threshold,
            lookback_minutes: app_state.discovery_lookback,
            min_phrase_len: app_state.discovery_min_len,
            max_phrase_len: app_state.discovery_max_len,
            excluded_apps: parse_comma_list(&app_state.discovery_excluded_apps),
            excluded_window_titles: parse_comma_list(&app_state.discovery_excluded_window_titles),
        },
    );
    let app_state = Arc::new(Mutex::new(app_state));
    let state_for_exit = app_state.clone();
    let state_for_tray = app_state.clone();
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
                        Migration {
                            version: 2,
                            description: "create_clipboard_history_schema",
                            sql: r#"
                                CREATE TABLE IF NOT EXISTS clipboard_history (
                                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                                    content TEXT NOT NULL,
                                    process_name TEXT NOT NULL DEFAULT '',
                                    window_title TEXT NOT NULL DEFAULT '',
                                    char_count INTEGER NOT NULL DEFAULT 0,
                                    word_count INTEGER NOT NULL DEFAULT 0,
                                    content_hash TEXT NOT NULL DEFAULT '',
                                    created_at_unix_ms INTEGER NOT NULL
                                );
                                CREATE INDEX IF NOT EXISTS idx_clipboard_history_created_at
                                    ON clipboard_history(created_at_unix_ms DESC);
                                CREATE INDEX IF NOT EXISTS idx_clipboard_history_content_hash
                                    ON clipboard_history(content_hash);
                            "#,
                            kind: MigrationKind::Up,
                        },
                        Migration {
                            version: 3,
                            description: "clipboard_history_add_image_support",
                            sql: r#"
                                ALTER TABLE clipboard_history ADD COLUMN entry_type TEXT NOT NULL DEFAULT 'text';
                                ALTER TABLE clipboard_history ADD COLUMN mime_type TEXT;
                                ALTER TABLE clipboard_history ADD COLUMN image_path TEXT;
                                ALTER TABLE clipboard_history ADD COLUMN thumb_path TEXT;
                                ALTER TABLE clipboard_history ADD COLUMN image_width INTEGER;
                                ALTER TABLE clipboard_history ADD COLUMN image_height INTEGER;
                                ALTER TABLE clipboard_history ADD COLUMN image_bytes INTEGER;
                            "#,
                            kind: MigrationKind::Up,
                        },
                    ],
                )
                .build(),
        )
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.emit("window-closed-to-tray", ());
                    let _ = window.hide();
                }
            }
        })
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
                        digicore_text_expander::application::ghost_follower::capture_target_window_for_quick_search_launch();
                        let _ = app.emit("show-quick-search", ());
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
            std::thread::spawn(|| {
                loop {
                    let result = std::panic::catch_unwind(|| {
                        crate::api::enforce_appearance_transparency_rules();
                    });
                    if let Err(e) = result {
                        log::error!("[BackgroundThread] Transparency enforcement panicked: {:?}", e);
                    }
                    std::thread::sleep(std::time::Duration::from_secs(3));
                }
            });
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
                log::info!("[GhostSuggestor] on_change: emitting ghost-suggestor-update");
                let _ = handle_cb.emit("ghost-suggestor-update", ());
            });
            ghost_suggestor::set_on_change_callback(Some(cb));

            // Discovery: use notification toast instead of overlay. Set callback to emit for frontend.
            let handle_discovery = app.handle().clone();
            discovery::set_suggestion_callback(move |phrase, count| {
                ghost_suggestor::set_pending_discovery_for_notification(phrase.to_string(), count);
                let _ = handle_discovery.emit("discovery-suggestion", (phrase.to_string(), count));
            });

            // Create Ghost Follower, Suggestor, and Quick Search windows from Rust (avoids URL/path issues).
            // Use a thread to avoid Windows deadlocks when creating windows.
            let handle_for_ghost = app.handle().clone();
            std::thread::spawn(move || {
                use tauri::WebviewUrl;
                let _ = (|| -> Result<(), Box<dyn std::error::Error>> {
                    let suggestor = tauri::WebviewWindowBuilder::new(
                        &handle_for_ghost,
                        "ghost-suggestor",
                        WebviewUrl::App("ghost-suggestor.html".into()),
                    )
                    .title("Ghost Suggestor")
                    .inner_size(320.0, 260.0)
                    .decorations(true)
                    .transparent(false)
                    .always_on_top(true)
                    .visible(false)
                    .build()?;
                    log::info!("[GhostSuggestor] window created from Rust");
                    let handle_suggestor = handle_for_ghost.clone();
                    suggestor.once("tauri://created", move |_| {
                        log::info!("[GhostSuggestor] webview ready, can receive ghost-suggestor-update events");
                        let _ = handle_suggestor.emit("ghost-suggestor-update", ());
                    });

                    let follower = tauri::WebviewWindowBuilder::new(
                        &handle_for_ghost,
                        "ghost-follower",
                        WebviewUrl::App("ghost-follower.html".into()),
                    )
                    .title("Ghost Follower")
                    .inner_size(64.0, 36.0)
                    .decorations(false)
                    .resizable(false)
                    .maximizable(false)
                    .minimizable(false)
                    .shadow(false)
                    .transparent(true)
                    .always_on_top(true)
                    .visible(true)
                    .build()?;

                    // Defensive enforcement: keep Ghost Follower borderless even if platform/window-state
                    // attempts to restore framed styles from prior sessions.
                    let _ = follower.set_decorations(false);
                    let _ = follower.set_resizable(false);
                    let _ = follower.set_maximizable(false);
                    let _ = follower.set_minimizable(false);

                    log::info!("[GhostFollower] window created from Rust");
                    let handle_emit = handle_for_ghost.clone();
                    follower.once("tauri://created", move |_| {
                        log::info!("[GhostFollower] webview ready, emitting update");
                        let _ = handle_emit.emit("ghost-follower-update", ());
                    });

                    let quick_search = tauri::WebviewWindowBuilder::new(
                        &handle_for_ghost,
                        "quick-search",
                        WebviewUrl::App("quick-search.html".into()),
                    )
                    .title("Quick Search")
                    .inner_size(520.0, 540.0)
                    .decorations(false)
                    .transparent(false)
                    .always_on_top(true)
                    .shadow(true)
                    .visible(false)
                    .build()?;

                    let _ = quick_search.set_resizable(false);
                    let _ = quick_search.set_maximizable(false);
                    let _ = quick_search.set_minimizable(false);
                    let _ = quick_search.hide();
                    log::info!("[QuickSearch] window created from Rust");
                    let handle_quick = handle_for_ghost.clone();
                    quick_search.once("tauri://created", move |_| {
                        if let Some(win) = handle_quick.get_webview_window("quick-search") {
                            let _ = win.hide();
                        }
                        let _ = handle_quick.emit("quick-search-refresh", ());
                    });

                    Ok(())
                })();
            });

            // Tray menu: View Management Console, Quick Search, Toggle Pause/Unpause, View Ghost Follower, Exit
            let paused = state_for_tray
                .lock()
                .map(|g| g.expansion_paused)
                .unwrap_or(false);
            install_tray_menu(&handle, paused);

            let tray_state_for_events = state_for_tray.clone();
            app.on_menu_event(move |app_handle, event| match event.id.as_ref() {
                "view_console" => {
                    if let Some(win) = app_handle.get_webview_window("main") {
                        let _ = win.show();
                        let _ = win.set_focus();
                        let _ = win.unminimize();
                    }
                }
                "quick_search" => {
                    digicore_text_expander::application::ghost_follower::capture_target_window_for_quick_search_launch();
                    if let Some(win) = app_handle.get_webview_window("quick-search") {
                        let _ = win.show();
                        let _ = win.set_focus();
                        let _ = win.unminimize();
                    }
                    let _ = app_handle.emit("quick-search-refresh", ());
                }
                "toggle_pause" => {
                    let paused = if let Ok(mut guard) = tray_state_for_events.lock() {
                        guard.expansion_paused = !guard.expansion_paused;
                        guard.expansion_paused
                    } else {
                        false
                    };
                    set_expansion_paused(paused);
                    if let Err(e) = crate::api::persist_settings_for_state(&tray_state_for_events) {
                        log::warn!("[Tray] persist pause toggle failed: {}", e);
                    }
                    install_tray_menu(app_handle, paused);
                    let _ = app_handle.emit(
                        "tray-expansion-paused-changed",
                        serde_json::json!({ "paused": paused }),
                    );
                }
                "view_follower" => {
                    if let Some(win) = app_handle.get_webview_window("ghost-follower") {
                        let _ = win.show();
                        let _ = win.set_focus();
                        let _ = win.unminimize();
                    }
                }
                "quit" => {
                    app_handle.exit(0);
                }
                _ => {}
            });

            app.listen("show-quick-search", move |_event| {
                digicore_text_expander::application::ghost_follower::capture_target_window_for_quick_search_launch();
                if let Some(win) = handle.get_webview_window("quick-search") {
                    let _ = win.show();
                    let _ = win.set_focus();
                    let _ = win.unminimize();
                }
                let _ = handle.emit("quick-search-refresh", ());
            });

            Ok(())
        })
        .invoke_handler(taurpc::create_ipc_handler(
            api::ApiImpl {
                state: app_state,
                app_handle,
            }
            .into_handler(),
        ))
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(move |_app_handle, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                save_all_on_exit(&state_for_exit);
            }
        });
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
    pub ghost_suggestor_snooze_duration_mins: Option<u32>,
    pub ghost_suggestor_offset_x: Option<i32>,
    pub ghost_suggestor_offset_y: Option<i32>,
    pub ghost_follower_enabled: Option<bool>,
    pub ghost_follower_edge_right: Option<bool>,
    pub ghost_follower_monitor_anchor: Option<u32>,
    pub ghost_follower_search: Option<String>,
    pub ghost_follower_hover_preview: Option<bool>,
    pub ghost_follower_collapse_delay_secs: Option<u32>,
    pub ghost_follower_opacity: Option<u32>,
    pub clip_history_max_depth: Option<u32>,
    pub script_library_run_disabled: Option<bool>,
    pub script_library_run_allowlist: Option<String>,
}

#[taurpc::ipc_type]
pub struct CopyToClipboardConfigDto {
    pub enabled: bool,
    pub min_log_length: u32,
    pub mask_cc: bool,
    pub mask_ssn: bool,
    pub mask_email: bool,
    pub blacklist_processes: String,
    pub max_history_entries: u32,
    pub json_output_enabled: bool,
    pub json_output_dir: String,
    pub image_storage_dir: String,
}

#[taurpc::ipc_type]
pub struct CopyToClipboardStatsDto {
    pub total_entries: u32,
}

#[taurpc::ipc_type]
pub struct ScriptingHttpConfigDto {
    pub timeout_secs: u32,
    pub retry_count: u32,
    pub retry_delay_ms: u32,
    pub use_async: bool,
}

#[taurpc::ipc_type]
pub struct ScriptingPyConfigDto {
    pub enabled: bool,
    pub path: String,
    pub library_path: String,
}

#[taurpc::ipc_type]
pub struct ScriptingLuaConfigDto {
    pub enabled: bool,
    pub path: String,
    pub library_path: String,
}

#[taurpc::ipc_type]
pub struct ScriptingDslConfigDto {
    pub enabled: bool,
}

#[taurpc::ipc_type]
pub struct ScriptingEngineConfigDto {
    pub dsl: ScriptingDslConfigDto,
    pub http: ScriptingHttpConfigDto,
    pub py: ScriptingPyConfigDto,
    pub lua: ScriptingLuaConfigDto,
}

#[taurpc::ipc_type]
pub struct AppearanceTransparencyRuleDto {
    pub app_process: String,
    pub opacity: u32,
    pub enabled: bool,
}

#[taurpc::ipc_type]
pub struct SettingsImportResultDto {
    pub applied_groups: Vec<String>,
    pub skipped_groups: Vec<String>,
    pub warnings: Vec<String>,
    pub updated_keys: u32,
    pub appearance_rules_applied: u32,
    pub theme: Option<String>,
    pub autostart_enabled: Option<bool>,
}

#[taurpc::ipc_type]
pub struct SettingsBundlePreviewDto {
    pub path: String,
    pub schema_version: String,
    pub available_groups: Vec<String>,
    pub warnings: Vec<String>,
    pub valid: bool,
}

#[taurpc::ipc_type]
pub struct ScriptingProfilePreviewDto {
    pub path: String,
    pub schema_version: String,
    pub available_groups: Vec<String>,
    pub warnings: Vec<String>,
    pub valid: bool,
    pub signed_bundle: bool,
    pub signature_valid: bool,
    pub migrated_from_schema: Option<String>,
    pub signature_key_id: Option<String>,
    pub signer_fingerprint: Option<String>,
    pub signer_trusted: bool,
}

#[taurpc::ipc_type]
pub struct ScriptingProfileImportResultDto {
    pub applied_groups: Vec<String>,
    pub skipped_groups: Vec<String>,
    pub warnings: Vec<String>,
    pub updated_keys: u32,
    pub schema_version_used: String,
    pub signature_valid: bool,
    pub migrated_from_schema: Option<String>,
    pub signer_fingerprint: Option<String>,
    pub signer_trusted: bool,
}

#[taurpc::ipc_type]
pub struct ScriptingProfileDiffEntryDto {
    pub group: String,
    pub field: String,
    pub current_value: String,
    pub incoming_value: String,
}

#[taurpc::ipc_type]
pub struct ScriptingProfileDryRunDto {
    pub path: String,
    pub selected_groups: Vec<String>,
    pub changed_groups: Vec<String>,
    pub estimated_updates: u32,
    pub warnings: Vec<String>,
    pub diff_entries: Vec<ScriptingProfileDiffEntryDto>,
    pub schema_version_used: String,
    pub signature_valid: bool,
    pub migrated_from_schema: Option<String>,
    pub signer_fingerprint: Option<String>,
    pub signer_trusted: bool,
}

#[taurpc::ipc_type]
pub struct ScriptingSignerRegistryDto {
    pub allow_unknown_signers: bool,
    pub trust_on_first_use: bool,
    pub trusted_fingerprints: Vec<String>,
    pub blocked_fingerprints: Vec<String>,
}

#[taurpc::ipc_type]
pub struct ScriptingDetachedSignatureExportDto {
    pub profile_path: String,
    pub signature_path: String,
    pub key_id: String,
    pub signer_fingerprint: String,
    pub payload_sha256: String,
}

#[cfg(test)]
mod tests {
    use super::tray_pause_menu_label;

    #[test]
    fn tray_pause_label_reflects_state() {
        assert_eq!(tray_pause_menu_label(false), "Toggle Pause - Running");
        assert_eq!(tray_pause_menu_label(true), "Toggle Unpause - Paused");
    }

    #[test]
    fn ipc_dtos_do_not_use_u64_or_u128_fields() {
        let src = include_str!("lib.rs");
        let lines: Vec<&str> = src.lines().collect();
        let mut i = 0usize;
        while i < lines.len() {
            if !lines[i].contains("#[taurpc::ipc_type]") {
                i += 1;
                continue;
            }

            // Move to struct declaration line.
            i += 1;
            while i < lines.len() && !lines[i].contains("pub struct ") {
                i += 1;
            }
            if i >= lines.len() {
                break;
            }
            let struct_line = lines[i].trim();
            let struct_name = struct_line
                .strip_prefix("pub struct ")
                .and_then(|s| s.split_whitespace().next())
                .unwrap_or("<unknown>");

            // Parse struct body by brace depth.
            let mut depth = 0i32;
            let mut started = false;
            while i < lines.len() {
                let line = lines[i];
                for ch in line.chars() {
                    if ch == '{' {
                        depth += 1;
                        started = true;
                    } else if ch == '}' {
                        depth -= 1;
                    }
                }

                if started {
                    let trimmed = line.trim();
                    if trimmed.starts_with("pub ")
                        && (trimmed.contains(": u64")
                            || trimmed.contains(": u128")
                            || trimmed.contains("<u64>")
                            || trimmed.contains("<u128>"))
                    {
                        panic!(
                            "taurpc ipc dto '{}' must not use u64/u128 field types: {}",
                            struct_name, trimmed
                        );
                    }
                    if depth == 0 {
                        i += 1;
                        break;
                    }
                }
                i += 1;
            }
        }
    }

    #[test]
    fn clip_history_depth_supports_unlimited_guard() {
        let lib_src = include_str!("lib.rs");
        assert!(
            !lib_src.contains(".clamp(5, 5000)"),
            "clip_history_max_depth startup should allow 0=unlimited and high values"
        );
    }
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
    /// Configured max clipboard history depth (e.g. 100, 20).
    pub clip_history_max_depth: u32,
    /// True when ribbon should auto-collapse (no activity for collapse_delay_secs).
    pub should_collapse: bool,
    /// Seconds of inactivity before auto-collapsing. 0 = disabled.
    pub collapse_delay_secs: u32,
    /// Opacity 0.0-1.0 (10-100% from config).
    pub opacity: f64,
    /// True when position is from user drag (saved); use position directly, skip positioner.
    pub saved_position: bool,
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

#[taurpc::ipc_type]
pub struct SnippetLogicTestResultDto {
    pub result: String,
    pub requires_input: bool,
    pub vars: Vec<InteractiveVarDto>,
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
    pub id: u32,
    pub content: String,
    pub process_name: String,
    pub window_title: String,
    pub length: u32,
    pub word_count: u32,
    pub created_at: String,
    pub entry_type: String,
    pub mime_type: Option<String>,
    pub image_path: Option<String>,
    pub thumb_path: Option<String>,
    pub image_width: Option<u32>,
    pub image_height: Option<u32>,
    pub image_bytes: Option<u32>,
}
