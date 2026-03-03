//! CLI parsing for Text Expander binary.
//!
//! Supports `--gui=egui|tauri` for dual/multi-GUI foundation.
//! Default: egui when no flag or when --gui=egui.

/// GUI backend selected from CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiBackend {
    Egui,
    Tauri,
}

/// Parse `--gui=<backend>` from `std::env::args()`.
///
/// Returns `GuiBackend::Egui` if no `--gui=` arg or if value is `egui`.
/// Returns `GuiBackend::Tauri` if `--gui=tauri`.
/// Unknown values fall back to egui.
///
/// # Examples
///
/// ```
/// use digicore_text_expander::cli::{parse_gui_from_slice, GuiBackend};
///
/// let args = vec!["app".to_string(), "--gui=tauri".to_string()];
/// assert_eq!(parse_gui_from_slice(&args), GuiBackend::Tauri);
///
/// let args = vec!["app".to_string()];
/// assert_eq!(parse_gui_from_slice(&args), GuiBackend::Egui);
/// ```
pub fn parse_gui_from_args() -> GuiBackend {
    let args: Vec<String> = std::env::args().collect();
    parse_gui_from_slice(&args)
}

/// Parse `--gui=<backend>` from a slice of args (testable).
pub fn parse_gui_from_slice(args: &[String]) -> GuiBackend {
    for arg in args {
        if let Some(value) = arg.strip_prefix("--gui=") {
            return match value.to_lowercase().as_str() {
                "tauri" => GuiBackend::Tauri,
                _ => GuiBackend::Egui,
            };
        }
    }
    GuiBackend::Egui
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gui_no_arg() {
        let args = vec!["digicore-text-expander".to_string()];
        assert_eq!(parse_gui_from_slice(&args), GuiBackend::Egui);
    }

    #[test]
    fn test_parse_gui_egui() {
        let args = vec!["digicore-text-expander".to_string(), "--gui=egui".to_string()];
        assert_eq!(parse_gui_from_slice(&args), GuiBackend::Egui);
    }

    #[test]
    fn test_parse_gui_tauri() {
        let args = vec!["digicore-text-expander".to_string(), "--gui=tauri".to_string()];
        assert_eq!(parse_gui_from_slice(&args), GuiBackend::Tauri);
    }

    #[test]
    fn test_parse_gui_tauri_case_insensitive() {
        let args = vec!["digicore-text-expander".to_string(), "--gui=TAURI".to_string()];
        assert_eq!(parse_gui_from_slice(&args), GuiBackend::Tauri);
    }

    #[test]
    fn test_parse_gui_unknown_falls_back_to_egui() {
        let args = vec!["digicore-text-expander".to_string(), "--gui=iced".to_string()];
        assert_eq!(parse_gui_from_slice(&args), GuiBackend::Egui);
    }

    #[test]
    fn test_parse_gui_first_wins() {
        let args = vec![
            "digicore-text-expander".to_string(),
            "--gui=tauri".to_string(),
            "--gui=egui".to_string(),
        ];
        assert_eq!(parse_gui_from_slice(&args), GuiBackend::Tauri);
    }
}
