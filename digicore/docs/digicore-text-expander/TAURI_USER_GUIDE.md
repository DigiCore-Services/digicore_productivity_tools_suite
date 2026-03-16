# DigiCore Text Expander - Tauri User Guide

**Version:** 1.1  
**Last Updated:** 2026-03-04  
**Product:** DigiCore Text Expander (Tauri + React + Vite)

---

## Table of Contents

1. [Overview](#1-overview)
2. [Prerequisites](#2-prerequisites)
3. [Build](#3-build)
4. [Development](#4-development)
5. [SQLite and Library Sync](#5-sqlite-and-library-sync)
6. [Key Features](#6-key-features)
7. [Troubleshooting](#7-troubleshooting)

---

## 1. Overview

The Tauri app is the web-based GUI for DigiCore Text Expander, built with:

- **Frontend:** React 18, Vite, TypeScript, Tailwind CSS, Framer Motion
- **Backend:** Rust (digicore-text-expander, digicore-core crates)
- **Plugins:** SQLite, notifications, deep-link, autostart, window-state, updater, etc.

---

## 2. Prerequisites

- [Rust](https://rustup.rs/) (cargo)
- [Node.js](https://nodejs.org/) (npm)
- Windows 10/11 (primary target)

---

## 3. Build

### 3.1 Using the Build Script (Recommended)

From the `digicore` directory:

```powershell
# Build Tauri app (debug - faster compile)
.\scripts\build.ps1 -Target Tauri

# Build Tauri app (release - optimized)
.\scripts\build.ps1 -Target Tauri -Release

# Skip npm install (deps already installed)
.\scripts\build.ps1 -Target Tauri -NoInstall
```

The build script:

1. Runs `npm install` in `tauri-app` (unless `-NoInstall`)
2. Runs `npm run tauri build` (full Tauri build)
3. Tauri build runs `beforeBuildCommand` = `npm run build` = `tsc && vite build` (frontend)
4. Compiles Rust backend (including SQLite, notifications, etc.)
5. Bundles into installers (`.msi`, `.exe`)

**Output:** `digicore\target\release\bundle\` (or `target\debug\bundle\` for debug)

### 3.2 Manual Build

```powershell
cd digicore\tauri-app
npm install
npm run tauri build          # Release
npm run tauri build -- --debug   # Debug
```

### 3.3 Frontend-Only Build (No Rust)

```powershell
cd digicore\tauri-app
npm run build   # tsc && vite build
```

---

## 4. Development

### 4.1 Run Dev Server

```powershell
cd digicore\tauri-app
npm run tauri dev
```

This starts:

- Vite dev server (http://localhost:5173)
- Tauri dev window with hot-reload

### 4.2 Run Frontend Only (No Tauri)

```powershell
cd digicore\tauri-app
npm run dev   # Vite only
```

---

## 5. SQLite and Library Sync

### 5.1 Overview

The app uses SQLite (`digicore.db`) for future partial loading and search at scale. Your JSON library is synced to SQLite automatically.

### 5.2 When Sync Happens

- **Load:** After clicking Load in the Library tab
- **Save:** After clicking Save in the Library tab
- **Startup:** When the app loads with an existing library path and data

### 5.3 Using Your Existing JSON Library

1. Set the library path to your JSON file (e.g. `C:\...\text_expansion_library.json`)
   - Type the path manually, or click **Browse** to open a native file picker (filtered for .json)
2. Click **Load** in the Library tab
3. Data is loaded into the app and synced to SQLite

### 5.4 Viewing the SQLite Database

The database file is at:

- **Windows:** `%APPDATA%\com.digicore.text-expander\databases\digicore.db`

Open with:

- [DB Browser for SQLite](https://sqlitebrowser.org/)
- VS Code SQLite extensions
- Any SQLite client

**Tables:** `categories`, `snippets` (trigger, content, options, profile, app_lock, pinned, last_modified)

---

## 6. Key Features

| Feature | Description |
|---------|-------------|
| **Command Palette** | Shift+Alt+Space; fuzzy search; Enter=copy, Ctrl+E=edit |
| **Rich Notifications** | Library load/save toasts with "View Library" action |
| **Web Workers** | Fuzzy search runs off main thread (responsive UI) |
| **Accessibility** | ARIA labels, prefers-reduced-motion, prefers-contrast |
| **Virtualization** | Library tab virtualizes when >= 500 items |
| **Native Title Bar** | Uses OS window decorations (no dual header) |
| **Browse (Library)** | Native file picker (tauri-plugin-dialog) for selecting library .json file |
| **Prevent Default** | Disables Ctrl+W, F12, etc. in webview (dev tools enabled in debug builds) |
| **HTTP Client** | tauri-plugin-http for sync, updates (no CORS); URL scope in capabilities |
| **Persisted Scope** | File dialog paths persisted across restarts |

---

## 7. Troubleshooting

### Icon Not Updating (Still Shows Old Icon)

1. **Clean rebuild:** Delete `digicore\target\debug` (or `target\release`) and rebuild so the new icon is embedded in the .exe.
2. **Windows icon cache:** Windows caches icons. Try: close the app, restart Explorer (`taskkill /f /im explorer.exe` then `start explorer`), or restart the PC.
3. **Tray icon:** Uses `icons/icon.ico` from tauri.conf.json; ensure the ICO has 16x16 and 32x32 sizes.

### Build Fails

- Ensure `icon.ico` exists in `tauri-app\src-tauri\icons\`
- Run `npm install` in `tauri-app`
- Check Rust: `cargo --version`

### SQLite Sync Fails

- Sync errors are logged to console; app continues with JSON
- Ensure `sql:default` and `sql:allow-execute` in capabilities

### Dual Header (Fixed)

- App uses native OS title bar (`decorations: true`)
- Custom TitleBar component was removed to avoid duplicate headers

### TauRPC / IPC

- All IPC uses TauRPC proxy via `getTaurpc()` from `@/lib/taurpc`
- Do not use `invoke()`; it will not route to backend commands
- Bindings are generated at `src/bindings.ts` when running `tauri dev` or `tauri build`
- Ghost overlays (ghost-follower.html, ghost-suggestor.html) are Vite entry points; built to dist/

---

## Related Documentation

- [TAURI_IMPLEMENTATION_STATUS.md](./TAURI_IMPLEMENTATION_STATUS.md) - Implementation status
- [TYPE_SAFE_IPC_IMPLEMENTATION_PLAN.md](./TYPE_SAFE_IPC_IMPLEMENTATION_PLAN.md) - TauRPC migration plan (implemented)
- [tauri_advanced_innovations.md](./tauri_advanced_innovations.md) - Elite features
- [tauri_phase3_future_polish.md](./tauri_phase3_future_polish.md) - Future polish
