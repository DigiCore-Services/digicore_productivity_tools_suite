# Tauri Advanced Innovations: The "Elite" Roadmap
## Taking DigiCore Text Expander to a World-Class Level

This document outlines "Stage 2" recommendations that go beyond standard polish, focusing on deep system integration and cutting-edge visual techniques available in Tauri 2.0.

**Roadmap**: This is **Document 2 (Elite)**. Builds on [Document 1 – Foundation](./tauri_analysis_recommendations.md). For long-term polish, see [Document 3 – Phase 3](./tauri_phase3_future_polish.md).

---

## 1. Visual Immersion: "The Native+ Look"

### 1.1 Windows Mica & Acrylic Effects
Leverage the `window-vibrancy` crate in the Rust backend to apply modern Windows 11 surfaces.
*   **Recommendation**: Use **Mica** for the main management dashboard (subtle, battery-efficient) and **Acrylic** for the Ghost Suggestor/Follower overlays (high-contrast blur).
*   **Result**: The app will feel like a first-party Windows 11 utility (e.g., Settings or Microsoft Store).

### 1.2 Bespoke Custom Titlebars
Disable standard window decorations (`decorations: false`) and implement a custom HTML-based title area.
*   **Innovation**: Integrate the App Icon and Search Bar directly into the titlebar area (similar to modern browsers or Discord).
*   **Interactivity**: Use `data-tauri-drag-region` to allow dragging from any empty space in the header.

---

## 2. Deep OS Integration & Intelligence

### 2.1 The "Global Command Palette" (Spotlight for Snippets)
Instead of just a management window, implement a global "Quick Access" palette ($Shift + Alt + Space$).
*   **Fuzzy Search**: Use a fast fuzzy search library (like `Fuse.js` or a Rust-based matcher) to find snippets instantly.
*   **Actionable Results**: Pressing `Enter` inserts the snippet, `Ctrl+E` opens it for editing.

### 2.2 Native Context Menus
Move away from custom HTML context menus (right-click) and use `tauri-plugin-context-menu`.
*   **Benefit**: Matches system dark/light mode perfectly, supports screen readers natively, and feels "right" to Windows users.

### 2.3 Rich Notifications with Actions
When discovery finds a new pattern or a sync completes:
*   **Interactive Toasts**: Add buttons like "Enable Now" or "View Library" directly inside the Windows notification toast.

---

## 3. High-Performance Architecture (Elite Level)

### 3.1 SQLite for Infinite Scalability
If the user's snippet library grows to 10,000+ items, JSON files become slow to parse and search.
*   **Recommendation**: Integrate `tauri-plugin-sql` (SQLite).
*   **Elite Feature**: Implement "Partial Loading"—only load what is visible in the UI, keeping memory usage near zero.

### 3.2 Off-Main-Thread Processing (Web Workers)
Keep the UI thread (Main) dedicated strictly to 60FPS rendering.
*   **Architecture**: Move the fuzzy search engine and heavy template processing into a **Web Worker**.
*   **Result**: Zero "hiccups" or "freezes" even when searching through massive libraries or running complex JS scripts inside snippets.

### 3.3 Rust-Side Telemetry & Health Monitoring
Integrate `tauri-plugin-log` with a custom frontend "Terminal/Log" view.
*   **Diagnostic Mode**: Allow power users to see exactly why a snippet didn't expand (e.g., AppLock conflict) in real-time.

---

## 4. The "Invisible" Polish

| Feature | Status | Notes |
|---------|--------|-------|
| **DPI Awareness** | Done | Positioner plugin used for Ghost Follower on primary monitor; windows_monitor uses virtual-screen coordinates |
| **Mouse Passthrough** | Done | Ghost Suggestor: setIgnoreCursorEvents(true) when display_duration elapses or no suggestions |
| **Positioner plugin** | Done | Ghost Follower: moveWindow(Position.TopRight/TopLeft) when primary monitor; positioner:default in ghost-windows capability |
| **Single-Instance Deep Linking** | Done | digicore://; --open-settings, --add-snippet; single-instance forwards args |

---

## 5. Implementation Verification & Plugin Compatibility (2026-03-03)

### 5.0 Document 1 (Foundation) Status
*   **Phase 1–3 from tauri_analysis_recommendations.md**: Complete – autostart, single-instance, window-state, event-driven overlays, Vite+React+Tailwind, Shadcn/ui, Framer Motion, Lucide, tray menu (Show/Quit/Pause/Add Snippet), Analytics tab, Updater.

### 5.1 Elite Features Status (Document 2)
| Feature | Status | Notes |
|---------|--------|-------|
| Mica/Acrylic | Done | Tauri native `set_effects(Effect::Mica)`; transparent html/body |
| Window decorations | Done | Native OS title bar (decorations: true); custom TitleBar removed to avoid dual header |
| Global Command Palette | Done | Shift+Alt+Space; Fuse.js fuzzy search; Enter=copy, Ctrl+E=edit |
| Native context menus | Done | Tauri Menu.popup; Edit/Delete on snippet rows |
| Rich notifications | Done | Actionable toasts; View Library action; onAction listener |
| SQLite | Done | tauri-plugin-sql; schema; preload digicore.db |
| SQLite partial loading | Done | useSqliteRows + loadSnippetsPage when library > 5000 items |
| DPI / Positioner (Invisible) | Done | Ghost Follower uses positioner plugin; Ghost Suggestor mouse passthrough |

### 5.3 window-vibrancy (Mica/Acrylic)
*   **Crate**: `window-vibrancy` (Rust crate, not a Tauri plugin). Add to `Cargo.toml`: `window-vibrancy = "0.4"`.
*   **Windows 11**: `apply_mica()` for main window; `apply_acrylic()` for overlays. Requires `transparent: true` and `decorations: false` in window config.
*   **Note**: Linux/macOS use different APIs (e.g., `apply_vibrancy`). Plan platform-specific setup.

### 5.4 Positioner Plugin (Official)
*   **Purpose**: Move windows to well-known positions (TopRight, BottomLeft, TrayLeft, etc.). Port of `electron-positioner`.
*   **Use case**: Replace custom `windows_monitor` + `setPosition` logic for Ghost Follower edge placement. Supports tray-relative positions.
*   **Setup**: `npm run tauri add positioner`; enable `tray-icon` feature; wire `on_tray_event` for tray-relative modes.
*   **Permission**: `positioner:default` in capabilities.

### 5.5 Deep Linking + Single-Instance
*   **tauri-plugin-deep-linking**: Register `digicore://` protocol. Handle URLs like `digicore://open/settings` or `digicore://snippet/trigger`.
*   **tauri-plugin-single-instance**: On second launch, emit event to first instance; first instance focuses and optionally navigates via deep link payload.
*   **Integration**: Combine both for "focus existing + open tab" behavior.

### 5.6 Native Context Menu (Community)
*   **tauri-plugin-context-menu** (c2r0b): Native OS context menus. Replaces custom right-click HTML menus in snippet table.
*   **Benefit**: System dark/light mode, accessibility, consistent UX.

### 5.7 Rich Notifications
*   **Current**: `tauri-plugin-notification` used for library load/save toasts. Basic `title` + `body`.
*   **Limitation**: Windows toasts support actions (buttons) via `notification.action` or similar; verify Tauri plugin support for actionable toasts.
