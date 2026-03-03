# egui to Tauri Migration Notes

**Version:** 1.0  
**Created:** 2026-03-02  
**Status:** In Progress  
**Product:** DigiCore Text Expander  

---

## Summary

DigiCore Text Expander uses a dual-binary architecture:
- **egui** (default): Native Rust UI via eframe/egui
- **Tauri**: Web frontend (HTML/CSS/JS) with Rust backend

Tauri was chosen over Azul for stability, maturity, multi-OS support (Windows, Linux, macOS), and mobile roadmap.

## Key Files

| Component | Location |
|-----------|----------|
| egui binary | `crates/digicore-text-expander/src/main.rs` |
| Tauri binary | `tauri-app/` (run via `npm run tauri dev`) |
| Tauri adapters | `adapters/storage/tauri_storage.rs`, `adapters/window/tauri_window.rs`, `adapters/timer/tauri_timer.rs` |
| UI integration | `ui/tauri/` (stub; web UI in tauri-app/src) |

## Run Commands

- **egui:** `cargo run -p digicore-text-expander`
- **Tauri:** `cd digicore/tauri-app && npm install && npm run tauri dev`

See [TAURI_MIGRATION_PLAN.md](./TAURI_MIGRATION_PLAN.md) for full details.
