# Technical & Architectural Document: DigiCore Knowledge Management Suite (Notes & Media)

## 1. Executive Summary
The DigiCore Knowledge Management Suite (KMS) is a privacy-first, local-only sub-application. It provides advanced note-taking, media organization, and knowledge graphing, utilizing the core DigiCore database (`digicore.db`) as its single source of truth for metadata while maintaining human-readable Markdown files for content portability.

## 2. Future-Proofing & Foundational Decisions
To ensure long-term scalability and parity with platforms like Notion or Obsidian, the following foundational architectures are established:

### A. Block-Based Content Model (The "Notion" Foundation)
- **Concept**: While notes are stored as `.md` files, the internal representation during editing and in the index reflects a **Block-based** model.
- **Why**: Allows for advanced features like inline Kanban boards, multi-column layouts, and nested interactive widgets (e.g., a "Snippet Block" directly in a note).
- **Implementation**: TipTap's JSON schema will be the canonical "live" format, serialized to Markdown for storage.

### B. Semantic Search & Vector Embeddings (The "AI" Foundation)
- **Concept**: Beyond keyword search (FTS5), KMS will support **Semantic Search** using local vectors.
- **Technology**: Integration of `sqlite-vec` (compact vector extension for SQLite) into `digicore.db`.
- **Capability**: Search across KMS notes, Clipboard history, and Snippets using concepts rather than exact words. This allows for "Show me notes related to my recent image captures."

### C. Unified Indexing Strategy (The "Ecosystem" Foundation)
- **Concept**: A single background service indices all DigiCore artifacts.
- **Cross-Linkage**: An Image in the Image Library can be "linked" to a specific line/block in a Note, and this relationship is stored in the `kms_links` table.

### D. Bookmark & Content Retrieval Service
- **Concept**: A dedicated "Bookmarks" tab that actively retrieves content.
- **Retrieval**: When a URL is bookmarked, a background worker fetches the page content, generates a summary, and creates a "Snapshot Note" in the KMS.
- **Future-Sync**: AI-driven "Related Content" retrieval based on existing notes.

## 3. Core Requirements
### Functional
- **Dual-Mode Editor**: Support for both absolute WYSIWYG and Source Code with Preview.
- **Local-Local Storage**: Data stored exclusively on the user's Windows machine.
- **Bi-directional Linking**: `[[Note Name]]` syntax for connecting thoughts.
- **Plugin System**: Standardized API for "Sub-Apps" (e.g., a separate Bookmark Manager plugin).

### Non-Functional
- **Scalability**: Designed to handle 10,000+ notes and 100,000+ clipboard entries with sub-second latency.
- **Extensibility**: Plugin hooks at the Editor and Database layers.

## 4. Proposed Architecture
### Frontend (React + Vite)
- **Editor**: Hybrid TipTap (WYSIWYG) + CodeMirror (Source).
- **Graph**: Canvas-based D3 graph for performance.

### Backend (Rust + Tauri)
- **Database**: `digicore.db` with `sqlite-vec` and `FTS5` extensions.
- **Watchdog**: Notify-based service for FS-to-DB sync.
- **Plugins**: A registry of Rust functions that can be called by the KMS UI.

## 5. Alternative Approaches & SWOT

### Option 1: Vector-First Hybrid (Recommended)
Add semantic embeddings to the SQLite hybrid approach.
- **Pros**: Future-proof for AI, unified search, conceptually powerful.
- **Cons**: Slightly higher initial development complexity.
- **SWOT**:
  - **S**: Best-in-class capability.
  - **W**: Initial setup of local LLM models for embeddings.
  - **O**: AI-driven auto-tagging.
  - **T**: Performance if models are too large.

### Option 2: Modular Plugin-Based Core
Build a minimal core and move features like "Bookmarks" into separate internal plugins.
- **Pros**: Low "bloat", highly customizable.
- **Cons**: Requires standardizing the internal API first.
- **SWOT**:
  - **S**: Flexibility.
  - **W**: API overhead.
  - **O**: Third-party plugin community.
  - **T**: Fragmented user experience.

| Decision Point | Selection | Rationale |
| :--- | :--- | :--- |
| **Search** | **Vector + FTS5** | Semantic search is the key differentiator for "AI notes". |
| **Data Model** | **Block-Aware Markdown** | Parity with modern apps without losing text file portability. |
| **Plugins** | **Internal Plugin Registry** | Clear separation between KMS core and specialized tools like Bookmarks. |

## 6. Key Decisions Required
1. **Embedding Model**: Which local model to use for generating vectors? (*Recommendation: `all-MiniLM-L6-v2` - tiny and fast on CPU.*)
2. **Bookmark Strategy**: Headless browser extraction vs. simple Meta-tag scraping.
3. **Graph Type**: 2D Force Graph vs. 3D Visualization.

## 7. Development Roadmap
- **Phase 1**: Vault Initialization & Hybrid Editor.
- **Phase 2**: Vector Search & FTS Integration.
- **Phase 3**: Link Graph & Bi-directional linking.
- **Phase 4**: Bookmark & Retrieval Plugin.
