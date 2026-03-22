//! Modal dialogs - snippet editor, delete confirm, variable input.
//!
//! SRP: each modal has a single responsibility.

use crate::{ClipViewContent, SnippetEditorMode, SnippetTestVarState, TextExpanderApp, SNIPPET_TEMPLATES};
use digicore_text_expander::application::clipboard_history;
use digicore_text_expander::application::template_processor;
use digicore_text_expander::application::variable_input;
use egui;

/// Render Variable Input viewport (F11: {var:}, {choice:}).
pub fn variable_input_viewport(
    ctx: &egui::Context,
    file_dialog: std::sync::Arc<dyn digicore_text_expander::ports::FileDialogPort>,
) {
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
        if let Some(r) = variable_input::render_viewport_modal(ctx, file_dialog.clone()) {
            *result_clone.lock().unwrap() = Some(r);
        }
    });
    let r = result.lock().unwrap().take();
    if let Some(r) = r {
        variable_input::process_viewport_result(r);
    }
}

/// Collect distinct profiles and options from library for dropdown suggestions.
/// Always includes default values "Work" and "*:" so they appear in the list.
fn collect_library_suggestions(library: &std::collections::HashMap<String, Vec<digicore_core::domain::Snippet>>) -> (Vec<String>, Vec<String>) {
    use std::collections::BTreeSet;
    let mut profiles: BTreeSet<String> = BTreeSet::new();
    let mut options: BTreeSet<String> = BTreeSet::new();
    profiles.insert("Work".to_string());
    options.insert("*:".to_string());
    for snippets in library.values() {
        for snip in snippets {
            if !snip.profile.trim().is_empty() {
                profiles.insert(snip.profile.trim().to_string());
            }
            if !snip.options.trim().is_empty() {
                options.insert(snip.options.trim().to_string());
            }
        }
    }
    (profiles.into_iter().collect(), options.into_iter().collect())
}

/// Render Snippet Editor modal (Add/Edit).
pub fn snippet_editor_modal(app: &mut TextExpanderApp, ctx: &egui::Context) {
    let mode = match &app.snippet_editor_mode {
        Some(m) => m.clone(),
        None => return,
    };
    let title = match &mode {
        SnippetEditorMode::Add { .. } => "Add Snippet",
        SnippetEditorMode::Edit { .. } => "Edit Snippet",
        SnippetEditorMode::Promote { .. } => "Promote to Snippet",
    };
    let (profile_suggestions, options_suggestions) = collect_library_suggestions(&app.library);
    let mut modal_open = app.snippet_editor_modal_open;
    let close_requested = std::sync::atomic::AtomicBool::new(false);
    egui::Window::new(title)
        .collapsible(false)
        .resizable(true)
        .default_width(400.0)
        .open(&mut modal_open)
        .show(ctx, |ui| {
            ui.label("Trigger (shortcut):");
            ui.add(egui::TextEdit::singleline(&mut app.snippet_editor_trigger).desired_width(300.0));
            ui.label("Profile (e.g., Work):");
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut app.snippet_editor_profile).desired_width(260.0));
                egui::ComboBox::from_id_salt("profile_combo")
                    .selected_text("\u{25BC}")
                    .width(36.0)
                    .show_ui(ui, |ui| {
                        for p in &profile_suggestions {
                            if ui.selectable_label(app.snippet_editor_profile == *p, p).clicked() {
                                app.snippet_editor_profile = p.clone();
                            }
                        }
                    });
            });
            ui.label("Options (e.g., *:):");
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut app.snippet_editor_options).desired_width(260.0));
                egui::ComboBox::from_id_salt("options_combo")
                    .selected_text("\u{25BC}")
                    .width(36.0)
                    .show_ui(ui, |ui| {
                        for o in &options_suggestions {
                            if ui.selectable_label(app.snippet_editor_options == *o, o).clicked() {
                                app.snippet_editor_options = o.clone();
                            }
                        }
                    });
            });
            ui.label("Category:");
            ui.add(egui::TextEdit::singleline(&mut app.snippet_editor_category).desired_width(300.0));
            ui.label("From template:");
            ui.horizontal(|ui| {
                let template_label = SNIPPET_TEMPLATES[app.snippet_editor_template_selected.min(SNIPPET_TEMPLATES.len().saturating_sub(1))].0;
                egui::ComboBox::from_id_salt("snippet_template")
                    .selected_text(template_label)
                    .width(300.0)
                    .show_ui(ui, |ui| {
                        for (i, (label, content)) in SNIPPET_TEMPLATES.iter().enumerate() {
                            if ui.selectable_label(app.snippet_editor_template_selected == i, *label).clicked() {
                                app.snippet_editor_template_selected = i;
                                if !content.is_empty() {
                                    app.snippet_editor_content = content.to_string();
                                }
                            }
                        }
                    });
            });
            ui.label("Content:");
            egui::ScrollArea::vertical()
                .max_height(200.0)
                .stick_to_bottom(false)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut app.snippet_editor_content)
                            .desired_width(300.0)
                            .desired_rows(6),
                    );
                });
            ui.label("App lock (comma-separated exe names, empty = all apps):");
            ui.add(egui::TextEdit::singleline(&mut app.snippet_editor_app_lock).desired_width(300.0));
            ui.checkbox(&mut app.snippet_editor_pinned, "Pinned (priority in search)");
            ui.checkbox(&mut app.snippet_editor_case_adaptive, "Case-Adaptive (auto-match Lower/Upper/Title)");
            ui.checkbox(&mut app.snippet_editor_case_sensitive, "Case-Sensitive Match (exact case match)");
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    app.snippet_editor_save_clicked = true;
                    close_requested.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                if ui.button("Cancel").clicked() {
                    close_requested.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                if ui.button("Preview Expansion").clicked() {
                    let content = app.snippet_editor_content.clone();
                    let vars = template_processor::collect_interactive_vars(&content);
                    if vars.is_empty() {
                        let current_clip = arboard::Clipboard::new().and_then(|mut c| c.get_text()).ok();
                        let clip_history: Vec<String> = clipboard_history::get_entries()
                            .iter()
                            .map(|e| e.content.clone())
                            .collect();
                        let result = template_processor::process_for_preview(
                            &content,
                            current_clip.as_deref(),
                            &clip_history,
                            None,
                        );
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
                            content,
                            vars,
                            values,
                            choice_indices,
                            checkbox_checked: std::collections::HashMap::new(),
                        });
                        app.snippet_test_var_modal_open = true;
                    }
                }
            });
        });
    app.snippet_editor_modal_open = modal_open;
    if close_requested.load(std::sync::atomic::Ordering::SeqCst) {
        app.snippet_editor_modal_open = false;
    }
    if !app.snippet_editor_modal_open {
        if app.snippet_editor_save_clicked {
            app.apply_snippet_editor_save();
        }
        app.snippet_editor_mode = None;
        app.snippet_editor_trigger.clear();
        app.snippet_editor_content.clear();
        app.snippet_editor_options = "*:".to_string();
        app.snippet_editor_category.clear();
        app.snippet_editor_profile = "Work".to_string();
        app.snippet_editor_app_lock.clear();
        app.snippet_editor_pinned = false;
        app.snippet_editor_case_adaptive = true;
        app.snippet_editor_case_sensitive = false;
        app.snippet_editor_save_clicked = false;
        app.snippet_editor_template_selected = 0;
    }
}

/// Render View Full Content modal (clipboard history or snippet library).
/// Buttons depend on source: Clipboard History -> "Promote to Snippet"; Snippet Library -> "Edit Snippet".
pub fn clip_view_content_modal(app: &mut TextExpanderApp, ctx: &egui::Context) {
    let view = match app.clip_view_content.as_ref() {
        Some(v) => v.clone(),
        None => return,
    };
    let content = match &view {
        ClipViewContent::ClipboardHistory { content } => content.clone(),
        ClipViewContent::SnippetLibrary { content, .. } => content.clone(),
    };
    let content_clone = content.clone();
    let close_requested = std::sync::atomic::AtomicBool::new(false);
    let promote_clicked = std::sync::atomic::AtomicBool::new(false);
    let edit_clicked = std::sync::atomic::AtomicBool::new(false);
    let close_ref = &close_requested;
    let promote_ref = &promote_clicked;
    let edit_ref = &edit_clicked;
    let mut open = true;
    let is_from_snippet_library = matches!(&view, ClipViewContent::SnippetLibrary { .. });
    egui::Window::new("View Full Content")
        .collapsible(false)
        .resizable(true)
        .default_width(500.0)
        .default_height(400.0)
        .open(&mut open)
        .show(ctx, |ui| {
            let mut display = content_clone.clone();
            egui::ScrollArea::vertical()
                .max_height(350.0)
                .stick_to_bottom(false)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut display)
                            .desired_width(f32::INFINITY)
                            .desired_rows(12)
                            .code_editor(),
                    );
                });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if is_from_snippet_library {
                    if ui.button("Edit Snippet").clicked() {
                        edit_ref.store(true, std::sync::atomic::Ordering::SeqCst);
                        close_ref.store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                } else {
                    if ui.button("Promote to Snippet").clicked() {
                        promote_ref.store(true, std::sync::atomic::Ordering::SeqCst);
                        close_ref.store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                }
                if ui.button("Close").clicked() {
                    close_ref.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            });
        });
    if close_requested.load(std::sync::atomic::Ordering::SeqCst) {
        if promote_clicked.load(std::sync::atomic::Ordering::SeqCst) {
            if let ClipViewContent::ClipboardHistory { content } = view {
                let cat = app
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
                app.snippet_editor_mode = Some(SnippetEditorMode::Promote {
                    category: cat.clone(),
                });
                app.snippet_editor_trigger = trigger;
                app.snippet_editor_content = content;
                app.snippet_editor_options = "*:".to_string();
                app.snippet_editor_category = cat;
                app.snippet_editor_profile = "Work".to_string();
                app.snippet_editor_template_selected = 0;
                app.snippet_editor_app_lock.clear();
                app.snippet_editor_pinned = false;
                app.snippet_editor_case_sensitive = false;
                app.snippet_editor_modal_open = true;
            }
        } else if edit_clicked.load(std::sync::atomic::Ordering::SeqCst) {
            if let ClipViewContent::SnippetLibrary {
                category,
                snippet_idx,
                trigger,
                content,
                options,
                snippet_category,
                profile,
                app_lock,
                pinned,
                case_sensitive,
            } = view
            {
                app.snippet_editor_mode = Some(SnippetEditorMode::Edit {
                    category,
                    snippet_idx,
                });
                app.snippet_editor_trigger = trigger;
                app.snippet_editor_content = content;
                app.snippet_editor_options = options;
                app.snippet_editor_category = snippet_category;
                app.snippet_editor_profile = profile;
                app.snippet_editor_app_lock = app_lock;
                app.snippet_editor_pinned = pinned;
                app.snippet_editor_case_sensitive = case_sensitive;
                app.snippet_editor_modal_open = true;
            }
        }
        app.clip_view_content = None;
    }
}

/// Render Delete Clipboard Item confirmation dialog.
pub fn clip_delete_confirm_dialog(app: &mut TextExpanderApp, ctx: &egui::Context) {
    let index = match app.clip_delete_confirm {
        Some(i) => i,
        None => return,
    };
    let entries = clipboard_history::get_entries();
    let content = entries.get(index).map(|e| e.content.as_str()).unwrap_or("");
    let preview = if content.len() > 30 {
        format!("{}...", &content[..30])
    } else {
        content.to_string()
    };
    let preview = preview.replace('\n', " ");

    let close_requested = std::sync::atomic::AtomicBool::new(false);
    let confirmed = std::sync::atomic::AtomicBool::new(false);
    let close_ref = &close_requested;
    let confirmed_ref = &confirmed;
    egui::Window::new("Confirm Deletion")
        .collapsible(false)
        .resizable(false)
        .open(&mut app.clip_delete_dialog_open)
        .show(ctx, |ui| {
            ui.label("Are you sure you want to delete this clipboard item?");
            ui.label(&preview);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Yes").clicked() {
                    confirmed_ref.store(true, std::sync::atomic::Ordering::SeqCst);
                    close_ref.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                if ui.button("No").clicked() {
                    close_ref.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            });
        });
    if close_requested.load(std::sync::atomic::Ordering::SeqCst) {
        app.clip_delete_dialog_open = false;
    }
    if !app.clip_delete_dialog_open {
        if confirmed.load(std::sync::atomic::Ordering::SeqCst) {
            clipboard_history::delete_entry_at(index);
            app.status = "Item deleted.".to_string();
        }
        app.clip_delete_confirm = None;
    }
}

/// Render Clear All Clipboard History confirmation dialog.
pub fn clip_clear_confirm_dialog(app: &mut TextExpanderApp, ctx: &egui::Context) {
    let close_requested = std::sync::atomic::AtomicBool::new(false);
    let confirmed = std::sync::atomic::AtomicBool::new(false);
    let close_ref = &close_requested;
    let confirmed_ref = &confirmed;
    egui::Window::new("Confirm Clear All")
        .collapsible(false)
        .resizable(false)
        .open(&mut app.clip_clear_confirm_open)
        .show(ctx, |ui| {
            ui.label("Clear all clipboard history?");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Yes").clicked() {
                    confirmed_ref.store(true, std::sync::atomic::Ordering::SeqCst);
                    close_ref.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                if ui.button("No").clicked() {
                    close_ref.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            });
        });
    if close_requested.load(std::sync::atomic::Ordering::SeqCst) {
        app.clip_clear_confirm_open = false;
    }
    if !app.clip_clear_confirm_open {
        if confirmed.load(std::sync::atomic::Ordering::SeqCst) {
            clipboard_history::clear_all();
            app.status = "Clipboard history cleared.".to_string();
        }
    }
}

/// Render Preview Expansion variable input modal (in-window, when content has {var:}, {choice:}, etc.).
pub fn snippet_test_var_modal(
    app: &mut TextExpanderApp,
    ctx: &egui::Context,
    file_dialog: std::sync::Arc<dyn digicore_text_expander::ports::FileDialogPort>,
) {
    let mut modal_open = app.snippet_test_var_modal_open;
    let ok_clicked = std::cell::Cell::new(false);
    let cancel_clicked = std::cell::Cell::new(false);
    let state = match &mut app.snippet_test_var_pending {
        Some(s) => s,
        None => return,
    };
    egui::Window::new("Enter Variable Values")
        .collapsible(false)
        .resizable(true)
        .default_width(360.0)
        .open(&mut modal_open)
        .show(ctx, |ui| {
            ui.label("Enter values for placeholders:");
            ui.add_space(8.0);
            for v in &state.vars {
                ui.label(format!("{}:", v.label));
                match &v.var_type {
                    template_processor::InteractiveVarType::Edit => {
                        let val = state.values.entry(v.tag.clone()).or_default();
                        ui.add(egui::TextEdit::singleline(val).desired_width(280.0));
                    }
                    template_processor::InteractiveVarType::Choice => {
                        let idx = state.choice_indices.entry(v.tag.clone()).or_insert(0);
                        let options: Vec<&str> = v.options.iter().map(|s| s.as_str()).collect();
                        egui::ComboBox::from_id_salt(v.tag.clone())
                            .selected_text(options.get(*idx).copied().unwrap_or(""))
                            .show_ui(ui, |ui| {
                                for (i, opt) in options.iter().enumerate() {
                                    if ui.selectable_label(*idx == i, *opt).clicked() {
                                        *idx = i;
                                        state.values.insert(v.tag.clone(), opt.to_string());
                                    }
                                }
                            });
                        if !v.options.is_empty() && state.values.get(&v.tag).is_none() {
                            state.values.insert(v.tag.clone(), v.options[0].clone());
                        }
                    }
                    template_processor::InteractiveVarType::Checkbox => {
                        let checked = state.checkbox_checked.entry(v.tag.clone()).or_insert(false);
                        let value = v.options.first().cloned().unwrap_or_default();
                        if ui.checkbox(checked, &v.label).changed() {
                            state.values.insert(
                                v.tag.clone(),
                                if *checked { value.clone() } else { String::new() },
                            );
                        }
                    }
                    template_processor::InteractiveVarType::DatePicker => {
                        let val = state.values.entry(v.tag.clone()).or_default();
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(val)
                                    .desired_width(200.0)
                                    .hint_text("YYYYMMDD"),
                            );
                            if ui.button("Today").clicked() {
                                *val = chrono::Local::now().format("%Y%m%d").to_string();
                            }
                        });
                    }
                    template_processor::InteractiveVarType::FilePicker => {
                        let val = state.values.entry(v.tag.clone()).or_default();
                        let filter_str = v.options.first().map(|s| s.as_str()).unwrap_or("All Files (*.*)");
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(val)
                                    .desired_width(220.0)
                                    .hint_text("Path to file"),
                            );
                            if ui.button("Browse...").clicked() {
                                digicore_text_expander::utils::with_file_filters(filter_str, |filters| {
                                    if let Some(path) = file_dialog.pick_file(filters) {
                                        *val = path.display().to_string();
                                    }
                                });
                            }
                        });
                    }
                }
                ui.add_space(4.0);
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() {
                    ok_clicked.set(true);
                }
                if ui.button("Cancel").clicked() {
                    cancel_clicked.set(true);
                }
            });
        });
    app.snippet_test_var_modal_open = modal_open;
    if cancel_clicked.get() {
        app.snippet_test_var_modal_open = false;
        app.snippet_test_var_pending = None;
    }
    if ok_clicked.get() {
        if let Some(state) = app.snippet_test_var_pending.take() {
            let mut values = state.values;
            for v in &state.vars {
                if let template_processor::InteractiveVarType::Choice = v.var_type {
                    let idx = state.choice_indices.get(&v.tag).copied().unwrap_or(0);
                    if let Some(opt) = v.options.get(idx) {
                        values.insert(v.tag.clone(), opt.clone());
                    }
                }
                if let template_processor::InteractiveVarType::Checkbox = v.var_type {
                    let checked = state.checkbox_checked.get(&v.tag).copied().unwrap_or(false);
                    let value = v.options.first().cloned().unwrap_or_default();
                    values.insert(v.tag.clone(), if checked { value } else { String::new() });
                }
            }
            let current_clip = arboard::Clipboard::new().and_then(|mut c| c.get_text()).ok();
            let clip_history: Vec<String> = clipboard_history::get_entries()
                .iter()
                .map(|e| e.content.clone())
                .collect();
            let result = template_processor::process_for_preview(
                &state.content,
                current_clip.as_deref(),
                &clip_history,
                Some(&values),
            );
            app.snippet_test_result = Some(result);
            app.snippet_test_result_modal_open = true;
        }
        app.snippet_test_var_modal_open = false;
    }
}

/// Render Preview Expansion result modal.
pub fn snippet_test_result_modal(app: &mut TextExpanderApp, ctx: &egui::Context) {
    let result = match &app.snippet_test_result {
        Some(r) => r.clone(),
        None => return,
    };
    let close_clicked = std::sync::atomic::AtomicBool::new(false);
    let close_ref = &close_clicked;
    egui::Window::new("Preview Expansion Result")
        .collapsible(false)
        .resizable(true)
        .default_width(500.0)
        .default_height(400.0)
        .open(&mut app.snippet_test_result_modal_open)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .max_height(350.0)
                .stick_to_bottom(false)
                .show(ui, |ui| {
                    let mut display = result.clone();
                    ui.add(
                        egui::TextEdit::multiline(&mut display)
                            .desired_width(f32::INFINITY)
                            .desired_rows(12)
                            .code_editor(),
                    );
                });
            ui.add_space(8.0);
            if ui.button("Close").clicked() {
                close_ref.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        });
    if close_clicked.load(std::sync::atomic::Ordering::SeqCst) {
        app.snippet_test_result_modal_open = false;
        app.snippet_test_result = None;
    }
}

/// Render Delete Snippet confirmation dialog.
pub fn delete_confirm_dialog(app: &mut TextExpanderApp, ctx: &egui::Context) {
    let (cat, idx) = match &app.snippet_delete_confirm {
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
        .open(&mut app.snippet_delete_dialog_open)
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
        app.snippet_delete_dialog_open = false;
    }
    if !app.snippet_delete_dialog_open {
        if confirmed.load(std::sync::atomic::Ordering::SeqCst) {
            if let Some(snippets) = app.library.get_mut(&cat) {
                if idx < snippets.len() {
                    snippets.remove(idx);
                    if snippets.is_empty() {
                        app.library.remove(&cat);
                    }
                    app.categories = app.library.keys().cloned().collect();
                    app.categories.sort();
                    if app.selected_category.map_or(false, |i| i >= app.categories.len()) {
                        app.selected_category = None;
                    }
                    app.sync_hotstring_listener();
                    if let Err(e) = app.try_save_library() {
                        app.status = format!("Save failed: {}", e);
                    } else {
                        app.status = "Snippet deleted".to_string();
                    }
                }
            }
        }
        app.snippet_delete_confirm = None;
    }
}
