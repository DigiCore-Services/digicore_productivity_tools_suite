# Phase 42: Advanced Performance Analytics

This phase focuses on quantifying the **cost** and **complexity** of OCR extractions. We will track processing time and calculate a **Layout Complexity Score** (Entropy) to identify which documents are most challenging for the engine.

## Proposed Changes

### [Windows OCR Adapter](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/src/adapters/extraction/windows_ocr.rs)

#### [MODIFY] [Telemetry & Complexity Logic](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/src/adapters/extraction/windows_ocr.rs)
- Wrap the extraction logic in a timer to capture `extraction_ms`.
- Calculate a **Complexity Score** based on structural diagnostics:
    - `Entropy = (Bridges * 1.5) + (Coercions * 1.0) + (Blocks * 5.0) + (Tables * 10.0)`
- Add these metrics to the `metadata` field of `ExtractionResult`.

### [Regression Test Suite](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/tests/ocr_regression_tests.rs)

#### [MODIFY] [HTML Report Template](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/tests/ocr_regression_tests.rs)
- Update the individual image reports to display "Processing Time" and "Complexity Score" in the header.
- Color-code the Complexity Score (Green < 20, Amber 20-50, Red > 50).

#### [MODIFY] [Summary Dashboard](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/tests/ocr_regression_tests.rs)
- Add a "Performance Leaderboard" section to `summary.html`.
- Show "Fastest Extraction", "Most Complex Layout", and "Avg Processing Time".

## Verification Plan

### Automated Tests
- Run the full regression suite:
  `cargo test --test ocr_regression_tests -- --nocapture`
- Verify that metadata in `ExtractionResult` now contains `performance_metrics`.

### Manual Verification
- Open `summary.html` in a browser.
- Confirm that the Performance Leaderboard is rendered and contains realistic data.
- Check an individual image report (e.g., `Example 19`) to see the complexity breakdown.
