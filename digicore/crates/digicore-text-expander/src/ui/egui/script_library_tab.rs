//! Script Library tab - Global JS/Python/Lua libraries and {run:} security (F86).
//!
//! Single responsibility: render script library editor and wire to scripting service.

use crate::TextExpanderApp;
use digicore_text_expander::application::js_syntax_highlighter::highlight_js;
use digicore_text_expander::application::scripting::{
    get_scripting_config, set_global_library, set_scripting_config,
};
use dirs;
use egui;
use std::path::Path;

/// Default Global Script Library content. Used when file is missing.
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

/// Render the Script Library tab content.
pub fn render(app: &mut TextExpanderApp, _ctx: &egui::Context, ui: &mut egui::Ui) {
    if !app.script_library_loaded {
        app.script_library_loaded = true;
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
            app.script_library_js_content = content.clone();
            set_global_library(content);
        } else {
            app.script_library_js_content = DEFAULT_GLOBAL_LIBRARY.to_string();
            set_global_library(app.script_library_js_content.clone());
        }
        if cfg.py.enabled {
            let py_path = base.join(&cfg.py.library_path);
            app.script_library_py_content = std::fs::read_to_string(&py_path)
                .unwrap_or_else(|_| "# Global Python library for {py:...} tags\n".to_string());
        }
        if cfg.lua.enabled {
            let lua_path = base.join(&cfg.lua.library_path);
            app.script_library_lua_content = std::fs::read_to_string(&lua_path)
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
                    &mut app.script_library_run_disabled,
                    "Disable {run:command} (recommended: keep checked for security)",
                );
                ui.label("Allowlist (when enabled):");
                egui::ScrollArea::vertical()
                    .max_height(80.0)
                    .stick_to_bottom(false)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut app.script_library_run_allowlist)
                                .desired_width(500.0)
                                .desired_rows(3),
                        );
                    });
                ui.label("Comma-separated: python, cmd, C:\\Scripts\\, etc. Empty = block all.");
                if ui.button("Save Run Settings").clicked() {
                    let mut cfg = get_scripting_config();
                    cfg.run.disabled = app.script_library_run_disabled;
                    cfg.run.allowlist = app.script_library_run_allowlist.clone();
                    set_scripting_config(cfg);
                    app.status = "Run settings saved.".to_string();
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
                            egui::TextEdit::multiline(&mut app.script_library_js_content)
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
                    if let Err(e) = std::fs::write(&lib_path, &app.script_library_js_content) {
                        app.status = format!("Save failed: {}", e);
                    } else {
                        set_global_library(app.script_library_js_content.clone());
                        app.status = "Global Library Saved! JS hot-reloaded.".to_string();
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
                                    egui::TextEdit::multiline(&mut app.script_library_py_content)
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
                            if let Err(e) = std::fs::write(&lib_path, &app.script_library_py_content) {
                                app.status = format!("Save failed: {}", e);
                            } else {
                                app.status = "Global Python Library saved.".to_string();
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
                                    egui::TextEdit::multiline(&mut app.script_library_lua_content)
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
                            if let Err(e) = std::fs::write(&lib_path, &app.script_library_lua_content) {
                                app.status = format!("Save failed: {}", e);
                            } else {
                                app.status = "Global Lua Library saved.".to_string();
                            }
                        }
                    },
                );
            }
        });
}
