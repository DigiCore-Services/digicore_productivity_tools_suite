# Text Expander Architecture (Phases 12-55)

The DigiCore Text Expander uses a Hexagonal Architecture (Ports and Adapters) to ensure business logic is decoupled from UI, storage, and platform-specific implementations. This architectural choice has been pivotal in adding extensible features like Advanced OCR, Automated Corpus Generation, and Adaptive Configuration over the course of Phases 12 through 55.

## High-Level System Architecture

The following diagram illustrates the relationship between the Tauri Frontend (Human Interaction layer), the Core Domain Logic, and the various external Adapters via defined Ports.

```mermaid
graph TD
    %% Define Styles for Human vs System
    classDef humanLayer fill:#e1f5fe,stroke:#01579b,stroke-width:2px;
    classDef systemLayer fill:#f3e5f5,stroke:#4a148c,stroke-width:2px;
    classDef coreLayer fill:#e8f5e9,stroke:#1b5e20,stroke-width:2px;
    classDef adapterLayer fill:#fff3e0,stroke:#e65100,stroke-width:2px;

    %% Human Layer
    subgraph Human Interaction Layer
        UI[Tauri React/TypeScript UI]
        Tray[System Tray / Hotkeys]
    end
    class UI,Tray humanLayer;

    %% Entry Points
    UI -- "Taurpc / IPC Actions" --> API[Tauri API Layer `api.rs`]
    Tray -- "OS Events" --> Platform[Platform Watchers `windows_clipboard_listener.rs`]
    
    %% Application Core
    subgraph Application Core
        App[Application Services `clipboard_history.rs`, `corpus_generator.rs`]
        Domain[Domain Models `ExtractionResult`, `ClipboardEntry`]
        App --> Domain
    end
    class API,Platform systemLayer;
    class App,Domain coreLayer;

    API --> App
    Platform --> App

    %% Ports
    subgraph Ports & Adapters
        ExtPort[TextExtractionPort]
        StoragePort[StoragePort]
        
        App --> ExtPort
        App --> StoragePort
        
        OCRAdapter[WindowsNativeOcrAdapter]
        FileAdapter[PlainFileAdapter]
        SqliteAdapter[SQLite Repository]
        JsonAdapter[JsonFileStorageAdapter]
        
        ExtPort -. "Impl" .-> OCRAdapter
        ExtPort -. "Impl" .-> FileAdapter
        
        StoragePort -. "Impl" .-> SqliteAdapter
        StoragePort -. "Impl" .-> JsonAdapter
    end
    class ExtPort,StoragePort,OCRAdapter,FileAdapter,SqliteAdapter,JsonAdapter adapterLayer;
```

### Explanation of Layers

1. **Human Interaction Layer (Blue):** This is where the user directly interacts with the application, either through the rich React/TypeScript configuration UI or by triggering OS-level hotkeys and clipboard events.
2. **System Entry Points (Purple):** The boundaries that receive input from the human layer (IPC calls from Tauri, or OS hooks for clipboard changes) and normalize them for the application core.
3. **Application Core (Green):** Contains the business rules, such as extraction dispatching (`ExtractionDispatcher`), adaptive OCR tuning, and clipboard deduplication.
4. **Ports & Adapters (Orange):** The infrastructure layer. The Core defines *what* it needs via Ports (e.g., `TextExtractionPort`), and the Adapters know *how* to do it (e.g., `WindowsNativeOcrAdapter` calling WinRT APIs).

## The OCR Processing Pipeline

The text extraction engine is a highly refined pipeline that converts raw images into structured Markdown, CSV, or raw text. This process is fully **automated (system-driven)** once triggered by a human action (like copying an image or dropping a file).

```mermaid
flowchart LR
    classDef humanAction fill:#bbdefb,stroke:#1976d2,stroke-width:2px;
    classDef systemAutomated fill:#e1bee7,stroke:#8e24aa,stroke-width:2px;

    User([User copies Image]):::humanAction --> Watcher[Clipboard Watcher]:::systemAutomated
    Watcher --> Dispatcher[Extraction Dispatcher]:::systemAutomated
    
    subgraph Windows Native OCR Engine
        Dispatcher --> WinRT[WinRT OCR Request]:::systemAutomated
        WinRT --> RawWords[Raw Word/Line Extraction]:::systemAutomated
    end
    
    subgraph Layout Reconstruction logic
        RawWords --> Spatial[Spatial Y-Grouping & Margin Analysis]:::systemAutomated
        Spatial --> TableDetect[Table / Grid Detection]:::systemAutomated
        TableDetect --> Jitter[Jitter Analysis & Confidence Scoring]:::systemAutomated
        Jitter --> Format[Markdown/CSV Formatting]:::systemAutomated
    end
    
    Format --> Store[Save to SQLite/JSON]:::systemAutomated
    Store --> UIUpdate([UI Refreshes History]):::humanAction
```

### Key automated features inside the OCR pipeline:
*   **Spatial Margin Analysis:** Dynamically reconstructs paragraphs and indentations by analyzing vertical gaps.
*   **Table Detection:** Automatically detects column boundaries using X-coordinate clustering to generate valid Markdown tables.
*   **Adaptive Heuristics:** The system identifies the entropy/complexity of an image and dynamically loads specific heuristic profiles via the `HeuristicConfig` without requiring human intervention.

## Configuration Data Flow (GUI-First)

In Phase 55, the architecture shifted to a GUI-First Configuration model. This means that human settings drive system behaviors directly through unified storage, eliminating disconnected external config files.

```mermaid
sequenceDiagram
    participant User as Human (User)
    participant UI as Tauri Frontend
    participant API as Tauri API (api.rs)
    participant Storage as JsonFileStorageAdapter
    participant Engine as OCR Adapters / Services

    User->>UI: Adjusts Extract Heuristic (e.g., Gap Multiplier)
    Note over User,UI: Human Interaction
    UI->>API: IPC update_config(ConfigUpdateDto)
    Note over API,Storage: Automated System Flow
    API->>Storage: save Configuration to file
    Storage-->>API: Success
    API-->>UI: Response
    UI->>User: Displays "Save Successful" feedback
    
    Note over Engine,Storage: Background System Interaction
    Engine->>Storage: Background sync / poll config
    Storage-->>Engine: Returns updated heuristics
    Note left of Engine: Next OCR extraction uses new heuristics
```

### Human vs. System Control
*   **Human Control:** The user utilizes the Tauri Frontend (`ConfigTab.tsx`) to set bounds, choose OCR sensitivity, toggle adaptive tuning, and select target directories for output.
*   **System Action:** `JsonFileStorageAdapter` persists these selections. Internal adapters (`WindowsNativeOcrAdapter`, `CorpusGenerator`) are injected with (or pull) these configurations at runtime to automatically shape their behavior during background processing.
