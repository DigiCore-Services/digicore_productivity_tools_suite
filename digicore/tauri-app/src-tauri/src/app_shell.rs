//! Tauri app handle access, main/ghost window focus, and small desktop helpers.

use std::sync::{Arc, Mutex};

use digicore_text_expander::application::template_processor::InteractiveVarType;
use tauri::{AppHandle, Manager};

pub(crate) fn get_app(app: &Arc<Mutex<Option<AppHandle>>>) -> AppHandle {
    app.lock()
        .unwrap()
        .clone()
        .expect("AppHandle not yet set (setup not run)")
}

pub(crate) fn bring_main_to_foreground(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

/// Lowers Ghost Follower's always_on_top so the main window can appear above it,
/// then brings the main window to foreground. Call ghost_follower_restore_always_on_top
/// when the modal is closed.
pub(crate) fn bring_main_to_foreground_above_ghost_follower(app: &AppHandle) {
    if let Some(ghost) = app.get_webview_window("ghost-follower") {
        let _ = ghost.set_always_on_top(false);
    }
    bring_main_to_foreground(app);
}

pub(crate) fn var_type_to_string(t: &InteractiveVarType) -> &'static str {
    match t {
        InteractiveVarType::Edit => "edit",
        InteractiveVarType::Choice => "choice",
        InteractiveVarType::Checkbox => "checkbox",
        InteractiveVarType::DatePicker => "date_picker",
        InteractiveVarType::FilePicker => "file_picker",
    }
}

pub(crate) fn open_file_in_default_app(path: &str) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("Image path is empty.".to_string());
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", path])
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[allow(unreachable_code)]
    Err("Open image not supported on this platform.".to_string())
}
