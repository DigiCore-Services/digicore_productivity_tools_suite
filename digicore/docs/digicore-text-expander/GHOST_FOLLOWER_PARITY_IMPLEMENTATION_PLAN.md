# Ghost Follower Parity Implementation Plan

**DigiCore Text Expander – Ghost Follower AHK/egui Parity**

**Version:** 1.0  
**Last Updated:** 2026-03-02  
**Status:** Implemented  
**Purpose:** Comprehensive implementation plan for bringing the Tauri Ghost Follower into parity with the AHK script and egui implementations. Serves as a living document for progress tracking and future developer reference.

---

## 1. Executive Summary

### 1.1 Current State

The Tauri Ghost Follower currently:
- Displays as a full ribbon (280×420) at all times
- Uses `SMOKE_TEST_CENTER = true` (always centered on primary monitor)
- Has no pill/collapsed state
- Has no hover-to-expand behavior
- Collapse delay setting exists in Config but is unused
- Search filter is in Config tab (should be in-ribbon only)
- No drag-to-reposition
- No transparency control

### 1.2 Target Behavior (User Decisions)

| Decision | Choice |
|----------|--------|
| **Pill + Expand** | Option C: Pill by default, expand on hover, collapse after delay when idle |
| **Activation** | Hover over pill expands directly (no click); "Collapse delay" = how long expanded stays open before auto-collapsing |
| **Collapse to Icon** | Pill-only; no taskbar icon. When user closes pill, add tray menu item "Display Ghost Follower" to re-launch |
| **Search Filter** | Option A: Only in-ribbon search; remove from Config |
| **Drag** | Implement drag-to-reposition |
| **Transparency** | Add Config slider 10% (near transparent) to 100% (opaque); real-time updates |

### 1.3 Reference Implementations

- **AHK:** `AHK_-_PROD-MAIN_STARTUP-SCRIPTZ/ACTIVE-Prod-LIVE-Apps/Text-Expansion/AHK_TE_Pro_GhostFollower.ahk`
- **AHK GUI:** `AHK_-_PROD-MAIN_STARTUP-SCRIPTZ/ACTIVE-Prod-LIVE-Apps/Text-Expansion/AHK_TE_Pro_GUI.ahk` (Follower tab)
- **egui:** `crates/digicore-text-expander/src/main.rs` (F48-F59), `application/ghost_follower.rs`

---

## 2. Implementation Progress Tracker

Update checkboxes as work completes.

### Phase 1: Core Pill + Hover + Collapse
- [x] 1.1 Backend – touch, should_collapse, set_collapsed
- [x] 1.2 Frontend – pill/ribbon layout, hover expand, collapse delay
- [x] 1.3 Window resize API and wiring

### Phase 2: Close Pill + Tray Re-launch
- [x] 2.1 Close button on pill and ribbon
- [x] 2.2 Tray menu "Display Ghost Follower" when hidden

### Phase 3: Remove Search Filter from Config
- [x] 3.1 Remove from ConfigTab
- [x] 3.2 Clean up backend/config DTOs

### Phase 4: Transparency
- [x] 4.1 Config slider + backend
- [x] 4.2 Real-time opacity updates

### Phase 5: Drag-to-Reposition
- [x] 5.1 Draggable region
- [x] 5.2 Persist position

### Phase 6: Edge + Monitor Positioning
- [x] 6.1 Disable SMOKE_TEST_CENTER
- [x] 6.2 Positioner integration

---

## 3. Phase 1: Core Pill + Hover + Collapse

### 3.1 Backend – Collapse State & Activity

**Files:** `crates/digicore-text-expander/src/application/ghost_follower.rs`, `tauri-app/src-tauri/src/api.rs`, `tauri-app/src-tauri/src/lib.rs`

#### 3.1.1 Activity Tracking
- Ensure `touch()` is called when:
  - Mouse enters the Ghost Follower window
  - User interacts (scroll, click, type in search)
- Add API `ghost_follower_touch()` for frontend to call on activity

#### 3.1.2 Collapse State API
- Add `ghost_follower_get_should_collapse()` or include `should_collapse` in `get_ghost_follower_state` response
- Add `ghost_follower_set_collapsed(bool)` for frontend to set collapsed state
- Backend already has: `should_collapse(delay_secs)`, `set_collapsed()`, `touch()`, `FOLLOWER_LAST_ACTIVE`

#### 3.1.3 API Additions (api.rs)
```rust
async fn ghost_follower_touch() -> Result<(), String>;
async fn ghost_follower_set_collapsed(collapsed: bool) -> Result<(), String>;
```
- Extend `GhostFollowerStateDto` or `get_ghost_follower_state` to include `should_collapse: bool` and `collapse_delay_secs: u64`

### 3.2 Frontend – Pill vs Ribbon Layout

**Files:** `tauri-app/src/ghost-follower.ts`, `tauri-app/ghost-follower.html`

#### 3.2.1 State
- Add `let collapsed = true` (default)
- Sync with backend via `get_ghost_follower_state` or dedicated API

#### 3.2.2 Pill UI (collapsed)
- Small pill (e.g. 60×40 px) with "TE" or "•••" text
- No lists, no search, minimal chrome
- Close button (X) visible

#### 3.2.3 Ribbon UI (expanded)
- Current layout: search input, Pinned Snippets, Clipboard History
- Close button (X) visible

#### 3.2.4 Conditional Render
- `if (collapsed) { renderPill() } else { renderRibbon() }`

### 3.3 Hover-to-Expand

**File:** `tauri-app/src/ghost-follower.ts`

- On `mouseenter` of pill: set `collapsed = false`, call `ghost_follower_touch()`, resize window to ribbon size
- Use Tauri `WebviewWindow.setSize()` / `setInnerSize()` to switch between pill (60×40) and ribbon (280×420)

### 3.4 Collapse Delay (Config)

**File:** `tauri-app/src/ghost-follower.ts`

- Poll `get_ghost_follower_state` (or new API) for `should_collapse` using existing refresh interval (e.g. 3s)
- When `should_collapse === true` and expanded: set `collapsed = true`, resize to pill, call `ghost_follower_set_collapsed(true)`
- `collapse_delay_secs` comes from state (Config tab value)

### 3.5 Window Sizing from Rust

**Files:** `tauri-app/src-tauri/src/lib.rs`, `tauri-app/src-tauri/src/api.rs`

- Add API `ghost_follower_set_size(width: number, height: number)` or `ghost_follower_set_collapsed(collapsed: boolean)` that resizes the Ghost Follower window
- Pill size: ~60×40; ribbon size: ~280×420 (or from config)

---

## 4. Phase 2: Close Pill + Tray Re-launch

### 4.1 Close Button

**Files:** `tauri-app/src/ghost-follower.ts`, `tauri-app/ghost-follower.html`

- Add close (X) button to pill and ribbon
- On close: call `ghost_follower_hide()` (new API) to hide the window
- App continues running; only Ghost Follower window is hidden

### 4.2 Tray Menu – "Display Ghost Follower"

**File:** `tauri-app/src-tauri/src/lib.rs`

- Add tray menu item "Display Ghost Follower" (or "Show Ghost Follower")
- On click: show and focus Ghost Follower window
- Show when Ghost Follower is hidden; clicking re-displays it
- May already exist as "View Ghost Follower" – verify behavior matches (show when hidden)

---

## 5. Phase 3: Remove Search Filter from Config

### 5.1 Config Tab

**File:** `tauri-app/src/components/ConfigTab.tsx`

- Remove "Search filter:" label and input from Ghost Follower section
- Remove `ghostFollowerSearch` from apply/save payloads
- Remove `ghost_follower_search` from `applyConfig` and `ConfigUpdateDto` if no longer used

### 5.2 Backend / App State

**Files:** `tauri-app/src-tauri/src/api.rs`, `tauri-app/src/bindings.ts`, `tauri-app/src/types.ts`, `crates/digicore-text-expander/src/app_config.rs`

- Remove `ghost_follower_search` from config DTOs and app state if it was a persisted default
- Keep in-ribbon search: `ghost_follower_set_search` and `get_ghost_follower_state(search_filter)` for live filtering only (search is typed in the ribbon UI)

---

## 6. Phase 4: Transparency

### 6.1 Config Tab

**File:** `tauri-app/src/components/ConfigTab.tsx`

- Add "Transparency" slider (10–100%) in Ghost Follower section
- Label: "Transparency (10% = near transparent, 100% = opaque)"
- On change: call `applyConfig` with `ghost_follower_opacity` (or similar)
- Real-time updates: apply immediately when slider moves (or on Apply)

### 6.2 Backend

**Files:** `tauri-app/src-tauri/src/api.rs`, `tauri-app/src-tauri/src/lib.rs`, `crates/digicore-text-expander/src/app_config.rs`

- Add `ghost_follower_opacity: u8` (10–100) to config
- Add API `ghost_follower_set_opacity(value: number)` that applies opacity to Ghost Follower window
- On Windows: use Tauri's opacity API if available (`WebviewWindow.set_opacity` or equivalent)
- Apply on config change and when window is shown

### 6.3 Tauri Opacity

- Verify Tauri 2 `WebviewWindow` has `set_opacity` or equivalent
- Map 10–100% to 0.1–1.0 (or platform-specific range)

---

## 7. Phase 5: Drag-to-Reposition

### 7.1 Draggable Region

**Files:** `tauri-app/src/ghost-follower.ts`, `tauri-app/ghost-follower.html`

- Add draggable region: pill handle "•••" or entire pill when collapsed; title bar or handle when expanded
- Use Tauri `@tauri-apps/plugin-window-state` or `WebviewWindow.startDragging()` for drag
- Ensure drag works in both pill and ribbon states

### 7.2 Persist Position

**Files:** `tauri-app/src-tauri/src/api.rs`, `crates/digicore-text-expander/src/application/ghost_follower.rs`, storage

- Add API `ghost_follower_save_position(x: number, y: number)` called on drag end
- Persist position (JSON file or existing config storage)
- On startup/refresh: restore position from storage
- Respect Edge (Left/Right) and Monitor when no saved position exists

---

## 8. Phase 6: Edge + Monitor Positioning

### 8.1 Disable SMOKE_TEST_CENTER

**File:** `tauri-app/src/ghost-follower.ts`

- Set `SMOKE_TEST_CENTER = false` for production
- Use positioner plugin for initial placement based on Edge + Monitor

### 8.2 Positioner Integration

**Files:** `tauri-app/src/ghost-follower.ts`, `tauri-app/src-tauri/src/api.rs`

- On first show (no saved position): place using Edge (Left/Right) and Monitor (Primary/Secondary/Current)
- If position saved: use saved position
- Ensure positioner works for both pill and ribbon sizes

---

## 9. Suggested Implementation Order

| Step | Phase | Description |
|------|-------|-------------|
| 1 | 1.1 | Backend: touch, should_collapse, set_collapsed APIs |
| 2 | 1.2–1.4 | Frontend: pill/ribbon layout, hover expand, collapse delay |
| 3 | 1.5 | Window resize API and wiring |
| 4 | 2 | Close button + tray "Display Ghost Follower" |
| 5 | 3 | Remove search filter from Config |
| 6 | 4 | Transparency slider + backend |
| 7 | 5 | Drag-to-reposition + persist position |
| 8 | 6 | Edge + Monitor + disable SMOKE_TEST_CENTER |

---

## 10. Config Tab – Ghost Follower Section (Final)

After implementation, the Ghost Follower section in Configuration tab will have:

| Setting | Type | Description |
|---------|------|-------------|
| Enable Ghost Follower | Checkbox | Toggle feature on/off |
| Hover preview | Checkbox | Show full content on hover (when expanded) |
| Collapse delay (s) | Number (0–60) | Seconds expanded stays open before auto-collapsing |
| Edge | Select | Left / Right |
| Monitor | Select | Primary / Secondary / Current |
| Transparency | Slider (10–100%) | 10% = near transparent, 100% = opaque |
| Apply Ghost Follower | Button | Save and apply settings |

**Removed:** Search filter (moved to in-ribbon only).

---

## 11. Dependencies and Verification

- [ ] Verify `tauri-plugin-positioner` supports Edge + Monitor
- [ ] Verify Tauri 2 opacity API for `WebviewWindow`
- [ ] Verify `startDragging()` or equivalent for drag
- [ ] Ensure `tauri.conf.json` grants required permissions

---

## 12. File Reference

| Area | Files |
|------|-------|
| Backend (Rust) | `crates/digicore-text-expander/src/application/ghost_follower.rs`, `tauri-app/src-tauri/src/api.rs`, `tauri-app/src-tauri/src/lib.rs` |
| Frontend | `tauri-app/src/ghost-follower.ts`, `tauri-app/ghost-follower.html` |
| Config UI | `tauri-app/src/components/ConfigTab.tsx` |
| Types | `tauri-app/src/bindings.ts`, `tauri-app/src/types.ts` |

---

## 13. Revision History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2026-03-02 | Initial plan based on user decisions |
