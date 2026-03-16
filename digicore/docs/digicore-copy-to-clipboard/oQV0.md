# Implementation Plan: Adaptive OCR Tuning (Phase 43)

This phase introduces an intelligent "Document Classifier" that uses the metrics from Phase 42 (Entropy, Density) to select the optimal `HeuristicConfig` for each document, ensuring maximum precision for both dense tables and sparse plain-text documents.

## Proposed Changes

### [Component Name] OCR Engine Core
#### [MODIFY] [windows_ocr.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/src/adapters/extraction/windows_ocr.rs)
- **Implement Document Classification**:
    - Analyze `Entropy` and `Mean Segment Density` early in the process.
    - Classify documents into: `FORMS_TABLES` (High Entropy), `REPORTS_DOCS` (Medium), and `PLAINTEXT` (Low).
- **Adaptive Configuration Selection**:
    - If `FORMS_TABLES`: Increase `column_consensus_density` and use aggressive pipe snapping.
    - If `PLAINTEXT`: Relax `Significant Gap Gate` and prioritize paragraph contiguity.
    - If `MULTI_COLUMN` (Gutter detected): Enable vertical priority flow.

### [Component Name] Regression Test Suite
#### [MODIFY] [ocr_regression_tests.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/tests/ocr_regression_tests.rs)
- **Expand Test Suite (Synthetic Generation)**:
    - Use `generate_image` to create 3 new high-fidelity test samples:
        - `Example21_Modern-Resume (Multi-Column Layout)`
        - `Example22_Dense-Financial-Form (Extreme Table Density)`
        - `Example23_Creative-Newsletter (Mixed Gutter flows)`
    - Establish Golden Master snapshots for these new samples.
- **Update HTML Reports**:
    - Display the "Selected Profile" (Adaptive Class) in the detail view.

## Verification Plan

### Automated Tests
- `cargo test --test ocr_regression_tests`
- Verify that accuracy scores remain at 100% or improve for known edge cases.
- Check metadata in generated JSON results to confirm correct classification.

### Manual Verification
- Review the "Adaptive Profile" badges in the generated HTML reports to ensure they match visual human intuition.
