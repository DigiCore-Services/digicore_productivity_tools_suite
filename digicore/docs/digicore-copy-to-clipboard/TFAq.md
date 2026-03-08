# OCR Roadmap v2: Enterprise-Grade Extraction (Phases 48-50)

With the self-correction loop and historical tracking in place, we now shift from "High Accuracy" to **"Domain Mastery"** and **"Visual Excellence"**.

## Proposed Phases

### PHASE 48: Layout-Aware Table Merging & Reconciliation
*   **The Problem**: Large tables that span multiple logical blocks (due to gutters or lines) are sometimes fragmented into multiple Markdown tables.
*   **The Level-Up**: Implement a **Structural Merger** that analyzes the alignment of adjacent table blocks.
    *   If two tables share the same column consensus and are separated by minor noise, they are merged into a single contiguous Markdown table.
    *   *Result*: Clean, unified data exports for long lists.

### PHASE 49: Semantic Entity Tagging (PII & Data Keys)
*   **The Problem**: extracted text is just a string. We don't know what's a "Date", "Amount", or "Email".
*   **The Level-Up**: Implement a heuristic Regex/Pattern matcher for common business entities.
    *   Identify and highlight Emails, Phone Numbers, Currency, and Dates in the HTML reports.
    *   Add a "Copy Entities" button to quickly grab just the structured data.
    *   *Result*: The tool becomes an intelligence layer for business documents.

### PHASE 50: Dashboard "Elite" UX Polish
*   **The Problem**: The current dashboard is functional but lacks the "WOW" factor of a premium enterprise app.
*   **The Level-Up**: Apply **Advanced Web Design Aesthetics**.
    *   **Glassmorphism**: Translucent card backgrounds with subtle blurs.
    *   **Dynamic Animations**: Smooth transitions for the Performance Pulse sparklines.
    *   **Custom Typography**: Switch to premium fonts like `Outfit` or `JetBrains Mono` for better scannability.
    *   **Actionable Badges**: Expand the "badge click" logic to auto-scroll and highlight specific fixes in the source view.
    *   *Result*: A dashboard that feels like a state-of-the-art diagnostic suite.

## Next Steps

1.  **Authorize Phase 48**: We will start by refining the `perform_reconstruction_pass` to return meta-data about table boundaries for easier merging.
