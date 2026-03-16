//! Tauri GUI binary entry point.
//!
//! Built when feature `gui-tauri` is enabled. The Tauri app runs from
//! digicore/tauri-app/ via `npm run tauri dev`. This binary prints instructions.
//! Run: cargo run -p digicore-text-expander --bin digicore-text-expander-tauri

use digicore_text_expander::cli::{parse_gui_from_args, GuiBackend};

fn main() {
    match parse_gui_from_args() {
        GuiBackend::Egui => {
            eprintln!("Egui GUI requires --features gui-egui. Use: cargo run --bin digicore-text-expander");
            std::process::exit(1);
        }
        GuiBackend::Tauri => run_tauri(),
    }
}

fn run_tauri() {
    eprintln!("Tauri GUI runs from tauri-app/. Execute:");
    eprintln!("  cd digicore/tauri-app");
    eprintln!("  npm install");
    eprintln!("  npm run tauri dev");
    std::process::exit(0);
}
