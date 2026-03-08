# Phase 42: Advanced Performance Analytics (Telemetry & Entropy)

Integration of high-resolution timing and layout complexity metrics into the OCR engine.

## Changes Implemented

### [Component Name] OCR Engine Core
- **[windows_ocr.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/src/adapters/extraction/windows_ocr.rs)**
    - Integrated high-resolution telemetry using `std::time::Instant` to capture extraction time.
    - Implemented **Layout Complexity Scoring (Entropy)** based on structural density and block distributions.
    - Exported metrics via `ExtractionResult.metadata` for downstream analysis.

### [Component Name] Regression Test Suite
- **[ocr_regression_tests.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/tests/ocr_regression_tests.rs)**
    - Updated report generation to display performance metrics.
    - Implemented a **Performance Leaderboard** in the summary dashboard (sorted by complexity).
    - Added NEW **Mini-Badges** to individual result cards (Timing, Entropy).
    - Enhanced Individual Detail Reports with performance headers (Extraction Cost, Complexity Class).

## Verification Results

### Automated Tests
- Ran full regression suite: `cargo test --test ocr_regression_tests -- --nocapture`
- Results: **100% Accuracy (20/20 images)**
- Performance: Average extraction time: **84ms**

### Dashboard Preview

````carousel
```html
<!-- Summary Dashboard with Performance Leaderboard -->
<div class="header">
    <h1>OCR Wall of Fame</h1>
    <div class="stats">
        <div class="stat-box"><span class="stat-val">20</span><span class="stat-lbl">Images</span></div>
        <div class="stat-box"><span class="stat-val">100%</span><span class="stat-lbl">Avg Accuracy</span></div>
        <div class="stat-box"><span class="stat-val">84ms</span><span class="stat-lbl">Avg Timing</span></div>
    </div>
</div>
```
<!-- slide -->
```rust
// Performance Leaderboard (Sorted by Entropy)
<tr>
    <td>Example1_Column-Data_OQjhJGtlge.png</td>
    <td><span class="badge high">HIGH</span></td>
    <td>75.0</td>
    <td>53ms</td>
</tr>
```
````

> [!TIP]
> **High Entropy** (>= 50.0) correctly identifies dense forms and complex tables (e.g., Example1, Example15).
> **Low Entropy** (< 20.0) identifies simple plain text documents with straightforward layouts.

## Next Steps
- [ ] Phase 43: Adaptive Tuning (Auto-selecting heuristics based on Entropy score).
