# DigiCore Text Expander

Cross-platform text expansion application built with Rust and Tauri. Migrated from AutoHotkey (AHK); AHK is legacy, Rust is the target.

**Stack:** Rust (digicore-core, digicore-text-expander) + Tauri 2 + React + Vite + TypeScript + Tailwind CSS.

---

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) (cargo)
- [Node.js](https://nodejs.org/) (npm)
- Windows 10/11 (primary target)

### Build & Run

```powershell
# From digicore directory
cd digicore
.\scripts\build.ps1 -Target Tauri              # Build (debug)
.\scripts\build.ps1 -Target Tauri -NoInstall   # Same, skip npm install if deps are current
.\scripts\build.ps1 -Target Tauri -Release     # Build (release)

# Run in dev mode (from digicore\tauri-app)
cd tauri-app
npm run tauri -- dev
```

### Installer Output

After `npm run tauri build`, installers are in:

- `digicore\target\release\bundle\msi\` – MSI installer
- `digicore\target\release\bundle\nsis\` – NSIS setup (.exe)

---

## MSI vs NSIS Installers

Tauri produces two Windows installer formats. Choose based on your needs:

| Aspect | MSI | NSIS |
|--------|-----|------|
| **Format** | `.msi` (Windows Installer) | `.exe` (Nullsoft Scriptable Install System) |
| **Build** | Windows only (requires WiX Toolset) | Cross-compile from Linux/macOS |
| **Uninstall** | Via Settings → Apps → Installed apps | Via Add/Remove Programs (uninstall.exe) |
| **Upgrades** | Native Windows Installer upgrade flow | Detects existing install, offers uninstall-first |
| **Enterprise** | Often preferred (GPO, silent install) | Common for consumer apps |
| **Size** | Typically similar | Typically similar |

**Recommendation:** Use **MSI** for enterprise or when building on Windows. Use **NSIS** when cross-compiling from Linux/macOS.

**Uninstall & Reinstall:** Both support standard Windows uninstall. Uninstall via Settings → Apps, then run the new installer. User data (`%APPDATA%\DigiCore`) persists across uninstall/reinstall.

---

## Key Features

- **Text Expansion** – Trigger-based expansion with snippets, categories, profiles
- **Discovery** – Detects repeated phrases; suggests Snooze, Ignore, or Promote to Snippet via in-app banner + toast
- **Ghost Follower** – Pinned snippets sidebar (edge-anchored)
- **Clipboard History** – Configurable depth, context menu (Copy, Promote, Delete)
- **Scripting** – `{js:}`, `{http:}`, `{date}`, `{time}`, `{clipboard}`, `{var:}`, `{choice:}`, etc.
- **Tray** – Runs in background; right-click tray icon for View Console, View Ghost Follower, Exit

---

## Project Structure

```
digicore/
├── crates/
│   ├── digicore-core/       # Domain, ports, adapters
│   └── digicore-text-expander/  # Expansion engine, discovery, hotstring, Ghost Suggestor
├── tauri-app/               # Tauri + React frontend
│   ├── src/                 # React components, App.tsx
│   └── src-tauri/           # Rust backend, lib.rs, api.rs
├── docs/
└── scripts/                 # build.ps1, dev-tauri-kms-debug.ps1, test.ps1
```

---

## Development

### Tauri dev (hot reload)

```powershell
cd tauri-app
npm run tauri -- dev
```

Use `npm run tauri -- dev` (note the `--`) so `dev` is passed to the Tauri CLI.

### Build script vs KMS debug dev script

| Command | What it does |
|--------|----------------|
| `.\scripts\build.ps1 -Target Tauri [-NoInstall]` | **Compiles / packages** the Tauri app (Rust + frontend steps). Produces binaries/installer inputs, then **exits**. Does **not** start interactive dev or set `RUST_LOG`. |
| `.\scripts\dev-tauri-kms-debug.ps1` | **Runs** `npm run tauri -- dev` from `tauri-app` with **`RUST_LOG` set for KMS embedding diagnostics**. Keeps the normal dev loop (Vite + Tauri). Does **not** replace a full build when you need a clean compile. |

Typical flow: run **`build.ps1`** when you need to verify or refresh a **build**; run **`dev-tauri-kms-debug.ps1`** when you are **troubleshooting KMS note embedding / “Re-embed vault” (D6)** and want structured logs without drowning in unrelated `TRACE` noise.

### KMS embedding logs (`kms_embed` target)

Rust code logs KMS text embedding details under the log target **`kms_embed`** (see `tauri-app/src-tauri/src/embedding_service.rs`). Failures also emit **`WARN`** on that target.

From **`digicore`** (recommended):

```powershell
.\scripts\dev-tauri-kms-debug.ps1
```

Default `RUST_LOG` is `info,kms_embed=debug` (general **info** plus **debug** for `kms_embed`). For more detail on that target only:

```powershell
.\scripts\dev-tauri-kms-debug.ps1 -RustLog "info,kms_embed=trace"
```

From **`digicore\tauri-app`** (one-liner):

```powershell
$env:RUST_LOG = "info,kms_embed=debug"; npm run tauri -- dev
```

### KMS embedding diagnostic log file

Embedding **WARN** / **ERROR** lines (D6 migration, file read, fastembed init/embed, sqlite vector upsert, pipeline stages) are also **appended** to a UTF-8 text file for offline review:

- **Path:** `%APPDATA%\DigiCore\logs\kms_embedding.log` (same as `dirs::config_dir()/DigiCore/logs/` on your OS).
- **Session markers:** Each D6 / re-embed job writes an **INFO** header block (generation, vault, model, chunk policy, full path to this file).
- **Optional file DEBUG:** Set `KMS_EMBED_LOG_FILE_DEBUG=1` before starting the app to append per-note **embed_start** and context lines to that file (console still uses `RUST_LOG`).

The in-app path is shown under **Configurations and Settings** > **KMS Search and embeddings**. The backend exposes `kms_get_embedding_diagnostic_log_path` if you need it from the UI or scripts.

### Other logging and egui

```powershell
# Broader Rust logging (all modules at info)
$env:RUST_LOG = "info"; npm run tauri -- dev

# egui GUI (alternative to Tauri)
cargo run -p digicore-text-expander
```

---

## Testing

```bash
cargo test --workspace
```

---

## Documentation

- [Tauri User Guide](docs/digicore-text-expander/TAURI_USER_GUIDE.md) – Build, dev, SQLite, features
- [Scripting User Guide](docs/digicore-text-expander/SCRIPTING_USER_GUIDE.md) – JavaScript, HTTP, placeholders
- [Clipboard History](docs/digicore-text-expander/CLIPBOARD_HISTORY.md) – Clipboard tab, context menu
- [Implementation Plan](docs/digicore-text-expander/IMPLEMENTATION_PLAN.md) – Status, testing
- [Tauri Implementation Status](docs/digicore-text-expander/TAURI_IMPLEMENTATION_STATUS.md) – Tauri features
- [Changelog](CHANGELOG.md) – Recent changes

---

## Configuration & Data

- **Config:** `%APPDATA%\DigiCore\config\`
- **Library:** Path configurable in app (default: `%APPDATA%\DigiCore\`)
- **SQLite:** `digicore.db` (snippets sync)

---

## Version

See `tauri.conf.json` and `package.json` for version. Bump version before releases for correct upgrade/uninstall behavior.
