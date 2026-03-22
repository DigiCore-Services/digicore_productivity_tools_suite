//! Clipboard History tab - Real-time clipboard history (F38-F42).
//!
//! Displays table with #, Content Preview, App, Window Title, Length.
//! Right-click context menu: Copy to Clipboard, View Full Content, Delete Item,
//! Promote to Snippet, Clear All History. Parity with AHK implementation.

use crate::TextExpanderApp;
use digicore_text_expander::application::clipboard_history::{self, ClipboardHistoryConfig};
use digicore_core::domain::entities::clipboard_entry::ClipEntry;
use digicore_text_expander::utils::truncate_for_display;
use egui;

/// Render the Clipboard History tab content.
pub fn render(app: &mut TextExpanderApp, _ctx: &egui::Context, ui: &mut egui::Ui) {
    let depth = app.clip_history_max_depth;
    ui.heading(format!("Real-time Clipboard History (Last {}):", depth));
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        if ui.button("Refresh").clicked() {
            clipboard_history::update_config(ClipboardHistoryConfig {
                enabled: true,
                max_depth: app.clip_history_max_depth,
            });
        }
        if ui.button("Clear All History").clicked() {
            app.clip_clear_confirm_open = true;
        }
        if !clipboard_history::is_enabled() {
            ui.colored_label(egui::Color32::YELLOW, "Clipboard monitoring is off. Enable in Configuration Settings tab.");
        }
    });
    ui.add_space(4.0);

    let entries = clipboard_history::get_entries();

    egui::ScrollArea::vertical()
        .max_height(400.0)
        .stick_to_bottom(false)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("#");
                ui.strong("Content Preview");
                ui.strong("App");
                ui.strong("Window Title");
                ui.strong("Length");
            });
            ui.separator();

            for (i, entry) in entries.iter().enumerate() {
                render_entry_row(ui, i, i + 1, entry, app);
            }
        });
}

fn render_entry_row(
    ui: &mut egui::Ui,
    index: usize,
    num: usize,
    entry: &ClipEntry,
    app: &mut TextExpanderApp,
) {
    let content_preview = if entry.content.len() > 40 {
        format!("{}...", &entry.content[..40])
    } else {
        entry.content.clone()
    };
    let content_preview = content_preview.replace('\n', " ");

    let app_display = if entry.process_name.is_empty() {
        "(unknown)".to_string()
    } else {
        truncate_for_display(&entry.process_name, 20)
    };
    let title_display = if entry.window_title.is_empty() {
        "(unknown)".to_string()
    } else {
        truncate_for_display(&entry.window_title, 30)
    };

    let content = entry.content.clone();
    let row_response = ui.horizontal(|ui| {
        ui.label(format!("{}", num));
        ui.label(&content_preview);
        ui.label(&app_display);
        ui.label(&title_display);
        ui.label(format!("{}", entry.content.len()));
    });
    let row_id = egui::Id::new(("clip_row", index));
    let response = ui.interact(
        row_response.response.rect,
        row_id,
        egui::Sense::click(),
    );
    response.context_menu(|ui| {
        if ui.button("Copy to Clipboard").clicked() {
            if let Ok(mut clip) = arboard::Clipboard::new() {
                if clip.set_text(&content).is_ok() {
                    app.status = format!("Copied item #{} to clipboard!", num);
                }
            }
            ui.close_menu();
        }
        if ui.button("View Full Content").clicked() {
            app.clip_view_content = Some(crate::ClipViewContent::ClipboardHistory {
                content: content.clone(),
            });
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
            app.snippet_editor_mode = Some(crate::SnippetEditorMode::Promote {
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
