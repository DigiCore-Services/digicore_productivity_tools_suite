# Audit & Vision: KMS Knowledge Hub & Rendering Engine (v1.0)

## 1. Executive Summary
This document provides a comprehensive audit of the current **Knowledge Management System (KMS)** and its **Rendering Engine** within the DigiCore ecosystem. While the foundation is solid—leveraging a file-first approach with hybrid AI search—there are significant opportunities to enhance robustness, reliability, and the richness of the user experience.

---

## 2. Current State Architectural Audit

### 2.1 Hexagonal & SOLID Principles
*   **Adapters**: Tauri IPC (`api.rs`) drives the domain (`kms_service.rs`), which uses `kms_repository.rs` for persistence. This follows hexagonal principles.
*   **Fat Adapter Risk**: Some domain logic (path normalization, backlink regex) still leaks into `api.rs`.
*   **Configuration-First**: Vault paths are externalized, but rendering preferences (font sizes, theme-sync, auto-save intervals) are currently hardcoded.

### 2.2 Rendering Stack
*   **WYSIWYG**: Tiptap (React) with `StarterKit`.
*   **Source Mode**: CodeMirror 6.
*   **Extensions**: 
    *   `MermaidExtension`: Custom rendering for diagrams.
    *   `FrontmatterExtension`: Custom handling for YAML metadata.
    *   `tiptap-markdown`: Serialization to Markdown.
*   **Styling**: Tailwind CSS Typography (`prose`).

---

## 3. Audit Findings & Gaps

### 3.1 Robustness & Reliability
*   **Transactional Integrity**: Bulk operations (recursive rename/move) lack atomic SQL transactions. A failure mid-operation could leave the DB and Filesystem in an inconsistent state.
*   **Error Granularity**: Most operations return `Result<T, String>`. The UI cannot distinguish between "Disk Full," "Access Denied," or "File Locked," leading to vague user feedback.
*   **Diagnostic Visibility**: Logs are persisted to `kms_logs`, but the user interface for viewing these is rudimentary.

### 3.2 Rendering Features
*   **[GAP] Math Support**: No KaTeX or MathJax integration for scientific/mathematical notes.
*   **[GAP] Callouts/Admonitions**: No support for Obsidian-style callouts (`> [!INFO]`).
*   **[GAP] Table of Contents**: No auto-generated TOC for long-form notes.
*   **[GAP] Interactive Wiki-links**: The editor recognizes `[[links]]` but lacks "Command + Click" or hover-preview functionality within the WYSIWYG view.
*   **[GAP] Task Management**: Basic task lists exist, but there's no "Global Task View" across the vault.

---

## 4. Proposed Implementation Plan

### Phase 1: Foundation of Robustness (Reliability Focus)
*   **Task 1.1: Structured Error System**: Convert `String` errors to a robust `KmsError` enum using `thiserror`.
*   **Task 1.2: Transactional Bulk Ops**: Wrap `rename_folder` and `move_item` in SQLite transactions.
*   **Task 1.3: External Sync & Conflict Management**: Implement a "Conflicts" notebook/folder to store versions of files modified externally while the app was offline. [DECIDED: YES]
*   **Task 1.4: Enhanced Diagnostics**: Implement a "System Health" dashboard in the Skill Hub showing index status, vector count, and recent sync errors.

### Phase 2: The "Premium Render" & UX Suite (Feature Focus)
*   **Task 2.1: Zen Mode (Default)**: Implement a distraction-free editing experience by default (toggleable sidebar, centered reading/writing area). [DECIDED: YES]
*   **Task 2.2: KaTeX Integration**: Add a Tiptap extension for inline and block math.
*   **Task 2.3: Admonition Extension**: Add support for styled callout blocks.
*   **Task 2.4: Wiki-Link Interactivity**: Enable clicking on `[[wiki-links]]` to open the target note.
*   **Task 2.5: Asset Previews**: Render image and PDF links directly within the editor.

### Phase 3: Hyper-Connectivity & Versioning (Intelligence Focus)
*   **Task 3.1: Robust Versioning System**: Implement a history mechanism (see Section 5.1 for options). [DECIDED: CUSTOM/ADVANCED]
*   **Task 3.2: Graph Visualization 2.0**: Improve the link graph to handle 1000+ nodes with clustering.
*   **Task 3.3: Smart Template System**: Advanced snippet insertion into KMS notes using the existing scripting engine.

---

## 5. Alternative Implementation Options & Deep Dives

### 5.1 Versioning Strategies
| Option | Description | Pros | Cons |
| :--- | :--- | :--- | :--- |
| **A: Local `.kms/history`** | Periodic snapshotting of files into a hidden sub-folder. | Offline-first, no external deps, easy "rollback" UI. | Increased disk usage; basic diffing logic required. |
| **B: Embedded Git (`git2-rs`)** | Silent git commits on every save or interval. | Industry-standard; perfect diffs; easy sync with GitHub/GitLab. | Complexity in handling merge conflicts; `.git` folder overhead. |
| **C: SQLite Page Snapshots** | Using SQLite's incremental backup or json-patch diffs in a DB table. | Atomic; single-file storage for history. | Harder to browse history outside the app; potential DB bloat. |

**SWOT Analysis (Versioning):**
*   **Strengths**: Ensures zero data loss; builds user trust.
*   **Weaknesses**: Implementation complexity for conflict resolution.
*   **Opportunities**: "Time Machine" UI for notes.
*   **Threats**: Versioning logs growing indefinitely without pruning policies.

#---

## 7. Technical Deep Dives

### 7.1 Git-Backed Reliability (The "Ghost Commit" Pattern)
To implement robust versioning without exposing the complexity of Git to the user:
*   **Shadow Repository**: A hidden `.git` folder within the vault.
*   **Auto-Commits**: The `KmsService` triggers a silent `git add .` and `git commit -m "Auto-save: <timestamp>"` after every successful note save (Task 1.2).
*   **Pruning**: A background job that runs `git gc` and potentially squashes old daily commits into monthly "archive" commits to prevent `.git` bloat.
*   **Conflict Resolution**: Use `git merge-file` on backend to automatically resolve simple text diffs when external changes are detected.

### 7.2 Zen Mode UI Layout
Defaulting to "Zen Mode" means prioritizing the content over the navigation:
*   **Centered Canvas**: The editor is centered with a max-width (e.g., 800px) to prevent long line lengths on ultrawide monitors.
*   **Collapsible Rails**: Both sidebars (Explorer and Backlinks) are collapsed by default and revealed only on hover or via hotkey (`Ctrl + \`).
*   **Floating Toolbar**: The editor toolbar only appears when the mouse moves near the top of the editor area, or is replaced by a slash-command (`/`) menu.

---

## 8. Finalized Phase 1-3 Implementation Plan

### Phase 1: Robustness (High Priority)
1.  **Task 1.1**: `KmsError` Enum Implementation (Rust).
2.  **Task 1.2**: Transactional multi-file operations (Rust).
3.  **Task 1.3**: **Conflict Checker**: On app launch, compare file hashes on disk with the last known hash in SQLite. If different, move the old DB version to `Conflicts/` before updating the index.

### Phase 2: Zen Mode & Premium Rendering
1.  **Task 2.1**: **Zen Mode Refactor**: Update `KmsApp.tsx` and `KmsEditor.tsx` to center the editor and auto-collapse sidebars.
2.  **Task 2.2**: KaTeX (Math) Tiptap extension.
3.  **Task 2.3**: Admonitions (Callouts) Tiptap extension.

### Phase 3: Advanced Versioning
1.  **Task 3.1**: **Git-Lite Service**: Integrate `git2-rs` to perform silent background versioning within the vault.
2.  **Task 3.2**: **History Browser**: A UI component to view and restore previous versions of a note.

---

## 9. Key Decisions [RESOLVED]
1.  **Sync Policy**: **YES**. Conflict management via "Conflicts" folder.
2.  **Versioning**: **ADVANCED (GIT-LITE)**. Use embedded Git for robust history tracking.
3.  **UI Density**: **YES**. Default to "Zen Mode" layout.

---

## 7. Verification Plan (Reliability Metrics)
*   **Unit Tests**: 100% coverage on path normalization and link extraction.
*   **Stress Tests**: Verify recursive rename performance on 5,000 nested files.
*   **Diagnostic Audit**: Ensure ALL errors in `KmsService` are logged to `kms_logs` with stack traces.
