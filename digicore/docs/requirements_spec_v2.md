# Requirements Specification (Phases 12-55)

This document outlines the system requirements successfully delivered in the DigiCore Text Expander between Phases 12 and 55. It delineates the responsibilities of the automated background **System** and the interactive **Human** interfaces.

## 1. Functional Requirements

### 1.1 Extensible OCR Engine (Phase 12-21)
*   **Req 1.1.1 [System]:** The application MUST automatically intercept image clipboard payloads and pass them to an abstraction layer (Port) for extraction processing without requiring explicit user action.
*   **Req 1.1.2 [System]:** The abstraction layer MUST route requests to registered Adapters, such as the `WindowsNativeOcrAdapter`.
*   **Req 1.1.3 [Human]:** The application MUST allow the Human user to globally toggle OCR extraction ON/OFF via the Tauri Settings GUI. Validated via visual `on/off` slider state.
*   **Req 1.1.4 [System]:** The extractor MUST accurately recognize words and reconstruct localized geometries, preserving lines, gaps, blanks, and paragraph indentations relative to the original image coordinates.
*   **Req 1.1.5 [System]:** Extracted semantic representations MUST be linked as `child_id` records to the original `parent_id` raw image to avoid duplication in the SQLite repository.

### 1.2 Automated Layout-Aware Table & Markdown Generation (Phase 22-29)
*   **Req 1.2.1 [System]:** The engine MUST automatically detect multi-column table layouts using a density consensus threshold for X/Y coordinates.
*   **Req 1.2.2 [System]:** The engine MUST generate valid Markdown formatting (e.g., `| Column 1 | Column 2 |`, complete with header separator rows) when sufficient columnar density is detected.
*   **Req 1.2.3 [Human]:** The Human user MAY review the resulting Markdown output directly in the "Clipboard History" Tauri tab. The user MAY copy this Markdown text to their primary system clipboard with a single click.

### 1.3 Entity Tagging and Structural Export (Phase 38-49)
*   **Req 1.3.1 [System]:** The engine MUST parse extracted text to flag semantic entities, specifically: Emails, Dates, Currency, and generic numerical series (e.g., SSN).
*   **Req 1.3.2 [System]:** The system MUST normalize punctuation fragmentations (e.g., converting consecutive ` | ` bars into `l`).
*   **Req 1.3.3 [Human]:** The Human user MUST be able to trigger (via Context Menu or Global Export button) a structured data dump of the currently highlighted Snippet, converting it to `.json` or `.csv`.
*   **Req 1.3.4 [System]:** The background execution MUST NOT block the Tauri GUI when large table extractions or disk writes for exports occur.

### 1.4 Automated Configuration & Heuristic Loading (Phase 55)
*   **Req 1.4.1 [Human]:** The Human user MUST be able to define OCR constants, gap ratios, and extraction triggers directly in the application's "Extraction Engine" settings UI.
*   **Req 1.4.2 [System]:** The system MUST deprecate `_runtime_config.yaml` to prevent split-brain states and fully migrate all values to the singleton `JsonFileStorageAdapter`.
*   **Req 1.4.3 [System]:** Internal components MUST retrieve configurations dynamically prior to processing payloads, bypassing the need for application restarts following a Human settings change.

## 2. Non-Functional Requirements

### 2.1 Performance & Reliability
*   **Req 2.1.1 [System]:** The OCR Extraction pipeline MUST execute from raw image to Markdown payload in under `100ms` for average screens. The system MUST NOT crash or loop infinitely when fed a pure blank image.
*   **Req 2.1.2 [System]:** If the system experiences high memory utilization during table segmentation mapping (`X/Y Coordinate matrices`), the application MUST aggressively evict older vectors or abort extraction rather than memory panicking (`OOM`).

### 2.2 Usability & Feedback
*   **Req 2.2.1 [Human]:** Settings updates saved by the user MUST yield an immediate UI success confirmation (e.g., a localized OS Toast). Failed imports of Legacy settings MUST yield informative context menus detailing which rules were dropped.
*   **Req 2.2.2 [Human]:** Extracted OCR table items MUST be visually distinct from plain text captures in the user’s history table (e.g., via a specialized `Image` or `A` [Text] Badge).

## 3. Diagnostic Testing & Golden Master Analytics (Phase 30-47)

### 3.1 Developer System Tooling
*   **Req 3.1.1 [Human]:** Developers MUST be able to invoke `cargo test --workspace` and trigger automated OCR regression suites against previously vetted `Corpus` images (`.png`).
*   **Req 3.1.2 [System]:** The regression suite MUST assert that newly generated Extractions exactly match the baseline Snapshot (`.snap`) files representing previous successful runs (Insta crate).

### 3.2 HTML Reporting Dashboards
*   **Req 3.2.1 [System]:** A background testing engine MUST generate an interactive self-contained `summary.html` report displaying OCR comparisons (Expected vs Actual).
*   **Req 3.2.2 [Human]:** The Human Reviewer (Developer) CAN toggle an Overlay Heatmap directly in the HTML report to view `Structural Confidence Scores` highlighting words with `Jitter` (amber/red).

```mermaid
mindmap
  root((Requirements))
    Functional
      [System] Adapter Architecture
      [Human] GUI Toggles
      [System] Semantic Entities
      [System] Markdown Tables
    NonFunctional
      [System] 100ms Execution 
      [System] Memory Safety
      [Human] UI Feedback
    Diagnostics
      [System] Automated `.snap` Regression
      [Human] Visual HTML Heatmaps
      [System] Layout Entropy Tracking
```
