# Implementation Plan - Phase 48: Layout-Aware Table Merging & Reconciliation

This phase focuses on improving the structural integrity of complex documents where tables might be fragmented across multiple logical blocks due to gutters or noise.

## Proposed Changes

### [digicore-text-expander]

#### [MODIFY] [windows_ocr.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/src/adapters/extraction/windows_ocr.rs)
- **Table Metadata Collection**: Enhance the `ReconstructionResult` to include the bounding boxes and column consensus signatures of each detected table.
- **Structural Merger**: Implement a post-processing pass that:
    - Analyzes vertical proximity between adjacent `all_extracted_tables` entries.
    - Compares "Column Consensus" signatures (X-coordinates and count of cells).
    - Merges tables that align horizontally and are separated by minimal non-table content.
- **Refined Markdown Output**: Ensure merged tables are rendered as a single unified Markdown table in the final output.

## Verification Plan

### Automated Tests
- **Regression Suite**: `cargo test --test ocr_regression_tests -- --nocapture`
- Verify that complex samples with fragmented zones (e.g., resumes or dense forms) now consolidate their tabular data into fewer, more meaningful blocks.

### Manual Verification
- **Multi-Segment Sample**: Use a sample with a large table split by a page break or vertical line.
- Inspect the `.md` and `.csv` artifacts to confirm they are unified into a single table/file.
- Verify through the [Summary Dashboard](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/docs/sample-ocr-images/results_2026-03-07_1252PM-0800/summary.html) that accuracy remains at 100% and refinement triggers normally.
