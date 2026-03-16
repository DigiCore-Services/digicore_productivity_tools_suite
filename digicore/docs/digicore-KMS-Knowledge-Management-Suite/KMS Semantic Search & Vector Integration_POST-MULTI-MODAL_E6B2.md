# Phase 78: Multi-Modal Semantic Search & Vector Integration

Implement a unified, multi-modal semantic search system that allows users to find Knowledge Base notes (**Text**), Text Expander snippets (**Trigger-based Text**), and Clipboard history items (**Text, Images, and future Media like Audio/Video**) using natural language queries, regex, and logical operators. All processing is local-first for maximum privacy and high-speed retrieval.

## User Review Required

> [!IMPORTANT]
> **Multi-Modal foundation**: This phase now includes image embeddings (CLIP) in addition to text. The first run will trigger model downloads for both **BGE-small** (Text) and **CLIP-ViT-B-32** (Image/Text Alignment). 

> [!CAUTION]
> **Resource Usage**: Processing images locally for embeddings is CPU/RAM intensive. We will implement a throttled background indexing service to ensure system stability.

> [!IMPORTANT]
> **Unified Cross-Component Search**: Results will seamlessly blend Notes, Snippet Triggers, and Clipboard History items of all types (Text/Images).

## Proposed Changes

### [Component Name] Backend (Rust)

#### [MODIFY] [Cargo.toml](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/Cargo.toml)
- Add `fastembed = "3.9"` (local embedding generation).
- Add `sqlite-vec = "0.1"` (vector storage and search).

### Model Selection & Multi-Modal Strategy

We use dual-encoder models that map different modalities into a shared vector space:

| Modality | Model | Size | Dims | Purpose |
|---|---|---|---|---|
| **Text** | `bge-small-en-v1.5` | ~33 MB | 384 | Core note/snippet/clipboard text search. |
| **Image** | `clip-ViT-B-32-vision` | ~300 MB | 512 | Searching for images using text or other images. |
| **Audio/Video** | (Foundation Only) | - | - | Initial schema will support adding Whisper/Frame embeddings. |

> [!TIP]
> By using **CLIP**, you can type "Blue chart with revenue data" and the system will find the specific screenshot from your Clipboard History even if the OCR text was partial.

#### [MODIFY] [lib.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/lib.rs)
- Add Migration 5:
  ```sql
  -- Create a virtual table for vectors (vss0 or vec0 depending on version)
  CREATE VIRTUAL TABLE kms_embeddings USING vec0(
      id INTEGER PRIMARY KEY,
      embedding float[384]
  );
  -- Add a mapping table to link vectors to source entities
  CREATE TABLE kms_embedding_map (
      vec_id INTEGER PRIMARY KEY,
      entity_type TEXT NOT NULL, -- 'note', 'snippet', 'clipboard'
      entity_id TEXT NOT NULL,   -- path or numeric ID
      content_hash TEXT,
      FOREIGN KEY (vec_id) REFERENCES kms_embeddings(id) ON DELETE CASCADE
  );
  ```

#### [NEW] [embedding_service.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/embedding_service.rs)
- Implement `init_model()` to load BGE-small and CLIP.
- Implement `generate_text_embedding(text: &str, metadata: &serde_json::Value)` -> `Vec<f32>`.
- Implement `generate_image_embedding(image_path: &Path, metadata: &serde_json::Value)` -> `Vec<f32>`.
    - **Metadata Enrichment**: Joins Window Title, App Name, and File Name into the embedding context to ensure a "Blue screenshot from Chrome" query works.
- Implement background task for incremental indexing of:
    - **KMS**: Notes and future media attachments.
    - **Snippets**: Triggers and expansion content.
    - **Clipboard**: Text snippets, Screenshots, and Image Library items.
    - **Foundation**: Placeholder logic for Audio/Video metadata extraction.

#### [MODIFY] [kms_repository.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/kms_repository.rs)
- Implement `upsert_embedding(entity_type, entity_id, vector)`.
- Implement `semantic_search(query_vector, limit) -> Vec<SearchResult>`.

#### [MODIFY] [api.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/api.rs)
- Expose `kms_search_semantic(query: String)`.
- Expose `kms_reindex_all()`.

---

### [Component Name] Frontend (React)

#### [MODIFY] [ConfigTab.tsx](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/components/ConfigTab.tsx)
- Add "Semantic Search" sub-tab under KMS.
- Add "Search Logic" help text (explaining regex and `AND`/`OR` operators).
- Add "Reindex Vault" button with progress indicator.

#### [MODIFY] [KmsEditor.tsx](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/components/kms/KmsEditor.tsx)
- **Mandatory**: Add "Similar Notes" sidebar/widget using vector proximity.
- Update UI to highlight semantic matches.

## Verification Plan

### Automated Tests
- Run `cargo test` in `src-tauri` (requires adding unit tests for `embedding_service`).
- Verify vector distance calculations between known strings (e.g., "apple" should be closer to "fruit" than to "car").

### Manual Verification
1. **Multi-Modal Retrieval**: Copy an image of a red car to the clipboard. Type "red vehicle" in KMS search. Verify the image appears in unified results.
2. **Metadata Context**: Search for "Note about marketing in Chrome". Verify results prioritize notes/snippets associated with that window title.
3. **Cross-Component Sync**: Verify a text search finds relevant Trigger Snippets alongside KMS Notes.
4. **Logic/Regex**: Test `*img* AND (cat OR dog)` to verify hybrid `vec0` + `fts5` integration.
