# OCR Snapshot & HTML Report Implementation Plan

## Goal Description
Enhance the existing OCR regression testing framework by introducing robust "Golden Master" snapshot validation and generating interactive HTML side-by-side reports. This will significantly elevate the QA process, automating baseline comparisons and visually demonstrating parser heuristics.

## Proposed Changes

### `digicore-text-expander/Cargo.toml`
#### [MODIFY] Cargo.toml
- Add the `insta` crate to `[dev-dependencies]` for snapshot testing.
- Add `html-escape` or similar simple HTML generation utilities if needed, though we can likely construct simple HTML strings natively.

### `tests/ocr_regression_tests.rs`
#### [MODIFY] ocr_regression_tests.rs
- **Snapshot Validation**: Refactor the test loop to use `insta::assert_snapshot!(file_name, text)`. This will automatically save baseline `.snap` files on the first run, and strictly fail on subsequent runs if even a single character or markdown pipe changes.
- **HTML Report Generation**: Append each run's results into an `index.html` file within the timestamped output directory. The HTML will use CSS grid/flexbox to lay out the original image side-by-side with a `<pre>` block of the extracted Markdown.
- **Edge & Negative Cases**: Introduce explicit sub-tests for error handling (e.g., non-existent files, unsupported MIME types, completely empty images/noise if available).

## Verification Plan
### Automated Tests
- Run `cargo test --test ocr_regression_tests` with the `UPDATE_EXPECT=1` environment variable to generate initial baselines.
- Modify a test image or add a new one, run `cargo test` again to ensure it properly catches the diff and generates the HTML report.
- Review the generated `index.html` report in the browser to ensure CSS layout handles various image aspect ratios correctly.
