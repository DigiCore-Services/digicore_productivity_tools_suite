# Simulate Script Output (Test Script Logic) - Implementation Plan

**Version:** 1.0  
**Created:** 2026-02-28  
**Status:** Implemented (Option A)  
**Related:** Snippet Editor enhancement, parity with legacy AHK "Test Script Logic"

**User decisions (2026-02-28):**
- Button label: "Preview Expansion"
- {key:} and {wait:}: Show as [KEY:...] / [WAIT:...ms] (AHK-style)
- {run:} in test: Execute (with allowlist)

---

## 1. Overview

### 1.1 Purpose

Add a **"Simulate Script Output"** (or "Test Script Logic") button to the Snippet Editor modal. When selected, it processes the current snippet content through the template engine and displays the resulting output. If the content contains interactive placeholders (`{var:}`, `{choice:}`, `{checkbox:}`, etc.), a pop-up modal is shown first to collect user-provided values before displaying the final output.

### 1.2 Legacy AHK Reference

**File:** `C:\Users\pinea\Scripts\AHK_AutoHotKey\AHK_-_PROD-MAIN_STARTUP-SCRIPTZ\ACTIVE-Prod-LIVE-Apps\Text-Expansion\AHK_TE_Pro_GUI.ahk`

**Button:** "Test Script Logic" (line 125, Snippet Editor tab)

**Legacy Flow:**
1. Reads content from `contentInput`
2. Processes static tags: `{date}`, `{am/pm}`, `{timezone}`, `{tz}`, `{time:fmt}`
3. Processes `{clip:1}`..`{clip:5}` from clipStack
4. Processes `{js:...}`, `{http:...}`, `{run:...}` via ScriptModule
5. Collects interactive vars: `{var:}`, `{choice:}`, `{checkbox:}`, `{date_picker:}`, `{file_picker:}`
6. If interactive vars exist: shows `VariableInputModal(vars)` to collect values, then substitutes
7. Replaces `{key:...}` and `{wait:...}` with `[KEY:...]` and `[WAIT:...ms]` for display (simulation only)
8. Shows result in `MsgBox("Simulated Expansion Result: ...")`

---

## 2. Existing DigiCore Implementation - Leverage Analysis

### 2.1 What Already Exists

| Component | Location | Reusability |
|-----------|----------|-------------|
| **Template processing** | `application/template_processor.rs` | **Full reuse** - `process_with_user_vars(content, clipboard, clip_history, user_vars)` handles all placeholders |
| **Interactive var collection** | `template_processor::collect_interactive_vars(content)` | **Full reuse** - returns `Vec<InteractiveVar>` for {var:}, {choice:}, {checkbox:}, {date_picker:}, {file_picker:} |
| **Variable input UI** | `application/variable_input.rs` | **Partial reuse** - `render_viewport_modal` is a viewport (always-on-top); UI logic could be extracted or we create an in-window variant |
| **Clipboard history** | `application/clipboard_history::get_entries()` | **Full reuse** - for {clip:N} resolution |
| **Clipboard current** | `arboard::Clipboard::get_text()` | **Full reuse** - for {clipboard} |
| **Template config** | `template_processor::get_config()` | **Full reuse** - date/time formats, etc. |

### 2.2 Processing Capabilities (DigiCore vs AHK)

| Placeholder | AHK | DigiCore |
|-------------|-----|----------|
| {date}, {time}, {time:fmt} | Yes | Yes |
| {clipboard}, {clip:N} | Yes | Yes |
| {env:VAR} | Yes | Yes |
| {uuid}, {random:N} | Yes | Yes |
| {js:...} | Yes | Yes (Boa engine) |
| {http:...} | Yes | Yes |
| {run:...} | Yes | Yes (allowlist) |
| {var:}, {choice:}, {checkbox:}, etc. | Yes | Yes |
| {key:...}, {wait:...} | Simulated as [KEY:...], [WAIT:...] | **Not implemented** - would need to add placeholder substitution for test mode |

### 2.3 Gap Summary

- **Core processing:** Fully available via `process_with_user_vars`
- **Variable input UI:** Exists as viewport; need in-window modal or adapt flow
- **{key:}, {wait:} simulation:** Legacy shows placeholders; DigiCore does not resolve these. Low priority (optional enhancement)
- **Result display:** Need new modal (similar to View Full Content)

---

## 3. Implementation Options

### Option A: In-Window Variable Input + Result Modal

**Approach:** Add `snippet_test_pending: Option<SnippetTestState>` to app. When "Simulate Script Output" clicked:
1. If no interactive vars: process immediately, show result modal
2. If interactive vars: show in-window modal "Enter Variable Values" (reuse UI structure from variable_input, but as egui::Window). On OK, process and show result modal.

**Pros:**
- Keeps test flow within main window (no viewport)
- Clear two-step UX when vars present
- Reuses template_processor fully

**Cons:**
- Requires building variable input UI in modals (or extracting shared component)
- Some code duplication with variable_input

---

### Option B: Reuse Variable Input Viewport in "Test Mode"

**Approach:** Extend `variable_input` with a "test mode" flag. When Test clicked, set `set_viewport_modal` with content and vars, but `response_tx = None` and a `test_mode: true` flag. When user submits, instead of sending to expansion, store result in `TEST_RESULT: Mutex<Option<String>>`. Main loop polls and shows result modal when set.

**Pros:**
- Reuses existing variable input UI 100%
- No duplication

**Cons:**
- Viewport is always-on-top (may feel disconnected from Snippet Editor)
- More complex state machine (pending test -> viewport -> result)
- Couples variable_input to test flow

---

### Option C: Single "Simulate" Modal with Tabs/Steps

**Approach:** One modal with two "screens": (1) Variable input form when vars exist, (2) Result display. User clicks "Run" on screen 1, sees screen 2. Or use collapsible sections.

**Pros:**
- Single modal, clear flow
- All test UX in one place

**Cons:**
- More complex modal state
- Larger modal when both vars and result shown

---

### Option D: Minimal - Process Only, No Interactive Var Support in First Release

**Approach:** "Simulate Script Output" only processes content that has no interactive vars. If interactive vars detected, show message: "Content has {var:} or {choice:} placeholders. Enter test values in a future release." Process and show result for non-interactive content only.

**Pros:**
- Fastest to implement
- Covers majority of snippets (date, time, js, http, etc.)

**Cons:**
- Incomplete parity with legacy
- Defers interactive var support

---

## 4. SWOT Analysis

| | **Strengths** | **Weaknesses** |
|---|---------------|----------------|
| **Internal** | Template processor is mature and tested; variable input exists; clipboard history available | Variable input is viewport-based; no {key}/{wait} simulation |
| **External** | Legacy AHK provides clear reference; user expects feature | {run:} may have security implications in test mode |

| | **Opportunities** | **Threats** |
|---|-------------------|--------------|
| **External** | Improves snippet authoring UX; reduces trial-and-error; could add "dry run" for {run:} (show command only) | Over-reliance on test mode for {run:} could mask allowlist issues |

---

## 5. Pros/Cons Summary by Option

| Option | Pros | Cons |
|--------|------|------|
| **A** | Clean separation, in-window UX | UI duplication or extraction effort |
| **B** | Zero duplication | Viewport disconnect, complex state |
| **C** | Single modal flow | Heavier modal, more state |
| **D** | Fastest delivery | No interactive var support |

---

## 6. Key Decisions Requiring Input

### 6.1 Button Label

- **"Simulate Script Output"** (user request)
- **"Test Script Logic"** (legacy parity)
- **"Preview Expansion"** (alternative)

**Recommendation:** "Simulate Script Output" with optional subtitle "Test Script Logic" in tooltip.

---

### 6.2 {key:} and {wait:} Handling

Legacy replaces these with `[KEY:...]` and `[WAIT:...ms]` in test mode (no actual key simulation).

**Options:**
- **(a)** Add same placeholder substitution in template_processor for "test mode"
- **(b)** Leave as-is (tags appear verbatim in output)
- **(c)** Add in Phase 2

**Recommendation:** (a) for full parity; low effort.

---

### 6.3 {run:} in Test Mode

**Options:**
- **(a)** Execute {run:} in test (respect allowlist) - matches legacy
- **(b)** Show `[RUN: command]` placeholder only (safer, no execution)
- **(c)** Execute only if allowlisted, otherwise show placeholder

**Recommendation:** (a) - same behavior as expansion; user expects real output.

---

### 6.4 Preferred Implementation Option

**Recommendation:** **Option A** - In-window variable input + result modal. Best UX, clear flow, and we can extract a shared `render_variable_input_form(ui, vars, values, ...)` used by both variable_input viewport and the test modal to avoid duplication.

---

## 7. Recommended Implementation Steps

1. **Phase 1 - Core**
   - Add "Simulate Script Output" button to Snippet Editor modal (bottom, next to Save/Cancel)
   - Add `snippet_test_result: Option<String>` and `snippet_test_var_pending: Option<SnippetTestVarState>` to app state
   - When clicked with no interactive vars: process via `process_with_user_vars`, show result in new "Simulated Output" modal
   - When clicked with interactive vars: show "Enter Variable Values" modal (inline in main window), on OK process and show result

2. **Phase 2 - Variable Input**
   - Extract or create `render_variable_input_form` for {var:}, {choice:}, {checkbox:}, {date_picker:}, {file_picker:}
   - Use in test flow; optionally refactor variable_input viewport to use same form

3. **Phase 3 - Optional**
   - Add {key:}, {wait:} placeholder substitution for test mode
   - Add tooltip to button

---

## 8. File Changes (Estimated)

| File | Changes |
|------|---------|
| `main.rs` | Add `snippet_test_result`, `snippet_test_var_pending`; wire button |
| `ui/modals.rs` | Add "Simulate Script Output" button; add `snippet_test_var_modal`, `snippet_test_result_modal` |
| `application/variable_input.rs` | Optional: extract `render_variable_input_form` for reuse |
| `application/template_processor.rs` | Optional: add test-mode substitution for {key:}, {wait:} |

---

## 9. Acceptance Criteria

- [x] "Preview Expansion" button visible in Snippet Editor
- [x] Content without interactive vars: processes and shows result in modal
- [x] Content with {var:}, {choice:}, etc.: shows variable input modal first, then result
- [x] Result modal displays final processed output (scrollable)
- [x] Uses current clipboard and clipboard history for {clipboard}, {clip:N}
- [x] {key:} and {wait:} shown as [KEY:...] / [WAIT:...ms] in preview
- [x] {run:} executes with allowlist (same as expansion)

---

## 10. References

- Legacy AHK: `AHK_TE_Pro_GUI.ahk` lines 125-126, 2152-2311
- DigiCore template_processor: `application/template_processor.rs`
- DigiCore variable_input: `application/variable_input.rs`
- DigiCore Snippet Editor: `ui/modals.rs` snippet_editor_modal
