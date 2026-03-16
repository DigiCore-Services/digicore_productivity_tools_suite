# 📸 One-Click Corpus Generation Tool (Phases 30-55)

The Corpus Generation tool is a specialized suite built into the DigiCore Text Expander designed to rapidly accelerate the development and tuning of our OCR Extraction Engine. It allows developers (Humans) to quickly capture complex screen layouts and automatically (System) generate the necessary baseline testing files (`.png`, `.json`, `.snap`) required for automated regression testing.

## Why Corpus Generation?
Before this tool, adding a new test case involved taking a screenshot, manually copying the image to a test folder, writing a Rust test function, tweaking heuristics, and manually compiling the expected Markdown output block.

Now, you press a single hotkey, and the system handles the rest.

## Workflow: Human vs. System

```mermaid
sequenceDiagram
    participant Dev as Human (Developer)
    participant Hotkey as Global Hotkey Hook
    participant App as DigiCore App
    participant Engine as OCR Engine
    participant Store as Corpus Storage
    participant Test as Regression Test Suite

    Dev->>Hotkey: 1. Snip complex data -> Press Corpus Hotkey (e.g., Ctrl+Shift+Alt+C)
    Note over Dev,Hotkey: Explicit Human Dev Action
    
    Hotkey->>App: 2. Invoke Capture-to-Corpus hook
    Note over App,Engine: Automated System Generation
    App->>App: 3. Read image from clipboard
    App->>Dev: 4. Native "Save File" Dialog Popup (Pre-filled with Sanitized Window Title)
    Note over Dev,App: Human names the file (e.g., Example_xx_Wikipedia_Main)
    App->>Engine: 5. Generate initial OCR Baseline Markdown
    Engine-->>App: 6. Returned ExtractionResult
    App->>Store: 7. Save image as `[Name].png`
    App->>Store: 8. Save baseline data as `[Name]_baseline.json`
    App->>Store: 9. Save snapshot as `[Name].snap`
    
    Note over App,Dev: System Feedback
    App-->>Dev: 10. OS Toast Notification (Success: Corpus Generated)
    
    Note over Dev,Test: Later Process
    Dev->>Test: 11. Run `cargo test --workspace`
    Test->>Store: 12. Reads `.png` and `.snap`
    Test->>Engine: 13. Run OCR against `.png`
    Engine-->>Test: 14. Compare new output to `.snap`
```

## How to Use the Corpus Generator

### 1. Enable Corpus Mode (Human)
Ensure you have the latest Tauri Application running. 
1. Navigate to the **Configurations and Settings** tab.
2. Select **Corpus Generation** and toggle the feature ON.
3. Configure your desired **Output Directory** (defaults to `docs/sample-ocr-images`).

### 2. Capture a Complex Layout (Human)
Use the standard OS Snipping Tool (e.g., Win+Shift+S) to capture a confusing paragraph, a densely packed table, or a form with irregular alignment. The image is now on your clipboard.

### 3. Generate Snapshot (Human -> System)
While the Tauri App is running in the background, press the designated Corpus Generation Shortcut (e.g., `Ctrl+Shift+Alt+C`).

The **System** will immediately:
1. Intercept the hook.
2. Extract the image from the clipboard.
3. Extract the active Window Title and sanitize it (replacing spaces/special characters with `_`).
4. **Pop open a native "Save File" dialog** pre-filled with the name format `Example_xx_[Sanitized_Title].png`.
5. Once the Human clicks **Save**, the system will run the complete OCR Pipeline, including Layout Reconstruction and Grid Alignment using the current configured heuristics.
6. Save three files to your configured Corpus Output Directory using the chosen name:
    - `[Name].png`: The raw image.
    - `[Name]_baseline.json`: Metadata about the extraction (processing time, selected heuristic profile).
    - `ocr_regression_tests__[Name].snap`: The Insta crate golden master snapshot containing the expected Markdown output.

You will receive an OS Toast Notification upon successful generation.

## Regression Analytics (System)
Once you have generated multiple Corpus entries, you can run the OCR regression test suite.

The system will loop over all `.png` files in the Corpus, run the extraction, and instantly compare the Markdown against the `.snap` file.

### Interactive HTML Reports
To help diagnose failures, the system automatically generates an interactive `summary.html` report.

```mermaid
flowchart LR
    classDef humanView fill:#e1f5fe,stroke:#01579b,stroke-width:2px;
    classDef systemAuto fill:#f3e5f5,stroke:#4a148c,stroke-width:2px;

    RunTest([cargo test]):::humanView --> Exec[ocr_regression_tests.rs]:::systemAuto
    Exec --> Diff[Diffing Engine (strsim)]:::systemAuto
    Diff --> HTML[Generate HTML Reports]:::systemAuto
    
    HTML --> Dashboard[summary.html Dashboard]:::systemAuto
    HTML --> SVG[Diagnostic SVG Heatmaps]:::systemAuto
    
    Dashboard --> HumanReview([Developer Reviews Failures]):::humanView
```

Developers (Humans) can open the `summary.html` report to view:
*   Side-by-side expected vs actual outputs.
*   Red/Green text highlighting of exact character additions or deletions.
*   An overlay SVG Heatmap displaying **Structural Confidence Scores** (amber/red jitter highlighting for misaligned words).
