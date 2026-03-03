//! VariableInputModal - collects user input for {var:}, {choice:} before expansion.
//!
//! Used when snippet content has interactive placeholders. Main thread polls
//! take_pending_expansion() and shows modal; on OK sends processed content.
//! Viewport modal is always-on-top so user sees it over any app (Sublime, Cursor, etc.).

use crate::application::template_processor::{self, InteractiveVarType};
use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::Mutex;

/// Result when user dismisses the variable input viewport.
pub enum ViewportModalResult {
    Ok,
    Cancel,
}

/// State for the always-on-top variable input viewport.
pub struct ViewportModalState {
    pub content: String,
    pub vars: Vec<template_processor::InteractiveVar>,
    pub values: HashMap<String, String>,
    pub choice_indices: HashMap<String, usize>,
    /// Checkbox checked state: tag -> bool
    pub checkbox_checked: HashMap<String, bool>,
    /// Window that had focus when trigger was typed (restore before paste).
    pub target_hwnd: Option<isize>,
    pub response_tx: Option<Sender<(Option<String>, Option<isize>)>>,
}

static VIEWPORT_MODAL: Mutex<Option<ViewportModalState>> = Mutex::new(None);

/// Set viewport modal state (called when taking pending expansion).
pub fn set_viewport_modal(state: ViewportModalState) {
    if let Ok(mut g) = VIEWPORT_MODAL.lock() {
        *g = Some(state);
    }
}

/// Check if viewport modal is active.
pub fn has_viewport_modal() -> bool {
    if let Ok(g) = VIEWPORT_MODAL.lock() {
        g.is_some()
    } else {
        false
    }
}

/// Get viewport modal state for display (e.g. Tauri frontend). Does not consume.
pub fn get_viewport_modal_display() -> Option<(
    String,
    Vec<template_processor::InteractiveVar>,
    HashMap<String, String>,
    HashMap<String, usize>,
    HashMap<String, bool>,
)> {
    if let Ok(g) = VIEWPORT_MODAL.lock() {
        if let Some(ref s) = *g {
            return Some((
                s.content.clone(),
                s.vars.clone(),
                s.values.clone(),
                s.choice_indices.clone(),
                s.checkbox_checked.clone(),
            ));
        }
    }
    None
}

/// Render the variable input UI and return result when OK/Cancel clicked.
#[cfg(feature = "gui-egui")]
pub fn render_viewport_modal(
    ctx: &egui::Context,
    file_dialog: std::sync::Arc<dyn crate::ports::FileDialogPort>,
) -> Option<ViewportModalResult> {
    use egui;
    use std::cell::RefCell;
    let result = RefCell::new(None);
    if let Ok(mut g) = VIEWPORT_MODAL.lock() {
        if let Some(ref mut state) = *g {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Snippet Input Required (F11)");
                ui.label("Enter values for placeholders:");
                ui.add_space(8.0);
                for v in &state.vars {
                    ui.label(format!("{}:", v.label));
                    match &v.var_type {
                        InteractiveVarType::Edit => {
                            let val = state.values.entry(v.tag.clone()).or_default();
                            ui.add(egui::TextEdit::singleline(val).desired_width(260.0));
                        }
                        InteractiveVarType::Choice => {
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
                        InteractiveVarType::Checkbox => {
                            let checked = state.checkbox_checked.entry(v.tag.clone()).or_insert(false);
                            let value = v.options.first().cloned().unwrap_or_default();
                            if ui.checkbox(checked, &v.label).changed() {
                                state.values.insert(
                                    v.tag.clone(),
                                    if *checked { value.clone() } else { String::new() },
                                );
                            }
                        }
                        InteractiveVarType::DatePicker => {
                            let val = state.values.entry(v.tag.clone()).or_default();
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::TextEdit::singleline(val)
                                        .desired_width(180.0)
                                        .hint_text("YYYYMMDD"),
                                );
                                if ui.button("Today").clicked() {
                                    *val = chrono::Local::now().format("%Y%m%d").to_string();
                                }
                            });
                        }
                        InteractiveVarType::FilePicker => {
                            let val = state.values.entry(v.tag.clone()).or_default();
                            let filter_str = v.options.first().map(|s| s.as_str()).unwrap_or("All Files (*.*)");
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::TextEdit::singleline(val)
                                        .desired_width(200.0)
                                        .hint_text("Path to file"),
                                );
                                if ui.button("Browse...").clicked() {
                                    crate::utils::with_file_filters(filter_str, |filters| {
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
                        *result.borrow_mut() = Some(ViewportModalResult::Ok);
                    }
                    if ui.button("Cancel").clicked() {
                        *result.borrow_mut() = Some(ViewportModalResult::Cancel);
                    }
                });
            });
        }
    }
    result.into_inner()
}

/// Take and clear viewport modal, returning the state for processing.
pub fn take_viewport_modal() -> Option<ViewportModalState> {
    if let Ok(mut g) = VIEWPORT_MODAL.lock() {
        g.take()
    } else {
        None
    }
}

/// Process viewport result (OK or Cancel) and clear. Call from main thread after render returns.
pub fn process_viewport_result(result: ViewportModalResult) {
    if let Some(state) = take_viewport_modal() {
        match result {
            ViewportModalResult::Ok => {
                let mut values = state.values;
                for v in &state.vars {
                    if let InteractiveVarType::Choice = v.var_type {
                        let idx = state.choice_indices.get(&v.tag).copied().unwrap_or(0);
                        if let Some(opt) = v.options.get(idx) {
                            values.insert(v.tag.clone(), opt.clone());
                        }
                    }
                    if let InteractiveVarType::Checkbox = v.var_type {
                        let checked = state.checkbox_checked.get(&v.tag).copied().unwrap_or(false);
                        let value = v.options.first().cloned().unwrap_or_default();
                        values.insert(v.tag.clone(), if checked { value } else { String::new() });
                    }
                }
                let clip_history: Vec<String> = crate::application::clipboard_history::get_entries()
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
                    crate::drivers::hotstring::request_expansion(processed);
                }
            }
            ViewportModalResult::Cancel => {
                if let Some(ref tx) = state.response_tx {
                    let _ = tx.send((None, None));
                }
            }
        }
    }
}

/// Pending expansion that requires user input.
pub struct PendingExpansion {
    pub content: String,
    /// Trigger length for backspacing (hotstring only); None for Ghost Follower.
    pub trigger_len: Option<usize>,
    /// Window that had focus when trigger was typed (restore before paste).
    pub target_hwnd: Option<isize>,
    /// Channel to send (expansion, target_hwnd); None for Ghost Follower.
    pub response_tx: Option<Sender<(Option<String>, Option<isize>)>>,
}

static PENDING: Mutex<Option<PendingExpansion>> = Mutex::new(None);

/// Set pending expansion from Ghost Follower (no response channel).
pub fn set_pending_from_ghost(content: String) {
    if let Ok(mut g) = PENDING.lock() {
        *g = Some(PendingExpansion {
            content,
            trigger_len: None,
            target_hwnd: None,
            response_tx: None,
        });
    }
}

/// Set pending expansion from hotstring (with response channel).
/// target_hwnd: window that had focus when trigger was typed (for restoring focus before paste).
/// If replacing an existing pending, sends (None, None) to the previous response channel.
pub fn set_pending_from_hotstring(
    content: String,
    trigger_len: usize,
    target_hwnd: Option<isize>,
    response_tx: Sender<(Option<String>, Option<isize>)>,
) {
    if let Ok(mut g) = PENDING.lock() {
        if let Some(ref old) = *g {
            if let Some(ref tx) = old.response_tx {
                let _ = tx.send((None, None));
            }
        }
        *g = Some(PendingExpansion {
            content,
            trigger_len: Some(trigger_len),
            target_hwnd,
            response_tx: Some(response_tx),
        });
    }
}

/// Take pending expansion for main thread to process. Returns None if nothing pending.
pub fn take_pending_expansion() -> Option<PendingExpansion> {
    if let Ok(mut g) = PENDING.lock() {
        g.take()
    } else {
        None
    }
}

/// Check if content has interactive vars (for quick check before expansion).
pub fn has_interactive_vars(content: &str) -> bool {
    !template_processor::collect_interactive_vars(content).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_interactive_vars_checkbox_datepicker_filepicker() {
        assert!(has_interactive_vars("Text {checkbox:Y|yes}"));
        assert!(has_interactive_vars("Date: {date_picker:When}"));
        assert!(has_interactive_vars("File: {file_picker:Path}"));
        assert!(has_interactive_vars("Mix: {var:x} {choice:a|1|2} {checkbox:b|c}"));
        assert!(!has_interactive_vars("Plain text only"));
        assert!(!has_interactive_vars("{date} {time}"));
    }

    #[test]
    fn test_set_take_pending_with_target_hwnd() {
        let (tx, rx) = std::sync::mpsc::channel();
        set_pending_from_hotstring("content".to_string(), 5, Some(12345), tx);
        let pending = take_pending_expansion().expect("should have pending");
        assert_eq!(pending.content, "content");
        assert_eq!(pending.trigger_len, Some(5));
        assert_eq!(pending.target_hwnd, Some(12345));
        assert!(pending.response_tx.is_some());
        let _ = pending.response_tx.unwrap().send((Some("expanded".to_string()), Some(12345)));
        let (expansion, hwnd) = rx.recv().unwrap();
        assert_eq!(expansion, Some("expanded".to_string()));
        assert_eq!(hwnd, Some(12345));
    }

    #[test]
    fn test_set_pending_replaces_old_and_notifies() {
        let (tx1, rx1) = std::sync::mpsc::channel();
        set_pending_from_hotstring("first".to_string(), 3, None, tx1);
        let (tx2, _rx2) = std::sync::mpsc::channel();
        set_pending_from_hotstring("second".to_string(), 4, Some(999), tx2);
        let (old_exp, _) = rx1.recv().unwrap();
        assert_eq!(old_exp, None);
        let pending = take_pending_expansion().unwrap();
        assert_eq!(pending.content, "second");
        assert_eq!(pending.target_hwnd, Some(999));
    }
}
