//! Configuration tab - Templates, Sync, Discovery, Ghost Suggestor/Follower, Clipboard History.
//!
//! Single responsibility: render configuration UI and wire to application services.

use crate::TextExpanderApp;
use digicore_text_expander::application::clipboard_history::{self, ClipboardHistoryConfig};
use digicore_text_expander::application::discovery;
use digicore_text_expander::application::expansion_engine::set_expansion_paused;
use digicore_text_expander::application::ghost_follower;
use digicore_text_expander::application::ghost_suggestor::{self, GhostSuggestorConfig};
use digicore_text_expander::services::sync_service::SyncResult;
use egui;

/// Render the Configuration tab content.
pub fn render(app: &mut TextExpanderApp, ui: &mut egui::Ui) {
    ui.heading("Configuration");
    ui.add_space(8.0);

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(false)
        .show(ui, |ui| {
            // F7: Global pause for expansion
            if ui.checkbox(&mut app.expansion_paused, "Pause expansion (F7)").changed() {
                set_expansion_paused(app.expansion_paused);
            }
            ui.add_space(4.0);
            ui.label("Tip: Run as normal user (not Administrator). UIPI blocks input from elevated apps to non-elevated apps like Sublime.");
            ui.add_space(8.0);

            ui.collapsing("Templates (F16-F20)", |ui| {
                ui.label("Placeholders: {date}, {time}, {time:fmt}, {clipboard}, {clip:1}-{clip:N}, {env:VAR}");
                ui.label("Date format (chrono strftime, e.g. %Y-%m-%d, %d/%m/%Y):");
                ui.add(egui::TextEdit::singleline(&mut app.template_date_format).desired_width(200.0));
                ui.label("Time format (chrono strftime, e.g. %H:%M, %I:%M %p):");
                ui.add(egui::TextEdit::singleline(&mut app.template_time_format).desired_width(200.0));
                if ui.button("Apply Templates").clicked() {
                    app.sync_template_config();
                    app.status = "Template settings applied".to_string();
                }
            });

            ui.collapsing("Sync (WebDAV)", |ui| {
                ui.label("WebDAV URL (e.g. https://webdav.example.com/library.json):");
                ui.text_edit_singleline(&mut app.sync_url);
                ui.label("Password:");
                ui.add(egui::TextEdit::singleline(&mut app.sync_password).password(true));

                let can_sync = !app.sync_url.is_empty()
                    && !app.sync_password.is_empty()
                    && !app.library_path.is_empty()
                    && app.sync_rx.is_none();

                ui.horizontal(|ui| {
                    if ui.add_enabled(can_sync, egui::Button::new("Push")).clicked() {
                        app.do_push_sync();
                    }
                    if ui.add_enabled(can_sync, egui::Button::new("Pull")).clicked() {
                        app.do_pull_sync();
                    }
                });

                match &app.sync_status {
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
                if ui.checkbox(&mut app.discovery_enabled, "Enable Discovery").changed() {
                    if app.discovery_enabled {
                        let config = app.build_discovery_config();
                        discovery::start(config);
                    } else {
                        discovery::stop();
                    }
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Threshold (repeats):");
                    ui.add(egui::DragValue::new(&mut app.discovery_threshold).range(2..=10));
                });
                ui.horizontal(|ui| {
                    ui.label("Lookback (min):");
                    ui.add(egui::DragValue::new(&mut app.discovery_lookback).range(5..=240));
                });
                ui.horizontal(|ui| {
                    ui.label("Min phrase length:");
                    ui.add(egui::DragValue::new(&mut app.discovery_min_len).range(2..=20));
                });
                ui.horizontal(|ui| {
                    ui.label("Max phrase length:");
                    ui.add(egui::DragValue::new(&mut app.discovery_max_len).range(10..=100));
                });
                ui.label("Excluded apps (comma-separated):");
                ui.text_edit_singleline(&mut app.discovery_excluded_apps);
                ui.label("Excluded window titles (comma-separated; substring match):");
                ui.text_edit_singleline(&mut app.discovery_excluded_window_titles);
                if app.discovery_enabled {
                    if ui.button("Apply Discovery changes").clicked() {
                        discovery::start(app.build_discovery_config());
                    }
                    ui.colored_label(egui::Color32::DARK_GREEN, "Discovery active - type repeated phrases to get suggestions");
                }
            });

            ui.collapsing("Ghost Suggestor (F43-F47)", |ui| {
                ui.label("Predictive overlay: type partial triggers to see suggestions. Tab to accept, Ctrl+Tab to cycle. Create Snippet, Ignore, Cancel buttons.");
                if ui.checkbox(&mut app.ghost_suggestor_enabled, "Enable Ghost Suggestor").changed() {
                    ghost_suggestor::update_config(GhostSuggestorConfig {
                        enabled: app.ghost_suggestor_enabled,
                        debounce_ms: app.ghost_suggestor_debounce_ms,
                        display_duration_secs: app.ghost_suggestor_display_secs,
                        snooze_duration_mins: 5,
                        offset_x: app.ghost_suggestor_offset_x,
                        offset_y: app.ghost_suggestor_offset_y,
                    });
                }
                ui.horizontal(|ui| {
                    ui.label("Debounce (ms):");
                    ui.add(egui::DragValue::new(&mut app.ghost_suggestor_debounce_ms).range(20..=200));
                });
                ui.horizontal(|ui| {
                    ui.label("Display duration (sec, 0=no auto-hide):");
                    ui.add(egui::DragValue::new(&mut app.ghost_suggestor_display_secs).range(0..=120));
                });
                ui.horizontal(|ui| {
                    ui.label("Offset from caret (F46):");
                    ui.add(egui::DragValue::new(&mut app.ghost_suggestor_offset_x).range(-100..=100));
                    ui.add(egui::DragValue::new(&mut app.ghost_suggestor_offset_y).range(-100..=100));
                });
                if ui.button("Apply Ghost Suggestor").clicked() {
                    ghost_suggestor::update_config(GhostSuggestorConfig {
                        enabled: app.ghost_suggestor_enabled,
                        debounce_ms: app.ghost_suggestor_debounce_ms,
                        display_duration_secs: app.ghost_suggestor_display_secs,
                        snooze_duration_mins: 5,
                        offset_x: app.ghost_suggestor_offset_x,
                        offset_y: app.ghost_suggestor_offset_y,
                    });
                }
                if app.ghost_suggestor_enabled {
                    ui.colored_label(egui::Color32::DARK_GREEN, "Ghost Suggestor active - type partial triggers in any app");
                }
            });

            ui.collapsing("Ghost Follower (F48-F59)", |ui| {
                ui.label("Edge ribbon with pinned snippets. Double-click to insert.");
                if ui.checkbox(&mut app.ghost_follower_enabled, "Enable Ghost Follower").changed() {
                    ghost_follower::update_config(app.build_ghost_follower_config());
                }
                ui.checkbox(&mut app.ghost_follower_hover_preview, "Hover preview (F53)");
                ui.horizontal(|ui| {
                    ui.label("Collapse delay (s):");
                    ui.add(egui::DragValue::new(&mut app.ghost_follower_collapse_delay_secs).range(0..=60));
                });
                ui.horizontal(|ui| {
                    ui.label("Edge:");
                    ui.radio_value(&mut app.ghost_follower_edge_right, true, "Right");
                    ui.radio_value(&mut app.ghost_follower_edge_right, false, "Left");
                });
                ui.horizontal(|ui| {
                    ui.label("Monitor (F49):");
                    ui.radio_value(&mut app.ghost_follower_monitor_anchor, 0, "Primary");
                    ui.radio_value(&mut app.ghost_follower_monitor_anchor, 1, "Secondary");
                    ui.radio_value(&mut app.ghost_follower_monitor_anchor, 2, "Current");
                });
                if ui.button("Apply Ghost Follower").clicked() {
                    ghost_follower::update_config(app.build_ghost_follower_config());
                }
                if app.ghost_follower_enabled {
                    ui.colored_label(egui::Color32::DARK_GREEN, "Ghost Follower active - ribbon shows pinned snippets");
                }
            });

            ui.collapsing("Clipboard History (F38-F42)", |ui| {
                ui.label("Monitor clipboard and show in Ghost Follower. Right-click to promote as snippet.");
                ui.horizontal(|ui| {
                    ui.label("Max depth (Clip History Depth):");
                    ui.add(egui::DragValue::new(&mut app.clip_history_max_depth).range(5..=100));
                });
                if ui.button("Apply").clicked() {
                    clipboard_history::update_config(ClipboardHistoryConfig {
                        enabled: true,
                        max_depth: app.clip_history_max_depth,
                    });
                }
            });
        });
}
