# Audit, Review, and Analysis: DigiCore Text Expansion & Clipboard Features

## 1. Executive Summary
This report provides a comprehensive audit of the "DigiCore Text Expansion" and "copy-to-clipboard" features. While the current implementation follows a solid Hexagonal architecture and SRP principles, there are significant opportunities for enhancement in multi-format support, trigger sophistication, and history intelligence.

---

## 2. Technical Audit Findings

### 2.1 Hexagonal & SOLID Compliance
- **Compliance**: High. Logic is decoupled from platform-specific APIs via `ClipboardPort`, `InputPort`, and `WindowContextPort`.
- **SRP**: Modules are well-scoped (`hotstring.rs` handles hook logic, `expansion_engine.rs` handles orchestration, `clipboard_history.rs` handles persistence/state).
- **Configuration-First**: Most settings are centralized in `ClipboardHistoryConfig` and `GhostConfig`, but expansion-specific timing (e.g., backspace delays) is currently hardcoded.

### 2.2 Feature Analysis
| Feature | Current State | Audit Rating | Recommendation |
| :--- | :--- | :--- | :--- |
| **Expansion** | Plain-text only. Uses `{clipboard}` and `{js:...}`. | B | Support RTF/HTML to preserve formatting (Email/Office). |
| **Triggers** | Simple suffix matching (Fixed string). | C | Implement Regex triggers for dynamic expansion. |
| **Clipboard History** | Basic dedup (compare last only). Metadata capture. | B- | Implement similarity-based deduplication (fuzzy). |
| **Robustness** | Hardcoded 20ms delay. Basic fallback to typing. | B | Dynamic delay or "Verification-after-paste" check. |
| **Diagnostics** | Real-time log file and UI-pushed diag messages. | B+ | Add "Expansion Trace" to explain *why* a trigger didn't match. |

---

## 3. Alternative Implementation Options

### Option A: Enhanced Rust Backend (Recommended)
- **Description**: Continue with the current Rust-based hook and engine, but extend the domain model for MIME types and Regex triggers.
- **Pros**: Maximum performance, low latency, full control over memory/threads.
- **Cons**: Requires more complex development for UI/Rust interaction (Tauri).
- **SWOT**:
    - **S**: Clean architecture, high-performance.
    - **W**: Higher complexity for Rich Text rendering in Rust.
    - **O**: Leverage `arboard` or native Win32 for advanced clipboard.
    - **T**: Maintaining keyboard hooks across OS updates.

### Option B: AHK Hybrid Support
- **Description**: Delegate the low-level expansion to an AHK script while the Rust app manages the library and history.
- **Pros**: AHK has 20+ years of "war-tested" reliability for expansion timing.
- **Cons**: Adds a dependency on AHK, breaks Hexagonal purity, harder to package.
- **SWOT**:
    - **S**: Easy implementation of expansion tricks.
    - **W**: Integration overhead, Windows-only.
    - **O**: Faster time-to-market for complex expansion features.
    - **T**: AHK blocked by some security software.

---

## 4. Key Decisions Required
1.  **Rich Text Format**: Should we prioritize HTML or RTF? (HTML is easier for web/Outlook, RTF for legacy Windows apps).
2.  **Regex Engine**: Do we need capture group expansion (e.g., `;;say:(.*)` -> `You said: $1`)?
3.  **Deduplication Strength**: Strict vs. Fuzzy. (Fuzzy requires more CPU but yields cleaner history).

---

## 5. Detailed Implementation Plan (Phase 1 & 2)

### Phase 1: Core Domain & Rich Text Expansion
- **[MODIFY] `snippet.rs`**: Add `ContentType` enum (Plain, HTML, Image) and `data: Vec<u8>` for binary content.
- **[MODIFY] `clipboard.rs` Port**: Update to `set_payload(items: Vec<ClipboardItem>)` where `ClipboardItem` has MIME type and data.
- **[NEW] `WindowsRichClipboardAdapter.rs`**: Native Win32 adapter to handle `CF_HTML` and `CF_RTF` for formatting-preserving pastes.

### Phase 2: Advanced Trigger Engine
- **[MODIFY] `hotstring.rs`**: Integrate `regex` crate.
- **[NEW] `TriggerMatcher` Service**: Decouple matching logic from the hook driver. Implement "Fast-path" (Fixed strings) and "Dynamic-path" (Regex).
- **[MODIFY] `expansion_diagnostics.rs`**: Add "Trace Match" capability to log every trigger candidate evaluated during a keystroke.

### Phase 3: Robust Expansion & Error Handling
- **Mechanism**: Monitor the active window title before and after paste. If title changes unexpectedly, abort expansion.
- **Logging**: Implement JSON-structured logging for expansion events to allow for easier automated analysis.

---

## 6. Verification Plan

### Automated Verification
- **Unit Tests**:
    - `find_snippet_regex_test`: Verify various regex triggers work (anchors, groups).
    - `clipboard_dedup_similarity_test`: Verify similarity scores correctly merge entries.
- **Integration Tests**:
    - `multi_format_clipboard_test`: Set HTML/Plain-text simultaneously and verify priority.

### Manual Verification
1.  **Outlook/Word Test**: Expand a formatted HTML snippet and verify bold/links are preserved.
2.  **Visual Studio Code Test**: Verify Regex expansion does not interfere with IDE intellisense.
3.  **Stress Test**: Rapid-fire expansions to check for race conditions in clipboard restoration.
