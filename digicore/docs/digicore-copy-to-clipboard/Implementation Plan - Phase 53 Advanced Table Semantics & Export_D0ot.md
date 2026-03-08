# Implementation Plan - Phase 53: Advanced Table Semantics & Export

## Goal Description
Enhance the existing Layout-Aware OCR extraction engine to not only detect grid structures but also semantically understand table regions (Header vs Body vs Footer). Additionally, provide a structured export mechanism (CSV/Excel) and introduce a dedicated "Table Accuracy" metric to our Regression testing suite to prevent structural regressions.

## Proposed Changes

### 1. Domain Layer (`digicore-core`)

#### [MODIFY] `src/domain/value_objects/extraction.rs`
- Update `TableBlock` to include semantic row classifications:
    ```rust
    pub struct TableBlock {
        pub y_top: f32,
        pub y_bottom: f32,
        pub column_centers: Vec<f32>,
        pub headers: Vec<Vec<String>>, // First N rows detected as headers
        pub body: Vec<Vec<String>>,    // Standard data rows
        pub footers: Vec<Vec<String>>, // Optional trailing summary rows (e.g., "Total")
    }
    ```
  *(Legacy `rows` field can be removed or kept as a computed property for backwards compatibility if needed, but we will migrate to semantic splits).*

#### [NEW] `src/domain/ports/export.rs`
- Define `TableExportPort`:
    ```rust
    pub trait TableExportPort: Send + Sync {
        fn export_to_csv(&self, table: &TableBlock, output_path: &std::path::Path) -> anyhow::Result<()>;
    }
    ```

### 2. Extraction Adapter (`digicore-text-expander`)

#### [MODIFY] `src/adapters/extraction/windows_ocr.rs`
- **Header Detection Heuristics**: Evolve the layout builder loop (around line 520) to split `current_table_rows` into semantics based on:
    - Row 0 usually being headers.
    - Large Y-gaps between Row 0 and Row 1 indicating a structural boundary.
    - Rows containing keywords like "Total", "Subtotal", or "Sum" at the bottom being footers.
- Update `TableBlock` instantiation to populate `headers`, `body`, and `footers` instead of a flat `rows` vector.

### 3. Export Adapter (`digicore-text-expander`)

#### [NEW] `src/adapters/export/csv_export.rs` (or `table_export.rs`)
- Add `CsvTableExportAdapter` implementing `TableExportPort`.
- Uses the standard `csv` crate (to be added to `Cargo.toml`) to write the `headers`, `body`, and `footers` seamlessly into a structured `.csv` format that Excel natively opens.

### 4. Testing & Regression (`digicore-text-expander`)

#### [MODIFY] `Cargo.toml`
- Add `csv = "1.3"` dependency for the export adapter.

#### [MODIFY] `tests/ocr_regression_tests.rs`
- **Table Accuracy Metric**: Enhance `RegressionResult` to track `table_count` and `total_cells`.
- Create a baseline comparison strictly for table grids, penalizing the score if columns merge incorrectly or rows are missed.
- Extend `generate_detail_report()` to output standard HTML `<table>` tags visualizing the reconstructed tables directly in the regression report.

## Verification Plan

### Automated Tests
- Run `cargo test --test ocr_regression_tests` against the existing corpus documents containing tables (e.g., `financial_statement.png` or `invoice.png`).
- Verify the new `Table Accuracy` score asserts >90% structural integrity.
- Verify `cargo insta review` accurately reflects the new generic `TableBlock` serialized JSON layout.

### Manual Verification
- Execute the application.
- Capture a complex table with a distinct header row and bottom "Total" row.
- Verify the `ExtractionResult` JSON outputs correctly segregated `headers` and `footers`.
- Trigger the export hook (if mapped to UI) and attempt to open the resulting `.csv` in Excel.
