# Professional Implementation Plan: Extensible Text Extraction & OCR Service

This document outlines a robust, multi-purpose architecture for text extraction within the DigiCore ecosystem. The design follows Hexagonal Architecture principles to ensure reusability across clipboard capture, file uploads (Images, PDFs, Docs), and future modules.

## 1. Architectural Vision: The Extraction Engine
Instead of a "Clipboard OCR" feature, we are building a **Unified Extraction Service**. It decouples *where* the data comes from (Source) from *how* we get the text (Strategy).

### Hexagonal Design
- **Core Strategy (Port)**: `TextExtractionPort` trait.
- **Domain Models**:
    - `ExtractionSource`: Enum supporting `MemoryBuffer`, `LocalFile(PathBuf)`, `Stream`.
    - `ExtractionMimeType`: Supports `Image`, `Pdf`, `Docx`, `Txt`, `Markdown`, `Python`.
    - `ExtractionResult`: Contains the text, confidence scores, and metadata (pages, dimensions).
- **Adapters**:
    - `WindowsNativeOcrAdapter`: Implements `TextExtractionPort` for images via WinRT.
    - `PlainFileAdapter`: Implements `TextExtractionPort` for `.txt`, `.py`, `.md`.
    - `FutureAdapter`: Placeholder for PDF or external OCR services.

## 2. Infrastructure & Persistence

### A. Database Schema (Parent-Child Polymorphism)
To support a child relationship, we will standardize `parent_id` as a reference to the source entry.
- **Table**: `clipboard_history` (and future `document_registry`)
- **New Columns**:
    - `parent_id INTEGER`: Links the extracted text to its source.
    - `source_uri TEXT`: Optional URI if the source is an external file.
- **Entry Types**:
    - `image`: The raw visual source.
    - `document_file`: A source like a PDF or script.
    - `extracted_text`: The result (OCR or File Read).

### B. Extensible API (Tauri / RPC Layer)
```rust
// Generic API usable by clipboard listener OR file upload UI
async fn extract_text_from_source(source: ExtractionSource) -> Result<ExtractionResult, Error>;
```

## 3. Implementation Phases

### Phase A: Core Extraction Service (SOLID/SRP)
1.  **Define the Port**: Create `digicore_text_expander::ports::extraction::TextExtractionPort`.
2.  **Dispatcher Service**: Create an `ExtractionDispatcher` that detects the MIME type and routes to the correct adapter.
    - *Example*: `.py` file -> `PlainFileAdapter`; `.png` -> `WindowsNativeOcrAdapter`.

### Phase B: Windows Native OCR Adapter
1.  Implement the `TextExtractionPort` using `Windows.Media.Ocr`.
2.  Ensures reliability by handling WinRT async boundaries and memory management of `SoftwareBitmap`.

### Phase C: Clipboard Integration (First Consumer)
1.  The existing `sync_current_clipboard_image_to_sqlite` calls the `ExtractionDispatcher`.
2.  Extracted text is saved as a child record.

### Phase D: Future File Upload Support (Next Consumer)
1.  A new "Document Workspace" or "Script Upload" feature calls the **same** `ExtractionDispatcher`.
2.  No rework required for OCR or file reading logic.

## 4. Pros/Cons & SWOT Analysis

| Option | Pros | Cons |
| :--- | :--- | :--- |
| **Monolithic Clipboard OCR** (Original) | Fast to implement. | Zero reuse; massive rework for file uploads. |
| **Hexagonal Extraction Service** (Proposed) | **Infinite reuse**; clean SRP; easy to test; decoupled from OS/UI. | Slightly higher initial boilerplate. |

### SWOT Analysis (Proposed Solution)
- **Strengths**: True SOLID compliance; one engine for OCR/Docs/Scripts; high reliability.
*   **Weaknesses**: Requires careful management of async Rust traits (`async-trait`).
*   **Opportunities**: Can integrate with LLMs for "Summarize Uploaded Image/PDF" in future.
*   **Threats**: Large PDF processing might require background workers.

## 5. Key Decisions & User Input
1.  **Child Record Visibility**: When you "Search", do you want to see both the Image and the OCR text as separate results, or should they be grouped? (Recommend: **Separate searchable results linked by UI grouping**).
2.  **Sync vs Async**: OCR/Large File reading can take 50ms-500ms. Should we block the capture event or run it as a background task? (Recommend: **Background task with "Processing..." placeholder**).

## 6. Verification & Logging
- **Diagnostic Logging**: Every extraction step (Source Detection -> Adapter Routing -> Extraction -> Persistence) will be logged with a unique session ID.
- **Resilience**: Gracious failure (e.g., if OCR fails but image save succeeds, we still keep the image).
