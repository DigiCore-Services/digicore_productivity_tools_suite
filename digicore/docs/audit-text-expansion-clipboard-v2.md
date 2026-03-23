# Audit, Review, and Analysis: DigiCore Text Expansion & Clipboard Features (v2)

## 1. Executive Summary
This audit provides a deep dive into the "DigiCore Text Expansion" and "copy-to-clipboard" implementations. While the hexagonal architecture provides a solid foundation, enhancements are needed in **expansion robustness**, **trigger sophistication**, and **diagnostic transparency**. This document outlines a roadmap to transition from a functional system to a "best-in-class," reliable, and feature-rich productivity engine.

---

## 2. Current Implementation Audit & SWOT Analysis

### 2.1 Text Expansion (`digicore-text-expander`)
*   **Strengths**: Clean decoupling via `InputPort` and `ClipboardPort`. Support for JS (Boa), rich templates, and case-adaptive matching.
*   **Weaknesses**: Fixed 20ms delays (jittery in some apps). In-memory buffer size is fixed (256 chars). Limited trigger-matching types (basic suffix and regex).
*   **Opportunities**: Adaptive delay logic. Regex capture group expansion (e.g., `;;say:(.*)` -> `You said: $1`). Post-expansion verification.
*   **Threats**: Keyboard hook conflicts. OS-level "Secure Desktop" blocking hooks.

### 2.2 Clipboard History (`clipboard_history.rs` / `sqlite_clipboard.rs`)
*   **Strengths**: Persistent SQLite storage. WinRT-native listening on Windows. Support for HTML, RTF, and Images with disk-backed thumbnails.
*   **Weaknesses**: Basic fuzzy deduplication (Levenshtein only). Lack of "Clean Paste" (strip formatting). Migration system is manual `ALTER TABLE` commands.
*   **Opportunities**: File-list tracking. AI-powered metadata extraction (e.g., extracting URLs from HTML blobs). Encrypted history-at-rest.
*   **Threats**: High-frequency clipboard updates flooding the DB. Privacy risks with PII.

### 2.3 SWOT (Summary)

| **Strengths** (Internal) | **Weaknesses** (Internal) |
| :--- | :--- |
| - Hexagonal / SOLID separation. | - Fixed expansion delays. |
| - Persistent SQLite storage. | - Simple fuzzy matching. |
| - Rich text / Image support. | - Diagnostic opacity (hard to trace matching failures). |

| **Opportunities** (External) | **Threats** (External) |
| :--- | :--- |
| - **Adaptive Delays**: Intelligent expansion timing. | - **Privacy**: PII captured in history. |
| - **Regex backreferences**: Dynamic snippets. | - **OS Hooks**: Hook latency / blocking. |
| - **File Tracking**: Tracking files across Explorer. | - **Performance**: DB growth over time. |

---

## 3. Alternative Implementation Options

### 3.1 Expansion Logic: Managed Hook vs. UI Automation
*   **Option A: Managed Low-Level Hook (Current)**
    *   *Pros*: High performance, captures input system-wide.
    *   *Cons*: Limited context (doesn't know what was in the textbox before typing).
*   **Option B: UI Automation (UIA) Fallback**
    *   *Pros*: Precise control over selected text. Can verify content before expansion.
    *   *Cons*: Slower (50-200ms overhead). Requires Accessibility permissions.
*   **Decision**: Maintain Managed Hook but implement "Verification after Paste" using `WindowContextPort`.

### 3.2 History Persistence: SQLite vs. Encrypted SQLite
*   **Option A: Standard SQLite (Current)**
    *   *Pros*: High speed, zero overhead.
    *   *Cons*: Clear-text history vulnerable to local disk access.
*   **Option B: SQLCipher (Encrypted)**
    *   *Pros*: Secure, PII protection.
    *   *Cons*: Slight performance hit, key management complexity.
*   **Decision**: Recommend standard SQLite with an **Optional Masking Service** (Regex based) before insertion.

---

## 4. Key Decisions & User Input Required

1.  **PII Masking**: Should DigiCore automatically detect and "star out" Credit Cards/SSNs in the clipboard history before saving to SQLite?
2.  **Delay Profile**: Do we prioritize **speed** (short fixed delays) or **reliability** (longer adaptive delays based on app responsiveness)?
3.  **Regex Complexity**: For regex triggers, should we allow multi-line patterns? (Increases CPU usage on every keystroke).

---

## 5. Detailed Implementation Plan

### Phase 1: Robustness & Reliability (Next 2 Sprints)
1.  **[MODIFY] `expansion_engine.rs`**: Implement **Adaptive Delays**. Instead of `sleep(20ms)`, use a configurable delay based on the `WindowContext`.
2.  **[MODIFY] `hotstring.rs`**: Increase buffer efficiency. Implement "Trimming" based on word boundaries rather than just a fixed length.
3.  **[ENHANCE] `do_expand()`**: Add post-expansion title verification. If the window title changed during the ~50ms of expansion, log an error and abort the paste if possible.

### Phase 2: Feature Richness & Intelligence
1.  **[NEW] `TriggerMatcher` Regex Groups**: Add support for `$1`, `$2` backreferences in snippet content.
2.  **[NEW] `ClipboardTransformerService`**: Implement "Strip Formatting" (Ctrl+Shift+V) as a core service available to the UI and Hotkeys.
3.  **[ENHANCE] `ClipEntry`**: Add `file_paths: Vec<String>` support for "Copy File" events in Windows Explorer.

### Phase 3: Diagnostic Logging & Error Handling
1.  **[MODIFY] `ExpansionLogger`**: Implement structured JSON-only logging to `digicore_expansion.json`.
2.  **[ENHANCE] `Gracious Error Handling`**: If `send_ctrl_v` fails, fall back to "Keystroke Simulation" but log a "Performance Warning" to the user-facing diagnostics.

---

## 6. Diagnostic Logging Strategy
Every expansion and clipboard event will generate a JSON record:
```json
{
  "event": "expansion_success",
  "trigger": ";;sig",
  "category": "personal",
  "duration_ms": 42,
  "method": "clipboard_swap",
  "window": "notepad.exe",
  "timestamp": "2026-03-23T02:52:04Z"
}
```
This allows for automated analysis of expansion reliability across different applications.
