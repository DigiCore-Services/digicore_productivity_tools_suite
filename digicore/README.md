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
.\scripts\build.ps1 -Target Tauri          # Build (debug)
.\scripts\build.ps1 -Target Tauri -Release  # Build (release)

# Run in dev mode
cd tauri-app
npm run tauri dev
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
└── scripts/
```

---

## Development

```powershell
# Tauri dev (hot reload)
cd tauri-app
npm run tauri dev

# With logging
$env:RUST_LOG="info"; npm run tauri dev

# egui GUI (alternative)
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
