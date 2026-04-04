# KMS (Knowledge Management System): UX Feature Gaps and Opportunities

**Purpose:** Standalone reference from a read-through of the current KMS implementation (frontend and high-level backend surface). It lists **what already exists**, **concrete gaps** visible in code, and **backlog features** not yet built. Shipped items in §3 are mirrored in §1–2 so the inventory stays current.

**Scope:** DigiCore Tauri app KMS: `KmsApp`, vault explorer, editor, semantic/hybrid search, global and local knowledge graphs, skills, diagnostics, vault settings. Not a full product roadmap; use this to prioritize UX work.

**Date:** April 2026

---

## 1. Current capabilities (inventory)

These are **present** in the codebase in a user-facing or near-user-facing form.

### 1.1 Shell and navigation

- **KMS window** with sidebar views: Explorer (tree quick filter), Search, Favorites (manual order, drag), Recents (last opened), Skill Hub, Operational Logs, Knowledge Graph (2D/3D toggle).
- **Recents** and **manual Favorites order** persist in SQLite (`kms_ui_state` via `kms_get/set_recent_note_paths`, `kms_get/set_favorite_path_order`); starred state remains on `kms_notes.is_favorite`. Legacy `localStorage` lists migrate once on first KMS init after upgrade.
- **Vault path** display, **Vault Settings** modal, **repair index**, theme / zen mode, **reindex** control with sync status and **provider progress** badge (ETA-oriented events).
- **Structured IPC errors** surfaced via shared formatting (`formatIpcOrRaw` / `ipcError` patterns).

### 1.2 Vault explorer (`FileExplorer.tsx`)

- Hierarchical tree from `kms_get_vault_structure`.
- **Create** note/folder, **rename** note/folder, **delete**, **drag-and-drop move** into folders (`kms_move_item`).
- Highlights **active note**; **quick filter** above the tree (name / path); **tag filter** (indexed YAML tags, comma/space tokens); **bulk select** mode with move/delete; **template gallery** (toolbar icon) with optional **dated path** (`notes/daily/YYYY-MM-DD.md`).
- **Open as reference** (note context menu): loads a note into the editor’s **read-only reference pane** without switching the active note (see §1.3).

### 1.3 Editor (`KmsEditor.tsx`)

- **WYSIWYG** (TipTap) and **source** (CodeMirror) modes; markdown round-trip.
- **Wiki links**, **Mermaid**, **math**, **admonitions**, **frontmatter**, **images** resolved to vault-relative paths (`convertFileSrc`).
- **Sidebar tabs:** backlinks (`kms_get_note_links`), **similar notes** (semantic search on content), **TOC**, **local 3D graph** (`KmsLocalGraph3D`).
- **Smart template** flow via `KmsSmartTemplateModal` and `kms_evaluate_placeholders` (interactive vars).
- **Save**, **delete**, **rename**, **history** entry point, zen/theme toggles, **Open global Knowledge Graph** (toolbar): switches to graph view and zooms to the active note when it appears in the current graph payload (paged graphs may show a toast and apply a filename filter if the note is off-page).
- **Split view / reference note:** optional **second read-only pane** (`KmsReferenceReadOnly`) to the right of the main editor. **Ctrl+click** / **Cmd+click** a `[[wiki link]]` opens the target in the reference pane; plain click still switches the active note. **Resize** + width persistence (`kms-reference-pane-width-v1` in `localStorage`). Hidden in **Zen** mode. Shared preprocessing: `lib/kmsEditorPrepareMarkdown.ts`; wiki resolution: `lib/kmsWikiResolve.ts`.

### 1.4 Search (`KmsApp` search view) — mirrors §3 P1 item **Search filters**

- **Query:** `handleSearch` → `getTaurpc().kms_search_semantic` (mode from UI: **Hybrid** / **Semantic** / **Keyword**); result limit from app defaults (`kms_search_default_limit`, `kms_search_default_mode` in `AppStateDto` / config).
- **Entity types in results:** **notes** (`entity_type: "note"`), **snippets**, **clipboard** (including **image** modality and paths that open in the image viewer where applicable).
- **Cancel:** in-flight search via **AbortController** (`Cancel Search` in the loading state).
- **Scope and filters** (collapsible panel **Scope and filters**, labeled **Client-side** in the UI): implemented in `KmsApp.tsx` with state `searchClientFilters` + date inputs; filtering via `filterSearchResults` in `lib/kmsSearchResultFilter.ts` (`KmsSearchClientFilters` type).
  - **Path contains:** case-insensitive substring on normalized `entity_id` (vault path); empty = no path filter.
  - **Modified from / to:** `<input type="date" />` values converted to day indices; applied only to **`note`** rows using indexed **`last_modified`** from `kms_list_notes` (see helper text in UI: vault index mtime, not raw filesystem for every edge case).
  - **Notes** dropdown: **All note files** (`noteScope: "all"`), **Hide skill files** (`standard_only`, paths not under `/skills/`), **Skills only** (`skills_only`).
  - **Checkboxes:** include **Notes**, **Snippets**, **Clipboard**, **Images** (when off, drops image modality and common image extensions on note paths; clipboard image rows respect modality).
  - **Reset filters** restores `defaultKmsSearchClientFilters()` and clears date inputs.
- **Post-filter UX:** if the server returns hits but none pass filters, a dedicated empty state explains **N hit(s) from search, but none match your filters**.
- **Embedding diagnostics banner** (when **Include embedding diagnostics in KMS search** is enabled in app config → `kms_search_include_embedding_diagnostics`): surfaces **query embedding ms** and **effective embedding model id** from result payloads (`searchEmbeddingDiagFromResults`); otherwise a hint points users to Config to enable it.
- **Navigation:** clicking a note result selects the note; snippet/clipboard flows open the existing **ViewFull** / viewer modals as implemented in `KmsApp.tsx`.

### 1.5 Global knowledge graph (`KmsGraph.tsx`, `KmsGraph3D.tsx`) — graph surface + §3 P1 **Open in global graph**

- **View toggle:** user switches **2D** / **3D** in the graph toolbar (`KmsApp` `graphMode`); both views receive the same navigation and paging props.
- **Data:** primary load via **`kms_get_graph(offset, limit, …)`** with **auto-paging** thresholds from `AppStateDto` (`kms_graph_auto_paging_enabled`, `kms_graph_auto_paging_note_threshold`) when the vault is large; optional **temporal** window and other parameters per backend graph service.
- **Navigate from editor (P1 shipped):** **Open global Knowledge Graph** in `KmsEditor` increments a token and sets **`graphNavigateRequest: { token, path }`** on `KmsApp`; view switches to **graph**. `KmsGraph` / `KmsGraph3D` watch `graphNavigateRequest`, zoom/focus the node’s **island** when present in the current page; if the note is **off-page** in a paged graph, UX includes a **toast** and applying a **filename / legend filter** so the user can locate the note (see component logic around `graphNavigateRequest`).
- **Shortest path:** **`kms_get_graph_shortest_path(pathFrom, pathTo)`** with UI to pick endpoints and show path in-graph.
- **Hover / preview:** e.g. **`kms_get_note_graph_preview`** for excerpt-style preview on hover (used from graph components).
- **Tools dock:** graph toolbar can toggle a **tools** region (legend, search, shortest path, etc.); shortcut referenced in UI (**Ctrl+Shift+G** pattern) aligns with toggling that dock.
- **Exports / diagnostics:** graph-related JSON exports, **GraphML**, wiki-links export, and ties to **`kms_get_diagnostics`** where the dashboard surfaces build/request metadata.
- **Rendering / UX:** **warnings** normalization, **request_id** / build timing, **folder** coloring, **edge** toggles (wiki / AI beam / semantic kNN), **island** detection and legend, **filter query**, optional **tag filter** in the tools dock (uses indexed note tags from `kms_list_notes`), **recency pulse**, optional **constellation** backdrop (`KmsGraphConstellationBackdrop`), **web worker** force layout (`kmsGraphForceLayout.worker.ts` / `runKmsGraphForceLayoutWorker`) for large graphs.
- **Config overlap:** global **`AppStateDto`** graph fields and **per-vault graph overrides** JSON (see `VaultSettingsModal` + §1.8) merge on the backend (Rust module `kms_graph_effective_params`, e.g. `effective_graph_build_params` for graph IPC/export) so the UI reflects vault-specific tuning where implemented.
- **Session helpers:** `lib/kmsGraph*` modules (paging, legend prefs, warnings, islands, bloom, etc.) persist or derive UI state without replacing server authority.

### 1.6 Skills

- **Skill Hub** list, **Skill Editor**, sync and **conflict checks**, resources; backed by KMS skills IPC.

### 1.7 Operations and health

- **Operational logs** viewer (`KmsLogViewer`).
- **Health dashboard** (`KmsHealthDashboard`): diagnostics counts, git **prune**, graph-related **exports**, refresh; ties into structured errors.

### 1.8 Vault settings (`VaultSettingsModal.tsx`)

- **Vault path** change with optional **migrate**; **per-vault graph overrides** JSON patch workflow alongside global `AppStateDto` graph-related fields.

### 1.9 Versioning

- **Git history** browser and **restore** (`KmsHistoryBrowser`, `kms_get_history` / `kms_restore_version`).
- **Review / diff** before restore (`KmsVersionDiffModal`, `kms_get_note_revision_content`); working copy from editor ref for comparison.

### 1.10 Data model hints

- `KmsNoteDto` includes **`is_favorite`**; Favorites list, editor star, and FileExplorer context menu call `kms_set_note_favorite` (see section 2).

---

## 2. Evidence-based gaps (implementation vs. promise)

| Area | What users see / expect | What code does |
|------|-------------------------|----------------|
| **Favorites** | A list of starred notes | **Favorites** view, **editor** toolbar star, and **FileExplorer** row menu (add/remove favorite) all use `kms_set_note_favorite`. |
| **Explorer discovery** | Find a note quickly inside a large tree | **Quick filter** above the tree (name / path). No full-text filter inside the tree beyond that. |
| **Recents** | “Continue where I left off” | **Recents** view lists last **opened** notes (order in SQLite), not strictly by DB `last_modified`. |
| **Multi-note workflows** | Compare two notes, reference while writing | **Shipped:** **Reference pane** (read-only second column) + **Ctrl/Cmd+click** wiki links and **Open as reference** in Explorer. Active note remains editable; reference clears if you navigate to the same path as the active note. |
| **History UX** | Understand changes before restore | **Shipped:** **Review** opens compare modal; **Restore** runs from the modal after explicit confirm. |
| **Tags / taxonomy** | Filter and browse by topic | No first-class **tags** UI (filters, tag picker, graph coloring by tag). Frontmatter may allow manual YAML but no guided UX. |
| **Search scoping** | “Only in this folder” or date range | **Path contains**, **modified** range (notes, index mtime), **entity** toggles, **skills** scope, **images** toggle (all client-side post-filter). Server query stays global. |
| **Templates** | One-click “Daily note”, “Meeting” | **Smart template** (placeholders) remains in the editor. **Shipped:** **Template gallery** modal (`KmsTemplateGalleryModal`, `lib/kmsTemplateGallery.ts`) with curated bodies and optional **dated path** for daily notes. |
| **Attachments** | Manage non-markdown assets | Images work via **markdown paths**. **Shipped:** **Attachments** tray under the editor (`KmsAssetTray`) listing vault media via `kms_list_vault_media`; **insert** at cursor; **unused** heuristic via `kms_list_unused_vault_media`. |
| **Bulk file ops** | Reorganize many notes | **Shipped:** Explorer **Bulk select** with checkboxes, **move** and **delete** with confirmation. |
| **Graph entry** | Open graph already focused on current note | **Shipped:** editor toolbar **global graph** control. Local graph remains in **editor sidebar**; Explorer has no dedicated shortcut (optional follow-up). |
| **Keyboard / power users** | Fast navigation without mouse | **Shipped:** **KMS command palette** (`KmsCommandPalette`, **Ctrl/Cmd+K** outside text fields): open note, jump views, **reindex vault**. |
| **Export / share** | Publish or send one note | No **export note** to HTML/PDF/Markdown bundle from KMS UI (aside from graph/vault-level exports). |
| **Collaboration** | Merge or conflict resolution | Git-backed history; **no merge UI** or conflict markers for multi-writer vaults. |

---

## 3. Recommended new features (backlog + shipped)

Prioritized by **user value** and **fit** to the existing architecture (indexed notes, graph, embeddings, vault files). Items marked **Shipped** are implemented; others remain opportunities.

### P0 — High impact, aligns with existing data

1. **Real Favorites experience**  
   - **Shipped:** `kms_set_note_favorite`, Favorites sidebar list (`notes.filter(is_favorite)`), editor toolbar star, **FileExplorer** context menu add/remove favorite (see `digicore/docs/ticket-kms-note-favorites-ipc-and-ui.md`). Manual drag order persists in **`kms_ui_state`** (`favorite_path_order`).

2. **Recent notes**  
   - **Shipped:** Navigation **Recents** view; order persists in **SQLite** (`kms_ui_state` via `kms_get/set_recent_note_paths`, cap 25). Legacy **`localStorage`** lists migrate once on first KMS init after upgrade. Titles resolved from `kms_list_notes`; prunes when notes disappear from the index.

3. **Explorer quick filter**  
   - **Shipped:** Filter field above the vault tree; `filterVaultStructure` prunes nodes by name / `rel_path` (case-insensitive); matching folders keep full subtree; folders on matching paths auto-expand while filtering.

### P1 — Strong UX wins; moderate scope

4. **Search filters**  
   - **Shipped (P1):** `Scope and filters` panel on KMS Search: path substring, date range on indexed note `last_modified`, toggles for notes/snippets/clipboard/images, note scope (all / hide skills / skills only).  
   - **Shipped:** Query embedding time and effective model id banner when **Include embedding diagnostics in KMS search** is on in Config (`kms_search_include_embedding_diagnostics`).

5. **Version diff before restore**  
   - **Shipped:** **Review** in `KmsHistoryBrowser` + `KmsVersionDiffModal`; backend `kms_get_note_revision_content` / `KmsGitService::get_file_content_at_revision`.

6. **Split view (reference note)**  
   - **Shipped:** Read-only **reference column** (`KmsReferenceReadOnly.tsx`) beside the main editor; **Ctrl/Cmd+click** `[[wiki links]]` in the active note, or **Explorer → Open as reference**. **Resize** handle + width persistence; **close** via header **X**. State in `KmsApp` (`referenceNote` / `referenceContent`); loads via `kms_load_note`. Wiki targets resolved with `resolveNoteFromWikiTarget` (`kmsWikiResolve.ts`). Not a separate tab strip (column layout only).

7. **“Open in global graph” from editor**  
   - **Shipped:** Toolbar control in `KmsEditor`; `graphNavigateRequest` on `KmsGraph` / `KmsGraph3D` zooms to the note and focuses its island when possible; off-page paged graphs get a destructive toast + legend filename filter.

### P2 — Differentiation / power users

8. **Tag system (lightweight)**  
   - **Shipped:** `tags` in YAML frontmatter parsed at sync (`kms_note_tags.rs`, `tags_json` on `kms_notes`); exposed on `KmsNoteDto`. **Explorer** and **Search** client filters (`kmsTagFilter.ts`, `kmsSearchResultFilter.ts`, `kmsVaultTreeFilter.ts`); optional **graph** tag line in **KmsGraph** / **KmsGraph3D** legend dock (indexed notes passed from `KmsApp`).  

9. **Template gallery**  
   - **Shipped:** `KmsTemplateGalleryModal` + `lib/kmsTemplateGallery.ts`; optional **dated path** `notes/daily/YYYY-MM-DD.md` for the daily template.

10. **Attachments / asset tray**  
    - **Shipped:** `kms_list_vault_media`, `kms_list_unused_vault_media` in `kms_notes_vault_ipc_service.rs`; UI `KmsAssetTray.tsx`; insert via `pendingMarkdownInsert` on `KmsEditor`.

11. **Bulk select in explorer**  
    - **Shipped:** `FileExplorer` checkboxes when **Bulk select** is on in `KmsApp`; move (single destination folder prompt) and delete (batch confirm).

12. **KMS command palette**  
    - **Shipped:** `KmsCommandPalette.tsx`; **Ctrl/Cmd+K** (ignores when focus is in input/textarea/contenteditable); note open + view jumps + reindex.

### P3 — Broader or strategic

13. **Single-note export** (HTML/PDF)  
    - Uses existing rendered markdown pipeline if available; otherwise markdown zip.

14. **Unlinked mentions**  
    - Scan note bodies for `[[Title]]`-like text that has no target file; list in sidebar (wiki garden pattern).

15. **Named graph presets**  
    - Save legend/filter/paging state as named profiles (export/import JSON next to existing vault overrides).

16. **External sync awareness**  
    - If vault is under Git or cloud sync, optional banner when **file changed on disk** (requires reliable watcher → UI event; verify backend behavior before promising).

---

## 4. What not to duplicate

- **Graph scale guardrails**, **island** UX, **IPC error** shaping, and **reindex progress** are already invested areas; new work should **extend** them rather than replace.  
- **Per-vault graph overrides** and **global AppState** graph fields already overlap; any new “preset” feature should **compose** with that model.  
- For **bindings / types**, the app uses `bindings.ts` with a full `Router`; codegen `bindings_new.ts` stays a **diff source** (see comment block in `bindings.ts`).

---

## 5. Suggested next step (for you)

P0, **P1**, and the **P2** items in §3 (tags, template gallery, attachments tray, bulk explorer actions, command palette) are **shipped** as documented in §1–3. Reasonable next bets: **P3** items, tag **editing** UX, or a keyboard shortcut map surfaced in-app.

---

## 6. File reference (for maintainers)

| Topic | Primary locations |
|-------|-------------------|
| KMS shell | `tauri-app/src/KmsApp.tsx` |
| Explorer | `tauri-app/src/components/kms/FileExplorer.tsx` (tree filter, bulk select, tags on rows from `kms_list_notes`) |
| Template gallery | `tauri-app/src/components/kms/KmsTemplateGalleryModal.tsx`, `tauri-app/src/lib/kmsTemplateGallery.ts` |
| Attachments tray | `tauri-app/src/components/kms/KmsAssetTray.tsx`; IPC `kms_list_vault_media`, `kms_list_unused_vault_media` |
| Command palette | `tauri-app/src/components/kms/KmsCommandPalette.tsx` (Ctrl/Cmd+K in `KmsApp.tsx`) |
| Tag helpers | `tauri-app/src/lib/kmsTagFilter.ts`; Rust `tauri-app/src-tauri/src/kms_note_tags.rs` |
| Editor | `tauri-app/src/components/kms/KmsEditor.tsx` |
| Reference pane (split view) | `tauri-app/src/components/kms/KmsReferenceReadOnly.tsx`, `tauri-app/src/lib/kmsEditorPrepareMarkdown.ts`, `tauri-app/src/lib/kmsWikiResolve.ts`, `tauri-app/src/components/kms/WikiLinkExtension.tsx`; reference state and IPC loads in `KmsApp.tsx` |
| Graph 2D | `tauri-app/src/components/kms/KmsGraph.tsx` (`kms_get_graph`, shortest path, `graphNavigateRequest`, tools dock event `kms-graph-toggle-tools-dock`) |
| Graph 3D / local | `tauri-app/src/components/kms/KmsGraph3D.tsx` (same IPC/nav pattern as 2D); `KmsLocalGraph3D.tsx` (editor sidebar; `kms-local-graph-toggle-tools-dock`) |
| Search + client filters | `KmsApp.tsx` (search view, `searchClientFilters`, date inputs, embedding diag banner); `lib/kmsSearchResultFilter.ts`; IPC `kms_search_semantic`; config flag `kms_search_include_embedding_diagnostics` |
| Health | `tauri-app/src/components/kms/KmsHealthDashboard.tsx` |
| Vault settings | `tauri-app/src/components/modals/VaultSettingsModal.tsx` |
| IPC / types | `tauri-app/src-tauri/src/api.rs`, `taurpc_ipc_types.rs`, `bindings.ts` |
| Note favorites field | `taurpc_ipc_types.rs` (`KmsNoteDto`), `kms_repository.rs`, `kms_notes_vault_ipc_service.rs`, `KmsApp.tsx`, `KmsEditor.tsx`, `FileExplorer.tsx` |
| Recents / favorite order persistence | `kms_repository.rs` (`kms_ui_state`), `kms_notes_vault_ipc_service.rs`, `lib/kmsSidebarStateDb.ts`, `KmsApp.tsx` |
| History diff + Git blob | `kms_git_service.rs`, `kms_git_history_ipc_service.rs`, `KmsHistoryBrowser.tsx`, `KmsVersionDiffModal.tsx`, `lib/kmsVaultRelPath.ts` |

---

*End of document.*
