# Audit & Review: digicore Text Expansion & Clipboard Features

## 1. Executive Summary
This document provides a comprehensive audit of the current "digicore Text Expansion" and "copy-to-clipboard" implementations. While the existing system is architecturally sound (Hexagonal, Configuration-first, SOLID), several high-value opportunities exist to increase robustness, feature-richness, and reliability. Key findings include volatile clipboard history, basic trigger matching, and limited rich-media support.

---

## 2. Current Implementation Audit

### 2.1 Text Expansion (`digicore-text-expander`)
*   **Architecture:** Implements Hexagonal architecture with clear Ports (`InputPort`, `ClipboardPort`) and Adapters (`EnigoInputAdapter`, `WindowsRichClipboardAdapter`).
*   **Capabilities:** 
    *   Trigger-based expansion (Suffix and Regex).
    *   Rich Template Processor supporting JS (Boa engine), variables, dates, and shell commands.
    *   App-Locking (process-specific snippets).
    *   Interactive variables (`{var:}`, `{choice:}`).
*   **Identified Gaps:**
    *   **Case Handling:** Expansion is currently case-insensitive for matching but does not adapt the expansion result to the trigger's case (e.g., `trig` -> `Result`, `Trig` -> `Result` instead of `Result`).
    *   **Trigger Precision:** Basic suffix matching can lead to accidental expansions in the middle of words (unless using Regex).
    *   **Async Safety:** Expansion is primarily synchronous; long-running JS templates may block the keyboard hook briefly.

### 2.2 Clipboard Features (`clipboard_history.rs`)
*   **Architecture:** Uses a robust WinRT `WM_CLIPBOARDUPDATE` listener on Windows with a poll-loop fallback.
*   **Capabilities:**
    *   Rich text capture (HTML/RTF).
    *   Fuzzy deduplication (92% Levenshtein threshold).
    *   Metadata tracking (Process Name, Window Title).
    *   "Promote to Snippet" workflow.
*   **Identified Gaps:**
    *   **Volatility:** History is stored in-memory only; all history is lost on application restart.
    *   **Text-Centric:** Explicitly ignores or minimally handles non-text content (Images, Files).
    *   **Management:** No built-in ability to "clean" the clipboard (e.g., strip formatting, trim whitespace) except via complex JS templates.

---

## 3. SWOT Analysis

| **Strengths** (Internal) | **Weaknesses** (Internal) |
| :--- | :--- |
| - Clean Hexagonal separation of concerns. | - Volatile clipboard history (In-memory). |
| - High-performance WinRT integration (Windows). | - Limited trigger case-adaptation logic. |
| - Extensible Template Processor (JS-powered). | - No image/file support in history. |
| - SOLID/SRP compliance in core crates. | - Synchronous expansion execution. |

| **Opportunities** (External/Growth) | **Threats** (External/Risks) |
| :--- | :--- |
| - **SQLite Persistence:** Enable long-term history. | - **OS Updates:** Changes to WinRT or Hook APIs. |
| - **Global Transformers:** AI/JS-based cleaning. | - **Security:** Sensitive data in history (needs encryption). |
| - **Multi-device Sync:** Link via `SyncPort`. | - **Conflict:** Interference with other hotkey apps. |
| - **Image OCR Integration:** Searchable image history. | - **Privacy:** Capturing passwords in history. |

---

## 4. Alternative Options & Key Decisions

### 4.1 Persistence Layer: SQLite vs. JSON
*   **Option A: SQLite (Recommended).**
    *   *Pros:* High performance for large history, full-text search (FTS), ACID compliance.
    *   *Cons:* Slight increase in binary size/complexity.
*   **Option B: Flat JSON File.**
    *   *Pros:* Simple, human-readable.
    *   *Cons:* Poor performance as history grows, risk of corruption on crash.
*   **Decision Required:** I recommend SQLite via `SqliteStorageAdapter` to support 1000+ history items and full-text search.

### 4.2 Trigger Matching: Trie vs. Linear
*   **Option A: Linear Iterator (Existing).**
    *   *Pros:* Simple to implement.
    *   *Cons:* O(N) complexity; slows down as library grows to 1000s of snippets.
*   **Option B: Aho-Corasick / Trie.**
    *   *Pros:* O(1) matching regardless of library size.
    *   *Cons:* More complex implementation, harder to integrate with Regex.
*   **Decision Required:** Should we prioritize O(1) scaling for extreme users?

---

## 5. Proposed Implementation Plan

### Phase 1: Robustness & Persistence (High Priority)
1.  **[MODIFY]** `ClipboardHistory`: Refactor to use a `ClipboardRepository` trait (Port).
2.  **[NEW]** `SqliteClipboardAdapter`: Implement persistence with automatic pruning (e.g., keep last 1000 items/30 days).
3.  **[ENHANCE]** `ExpansionEngine`: Implement "Case-Adaptive" expansion (Match case of trigger).

### Phase 2: Rich Media & Advanced Matching
1.  **[ENHANCE]** `ClipEntry`: Add support for image thumbnails (Base64/Path) and file lists.
2.  **[ENHANCE]** `TriggerMatcher`: Add "Smart Suffix" (expand only after space/punctuation) and "Regex Groups" UI.
3.  **[NEW]** `ClipTransformerService`: Global "Paste as Plain Text" or "JS Sanitize" hotkeys.

### Phase 3: Diagnostics & Reliability
1.  **[ENHANCE]** Logging: Implement structured JSON logging for expansion events to `digicore_debug.log`.
2.  **[ENHANCE]** Error Handling: Add "Gracious Fallback" for JS template failures (returning a descriptive error string instead of empty).

---

## 6. Key Decisions for USER Input
1.  **Encryption:** Should clipboard history be encrypted at rest? (Default: No, for performance).
2.  **Privacy:** Should we implement an "Automatic Exclude List" for apps like 1Password/KeePass?
3.  **Scaling:** Is a library size of >5,000 snippets expected? (Influences Trie vs. Linear decision).
4.  **Target Folder:** Confirm all new logs should reside in `digicore\logs` rather than `%TEMP%`.
