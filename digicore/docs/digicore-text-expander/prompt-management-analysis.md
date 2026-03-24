# Analysis: Re-imagining Text Expansion for AI Prompt Management

**Date:** March 23, 2026  
**Subject:** Transitioning from "General Text Expansion" to "AI Engineering Prompt Management"  
**Target Platform:** DigiCore Text Expander (Rust/Tauri) + Cursor/Antigravity IDE Integration

---

## 1. Executive Summary
The goal is to evolve the current `digicore-text-expander` from a utility for day-to-day text snippets into a professional-grade **Prompt Management Library**. This transition is designed to empower "vibe coders" and AI engineers by providing versioned, categorized, and testable prompt assets that can be seamlessly injected into IDE workflows (Cursor, Visual Studio Code).

## 2. Current Implementation Audit

### 2.1 Backend: `digicore-core` & `Snippet` Entity
*   **Data Model:** The `Snippet` struct is a flat entity. However, the **DigiCore Knowledge Management Suite (KMS)** already implements a more robust Markdown-based block model.
*   **Storage:** Snippets use JSON; KMS uses Markdown files + SQLite (`sqlite-vec`) for semantic search.
*   **Template Logic:** Text Expander has basic replacement; KMS aims for a standard "Knowledge Hub" approach.

### 2.2 Frontend: `tauri-app` (React/TS)
*   **GUI Pattern:** A traditional table view with standard CRUD operations.
*   **Search/Discovery:** Fast fuzzy search is implemented, but lacks hierarchical navigation or semantic search.
*   **Editor:** A simple modal editor that lacks version history or a playground for immediate AI response validation.

---

## 3. Gap Analysis: Snippets vs. AI Engineering Requirements

| Feature | Current Snippet System | World-Class Prompt Management |
| :--- | :--- | :--- |
| **Organization** | Flat Categories | Hierarchical Folders + Multi-Tagging |
| **Versioning** | None (Overwrites on save) | Git-like History (Commits, Rollbacks) |
| **Variables** | Simple `{}` placeholders | Typed variables (Select, Number, Multi-line) |
| **Metadata** | Basic (Category, Profile) | Model Specs (Temp, TopP), Provider, Tokens |
| **Testing** | Manual copy-paste to IDE | Integrated Prompt Playground (LLM API Sync) |
| **IDE Synergy** | Hotkey injection only | Context-aware injection (Files, Symbols, @-links) |

---

## 4. Re-imagined GUI & UX Design

### 4.1 Hierarchical & Tag-Based Organization
Instead of a single "Category" column, we propose a **Semantic Sidebar**:
*   **Workspaces:** Grouping by project (e.g., "Project-X", "Internal-Tools").
*   **Nested Folders:** `/Coding/Rust/Refactoring` vs `/Coding/Python/Docstrings`.
*   **Smart Tags:** `#system-prompt`, `#one-shot`, `#creative`, `#strict`.

### 4.2 The "Prompt Playground" UX
A new **Split-Pane View** for snippet selection and editing:
*   **Left Pane:** Library list with rich metadata (Last run success, Token count).
*   **Center Pane:** Editor with syntax highlighting for variables (Handlebars/Jinja-style).
*   **Right Pane:** Instant Test Bench. Select an LLM provider (OpenAI, Anthropic, Ollama) and test the expansion immediately.

### 4.3 Variable Injection UI
When a prompt with many variables is triggered, a **Glassmorphism Overlay** (similar to Spotlight) should appear:
- Inline form fields for each variable.
- Presets for common variable values.
- Real-time preview of the *rendered* prompt before it's sent to the target app.

### 4.4 Convergence with KMS (The "Skill Hub")
The analysis reveals that the **KMS (Knowledge Management Suite)** should provide the underlying storage and indexing for the Prompt Management Library.
*   **Skills Standard:** Support `SKILL.md` files with YAML frontmatter natively in the KMS vault, adhering to Anthropic/Cursor progressive disclosure standards.
*   **Hierarchical Generation:** Use **Templates** to guide users through creating professional skills (Metadata -> Instructions -> Resources).
*   **Global & Local Sync:**
    - **Global Path Sync:** Automatically populate `%user%\.cursor\skills` and `~/.claude/skills/` from the KMS "Global Skills" collection.
    - **Project-Level Injection:** Map KMS "Rule" notes to `.cursorrules` and Claude-equivalent `.clauderules` in the active workspace.
*   **Multi-Agent Rule Sync:** Ensure that project-level rules are mirrored across Cursor and Claude Code surfaces to maintain a consistent agent persona.
*   **Semantic Skills Retrieval:** Leverage `sqlite-vec` to allow AI agents to query the DigiCore KMS for relevant skills, keeping context usage efficient through "load on demand" patterns.

---

## 5. Alternative Implementation Options

### Option A: Extend Existing Snippet Model (Evolutionary)
*   **Approach:** Add optional fields to the `Snippet` struct for `tags`, `version_hash`, and `model_params`.
*   **Pros:** Backward compatibility; avoids codebase fragmentation.
*   **Cons:** Backend storage (JSON) might become bloated.

### Option B: Parallel "Prompt Library" Component (DEPRECATED)
*   **Recommendation:** Move away from a separate library and instead **embed Prompt Management into KMS**.

### Option C: KMS-Driven "Unified Skill Repository" (New Recommended)
*   **Approach:** Use the KMS vault as the single source of truth for both snippets, prompts, and "Agent Skills".
*   **Pros:** Native Markdown versioning (Git-compatible); Unified search via `sqlite-vec`; Full compatibility with Cursor/Anthropic standards.
*   **Cons:** Requires completing the KMS implementation to a stable state.

---

## 6. SWOT Analysis (Option A vs. Option B)

| | **Option A: Extend Snippets** | **Option B: New Prompt Library** |
| :--- | :--- | :--- |
| **Strengths** | Fast time-to-market; uses existing hotkey engine. | Specialized UX; better performance for large prompt sets. |
| **Weaknesses** | UI might feel cluttered for "simple text" users. | Increased complexity; potential user confusion. |
| **Opportunities** | Unified management for all text assets. | Can integrate directly with LLM SDKs for evaluation. |
| **Threats** | Feature creep might degrade the original app's speed. | May alienate users who just want simple expansion. |

---

## 7. Final Architectural Decisions

Based on user configuration and strategic alignment, the following architectural decisions are finalized:

1.  **KMS Convergence:** The Prompt Management Library and Agent Skills Repository are **converged** into the DigiCore KMS. The KMS vault serves as the single source of truth.
2.  **Rule Precedence:** **Project-level specific rules** (e.g., in `.cursorrules`) always override global rules (e.g., in `%user%\.cursor\skills`).
3.  **Sync Logic:** **Bidirectional Synchronization** is enforced. The system will monitor both the KMS vault and project-level rule files. Conflicts will trigger a **User Review UI** with a side-by-side diff to confirm changes before syncing.
4.  **Skill Scope:** The system will support all relevant skill fields for **Vibe Coding** workflows, including specialized templates for code generation, autonomous testing, and technical documentation.

---

## 8. Implementation Roadmap (Phase 5)

### 8.1 Backend (Rust)
- [ ] **KMS Skill Schema:** Implement the metadata extraction logic for `SKILL.md` YAML frontmatter.
- [ ] **Sync Engine:** A background service using `notify` to watch `.cursorrules` and `.clauderules` in active workspaces.
- [ ] **Diff Algorithm:** Implement a text-based diffing service for identifying cross-source conflicts.

### 8.2 Frontend (Tauri/React)
- [ ] **Skill Creator:** A multi-step form (Wizard) for filling out skill templates.
- [ ] **Sync Manager:** A dedicated UI tab to review "Pending Syncs" and resolve diffs.
- [ ] **Playground Enhancement:** Integrated "Test Run" button for Skills that executes the instructions against a local/remote LLM.

### 8.3 IDE Synergy
- [ ] **Global Provisioning:** Logic to ensure `%user%\.cursor\skills` is always populated with the user's "Golden Skills" from KMS.
- [ ] **Project Mapping:** A "Link Project" UI to associate a KMS folder with a specific local directory.
