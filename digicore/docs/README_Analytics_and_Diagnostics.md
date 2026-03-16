# Analytics and Diagnostics Reports (Phases 34-47)

The OCR Analytics and Diagnostics system automates the generation of rich HTML reports, visual heatmaps, and mult-run timeline analytics. These tools aid humans in interpreting internal system heuristics and building confidence in extracted layouts.

## Generating and Viewing Interactive Reports

**Goal:** Understand how the OCR engine interprets dense tables and complex layouts by viewing an interactive `summary.html` report.

```mermaid
sequenceDiagram
    participant Dev as Human (Developer)
    participant Suite as Regression Test Suite (Automated System)
    participant Diff as HTML Diffing Engine
    participant FS as File System
    participant Browser as Web Browser (Human UI)

    Dev->>Suite: 1. `cargo test --workspace`
    Note over Dev,Suite: Human run command
    Suite->>Diff: 2. Generate expected vs. actual HTML comparison
    Note over Diff,FS: Automated System processing
    Diff->>FS: 3. Writes timestamped folders (e.g. `tests/results/2026-03...`)
    Diff->>FS: 4. Generates `summary.html` dashboard
    
    Note over Dev,Browser: Human Analysis Action
    Dev->>Browser: 5. Opens `summary.html`
    Browser-->>Dev: 6. Views Accuracy Leaderboard and Timeline Sparklines
    Dev->>Browser: 7. Toggles "Expected vs Actual" view
    Dev->>Browser: 8. Views inline diffing (Red/Green text)
```

### Human vs. System Control
- **System Action:** During `cargo test`, the regression suite silently iterates over every sample image (`.png`) and baseline (`.snap`). It runs string similarity algorithms (Levenshtein) and compiles an interactive wall-of-fame directory containing detailed HTML logs for each image, plus a high-level `summary.html` index.
- **Human Action:** The developer uses a standard Web Browser to open the `summary.html` file. They use mouse clicks to expand failing test cases, scroll side-by-side synchronized views, and toggle between Expected markdown and the Actual Engine generated markdown.

## Diagnostic Modes: Structural Confidence & Heatmaps

```mermaid
flowchart TD
    classDef humanView fill:#bbdefb,stroke:#01579b,stroke-width:2px;
    classDef systemAuto fill:#e1bee7,stroke:#8e24aa,stroke-width:2px;

    Extract[Pipeline Generation]:::systemAuto --> Jitter[Jitter Analysis & Scoring]:::systemAuto
    Jitter --> Matrix[Confidence Matrix Creation]:::systemAuto
    Matrix --> HTML[Inject SVG Heatmap Overlay into Report]:::systemAuto
    
    HTML --> HumanOpen([Developer opens Report]):::humanView
    HumanOpen --> ViewHeatmap([Developer toggles Diagnostic Layer]):::humanView
```

### Deciphering the Heatmap
When the developer toggles the **Diagnostic Heatmap** in an HTML report, they'll see:
*   **Green:** Perfect row/column consensus.
*   **Amber:** Mild jitter (word is slightly off-center from the recognized column). The system used a wide Gap Gate to reconstruct this.
*   **Red:** Low Structural Confidence. The word "floated" across multiple layout heuristics without matching a contiguous block. 

*Human Action Required:* Red highlights indicate the layout was too complex for the current heuristics. The developer should modify the system Configuration parameters via the Tauri UI and regenerate the Corpus snapshot.

## Multi-Run Timeline Tracking

To prevent regression degradation over multiple development cycles, the system tracks specific metrics over time.

### System Automated Telemetry Storage
1. At the conclusion of `cargo test`, the system aggregates the total Time (ms), total Entropy (Layout Complexity), and total Accuracy (%) across all samples.
2. The system appends this record to `history.json`.

### Human Dashboard Pulse
1. When generating `summary.html`, the system reads `history.json`.
2. It generates sleek, animated SVG sparklines representing "Performance Pulses".
3. The Human developer views these pulses in the browser to identify if recent commits caused a latency spike or broad accuracy drop across the entire regression suite.
