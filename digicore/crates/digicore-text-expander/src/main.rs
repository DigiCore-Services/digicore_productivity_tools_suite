//! Text Expander - DigiCore Services.
//!
//! Cross-platform text expansion with egui management console.
//! UI modules follow SRP (one tab per module); orchestration in App.
//! Phase 0/1: StoragePort, WindowPort, AppState for framework-agnostic UI.
//! Supports --gui=egui|tauri for dual/multi-GUI foundation.

mod ui;

use digicore_text_expander::cli::{parse_gui_from_args, GuiBackend};
use digicore_text_expander::adapters::{EframeStorageAdapter, EguiWindowAdapter, RfdFileDialogAdapter, EguiTimerAdapter};
// Re-export for ui modules; use for main
pub use digicore_text_expander::application::app_state::{AppState, ClipViewContent, SnippetEditorMode, SnippetTestVarState, Tab};
use digicore_text_expander::ports::{storage_keys, FileDialogPort, TimerPort, ViewportCommand, WindowLevel, WindowPort, StoragePort};
use digicore_core::domain::ports::WindowContextPort;
use digicore_core::domain::{LastModified, Snippet};
use digicore_text_expander::application::discovery;
#[cfg(target_os = "windows")]
use digicore_text_expander::platform::windows_caret;
use digicore_text_expander::application::clipboard_history;
use digicore_text_expander::application::ghost_follower::{self, FollowerEdge, GhostFollowerConfig, MonitorAnchor};
use digicore_text_expander::application::ghost_suggestor;
use digicore_text_expander::application::scripting::{
    get_scripting_config, load_and_apply_script_libraries, load_scripting_config, set_scripting_config,
};
use digicore_text_expander::application::template_processor::{self, InteractiveVarType, TemplateConfig};
use digicore_text_expander::application::variable_input;
use digicore_text_expander::drivers::hotstring::{is_listener_running, request_expansion, start_listener, update_library};
use digicore_text_expander::services::sync_service::{pull_sync, push_sync, SyncResult};
use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc;
use std::sync::Arc;

fn main() -> eframe::Result<()> {
    match parse_gui_from_args() {
        GuiBackend::Egui => run_egui(),
        GuiBackend::Tauri => run_tauri(),
    }
}

fn run_egui() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 600.0])
            .with_min_inner_size([600.0, 400.0])
            .with_visible(true)
            .with_taskbar(true)
            .with_minimize_button(true)
            .with_maximize_button(true),
        ..Default::default()
    };
    eframe::run_native(
        "DigiCore Text Expander",
        options,
        Box::new(|cc| Ok(Box::new(TextExpanderApp::new(cc)))),
    )
}

/// Tauri GUI entry point. Tauri app runs via tauri-app/.
fn run_tauri() -> eframe::Result<()> {
    eprintln!("Tauri GUI: use tauri-app. Run: cd digicore/tauri-app && npm run tauri dev");
    std::process::exit(1);
}

/// True if the foreground window belongs to our app (avoids Ghost Suggestor on hover over our UI).
#[cfg(target_os = "windows")]
fn is_foreground_our_app() -> bool {
    let our_exe = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    let Some(ref exe) = our_exe else { return false };
    let foreground = digicore_core::adapters::platform::window::WindowsWindowAdapter::new()
        .get_active()
        .ok();
    let Some(ctx) = foreground else { return false };
    let proc = ctx.process_name.to_lowercase();
    proc == *exe || proc.contains("digicore-text-expander")
}

#[cfg(not(target_os = "windows"))]
fn is_foreground_our_app() -> bool {
    false
}

/// Main application - thin UI binding over AppState (Phase 0/1).
/// Holds framework-specific storage and file dialog adapters.
struct TextExpanderApp {
    /// Framework-agnostic application state.
    state: AppState,
    /// Framework-specific persistence. EframeStorageAdapter for egui.
    storage: EframeStorageAdapter,
    /// File dialog for Load/Save/Browse. Arc allows sharing without borrowing app.
    file_dialog: Arc<RfdFileDialogAdapter>,
}

impl std::ops::Deref for TextExpanderApp {
    type Target = AppState;
    fn deref(&self) -> &AppState {
        &self.state
    }
}

impl std::ops::DerefMut for TextExpanderApp {
    fn deref_mut(&mut self) -> &mut AppState {
        &mut self.state
    }
}

/// Built-in snippet templates (Phase 8; AHK parity).
pub const SNIPPET_TEMPLATES: &[(&str, &str)] = &[
    ("(none)", ""),
    ("Email signature", "Best regards,\n{env:USERNAME}\n{env:USEREMAIL}"),
    ("Code block", "```\n\n```"),
    ("Date block", "{date}"),
    ("Bullet list", "- Item 1\n- Item 2\n- Item 3"),
    ("Numbered list", "1. First\n2. Second\n3. Third"),
    ("Blockquote", "> Quote text here"),
    ("Markdown header", "# Header 1\n\n## Header 2\n\n### Header 3"),
    ("Placeholder", "[TODO: describe]"),
];

impl TextExpanderApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let storage = EframeStorageAdapter::load_from(cc.storage);
        let library_path = storage
            .get(storage_keys::LIBRARY_PATH)
            .unwrap_or_else(|| {
                dirs::config_dir()
                    .map(|p| p.join("DigiCore").join("text_expansion_library.json"))
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
            .map(|v| (v == "true", storage.get(storage_keys::SCRIPT_LIBRARY_RUN_ALLOWLIST).unwrap_or_default()))
            .unwrap_or_else(|| {
                let cfg = load_scripting_config();
                (cfg.run.disabled, cfg.run.allowlist)
            });
        let ghost_suggestor_display_secs = storage
            .get(storage_keys::GHOST_SUGGESTOR_DISPLAY_SECS)
            .and_then(|s| s.parse().ok())
            .unwrap_or(10u64);
        {
            let mut cfg = get_scripting_config();
            cfg.run.disabled = run_disabled;
            cfg.run.allowlist = run_allowlist.clone();
            set_scripting_config(cfg);
        }

        let mut state = AppState::new();
        state.library_path = library_path.clone();
        state.sync_url = sync_url;
        state.template_date_format = template_date_format;
        state.template_time_format = template_time_format;
        state.ghost_suggestor_display_secs = ghost_suggestor_display_secs;
        state.script_library_run_disabled = run_disabled;
        state.script_library_run_allowlist = run_allowlist;

        Self {
            state,
            storage,
            file_dialog: Arc::new(RfdFileDialogAdapter::new()),
        }
    }

    fn file_dialog(&self) -> Arc<dyn FileDialogPort> {
        self.file_dialog.clone()
    }
}

impl eframe::App for TextExpanderApp {
    fn save(&mut self, eframe_storage: &mut dyn eframe::Storage) {
        let library_path = self.library_path.clone();
        let sync_url = self.sync_url.clone();
        let template_date_format = self.template_date_format.clone();
        let template_time_format = self.template_time_format.clone();
        let script_library_run_disabled = self.script_library_run_disabled.to_string();
        let script_library_run_allowlist = self.script_library_run_allowlist.clone();
        let ghost_suggestor_display_secs = self.ghost_suggestor_display_secs.to_string();
        self.storage.set(storage_keys::LIBRARY_PATH, &library_path);
        self.storage.set(storage_keys::SYNC_URL, &sync_url);
        self.storage.set(storage_keys::TEMPLATE_DATE_FORMAT, &template_date_format);
        self.storage.set(storage_keys::TEMPLATE_TIME_FORMAT, &template_time_format);
        self.storage.set(storage_keys::SCRIPT_LIBRARY_RUN_DISABLED, &script_library_run_disabled);
        self.storage.set(storage_keys::SCRIPT_LIBRARY_RUN_ALLOWLIST, &script_library_run_allowlist);
        self.storage.set(storage_keys::GHOST_SUGGESTOR_DISPLAY_SECS, &ghost_suggestor_display_secs);
        self.storage.save_to_eframe(eframe_storage);
        // SE-23: Persist run config to scripting.json for template processor
        let mut cfg = get_scripting_config();
        cfg.run.disabled = self.script_library_run_disabled;
        cfg.run.allowlist = self.script_library_run_allowlist.clone();
        set_scripting_config(cfg);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Ensure main window is visible and maximized on first frame (override persistence-restored minimized state)
        if !self.window_visibility_ensured {
            self.window_visibility_ensured = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
        }

        // Auto-load library on startup (once) so hotstring listener starts
        if !self.initial_load_attempted {
            self.initial_load_attempted = true;
            self.sync_template_config();
            // Load global JS library from config path (SE-17)
            load_and_apply_script_libraries();
            if !self.library_path.is_empty() {
                match self.try_load_library() {
                    Ok(n) => self.status = format!("Loaded {} categories", n),
                    Err(e) => self.status = format!("Load failed: {}", e),
                }
            }
        }

        // F44: Tick Ghost Suggestor debounce (recompute suggestions when timer elapsed)
        let _ = ghost_suggestor::tick_debounce();

        // F41: Promote to snippet from Ghost Follower - open Edit Snippet modal (Promote mode)
        if let Some(content) = clipboard_history::take_promote_pending() {
            let cat = self
                .categories
                .first()
                .cloned()
                .unwrap_or_else(|| "General".to_string());
            let trigger: String = content
                .chars()
                .take(20)
                .filter(|c| !c.is_whitespace())
                .collect();
            let trigger = if trigger.is_empty() {
                "clip".to_string()
            } else {
                trigger
            };
            self.snippet_editor_mode = Some(SnippetEditorMode::Promote {
                category: cat.clone(),
            });
            self.snippet_editor_trigger = trigger;
            self.snippet_editor_content = content;
            self.snippet_editor_options = "*:".to_string();
            self.snippet_editor_category = cat;
            self.snippet_editor_profile = "Work".to_string();
            self.snippet_editor_app_lock.clear();
            self.snippet_editor_pinned = false;
            self.snippet_editor_template_selected = 0;
            self.snippet_editor_modal_open = true;
        }

        // F11: Check for pending expansion with interactive vars (from hotstring or Ghost Follower)
        if !variable_input::has_viewport_modal() {
            if let Some(pending) = variable_input::take_pending_expansion() {
                let vars = template_processor::collect_interactive_vars(&pending.content);
                let mut values = HashMap::new();
                let mut choice_indices = HashMap::new();
                for v in &vars {
                    values.insert(v.tag.clone(), String::new());
                    if let InteractiveVarType::Choice = v.var_type {
                        choice_indices.insert(v.tag.clone(), 0);
                    }
                }
                variable_input::set_viewport_modal(variable_input::ViewportModalState {
                    content: pending.content,
                    vars,
                    values,
                    choice_indices,
                    checkbox_checked: HashMap::new(),
                    target_hwnd: pending.target_hwnd,
                    response_tx: pending.response_tx,
                });
            }
        }

        // Check for discovery suggestion (F65 toast)
        if let Some((phrase, count)) = discovery::take_suggestion() {
            self.status = format!("Discovery: Add \"{}\" as snippet? (typed {}x)", phrase, count);
        }

        // F48-F59: Ghost Follower ribbon - show when enabled.
        if ghost_follower::is_enabled() {
            ghost_follower::set_search_filter(&self.ghost_follower_search);
            let filter = ghost_follower::get_search_filter();
            let pinned = ghost_follower::get_pinned_snippets(&filter);
            let clips = ghost_follower::get_clipboard_entries();
            let cfg = ghost_follower::get_config();
            let delay = cfg.collapse_delay_secs;
            if ghost_follower::should_collapse(delay) && !ghost_follower::is_collapsed() {
                ghost_follower::set_collapsed(true);
            }
            let collapsed = ghost_follower::is_collapsed();
            let edge_right = self.ghost_follower_edge_right;
            let viewport_id = egui::ViewportId::from_hash_of("ghost_follower_ribbon");
            let (width, height) = if collapsed { (50.0, 50.0) } else { (220.0, 400.0) };
            let mut builder = egui::ViewportBuilder::default()
                .with_title("Ghost Follower")
                .with_inner_size([width, height])
                .with_decorations(!collapsed)
                .with_always_on_top()
                .with_window_level(egui::WindowLevel::AlwaysOnTop)
                .with_taskbar(false);
            let (pos_x, pos_y) = {
                #[cfg(target_os = "windows")]
                {
                    use digicore_text_expander::platform::windows_monitor;
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
                    let y = work.top as f32 + 50.0;
                    let x = if edge_right {
                        (work.right as f32) - width
                    } else {
                        work.left as f32
                    };
                    (x, y)
                }
                #[cfg(not(target_os = "windows"))]
                {
                    let rect = ctx.available_rect();
                    let y = 50.0;
                    let x = if edge_right {
                        rect.max.x - width
                    } else {
                        0.0
                    };
                    (x, y)
                }
            };
            builder = builder.with_position(egui::pos2(pos_x, pos_y));
            let pinned_with_idx: Vec<(Snippet, String, usize)> = pinned;
            let clips_with_idx: Vec<(usize, clipboard_history::ClipEntry)> = clips
                .iter()
                .enumerate()
                .map(|(i, e)| (i, e.clone()))
                .collect();
            let hover_preview = cfg.hover_preview;
            let ensure_visible = !self.ghost_follower_visibility_ensured;
            let app_ptr = self as *mut TextExpanderApp;
            ctx.show_viewport_immediate(viewport_id, builder, move |ctx, _class| {
                if ensure_visible {
                    let window = EguiWindowAdapter::new(ctx);
                    window.send_viewport_command("ghost_follower_ribbon", ViewportCommand::WindowLevel(WindowLevel::AlwaysOnTop));
                    window.send_viewport_command("ghost_follower_ribbon", ViewportCommand::Visible(true));
                    window.send_viewport_command("ghost_follower_ribbon", ViewportCommand::Minimized(false));
                }
                if !collapsed && ctx.input(|i| i.pointer.hover_pos().is_some()) {
                    ghost_follower::touch();
                }
                if collapsed && !ctx.input(|i| i.pointer.any_down() || i.pointer.hover_pos().is_some()) {
                    let timer = EguiTimerAdapter::new(ctx);
                    timer.schedule_repaint_after(std::time::Duration::from_millis(200));
                }
                egui::CentralPanel::default().show(ctx, |ui| {
                    if collapsed {
                        if ui.button("TE").clicked() {
                            ghost_follower::touch();
                            ghost_follower::set_collapsed(false);
                        }
                        ui.label("Click to expand");
                    } else {
                        ui.heading("Pinned + Clipboard");
                        ui.label("Double-click insert, right-click for menu");
                        ui.separator();
                        let mut search = ghost_follower::get_search_filter();
                        if ui.text_edit_singleline(&mut search).changed() {
                            ghost_follower::set_search_filter(&search);
                            ghost_follower::touch();
                        }
                        ui.separator();
                        ui.collapsing("Pinned Snippets", |ui| {
                            egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
                                for (snip, cat, snippet_idx) in &pinned_with_idx {
                                    let content_preview = if snip.content.len() > 30 {
                                        format!("{}...", &snip.content[..30])
                                    } else {
                                        snip.content.clone()
                                    };
                                    let label = format!("[{}] {}", snip.trigger, content_preview);
                                    let row_response = if hover_preview {
                                        ui.selectable_label(false, label)
                                            .on_hover_ui(|ui| { ui.label(snip.content.replace('\n', "\n")); })
                                    } else {
                                        ui.selectable_label(false, label)
                                    };
                                    if row_response.double_clicked() {
                                        ghost_follower::touch();
                                        request_expansion(snip.content.clone());
                                    }
                                    if row_response.hovered() {
                                        ghost_follower::touch();
                                    }
                                    let row_id = egui::Id::new(("gf_pinned", cat, snippet_idx));
                                    let response = ui.interact(
                                        row_response.rect,
                                        row_id,
                                        egui::Sense::click(),
                                    );
                                    response.context_menu(|ui| {
                                        let app = unsafe { &mut *app_ptr };
                                        let snip = snip.clone();
                                        let cat = cat.clone();
                                        let snippet_idx = *snippet_idx;
                                        let content = snip.content.clone();
                                        let trigger = snip.trigger.clone();
                                        let options = snip.options.clone();
                                        let snip_category = snip.category.clone();
                                        let profile = snip.profile.clone();
                                        let app_lock = snip.app_lock.clone();
                                        let is_pinned = snip.is_pinned();
                                        if ui.button("View Full Snippet Content").clicked() {
                                            app.clip_view_content = Some(ClipViewContent::SnippetLibrary {
                                                category: cat.clone(),
                                                snippet_idx,
                                                trigger: trigger.clone(),
                                                content: content.clone(),
                                                options: options.clone(),
                                                snippet_category: snip_category.clone(),
                                                profile: profile.clone(),
                                                app_lock: app_lock.clone(),
                                                pinned: is_pinned,
                                            });
                                            ui.close_menu();
                                        }
                                        if ui
                                            .button(if is_pinned { "Unpin Snippet" } else { "Pin Snippet" })
                                            .clicked()
                                        {
                                            if let Some(snippets) = app.library.get_mut(&cat) {
                                                if let Some(s) = snippets.get_mut(snippet_idx) {
                                                    s.pinned = if is_pinned { "false".to_string() } else { "true".to_string() };
                                                    app.sync_hotstring_listener();
                                                    let _ = app.try_save_library();
                                                    app.status = if is_pinned { "Snippet unpinned".to_string() } else { "Snippet pinned".to_string() };
                                                }
                                            }
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        if ui.button("Edit Snippet").clicked() {
                                            app.snippet_editor_mode = Some(SnippetEditorMode::Edit { category: cat.clone(), snippet_idx });
                                            app.snippet_editor_trigger = trigger;
                                            app.snippet_editor_content = content.clone();
                                            app.snippet_editor_category = snip_category;
                                            app.snippet_editor_profile = profile;
                                            app.snippet_editor_app_lock = app_lock;
                                            app.snippet_editor_pinned = is_pinned;
                                            app.snippet_editor_modal_open = true;
                                            ui.close_menu();
                                        }
                                        if ui.button("Preview Snippet").clicked() {
                                            let vars = template_processor::collect_interactive_vars(&content);
                                            if vars.is_empty() {
                                                let current_clip = arboard::Clipboard::new().and_then(|mut c| c.get_text()).ok();
                                                let clip_history: Vec<String> = clipboard_history::get_entries().iter().map(|e| e.content.clone()).collect();
                                                let result = template_processor::process_for_preview(&content, current_clip.as_deref(), &clip_history, None);
                                                app.snippet_test_result = Some(result);
                                                app.snippet_test_result_modal_open = true;
                                            } else {
                                                let mut values = std::collections::HashMap::new();
                                                let mut choice_indices = std::collections::HashMap::new();
                                                for v in &vars {
                                                    values.insert(v.tag.clone(), String::new());
                                                    if let template_processor::InteractiveVarType::Choice = v.var_type {
                                                        choice_indices.insert(v.tag.clone(), 0);
                                                    }
                                                }
                                                app.snippet_test_var_pending = Some(SnippetTestVarState {
                                                    content: content.clone(),
                                                    vars,
                                                    values,
                                                    choice_indices,
                                                    checkbox_checked: std::collections::HashMap::new(),
                                                });
                                                app.snippet_test_var_modal_open = true;
                                            }
                                            ui.close_menu();
                                        }
                                        if ui.button("Copy Full Content to Clipboard").clicked() {
                                            if let Ok(mut clip) = arboard::Clipboard::new() {
                                                if clip.set_text(&content).is_ok() {
                                                    app.status = "Copied snippet content to clipboard!".to_string();
                                                }
                                            }
                                            ui.close_menu();
                                        }
                                        if ui.button("Delete Snippet").clicked() {
                                            app.snippet_delete_confirm = Some((cat, snippet_idx));
                                            app.snippet_delete_dialog_open = true;
                                            ui.close_menu();
                                        }
                                    });
                                }
                                if pinned_with_idx.is_empty() {
                                    ui.label("No pinned snippets.");
                                }
                            });
                        });
                        ui.collapsing("Clipboard History", |ui| {
                            egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
                                for (index, entry) in &clips_with_idx {
                                    let content_preview = if entry.content.len() > 30 {
                                        format!("{}...", &entry.content[..30])
                                    } else {
                                        entry.content.clone()
                                    };
                                    let label = content_preview.replace('\n', " ");
                                    let row_response = if hover_preview {
                                        ui.selectable_label(false, label)
                                            .on_hover_ui(|ui| { ui.label(entry.content.replace('\n', "\n")); })
                                    } else {
                                        ui.selectable_label(false, label)
                                    };
                                    if row_response.double_clicked() {
                                        ghost_follower::touch();
                                        request_expansion(entry.content.clone());
                                    }
                                    if row_response.hovered() {
                                        ghost_follower::touch();
                                    }
                                    let row_id = egui::Id::new(("gf_clip", index));
                                    let response = ui.interact(
                                        row_response.rect,
                                        row_id,
                                        egui::Sense::click(),
                                    );
                                    response.context_menu(|ui| {
                                        let app = unsafe { &mut *app_ptr };
                                        let content = entry.content.clone();
                                        let index = *index;
                                        let num = index + 1;
                                        if ui.button("Copy to Clipboard").clicked() {
                                            if let Ok(mut clip) = arboard::Clipboard::new() {
                                                if clip.set_text(&content).is_ok() {
                                                    app.status = format!("Copied item #{} to clipboard!", num);
                                                }
                                            }
                                            ui.close_menu();
                                        }
                                        if ui.button("View Full Content").clicked() {
                                            app.clip_view_content = Some(ClipViewContent::ClipboardHistory { content: content.clone() });
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        if ui.button("Delete Item").clicked() {
                                            app.clip_delete_confirm = Some(index);
                                            app.clip_delete_dialog_open = true;
                                            ui.close_menu();
                                        }
                                        if ui.button("Promote to Snippet").clicked() {
                                            clipboard_history::request_promote(content.clone());
                                            let cat = app.categories.first().cloned().unwrap_or_else(|| "General".to_string());
                                            let trigger: String = content.chars().take(20).filter(|c| !c.is_whitespace()).collect();
                                            let trigger = if trigger.is_empty() { "clip".to_string() } else { trigger };
                                            app.snippet_editor_mode = Some(SnippetEditorMode::Promote { category: cat.clone() });
                                            app.snippet_editor_trigger = trigger;
                                            app.snippet_editor_content = content;
                                            app.snippet_editor_options = "*:".to_string();
                                            app.snippet_editor_category = cat;
                                            app.snippet_editor_profile = "Work".to_string();
                                            app.snippet_editor_template_selected = 0;
                                            app.snippet_editor_app_lock.clear();
                                            app.snippet_editor_pinned = false;
                                            app.snippet_editor_modal_open = true;
                                            app.status = "Promote to snippet - set trigger and save.".to_string();
                                            ui.close_menu();
                                        }
                                        if ui.button("Clear All History").clicked() {
                                            app.clip_clear_confirm_open = true;
                                            ui.close_menu();
                                        }
                                    });
                                }
                                if clips_with_idx.is_empty() {
                                    ui.label("No clipboard history.");
                                }
                            });
                        });
                    }
                });
            });
            if !self.ghost_follower_visibility_ensured {
                self.ghost_follower_visibility_ensured = true;
            }
            self.ghost_follower_search = ghost_follower::get_search_filter();
        }

        // F43-F47: Ghost Suggestor overlay - show when suggestions exist (F46: caret-based position).
        // Do not show when our app is foreground (avoids overlay appearing on hover over tabs).
        // AHK parity: AlwaysOnTop, configurable display duration, Create/Ignore/Cancel buttons.
        let show_ghost = ghost_suggestor::is_enabled()
            && ghost_suggestor::has_suggestions()
            && !is_foreground_our_app();
        let window = EguiWindowAdapter::new(ctx);
        if !show_ghost {
            window.close_viewport("ghost_suggestor_overlay");
        }
        if show_ghost {
            if ghost_suggestor::should_auto_hide() {
                ghost_suggestor::dismiss();
            } else {
                ghost_suggestor::set_overlay_shown();
                ctx.request_repaint();
                let suggestions = ghost_suggestor::get_suggestions();
                let selected = ghost_suggestor::get_selected_index();
                let viewport_id = egui::ViewportId::from_hash_of("ghost_suggestor_overlay");
                let mut builder = egui::ViewportBuilder::default()
                    .with_title("Ghost Suggestor")
                    .with_inner_size([320.0, 260.0])
                    .with_decorations(true)
                    .with_always_on_top()
                    .with_window_level(egui::WindowLevel::AlwaysOnTop);
                #[cfg(target_os = "windows")]
                {
                    if let Some((cx, cy)) = windows_caret::get_caret_screen_position() {
                        let cfg = ghost_suggestor::get_config();
                        let x = (cx as f32) + (cfg.offset_x as f32);
                        let y = (cy as f32) + (cfg.offset_y as f32);
                        builder = builder.with_position(egui::pos2(x, y));
                    }
                }
                let suggestions_clone = suggestions.clone();
                ctx.show_viewport_immediate(viewport_id, builder, move |ctx, _class| {
                    let window = EguiWindowAdapter::new(ctx);
                    window.send_viewport_command("ghost_suggestor_overlay", ViewportCommand::WindowLevel(WindowLevel::AlwaysOnTop));
                    window.send_viewport_command("ghost_suggestor_overlay", ViewportCommand::Visible(true));
                    window.send_viewport_command("ghost_suggestor_overlay", ViewportCommand::Minimized(false));
                    window.send_viewport_command("ghost_suggestor_overlay", ViewportCommand::Focus);
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.heading("Suggestions (Tab to accept, Ctrl+Tab to cycle)");
                        ui.separator();
                        egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
                            for (i, s) in suggestions_clone.iter().enumerate() {
                                let content_preview = if s.snippet.content.len() > 40 {
                                    format!("{}...", &s.snippet.content[..40])
                                } else {
                                    s.snippet.content.clone()
                                };
                                let is_selected = i == selected;
                                let label = format!("[{}] -> {}", s.snippet.trigger, content_preview);
                                let _ = ui.selectable_label(is_selected, label);
                            }
                        });
                        ui.separator();
                        ui.horizontal(|ui| {
                            let sel = selected.min(suggestions_clone.len().saturating_sub(1));
                            if let Some(s) = suggestions_clone.get(sel) {
                                if ui.button("Create Snippet").clicked() {
                                    ghost_suggestor::request_create_snippet(
                                        s.snippet.trigger.clone(),
                                        s.snippet.content.clone(),
                                    );
                                    ghost_suggestor::dismiss();
                                }
                            }
                            if ui.button("Ignore").clicked() {
                                ghost_suggestor::ignore();
                            }
                            if ui.button("Cancel").clicked() {
                                ghost_suggestor::dismiss();
                            }
                        });
                    });
                });
            }
        }

        // Process Create Snippet request from Ghost Suggestor overlay
        if let Some((trigger, content)) = ghost_suggestor::take_pending_create_snippet() {
            let cat = self.categories.first().cloned().unwrap_or_else(|| "General".to_string());
            self.snippet_editor_mode = Some(SnippetEditorMode::Add {
                category: cat.clone(),
            });
            self.snippet_editor_trigger = trigger;
            self.snippet_editor_content = content;
            self.snippet_editor_options = "*:".to_string();
            self.snippet_editor_category = cat;
            self.snippet_editor_profile = "Work".to_string();
            self.snippet_editor_template_selected = 0;
            self.snippet_editor_modal_open = true;
            self.status = "Create snippet from Ghost Suggestor".to_string();
        }

        // Check for sync completion (from background thread)
        if let Some(ref rx) = self.sync_rx {
            if let Ok((result, was_pull)) = rx.try_recv() {
                self.sync_status = result.clone();
                self.sync_rx = None;
                match &result {
                    SyncResult::Success(msg) => {
                        self.status = msg.clone();
                        if was_pull {
                            if let Ok(n) = self.reload_library_from_disk() {
                                self.status = format!("Pull complete ({} categories)", n);
                            }
                        }
                    }
                    SyncResult::Error(msg) => {
                        self.status = format!("Sync error: {}", msg);
                    }
                    _ => {}
                }
            }
        }

        // Snippet Editor modal (Add/Edit/Promote)
        if self.snippet_editor_mode.is_some() {
            ui::modals::snippet_editor_modal(self, ctx);
        }

        // Preview Expansion modals (variable input, then result)
        if self.snippet_test_var_pending.is_some() && self.snippet_test_var_modal_open {
            ui::modals::snippet_test_var_modal(self, ctx, self.file_dialog());
        }
        if self.snippet_test_result.is_some() && self.snippet_test_result_modal_open {
            ui::modals::snippet_test_result_modal(self, ctx);
        }

        // Delete confirmation dialog
        if self.snippet_delete_confirm.is_some() {
            ui::modals::delete_confirm_dialog(self, ctx);
        }

        // Clipboard History modals
        if self.clip_view_content.is_some() {
            ui::modals::clip_view_content_modal(self, ctx);
        }
        if self.clip_delete_confirm.is_some() {
            ui::modals::clip_delete_confirm_dialog(self, ctx);
        }
        if self.clip_clear_confirm_open {
            ui::modals::clip_clear_confirm_dialog(self, ctx);
        }

        // F11: VariableInputModal for {var:}, {choice:} - always-on-top viewport
        if variable_input::has_viewport_modal() {
            ui::modals::variable_input_viewport(ctx, self.file_dialog());
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Load Library").clicked() {
                        match self.try_load_library() {
                            Ok(n) => self.status = format!("Loaded {} categories", n),
                            Err(e) => self.status = format!("Load failed: {}", e),
                        }
                        ui.close_menu();
                    }
                    if ui.button("Save Library").clicked() {
                        match self.try_save_library() {
                            Ok(()) => self.status = "Saved".to_string(),
                            Err(e) => self.status = format!("Save failed: {}", e),
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Sync", |ui| {
                    if ui.button("Push to WebDAV").clicked() {
                        self.do_push_sync();
                        ui.close_menu();
                    }
                    if ui.button("Pull from WebDAV").clicked() {
                        self.do_pull_sync();
                        ui.close_menu();
                    }
                });
            });
        });

        // Categories pane only visible when Text Expansion Library tab is selected
        if self.active_tab == Tab::Library {
            egui::SidePanel::left("categories")
                .resizable(true)
                .default_width(200.0)
                .show(ctx, |ui| {
                    ui.heading("Categories");
                    ui.separator();
                    let all_selected = self.selected_category.is_none();
                    if ui.selectable_label(all_selected, "ALL").clicked() {
                        self.selected_category = None;
                    }
                    ui.separator();
                    let categories: Vec<_> = self.categories.iter().cloned().collect();
                    for (i, cat) in categories.iter().enumerate() {
                        let selected = self.selected_category == Some(i);
                        if ui.selectable_label(selected, cat).clicked() {
                            self.selected_category = Some(i);
                        }
                    }
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_tab, Tab::Library, "Text Expansion Library");
                ui.selectable_value(&mut self.active_tab, Tab::Configuration, "Configuration Settings");
                ui.selectable_value(&mut self.active_tab, Tab::ClipboardHistory, "Clipboard History");
                ui.selectable_value(&mut self.active_tab, Tab::ScriptLibrary, "Scripting Engine Library");
            });
            ui.add_space(8.0);

            match self.active_tab {
                Tab::Library => ui::library_tab::render(self, ctx, ui),
                Tab::Configuration => ui::configuration_tab::render(self, ui),
                Tab::ClipboardHistory => ui::clipboard_history_tab::render(self, ctx, ui),
                Tab::ScriptLibrary => ui::script_library_tab::render(self, ctx, ui),
            }

            ui.add_space(8.0);
            ui.separator();
            ui.label(&self.status);
        });
    }
}

impl TextExpanderApp {
    fn apply_snippet_editor_save(&mut self) {
        let mode = match &self.snippet_editor_mode {
            Some(m) => m.clone(),
            None => return,
        };
        let trigger = self.snippet_editor_trigger.trim().to_string();
        let content = self.snippet_editor_content.clone();
        let category = if self.snippet_editor_category.trim().is_empty() {
            "General".to_string()
        } else {
            self.snippet_editor_category.trim().to_string()
        };
        let profile = if self.snippet_editor_profile.trim().is_empty() {
            "Default".to_string()
        } else {
            self.snippet_editor_profile.trim().to_string()
        };
        let last_modified = LastModified::now().to_string();

        match &mode {
            SnippetEditorMode::Add { category: add_cat }
            | SnippetEditorMode::Promote { category: add_cat } => {
                let cat = if add_cat.is_empty() {
                    category.clone()
                } else {
                    add_cat.clone()
                };
                let snip = Snippet {
                    trigger: trigger.clone(),
                    content,
                    options: self.snippet_editor_options.trim().to_string(),
                    category: cat.clone(),
                    profile,
                    app_lock: self.snippet_editor_app_lock.trim().to_string(),
                    pinned: if self.snippet_editor_pinned { "true" } else { "false" }.to_string(),
                    last_modified,
                };
                self.library.entry(cat).or_default().push(snip);
                self.status = "Snippet added".to_string();
            }
            SnippetEditorMode::Edit { category: cat, snippet_idx } => {
                let mut to_move: Option<(Snippet, String, String, usize)> = None;
                let cat_owned = cat.clone();
                let options = self.snippet_editor_options.trim().to_string();
                let app_lock = self.snippet_editor_app_lock.trim().to_string();
                let pinned = if self.snippet_editor_pinned { "true" } else { "false" }.to_string();
                if let Some(snippets) = self.library.get_mut(&cat_owned) {
                    if let Some(snip) = snippets.get_mut(*snippet_idx) {
                        snip.trigger = trigger;
                        snip.content = content;
                        snip.options = options;
                        snip.category = category.clone();
                        snip.profile = profile;
                        snip.app_lock = app_lock;
                        snip.pinned = pinned;
                        snip.last_modified = last_modified.clone();
                        if snip.category != cat_owned {
                            to_move = Some((snip.clone(), category.clone(), cat_owned, *snippet_idx));
                        }
                        self.status = "Snippet updated".to_string();
                    }
                }
                if let Some((snip, new_cat, old_cat, idx)) = to_move {
                    if let Some(snippets) = self.library.get_mut(&old_cat) {
                        snippets.remove(idx);
                        if snippets.is_empty() {
                            self.library.remove(&old_cat);
                        }
                    }
                    self.library.entry(new_cat).or_default().push(snip);
                }
            }
        }
        self.categories = self.library.keys().cloned().collect();
        self.categories.sort();

        // Refresh library view: clear search filter and ensure selected category is valid
        self.library_search.clear();
        let target_cat = match &mode {
            SnippetEditorMode::Add { category: add_cat } => {
                if add_cat.is_empty() {
                    category.clone()
                } else {
                    add_cat.clone()
                }
            }
            SnippetEditorMode::Edit { .. } => category.clone(),
            SnippetEditorMode::Promote { category: promo_cat } => promo_cat.clone(),
        };
        self.selected_category = self
            .categories
            .iter()
            .position(|c| c == &target_cat)
            .or_else(|| if self.categories.is_empty() { None } else { Some(0) });

        self.sync_hotstring_listener();
        if let Err(e) = self.try_save_library() {
            self.status = format!("Save failed: {}", e);
        }
    }

    fn do_push_sync(&mut self) {
        if self.sync_rx.is_some() {
            return;
        }
        let path = self.library_path.clone();
        let url = self.sync_url.clone();
        let password = self.sync_password.clone();
        let (tx, rx) = mpsc::channel();
        self.sync_status = SyncResult::InProgress;
        self.sync_rx = Some(rx);
        std::thread::spawn(move || {
            let result = match push_sync(Path::new(&path), &url, &password) {
                Ok(()) => SyncResult::Success("Push complete".to_string()),
                Err(e) => SyncResult::Error(e.to_string()),
            };
            let _ = tx.send((result, false));
        });
    }

    fn do_pull_sync(&mut self) {
        if self.sync_rx.is_some() {
            return;
        }
        let path = self.library_path.clone();
        let url = self.sync_url.clone();
        let password = self.sync_password.clone();
        let (tx, rx) = mpsc::channel();
        self.sync_status = SyncResult::InProgress;
        self.sync_rx = Some(rx);
        std::thread::spawn(move || {
            let result = match pull_sync(Path::new(&path), &url, &password) {
                Ok(lib) => SyncResult::Success(format!("Pull complete ({} categories)", lib.len())),
                Err(e) => SyncResult::Error(e.to_string()),
            };
            let _ = tx.send((result, true));
        });
    }
}

impl TextExpanderApp {
    /// Start or update hotstring listener with current library.
    fn sync_hotstring_listener(&mut self) {
        self.sync_template_config();
        self.sync_ghost_suggestor_config();
        if self.library.is_empty() {
            return;
        }
        if is_listener_running() {
            update_library(self.library.clone());
        } else if let Err(e) = start_listener(self.library.clone()) {
            self.status = format!("Hotstring failed to start: {}", e);
        }
    }

    fn sync_ghost_suggestor_config(&self) {
        ghost_suggestor::update_config(ghost_suggestor::GhostSuggestorConfig {
            enabled: self.ghost_suggestor_enabled,
            debounce_ms: self.ghost_suggestor_debounce_ms,
            display_duration_secs: self.ghost_suggestor_display_secs,
            snooze_duration_mins: 5,
            offset_x: self.ghost_suggestor_offset_x,
            offset_y: self.ghost_suggestor_offset_y,
        });
    }

    fn build_discovery_config(&self) -> discovery::DiscoveryConfig {
        discovery::DiscoveryConfig {
            threshold: self.discovery_threshold,
            lookback_minutes: self.discovery_lookback,
            min_phrase_len: self.discovery_min_len,
            max_phrase_len: self.discovery_max_len,
            excluded_apps: self
                .discovery_excluded_apps
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            excluded_window_titles: self
                .discovery_excluded_window_titles
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        }
    }

    fn export_library_json(&self, path: &Path) -> anyhow::Result<()> {
        use digicore_core::adapters::persistence::JsonLibraryAdapter;
        use digicore_core::domain::ports::SnippetRepository;
        JsonLibraryAdapter.save(path, &self.library)?;
        Ok(())
    }

    fn export_library_csv(&self, path: &Path) -> anyhow::Result<()> {
        let mut w = std::io::BufWriter::new(std::fs::File::create(path)?);
        use std::io::Write;
        writeln!(w, "trigger,content,options,category,profile,app_lock,pinned,last_modified")?;
        for (_cat, snippets) in &self.library {
            for s in snippets {
                let escape = |s: &str| {
                    let t = s.replace('\\', "\\\\").replace('"', "\\\"");
                    if t.contains(',') || t.contains('"') || t.contains('\n') {
                        format!("\"{}\"", t)
                    } else {
                        t
                    }
                };
                writeln!(
                    w,
                    "{},{},{},{},{},{},{},{}",
                    escape(&s.trigger),
                    escape(&s.content),
                    escape(&s.options),
                    escape(&s.category),
                    escape(&s.profile),
                    escape(&s.app_lock),
                    escape(&s.pinned),
                    escape(&s.last_modified)
                )?;
            }
        }
        Ok(())
    }

    fn import_library(&mut self, path: &Path, replace: bool) -> anyhow::Result<()> {
        use digicore_core::adapters::persistence::JsonLibraryAdapter;
        use digicore_core::domain::ports::SnippetRepository;
        let incoming = JsonLibraryAdapter.load(path)?;
        if replace {
            self.library = incoming;
        } else {
            for (cat, snippets) in incoming {
                self.library.entry(cat).or_default().extend(snippets);
            }
        }
        self.normalize_library_by_snippet_category();
        self.sync_hotstring_listener();
        self.try_save_library()?;
        Ok(())
    }

    fn import_library_csv(&mut self, path: &Path, replace: bool) -> anyhow::Result<()> {
        let incoming = Self::parse_csv_library(path)?;
        if replace {
            self.library = incoming;
        } else {
            for (cat, snippets) in incoming {
                self.library.entry(cat).or_default().extend(snippets);
            }
        }
        self.normalize_library_by_snippet_category();
        self.sync_hotstring_listener();
        self.try_save_library()?;
        Ok(())
    }

    /// Parse CSV file matching export format: trigger,content,options,category,profile,app_lock,pinned,last_modified
    fn parse_csv_library(path: &Path) -> anyhow::Result<HashMap<String, Vec<Snippet>>> {
        let s = std::fs::read_to_string(path)?;
        let rows = Self::parse_csv_rows(&s);
        let mut library: HashMap<String, Vec<Snippet>> = HashMap::new();
        let mut iter = rows.into_iter();
        if let Some(header) = iter.next() {
            if header.len() < 8 || header[0] != "trigger" || header[1] != "content" {
                if let Some(snip) = Self::row_to_snippet(&header) {
                    let cat = if snip.category.is_empty() {
                        "General".to_string()
                    } else {
                        snip.category.clone()
                    };
                    library.entry(cat).or_default().push(snip);
                }
            }
        }
        for row in iter {
            if let Some(snip) = Self::row_to_snippet(&row) {
                let cat = if snip.category.is_empty() {
                    "General".to_string()
                } else {
                    snip.category.clone()
                };
                library.entry(cat).or_default().push(snip);
            }
        }
        Ok(library)
    }

    fn parse_csv_rows(s: &str) -> Vec<Vec<String>> {
        let mut rows = Vec::new();
        let mut row = Vec::new();
        let mut field = String::new();
        let mut in_quotes = false;
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '"' if !in_quotes => in_quotes = true,
                '"' if in_quotes => {
                    match chars.peek() {
                        Some('"') => {
                            chars.next();
                            field.push('"');
                        }
                        _ => in_quotes = false,
                    }
                }
                '\\' if in_quotes => {
                    match chars.next() {
                        Some('n') => field.push('\n'),
                        Some('r') => field.push('\r'),
                        Some('t') => field.push('\t'),
                        Some('"') => field.push('"'),
                        Some('\\') => field.push('\\'),
                        Some(x) => {
                            field.push('\\');
                            field.push(x);
                        }
                        None => field.push('\\'),
                    }
                }
                ',' if !in_quotes => {
                    row.push(std::mem::take(&mut field));
                }
                '\n' if !in_quotes => {
                    row.push(std::mem::take(&mut field));
                    if !row.is_empty() || !field.is_empty() {
                        rows.push(std::mem::take(&mut row));
                    }
                }
                '\r' if !in_quotes => {
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    row.push(std::mem::take(&mut field));
                    if !row.is_empty() || !field.is_empty() {
                        rows.push(std::mem::take(&mut row));
                    }
                }
                _ => field.push(c),
            }
        }
        if !field.is_empty() || !row.is_empty() {
            row.push(std::mem::take(&mut field));
            rows.push(row);
        }
        rows
    }

    fn row_to_snippet(row: &[String]) -> Option<Snippet> {
        if row.len() < 2 {
            return None;
        }
        let trigger = row.get(0).cloned().unwrap_or_default();
        if trigger.is_empty() {
            return None;
        }
        Some(Snippet {
            trigger,
            content: row.get(1).cloned().unwrap_or_default(),
            options: row.get(2).cloned().unwrap_or_default(),
            category: row.get(3).cloned().unwrap_or_default(),
            profile: row.get(4).cloned().unwrap_or_else(|| "Default".to_string()),
            app_lock: row.get(5).cloned().unwrap_or_default(),
            pinned: row.get(6).cloned().unwrap_or_else(|| "false".to_string()),
            last_modified: row.get(7).cloned().unwrap_or_default(),
        })
    }

    fn build_ghost_follower_config(&self) -> GhostFollowerConfig {
        let monitor_anchor = match self.ghost_follower_monitor_anchor {
            1 => MonitorAnchor::Secondary,
            2 => MonitorAnchor::Current,
            _ => MonitorAnchor::Primary,
        };
        GhostFollowerConfig {
            enabled: self.ghost_follower_enabled,
            edge: if self.ghost_follower_edge_right {
                FollowerEdge::Right
            } else {
                FollowerEdge::Left
            },
            monitor_anchor,
            search_filter: self.ghost_follower_search.clone(),
            hover_preview: self.ghost_follower_hover_preview,
            collapse_delay_secs: self.ghost_follower_collapse_delay_secs,
        }
    }

    fn sync_template_config(&self) {
        template_processor::set_config(TemplateConfig {
            date_format: self.template_date_format.clone(),
            time_format: self.template_time_format.clone(),
            clip_max_depth: self.clip_history_max_depth,
        });
    }

    /// Reload library from disk (used after pull). No startup sync.
    fn reload_library_from_disk(&mut self) -> anyhow::Result<usize> {
        use digicore_core::adapters::persistence::JsonLibraryAdapter;
        use digicore_core::domain::ports::SnippetRepository;
        use std::path::Path;

        let path = Path::new(&self.library_path);
        let repo = JsonLibraryAdapter;
        let library = repo.load(path)?;
        self.library = library;
        self.normalize_library_by_snippet_category();
        self.selected_category = if self.categories.is_empty() {
            None
        } else {
            None
        }; // None = ALL
        self.sync_hotstring_listener();
        Ok(self.categories.len())
    }

    /// F37: Startup sync - pull before load if sync configured.
    fn try_load_library(&mut self) -> anyhow::Result<usize> {
        use digicore_core::adapters::persistence::JsonLibraryAdapter;
        use digicore_core::domain::ports::SnippetRepository;
        use std::path::Path;

        let path = Path::new(&self.library_path);

        // F37: Startup sync before LoadLibrary (once per session)
        if !self.startup_sync_done
            && !self.sync_url.is_empty()
            && !self.sync_password.is_empty()
        {
            if let Ok(lib) = pull_sync(path, &self.sync_url, &self.sync_password) {
                self.library = lib;
                self.normalize_library_by_snippet_category();
                self.selected_category = if self.categories.is_empty() {
                    None
                } else {
                    None
                }; // None = ALL
                self.startup_sync_done = true;
                self.sync_hotstring_listener();
                return Ok(self.categories.len());
            }
        }

        let repo = JsonLibraryAdapter;
        let library = repo.load(path)?;
        self.library = library;
        self.normalize_library_by_snippet_category();
        self.selected_category = if self.categories.is_empty() {
            None
        } else {
            None
        }; // None = ALL
        self.startup_sync_done = true;
        self.sync_hotstring_listener();
        Ok(self.categories.len())
    }

    /// Re-group library by each snippet's `category` field (not JSON structure keys).
    /// Handles legacy JSON where all snippets may be under a single container key like "User Library".
    fn normalize_library_by_snippet_category(&mut self) {
        let mut regrouped: HashMap<String, Vec<Snippet>> = HashMap::new();
        for snippets in self.library.values() {
            for snip in snippets {
                let cat = if snip.category.trim().is_empty() {
                    "General".to_string()
                } else {
                    snip.category.trim().to_string()
                };
                regrouped.entry(cat).or_default().push(snip.clone());
            }
        }
        self.library = regrouped;
        self.categories = self.library.keys().cloned().collect();
        self.categories.sort();
    }

    fn try_save_library(&mut self) -> anyhow::Result<()> {
        use digicore_core::adapters::persistence::JsonLibraryAdapter;
        use digicore_core::domain::ports::SnippetRepository;
        use std::path::Path;

        let path = Path::new(&self.library_path);
        let repo = JsonLibraryAdapter;
        let library = if self.library.is_empty() {
            repo.load(path)?
        } else {
            self.library.clone()
        };
        repo.save(path, &library)?;
        Ok(())
    }
}
