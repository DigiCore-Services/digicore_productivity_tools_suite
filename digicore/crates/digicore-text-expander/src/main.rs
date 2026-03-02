//! Text Expander - DigiCore Services.
//!
//! Cross-platform text expansion with egui management console.

use digicore_core::domain::{LastModified, Snippet};
use digicore_text_expander::application::discovery;
use digicore_text_expander::application::expansion_engine::set_expansion_paused;
#[cfg(target_os = "windows")]
use digicore_text_expander::platform::windows_caret;
use digicore_text_expander::application::clipboard_history::{self, ClipboardHistoryConfig};
use digicore_text_expander::application::ghost_follower::{self, FollowerEdge, GhostFollowerConfig, MonitorAnchor};
use digicore_text_expander::application::ghost_suggestor::{self, GhostSuggestorConfig};
use digicore_text_expander::application::scripting::{
    get_scripting_config, load_and_apply_script_libraries, load_scripting_config, set_global_library,
    set_scripting_config,
};
use digicore_text_expander::application::js_syntax_highlighter::highlight_js;
use digicore_text_expander::application::template_processor::{self, InteractiveVarType, TemplateConfig};
use digicore_text_expander::application::variable_input;
use digicore_text_expander::drivers::hotstring::{is_listener_running, request_expansion, start_listener, update_library};
use digicore_text_expander::services::sync_service::{pull_sync, push_sync, SyncResult};
use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc;

/// Default Global Script Library content (plan 6.8.3). Used when file is missing.
const DEFAULT_GLOBAL_LIBRARY: &str = r#"/**
 * Text Expansion Pro - Global Script Library
 * Define reusable JavaScript functions here for use in any snippet.
 *
 * Simply call these from any {js:...} tag!
 */

/**
 * Greets a user by name.
 * @param {string} name
 * @returns {string}
 */
function greet(name) {
    return "Hello, " + name + "!";
}

/**
 * Returns a friendly greeting based on the current time of day.
 * @returns {string}
 */
function getTimeGreeting() {
    var hour = new Date().getHours();
    if (hour < 12) return "Good Morning";
    if (hour < 18) return "Good Afternoon";
    return "Good Evening";
}

/**
 * Cleans a string by removing extra whitespace and trimming.
 * Useful for {js: clipClean("{clipboard}")}
 * @param {string} str
 * @returns {string}
 */
function clipClean(str) {
    if (!str) return "";
    return str.replace(/\s+/g, ' ').trim();
}

/**
 * Formats a number to a specific number of decimal places.
 * @param {number} num
 * @param {number} decimals
 * @returns {string}
 */
function mathRound(num, decimals) {
    return Number(num).toFixed(decimals || 2);
}

/**
 * Test GUI Works Function
 */
function guiTest() { return "GUI Save Works!"; }
"#;

fn main() -> eframe::Result<()> {
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

/// Active tab index.
#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Library = 0,
    Configuration = 1,
    ScriptLibrary = 2,
}

/// Main application state.
struct TextExpanderApp {
    library_path: String,
    library: HashMap<String, Vec<Snippet>>,
    categories: Vec<String>,
    selected_category: Option<usize>,
    status: String,
    active_tab: Tab,

    // Sync config (Configuration tab)
    sync_url: String,
    sync_password: String,
    sync_status: SyncResult,
    sync_rx: Option<mpsc::Receiver<(SyncResult, bool)>>, // (result, was_pull)

    // Startup sync (F37): perform once on first load if sync configured
    startup_sync_done: bool,

    // F7: Global pause for expansion
    expansion_paused: bool,

    // Auto-load library once on startup (when path exists)
    initial_load_attempted: bool,

    // Ensure window shown on first frame (override persistence-restored minimized state)
    window_visibility_ensured: bool,

    // Discovery (F60-F69)
    discovery_enabled: bool,
    discovery_threshold: u32,
    discovery_lookback: u32,
    discovery_min_len: usize,
    discovery_max_len: usize,
    discovery_excluded_apps: String,
    discovery_excluded_window_titles: String,

    // Ghost Suggestor (F43-F47)
    ghost_suggestor_enabled: bool,
    ghost_suggestor_debounce_ms: u64,
    ghost_suggestor_offset_x: i32,
    ghost_suggestor_offset_y: i32,

    // Ghost Follower (F48-F59)
    ghost_follower_enabled: bool,
    ghost_follower_edge_right: bool,
    ghost_follower_monitor_anchor: usize, // 0=Primary, 1=Secondary, 2=Current
    ghost_follower_search: String,
    ghost_follower_hover_preview: bool,
    ghost_follower_collapse_delay_secs: u64,
    clip_history_max_depth: usize,

    // Templates (F16-F20): configurable date/time formats
    template_date_format: String,
    template_time_format: String,

    // F41: Promote to snippet - pending content + modal state
    promote_pending: Option<String>,
    promote_trigger: String,
    promote_save_clicked: bool,
    promote_modal_open: bool,

    // Snippet Editor (Add/Edit) - Library tab
    snippet_editor_mode: Option<SnippetEditorMode>,
    snippet_editor_trigger: String,
    snippet_editor_content: String,
    snippet_editor_options: String,
    snippet_editor_category: String,
    snippet_editor_profile: String,
    snippet_editor_app_lock: String,
    snippet_editor_pinned: bool,
    snippet_editor_save_clicked: bool,
    snippet_editor_modal_open: bool,
    snippet_editor_template_selected: usize,

    // Delete confirmation: (category, snippet_idx)
    snippet_delete_confirm: Option<(String, usize)>,
    snippet_delete_dialog_open: bool,

    // Snippet Library search (AND multi-word filter)
    library_search: String,

    // Script Library tab (F86): Phase 7 {run:} security + Global JavaScript Library
    script_library_run_disabled: bool,
    script_library_run_allowlist: String,
    script_library_js_content: String,
    script_library_loaded: bool,
    // Plan 6.8.4: Python/Lua library sections (when py.enabled / lua.enabled)
    script_library_py_content: String,
    script_library_lua_content: String,
}

/// Built-in snippet templates (Phase 8; AHK parity).
const SNIPPET_TEMPLATES: &[(&str, &str)] = &[
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

/// Add new snippet or edit existing.
#[derive(Clone)]
enum SnippetEditorMode {
    Add { category: String },
    Edit { category: String, snippet_idx: usize },
}

impl TextExpanderApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let storage = cc.storage;
        let library_path = storage
            .and_then(|s| s.get_string("library_path"))
            .unwrap_or_else(|| {
                dirs::config_dir()
                    .map(|p| p.join("DigiCore").join("text_expansion_library.json"))
                    .and_then(|p| p.to_str().map(String::from))
                    .unwrap_or_else(|| "text_expansion_library.json".to_string())
            });
        let sync_url = storage
            .and_then(|s| s.get_string("sync_url"))
            .unwrap_or_default();
        let template_date_format = storage
            .and_then(|s| s.get_string("template_date_format"))
            .unwrap_or_else(|| "%Y-%m-%d".to_string());
        let template_time_format = storage
            .and_then(|s| s.get_string("template_time_format"))
            .unwrap_or_else(|| "%H:%M".to_string());
        let (run_disabled, run_allowlist) = storage
            .and_then(|s| {
                let d = s.get_string("script_library_run_disabled").map(|v| v == "true");
                let a = s.get_string("script_library_run_allowlist").unwrap_or_default();
                d.map(|d| (d, a))
            })
            .unwrap_or_else(|| {
                let cfg = load_scripting_config();
                (cfg.run.disabled, cfg.run.allowlist)
            });
        {
            let mut cfg = get_scripting_config();
            cfg.run.disabled = run_disabled;
            cfg.run.allowlist = run_allowlist.clone();
            set_scripting_config(cfg);
        }

        Self {
            library_path: library_path.clone(),
            library: HashMap::new(),
            categories: Vec::new(),
            selected_category: None,
            status: "Ready".to_string(),
            active_tab: Tab::Library,
            sync_url,
            sync_password: String::new(),
            sync_status: SyncResult::Idle,
            sync_rx: None,
            startup_sync_done: false,
            expansion_paused: false,
            initial_load_attempted: false,
            window_visibility_ensured: false,
            discovery_enabled: false,
            discovery_threshold: 2,
            discovery_lookback: 60,
            discovery_min_len: 3,
            discovery_max_len: 50,
            discovery_excluded_apps: String::new(),
            discovery_excluded_window_titles: String::new(),
            ghost_suggestor_enabled: true,
            ghost_suggestor_debounce_ms: 50,
            ghost_suggestor_offset_x: 0,
            ghost_suggestor_offset_y: 20,
            ghost_follower_enabled: true,
            ghost_follower_edge_right: true,
            ghost_follower_monitor_anchor: 0,
            ghost_follower_search: String::new(),
            ghost_follower_hover_preview: true,
            ghost_follower_collapse_delay_secs: 5,
            clip_history_max_depth: 20,
            template_date_format,
            template_time_format,
            promote_pending: None,
            promote_trigger: String::new(),
            promote_save_clicked: false,
            promote_modal_open: true,
            snippet_editor_mode: None,
            snippet_editor_trigger: String::new(),
            snippet_editor_content: String::new(),
            snippet_editor_options: String::new(),
            snippet_editor_category: String::new(),
            snippet_editor_profile: "Default".to_string(),
            snippet_editor_app_lock: String::new(),
            snippet_editor_pinned: false,
            snippet_editor_save_clicked: false,
            snippet_editor_modal_open: false,
            snippet_editor_template_selected: 0,
            snippet_delete_confirm: None,
            snippet_delete_dialog_open: false,
            library_search: String::new(),
            script_library_run_disabled: run_disabled,
            script_library_run_allowlist: run_allowlist,
            script_library_js_content: String::new(),
            script_library_loaded: false,
            script_library_py_content: String::new(),
            script_library_lua_content: String::new(),
        }
    }
}

impl eframe::App for TextExpanderApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("library_path", self.library_path.clone());
        storage.set_string("sync_url", self.sync_url.clone());
        storage.set_string("template_date_format", self.template_date_format.clone());
        storage.set_string("template_time_format", self.template_time_format.clone());
        storage.set_string(
            "script_library_run_disabled",
            self.script_library_run_disabled.to_string(),
        );
        storage.set_string("script_library_run_allowlist", self.script_library_run_allowlist.clone());
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

        // F41: Check for promote to snippet request from Ghost Follower
        if let Some(content) = clipboard_history::take_promote_pending() {
            self.promote_pending = Some(content);
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

        // F48-F59: Ghost Follower ribbon - show when enabled
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
                .with_always_on_top();
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
            let pinned_clone = pinned.clone();
            let clips_clone = clips.clone();
            let hover_preview = cfg.hover_preview;
            ctx.show_viewport_immediate(viewport_id, builder, move |ctx, _class| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    if collapsed {
                        if ui.button("TE").clicked() {
                            ghost_follower::touch();
                            ghost_follower::set_collapsed(false);
                        }
                        ui.label("Click to expand");
                    } else {
                        ui.heading("Pinned + Clipboard");
                        ui.label("Double-click insert, right-click Promote");
                        ui.separator();
                        let mut search = ghost_follower::get_search_filter();
                        if ui.text_edit_singleline(&mut search).changed() {
                            ghost_follower::set_search_filter(&search);
                            ghost_follower::touch();
                        }
                        ui.separator();
                        ui.collapsing("Pinned Snippets", |ui| {
                            egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
                                for (snip, _cat) in &pinned_clone {
                                    let content_preview = if snip.content.len() > 30 {
                                        format!("{}...", &snip.content[..30])
                                    } else {
                                        snip.content.clone()
                                    };
                                    let label = format!("[{}] {}", snip.trigger, content_preview);
                                    let r = if hover_preview {
                                        ui.selectable_label(false, label)
                                            .on_hover_ui(|ui| { ui.label(snip.content.replace('\n', "\n")); })
                                    } else {
                                        ui.selectable_label(false, label)
                                    };
                                    if r.double_clicked() {
                                        ghost_follower::touch();
                                        request_expansion(snip.content.clone());
                                    }
                                    if r.secondary_clicked() {
                                        ghost_follower::touch();
                                        clipboard_history::request_promote(snip.content.clone());
                                    }
                                    if r.hovered() {
                                        ghost_follower::touch();
                                    }
                                }
                                if pinned_clone.is_empty() {
                                    ui.label("No pinned snippets.");
                                }
                            });
                        });
                        ui.collapsing("Clipboard History", |ui| {
                            egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
                                for entry in &clips_clone {
                                    let content_preview = if entry.content.len() > 30 {
                                        format!("{}...", &entry.content[..30])
                                    } else {
                                        entry.content.clone()
                                    };
                                    let label = content_preview.replace('\n', " ");
                                    let r = if hover_preview {
                                        ui.selectable_label(false, label)
                                            .on_hover_ui(|ui| { ui.label(entry.content.replace('\n', "\n")); })
                                    } else {
                                        ui.selectable_label(false, label)
                                    };
                                    if r.double_clicked() {
                                        ghost_follower::touch();
                                        request_expansion(entry.content.clone());
                                    }
                                    if r.secondary_clicked() {
                                        ghost_follower::touch();
                                        clipboard_history::request_promote(entry.content.clone());
                                    }
                                    if r.hovered() {
                                        ghost_follower::touch();
                                    }
                                }
                                if clips_clone.is_empty() {
                                    ui.label("No clipboard history.");
                                }
                            });
                        });
                    }
                });
            });
            self.ghost_follower_search = ghost_follower::get_search_filter();
        }

        // F43-F47: Ghost Suggestor overlay - show when suggestions exist (F46: caret-based position)
        if ghost_suggestor::is_enabled() && ghost_suggestor::has_suggestions() {
            ctx.request_repaint();
            let suggestions = ghost_suggestor::get_suggestions();
            let selected = ghost_suggestor::get_selected_index();
            let viewport_id = egui::ViewportId::from_hash_of("ghost_suggestor_overlay");
            let mut builder = egui::ViewportBuilder::default()
                .with_title("Ghost Suggestor")
                .with_inner_size([280.0, 200.0])
                .with_decorations(true)
                .with_always_on_top();
            #[cfg(target_os = "windows")]
            {
                if let Some((cx, cy)) = windows_caret::get_caret_screen_position() {
                    let cfg = ghost_suggestor::get_config();
                    let x = (cx as f32) + (cfg.offset_x as f32);
                    let y = (cy as f32) + (cfg.offset_y as f32);
                    builder = builder.with_position(egui::pos2(x, y));
                }
            }
            ctx.show_viewport_immediate(viewport_id, builder, |ctx, _class| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.heading("Suggestions (Tab to accept, Ctrl+Tab to cycle)");
                    ui.separator();
                    egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
                        for (i, s) in suggestions.iter().enumerate() {
                            let content_preview = if s.snippet.content.len() > 40 {
                                format!("{}...", &s.snippet.content[..40])
                            } else {
                                s.snippet.content.clone()
                            };
                            let is_selected = i == selected;
                            let label = format!("[{}] -> {}", s.snippet.trigger, content_preview);
                            if ui.selectable_label(is_selected, label).clicked() {
                                // Click to accept - would need to inject into suggestor
                            }
                        }
                    });
                });
            });
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

        // F41: Promote to snippet modal
        if self.promote_pending.is_some() {
            self.ui_promote_modal(ctx);
        }

        // Snippet Editor modal (Add/Edit)
        if self.snippet_editor_mode.is_some() {
            self.ui_snippet_editor_modal(ctx);
        }

        // Delete confirmation dialog
        if self.snippet_delete_confirm.is_some() {
            self.ui_delete_confirm_dialog(ctx);
        }

        // F11: VariableInputModal for {var:}, {choice:} - always-on-top viewport
        if variable_input::has_viewport_modal() {
            self.ui_variable_input_viewport(ctx);
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

        egui::SidePanel::left("categories")
            .resizable(true)
            .default_width(200.0)
            .show(ctx, |ui| {
                ui.heading("Categories");
                ui.separator();
                for (i, cat) in self.categories.iter().enumerate() {
                    let selected = self.selected_category == Some(i);
                    if ui.selectable_label(selected, cat).clicked() {
                        self.selected_category = Some(i);
                    }
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_tab, Tab::Library, "Library");
                ui.selectable_value(&mut self.active_tab, Tab::Configuration, "Configuration");
                ui.selectable_value(&mut self.active_tab, Tab::ScriptLibrary, "Script Library");
            });
            ui.add_space(8.0);

            match self.active_tab {
                Tab::Library => self.ui_library_tab(ctx, ui),
                Tab::Configuration => self.ui_configuration_tab(ui),
                Tab::ScriptLibrary => self.ui_script_library_tab(ctx, ui),
            }

            ui.add_space(8.0);
            ui.separator();
            ui.label(&self.status);
        });
    }
}

impl TextExpanderApp {
    fn ui_promote_modal(&mut self, ctx: &egui::Context) {
        let content = match &self.promote_pending {
            Some(c) => c.clone(),
            None => return,
        };
        if self.promote_trigger.is_empty() {
            self.promote_trigger = content
                .chars()
                .take(20)
                .filter(|c| !c.is_whitespace())
                .collect();
            if self.promote_trigger.is_empty() {
                self.promote_trigger = "clip".to_string();
            }
        }
        let trigger = &mut self.promote_trigger;
        let save_clicked = &mut self.promote_save_clicked;
        let close_requested = std::sync::atomic::AtomicBool::new(false);
        let close_requested_ref = &close_requested;
        egui::Window::new("Promote to Snippet (F41)")
            .collapsible(false)
            .resizable(true)
            .open(&mut self.promote_modal_open)
            .show(ctx, |ui| {
                ui.label("Trigger (shortcut):");
                ui.add(egui::TextEdit::singleline(trigger).desired_width(200.0));
                ui.label("Content:");
                let content_preview = if content.len() > 100 {
                    format!("{}...", &content[..100])
                } else {
                    content.clone()
                };
                ui.label(content_preview);
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        *save_clicked = true;
                        close_requested_ref.store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                    if ui.button("Cancel").clicked() {
                        close_requested_ref.store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                });
            });
        if close_requested.load(std::sync::atomic::Ordering::SeqCst) {
            self.promote_modal_open = false;
        }
        if !self.promote_modal_open {
            if self.promote_save_clicked {
                let cat = self
                    .categories
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "General".to_string());
                let snip = Snippet::new(self.promote_trigger.trim(), content);
                self.library.entry(cat.clone()).or_default().push(snip);
                self.categories = self.library.keys().cloned().collect();
                self.categories.sort();
                update_library(self.library.clone());
                if let Err(e) = self.try_save_library() {
                    self.status = format!("Save failed: {}", e);
                } else {
                    self.status = "Snippet added".to_string();
                }
            }
            self.promote_pending = None;
            self.promote_trigger.clear();
            self.promote_save_clicked = false;
            self.promote_modal_open = true;
        }
    }

    fn ui_variable_input_viewport(&mut self, ctx: &egui::Context) {
        let viewport_id = egui::ViewportId::from_hash_of("variable_input_modal");
        let builder = egui::ViewportBuilder::default()
            .with_title("Snippet Input Required (F11)")
            .with_inner_size([340.0, 280.0])
            .with_resizable(true)
            .with_decorations(true)
            .with_taskbar(true)
            .with_always_on_top()
            .with_window_level(egui::WindowLevel::AlwaysOnTop);
        let result = std::sync::Arc::new(std::sync::Mutex::new(None));
        let result_clone = result.clone();
        ctx.show_viewport_immediate(viewport_id, builder, move |ctx, _class| {
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                egui::WindowLevel::AlwaysOnTop,
            ));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            if let Some(r) = variable_input::render_viewport_modal(ctx) {
                *result_clone.lock().unwrap() = Some(r);
            }
        });
        let r = result.lock().unwrap().take();
        if let Some(r) = r {
            variable_input::process_viewport_result(r);
        }
    }

    fn ui_snippet_editor_modal(&mut self, ctx: &egui::Context) {
        let mode = match &self.snippet_editor_mode {
            Some(m) => m.clone(),
            None => return,
        };
        let title = match &mode {
            SnippetEditorMode::Add { .. } => "Add Snippet",
            SnippetEditorMode::Edit { .. } => "Edit Snippet",
        };
        let save_clicked = &mut self.snippet_editor_save_clicked;
        let close_requested = std::sync::atomic::AtomicBool::new(false);
        let close_requested_ref = &close_requested;
        egui::Window::new(title)
            .collapsible(false)
            .resizable(true)
            .default_width(400.0)
            .open(&mut self.snippet_editor_modal_open)
            .show(ctx, |ui| {
                ui.label("Trigger (shortcut):");
                ui.add(egui::TextEdit::singleline(&mut self.snippet_editor_trigger).desired_width(300.0));
                ui.label("From template:");
                egui::ComboBox::from_id_salt("snippet_template")
                    .selected_text(SNIPPET_TEMPLATES[self.snippet_editor_template_selected.min(SNIPPET_TEMPLATES.len().saturating_sub(1))].0)
                    .show_ui(ui, |ui| {
                        for (i, (label, content)) in SNIPPET_TEMPLATES.iter().enumerate() {
                            if ui.selectable_label(self.snippet_editor_template_selected == i, *label).clicked() {
                                self.snippet_editor_template_selected = i;
                                if !content.is_empty() {
                                    self.snippet_editor_content = content.to_string();
                                }
                            }
                        }
                    });
                ui.label("Content:");
                egui::ScrollArea::vertical()
                    .max_height(200.0)
                    .stick_to_bottom(false)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut self.snippet_editor_content)
                                .desired_width(300.0)
                                .desired_rows(6),
                        );
                    });
                ui.label("Options (e.g. * for anywhere, ? for word):");
                ui.add(egui::TextEdit::singleline(&mut self.snippet_editor_options).desired_width(300.0));
                ui.label("Category:");
                ui.add(egui::TextEdit::singleline(&mut self.snippet_editor_category).desired_width(300.0));
                ui.label("Profile:");
                ui.add(egui::TextEdit::singleline(&mut self.snippet_editor_profile).desired_width(300.0));
                ui.label("App lock (comma-separated exe names, empty = all apps):");
                ui.add(egui::TextEdit::singleline(&mut self.snippet_editor_app_lock).desired_width(300.0));
                ui.checkbox(&mut self.snippet_editor_pinned, "Pinned (priority in search)");
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        *save_clicked = true;
                        close_requested_ref.store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                    if ui.button("Cancel").clicked() {
                        close_requested_ref.store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                });
            });
        if close_requested.load(std::sync::atomic::Ordering::SeqCst) {
            self.snippet_editor_modal_open = false;
        }
        if !self.snippet_editor_modal_open {
            if self.snippet_editor_save_clicked {
                self.apply_snippet_editor_save();
            }
            self.snippet_editor_mode = None;
            self.snippet_editor_trigger.clear();
            self.snippet_editor_content.clear();
            self.snippet_editor_options.clear();
            self.snippet_editor_category.clear();
            self.snippet_editor_profile = "Default".to_string();
            self.snippet_editor_app_lock.clear();
            self.snippet_editor_pinned = false;
            self.snippet_editor_save_clicked = false;
            self.snippet_editor_template_selected = 0;
            self.snippet_editor_modal_open = true;
        }
    }

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

        match mode {
            SnippetEditorMode::Add { category: add_cat } => {
                let cat = if add_cat.is_empty() { category } else { add_cat };
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
                if let Some(snippets) = self.library.get_mut(&cat) {
                    if let Some(snip) = snippets.get_mut(snippet_idx) {
                        snip.trigger = trigger;
                        snip.content = content;
                        snip.options = self.snippet_editor_options.trim().to_string();
                        snip.category = category.clone();
                        snip.profile = profile;
                        snip.app_lock = self.snippet_editor_app_lock.trim().to_string();
                        snip.pinned = if self.snippet_editor_pinned { "true" } else { "false" }.to_string();
                        snip.last_modified = last_modified.clone();
                        if snip.category != cat {
                            to_move = Some((snip.clone(), category.clone(), cat.clone(), snippet_idx));
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
        self.sync_hotstring_listener();
        if let Err(e) = self.try_save_library() {
            self.status = format!("Save failed: {}", e);
        }
    }

    fn ui_delete_confirm_dialog(&mut self, ctx: &egui::Context) {
        let (cat, idx) = match &self.snippet_delete_confirm {
            Some(p) => p.clone(),
            None => return,
        };
        let close_requested = std::sync::atomic::AtomicBool::new(false);
        let confirmed = std::sync::atomic::AtomicBool::new(false);
        let close_ref = &close_requested;
        let confirmed_ref = &confirmed;
        egui::Window::new("Delete Snippet?")
            .collapsible(false)
            .resizable(false)
            .open(&mut self.snippet_delete_dialog_open)
            .show(ctx, |ui| {
                ui.label(format!("Delete snippet in category \"{}\"?", cat));
                ui.label("This cannot be undone.");
                ui.horizontal(|ui| {
                    if ui.button("Delete").clicked() {
                        confirmed_ref.store(true, std::sync::atomic::Ordering::SeqCst);
                        close_ref.store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                    if ui.button("Cancel").clicked() {
                        close_ref.store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                });
            });
        if close_requested.load(std::sync::atomic::Ordering::SeqCst) {
            self.snippet_delete_dialog_open = false;
        }
        if !self.snippet_delete_dialog_open {
            if confirmed.load(std::sync::atomic::Ordering::SeqCst) {
                if let Some(snippets) = self.library.get_mut(&cat) {
                    if idx < snippets.len() {
                        snippets.remove(idx);
                        if snippets.is_empty() {
                            self.library.remove(&cat);
                        }
                        self.categories = self.library.keys().cloned().collect();
                        self.categories.sort();
                        self.sync_hotstring_listener();
                        if let Err(e) = self.try_save_library() {
                            self.status = format!("Save failed: {}", e);
                        } else {
                            self.status = "Snippet deleted".to_string();
                        }
                    }
                }
            }
            self.snippet_delete_confirm = None;
        }
    }

    fn ui_library_tab(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.heading("DigiCore Text Expander");
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Library path:");
            ui.text_edit_singleline(&mut self.library_path);
            if ui.button("Load").clicked() {
                match self.try_load_library() {
                    Ok(n) => self.status = format!("Loaded {} categories", n),
                    Err(e) => self.status = format!("Load failed: {}", e),
                }
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Export JSON").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("JSON", &["json"])
                    .set_file_name("text_expansion_library.json")
                    .save_file()
                {
                    if let Err(e) = self.export_library_json(&path) {
                        self.status = format!("Export failed: {}", e);
                    } else {
                        self.status = format!("Exported to {}", path.display());
                    }
                }
            }
            if ui.button("Export CSV").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("CSV", &["csv"])
                    .set_file_name("snippets.csv")
                    .save_file()
                {
                    if let Err(e) = self.export_library_csv(&path) {
                        self.status = format!("Export failed: {}", e);
                    } else {
                        self.status = format!("Exported to {}", path.display());
                    }
                }
            }
            if ui.button("Import (Replace)").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("JSON", &["json"])
                    .pick_file()
                {
                    if let Err(e) = self.import_library(&path, true) {
                        self.status = format!("Import failed: {}", e);
                    } else {
                        self.status = "Import complete (replaced)".to_string();
                    }
                }
            }
            if ui.button("Import (Merge)").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("JSON", &["json"])
                    .pick_file()
                {
                    if let Err(e) = self.import_library(&path, false) {
                        self.status = format!("Import failed: {}", e);
                    } else {
                        self.status = "Import complete (merged)".to_string();
                    }
                }
            }
            if ui.button("Import CSV (Replace)").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("CSV", &["csv"])
                    .pick_file()
                {
                    if let Err(e) = self.import_library_csv(&path, true) {
                        self.status = format!("Import CSV failed: {}", e);
                    } else {
                        self.status = "Import CSV complete (replaced)".to_string();
                    }
                }
            }
            if ui.button("Import CSV (Merge)").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("CSV", &["csv"])
                    .pick_file()
                {
                    if let Err(e) = self.import_library_csv(&path, false) {
                        self.status = format!("Import CSV failed: {}", e);
                    } else {
                        self.status = "Import CSV complete (merged)".to_string();
                    }
                }
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        ui.heading("Snippets");
        ui.horizontal(|ui| {
            ui.label("Search:");
            ui.text_edit_singleline(&mut self.library_search);
        });
        ui.add_space(4.0);
        if let Some(idx) = self.selected_category {
            if let Some(cat) = self.categories.get(idx) {
                let snippets = self.library.get(cat).map(|v| v.as_slice()).unwrap_or(&[]);
                let search_words: Vec<&str> = self
                    .library_search
                    .split_whitespace()
                    .filter(|w| !w.is_empty())
                    .collect();
                let filtered: Vec<(usize, &Snippet)> = snippets
                    .iter()
                    .enumerate()
                    .filter(|(_, snip)| {
                        if search_words.is_empty() {
                            return true;
                        }
                        let trigger_lower = snip.trigger.to_lowercase();
                        let content_lower = snip.content.to_lowercase();
                        search_words.iter().all(|w| {
                            let wl = w.to_lowercase();
                            trigger_lower.contains(&wl) || content_lower.contains(&wl)
                        })
                    })
                    .collect();
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "Category: {} ({} snippets)",
                        cat,
                        if search_words.is_empty() {
                            snippets.len()
                        } else {
                            filtered.len()
                        }
                    ));
                    if ui.button("Add Snippet").clicked() {
                        self.snippet_editor_mode = Some(SnippetEditorMode::Add { category: cat.clone() });
                        self.snippet_editor_trigger.clear();
                        self.snippet_editor_content.clear();
                        self.snippet_editor_options.clear();
                        self.snippet_editor_category = cat.clone();
                        self.snippet_editor_profile = "Default".to_string();
                        self.snippet_editor_app_lock.clear();
                        self.snippet_editor_pinned = false;
                        self.snippet_editor_modal_open = true;
                    }
                });
                ui.add_space(4.0);
                if is_listener_running() {
                    ui.colored_label(egui::Color32::DARK_GREEN, "Hotstring listener active - type triggers in any app to expand");
                }
                egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                    for (display_idx, (orig_idx, snip)) in filtered.iter().enumerate() {
                        let i = *orig_idx;
                        let content_preview = if snip.content.len() > 60 {
                                format!("{}...", &snip.content[..60])
                            } else {
                                snip.content.clone()
                            };
                            let content_preview = content_preview.replace('\n', " ");
                            let app_lock = if snip.app_lock.is_empty() {
                                "all apps".to_string()
                            } else {
                                snip.app_lock.clone()
                            };
                        ui.horizontal(|ui| {
                            ui.label(format!("{}.", display_idx + 1));
                                ui.strong(format!("[{}]", snip.trigger));
                                ui.label("->");
                                ui.label(&content_preview);
                                ui.label(format!("({})", app_lock));
                                if ui.small_button("Edit").clicked() {
                                    self.snippet_editor_mode = Some(SnippetEditorMode::Edit {
                                        category: cat.clone(),
                                        snippet_idx: i,
                                    });
                                    self.snippet_editor_trigger = snip.trigger.clone();
                                    self.snippet_editor_content = snip.content.clone();
                                    self.snippet_editor_options = snip.options.clone();
                                    self.snippet_editor_category = snip.category.clone();
                                    self.snippet_editor_profile = snip.profile.clone();
                                    self.snippet_editor_app_lock = snip.app_lock.clone();
                                    self.snippet_editor_pinned = snip.is_pinned();
                                    self.snippet_editor_modal_open = true;
                                }
                                if ui.small_button("Delete").clicked() {
                                    self.snippet_delete_confirm = Some((cat.clone(), i));
                                    self.snippet_delete_dialog_open = true;
                                }
                            });
                        }
                    });
                }
        } else {
            ui.label("Select a category or load a library");
            ui.add_space(4.0);
            if !self.categories.is_empty() && ui.button("Add Snippet (no category selected)").clicked() {
                let cat = "General".to_string();
                self.snippet_editor_mode = Some(SnippetEditorMode::Add { category: cat.clone() });
                self.snippet_editor_trigger.clear();
                self.snippet_editor_content.clear();
                self.snippet_editor_options.clear();
                self.snippet_editor_category = cat;
                self.snippet_editor_profile = "Default".to_string();
                self.snippet_editor_app_lock.clear();
                self.snippet_editor_pinned = false;
                self.snippet_editor_modal_open = true;
            }
        }
    }

    fn ui_configuration_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Configuration");
        ui.add_space(8.0);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(false)
            .show(ui, |ui| {
        // F7: Global pause for expansion
        if ui.checkbox(&mut self.expansion_paused, "Pause expansion (F7)").changed() {
            set_expansion_paused(self.expansion_paused);
        }
        ui.add_space(4.0);
        ui.label("Tip: Run as normal user (not Administrator). UIPI blocks input from elevated apps to non-elevated apps like Sublime.");
        ui.add_space(8.0);

        ui.collapsing("Templates (F16-F20)", |ui| {
            ui.label("Placeholders: {date}, {time}, {time:fmt}, {clipboard}, {clip:1}-{clip:N}, {env:VAR}");
            ui.label("Date format (chrono strftime, e.g. %Y-%m-%d, %d/%m/%Y):");
            ui.add(egui::TextEdit::singleline(&mut self.template_date_format).desired_width(200.0));
            ui.label("Time format (chrono strftime, e.g. %H:%M, %I:%M %p):");
            ui.add(egui::TextEdit::singleline(&mut self.template_time_format).desired_width(200.0));
            if ui.button("Apply Templates").clicked() {
                self.sync_template_config();
                self.status = "Template settings applied".to_string();
            }
        });

        ui.collapsing("Sync (WebDAV)", |ui| {
            ui.label("WebDAV URL (e.g. https://webdav.example.com/library.json):");
            ui.text_edit_singleline(&mut self.sync_url);
            ui.label("Password:");
            ui.add(egui::TextEdit::singleline(&mut self.sync_password).password(true));

            let can_sync = !self.sync_url.is_empty()
                && !self.sync_password.is_empty()
                && !self.library_path.is_empty()
                && self.sync_rx.is_none();

            ui.horizontal(|ui| {
                if ui.add_enabled(can_sync, egui::Button::new("Push")).clicked() {
                    self.do_push_sync();
                }
                if ui.add_enabled(can_sync, egui::Button::new("Pull")).clicked() {
                    self.do_pull_sync();
                }
            });

            match &self.sync_status {
                SyncResult::Idle => {}
                SyncResult::InProgress => {
                    let _ = ui.label("Syncing...");
                }
                SyncResult::Success(msg) => {
                    let _ = ui.colored_label(egui::Color32::GREEN, msg);
                }
                SyncResult::Error(msg) => {
                    let _ = ui.colored_label(egui::Color32::RED, msg);
                }
            }
        });

        ui.collapsing("Discovery (F60-F69)", |ui| {
            ui.label("Harvest repeated phrases from typing and suggest as snippets.");
            if ui.checkbox(&mut self.discovery_enabled, "Enable Discovery").changed() {
                if self.discovery_enabled {
                    let config = self.build_discovery_config();
                    discovery::start(config);
                } else {
                    discovery::stop();
                }
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Threshold (repeats):");
                ui.add(egui::DragValue::new(&mut self.discovery_threshold).range(2..=10));
            });
            ui.horizontal(|ui| {
                ui.label("Lookback (min):");
                ui.add(egui::DragValue::new(&mut self.discovery_lookback).range(5..=240));
            });
            ui.horizontal(|ui| {
                ui.label("Min phrase length:");
                ui.add(egui::DragValue::new(&mut self.discovery_min_len).range(2..=20));
            });
            ui.horizontal(|ui| {
                ui.label("Max phrase length:");
                ui.add(egui::DragValue::new(&mut self.discovery_max_len).range(10..=100));
            });
            ui.label("Excluded apps (comma-separated):");
            ui.text_edit_singleline(&mut self.discovery_excluded_apps);
            ui.label("Excluded window titles (comma-separated; substring match):");
            ui.text_edit_singleline(&mut self.discovery_excluded_window_titles);
            if self.discovery_enabled {
                if ui.button("Apply Discovery changes").clicked() {
                    discovery::start(self.build_discovery_config());
                }
                ui.colored_label(egui::Color32::DARK_GREEN, "Discovery active - type repeated phrases to get suggestions");
            }
        });

        ui.collapsing("Ghost Suggestor (F43-F47)", |ui| {
            ui.label("Predictive overlay: type partial triggers to see suggestions. Tab to accept, Ctrl+Tab to cycle.");
            if ui.checkbox(&mut self.ghost_suggestor_enabled, "Enable Ghost Suggestor").changed() {
                ghost_suggestor::update_config(GhostSuggestorConfig {
                    enabled: self.ghost_suggestor_enabled,
                    debounce_ms: self.ghost_suggestor_debounce_ms,
                    offset_x: self.ghost_suggestor_offset_x,
                    offset_y: self.ghost_suggestor_offset_y,
                });
            }
            ui.horizontal(|ui| {
                ui.label("Debounce (ms):");
                ui.add(egui::DragValue::new(&mut self.ghost_suggestor_debounce_ms).range(20..=200));
            });
            ui.horizontal(|ui| {
                ui.label("Offset from caret (F46):");
                ui.add(egui::DragValue::new(&mut self.ghost_suggestor_offset_x).range(-100..=100));
                ui.add(egui::DragValue::new(&mut self.ghost_suggestor_offset_y).range(-100..=100));
            });
            if ui.button("Apply Ghost Suggestor").clicked() {
                ghost_suggestor::update_config(GhostSuggestorConfig {
                    enabled: self.ghost_suggestor_enabled,
                    debounce_ms: self.ghost_suggestor_debounce_ms,
                    offset_x: self.ghost_suggestor_offset_x,
                    offset_y: self.ghost_suggestor_offset_y,
                });
            }
            if self.ghost_suggestor_enabled {
                ui.colored_label(egui::Color32::DARK_GREEN, "Ghost Suggestor active - type partial triggers in any app");
            }
        });

        ui.collapsing("Ghost Follower (F48-F59)", |ui| {
            ui.label("Edge ribbon with pinned snippets. Double-click to insert.");
            if ui.checkbox(&mut self.ghost_follower_enabled, "Enable Ghost Follower").changed() {
                ghost_follower::update_config(self.build_ghost_follower_config());
            }
            ui.checkbox(&mut self.ghost_follower_hover_preview, "Hover preview (F53)");
            ui.horizontal(|ui| {
                ui.label("Collapse delay (s):");
                ui.add(egui::DragValue::new(&mut self.ghost_follower_collapse_delay_secs).range(0..=60));
            });
            ui.horizontal(|ui| {
                ui.label("Edge:");
                ui.radio_value(&mut self.ghost_follower_edge_right, true, "Right");
                ui.radio_value(&mut self.ghost_follower_edge_right, false, "Left");
            });
            ui.horizontal(|ui| {
                ui.label("Monitor (F49):");
                ui.radio_value(&mut self.ghost_follower_monitor_anchor, 0, "Primary");
                ui.radio_value(&mut self.ghost_follower_monitor_anchor, 1, "Secondary");
                ui.radio_value(&mut self.ghost_follower_monitor_anchor, 2, "Current");
            });
            if ui.button("Apply Ghost Follower").clicked() {
                ghost_follower::update_config(self.build_ghost_follower_config());
            }
            if self.ghost_follower_enabled {
                ui.colored_label(egui::Color32::DARK_GREEN, "Ghost Follower active - ribbon shows pinned snippets");
            }
        });

        ui.collapsing("Clipboard History (F38-F42)", |ui| {
            ui.label("Monitor clipboard and show in Ghost Follower. Right-click to promote as snippet.");
            ui.horizontal(|ui| {
                ui.label("Max depth:");
                ui.add(egui::DragValue::new(&mut self.clip_history_max_depth).range(5..=50));
            });
            if ui.button("Apply").clicked() {
                clipboard_history::update_config(ClipboardHistoryConfig {
                    enabled: true,
                    max_depth: self.clip_history_max_depth,
                });
            }
        });
            });
    }

    fn ui_script_library_tab(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) {
        if !self.script_library_loaded {
            self.script_library_loaded = true;
            let cfg = get_scripting_config();
            let base = dirs::config_dir()
                .unwrap_or_else(|| Path::new(".").into())
                .join("DigiCore");
            let lib_path = if cfg.js.library_paths.is_empty() {
                base.join(&cfg.js.library_path)
            } else {
                base.join(cfg.js.library_paths.first().unwrap_or(&String::new()))
            };
            if let Ok(content) = std::fs::read_to_string(&lib_path) {
                self.script_library_js_content = content.clone();
                set_global_library(content);
            } else {
                self.script_library_js_content = DEFAULT_GLOBAL_LIBRARY.to_string();
                set_global_library(self.script_library_js_content.clone());
            }
            if cfg.py.enabled {
                let py_path = base.join(&cfg.py.library_path);
                self.script_library_py_content = std::fs::read_to_string(&py_path)
                    .unwrap_or_else(|_| "# Global Python library for {py:...} tags\n".to_string());
            }
            if cfg.lua.enabled {
                let lua_path = base.join(&cfg.lua.library_path);
                self.script_library_lua_content = std::fs::read_to_string(&lua_path)
                    .unwrap_or_else(|_| "-- Global Lua library for {lua:...} tags\n".to_string());
            }
        }
        ui.heading("Script Library (F86)");
        ui.add_space(8.0);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(false)
            .show(ui, |ui| {
                ui.collapsing("Phase 7: {run:} Security", |ui| {
                    ui.checkbox(
                        &mut self.script_library_run_disabled,
                        "Disable {run:command} (recommended: keep checked for security)",
                    );
                    ui.label("Allowlist (when enabled):");
                    egui::ScrollArea::vertical()
                        .max_height(80.0)
                        .stick_to_bottom(false)
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut self.script_library_run_allowlist)
                                    .desired_width(500.0)
                                    .desired_rows(3),
                            );
                        });
                    ui.label("Comma-separated: python, cmd, C:\\Scripts\\, etc. Empty = block all.");
                    if ui.button("Save Run Settings").clicked() {
                        let mut cfg = get_scripting_config();
                        cfg.run.disabled = self.script_library_run_disabled;
                        cfg.run.allowlist = self.script_library_run_allowlist.clone();
                        set_scripting_config(cfg);
                        self.status = "Run settings saved.".to_string();
                    }
                });

                ui.add_space(8.0);

                ui.collapsing("Global JavaScript Library (scripts/global_library.js)", |ui| {
                    ui.label("Define reusable JS functions here. These are available in all {js:...} tags.");
                    ui.add_space(4.0);
                    let font_size = egui::TextStyle::Monospace.resolve(ui.style()).size;
                    let mut layouter = |ui: &egui::Ui, text: &str, wrap_width: f32| {
                        let mut job = highlight_js(text, font_size);
                        job.wrap.max_width = wrap_width;
                        ui.fonts(|f| f.layout_job(job))
                    };
                    egui::ScrollArea::vertical()
                        .max_height(350.0)
                        .stick_to_bottom(false)
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut self.script_library_js_content)
                                    .desired_width(500.0)
                                    .desired_rows(16)
                                    .font(egui::TextStyle::Monospace)
                                    .layouter(&mut layouter),
                            );
                        });
                    ui.add_space(8.0);
                    if ui.button("Save & Reload JS").clicked() {
                        let cfg = get_scripting_config();
                        let base = dirs::config_dir()
                            .unwrap_or_else(|| Path::new(".").into())
                            .join("DigiCore");
                        let lib_path = if cfg.js.library_paths.is_empty() {
                            base.join(&cfg.js.library_path)
                        } else {
                            base.join(cfg.js.library_paths.first().unwrap_or(&String::new()))
                        };
                        let _ = std::fs::create_dir_all(lib_path.parent().unwrap_or(Path::new(".")));
                        if let Err(e) = std::fs::write(&lib_path, &self.script_library_js_content) {
                            self.status = format!("Save failed: {}", e);
                        } else {
                            set_global_library(self.script_library_js_content.clone());
                            self.status = "Global Library Saved! JS hot-reloaded.".to_string();
                        }
                    }
                });

                let cfg = get_scripting_config();
                if cfg.py.enabled {
                    ui.add_space(8.0);
                    ui.collapsing(
                        format!("Global Python Library ({})", cfg.py.library_path),
                        |ui| {
                            ui.label("Define reusable Python functions for {py:...} tags. Enable in scripting.json.");
                            ui.add_space(4.0);
                            egui::ScrollArea::vertical()
                                .max_height(350.0)
                                .stick_to_bottom(false)
                                .show(ui, |ui| {
                                    ui.add(
                                        egui::TextEdit::multiline(&mut self.script_library_py_content)
                                            .desired_width(500.0)
                                            .desired_rows(16)
                                            .font(egui::TextStyle::Monospace),
                                    );
                                });
                            ui.add_space(8.0);
                            if ui.button("Save & Reload Python").clicked() {
                                let base = dirs::config_dir()
                                    .unwrap_or_else(|| Path::new(".").into())
                                    .join("DigiCore");
                                let lib_path = base.join(&cfg.py.library_path);
                                let _ = std::fs::create_dir_all(lib_path.parent().unwrap_or(Path::new(".")));
                                if let Err(e) = std::fs::write(&lib_path, &self.script_library_py_content) {
                                    self.status = format!("Save failed: {}", e);
                                } else {
                                    self.status = "Global Python Library saved.".to_string();
                                }
                            }
                        },
                    );
                }
                if cfg.lua.enabled {
                    ui.add_space(8.0);
                    ui.collapsing(
                        format!("Global Lua Library ({})", cfg.lua.library_path),
                        |ui| {
                            ui.label("Define reusable Lua functions for {lua:...} tags. Enable in scripting.json.");
                            ui.add_space(4.0);
                            egui::ScrollArea::vertical()
                                .max_height(350.0)
                                .stick_to_bottom(false)
                                .show(ui, |ui| {
                                    ui.add(
                                        egui::TextEdit::multiline(&mut self.script_library_lua_content)
                                            .desired_width(500.0)
                                            .desired_rows(16)
                                            .font(egui::TextStyle::Monospace),
                                    );
                                });
                            ui.add_space(8.0);
                            if ui.button("Save & Reload Lua").clicked() {
                                let base = dirs::config_dir()
                                    .unwrap_or_else(|| Path::new(".").into())
                                    .join("DigiCore");
                                let lib_path = base.join(&cfg.lua.library_path);
                                let _ = std::fs::create_dir_all(lib_path.parent().unwrap_or(Path::new(".")));
                                if let Err(e) = std::fs::write(&lib_path, &self.script_library_lua_content) {
                                    self.status = format!("Save failed: {}", e);
                                } else {
                                    self.status = "Global Lua Library saved.".to_string();
                                }
                            }
                        },
                    );
                }
            });
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
        if self.library.is_empty() {
            return;
        }
        if is_listener_running() {
            update_library(self.library.clone());
        } else if let Err(e) = start_listener(self.library.clone()) {
            self.status = format!("Hotstring failed to start: {}", e);
        }
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
        self.categories = self.library.keys().cloned().collect();
        self.categories.sort();
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
        self.categories = self.library.keys().cloned().collect();
        self.categories.sort();
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
        self.categories = self.library.keys().cloned().collect();
        self.categories.sort();
        self.selected_category = if self.categories.is_empty() {
            None
        } else {
            Some(0)
        };
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
                self.categories = self.library.keys().cloned().collect();
                self.categories.sort();
                self.selected_category = if self.categories.is_empty() {
                    None
                } else {
                    Some(0)
                };
                self.startup_sync_done = true;
                self.sync_hotstring_listener();
                return Ok(self.categories.len());
            }
        }

        let repo = JsonLibraryAdapter;
        let library = repo.load(path)?;
        self.library = library;
        self.categories = self.library.keys().cloned().collect();
        self.categories.sort();
        self.selected_category = if self.categories.is_empty() {
            None
        } else {
            Some(0)
        };
        self.startup_sync_done = true;
        self.sync_hotstring_listener();
        Ok(self.categories.len())
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
