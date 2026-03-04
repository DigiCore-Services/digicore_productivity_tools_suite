# DigiCore Services

Cross-platform application ecosystem (Text Expander, Copy-to-Clipboard, Appearance) built with Rust, following Hexagonal architecture and SOLID principles.

**Migration baseline:** This project refactors and migrates from AutoHotkey (AHK). AHK is the legacy source; Rust is the target. AHK will not be used going forward.

## Phase 0/1 Status

- [x] Workspace root
- [x] digicore-core crate (domain, ports, JsonLibraryAdapter)
- [x] CLI proof-of-concept
- [x] digicore-text-expander: Scripting Engine ({js:}, {http:}), template processor, VariableInputModal
- [x] egui GUI (Library, Configuration, Clipboard History, Script Library tabs)
- [x] Clipboard History tab with right-click context menu (Copy, View Full Content, Delete, Promote to Snippet, Clear All)
- [x] Modals: Promote to Snippet, Snippet Editor, Delete/Clear confirmations, View Full Content

## Prerequisites

- [Rust](https://rustup.rs/) (install via `rustup` to get `cargo`)

## Build

```bash
cargo build
```

### Tauri App (React + Vite)

```powershell
# From digicore directory - full Tauri build (frontend + Rust + bundle)
.\scripts\build.ps1 -Target Tauri

# Release (optimized)
.\scripts\build.ps1 -Target Tauri -Release
```

See [Tauri User Guide](docs/digicore-text-expander/TAURI_USER_GUIDE.md) for build, dev, and SQLite details.

## Run GUI (Text Expander)

```bash
# egui (native Rust UI)
cargo run -p digicore-text-expander

# Tauri (React + Vite)
cd tauri-app; npm run tauri dev
```

## Run CLI (proof-of-concept)

```bash
cargo run -p digicore-core --bin cli -- ..\AHK_-_PROD-MAIN_STARTUP-SCRIPTZ\ACTIVE-Prod-LIVE-Apps\Text-Expansion\text_expansion_library.json
```

Or from the workspace root:

```powershell
cargo run -p digicore-core --bin cli -- "C:\Users\pinea\Scripts\AHK_AutoHotKey\AHK_-_PROD-MAIN_STARTUP-SCRIPTZ\ACTIVE-Prod-LIVE-Apps\Text-Expansion\text_expansion_library.json"
```

## Testing

```bash
cargo test --workspace
```

**Test coverage (as of 2026-02-28):**

| Crate | Unit Tests | Integration Tests | Status |
|-------|------------|-------------------|--------|
| digicore-core | 0 | 48 | Pass |
| digicore-text-expander | 90 | 30 | Pass |

**Recent test additions:** Clipboard history (`add_entry_with_metadata`, `update_config_max_depth_trims`, `update_config_disabled`, `suppress_for_duration_no_panic`, `delete_entry_at`, `clear_all`), `truncate_for_display` utils, doc-tests for utils. See [Implementation Plan](docs/digicore-text-expander/IMPLEMENTATION_PLAN.md).

## Scripting Engine

The Text Expander uses a port-based Scripting Engine (`ScriptEnginePort`, `HttpFetcherPort`) for `{js:...}` (Boa) and `{http:url|path}` (reqwest). Placeholders: `{date}`, `{time}`, `{clipboard}`, `{clip:N}`, `{env:VAR}`, `{var:}`, `{choice:}`, `{checkbox:}`, `{date_picker:}`, `{file_picker:}`.

**Config:** `%APPDATA%/DigiCore/config/scripting.json` (HttpConfig, JsConfig, RunConfig). JS timeout, HTTP timeout, run allowlist configurable.

**Recent (Section 11):** Script config externalization, ScriptEnginePort/HttpFetcherPort DI, ScriptContext builder, JS execution timeout, MockScriptEngine, run persistence. See [Dynamic Templates Plan](../AHK_-_PROD-MAIN_STARTUP-SCRIPTZ/features_new_and_updated/TE_Pro_DYNAMIC_TEMPLATES_IMPLEMENTATION_PLAN_2026-02-28.md) Section 11.8 for next remaining steps.

## Clipboard History

Real-time clipboard monitoring (F38-F42 parity). Configurable depth (5-100). Right-click context menu: Copy to Clipboard, View Full Content, Delete Item, Promote to Snippet, Clear All History. See [Clipboard History Guide](docs/digicore-text-expander/CLIPBOARD_HISTORY.md).

## Documentation

- [Tauri User Guide](docs/digicore-text-expander/TAURI_USER_GUIDE.md) - Build, dev, SQLite sync, key features
- [Scripting User Guide](docs/digicore-text-expander/SCRIPTING_USER_GUIDE.md) - JavaScript, DSL, HTTP, Run, Script Library
- [Clipboard History](docs/digicore-text-expander/CLIPBOARD_HISTORY.md) - Clipboard History tab and context menu
- [Implementation Plan](docs/digicore-text-expander/IMPLEMENTATION_PLAN.md) - Implementation status, testing details
- [Tauri Implementation Status](docs/digicore-text-expander/TAURI_IMPLEMENTATION_STATUS.md) - Tauri feature status
- [Changelog](CHANGELOG.md) - Recent changes and test status

## Structure

See `../AHK_-_PROD-MAIN_STARTUP-SCRIPTZ/features_new_and_updated/TE_Pro_REFACTOR_AND_MIGRATION_ANALYSIS_2026-02-28.md` Section 11 for full directory layout.
