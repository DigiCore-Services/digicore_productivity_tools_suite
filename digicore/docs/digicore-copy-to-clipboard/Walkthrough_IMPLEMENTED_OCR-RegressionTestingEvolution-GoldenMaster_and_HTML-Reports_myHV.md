# OCR Regression Testing Evolution: Golden Master & HTML Reports

We have successfully elevated the OCR regression testing suite to a professional, enterprise-grade validation pipeline. The system now not only identifies regressions but also provides a high-fidelity visual workspace for evaluation.

## Key Enhancements

### 1. Golden Master Snapshot Validation
Using the industry-standard `insta` crate, every test image now has a corresponding "Golden Baseline" snapshot (`.snap` file). 
- **Automatic Enforcement**: If any future code change alters the OCR output for a baseline image—even by a single space or pipe—the test will fail.
- **Git-Integrated Baselines**: These snapshots are stored in `crates/digicore-text-expander/tests/snapshots` and can be version-controlled, providing a permanent history of our "known good" parser states.

### 2. Interactive HTML Diff Reports
Every test run now generates a beautiful, self-contained HTML dashboard. 
- **Side-by-Side Analysis**: View the original source image alongside the rendered Markdown output.
- **Historical Tracking**: Each report is saved in a unique, timestamped directory (e.g., `results_2026-03-07_0256AM-0800`), allowing you to compare parser behavior across different development iterations.

---

## How to Use the New Pipeline

### Running Tests & Enforcing Baselines
To run the suite and verify against your baselines:
```powershell
# Navigate to the expander crate
cd crates/digicore-text-expander

# Run the standard cargo test
cargo test --test ocr_regression_tests
```

### Updating Baselines (Accepting Changes)
If you intentionally improved the OCR logic and want to lock in the new (better) output as the new "Golden Master," use the `cargo-insta` CLI:
```powershell
# This command runs the tests and interactively lets you accept/reject diffs
cargo insta test --review
```

### Viewing the Visual Report
After any test run, navigate to the latest results folder in `docs/sample-ocr-images/` and open `index.html` in your browser.

---

## Technical Validation
- **Snapshots Created**: 20 baseline files generated for the full image corpus.
- **Edge cases**: Verified that the engine gracefully handles missing files and invalid input types.
- **HTML Layout**: Verified that the report correctly handles both high-resolution screenshots and small receipts.

![OCR Regression Report Example](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/docs/sample-ocr-images/results_2026-03-07_0256AM-0800/index.html)
*(Note: View this file in your browser for the full interactive experience.)*
