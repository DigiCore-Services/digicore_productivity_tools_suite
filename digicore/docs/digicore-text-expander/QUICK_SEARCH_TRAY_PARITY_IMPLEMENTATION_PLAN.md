# Quick Search Tray Parity Plan (AHK -> Tauri)

## Purpose

Define what must change to achieve parity for tray-driven **Quick Search** between legacy AHK and the Tauri frontend/backend, including:

- separate/disconnected UX from the Management Console tabs
- reliable insertion into the previously focused external app (Cursor, Sublime, Outlook, etc.)
- clear implementation options with trade-offs (pros/cons + SWOT)
- decisions required from product/owner before coding

---

## Current Behavior Audit

## AHK legacy behavior (reference)

In `AHK_TE_Pro_ExpansionEngine...ahk`:

- tray menu has `Quick Search (Ctrl+Alt+S)`
- `QuickSearch(*)` captures active window handle:
  - `savedWin := WinActive("A")`
- double-click executes `CommitQuickPaste(...)`:
  - closes Quick Search GUI
  - re-activates saved external window
  - processes dynamic tags
  - sends text into that target app via `SendText(processed)`

Net effect: quick-search interaction is decoupled from management GUI and commits into the active app context.

## Tauri current behavior

- tray item `Quick Search (Shift+Alt+Space)` currently emits `show-command-palette`
- `App.tsx` listens and sets `commandPaletteVisible = true`
- `CommandPalette.tsx` is rendered as an overlay/modal inside the main webview
- selecting result behavior currently is:
  - Enter/click -> copy content to clipboard
  - Ctrl+E / double-click -> open snippet editor

No target window restore/commit path exists in this flow, so parity gap is expected.

---

## Problem Statement

The Tauri tray Quick Search currently behaves as **in-console snippet browser** instead of **external-app commit tool**.

This creates two user-visible failures:

1. UX coupling: quick search appears tied to Management Console context.
2. Action mismatch: result interaction does not insert snippet into external app target.

---

## Requirements for Parity

1. Quick Search must be operable without requiring the Management Console to be the active workspace.
2. Tray-triggered Quick Search must capture and preserve the external target window context.
3. Primary action from quick search result must commit snippet content into target app.
4. Secondary actions must support parity with `Text Expansion Library` row context menu patterns where applicable.
5. Right-click entry menu should include consistent iconized actions:
   - Copy Full Content to Clipboard
   - View Full Snippet Content
   - Pin / Unpin Snippet (Unpin requires confirmation)
   - Edit Snippet
   - Delete Snippet (requires confirmation)
6. Context actions should honor state rules (e.g., show `Pin` vs `Unpin`, disable inappropriate actions).
5. Behavior must be robust when target window no longer exists.

---

## Implementation Options

## Option A - Keep current Command Palette, add "insert to last app" mode

### Summary

Reuse current `CommandPalette` component in main window, but:

- capture target window before bringing main window forward
- add tray-mode context flag
- on Enter/double-click call backend insert function instead of copy/edit
- add right-click action menu on entries with Library-style action parity and icons

### Pros

- minimal code churn
- reuses existing search UI and fuzzy logic
- fastest implementation

### Cons

- still visually tied to Management Console
- may continue to feel modal/behind-tabs
- more edge cases around focus and click routing

### SWOT

- **Strengths:** quickest path, low risk of UI rewrite
- **Weaknesses:** imperfect UX parity with AHK standalone feel
- **Opportunities:** can become universal command mode switch
- **Threats:** persistent confusion between "console command palette" and "quick insert"

---

## Option B - New dedicated Quick Search window (recommended)

### Summary

Create a dedicated Tauri webview window (similar architecture to Ghost overlays) for tray quick search:

- tray action captures target window handle
- opens standalone quick-search window (always-on-top, no console dependency)
- Enter/double-click commits into captured target via backend command
- right-click context menu matches Library action expectations for consistency
- closes and returns focus

### Pros

- closest UX parity to legacy AHK
- clean separation from Management Console tabs
- easier to reason about click/focus lifecycle

### Cons

- more implementation work (new window + event wiring)
- new component/test surface
- additional cross-window communication complexity

### SWOT

- **Strengths:** best parity, clearer user mental model
- **Weaknesses:** higher implementation complexity than Option A
- **Opportunities:** future contextual quick tools can reuse this window pattern
- **Threats:** window management regressions if not tested thoroughly

---

## Option C - Repurpose Ghost Follower search surface

### Summary

Use ghost follower infrastructure as quick search entry point.

### Pros

- some insertion plumbing already exists (`ghost_follower_insert`)
- existing target capture command exists (`ghost_follower_capture_target_window`)

### Cons

- conceptual mismatch: ghost follower is persistent assistant, not tray quick launcher
- risk of feature entanglement and complexity
- may confuse users and future maintainers

### SWOT

- **Strengths:** leverages existing insertion route
- **Weaknesses:** high coupling to unrelated feature set
- **Opportunities:** none significant vs dedicated window
- **Threats:** regressions in ghost follower behavior and harder debugging

---

## Recommended Direction

**Option B (dedicated Quick Search window)** for true parity and cleaner UX boundaries.

Fallback if you prefer speed over parity: Option A as interim, then migrate to B.

---

## Proposed Technical Design (Option B)

## Backend / Tauri (`src-tauri`)

1. Add tray event handler action:
  - capture target window handle before opening quick-search window
2. Add quick-search window create/show helpers:
  - hidden from taskbar (optional)
  - always on top
  - compact dimensions and dark theme consistency
3. Add IPC command:
  - `quick_search_commit(content: String)`:
    - process dynamic tags if required
    - restore/focus captured target window
    - send insertion request (same family as ghost follower insertion path)
4. Add robust failure responses:
  - target missing
  - insertion failed
  - processing failed

## Frontend (new quick-search window UI)

1. New component/page dedicated to quick search:
  - search input
  - filtered snippet list
  - Enter/double-click = commit
- right-click menu (required for parity, iconized):
   - Copy Full Content to Clipboard
   - View Full Snippet Content
   - Pin / Unpin Snippet (confirm on unpin)
   - Edit Snippet
   - Delete Snippet (confirm)
2. No rendering inside Management Console tabs.
3. Escape closes window and restores user focus path safely.

## Data/Search

- Reuse existing snippet flattening and fuzzy search logic.
- Keep consistent ranking behavior with CommandPalette unless intentionally changed.

---

## Error Handling + Diagnostics

Emit diagnostic entries for Log tab visibility:

- quick search opened from tray
- target window captured/restored
- commit success/failure
- copy fallback usage
- right-click action invoked (view/copy/pin/unpin/edit/delete)
- confirm prompts accepted/cancelled for unpin/delete

Use both:

- `expansion_diagnostics::push(...)` for end-user Log tab
- `log::info!/warn!/error!` for deeper troubleshooting

---

## Test Plan

## Unit / Component

- quick-search list filtering
- Enter/double-click commit action dispatch
- right-click menu renders all required actions with icons
- right-click menu state logic (Pin vs Unpin, enabled/disabled cases)
- unpin/delete confirmation handling and cancellation behavior
- warning/error message rendering

## Integration

- tray quick-search event opens dedicated window
- captured target handle consumed on commit
- fallback path when target handle invalid
- right-click actions route correctly to existing snippet operations APIs

## Regression

- existing CommandPalette behavior remains unchanged for console workflow
- ghost follower insertion path unaffected

---

## Key Decisions Needed From You

1. **Primary path choice:** Option B (recommended) or Option A interim?
2. **Shortcut parity:** keep `Shift+Alt+Space`, switch to `Ctrl+Alt+S`, or support both?
3. **Result interaction policy:**
  - Enter = commit, double-click = commit (AHK parity)
  - keep Ctrl+E for edit only in console palette (recommended)
4. **Dynamic tag processing in quick-search commit:**
  - identical to AHK `ProcessDynamicTags` intent (recommended)
  - or raw snippet insertion only
5. **Failure fallback:**
  - if target unavailable, copy to clipboard + notify?
  - or hard error only
6. **Right-click parity scope:** full parity with Library menu now, or phased rollout (copy/view first, then edit/pin/delete)?

---

## Suggested Iterative Sequence

1. Introduce dedicated quick-search window shell + tray route.
2. Add target capture/commit backend path.
3. Hook Enter/double-click commit behavior.
4. Add diagnostics and failure fallback.
5. Add integration/regression tests and tune UX.

