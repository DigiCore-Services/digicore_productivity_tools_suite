# egui to Azul Migration Proposal

**Version:** 1.0  
**Created:** 2026-02-28  
**Status:** Proposal  
**Product:** DigiCore Text Expander  
**Architecture:** Hexagonal, Configuration-first, SOLID, SRP

---

## 1. Executive Summary

This document proposes a refactoring and migration plan to move the DigiCore Text Expander GUI frontend from **egui/eframe** to **Azul**. It analyzes the current implementation, evaluates Azul and alternatives, and outlines a phased migration strategy that preserves Hexagonal architecture, Configuration-first principles, and SOLID/SRP best practices.

**Critical Note:** Azul is currently under heavy development ([GitHub README](https://github.com/fschutt/azul)) and states it is **NOT yet usable**. APIs may change frequently. A v1.0.0-alpha1 release exists (2025-04-03) per [azul.rs](https://azul.rs/). Migration to Azul should be considered a **medium-to-long-term** option, with timing dependent on Azul's stabilization roadmap.

---

## 2. Current egui Implementation Analysis

### 2.1 Architecture Overview

| Layer | Location | egui/eframe Usage |
|-------|----------|-------------------|
| **Entry** | `main.rs` | `eframe::run_native()`, `eframe::App`, `ViewportBuilder` |
| **App State** | `main.rs` | `TextExpanderApp` (~90 fields), `save()`/`update()` |
| **Tabs** | `ui/library_tab.rs`, `configuration_tab.rs`, `clipboard_history_tab.rs`, `script_library_tab.rs` | `egui::Ui`, `egui::ScrollArea`, `egui::TextEdit`, `egui::ComboBox`, `egui::Button` |
| **Modals** | `ui/modals.rs` | `egui::Window`, `egui::TextEdit`, `egui::ComboBox`, `egui::Checkbox` |
| **Viewports** | `main.rs` | `ctx.show_viewport_immediate()`, `ViewportBuilder`, `ViewportCommand` |
| **Variable Input** | `application/variable_input.rs` | `egui::CentralPanel`, viewport modal |
| **Syntax Highlighting** | `application/js_syntax_highlighter.rs` | `egui::text::{LayoutJob, TextFormat}` |

### 2.2 Key egui-Specific Patterns

- **Immediate-mode rendering:** UI rebuilt every frame; state in `TextExpanderApp`.
- **Viewports:** Ghost Follower, Ghost Suggestor, Variable Input use `show_viewport_immediate()` (separate windows).
- **Persistence:** `eframe::Storage` for library_path, sync_url, template formats, etc.
- **Panels:** `TopBottomPanel`, `SidePanel`, `CentralPanel` for layout.
- **Widgets:** `TextEdit`, `ComboBox`, `Checkbox`, `ScrollArea`, `Button`, `SelectableLabel`.

### 2.3 Hexagonal Boundaries (Current)

- **Domain (digicore-core):** No egui/eframe; pure entities and ports.
- **Adapters:** Persistence, platform, sync—all framework-agnostic.
- **Drivers:** Hotstring, discovery—no direct egui dependency.
- **Application:** template_processor, clipboard_history, ghost_*, variable_input—minimal egui (variable_input viewport only).
- **UI (main.rs, ui/*):** Full egui/eframe dependency; orchestrates application and drivers.

### 2.4 Configuration Management (Current)

| Source | Purpose |
|--------|---------|
| `eframe::Storage` | library_path, sync_url, template_date_format, template_time_format, script_library_run_*, ghost_suggestor_display_secs |
| `scripting.json` | Scripting config (JS, HTTP, run allowlist) |
| `AppConfig` (app_config.rs) | Struct only; not yet used for persistence |
| Module-level `set_config`/`get_config` | TemplateConfig, GhostSuggestorConfig, GhostFollowerConfig, ClipboardHistoryConfig, DiscoveryConfig |

---

## 3. Azul Framework Analysis

### 3.1 Azul Overview

**Source:** [azul.rs](https://azul.rs/), [GitHub - fschutt/azul](https://github.com/fschutt/azul)

- **Rendering:** Mozilla WebRender (GPU-accelerated).
- **Layout:** HTML/CSS-like DOM; Flexbox, Grid.
- **Languages:** Rust, C, C++, Python.
- **Binary:** ~15MB DLL; no Chromium/V8.
- **State:** App state in user code; Azul renders; explicit `Update.RefreshDom` for refresh.

### 3.2 Azul Architecture (from azul.rs examples)

```rust
// Conceptual Azul pattern
fn layout(data: &AppState, info: &LayoutInfo) -> StyledDom {
    let body = Dom::body();
    body.add_child(Dom::text("Hello"));
    body.add_child(Button::new("Click").set_callback(On::MouseUp, data, on_click));
    body.style(Css::empty())
}

fn on_click(data: &mut AppState, info: &CallbackInfo) -> Update {
    data.counter += 1;
    Update::RefreshDom
}

let app = App::create(model, AppConfig::create());
app.run(window);
```

- **Functional/reactive:** Layout function returns DOM; callbacks return `Update` (RefreshDom, DoNothing, etc.).
- **Widgets:** CheckBox, ProgressBar, TextInput, ColorInput, NumberInput, DropDown.
- **CSS:** Inline styles, external CSS, XHTML loading.
- **Timers:** `Timer`, `info.start_timer()` for animations/async.
- **OpenGL:** `ImageRef.callback` for custom rendering.
- **Infinite scroll:** `Dom::iframe` with `IFrameCallbacks`.

### 3.3 Azul Gaps for DigiCore (as of 2025)

| Requirement | Azul Support | Notes |
|-------------|--------------|-------|
| Multi-window / viewports | Unclear | Ghost Follower, Ghost Suggestor, Variable Input need separate windows |
| Always-on-top windows | Unclear | Ghost Suggestor overlay |
| Window positioning (caret-based) | Unclear | Ghost Suggestor near caret |
| File dialogs | Unclear | rfd used today; may need Azul-native or keep rfd |
| Persistence / storage | None built-in | Must implement StoragePort |
| Context menus | Unclear | Right-click menus in Library, Clipboard, Ghost Follower |
| Maturity | **NOT usable yet** | README: "APIs may change frequently" |

### 3.4 Azul Maturity Risk

The [Azul GitHub README](https://github.com/fschutt/azul) states:

> **This repository is currently under heavy development. Azul is NOT usable yet.**  
> APIs may change frequently and features may be incomplete or unstable.

Migration to Azul should be **deferred** until Azul reaches a stable, production-ready release, or undertaken as an **experimental branch** with full acceptance of API churn.

---

## 4. Alternative Implementation Options

### 4.1 Option A: Stay with egui (Status Quo)

| Aspect | Assessment |
|--------|------------|
| **Pros** | Mature, fast iteration, immediate-mode simplicity, viewports work, persistence built-in, active community |
| **Cons** | Non-native look, layout limitations, focus/viewport quirks (e.g. Ghost Follower) |
| **Effort** | Zero |
| **Risk** | Low |

### 4.2 Option B: Migrate to Azul

| Aspect | Assessment |
|--------|------------|
| **Pros** | CSS styling, WebRender, lightweight binary, functional model, multi-language |
| **Cons** | **Not production-ready**, multi-window unclear, persistence DIY, API churn |
| **Effort** | High (full UI rewrite) |
| **Risk** | High (framework instability) |

### 4.3 Option C: Migrate to Iced

| Aspect | Assessment |
|--------|------------|
| **Pros** | Elm architecture, reactive, wgpu, native-ish look |
| **Cons** | Experimental (0.7), different paradigm (MVU), multi-window support to verify |
| **Effort** | High |
| **Risk** | Medium |

### 4.4 Option D: Migrate to Slint

| Aspect | Assessment |
|--------|------------|
| **Pros** | Declarative, design tool, commercial support option |
| **Cons** | Different markup language, learning curve |
| **Effort** | High |
| **Risk** | Medium |

### 4.5 Option E: Migrate to Tauri

| Aspect | Assessment |
|--------|------------|
| **Pros** | Web tech (HTML/CSS/JS), 1.0 stable, small binary, strong ecosystem |
| **Cons** | Webview dependency, different stack (frontend + backend), larger refactor |
| **Effort** | Very high (new frontend stack) |
| **Risk** | Low (Tauri stable) |

### 4.6 Option F: Introduce UI Port (Framework-Agnostic) - **Recommended Foundation**

| Aspect | Assessment |
|--------|------------|
| **Pros** | Decouples UI from framework; enables future migration with less rewrite; supports multiple GUIs via feature flags (egui, Azul, Iced, Tauri) side-by-side |
| **Cons** | Abstraction overhead; minimal UIPort (Option A) keeps tab rendering framework-specific to avoid poor fit with immediate-mode |
| **Effort** | Medium (Phase 0/1) |
| **Risk** | Low |

**Implementation:** See [UI_DECOUPLING_IMPLEMENTATION_PLAN.md](./UI_DECOUPLING_IMPLEMENTATION_PLAN.md) for StoragePort, WindowPort, AppState extraction, and feature flag layout.

---

## 5. SWOT Analysis

| | **Strengths** | **Weaknesses** |
|---|---------------|----------------|
| **Internal** | Domain/adapter separation; no egui in core; modular tabs/modals | Large `TextExpanderApp`; static state in modules; egui coupled to main.rs |
| **External** | egui mature; Azul has promising design | Azul not production-ready; Iced/Slint experimental |

| | **Opportunities** | **Threats** |
|---|-------------------|-------------|
| **External** | Introduce `StoragePort` and `UIPort` for future flexibility; Azul may stabilize | Azul API churn; framework lock-in if migrating too early |

---

## 6. Recommended Migration Plan (Hexagonal-Compliant)

**See [UI_DECOUPLING_IMPLEMENTATION_PLAN.md](./UI_DECOUPLING_IMPLEMENTATION_PLAN.md) for the detailed Phase 0/1 implementation plan, port definitions, feature flags, and directory structure.**

### 6.1 Phase 0/1: Foundation (UI Framework Decoupling)

**Goal:** Decouple UI from framework so future migration (egui, Azul, Iced, Tauri) requires minimal rewrite. Enable multiple GUI binaries via feature flags for side-by-side comparison.

1. **Introduce `StoragePort`**
   - Define port: `get(key) -> Option<String>`, `set(key, value)`.
   - Implement `EframeStorageAdapter` wrapping `eframe::Storage`.
   - Implement `JsonFileStorageAdapter` for non-eframe runtimes.
   - Migrate all persistence calls to use `StoragePort`.
   - **Benefit:** Framework-agnostic persistence; any framework can use JSON file adapter.

2. **Introduce `WindowPort`**
   - Define port for: create viewport, close viewport, send viewport command (visible, focus, position, etc.).
   - Implement `EguiWindowAdapter` for current behavior.
   - **Benefit:** Isolates viewport/modal logic (Ghost Follower, Ghost Suggestor, Variable Input) for future swap.

3. **Extract `AppState` from `TextExpanderApp`**
   - Move domain/application state into a framework-agnostic `AppState` struct.
   - `TextExpanderApp` (or `EguiApp`) becomes a thin UI binding over `AppState`.
   - **Benefit:** State can be reused by any UI framework; core logic unchanged.

4. **Feature Flags and Binary Layout**
   - Cargo features: `gui-egui` (default), `gui-azul`, `gui-iced`, `gui-tauri`.
   - Single binary with `--gui=egui|azul|iced` or separate binaries per framework.
   - **Benefit:** Side-by-side comparison, easy swap without impacting core code.

5. **Additional Ports (Optional / Future)**
   - `FileDialogPort` – file picker (rfd today; framework-specific adapters later).
   - `TimerPort` – debounce, repaint scheduling.
   - `UIPort` – minimal: event/command types; tab rendering stays framework-specific (Option A in decoupling plan).

### 6.2 Phase 2: Azul Readiness (When Azul Stabilizes)

*Prerequisite: Phase 0/1 complete (StoragePort, WindowPort, AppState extracted).*

1. **Implement `AzulStorageAdapter`**
   - Use `JsonFileStorageAdapter` (from Phase 0/1) or Azul-specific JSON file in config dir (e.g. `%APPDATA%/DigiCore/text_expander_state.json`).
   - Implement `StoragePort` for load/save.

2. **Implement `AzulWindowAdapter`**
   - Map Ghost Follower, Ghost Suggestor, Variable Input to Azul windows (if supported).
   - Verify multi-window, always-on-top, positioning.

3. **Build main window layout in Azul**
   - Tabs: Library, Configuration, Clipboard History, Script Library.
   - Use Azul `Dom`, CSS, callbacks.
   - Wire to `AppState` via `Update::RefreshDom`.

4. **Migrate modals**
   - Snippet editor, delete confirm, clip view, variable input, preview result.
   - Use Azul dialogs/windows.

5. **Replace `js_syntax_highlighter`**
   - Azul text rendering or custom `ImageRef` if needed.

### 6.3 Phase 3: Cutover

1. **Dual-binary or feature flag** (foundation from Phase 0/1)
   - `--gui=egui` (default) vs `--gui=azul` for testing.
   - Side-by-side comparison without impacting core logic.
2. **Full switch** when Azul path is validated.
3. **Remove egui/eframe** dependencies (optional; can retain both).

### 6.4 Configuration-First Compliance

- All config in JSON/env; no hardcoding.
- `StoragePort` for user prefs; `scripting.json` for scripting; separate config files per domain.
- Validation on load; fallbacks for missing values.

### 6.5 SOLID/SRP Compliance

- **SRP:** Each tab = one module; each modal = one function; `StoragePort` = single responsibility.
- **DIP:** UI depends on `StoragePort`, `WindowPort` abstractions; domain has no UI deps.
- **OCP:** New UI framework = new adapter; core unchanged.

---

## 7. Key Decisions Requiring Your Input

### 7.1 Migration Timing

- **Q1:** Proceed with Azul migration now (experimental branch) or wait for Azul production release?
- **Q2:** Is introducing `StoragePort` and `WindowPort` (Phase 0) acceptable as a first step, independent of Azul?

### 7.2 Alternative Framework Preference

- **Q3:** If not Azul, which alternative do you prefer: **Iced**, **Slint**, **Tauri**, or **stay with egui**?
- **Q4:** Is a web-based frontend (Tauri) acceptable, or must the UI remain fully native Rust?

### 7.3 Scope

- **Q5:** Migrate entire UI in one effort, or incremental (e.g. main window first, viewports later)?
- **Q6:** Maintain dual egui/Azul support during transition, or hard cutover?

### 7.4 Risk Tolerance

- **Q7:** Accept Azul API churn and potential rewrites if Azul changes?
- **Q8:** Budget for external support (e.g. Slint commercial) if needed?

---

## 8. Summary Table

| Option | Effort | Risk | Maturity | Hexagonal Fit |
|--------|--------|------|----------|---------------|
| Stay egui | None | Low | High | Good |
| Azul | High | High | Low | Good (with ports) |
| Iced | High | Medium | Medium | Good |
| Slint | High | Medium | Medium | Good |
| Tauri | Very High | Low | High | Good (new frontend) |
| UI Port only | Medium | Low | N/A | Best (decouples) |

---

## 9. References

- [Azul GUI Framework](https://azul.rs/)
- [Azul GitHub](https://github.com/fschutt/azul)
- [egui](https://github.com/emilk/egui)
- [DigiCore Implementation Plan](./IMPLEMENTATION_PLAN.md)
- [DigiCore Hexagonal Architecture](../../crates/digicore-core/)

---

## 10. Recommended Next Steps

1. **Immediate:** Execute Phase 0/1 from [UI_DECOUPLING_IMPLEMENTATION_PLAN.md](./UI_DECOUPLING_IMPLEMENTATION_PLAN.md): introduce `StoragePort`, `WindowPort`, extract `AppState`, add feature flags. Low-risk; improves architecture regardless of future GUI choice.
2. **Short-term:** Validate Phase 0/1 with full test suite; document port contracts.
3. **Medium-term:** Monitor Azul releases; re-evaluate migration when Azul declares production readiness.
4. **Alternative:** If Azul remains unstable, consider Iced or Slint for a native Rust UI with different trade-offs. Phase 0/1 foundation supports any of these with minimal core changes.
