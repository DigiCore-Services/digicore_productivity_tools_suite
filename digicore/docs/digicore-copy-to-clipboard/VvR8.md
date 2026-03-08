# Phase 39: OCR Semantic & Structural Diagnostics (Heatmaps) Implementation Plan

Since the Windows OCR API does not provide native word-level confidence, we will implement "Diagnostic Confidence" tracking. This identifies which parts of the text were most heavily "processed" or "reconstructed" by our layout engine.

## Proposed Changes

### [Core Domain]

#### [MODIFY] [extraction.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-core/src/domain/value_objects/extraction.rs)
- Add `diagnostics: Option<serde_json::Value>` to `ExtractionResult`.
- This will store a flat list of `WordDiagnostic { text, x, y, width, height, flags: ["coerced", "bridged"] }`.

### [OCR Engine]

#### [MODIFY] [windows_ocr.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/src/adapters/extraction/windows_ocr.rs)
- Update `WordInfo` and `RowSegment` to include `flags: Vec<String>`.
- **Bridged Detection**: Tag words separated by >0.6x height as `bridged`.
- **Coerced Detection**: Pass coercion info back from `refine_extracted_text` to the final result.
- Populate the `diagnostics` metadata field.

### [Regression Suite]

#### [MODIFY] [ocr_regression_tests.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/tests/ocr_regression_tests.rs)
- Update `generate_detail_report` to include a **Heatmap View**.
- In the "Raw Output" pane:
    - **Blue Underline**: Coerced characters (Structural symbols).
    - **Amber Underline**: Bridged content (Potential layout jitter).
- Add a legend to the HTML report explaining the colors.

## Verification Plan

### Automated Tests
- Run the regression suite:
  ```powershell
  cargo test --test ocr_regression_tests -- --nocapture
  ```
- Verify `results_*.json` for a table-heavy sample (e.g., `Example3_Simple-Table`) contains a populated `diagnostics` array.

### Manual Verification
- Open the HTML report for `Example3_Simple-Table`.
- Confirm that the `|` symbols in the "Raw Output" or "Side-by-Side" view are underlined in blue (indicating they were coerced from `l`/`1`).
- Confirm that words with large gaps between them are highlighted in amber.
