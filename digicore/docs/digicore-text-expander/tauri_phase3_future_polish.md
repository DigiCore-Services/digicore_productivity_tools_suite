# Tauri Phase 3: Future Polish & Beyond Elite

## DigiCore Text Expander - Long-Term Roadmap

This document outlines Phase 3 recommendations that build on [Document 1](./tauri_analysis_recommendations.md) (Foundation) and [Document 2](./tauri_advanced_innovations.md) (Elite). It focuses on advanced Tauri 2.x features, community plugins, platform-specific polish, and long-term robustness.

---

## 1. Advanced Tauri 2.x Features

### 1.1 Persisted Scope

- **Plugin**: `tauri-plugin-persisted-scope`
- **Purpose**: Persist runtime scope changes (e.g., permission grants, window state) to filesystem.
- **Use case**: Preserve user consent and UI state across restarts without manual storage.

### 1.2 Logging & Diagnostics

- **Plugin**: `tauri-plugin-log` (official)
- **Purpose**: Configurable logging with JS-to-Rust log bridging.
- **Use case**: Diagnostic mode for power users; real-time expansion failure reasons (AppLock, trigger mismatch, etc.).
- **Community**: `tauri-plugin-tracing` for structured logging, flamegraph profiling.

### 1.3 HTTP Client

- **Plugin**: `tauri-plugin-http` (built-in)
- **Purpose**: Rust-based HTTP client; no CORS, no browser limits.
- **Use case**: WebDAV sync, update checks, remote snippet fetch. Offload from frontend fetch.

### 1.4 Process & CLI

- **Plugin**: `tauri-plugin-process`, `tauri-plugin-cli`
- **Purpose**: Process info, CLI argument parsing.
- **Use case**: `digicore://` deep links from CLI; `--open-settings`, `--add-snippet "sig"`.

---

## 2. Community Plugins of Interest


| Plugin                         | Purpose                    | Notes                                          |
| ------------------------------ | -------------------------- | ---------------------------------------------- |
| `tauri-plugin-context-menu`    | Native OS context menus    | Replace custom right-click menus               |
| `tauri-plugin-theme`           | Dynamic theme switching    | Sync with system theme; beyond manual toggle   |
| `tauri-plugin-aptabase`        | Privacy-first analytics    | Optional usage stats; respect user privacy     |
| `tauri-plugin-prevent-default` | Disable browser shortcuts  | Prevent accidental Ctrl+W, etc. in webview     |
| `tauri-plugin-screenshots`     | Window/monitor screenshots | Potential for snippet preview or documentation |
| `tauri-plugin-cache`           | Disk + memory caching      | TTL, compression; for sync or large assets     |


---

## 3. Platform-Specific Polish

### 3.1 Windows

- **Mica/Acrylic**: `window-vibrancy` (see Document 2).
- **Taskbar**: Consider `AppUserModelID` for proper grouping and jump list (e.g., "New Snippet", "Open Library").
- **Auto-start**: Registry or Start Menu shortcut via `tauri-plugin-autostart`.

### 3.2 macOS (Future)

- **Vibrancy**: `apply_vibrancy()` from `window-vibrancy`; different API than Windows.
- **Menu bar**: Optional menu-bar-only mode (like `tauri-macos-menubar-app-example`).
- **Spotlight-style**: Global palette similar to `tauri-macos-spotlight-example`.

### 3.3 Linux

- **AppIndicator**: Tray behavior varies (GTK vs KDE). Test `core:tray` across distros.
- **Portal**: Consider `xdg-desktop-portal` for file dialogs if native dialogs are inconsistent.

---

## 4. Accessibility & Inclusivity

- **Screen readers**: Ensure semantic HTML, ARIA labels. React/Radix components help.
- **Keyboard navigation**: Full tab order, Escape to close modals, arrow keys in lists.
- **High contrast**: Support Windows High Contrast mode; test `prefers-contrast` media query.
- **Reduced motion**: Respect `prefers-reduced-motion`; disable or simplify animations.

---

## 5. Performance & Scalability

### 5.1 Virtualization

- **Large snippet lists**: Use virtualized lists (e.g., `@tanstack/react-virtual`) when library exceeds ~500 items.
- **Partial loading**: With SQLite (Document 2), load only visible rows + prefetch.

### 5.2 Web Workers

- **Fuzzy search**: Move Fuse.js or equivalent to Worker; keep main thread responsive.
- **Template processing**: Heavy `{js:...}` or `{run:...}` in Worker where safe.

### 5.3 Lazy Loading

- **Tabs**: Lazy-load Clipboard and Script panels on first visit.
- **Modals**: Defer snippet editor DOM until opened.

---

## 6. Security & Trust

- **CSP**: Content Security Policy for webview; restrict inline scripts if migrating to bundled frontend.
- **Sandbox**: Ensure `{run:}` allowlist is enforced; consider `tauri-plugin-stronghold` for sensitive config.
- **Updates**: Sign updates; use `tauri-plugin-updater` with verified endpoints.

---

## 7. Integration Patterns

### 7.1 Type-Safe IPC

- **Tauri Specta**: Generate TypeScript types from Rust commands.
- **TauRPC**: Type-safe RPC wrapper for Tauri.
- **Benefit**: Fewer runtime errors; better IDE support.

### 7.2 Vite Integration

- **vite-plugin-tauri**: Integrate Tauri in Vite project for HMR, optimized builds.
- **Prerequisite**: Framework migration (Document 1 Phase 2).

---

## 8. Phase 3 Implementation Order (Suggested)

**Prerequisite**: Document 1 (Foundation) complete. Document 2 (Elite) and Document 3 (this doc) build on it.

| # | Task | Source Doc | Status |
|---|------|------------|--------|
| 1 | ~~**Diagnostics**~~ | Doc 3 | Done |
| 2 | ~~**CLI + Deep Link**~~ | Doc 2, 3 | Done |
| 3 | ~~**Context menu**~~ | Doc 2, 3 | Done (Radix ContextMenu) |
| 4 | ~~**Theme sync**~~ | Doc 3 | Done (prefers-color-scheme) |
| 5 | **Virtualization**: `@tanstack/react-virtual` when library exceeds ~500 items | Doc 2, 3 | Not started |
| 6 | **Platform polish**: Windows Mica first (window-vibrancy); macOS/Linux as needed | Doc 2 | Not started |

---

## 9. Related Documentation

- [tauri_analysis_recommendations.md](./tauri_analysis_recommendations.md) – Foundation & Phase 1–3
- [tauri_advanced_innovations.md](./tauri_advanced_innovations.md) – Elite features
- [TAURI_IMPLEMENTATION_STATUS.md](./TAURI_IMPLEMENTATION_STATUS.md) – Current status
- [Tauri Plugin Directory](https://v2.tauri.app/plugin/) – Official and community plugins

