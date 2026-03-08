# Phase 47: Self-Correction Loop (Recursive Refinement)

Implement a two-pass reconstruction system that automatically detects complex layouts ("High Entropy") and applies refined heuristics to boost accuracy.

## Proposed Changes

### [Component] Extraction Engine (`windows_ocr.rs`)

#### [MODIFY] [windows_ocr.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/src/adapters/extraction/windows_ocr.rs)

1.  **Refactor Reconstruction Logic**:
    *   Extract steps 3-8 into a helper method: `perform_reconstruction_pass(&self, words: &[WordInfo], config: &HeuristicConfig, img_width: f32, img_height: f32) -> ReconstructionResult`.
    *   This keeps the expensive `OcrEngine` call (Step 1-2) separate.

2.  **Implement Recursive Loop**:
    *   In `extract()`:
        *   Call `perform_reconstruction_pass` with initial config.
        *   Analyze Entropy.
        *   If `entropy > 50.0` (High):
            *   Create a `refined_config` (e.g., `cross_zone_gap_factor` slightly loosened, `significant_gap_gate` tightened).
            *   Call `perform_reconstruction_pass` again with refined config.
            *   Compare results: Keep the one with **lower entropy** or fewer "bridged" diagnostics.

3.  **Update Metadata**:
    *   Add `refinement_executed: true/false` and `pass_comparison` details to the final metadata.

## Verification Plan

### Automated Tests
- Run regression tests on known "High Entropy" images (e.g., Example 23 Newsletter, Example 22 Financial Form).
- Verify that `metadata` shows `refinement_executed: true` for these samples.
- Ensure accuracy remains at 100% or improves.

### Manual Verification
- Review [Summary Dashboard](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/docs/sample-ocr-images/results/summary.html) to see the "Entropy" score distribution and verify the leaderboard reflects the refined processing.
