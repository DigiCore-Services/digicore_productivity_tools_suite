# Tauri Migration Plan - DigiCore Text Expander

**Version:** 1.1  
**Created:** 2026-03-02  
**Last Updated:** 2026-03-02  
**Status:** Phase 1-2 partial; Phase 3 Library tab in progress  
**Product:** DigiCore Text Expander  
**Architecture:** Hexagonal, Configuration-first, SOLID, SRP

---

## 1. Executive Summary

This document defines the migration of the DigiCore Text Expander from **Azul** to **Tauri**. The migration introduces a dual-binary architecture: **egui** (native Rust UI) and **Tauri** (web frontend + Rust backend). Tauri provides a stable, mature foundation with multi-OS support (Windows, Linux, macOS) and a roadmap for mobile/tablet.

**Key Outcomes:**
- **egui binary** remains the primary native UI for power users and minimal footprint
- **Tauri binary** provides web-based UI with HTML/CSS/JS frontend for flexibility and cross-platform consistency
- Framework-agnostic core (AppState, ports, adapters) preserved; Tauri uses TauriStorageAdapter, TauriWindowAdapter, etc.
- Single codebase; feature flags and separate binaries for each GUI backend

---

## 2. Rationale for Tauri

| Criterion | Tauri | Azul |
|-----------|-------|------|
| **Stability** | Production-ready, v2.x stable | Alpha, APIs in flux |
| **Maturity** | Active development, large ecosystem | Early stage, limited adoption |
| **Multi-OS** | Windows, Linux, macOS first-class | Windows build issues (objc2, codegen) |
| **Mobile/Tablet** | Roadmap for Tauri Mobile | Not applicable |
| **Distribution** | Single executable, small footprint | DLL dependencies, build complexity |
| **UI Stack** | Web tech (HTML/CSS/JS) or any frontend | Native DOM (WebRender) |

**Decision:** Tauri is more stable, mature, and supports the target platforms. The web frontend allows rapid UI iteration and future mobile/tablet expansion.

---

## 3. Architecture: Dual-Binary Plan

```
digicore/
  crates/
    digicore-core/           # Domain, ports (unchanged)
    digicore-text-expander/  # Application logic, adapters
      src/
        main.rs              # egui binary (--gui=egui)
        main_tauri.rs        # Tauri binary entry (invokes tauri-app)
  tauri-app/                 # Tauri application
    src/                     # Web frontend (HTML, CSS, JS)
    src-tauri/               # Rust backend (Tauri commands)
```

**Binary Layout:**
| Binary | Feature | Purpose |
|--------|---------|---------|
| `digicore-text-expander` | `gui-egui` (default) | egui native UI |
| `digicore-text-expander-tauri` | `gui-tauri` | Tauri web UI (via tauri-app) |

**Note:** The Tauri binary is built from `tauri-app/` (Tauri CLI). The `digicore-text-expander` crate exposes a library that `tauri-app/src-tauri` depends on. The `main_tauri.rs` binary may invoke the Tauri app or the Tauri app may be run separately via `npm run tauri dev`.

---

## 4. Tauri Project Structure

```
digicore/tauri-app/
  package.json               # npm scripts, Tauri CLI
  src/
    index.html               # Frontend entry
    assets/                  # CSS, JS, images (optional)
  src-tauri/
    Cargo.toml               # Depends on digicore-text-expander, digicore-core
    tauri.conf.json          # Tauri app config
    capabilities/
      default.json           # Permissions
    src/
      main.rs                # Tauri builder, run
      lib.rs                 # Tauri commands (invoke digicore-text-expander)
    build.rs                 # Build script
```

---

## 5. Adapter Mapping

| Azul Adapter | Tauri Adapter | Notes |
|--------------|---------------|-------|
| AzulStorageAdapter | TauriStorageAdapter | JsonFileStorageAdapter (same path); Tauri can use tauri::api::path for app data dir |
| AzulWindowAdapter | TauriWindowAdapter | Tauri WebviewWindow API for secondary windows (Ghost Follower, Ghost Suggestor, Variable Input) |
| AzulTimerAdapter | TauriTimerAdapter | Channel-based or tauri::async_runtime::spawn with delay |
| AzulViewportState | TauriViewportState | Shared state for viewport close/command requests |
| AzulAppConfig | TauriAppConfig | AppState + Tauri adapters |

**Port Implementations:**
- `StoragePort`: TauriStorageAdapter (JSON file in app data dir)
- `WindowPort`: TauriWindowAdapter (WebviewWindow::create, close, etc.)
- `TimerPort`: TauriTimerAdapter (spawn delayed task, emit event to frontend)
- `FileDialogPort`: RfdFileDialogAdapter (unchanged; framework-agnostic)

---

## 6. Implementation Phases

### Phase 1: Foundation
| Step | Task | Status | Output |
|------|------|--------|--------|
| 1.1 | Rename gui-azul to gui-tauri in Cargo.toml | Done | Feature flag, binary names |
| 1.2 | Replace Azul adapters with Tauri adapters (stubs) | Done | tauri_storage.rs, tauri_window.rs, tauri_timer.rs |
| 1.3 | Replace ui/azul with ui/tauri (minimal stub) | Done | mod.rs, app.rs documenting integration points |
| 1.4 | Create tauri-app/ structure | Done | package.json, src-tauri, tauri.conf.json |
| 1.5 | Wire digicore-text-expander as lib dependency | Done | src-tauri/Cargo.toml |

### Phase 2: Tauri Backend Commands
| Step | Task | Status | Output |
|------|------|--------|--------|
| 2.1 | Implement Tauri commands in src-tauri/src/lib.rs | Done | load_library, save_library, get_app_state, save_settings, get_ui_prefs, save_ui_prefs |
| 2.2 | Wire StoragePort via TauriStorageAdapter | Done | Persistence in app data dir; UI prefs (last tab, column order) |
| 2.3 | Implement WindowPort for secondary windows | Pending | Ghost Follower, Ghost Suggestor, Variable Input |

### Phase 3: Frontend
| Step | Task | Status | Output |
|------|------|--------|--------|
| 3.1 | Build HTML/CSS/JS tabs (Library, Configuration, Clipboard, Script) | Partial | Library tab complete; Config/Clipboard/Script placeholders |
| 3.2 | Invoke Tauri commands from frontend | Done | IPC via invoke() |
| 3.3 | Implement modals (snippet editor, delete confirm, etc.) | Pending | Modal dialogs |

### Phase 4: Integration and Polish
| Step | Task | Output |
|------|------|--------|
| 4.1 | Ghost Follower, Ghost Suggestor as Tauri windows | Secondary webview windows |
| 4.2 | Variable Input (F11) viewport | Modal or secondary window |
| 4.3 | Hotstring listener integration | Same as egui; platform layer |
| 4.4 | System tray, notifications | Tauri plugins |

---

## 7. Multi-OS and Mobile Foundation

**Desktop:** Tauri v2 supports Windows, Linux, macOS out of the box. Same Rust backend; frontend can use responsive CSS for different screen sizes.

**Mobile/Tablet (Future):** Tauri has a mobile roadmap. The digicore-text-expander library is already platform-agnostic; Tauri mobile would add a new binary/frontend. No changes to core required.

---

## 8. Prerequisites

- **Node.js/npm** - for Tauri CLI and frontend
- **Rust** - for backend compilation
- **icon.ico** - Required for Windows build. If missing, run:
  `python tauri-app/scripts/create-icon.py` (or the build script will create it automatically)

## 9. Run Commands

### Reusable build script (recommended)
```powershell
cd digicore
.\scripts\build.ps1                    # Build both egui + Tauri
.\scripts\build.ps1 -Target Egui       # egui only
.\scripts\build.ps1 -Target Tauri      # Tauri only (runs npm install first)
.\scripts\build.ps1 -Release           # Release builds
.\scripts\build.ps1 -Target Tauri -NoInstall   # Skip npm install
```

### egui binary (manual)
```powershell
cargo run -p digicore-text-expander
cargo run -p digicore-text-expander -- --gui=egui
```

### Tauri binary (manual)
```powershell
cd digicore/tauri-app
npm install
npm run tauri dev
# or
npm run tauri build
```

### Build both (manual)
```powershell
cargo build -p digicore-text-expander
cd digicore/tauri-app; npm run tauri build
```

---

## 10. References

- [Tauri Documentation](https://v2.tauri.app/)
- [Tauri v2 Migration](https://v2.tauri.app/start/migrate/)
- [UI_DECOUPLING_IMPLEMENTATION_PLAN.md](./UI_DECOUPLING_IMPLEMENTATION_PLAN.md)
- [EGUI_TO_TAURI_MIGRATION_NOTES.md](./EGUI_TO_TAURI_MIGRATION_NOTES.md)
