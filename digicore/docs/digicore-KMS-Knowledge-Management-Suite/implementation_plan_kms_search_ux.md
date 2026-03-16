# Phase 89: KMS Search & Discovery UX Enhancement

Enhance the KMS search experience by providing contextual text snippets for semantic and hybrid search results, and improving the discovery UX within the sidebar.

## Proposed Changes

### [Component] Backend Search API

#### [MODIFY] [api.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/api.rs)
- Update `kms_search_semantic` to include a `snippet` field in the response metadata.
- Implement a helper to extract ~200 characters of text around the most relevant chunk from the note's content.
- Support basic term highlighting (wrapping matches in `==` or similar) for keyword-based search.

#### [MODIFY] [bindings.ts](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/bindings.ts)
- Update `SearchResultDto` to include `snippet: string | null`.

### [Component] Frontend Search View

#### [MODIFY] [KmsApp.tsx](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/KmsApp.tsx)
- Update the search result list to display the new `snippet` field.
- Add a "Quick Peek" feature where hovering over a result shows a slightly larger preview.
- Improve the "Knowledge Search" input design with clearer loading states and result counts.

---

## Future Roadmap (Phases 90+)
- **Phase 90: Metadata & Tagging**: Full support for `#tags` and frontmatter.
- **Phase 91: Graph Visualization**: Visual map of connections.
- **Phase 92: Image & PDF Search**: Indexing text within attachments.

## Verification Plan

### Automated Tests
- **Unit Test**: Add a test in `kms_service.rs` (or `api.rs`) to verify that the snippet extraction logic correctly finds text around a query match.

### Manual Verification
1.  Perform a semantic search for a specific topic (e.g., "minimax algorithm").
2.  Verify that the search results show a relevant sentence or paragraph from the note.
3.  Click a result and verify it opens the correct note.
