//! Library tab - Snippet Explorer (F76).
//!
//! Single responsibility: render library path, import/export, category snippets,
//! search filter, add/edit/delete snippet actions.

use crate::{SnippetEditorMode, SnippetTestVarState, TextExpanderApp};
use digicore_core::domain::Snippet;
use digicore_text_expander::application::clipboard_history;
use digicore_text_expander::application::template_processor;
use digicore_text_expander::drivers::hotstring::is_listener_running;
use egui;

/// Render the Library tab content.
pub fn render(app: &mut TextExpanderApp, _ctx: &egui::Context, ui: &mut egui::Ui) {
    let file_dialog = app.file_dialog();
    ui.heading("DigiCore Text Expander");
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.label("Library path:");
        ui.text_edit_singleline(&mut app.library_path);
        if ui.button("Load").clicked() {
            match app.try_load_library() {
                Ok(n) => app.status = format!("Loaded {} categories", n),
                Err(e) => app.status = format!("Load failed: {}", e),
            }
        }
    });
    ui.horizontal(|ui| {
        if ui.button("Export JSON").clicked() {
            if let Some(path) = file_dialog.save_file(&[("JSON", &["json"][..])], "text_expansion_library.json")
            {
                if let Err(e) = app.export_library_json(&path) {
                    app.status = format!("Export failed: {}", e);
                } else {
                    app.status = format!("Exported to {}", path.display());
                }
            }
        }
        if ui.button("Export CSV").clicked() {
            if let Some(path) = file_dialog.save_file(&[("CSV", &["csv"][..])], "snippets.csv")
            {
                if let Err(e) = app.export_library_csv(&path) {
                    app.status = format!("Export failed: {}", e);
                } else {
                    app.status = format!("Exported to {}", path.display());
                }
            }
        }
        if ui.button("Import (Replace)").clicked() {
            if let Some(path) = file_dialog.pick_file(&[("JSON", &["json"][..])])
            {
                if let Err(e) = app.import_library(&path, true) {
                    app.status = format!("Import failed: {}", e);
                } else {
                    app.status = "Import complete (replaced)".to_string();
                }
            }
        }
        if ui.button("Import (Merge)").clicked() {
            if let Some(path) = file_dialog.pick_file(&[("JSON", &["json"][..])])
            {
                if let Err(e) = app.import_library(&path, false) {
                    app.status = format!("Import failed: {}", e);
                } else {
                    app.status = "Import complete (merged)".to_string();
                }
            }
        }
        if ui.button("Import CSV (Replace)").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("CSV", &["csv"])
                .pick_file()
            {
                if let Err(e) = app.import_library_csv(&path, true) {
                    app.status = format!("Import CSV failed: {}", e);
                } else {
                    app.status = "Import CSV complete (replaced)".to_string();
                }
            }
        }
        if ui.button("Import CSV (Merge)").clicked() {
            if let Some(path) = file_dialog.pick_file(&[("CSV", &["csv"][..])])
            {
                if let Err(e) = app.import_library_csv(&path, false) {
                    app.status = format!("Import CSV failed: {}", e);
                } else {
                    app.status = "Import CSV complete (merged)".to_string();
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
        ui.text_edit_singleline(&mut app.library_search);
    });
    ui.add_space(4.0);
    // (snippet, category, idx_in_category) for Edit/Delete - owned to avoid borrow conflict with ui closure
    let snippets_with_meta: Vec<(Snippet, String, usize)> = if let Some(idx) = app.selected_category {
        if let Some(cat) = app.categories.get(idx) {
            app.library
                .get(cat)
                .map(|v| {
                    v.iter()
                        .enumerate()
                        .map(|(i, s)| (s.clone(), cat.clone(), i))
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        }
    } else {
        // ALL: collect (snippet, category, idx_in_category) from all categories
        app.library
            .iter()
            .flat_map(|(cat, snips)| {
                snips
                    .iter()
                    .enumerate()
                    .map(move |(i, s)| (s.clone(), cat.clone(), i))
            })
            .collect()
    };
    let search_words: Vec<String> = app
        .library_search
        .split_whitespace()
        .filter(|w| !w.is_empty())
        .map(|w| w.to_string())
        .collect();
    let mut filtered: Vec<&(Snippet, String, usize)> = snippets_with_meta
        .iter()
        .filter(|(snip, _, _)| {
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
    // Pinned snippets always at top (per category when selected, or all pinned first when viewing All)
    filtered.sort_by_key(|(snip, _, _)| !snip.is_pinned());
    if !snippets_with_meta.is_empty() {
        let cat_label = app
            .selected_category
            .and_then(|i| app.categories.get(i))
            .cloned()
            .unwrap_or_else(|| "All".to_string());
        let add_cat = app
            .selected_category
            .and_then(|i| app.categories.get(i))
            .cloned()
            .unwrap_or_else(|| "General".to_string());
        ui.horizontal(|ui| {
            ui.label(format!(
                "Category: {} ({} snippets)",
                &cat_label,
                if search_words.is_empty() {
                    snippets_with_meta.len()
                } else {
                    filtered.len()
                }
            ));
            if ui.button("Add Snippet").clicked() {
                app.snippet_editor_mode = Some(SnippetEditorMode::Add {
                    category: add_cat.clone(),
                });
                app.snippet_editor_trigger.clear();
                app.snippet_editor_content.clear();
                app.snippet_editor_options = "*:".to_string();
                app.snippet_editor_category = add_cat;
                app.snippet_editor_profile = "Work".to_string();
                app.snippet_editor_app_lock.clear();
                app.snippet_editor_pinned = false;
                app.snippet_editor_template_selected = 0;
                app.snippet_editor_modal_open = true;
            }
        });
        ui.add_space(4.0);
        if is_listener_running() {
            ui.colored_label(
                egui::Color32::DARK_GREEN,
                "Hotstring listener active - type triggers in any app to expand",
            );
        }
        // Collect row data into owned values to avoid borrow conflicts in context menu
        let row_data: Vec<_> = filtered
            .iter()
            .enumerate()
            .map(|(display_idx, (snip, cat, i))| {
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
                (
                    display_idx,
                    cat.clone(),
                    *i,
                    snip.trigger.clone(),
                    content_preview,
                    app_lock,
                    snip.content.clone(),
                    snip.is_pinned(),
                    snip.options.clone(),
                    snip.category.clone(),
                    snip.profile.clone(),
                    snip.app_lock.clone(),
                )
            })
            .collect();
        egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
            for (
                display_idx,
                cat,
                i_val,
                trigger,
                content_preview,
                app_lock,
                content_clone,
                is_pinned,
                options,
                snip_category,
                profile,
                snip_app_lock,
            ) in row_data
            {
                let row_response = ui.horizontal(|ui| {
                    ui.label(format!("{}.", display_idx + 1));
                    if is_pinned {
                        ui.label(egui::RichText::new("\u{2605}").color(egui::Color32::from_rgb(255, 215, 0)));
                    }
                    ui.strong(format!("[{}]", trigger));
                    ui.label("->");
                    ui.label(&content_preview);
                    ui.label(format!("({})", app_lock));
                    if ui.small_button("Edit").clicked() {
                        app.snippet_editor_mode = Some(SnippetEditorMode::Edit {
                            category: cat.clone(),
                            snippet_idx: i_val,
                        });
                        app.snippet_editor_trigger = trigger.clone();
                        app.snippet_editor_content = content_clone.clone();
                        app.snippet_editor_options = options.clone();
                        app.snippet_editor_category = snip_category.clone();
                        app.snippet_editor_profile = profile.clone();
                        app.snippet_editor_app_lock = snip_app_lock.clone();
                        app.snippet_editor_pinned = is_pinned;
                        app.snippet_editor_modal_open = true;
                    }
                    if ui.small_button("Delete").clicked() {
                        app.snippet_delete_confirm = Some((cat.clone(), i_val));
                        app.snippet_delete_dialog_open = true;
                    }
                });
                let row_id = egui::Id::new(("snippet_row", cat.as_str(), i_val));
                let response = ui.interact(
                    row_response.response.rect,
                    row_id,
                    egui::Sense::click(),
                );
                response.context_menu(|ui| {
                    if ui.button("View Full Snippet Content").clicked() {
                        app.clip_view_content = Some(crate::ClipViewContent::SnippetLibrary {
                            category: cat.clone(),
                            snippet_idx: i_val,
                            trigger: trigger.clone(),
                            content: content_clone.clone(),
                            options: options.clone(),
                            snippet_category: snip_category.clone(),
                            profile: profile.clone(),
                            app_lock: snip_app_lock.clone(),
                            pinned: is_pinned,
                        });
                        ui.close_menu();
                    }
                    if ui
                        .button(if is_pinned {
                            "Unpin Snippet"
                        } else {
                            "Pin Snippet"
                        })
                        .clicked()
                    {
                        if let Some(snippets) = app.library.get_mut(&cat) {
                            if let Some(s) = snippets.get_mut(i_val) {
                                s.pinned = if is_pinned {
                                    "false".to_string()
                                } else {
                                    "true".to_string()
                                };
                                app.sync_hotstring_listener();
                                let _ = app.try_save_library();
                                app.status = if is_pinned {
                                    "Snippet unpinned".to_string()
                                } else {
                                    "Snippet pinned".to_string()
                                };
                            }
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Edit Snippet").clicked() {
                        app.snippet_editor_mode = Some(SnippetEditorMode::Edit {
                            category: cat.clone(),
                            snippet_idx: i_val,
                        });
                        app.snippet_editor_trigger = trigger.clone();
                        app.snippet_editor_content = content_clone.clone();
                        app.snippet_editor_options = options.clone();
                        app.snippet_editor_category = snip_category.clone();
                        app.snippet_editor_profile = profile.clone();
                        app.snippet_editor_app_lock = snip_app_lock.clone();
                        app.snippet_editor_pinned = is_pinned;
                        app.snippet_editor_modal_open = true;
                        ui.close_menu();
                    }
                    if ui.button("Preview Snippet").clicked() {
                        let content = content_clone.clone();
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
                        ui.close_menu();
                    }
                    if ui.button("Copy Full Content to Clipboard").clicked() {
                        if let Ok(mut clip) = arboard::Clipboard::new() {
                            if clip.set_text(&content_clone).is_ok() {
                                app.status = "Copied snippet content to clipboard!".to_string();
                            }
                        }
                        ui.close_menu();
                    }
                    if ui.button("Delete Snippet").clicked() {
                        app.snippet_delete_confirm = Some((cat.clone(), i_val));
                        app.snippet_delete_dialog_open = true;
                        ui.close_menu();
                    }
                });
            }
        });
    } else {
        ui.label("Select a category or load a library");
        ui.add_space(4.0);
        if !app.categories.is_empty() && ui.button("Add Snippet (no category selected)").clicked() {
            let cat = "General".to_string();
            app.snippet_editor_mode = Some(SnippetEditorMode::Add { category: cat.clone() });
            app.snippet_editor_trigger.clear();
            app.snippet_editor_content.clear();
            app.snippet_editor_options = "*:".to_string();
            app.snippet_editor_category = cat;
            app.snippet_editor_profile = "Work".to_string();
            app.snippet_editor_app_lock.clear();
            app.snippet_editor_pinned = false;
            app.snippet_editor_template_selected = 0;
            app.snippet_editor_modal_open = true;
        }
    }
}
