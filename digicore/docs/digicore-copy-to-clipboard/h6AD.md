# Phase 46: Multi-Page "Timeline" Regression

Track accuracy and performance metrics across multiple test runs to provide a "Performance Pulse" for the OCR engine.

## Proposed Changes

### [Component] Regression Suite (`ocr_regression_tests.rs`)

#### [NEW] [history.json](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/docs/sample-ocr-images/history.json)
- Store an array of run objects:
  ```json
  {
    "timestamp": "2026-03-07T12:00:00Z",
    "avg_accuracy": 0.98,
    "avg_latency_ms": 95,
    "total_samples": 23,
    "commit_hash": "optional"
  }
  ```

#### [MODIFY] [ocr_regression_tests.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/tests/ocr_regression_tests.rs)
- **Persistence**: After each run, append the summary results to `history.json` (limiting to the last 50 runs).
- **Dashboard Update**:
    - Update `generate_summary_dashboard` to load `history.json`.
    - Inject an SVG Sparkline in the header showing **Accuracy** and **Latency** trends.
    - Add a "Timeline" tab or section showing a table of recent runs.

### [Component] UI/UX (HTML Summary)

#### [MODIFY] [summary.html](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/docs/sample-ocr-images/summary.html)
- Add a CSS-based sparkline component or simple inline SVG.
- Highlight "Trends":
    - 🟢 Latency decreased by 5% vs last run.
    - 🔴 Accuracy dropped to 99% (1 sample mismatch).

## Verification Plan

### Automated Tests
- Run the regression suite twice:
  ```powershell
  cargo test --test ocr_regression_tests
  ```
- Verify that `history.json` is created and contains two entries.

### Manual Verification
- Open `summary.html` and verify the "Performance Pulse" chart is visible and correctly reflects the history data.
