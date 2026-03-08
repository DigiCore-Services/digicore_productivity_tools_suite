# Phase 41: Universal OCR Coverage (Multi-Column Layouts)

The current layout engine is optimized for single-column text and horizontal tables. However, multi-column documents (e.g., resumes, legal briefs, newspapers) often have their vertical flows incorrectly merged. This plan introduces **Gutter Detection** to recognize independent text blocks.

## Proposed Changes

### [Windows OCR Adapter](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/src/adapters/extraction/windows_ocr.rs)

#### [MODIFY] [Gutter Discovery](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/src/adapters/extraction/windows_ocr.rs)
- Implement a "Vertical Void" scanner that identifies X-axis ranges where no words exist across a significant majority of rows.
- Distinguish between **Table Gutters** (small, frequent) and **Layout Gutters** (wide, spanning most of the page height).

#### [MODIFY] [Logical Block Reconstruction](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/src/adapters/extraction/windows_ocr.rs)
- If a wide "Layout Gutter" is detected, the engine will switch to **Vertical Priority Reconstruction**.
- Instead of reading Row 1 [L, R], Row 2 [L, R], it will extract all segments from the Left Block, then all segments from the Right Block.
- This ensures that a 2-column resume remains readable as two distinct sections.

#### [MODIFY] [Indentation Preservation](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/src/adapters/extraction/windows_ocr.rs)
- Refine the margin analysis to be local to each detected block.

## Verification Plan

### Automated Tests
- Create a new regression sample `Example14_MultiColumn_Resume.png` (or simulate it in tests).
- Verify that text from the left column does not interleave with text from the right column.
- `cargo test --test ocr_regression_tests -- --nocapture`

### Manual Verification
- Review the HTML reports for multi-column samples.
- Verify that the "Visual Diff" pane shows the logical order of reading (Column 1 then Column 2).
