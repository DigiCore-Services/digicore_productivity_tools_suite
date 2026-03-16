# OCR Roadmap: The Ultimate Tuning & Structure Upgrade (Phases 36-47)

We have built a professional regression suite and perfected table extraction. Now, we expand the engine to handle **any** document type and optimize for **scale**.

## Completed Phases
- [x] **PHASE 36**: Project "Heuristic Auto-Search" (Scientific Tuning)
- [x] **PHASE 37**: Semantic OCR Correction (Pipe Snapping)
- [x] **PHASE 38**: Structured Data Export (CSV/JSON/MD)
- [x] **PHASE 39**: OCR Diagnostic Heatmaps (Visual X-Ray)
- [x] **PHASE 40**: Heuristic Refinement (False Positive Suppression)
- [x] **PHASE 41**: Universal OCR Coverage (Multi-Column Layouts)
- [x] **PHASE 42**: Advanced Performance Analytics (Telemetry & Entropy)
- [x] **PHASE 43**: Adaptive OCR Tuning (Per-Document Heuristics)
- [x] **PHASE 44**: Structural Confidence Highlighting (Jitter Analysis)
- [x] **PHASE 45**: Layout-Aware Header Detection (Hierarchical Markdown)

## Upcoming Phases

### PHASE 46: Multi-Run "Timeline" Performance Tracking
*   **The Problem**: We don't have a historical view of how accuracy or timing changes over repository history.
*   **The Level-Up**: Implement a persistent `history.json` and a "Performance Pulse" dashboard.
    *   Track accuracy/timing trends across multiple test runs.
    *   Generate sparklines in the summary dashboard.
    *   *Result*: Instant visibility into regressions or improvements over time.

### PHASE 47: Self-Correction Loop (Recursive Refinement)
*   **The Problem**: Some images are just "too complex" for a single pass.
*   **The Level-Up**: Implement a recursive refinement loop.
    *   If entropy is "High", re-run the extraction with aggressive noise damping or jitter correction.
    *   *Result*: A final 1-2% accuracy boost for low-contrast/rotated images.
