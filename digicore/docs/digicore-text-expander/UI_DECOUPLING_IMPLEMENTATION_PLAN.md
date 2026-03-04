# UI Framework Decoupling - Phase 0/1 Implementation Plan

**Version:** 1.2  
**Created:** 2026-02-28  
**Last Updated:** 2026-02-28  
**Status:** Phase 0/1 Implemented; Migrated from Azul to Tauri (dual-binary: egui + Tauri)  
**Product:** DigiCore Text Expander  
**Architecture:** Hexagonal, Configuration-first, SOLID, SRP

---

## 1. Executive Summary

This document defines **Phase 0/1** of the UI framework decoupling effort. The goal is to introduce framework-agnostic ports and adapters so that the GUI frontend can be swapped between egui, Tauri, Iced, etc. with minimal rewrite of core logic. Multiple GUI binaries can coexist via feature flags for side-by-side comparison and easy migration.

**Key Outcomes:**
- Core application logic and domain remain **framework-agnostic**
- New UI frameworks require only **new adapters**, not core changes
- **Feature flags** enable dual/tertiary binaries (e.g., `--gui=egui`, `--gui=tauri`)
- Hexagonal architecture preserved; Configuration-first and SOLID/SRP principles enforced

---

## 2. Current Implementation Analysis

### 2.1 Framework Coupling Points

| Location | egui/eframe Usage | Responsibility |
|----------|-------------------|----------------|
| `main.rs` | `eframe::run_native`, `eframe::App`, `eframe::Storage`, `eframe::CreationContext` | Entry, app lifecycle, persistence |
| `main.rs` | `egui::Context`, `egui::ViewportBuilder`, `egui::ViewportCommand`, `egui::ViewportId` | Viewports (Ghost Follower, Ghost Suggestor, Variable Input) |
| `main.rs` | `egui::TopBottomPanel`, `egui::SidePanel`, `egui::CentralPanel`, `egui::menu::bar` | Layout, menu bar |
| `ui/egui/library_tab.rs` | `egui::Ui`, `egui::ScrollArea`, `egui::TextEdit`, `egui::ComboBox`, `egui::Button`, `egui::Sense` | Library tab rendering |
| `ui/egui/configuration_tab.rs` | `egui::Ui`, `egui::TextEdit`, `egui::DragValue`, `egui::Button` | Configuration tab |
| `ui/egui/clipboard_history_tab.rs` | `egui::Ui`, `egui::ScrollArea`, `egui::Sense` | Clipboard history tab |
| `ui/egui/script_library_tab.rs` | `egui::Ui`, `egui::TextEdit`, `egui::ScrollArea`, `egui::TextStyle` | Script library tab |
| `ui/egui/modals.rs` | `egui::Window`, `egui::TextEdit`, `egui::ComboBox`, `egui::Checkbox`, `egui::ViewportBuilder` | Modals, variable input viewport |
| `application/variable_input.rs` | `egui::CentralPanel`, `egui::TextEdit`, `egui::ComboBox`, `egui::Checkbox` | Variable input UI |
| `application/js_syntax_highlighter.rs` | `egui::text::{LayoutJob, TextFormat}`, `egui::Color32`, `egui::FontId` | Syntax highlighting |

### 2.2 Persistence (Storage) Usage

| Key | Storage | Purpose |
|-----|---------|---------|
| `library_path` | eframe::Storage | Library JSON path |
| `sync_url` | eframe::Storage | WebDAV sync URL |
| `template_date_format` | eframe::Storage | Date format |
| `template_time_format` | eframe::Storage | Time format |
| `script_library_run_disabled` | eframe::Storage | Run command disabled flag |
| `script_library_run_allowlist` | eframe::Storage | Run allowlist |
| `ghost_suggestor_display_secs` | eframe::Storage | Ghost Suggestor display duration |

### 2.3 Window/Viewport Usage

| Viewport | Purpose | egui-Specific |
|----------|---------|----------------|
| Main window | Primary app, tabs, menu | `ViewportBuilder`, `ViewportCommand` |
| Ghost Follower | Pinned + clipboard ribbon | `show_viewport_immediate`, `ViewportBuilder`, `ViewportCommand` |
| Ghost Suggestor | Suggestion overlay | `show_viewport_immediate`, caret-based positioning |
| Variable Input (F11) | {var:}, {choice:} input | `show_viewport_immediate`, `ViewportBuilder` |

### 2.4 Existing Ports (digicore-core)

| Port | Purpose | Adapter |
|------|---------|---------|
| `SnippetRepository` | Library load/save | JsonLibraryAdapter |
| `InputPort` | Text/key injection | EnigoInputAdapter |
| `ClipboardPort` | Clipboard read/write | ArboardClipboardAdapter |
| `WindowContextPort` | Active window context | WindowsWindowAdapter |
| `CryptoPort` | Encrypt/decrypt | AesCryptoAdapter |
| `SyncPort` | WebDAV sync | WebDAVAdapter |

---

## 3. Ports and Adapters to Introduce

### 3.1 StoragePort

**Purpose:** Key-value persistence for user preferences (library_path, sync_url, template formats, etc.). Framework-agnostic; eframe uses its built-in storage; Tauri uses JSON file.

**Location:** `digicore-core/src/domain/ports/storage.rs` (or `digicore-text-expander/src/ports` if app-specific)

```rust
/// Port for key-value persistence (user preferences, window state).
pub trait StoragePort: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;
    fn set(&mut self, key: &str, value: &str);
}
```

**Adapters:**
- `EframeStorageAdapter` – wraps `eframe::Storage` (current behavior)
- `JsonFileStorageAdapter` – JSON file in `%APPDATA%/DigiCore/text_expander_state.json` (for Tauri, Iced, etc.)

**Storage keys (enum or constants):** `library_path`, `sync_url`, `template_date_format`, `template_time_format`, `script_library_run_disabled`, `script_library_run_allowlist`, `ghost_suggestor_display_secs`

---

### 3.2 WindowPort

**Purpose:** Create, show, close, and position viewports/windows. Abstracts multi-window behavior (Ghost Follower, Ghost Suggestor, Variable Input).

**Location:** `digicore-core` or `digicore-text-expander/src/ports`

```rust
/// Viewport descriptor - framework-agnostic.
pub struct ViewportDescriptor {
    pub id: String,
    pub title: String,
    pub size: (f32, f32),
    pub position: Option<(f32, f32)>,
    pub always_on_top: bool,
    pub decorations: bool,
    pub taskbar: bool,
}

/// Port for viewport/window management.
pub trait WindowPort: Send + Sync {
    /// Show viewport with given descriptor. Render callback invoked each frame.
    fn show_viewport(&self, descriptor: ViewportDescriptor, render: impl FnMut() -> ViewportRenderResult);
    /// Close viewport by id.
    fn close_viewport(&self, id: &str);
    /// Send command (visible, minimized, focus, etc.).
    fn send_viewport_command(&self, id: &str, cmd: ViewportCommand);
}

pub enum ViewportCommand {
    Visible(bool),
    Minimized(bool),
    Maximized(bool),
    Focus,
    Close,
    WindowLevel(WindowLevel),
}

pub enum WindowLevel {
    Normal,
    AlwaysOnTop,
}
```

**Note:** egui's immediate-mode `show_viewport_immediate` is callback-based. Tauri/Iced use different models. The port should abstract the *intent* (show window, close, position) rather than the exact API. Adapters translate to framework-specific calls.

**Adapters:**
- `EguiWindowAdapter` – uses `ctx.show_viewport_immediate`, `ViewportBuilder`, `ViewportCommand`
- `TauriWindowAdapter` – Tauri WebviewWindow for secondary windows (Ghost Follower, Ghost Suggestor, Variable Input)

---

### 3.3 UIPort (Framework-Agnostic UI Primitives)

**Purpose:** Abstract common UI operations so that tab/modal logic can be expressed in framework-agnostic terms. This is the most challenging port because egui is immediate-mode while Tauri/Iced are retained/reactive.

**Strategy:** Two approaches:

**Option A – Minimal UIPort (Recommended for Phase 0/1):**  
Do NOT abstract widgets. Keep tab/modal rendering in framework-specific modules. Only abstract:
- **AppState** (extracted, framework-agnostic)
- **StoragePort**, **WindowPort** (as above)
- **Event/Command** types that tabs emit (e.g., `TabEvent::LoadLibrary`, `TabEvent::OpenSnippetEditor`)

Tabs receive `&mut AppState` and a **framework-specific** `Ui`/`Context`. The *logic* (what to do on click) lives in AppState or application services; the *rendering* stays framework-specific.

**Option B – Full UIPort (Future Phase):**  
Define traits like `UiContext`, `Button`, `TextEdit`, `ComboBox` that each framework implements. Higher effort; may not fit immediate-mode well.

**Recommendation:** Use **Option A** for Phase 0/1. Extract AppState, introduce StoragePort and WindowPort. Tabs remain `ui_egui/library_tab.rs` etc. When adding Tauri, create `ui_tauri/` with web frontend (HTML/CSS/JS) that invokes Tauri commands; backend uses same AppState.

---

### 3.4 FileDialogPort (Optional)

**Purpose:** File picker for Load Library, Save Library, Browse in variable input. Currently uses `rfd::FileDialog` directly.

```rust
pub trait FileDialogPort: Send + Sync {
    fn pick_file(&self, filters: &[(&str, &[&str])]) -> Option<std::path::PathBuf>;
    fn save_file(&self, default_name: &str) -> Option<std::path::PathBuf>;
}
```

**Adapters:** `RfdFileDialogAdapter` (current), `TauriFileDialogAdapter` (future). Low priority for Phase 0/1; rfd is already framework-agnostic.

---

### 3.5 TimerPort (Optional)

**Purpose:** Debounce (Ghost Suggestor), repaint scheduling. Currently uses module-level state and `ctx.request_repaint_after()`.

```rust
pub trait TimerPort: Send + Sync {
    fn schedule_repaint_after(&self, duration: std::time::Duration);
}
```

**Adapters:** `EguiTimerAdapter` (wraps `ctx.request_repaint_after`), `TauriTimerAdapter` (channel or Tauri async). Can be deferred to Phase 2.

---

### 3.6 ConfigPort (Optional – Fold into StoragePort)

**Purpose:** Load/save structured config (e.g., `AppConfig`). Could be a thin wrapper over `StoragePort` with serialization. For Phase 0/1, `StoragePort` with string values is sufficient; `AppConfig` can be built from `StoragePort::get` and saved via `StoragePort::set` with JSON serialization.

---

## 4. AppState Extraction

### 4.1 Current State (TextExpanderApp)

`TextExpanderApp` has ~90 fields mixing:
- **Domain/application state:** library, categories, selected_category, sync_status, etc.
- **UI state:** active_tab, snippet_editor_modal_open, clip_view_content, etc.
- **Transient:** sync_rx, window_visibility_ensured, etc.

### 4.2 Extracted AppState (Framework-Agnostic)

Move all state that does not depend on egui into a new `AppState` struct in `application/app_state.rs` (or `domain/`):

```rust
/// Framework-agnostic application state.
/// Used by all UI adapters (egui, Tauri, Iced).
pub struct AppState {
    // Library
    pub library_path: String,
    pub library: HashMap<String, Vec<Snippet>>,
    pub categories: Vec<String>,
    pub selected_category: Option<usize>,
    pub status: String,
    pub active_tab: Tab,

    // Sync
    pub sync_url: String,
    pub sync_password: String,
    pub sync_status: SyncResult,
    pub sync_rx: Option<mpsc::Receiver<(SyncResult, bool)>>,
    pub startup_sync_done: bool,

    // Expansion
    pub expansion_paused: bool,

    // Discovery, Ghost Suggestor, Ghost Follower, Templates, etc.
    // ... (all config and UI-modal state that is framework-agnostic)
}
```

**TextExpanderApp** becomes a thin wrapper:

```rust
pub struct TextExpanderApp {
    pub state: AppState,
    // egui-specific: viewport visibility flags, etc.
    window_visibility_ensured: bool,
    ghost_follower_visibility_ensured: bool,
}
```

Or, for full decoupling, `TextExpanderApp` is renamed to `EguiApp` and only exists in the egui binary. The `main` entry for egui constructs `EguiApp { state: AppState::new(storage) }` and implements `eframe::App`.

---

## 5. Feature Flag and Binary Layout

### 5.1 Cargo Features

```toml
# digicore-text-expander/Cargo.toml

[features]
default = ["gui-egui"]
gui-egui = ["eframe", "egui"]
gui-tauri = []  # Tauri web UI (tauri-app)
gui-iced = []   # Future
```

### 5.2 Binary Layout

| Binary | Feature | Purpose |
|--------|---------|---------|
| `digicore-text-expander` | `gui-egui` (default) | Current egui app |
| `digicore-text-expander-tauri` | `gui-tauri` | Tauri app (tauri-app/) |
| `digicore-text-expander-iced` | `gui-iced` | Iced app (future) |

Or single binary with `--gui=egui|tauri|iced`:

```rust
fn main() {
    let gui = std::env::args()
        .find(|a| a.starts_with("--gui="))
        .and_then(|a| a.strip_prefix("--gui=").map(String::from))
        .unwrap_or_else(|| "egui".to_string());

    match gui.as_str() {
        "egui" => run_egui(),
        "tauri" => run_tauri(),
        _ => run_egui(),
    }
}
```

### 5.3 Directory Structure (Post Phase 0/1)

```
digicore-text-expander/
  src/
    lib.rs
    main.rs                    # Dispatches to gui based on feature/arg
    application/
      app_state.rs             # Extracted AppState (framework-agnostic)
      template_processor.rs
      variable_input.rs        # State only; render moved to ui adapter
      ...
    ports/
      storage.rs               # StoragePort trait
      window.rs                # WindowPort trait
    adapters/
      storage/
        eframe_storage.rs      # EframeStorageAdapter
        json_file_storage.rs   # JsonFileStorageAdapter
      window/
        egui_window.rs         # EguiWindowAdapter
    ui/
      egui/                    # cfg(feature = "gui-egui")
        mod.rs
        app.rs                 # EguiApp, eframe::App impl
        library_tab.rs
        configuration_tab.rs
        clipboard_history_tab.rs
        script_library_tab.rs
        modals.rs
      tauri/                   # cfg(feature = "gui-tauri") - web frontend in tauri-app/
        mod.rs
        app.rs                 # Tauri integration points (backend in src-tauri)
        ...
```

---

## 6. Phase 0/1 Implementation Steps

### Phase 0: Preparation (No Behavior Change)

| Step | Task | Effort | Output |
|------|------|--------|--------|
| 0.1 | Define `StoragePort` trait in `digicore-core` or `digicore-text-expander/ports` | S | `storage.rs` |
| 0.2 | Implement `EframeStorageAdapter` wrapping `eframe::Storage` | S | `adapters/storage/eframe_storage.rs` |
| 0.3 | Migrate all `storage.get_string` / `storage.set_string` calls in `TextExpanderApp::new` and `save` to use `StoragePort` | M | main.rs refactor |
| 0.4 | Define `WindowPort` trait (simplified: create_viewport, close_viewport, send_command) | M | `window.rs` |
| 0.5 | Implement `EguiWindowAdapter` that holds `egui::Context` and translates port calls to egui | M | `adapters/window/egui_window.rs` |
| 0.6 | Extract `AppState` from `TextExpanderApp` into `application/app_state.rs` | L | `app_state.rs`, `TextExpanderApp { state }` |
| 0.7 | Add Cargo features `gui-egui`, `gui-tauri` (stub) | S | Cargo.toml |

### Phase 1: Wire and Validate

| Step | Task | Effort | Output |
|------|------|--------|--------|
| 1.1 | Inject `StoragePort` and `WindowPort` into `TextExpanderApp` (or `EguiApp`) via constructor | S | DI in main |
| 1.2 | Refactor viewport logic (Ghost Follower, Ghost Suggestor, Variable Input) to use `WindowPort` where possible | M | main.rs, modals.rs |
| 1.3 | Move `variable_input::render_viewport_modal` behind `#[cfg(feature = "gui-egui")]`; keep state in `variable_input` | S | variable_input.rs |
| 1.4 | Add `JsonFileStorageAdapter` for non-eframe runtimes (used when `gui-tauri` etc.) | M | `adapters/storage/json_file_storage.rs` |
| 1.5 | Document port contracts and adapter responsibilities | S | This doc, inline docs |
| 1.6 | Run full test suite; manual smoke test | S | CI green |

### Phase 2 (Future): Additional Ports and Second GUI

| Step | Task |
|------|------|
| 2.1 | Introduce `FileDialogPort` if needed |
| 2.2 | Introduce `TimerPort` for debounce/repaint |
| 2.3 | Implement Tauri adapters (TauriStorageAdapter, TauriWindowAdapter, TauriTimerAdapter) |
| 2.4 | Create `ui/tauri/` with integration points; web frontend in tauri-app/ |
| 2.5 | Add `--gui=egui|tauri` CLI flag for dual-binary testing |

---

## 7. Dependency Flow (Hexagonal)

```
                    +------------------+
                    |   Domain/Core    |
                    |  (AppState,      |
                    |   Snippet, etc.) |
                    +--------+---------+
                             |
              +--------------+--------------+
              |              |              |
              v              v              v
    +---------+----+  +------+------+  +----+-----+
    | StoragePort  |  | WindowPort  |  | Snippet  |
    |              |  |             |  | Repo     |
    +------+-------+  +------+-------+  +----------+
           |                 |
           v                 v
    +------+------+   +------+------+
    | EframeStorage|   | EguiWindow  |
    | Adapter      |   | Adapter     |
    +--------------+   +-------------+
           |                 |
           +--------+--------+
                    |
                    v
            +-------+-------+
            |  egui/eframe  |
            |  (Driver)    |
            +--------------+
```

- **Domain** has no knowledge of egui, eframe, Tauri, etc.
- **Ports** define interfaces; **Adapters** implement them for specific frameworks.
- **UI layer** (egui tabs, modals) depends on Ports and AppState, not on framework internals beyond the adapter boundary.

---

## 8. Configuration-First Compliance

- All user preferences loaded via `StoragePort` (or `ConfigPort` if introduced).
- `scripting.json` remains separate (scripting config).
- `AppConfig` struct used for in-memory representation; persistence via `StoragePort` with JSON or key-value.
- Validation on load; fallbacks for missing values.

---

## 9. SOLID/SRP Compliance

| Principle | Application |
|-----------|-------------|
| **SRP** | StoragePort = persistence only; WindowPort = window management only; each tab = one module |
| **OCP** | New UI framework = new adapters; core unchanged |
| **LSP** | Adapters are substitutable for their port traits |
| **ISP** | Ports are minimal (StoragePort: get/set; WindowPort: show/close/command) |
| **DIP** | AppState and application logic depend on StoragePort/WindowPort abstractions; UI depends on AppState |

---

## 10. Risk and Mitigation

| Risk | Mitigation |
|------|------------|
| Over-abstraction of UI (UIPort too complex) | Use Option A: minimal UIPort; keep tab rendering framework-specific |
| WindowPort doesn't fit Tauri/Iced model | Design port around *intent* (show, close, position); adapters handle framework quirks |
| AppState extraction breaks existing behavior | Incremental extraction; run tests after each step |
| Feature flags increase build complexity | Default = egui; other features opt-in; single default binary for users |

---

## 11. Success Criteria

- [x] All persistence goes through `StoragePort`; no direct `eframe::Storage` in domain/application
- [x] `AppState` is fully extracted and framework-agnostic
- [x] `WindowPort` abstracts viewport close/command; Ghost Follower, Ghost Suggestor use it (show_viewport deferred)
- [x] Feature `gui-egui` builds and runs identically to current behavior
- [x] Feature `gui-tauri` (stub) compiles; Tauri app in tauri-app/
- [x] StoragePort unit tests (JsonFileStorageAdapter)
- [x] All existing tests pass (ghost_follower test_start_stop: added #[serial] for robustness)
- [x] Documentation updated (this plan)

---

## 12. DECISION REQUIRED: WindowPort show_viewport

**Status:** Deferred in Phase 0/1.

**Issue:** egui's `show_viewport_immediate` is callback-based and receives `(ctx, class)` each frame. The render closure is `FnMut` and is invoked by the framework. Abstracting this into a generic `WindowPort::show_viewport(descriptor, render: FnMut() -> Result)` is non-trivial because:
- The render callback needs framework context (egui::Context) to draw
- Tauri/Iced use different models (web/MVU)

**Options:**
- **A)** Keep show_viewport framework-specific; only abstract close and send_command (current Phase 0 approach)
- **B)** Define `WindowPort::show_viewport` with a context parameter: `fn show_viewport<Ctx>(&self, ctx: &Ctx, descriptor, render)` - adapter receives framework context each frame
- **C)** Use a "viewport builder" pattern: adapter returns a handle; caller invokes render with the handle each frame

**Recommendation:** Option A for Phase 0/1. Revisit when implementing Tauri adapter.

---

## 13. Post Phase 0/1 Next Steps

**Status:** In progress.

### Completed

| Item | Status |
|------|--------|
| ghost_follower test_start_stop | Added `#[serial]` to `tests/ghost_follower_tests.rs` for robustness (shared static state) |
| TauriStorageAdapter | Implemented via `JsonFileStorageAdapter` type alias; `adapters/storage/tauri_storage.rs` |
| TauriWindowAdapter | Stub in `adapters/window/tauri_window.rs`; no-op close_viewport/send_viewport_command |
| ui/tauri/ | Placeholder in `ui/tauri/mod.rs`; web frontend in tauri-app/ |
| --gui=egui\|tauri CLI | `cli::parse_gui_from_args()`, `GuiBackend` enum; main dispatches to egui or tauri |
| CLI unit tests | `tests/cli_tests.rs` for parse_gui_from_args |
| FileDialogPort | RfdFileDialogAdapter, utils::parse_file_filter, with_file_filters |
| TimerPort | EguiTimerAdapter for Ghost Follower debounce |
| gui-egui optional | eframe/egui optional; main.rs requires gui-egui; main_tauri.rs for gui-tauri |
| main_tauri binary | `src/main_tauri.rs`; runs Tauri app when gui-tauri enabled |
| ui/egui restructure | Tabs and modals moved to `ui/egui/`; re-exported via `ui::` for main |
| tauri_stub | `tauri_stub::run_tauri_app()` in lib; main_tauri invokes tauri-app |
| ui/tauri/app.rs | Integration points documentation for Tauri (web frontend in tauri-app/) |

### Completed (Tauri Migration)

| Item | Description |
|------|-------------|
| TauriWindowAdapter | Shared viewport state (close/command requests); framework-agnostic |
| TauriTimerAdapter | Channel-based repaint scheduling; TauriTimerContext for app integration |
| ui/tauri | Minimal stub; web UI in tauri-app/src |
| tauri-app/ | Full Tauri project structure (src-tauri, src, tauri.conf.json) |
| TauriAppConfig | AppState, TauriStorageAdapter, RfdFileDialogAdapter, viewport/timer adapters |

### Completed (Tauri Frontend - 2026-03-02)

| Item | Description |
|------|-------------|
| Tauri commands | load_library, save_library, get_app_state, save_settings, get_ui_prefs, save_ui_prefs |
| Library tab | Load, Save, search, full columns (Profile, Category, Trigger, Content Preview, AppLock, Options, Last Modified), sortable, draggable column reorder, persist last tab + column order, row shading |
| Storage keys | UI_LAST_TAB, UI_COLUMN_ORDER |

### Completed (Tauri Snippet CRUD - 2026-03-02)

| Item | Description |
|------|-------------|
| Tauri Snippet Editor modal | Add/Edit; trigger, profile, options, category, content, appLock, pinned |
| Tauri Add/Edit/Delete | add_snippet, update_snippet, delete_snippet commands; Actions column |
| AppState add_snippet, update_snippet, delete_snippet | Framework-agnostic methods |

### Pending

| Item | Description |
|------|-------------|
| Tauri Configuration tab | Templates, Sync, Discovery, Ghost Suggestor, Ghost Follower, Clipboard config |
| Tauri Clipboard History tab | List, context menu, Promote, View Full, Delete, Clear All |
| Tauri Script Library tab | {run:} security, allowlist, global JS editor |
| Tauri secondary windows | Ghost Follower, Ghost Suggestor, Variable Input as WebviewWindow |

### Directory Structure (current)

```
digicore/
  crates/
    digicore-text-expander/src/
      cli.rs                # parse_gui_from_args, GuiBackend
      tauri_stub.rs         # run_tauri_app() stub (gui-tauri)
      ports/
      adapters/
        storage/
          tauri_storage.rs   # TauriStorageAdapter = JsonFileStorageAdapter
        window/
          tauri_window.rs    # TauriWindowAdapter
        timer/
          tauri_timer.rs     # TauriTimerAdapter
      ui/
        egui/               # gui-egui: tabs and modals
        tauri/              # gui-tauri: integration points (web UI in tauri-app/)
          mod.rs
          app.rs
  tauri-app/                # Tauri application
    src/                    # Web frontend (HTML, CSS, JS)
    src-tauri/              # Rust backend (Tauri commands)
```

### Run Commands

| Command | Result |
|---------|--------|
| `cargo run -p digicore-text-expander` | Runs egui (default) |
| `cargo run -p digicore-text-expander -- --gui=egui` | Runs egui |
| `cargo run -p digicore-text-expander -- --gui=tauri` | Egui binary: prints "Tauri GUI: use tauri-app. Run: cd tauri-app && npm run tauri dev" and exits 1 |
| `cd digicore/tauri-app && npm run tauri dev` | Runs Tauri app (web UI) |
| `cargo run -p digicore-text-expander --bin digicore-text-expander-tauri --no-default-features --features gui-tauri` | Runs Tauri binary (invokes tauri-app or shows instructions) |

---

## 14. References

- [TAURI_USER_GUIDE.md](./TAURI_USER_GUIDE.md) - **Build, dev, SQLite sync, troubleshooting**
- [TAURI_IMPLEMENTATION_STATUS.md](./TAURI_IMPLEMENTATION_STATUS.md) - Tauri feature status
- [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md) - **Parity matrix, Tauri roadmap, key decisions**
- [TAURI_MIGRATION_PLAN.md](./TAURI_MIGRATION_PLAN.md)
- [EGUI_TO_TAURI_MIGRATION_NOTES.md](./EGUI_TO_TAURI_MIGRATION_NOTES.md)
- [DigiCore Hexagonal Architecture](../../crates/digicore-core/)

---

## Appendix A: Tauri Migration Summary

**Last Updated:** 2026-03-02  
**Purpose:** Brief summary of the migration from Azul to Tauri.

### Decision

Azul was replaced with Tauri due to:
- **Stability:** Tauri is production-ready; Azul is alpha with Windows build issues.
- **Maturity:** Tauri has a large ecosystem; Azul has crates.io objc2/codegen blockers.
- **Multi-OS:** Tauri supports Windows, Linux, macOS out of the box.
- **Mobile:** Tauri has a mobile roadmap for future tablet/phone support.

### Migration Status

| Component | Status |
|-----------|--------|
| gui-azul -> gui-tauri | Feature flag renamed |
| AzulStorageAdapter -> TauriStorageAdapter | JsonFileStorageAdapter type alias |
| AzulWindowAdapter -> TauriWindowAdapter | Shared viewport state |
| AzulTimerAdapter -> TauriTimerAdapter | Channel-based |
| ui/azul -> ui/tauri | Minimal stub; web UI in tauri-app/ |
| tauri-app/ | Created with src-tauri, tauri.conf.json |
| main_azul -> main_tauri | Binary renamed |

### Run Commands

- **egui:** `cargo run -p digicore-text-expander`
- **Tauri:** `cd digicore/tauri-app && npm run tauri dev`

See [TAURI_MIGRATION_PLAN.md](./TAURI_MIGRATION_PLAN.md) for full details.
