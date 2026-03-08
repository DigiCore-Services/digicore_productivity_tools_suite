# Plan: Final OCR Engine Polish

This plan addresses the remaining edge cases in the "Screen-to-Markdown" engine, specifically column-over-piping in complex documents and missing 1-character columns.

## Proposed Changes

### [digicore-text-expander]

#### [MODIFY] [windows_ocr.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/src/adapters/extraction/windows_ocr.rs)
- **Significant Gap Gate (Paragraph Integrity)**:
    - Instead of splitting on `0.4 * row_h`, only split segments if the gap is truly significant (`0.8 * row_h`) OR if it crosses a **Strong Consensus Zone** (density >= 5). This prevents normal word spaces in paragraphs from becoming table pipes.
- **Strong Grid Consensus**:
    - Increase the density requirement for common column zones from `3` to `5`. This ensures that random word alignments in paragraphs don't create "ghost columns".
- **Contiguous Block Requirement**:
    - A block of rows should only trigger Markdown Table mode if it contains **at least 3 contiguous TableCandidate rows**. 2-row "tables" will be treated as plaintext with indentation.
- **Universal Word Recovery (Safe Appending)**:
    - Ensure that any "floating" words between zones are always appended to the current segment rather than starting a new empty-piped column.
- **Gap Sensitivity**: 
    - Adjust the "Significant Gap" gate to be more context-aware. Once a Table Block is established, the gap tolerance for inner cells will be lowered to `0.8 * row_height` to ensure tightly packed tables don't fall back to plaintext.

## Verification Plan

### Manual Verification
1. **Wikipedia Age Table (Ex 1)**:
   - Verify that the **Gender (F/M)** column is correctly captured as a distinct column.
   - Verify that the header (First name, Last name, etc.) is wrapped in pipes.
2. **README Pagination (Ex 4)**:
   - Verify that the table only has 3 columns (| Flag | Description | Default |), ignoring the global zones from the README header.
3. **Zillow List (Ex 3)**:
   - Verify it remains a clean, 2-column Markdown table.
4. **Job Posting (Ex 2)**:
   - Verify it remains clean plaintext.
