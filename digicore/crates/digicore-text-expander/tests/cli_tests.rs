//! Integration tests for CLI parsing.
//!
//! Tests parse_gui_from_slice (unit-tested in lib) and documents expected binary behavior.

use digicore_text_expander::cli::{parse_gui_from_slice, GuiBackend};

#[test]
fn test_cli_parse_egui_default() {
    let args = vec!["digicore-text-expander".to_string()];
    assert_eq!(parse_gui_from_slice(&args), GuiBackend::Egui);
}

#[test]
fn test_cli_parse_tauri_explicit() {
    let args = vec![
        "digicore-text-expander".to_string(),
        "--gui=tauri".to_string(),
    ];
    assert_eq!(parse_gui_from_slice(&args), GuiBackend::Tauri);
}

#[test]
fn test_cli_parse_egui_explicit() {
    let args = vec![
        "digicore-text-expander".to_string(),
        "--gui=egui".to_string(),
    ];
    assert_eq!(parse_gui_from_slice(&args), GuiBackend::Egui);
}
