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

### 3. Advanced Analytics & Dashboard (New!)
The regression suite now does more than just diffing; it provides quantitative metrics and a high-level overview:
- **Accuracy Scoring**: Every test run now calculates a percentage accuracy score (using Levenshtein distance) against the Golden Master.
- **The "Wall of Fame" Dashboard**: A `summary.html` file is generated in the results folder, providing a grid view of all tested images with color-coded "Health" badges.
- **Interactive Visual Diffs**: Clicking "Details" on the dashboard leads to a side-by-side view with a **character-level visual diff** (Red for deletions, Green for additions) computed directly in your browser.

### 4. Advanced Diagnostics & Heuristic Tuning (Phase 35)
The regression suite is now a powerful "IDE" for OCR development:
- **Word-Level Diffing**: Diffs now compare words rather than characters. This drastically reduces "noise" from minor spacing or newline shifts.
- **Side-by-Side Sync Scroll**: A new view that places the **Baseline** and **Actual** text side-by-side with synchronized scrolling.
- **Visual Toggle**: Instantly flip between the **Visual Diff**, **Side-by-Side**, and **Raw Output** using the new diagnostics toolbar.
- **Word Metrics**: Real-time counts of `+Added` and `-Deleted` words are displayed in the header.

---

## How to Use the New Pipeline

### Running Tests & Enforcing Baselines
To run the suite and verify against your baselines without noise from unrelated unit tests:
```powershell
# Navigate to the expander crate
cd crates/digicore-text-expander

# Run ONLY the OCR integration tests
cargo test --test ocr_regression_tests -- --nocapture
```
*(Note: `--nocapture` allows you to see the accuracy scores in the terminal in real-time).*

### Updating Baselines (Accepting Changes)
Use the `cargo-insta` CLI with the same filter to review and accept new Golden Masters:
```powershell
# Filter by --test to bypass unrelated test failures and speed up review
cargo insta test --test ocr_regression_tests --review
```

### Viewing the Analytical Reports
1. Navigate to the `docs/sample-ocr-images/results_[TIMESTAMP]` folder.
2. Open `summary.html` (or `index.html`) in your browser to see the "Wall of Fame".
3. Click "Details" on any card to see the interactive visual diff.

---

## Technical Validation
- **Snapshots Created**: 20 baseline files generated for the full image corpus.
- **Edge cases**: Verified that the engine gracefully handles missing files and invalid input types.
- **HTML Layout**: Verified that the report correctly handles both high-resolution screenshots and small receipts.

![OCR Regression Report Example](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/docs/sample-ocr-images/results_2026-03-07_0256AM-0800/index.html)
*(Note: View this file in your browser for the full interactive experience.)*
