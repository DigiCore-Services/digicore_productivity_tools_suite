# Audit & Implementation Plan: Text Expansion & Copy-to-Clipboard

## Executive Summary
This document provides a detailed audit, review, and analysis of the current "DigiCore Text Expansion" and "copy-to-clipboard" implementations. It identifies areas for improvement and outlines a robust, feature-rich implementation plan for new and enhanced capabilities, adhering to Hexagonal architecture, SOLID, and SRP principles.

---

## 1. Audit & Analysis of Existing Implementation

### 1.1 Text Expansion Engine (`expansion_engine.rs`, `hotstring.rs`)
**Current Capabilities:**
- Exact suffix matching for hotstrings.
- App-lock (process-based filtering).
- Simple plain-text injection via `Ctrl+V` (clipboard swap) or `type_text`.
- Global pause/resume functionality.
- Expansion diagnostics for visibility.

**Strengths:**
- **Hexagonal Alignment**: Logic is decoupled from the UI and platform-specific keyboard hooks via ports (`InputPort`, `ClipboardPort`).
- **Robustness**: Fallback from clipboard-pasting to typing ensures reliability in volatile environments.
- **Diagnostics**: Real-time logging of expansion events helps with troubleshooting.

**Weaknesses:**
- **Plain Text Only**: No support for RTF, HTML, or multi-modal expansions.
- **Rigid Triggers**: Only exact suffix matching; lacks regex or start-of-word constraints.
- **Limited Control Flow**: No conditional logic or loops within templates (outside of complex JS).

### 1.2 Copy-to-Clipboard & History (`clipboard_history.rs`)
**Current Capabilities:**
- Real-time monitoring with Windows-native listeners or polling fallback.
- Metadata capture (application name, window title).
- Configurable history depth and duplicate prevention.
- "Promote to Snippet" integration.

**Strengths:**
- **Reliability**: Use of native Windows events minimizes CPU usage and latency.
- **Context Aware**: Capturing window titles adds significant value for history searching.

**Weaknesses:**
- **MIME Support**: Focused almost exclusively on plain text; limited handling for images or files.
- **Basic Dedup**: Strict content comparison; no fuzzy or similarity-based deduplication.

---

## 2. SWOT Analysis

| **Strengths** | **Weaknesses** |
| :--- | :--- |
| - Clean Hexagonal architecture.<br>- SOLID/SRP compliant modules.<br>- Mature template processor with JS/HTTP support. | - Lack of Rich Text Support.<br>- Limited trigger flexibility (no Regex).<br>- Sparse binary/multi-modal content handling. |
| **Opportunities** | **Threats** |
| - **Rich Text/HTML**: Support formatted emails/docs.<br>- **Regex Triggers**: Advanced user productivity.<br>- **Smart Dedup**: Improve history relevance.<br>- **Persistent Variables**: Global state across snippets. | - **Race Conditions**: Clipboard swapping is inherently racy.<br>- **App Compatibility**: Some apps block simulated input or fast pasting. |

---

## 3. Proposed Enhancements & New Features

### 3.1 Feature: Rich Text Expansion (RTF/HTML)
**Requirement**: Allow snippets to contain and expand with formatting (bold, links, tables).
- **Implementation**: Update `Snippet` domain model to support `content_type` (Plain, RTF, HTML).
- **Adapter Update**: Leverage `arboard` or native Win32 API to set multiple MIME types during the expansion paste.

### 3.2 Feature: Regex Triggers
**Requirement**: Support dynamic triggers (e.g., `;;d\d{2}` for double digits).
- **Implementation**: Introduce `TriggerType` (Exact, Regex). Use `regex` crate for matching against the hotstring buffer.
- **Decision**: Should Regex triggers support capture groups? (e.g. `;;say:(.*)` expands to `You said: $1`).

### 3.4 Feature: Smart Clipboard Deduplication
**Requirement**: Minimize noise in history by ignoring minor variations or whitespace-only changes.
- **Implementation**: Integrate a similarity score (e.g. Levenshtein) in the `add_entry` logic.

---

## 4. Implementation Strategy (Hexagonal/SOLID)

### Phase 1: Foundation & Rich Text
1.  **Domain**: Update `Snippet` and `ClipEntry` to include `MimeType` and optional `RawData`.
2.  **Infrastructure**: Update `StoragePort` to persist rich content (Base64 or external files).
3.  **Adapter**: Enhance `ClipboardPort` to handle multi-format payloads.

### Phase 2: Advanced Triggers
1.  **Engine**: Refactor `hotstring.rs` buffer matching to support regex.
2.  **UI**: Update Snippet Editor to allow selecting "Trigger Type".

### Phase 3: Robustness & Diagnostics
1.  **Logging**: Implement "Failed Trigger" diagnostics (e.g. "Trigger 'sig' matched but AppLock blocked it").
2.  **Safety**: Implement a "Safety Buffer" for clipboard restoration to prevent data loss in slow apps.

---

## 5. Key Decisions Required
1.  **Rich Text Editor**: Do we want a full WYSIWYG editor in the UI, or just raw HTML/RTF support? (WYSIWYG is high effort, raw is low).
2.  **Clipboard Sync**: Should clipboard history be synced across devices? (Requires encryption and storage considerations).
3.  **Regex Complexity**: Should we limit regex complexity to prevent performance impacts on the keyboard hook?

---

## 6. Verification Plan

### Automated Tests
- **Unit Tests**: Test `TemplateProcessor` with nested and persistent variables.
- **Integration Tests**: Verify `HotstringDriver` suffix vs regex matching priorities.

### Manual Verification
- Verify Rich Text expansion in Outlook, Word, and Gmail.
- Test "Smart Dedup" by copying similar blocks of code.
- Validate "Failed Trigger" logs by attempting to expand locked snippets.
