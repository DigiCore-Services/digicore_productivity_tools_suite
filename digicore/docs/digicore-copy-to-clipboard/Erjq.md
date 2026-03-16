# Phase 44: Structural Confidence & Uncertainty Highlighting

Improve OCR diagnostic visibility by deriving geometric confidence scores for every extracted word and highlighting "shaky" regions in the final reports.

## Proposed Changes

### [Component] OCR Engine (`windows_ocr.rs`)

#### [MODIFY] [windows_ocr.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/src/adapters/extraction/windows_ocr.rs)
- Implement `calculate_word_confidence(word, row, zones)` logic.
- Add `confidence` field to the `diagnostics` JSON objects.
- Penalize words with high "X-Jitter" (misalignment from column centers) or "Y-Jitter" (row baseline deviation).
- Flag "Size Anomalies" (e.g., extremely narrow or tall words that might be noise).

### [Component] Regression Suite (`ocr_regression_tests.rs`)

#### [MODIFY] [ocr_regression_tests.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/tests/ocr_regression_tests.rs)
- Update `generate_detail_report` to support a "Confidence" heatmap layer in SVG.
- Add a CSS class `.conf-low` (red) and `.conf-med` (amber) for underlining text in the report.
- Add a toggle in the UI to switch between "Structural Diagnostics" and "Confidence Heatmap".

## Verification Plan

### Automated Tests
- Run full regression suite:
  ```powershell
  $env:DIGICORE_OCR_FORCE_UPDATE=1; cargo test --test ocr_regression_tests -- --nocapture
  ```
- Verify `diagnostics` in `summary.html` contains the new `confidence` field.

### Manual Verification
- Open a generated `.html` report for a complex image (e.g., `Example13_Ukrainian-President`).
- Toggle "Confidence View" and verify that words with poor alignment are visually flagged.
