# DigiCore Services

Cross-platform application ecosystem (Text Expander, Copy-to-Clipboard, Appearance) built with Rust, following Hexagonal architecture and SOLID principles.

**Migration baseline:** This project refactors and migrates from AutoHotkey (AHK). AHK is the legacy source; Rust is the target. AHK will not be used going forward.

## Phase 0/1 Status

- [x] Workspace root
- [x] digicore-core crate (domain, ports, JsonLibraryAdapter)
- [x] CLI proof-of-concept
- [x] digicore-text-expander: Scripting Engine ({js:}, {http:}), template processor, VariableInputModal

## Prerequisites

- [Rust](https://rustup.rs/) (install via `rustup` to get `cargo`)

## Build

```bash
cargo build
```

## Run CLI (proof-of-concept)

```bash
cargo run -p digicore-core --bin cli -- ..\AHK_-_PROD-MAIN_STARTUP-SCRIPTZ\ACTIVE-Prod-LIVE-Apps\Text-Expansion\text_expansion_library.json
```

Or from the workspace root:

```powershell
cargo run -p digicore-core --bin cli -- "C:\Users\pinea\Scripts\AHK_AutoHotKey\AHK_-_PROD-MAIN_STARTUP-SCRIPTZ\ACTIVE-Prod-LIVE-Apps\Text-Expansion\text_expansion_library.json"
```

## Scripting Engine

The Text Expander uses a port-based Scripting Engine (`ScriptEnginePort`, `HttpFetcherPort`) for `{js:...}` (Boa) and `{http:url|path}` (reqwest). Placeholders: `{date}`, `{time}`, `{clipboard}`, `{clip:N}`, `{env:VAR}`, `{var:}`, `{choice:}`, `{checkbox:}`, `{date_picker:}`, `{file_picker:}`.

**Config:** `%APPDATA%/DigiCore/config/scripting.json` (HttpConfig, JsConfig, RunConfig). JS timeout, HTTP timeout, run allowlist configurable.

**Recent (Section 11):** Script config externalization, ScriptEnginePort/HttpFetcherPort DI, ScriptContext builder, JS execution timeout, MockScriptEngine, run persistence. See [Dynamic Templates Plan](../AHK_-_PROD-MAIN_STARTUP-SCRIPTZ/features_new_and_updated/TE_Pro_DYNAMIC_TEMPLATES_IMPLEMENTATION_PLAN_2026-02-28.md) Section 11.8 for next remaining steps.

## Structure

See `../AHK_-_PROD-MAIN_STARTUP-SCRIPTZ/features_new_and_updated/TE_Pro_REFACTOR_AND_MIGRATION_ANALYSIS_2026-02-28.md` Section 11 for full directory layout.
