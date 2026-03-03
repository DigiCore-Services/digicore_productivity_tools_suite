# DigiCore Text Expander - Implementation Plan

**Version:** 2.1  
**Last Updated:** 2026-03-02  
**Product:** DigiCore Text Expander (Rust)  
**Status:** egui complete; Tauri Phase 1.1-1.2 partial (Library tab with Load/Save/search/table); AHK legacy retained for reference

---

## 1. Overview

This document tracks implementation status, parity across GUI frontends (egui, Tauri, legacy AHK), migration roadmap, and key decisions. The goal is **parity** across egui and Tauri, with the legacy AHK GUI (`AHK_TE_Pro_GUI.ahk`) as the reference for feature completeness.

**Context:**
- **egui** remains the primary native UI for power users and minimal footprint.
- **Tauri** is the preferred web-based GUI going forward (replaced Azul).
- **AHK legacy** (`C:\...\AHK_TE_Pro_GUI.ahk`) is the reference for full feature set.
- **Future:** Decision on deprecating egui deferred until Tauri is fully implemented.

---

## 2. Parity Matrix

### 2.1 Tab/Feature Mapping

| Feature | egui | AHK Legacy | Tauri (Target) | Notes |
|---------|------|------------|---------------|-------|
| **Library Tab** | Yes | Library Explorer | Yes | Search, categories, ListView, Export/Import JSON/CSV |
| **Snippet Editor** | Modal | Separate tab | Modal or tab | **DECISION:** Modal (egui) vs tab (AHK) |
| **Clipboard History** | Yes | Yes | Yes | F38-F42, context menu, Promote, View Full, Delete, Clear All |
| **Configuration** | Yes (collapsed) | Settings + Config | Yes | Paths, Sync, Discovery, Ghost Suggestor/Follower, Clipboard config |
| **Script Library** | Yes | Yes | Yes | {run:} security, allowlist, global JS library |
| **Performance/Analytics** | No | Yes (Tab 6) | **Future** | Chars saved, time saved, top snippets, history timeline |
| **Appearance** | No | Yes (Tab 7) | **Future** | Transparency rules per app |
| **Always-On-Top Manager** | No | Yes (Tab 8) | **Future** | AOT rules per app (distinct from Ghost Suggestor always-on-top) |
| **Intelligence/Discovery** | Yes (in Config) | Yes (Tab 9) | Yes | Smart-Discovery, harvesting settings |
| **Follower** | Yes (in Config) | Yes (Tab 10) | Yes | Ghost Follower settings |
| **Dashboard** | No | Yes (Tab 12) | **Future** | WebView2 embed or launch; productivity analytics |
| **Settings (Core)** | Yes (in Config) | Yes (Tab 4) | Yes | Pause, Reload Library |

### 2.2 Secondary Windows / Viewports

| Viewport | egui | AHK | Tauri |
|----------|------|-----|-------|
| Ghost Follower | Yes (egui viewport) | Yes | Tauri WebviewWindow |
| Ghost Suggestor | Yes (egui viewport) | Yes | Tauri WebviewWindow |
| Variable Input (F11) | Yes (egui viewport) | Yes | Tauri WebviewWindow or modal |

### 2.3 Modals

| Modal | egui | Tauri |
|-------|------|-------|
| Snippet Editor (Add/Edit/Promote) | Yes | Yes |
| View Full Content | Yes | Yes |
| Delete confirmation | Yes | Yes |
| Clear All (clipboard) | Yes | Yes |
| Promote to Snippet | Yes | Yes |

---

## 3. Tauri Migration Roadmap

### Phase 1: Core Tabs (egui parity)

| Step | Task | Status | Output |
|------|------|--------|--------|
| 1.1 | Tauri commands: `load_library`, `save_library`, `get_app_state`, `save_settings`, `get_ui_prefs`, `save_ui_prefs` | Done | src-tauri/lib.rs |
| 1.2 | Library tab: search, categories, snippet list (Load/Save, sortable columns, reorder, persist, row shading) | Done | tauri-app/src/index.html |
| 1.2b | Library tab: Add/Edit/Delete snippets | Done | add_snippet, update_snippet, delete_snippet commands; Actions column |
| 1.3 | Snippet Editor modal: trigger, options, category, content, templates | Done | tauri-app/src/index.html |
| 1.4 | Configuration tab: Templates, Sync, Discovery, Ghost Suggestor, Ghost Follower, Clipboard History config | Done | tauri-app/src |
| 1.5 | Clipboard History tab: list, context menu, Promote, View Full, Delete, Clear All | Done | tauri-app/src |
| 1.6 | Script Library tab: {run:} security, allowlist, global JS editor | Done | tauri-app/src |
| 1.7 | StoragePort: TauriStorageAdapter (JSON file in app data dir) | Done | adapters/storage |
| 1.8 | FileDialogPort: rfd or Tauri dialog plugin | Pending | rfd or tauri-plugin-dialog |

### Phase 2: Secondary Windows

| Step | Task | Output |
|------|------|--------|
| 2.1 | Ghost Follower as Tauri WebviewWindow | tauri-app |
| 2.2 | Ghost Suggestor as Tauri WebviewWindow | tauri-app |
| 2.3 | Variable Input (F11) viewport | Modal or secondary window |
| 2.4 | Hotstring listener integration | Same as egui; platform layer |

### Phase 3: Polish and Optional Features

| Step | Task | Output |
|------|------|--------|
| 3.1 | System tray, notifications | Tauri plugins |
| 3.2 | Performance/Analytics tab (if AHK parity desired) | Tauri frontend |
| 3.3 | Appearance/Transparency tabs (if AHK parity desired) | Tauri frontend |
| 3.4 | Dashboard (if AHK parity desired) | Tauri frontend |
| 3.5 | Theme (Dark/Light) | Tauri frontend |

---

## 4. Tauri Implementation Alternatives

### 4.1 Frontend Framework

| Option | Pros | Cons | Recommendation |
|--------|------|------|----------------|
| **Vanilla HTML/CSS/JS** | No build step, minimal deps, full control | More boilerplate, manual state | Good for Phase 1 |
| **Vite + Vanilla** | Fast dev, HMR, no framework lock-in | Adds build step | **Recommended** |
| **Vite + React** | Component model, large ecosystem | Heavier, learning curve | If team prefers React |
| **Vite + Vue** | Lightweight, reactive | Smaller ecosystem than React | Alternative |
| **Vite + Svelte** | Compile-time, smaller bundle | Newer than React/Vue | Alternative |

**DECISION REQUIRED:** Which frontend stack for Phase 1? Vanilla (or Vite+Vanilla) is fastest to parity; React/Vue if team prefers component framework.

### 4.2 State Management

| Option | Pros | Cons |
|--------|------|------|
| **Tauri invoke() only** | Simple, no extra deps | All state in Rust; frontend re-fetches on each action |
| **Tauri + frontend state** | Responsive UI, fewer round-trips | Sync complexity between Rust and frontend |
| **Tauri + Tauri events** | Rust can push updates to frontend | Event wiring overhead |

**Recommendation:** Start with invoke() for Phase 1; add Tauri events for real-time updates (Clipboard History, Sync status) in Phase 2.

### 4.3 Snippet Editor Layout

| Option | Pros | Cons |
|--------|------|------|
| **Modal (dialog)** | Matches egui; focused; no tab switch | Smaller; may feel cramped for long content |
| **Inline tab** | Matches AHK; more space | Tab switch; context loss |
| **Slide-out panel** | Modern; no full modal | More complex layout |

**DECISION REQUIRED:** Modal (egui parity) vs Tab (AHK parity) vs Slide-out panel for Phase 1.

### 4.4 Variable Input (F11)

| Option | Pros | Cons |
|--------|------|------|
| **Secondary Tauri window** | Matches egui viewport; separate process | Window management |
| **Modal in main window** | Simpler; single window | May block main UI |
| **Floating overlay** | Always visible | Positioning complexity |

**Recommendation:** Secondary Tauri window for parity with egui viewport.

### 4.5 Ghost Follower / Suggestor Positioning

| Option | Pros | Cons |
|--------|------|------|
| **Tauri WebviewWindow** | Native window; can overlay | Caret positioning requires Rust backend |
| **Tauri overlay plugin** | May simplify | Check plugin availability |
| **HTML overlay in main window** | Simpler | Cannot overlay other apps |

**Recommendation:** Tauri WebviewWindow; backend computes caret position via platform layer (same as egui).

---

## 5. Key Decisions Requiring User Input

| # | Decision | Options | Impact |
|---|----------|---------|--------|
| 1 | **Frontend stack** | Vanilla, Vite+Vanilla, React, Vue, Svelte | Dev speed, build complexity |
| 2 | **Snippet Editor layout** | Modal, Tab, Slide-out panel | UX parity |
| 3 | **AHK-only features scope** | Include in Tauri Phase 2+ vs defer | Performance, Appearance, AOT, Dashboard |
| 4 | **egui deprecation timeline** | After Tauri parity vs keep indefinitely | Maintenance burden |
| 5 | **Shared library path** | egui and Tauri use same JSON path? | User migration; config sync |

---

## 6. Recent Integrations and Enhancements (2026-02-28)

### Clipboard History (F38-F42)

| Feature | Implementation | Status |
|---------|----------------|--------|
| Real-time clipboard monitoring | `application/clipboard_history.rs` | Done |
| App and Window Title metadata | Windows clipboard listener (WM_CLIPBOARDUPDATE) | Done |
| Right-click context menu | `ui/clipboard_history_tab.rs` | Done |
| Copy to Clipboard | arboard::Clipboard::set_text | Done |
| View Full Content modal | `ui/modals.rs` clip_view_content_modal | Done |
| Delete Item with confirmation | `ui/modals.rs` clip_delete_confirm_dialog | Done |
| Promote to Snippet | `ui/modals.rs` promote_modal | Done |
| Clear All History with confirmation | `ui/modals.rs` clip_clear_confirm_dialog | Done |
| Max depth config (5-100) | ClipboardHistoryConfig | Done |

### Ghost Suggestor (F43-F47)

| Feature | AHK Parity | Status |
|---------|------------|--------|
| AlwaysOnTop overlay | +AlwaysOnTop | Done |
| Tab accept, Ctrl+Tab cycle | Tab/Ctrl+Tab | Done |
| Configurable display duration | New (0=no auto-hide) | Done |
| Create Snippet button | Enhancement | Done |
| Ignore button | HideGhost | Done |
| Cancel button | Esc | Done |
| Debounce, offset | ghostOverlayOffsetX/Y | Done |

### Tauri (2026-03-02)

| Item | Status |
|------|--------|
| tauri-app skeleton | Done (builds, runs) |
| tauri.conf.json | Fixed (devUrl, bundle schema) |
| icon.ico | Done |
| serde_json | Done |
| Build script | `scripts/build.ps1` | Done |
| Tauri commands | get_app_state, load_library, save_library, set_library_path, save_settings, get_ui_prefs, save_ui_prefs, add_snippet, update_snippet, delete_snippet | Done |
| Library tab frontend | Load, Save, search, snippet table | Done |
| Library tab UI enhancements | Title removed; tabs renamed; columns (Profile, Category, Trigger, Content Preview, AppLock, Options, Last Modified); sortable; draggable reorder; persist last tab + column order; row shading; Add Snippet, Edit, Delete, Snippet Editor modal, Delete confirmation | Done |
| Storage keys | UI_LAST_TAB, UI_COLUMN_ORDER | Done |
| AppState try_save_library | Added to lib for Tauri | Done |
| Unit tests (app_state) | 6 tests | Done |
| Tauri command tests | 4 integration tests | Done |

---

## 7. Implementation Details

### Clipboard History Architecture

- **On Windows:** Uses `AddClipboardFormatListener` with a message-only window. On `WM_CLIPBOARDUPDATE`, reads clipboard, queries foreground window via `WindowsWindowAdapter`, and calls `add_entry(text, process_name, window_title)`. AHK parity for App/Window Title.
- **On other platforms:** Poll loop (500ms) in background thread. App/Window Title remain empty.
- **Suppression:** `suppress_for_duration` prevents adding to history during our own paste (e.g. Copy to Clipboard from context menu).

### Config Loading (Scripting)

- `load_config()` no longer overwrites `SCRIPTING_CONFIG` if already set. Enables tests to call `set_scripting_config` before `process_with_config` without being overwritten by `get_registry()` -> `load_config()`.

---

## 8. Testing Status

### Test Counts (2026-02-28)

| Crate | Unit Tests | Integration Tests | Doc-Tests | Total |
|-------|------------|------------------|-----------|-------|
| digicore-text-expander | 90 | 30 | 1 | 121 |

### Run Command

```bash
cargo test -p digicore-text-expander -- --test-threads=1
# or
.\scripts\test.ps1
```

---

## 9. Known Limitations

- **Windows clipboard listener:** No explicit shutdown path; runs until process exit.
- **Non-Windows:** Poll loop used; App/Window Title remain empty.
- **egui:** No Performance, Appearance, AOT Manager, Dashboard tabs (AHK-only).
- **Tauri:** All Phase 1 tabs complete (Library, Configuration, Clipboard History, Script Library); hotstring listener runs; expansion works; Ghost Follower/Suggestor overlays use egui (not shown in Tauri; future: WebviewWindow).

---

## 10. Related Documentation

- [UI_DECOUPLING_IMPLEMENTATION_PLAN.md](./UI_DECOUPLING_IMPLEMENTATION_PLAN.md) - Ports, adapters, AppState extraction
- [TAURI_MIGRATION_PLAN.md](./TAURI_MIGRATION_PLAN.md) - Tauri architecture, run commands
- [EGUI_TO_TAURI_MIGRATION_NOTES.md](./EGUI_TO_TAURI_MIGRATION_NOTES.md) - Dual-binary summary
- [CLIPBOARD_HISTORY.md](./CLIPBOARD_HISTORY.md)
- [SCRIPTING_USER_GUIDE.md](./SCRIPTING_USER_GUIDE.md)
- [AHK Legacy Reference](../../../AHK_-_PROD-MAIN_STARTUP-SCRIPTZ/ACTIVE-Prod-LIVE-Apps/Text-Expansion/AHK_TE_Pro_GUI.ahk)
