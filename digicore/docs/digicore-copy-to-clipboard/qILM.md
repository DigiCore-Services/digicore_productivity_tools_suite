# Task: Unify App Data Paths and Fix Image Persistence

## Status
- [x] PLANNING: Unified Path Resolution
    - [x] Analyze codebase for hardcoded `DigiCore` paths
    - [x] Design `DataPathResolver` strategy
    - [x] Update implementation plan for Option A standardized paths
- [x] EXECUTION: Path Unification
    - [x] Implement configurable toggle for clipboard image capture ([2026-03-06](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/components/ConfigTab.tsx))
    - [x] Rename "Enable Copy-to-Clipboard Capture" to "Enable Copy-to-Clipboard (Text) Capture"
    - [x] Add "Enable Copy-to-Clipboard (Image) Capture" checkbox
    - [x] Update backend to respect image capture toggle
    - [x] Update Windows clipboard listener for real-time image capture
    - [x] Decouple image persistence from JSON toggle
    - [x] Add application metadata (Process/Window Title) to image entries
- [x] VERIFICATION: Final Commit and Sync
    - [x] Commit and push Core & Backend changes
    - [x] Commit and push Frontend UI & Binding changes
    - [x] Commit and push Documentation & Audit changes
    - [x] Verify remote synchronization on `feature/ui-decoupling-phase-0-1`

- [x] PHASE 3: GUI Thumbnail Fix
    - [x] Update Tauri capabilities with `core:asset` and `core:path` permissions
    - [x] Update `tauri.conf.json` with appropriate CSP for `asset:` protocol
    - [ ] Verify thumbnail rendering in Clipboard History

- [x] PHASE 4: Cleanup & Warnings
    - [x] Resolve `default_assets_root_dir` warning in `clipboard_repository.rs`
    - [x] Resolve `show_toast` warning in `discovery.rs`

- [x] PHASE 5: Settings UI Refinement
    - [x] Rename "Apply" buttons to "Save" for consistency
    - [x] Implement visual feedback (status message) for all save actions
    - [x] Position status message prominently (e.g., next to buttons)

- [x] PHASE 6: Dependency Migration & Compatibility
    - [x] Replace `meval` with `evalexpr` to resolve `nom v1.2.4` warning
    - [x] Verify DSL evaluation parity

- [x] PHASE 7: Tauri Plugin Order & Permissions
    - [x] Add `tauri-plugin-fs` dependency
    - [x] Reorder plugin registration in `src-tauri/src/lib.rs`
    - [x] Update `default.json` with correct permissions (fixed invalid `persisted-scope:default`)

- [x] PHASE 10: Fix Clipboard Image Capture
    - [x] Analyze and debug capture failure (identified sequential image dedup bug)
    - [x] Update `api.rs` to correctly merge text/image enabled flags in `update_config`
    - [x] Update `windows_clipboard_listener.rs` with retry logic and verbose logging
    - [x] Fix `clipboard_history.rs` to allow sequential images to bypass string-based dedup
    - [x] Resolve compilation error (missing `Duration` import) in `windows_clipboard_listener.rs`
    - [x] Verify image capture and storage on disk (robust against sequential captures)

- [x] PHASE 11: Decouple SQLite and JSON Persistence
    - [x] Identify coupling bug in `persist_clipboard_entry_with_settings`
    - [x] Remove `json_output_enabled` early return in `api.rs`
    - [x] Ensure JSON writing only occurs if toggle is enabled
    - [x] Verify SQLite capture works independently of JSON output

- [x] PHASE 12: Extensible Text Extraction & OCR
    - [x] Audit current implementation and research extensible OCR/Doc options
    - [x] Generate standalone `OCR_IMPLEMENTATION_PLAN.md` (Updated for Extensibility)
    - [x] Define `TextExtractionPort` and `ExtractionSource` in Core
    - [x] Add `COPY_TO_CLIPBOARD_OCR_ENABLED` storage key
    - [x] Update `ConfigTab.tsx` with conditional OCR toggle
    - [x] Implement `WindowsNativeOcrAdapter` (Hexagonal Adapter)
    - [x] Link extraction results in DB (Parent-Child relationship for reusability)
    - [x] Verify extraction from both clipboard and simulated file inputs

- [x] PHASE 13: Comprehensive Testing & Verification
    - [x] Implement unit tests for `ExtractionDispatcher`
    - [x] Implement unit tests for `PlainFileExtractionAdapter`
    - [x] Implement unit tests for `WindowsNativeOcrAdapter` (Mock/Capability check)
    - [x] Verify error handling (negative cases: empty files, invalid MIME types)
    - [x] Perform smoke test on integrated application

- [x] PHASE 14: Post-Extraction Bug Fixes
    - [x] Resolve `tokio::spawn` panic by switching to `tauri::async_runtime::spawn` in `api.rs`
    - [x] Fix `WINCODEC_ERR_COMPONENTNOTFOUND` (0x88982F50) by encoding raw pixels to PNG in `api.rs`
    - [x] Fix duplicate capture bug by ignoring child (extracted) records in de-duplication SQL
    - [x] Verify background thread safety and extraction reliability with unit tests

- [x] PHASE 15: OCR UI Enhancements
    - [x] Add image icon next to OCR text in Clipboard History table
    - [x] Implement "View Source Image" context menu option for OCR entries
    - [x] Add "Image" action button to the "Actions" column for OCR text
    - [x] Ensure interactive icon/menu correctly opens parent image in system viewer
    - [x] Resolve TypeScript type issues in context menu action filtering

- [x] PHASE 16: Table UX Polish
    - [x] Remove redundant "Snippet Created" column
    - [x] Move "Actions" column to prominent second position (between # and Preview)
    - [x] Refactor action buttons to a compact badge-style layout
    - [x] Integrate "Promoted" checkmark/state directly into the action buttons

- [x] PHASE 17: Layout-Aware OCR
    - [x] Implement line-by-line text reconstruction in `windows_ocr.rs`
    - [x] Detect paragraph breaks using vertical spatial gaps
    - [x] Preserve indentation and bullet alignment
    - [x] Handle potential text rotation using `TextAngle`

- [x] PHASE 18: Feature Documentation
    - [x] Generate comprehensive `README_Files-and-Images-OCR-Text-Extraction.md`
    - [x] Include Mermaid diagrams for architecture and flow
    - [x] Document layout preservation algorithm and vertical gap logic
    - [x] Provide future extensibility guide (Hexagonal ports)

- [x] PHASE 19: Advanced OCR Indentation
    - [x] Analyze `X` coordinate logic for bullets/sub-levels
    - [x] Implement margin-based whitespace injection
    - [x] Preserve bullet characters and list alignment

- [x] PHASE 20: UX Polish: Copy Feedback
    - [x] Add `copied` state to `ClipboardTab.tsx`
    - [x] Implement "Copied!" message for "View Full Content" Copy button
    - [x] Implement "Copied!" message for "Actions" column Copy button

- [x] PHASE 21: Grid-Aware OCR Reconstruction
    - [x] Design word-grouping algorithm (Y-coordinate binning)
    - [x] Implement row-based sorting and horizontal alignment
    - [x] Refine vertical gap detection for multi-column layouts

- [x] PHASE 22: Column-Aligned Table Reconstruction
    - [x] Design column-zone detection (X-coordinate clustering)
    - [x] Implement global "tab stop" mapping
    - [x] Refine whitespace injection to snap to column zones

- [x] PHASE 23: Markdown Table Generation
    - [x] Analyze row/column density for table detection
    - [x] Implement Markdown pipe-separator logic
    - [x] Detect and format header separator rows (`|---|---|`)
    - [x] Preserve and normalize bullets (e.g., `•` -> `-`)
    - [x] Support side-by-side "Hybrid" layouts (Text + Table)

- [x] PHASE 24: Consensus-Based Table Alignment
    - [x] Design vertical consensus algorithm (X-alignment counts)
    - [x] Implement Gap Significance checks (>1.5x height)
    - [x] Implement vertical density score for table blocks
    - [x] Finalize bullet / icon normalization

- [x] PHASE 25: High-Fidelity OCR Polish
    - [x] Implement Global Margin Analysis (Indentation-faithful)
    - [x] High-Resolution Column Discovery (0.4x avg_h)
    - [x] Active Proximity Splitting (Cell split precision)
    - [x] Universal Word Recovery (Data loss prevention)
    - [x] Logic Header Grid Alignment

- [x] PHASE 26: OCR Recalibration & Paragraph Integrity
    - [x] Implement Significant Gap Gate (0.8x height)
    - [x] Increase Column Consensus Density (>= 5)
    - [x] Implement Contiguous Block Requirement (3+ rows)
    - [x] Refine Table Mode Escape Logic

- [x] PHASE 27: Final Grid Tuning
    - [x] Implement Adaptive Zone Splitting (0.35x gap for strong zones)
    - [x] Align Header Segmentation (Pre-table split logic)

- [x] PHASE 28: Small Table Refinement
    - [x] Implement Vertical Contiguity checking (3+ unbroken rows per column)
    - [x] Apply Hyper-Sensitive Transitions (0.15x gap if crossing verified zones)

- [x] PHASE 29: Grid Alignment Refinement
    - [x] Integrate Jitter-Tolerant Column Zones (1.5x cluster threshold)
    - [x] Restore 0.35x Adaptive Gate to prevent paragraph fragmentation

- [x] PHASE 30: Automated Regression Testing
    - [x] Create OCR integration test suite
    - [x] Execute against all sample images
    - [x] Review OCR evaluation outputs

- [x] PHASE 31: Dynamic Test Output Management
    - [x] Update regression test to use timestamped result folders
    - [x] Run test suite against newly added images
    - [x] Review new image outputs and update documentation

- [x] PHASE 32: Snapshot Validation (Golden Master)
    - [x] Introduce `insta` crate for robust assertion testing
    - [x] Update `ocr_regression_tests.rs` to validate against `.snap` baselines
    - [x] Create edge and negative test cases (missing files, invalid types)

- [x] PHASE 33: Interactive HTML Reports
    - [x] Generate side-by-side HTML view of Image vs Markdown
    - [x] Incorporate HTML generation into regression test pipeline
    - [x] Verify HTML report outputs visually

- [x] PHASE 34: Advanced Regression Analytics & Dashboard
    - [x] Integrate `strsim` or custom Levenshtein for accuracy scoring
    - [x] Implement inline HTML diffing (Red/Green) in reports
    - [x] Create `summary.html` (Wall of Fame) dashboard
    - [x] Update README with analytics and dashboard instructions

- [x] PHASE 35: Advanced OCR Diagnostics & Heuristic Tuning
    - [x] Implement Word-level diffing for stable comparison
    - [x] Add "Side-by-Side Sync Scroll" to HTML reports
    - [x] Add "Expected vs Actual" toggle/flip view
    - [x] Implement "Accuracy Breakdown" (Additions vs Deletions counts)

- [x] PHASE 36: Project "Heuristic Auto-Search"
    - [x] Decouple heuristic constants from `windows_ocr.rs` into Config
    - [x] Implement `--tune` sweep mode in `ocr_regression_tests.rs`
    - [x] Generate "Sweep Report" dashboard with optimal values

- [x] PHASE 37: Semantic OCR Correction
    - [x] Implement Column-Aware Symbol Coercion (fixing `|`, `l`, `1`)
    - [x] Add post-processing pass to cleanup fragmented punctuations

- [x] PHASE 38: Structured Data Export (CSV/JSON/MD)
    - [x] Extend `ExtractionResult` with structured data model
    - [x] Generate `.csv`, `.json`, and `.md` artifacts in results folder

- [x] PHASE 39: OCR Confidence Heatmaps (Diagnostic Mode)
    - [x] Implement structural diagnostic tracking (bridging & coercion)
    - [x] Implement SVG heatmap overlay in HTML reports
    - [x] Add diagnostic legend and amber/blue highlighting
    - [x] Implement "Mean Segment Density" consensus check
    - [x] Restrict "Strong Column" coercion to confirmed table blocks

- [x] PHASE 41: Universal OCR Coverage (Multi-Column Layouts)
    - [x] Implement layout gutter discovery (Vertical Voids)
    - [x] Implement Logical Block Reconstruction (Vertical Priority)
    - [x] Refine local margin analysis for independent blocks

- [x] Phase 42: Advanced Performance Analytics (Telemetry & Entropy)
    - [x] Integrate high-resolution timing into `windows_ocr.rs`
    - [x] Implement Layout Complexity Score (Entropy) calculation
    - [x] Update regression test reporting to display performance metrics
    - [x] Implement "Performance Leaderboard" in summary dashboard
- [x] Phase 43: Adaptive OCR Tuning (Per-Document Heuristics)
    - [x] Implement Document Classifier (use Entropy & Density)
    - [x] Define specialized `HeuristicConfig` presets
    - [x] Implement two-pass/adaptive configuration selection logic
    - [x] Update regression reports to show selected adaptive profile
    - [x] Add new complex test cases to verify adaptive switching

## Next Level Up (Phases 44-47)

### PHASE 44: Structural Confidence & Uncertainty Highlighting
*   **The Level-Up**: Calculate a **Structural Confidence Score** based on geometric alignment.
    *   Compare word alignment to nearest column center (Jitter analysis).
    *   Flag "floating" words that don't satisfy row/column consensus.
    *   *Result*: Amber/Red highlighting in HTML reports for "shaky" extractions.

### PHASE 45: Layout-Aware Header Detection (Hierarchical Markdown)
*   **The Level-Up**: Detect `H1`, `H2` headers based on relative font height and centering.
    *   Classify headers by comparing word height to document-wide average.
    *   *Result*: Genuine hierarchical Markdown (`#`, `##`, `###`) in exports.

### PHASE 46: Multi-Page "Timeline" Regression
*   **The Level-Up**: Track accuracy/timing trends across multiple test runs.
    *   Implement `history.json` to store multi-run telemetry.
    *   Generate "Performance Pulse" sparklines in the summary dashboard.

### PHASE 47: Self-Correction Loop (Recursive Refinement)
*   **The Level-Up**: If entropy is "High", re-run with aggressive jitter damping.
    *   *Result*: Final 1% accuracy boost for low-contrast/rotated images.
- [x] Phase 44: Structural Confidence & Uncertainty Highlighting
    - [x] Calculate "Word Jitter" (Alignment deviation score)
    - [x] Implement Structural Confidence Score (0-100%)
    - [x] Update `ExtractionResult` metadata with confidence map
    - [x] Update HTML reports with amber/red underlines for low confidence
- [x] Phase 45: Layout-Aware Header Detection (Hierarchical Markdown)
    - [x] Calculate "Relative Font Height" (Height vs Doc Average)
    - [x] Detect centered/hero primary text as Headers
    - [x] Inject `#`, `##` into Markdown reconstruction
- [x] Phase 46: Multi-Run "Timeline" Performance Tracking
    - [x] Design `history.json` schema (Timestamp, Accuracy, Latency, Entropy)
    - [x] Implement historical data persistence in regression suite
    - [x] Generate "Performance Pulse" sparklines in `summary.html`
- [x] Phase 47: Self-Correction Loop (Recursive Refinement)
    - [x] Refactor reconstruction logic into standalone pass
    - [x] Implement entropy-driven recursive trigger
    - [x] Add refinement metadata to extraction results

### PHASE 48: Layout-Aware Table Merging & Reconciliation
- [x] Implement Structural Merger for adjacent table blocks
- [x] Add column consensus check for block reconciliation
- [x] Verify merging on multi-segment table samples

### PHASE 49: Semantic Entity Tagging (PII & Data Keys)
- [x] Implement Regex/Pattern matching for Emails, Dates, and Currency
- [x] Add semantic metadata layer to `ExtractionResult`
- [x] Update HTML reports with data-type highlighting

### PHASE 50: Dashboard "Elite" UX Polish
- [x] Implement Glassmorphism and Backdrop Blur for cards
- [x] Enhance Performance Pulse with smooth SVG transitions
- [x] Add "Actionable Leaderboard" auto-scroll logic

### PHASE 51: Heuristic Fuzzing & Synthetic Stress Testing
- [x] Implement image rotation/jitter simulation in test suite
- [x] Add "Resilience Score" to dashboard metrics
- [x] Verify threshold stability against low-res/skewed samples

### PHASE 52: "One-Click" Corpus Generation Utility
- [x] Implement capture-to-corpus backend hook
- [x] Add developer hotkey handler in application layer
- [x] Auto-generate "Baseline" snapshot for new samples

### PHASE 53: Advanced Table Semantics & Export
- [x] Detect Table Header vs Body vs Footer structures
- [x] Reconstruct isolated structured tables natively
- [x] Integrate single-click "Export to CSV/Excel" flows

### PHASE 54: Final Profiling & Dependency Audit
- [x] Implement Configuration-first `_runtime_config.yaml` for heuristic tuning
- [x] Perform crate dependency audit and optimization
- [x] Profile and eliminate hot-path allocation bottlenecks
- [x] Finalize documentation and architecture diagrams

### PHASE 55: GUI-First Configuration Integration
- [x] Implement new config keys in `digicore-text-expander/src/ports/storage.rs`
- [x] Plumb new configs (Corpus, Extraction heuristics, Golden ratios) through `api.rs` and `types.ts`
- [x] Add "Corpus Generation" and "Extraction Engine" settings UI to Tauri frontend (`ConfigTab.tsx`)
- [x] Refactor Corpus and Windows OCR adapters to load configs directly from `JsonFileStorageAdapter`
- [x] Delete `_runtime_config.yaml` to prevent split-brain states

### PHASE 56: Comprehensive Documentation Generation (Phases 12-55)
- [x] Generate/Update `README_...md` files (e.g., Corpus, Core features)
- [x] Generate `architecture.md` with Mermaid diagrams
- [x] Generate `use_cases.md` with Mermaid diagrams
- [x] Generate `requirements_spec.md` with Mermaid diagrams
- [x] Review and refine embedded visual diagrams for all markdown output

### PHASE 57: Bug Fix: Corpus Generation Hotkey Conflict
- [x] Investigate `Ctrl+Shift+Alt+S` conflict on Wikipedia/Browsers
- [x] Fix bitwise logic in `hotstring.rs` failing to consume the valid shortcut
- [x] Update default shortcut to `Ctrl+Shift+Alt+C` mapped to 0x43 ('C')
- [x] Verify functionality and update README
- [x] Inject `CorpusConfig` hot-reloading into Tauri `api.rs` so dynamic changes apply
- [x] Implement native `rfd::FileDialog` when the hotkey is pressed
- [x] Auto-extract active Window Title and sanitize it for the default filename `Example_xx_[Title].png`
- [x] Fix Win32 Compilation error (AttachThreadInput namespace)
- [x] Correct sequence diagram and README for new naming conventions

### PHASE 58: OCR Regression Suite Refactor & Duplication Bugfix
- [x] Refactor `ocr_regression_tests.rs` to use `insta::Settings` with `force_pass(true)`
- [x] Implement non-halting loop with aggregate failure summary
- [x] Investigate and fix duplication bug in `windows_ocr.rs` (Table reconstruction phase)
- [x] Verify `Example18` snapshot output no longer duplicates text
- [x] Update Summary Dashboard to maintain aggregate stats during non-halting runs
