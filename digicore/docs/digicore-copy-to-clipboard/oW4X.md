# Advanced OCR Analytics & Dashboard Implementation Plan

This plan outlines the steps to take our OCR regression testing to the next level by adding quantitative accuracy metrics, visual diffing, and a centralized dashboard.

## User Review Required

> [!IMPORTANT]
> - This implementation adds `strsim` as a development dependency for string similarity calculations.
> - The HTML report will now include a JavaScript-based character-level diffing engine (using a lightweight inline implementation to avoid external CDN dependencies where possible).

## Proposed Changes

### [Component] OCR Regression Tests

#### [MODIFY] [Cargo.toml](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/Cargo.toml)
- Add `strsim = "0.11"` to `[dev-dependencies]`.

#### [MODIFY] [ocr_regression_tests.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/tests/ocr_regression_tests.rs)
- **Snapshot Retrieval**: Implement logic to find and read the `.snap` file content for each image.
- **Accuracy Calculation**: Use `strsim::normalized_levenshtein` to compute a percentage match between current output and the snapshot.
- **Diff Injection**:
    - Inject a JS-based diffing algorithm into the report.
    - Pass both "Expected" (Snapshot) and "Actual" (Current) text to the HTML template.
- **Summary Dashboard**:
    - Accumulate results (name, accuracy, status) into a `summary.html` file at the root of the timestamped results directory.
    - Create a "Wall of Fame" grid with color-coded accuracy badges (Green > 95%, Yellow > 80%, Red < 80%).

## Verification Plan

### Automated Tests
1. **Run Regression Suite**:
   ```powershell
   cargo test --test ocr_regression_tests -- --nocapture
   ```
   - Verify that the console output shows "Accuracy Score" for each image.
   - Verify that `summary.html` and `index.html` files are generated correctly.

### Manual Verification
1. **Visual Dashboard Inspection**:
   - Open `summary.html` in a browser.
   - Confirm it shows a grid of all images with accurate status badges.
2. **Interactive Diff Inspection**:
   - Click into a specific image's report (`ExampleN.png.html`).
   - Confirm that the "Detailed Diff" section correctly highlights additions/deletions in the text.
