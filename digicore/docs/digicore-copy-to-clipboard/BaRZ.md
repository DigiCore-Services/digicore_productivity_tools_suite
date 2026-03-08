# OCR Roadmap: The Ultimate Tuning & Structure Upgrade (Phases 36-43)

We have built a professional regression suite and perfected table extraction. Now, we expand the engine to handle **any** document type and optimize for **scale**.

## Completed Phases
- [x] **PHASE 36**: Project "Heuristic Auto-Search" (Scientific Tuning)
- [x] **PHASE 37**: Semantic OCR Correction (Pipe Snapping)
- [x] **PHASE 38**: Structured Data Export (CSV/JSON/MD)
- [x] **PHASE 39**: OCR Diagnostic Heatmaps (Visual X-Ray)
- [x] **PHASE 40**: Heuristic Refinement (False Positive Suppression)

## Upcoming Phases

### PHASE 41: Universal OCR Coverage (Multi-Column Layouts)
*   **The Problem**: 2-column resumes or legal briefs currently get merged into "long lines" or false tables.
*   **The Level-Up**: Implement **Gutter Detection** and **Block Reconstruction**.
    *   Detect vertical empty "gutters" that span multiple rows.
    *   Treat columns as independent flows rather than a single horizontal strip.
    *   *Result*: Professional handling of non-tabular multi-column documents.

### PHASE 42: Advanced Performance Analytics
*   **The Problem**: We know accuracy, but we don't know "Processing Cost" or "Layout Confidence".
*   **The Level-Up**: Integrate a performance telemetry layer in the regression suite.
    *   Track extraction time per image.
    *   Calculate "Layout Entropy" (how many bridges/coersions happened) to score document complexity.
    *   *Result*: A leaderboard that identifies which images are "cheap" vs "expensive" to process.

### PHASE 43: Adaptive Tuning (Per-Document Heuristics)
*   **The Problem**: One set of global heuristics doesn't fit every document perfectly.
*   **The Level-Up**: Implement a "Two-Pass" adaptive system.
    *   Pass 1: Low-res scan to identify document type (Receipt vs. Table vs. Letter).
    *   Pass 2: Apply a specialized `HeuristicConfig` tuned specifically for that document class.
    *   *Result*: Maximum precision across diverse input sources.
