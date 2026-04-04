# KMS graph PRD + audit plan — progress tracker

> Doc governance status: Mirror (status/progress mirror only)
> Canonical sources: `knowledge-graph-comprehensive-audit-and-roadmap-2026-03.md`, `kms-notebook-capabilities-audit-and-implementation-plan-2026-04.md`
> Governance map: `kms-graph-doc-governance-map-2026-04.md`

**Last updated:** 2026-03-28 (align with `knowledge-graph-comprehensive-audit-and-roadmap-2026-03.md` v1.15 + §8.3 mirror refresh)

Use this checklist with the Cursor todo list. IDs match `prd-`* / `audit-*` / `bundle-*` / `roadmap-*` tasks.

## Done (shipped in repo)

- Epic A: `kms_graph_auto_paging_enabled` / `kms_graph_auto_paging_note_threshold`, session keys (`kms_graph_session_*`), 2D/3D paged fetch + Prev/Next/Full graph, pathfinding + hover preview off in paged mode.
- Epic A (follow-up): Page size presets in 2D/3D pagination bar (`PAGE_SIZE_PRESETS`, `kms_graph_session_paged_limit`), path-sort `title` on page label, empty-page overlay when paged fetch returns 0 nodes, Config `title` hints on paged-graph block.
- Settings bundle: export `schema_version` **1.1.0** (`SETTINGS_BUNDLE_SCHEMA_V1_1`); import/preview accept **1.0.0** and **1.1.0** (`settings_bundle_schema_supported`).
- Epic B: `kms_graph_vault_overrides_json`, merge in `kms_get_graph`, Vault Settings JSON editor, bundle export/import `kms_graph_vault_overrides`.
- DB repair: `kms_notes.sync_status` / `last_error` idempotent `ALTER` in `kms_repository::init`.
- Audit doc: `knowledge-graph-features-audit-and-implementation-plan.md` updated (per-vault overrides, Phase 6 row, decision log).
- Audit: structured IPC — Rust `ipc_error` codes on graph path / preview / vault overrides / repo errors; TS `KMS_IPC_CODES`, `parseIpcErrorFromUnknown`, `formatIpcOrRaw` includes optional `details`; graph pathfinder + Vault overrides toast use formatted errors.
- Audit: graph diagnostics in UI — `KmsGraphDto.build_time_ms` (**u32** ms; Specta/taurpc cannot export `u64` on IPC DTOs); legend in 2D/3D shows build ms + `kms_get_diagnostics()` vault counts when the graph view is visible.
- Epic B (follow-up): `kms_clear_vault_graph_overrides_json` RPC; Vault Settings form mirroring Config graph tunables (not auto-paging), JSON preview, **Remove saved overrides**, **Reset form (inherit all)**.
- Roadmap 3.0 (centrality): undirected wiki-link **PageRank** in `kms_graph_service::undirected_pagerank`; `BuiltGraphNode.link_centrality` + `KmsNodeDto.link_centrality`; full graph + local subgraph; 2D/3D node sizing blends degree with `link_centrality`.
- Roadmap 3.0 (folder palette + legend): `kmsGraphFolderPalette.ts` (deterministic folder colors), `kmsGraphLegendPrefs.ts` (session: color mode, edge visibility, legend section toggles); **2D** `KmsGraph.tsx` and **3D** `KmsGraph3D.tsx` support **By type / By folder** coloring, optional legend rows, wiki/AI beam (and 3D cluster) edge toggles; **local** `KmsLocalGraph3D.tsx` shares color mode + folder coloring (focus note stays white).
- Roadmap 3.0 (pulse + icons + legend search): `kmsGraphPulse.ts` (recency vs graph date range, top-% pulse), `kmsGraphNodeIcons.ts` (2D SVG paths + 3D shape per `node_type`), `kmsGraphGraphFilter.ts` + extended `kmsGraphLegendPrefs` (pulse + legend query); **2D/3D** and **local 3D** show teal pulse on recently edited nodes (optional), type-based 3D meshes, legend search that narrows visible nodes/links with counts when filtered.
- Roadmap 3.0 (island legend follow-up): `kmsGraphLegendVisibility.ts` + `kmsGraphIslands.ts` + hidden folder/type session keys; per-legend-row visibility, connected-component island UI, dim/hide focus, 3D frame; **2D/3D/local 3D** (see `roadmap-6-islands` row below).

## Pending (ordered roughly by priority)

Follow **`knowledge-graph-comprehensive-audit-and-roadmap-2026-03.md` [section 8.2 Prioritized next actions](knowledge-graph-comprehensive-audit-and-roadmap-2026-03.md#82-prioritized-next-actions-after-seq-11--12--13-closure)** and **section 8.3** (embedding / search mirror; authoritative copy in the roadmap). Optional UI polish remains in [`kms_graph_3.0_roadmap.md`](./kms_graph_3.0_roadmap.md) and [`kms-graph-island-legend-followup-scope.md`](./kms-graph-island-legend-followup-scope.md).

**Section 8.3 mirror** (keep in sync with [roadmap 8.3](knowledge-graph-comprehensive-audit-and-roadmap-2026-03.md#83-embeddings-semantic-search-and-seq-12-mirror-of-companion-doc); detail in [`kms-embeddings-temporal-graph-semantic-search-hexagonal-plan-2026-03.md`](./kms-embeddings-temporal-graph-semantic-search-hexagonal-plan-2026-03.md)):

| Theme | Status (2026-03-30) |
|-------|---------------------|
| **O3 pipeline** | **Shipped:** normalize, chunk mean-pool, ports in **`digicore-kms-ports`**, note sync + query path |
| **Multi-model fastembed** | **Shipped:** per **`kms_embedding_model_id`** (`EmbeddingModel::from_str`) |
| **D5 search floor** | **Shipped:** global + per-vault **`kms_search_min_similarity`** |
| **D6 migrate** | **Shipped:** background job + events + Config log / re-embed button |
| **D7 PR on settings** | **Shipped:** debounced wiki PageRank materialize on pagerank-affecting config changes |
| **D8 Config** | **Shipped:** KMS Search & embeddings sibling area |
| **Search diagnostics** | **Shipped:** optional via **`kms_search_include_embedding_diagnostics`**; **`SearchResultDto`** **Option** fields when off |
| **Local graph embeddings** | **Shipped:** **`get_note_embeddings_for_paths`** (scoped) + port **`load_note_embeddings_for_paths`** |
| **Seq 12 temporal** | **Shipped (MVP):** A + B + IPC + flags; **not** Option C |
| **Text search dimension guard** | **Shipped:** hybrid text modality rejects query vectors whose length is not **384** (matches stored **`vec0`**) |
| **Default search mode/limit** | **Shipped:** **`kms_search_default_mode`**, **`kms_search_default_limit`** (AppState, Config, KMS Explorer init + search) |
| **Open (embedding track)** | **`embedding_model_version`** / DB column for stored dims beyond the text query guard; auto re-embed on chunk-only change; richer D6 UX |

**Recently shipped (not duplicated in Done above):** full graph **DTO JSON** export (`kms_export_graph_dto_json`, KMS Health); Leiden MVP + local/full BFS undirected dedup parity tests (see comprehensive audit **8.1** seq 11 / 13).

### Roadmap vision breakdown (`digicore/docs/kms_graph_3.0_roadmap.md`)


| Theme        | Vision item                                            | Status in repo (2026-03)                                                                |
| ------------ | ------------------------------------------------------ | --------------------------------------------------------------------------------------- |
| Intelligence | AI / embedding clustering (“topic continents”)         | Partial: k-means + semantic beams + cluster labels; optional **Leiden** on wiki + kNN graph (flag). |
| Intelligence | Node centrality / **PageRank** sizing                  | Shipped: `link_centrality` on DTO; 2D/3D blend with wiki degree.                         |
| Intelligence | Multi-color by **folder path**                         | Shipped: folder palette + By folder mode (2D / 3D / local 3D).                          |
| UI           | **3D** exploration / “fly through”                     | Shipped: `KmsGraph3D.tsx` + ForceGraph3D camera fly on click.                           |
| UI           | Glassmorphic **hover previews**                        | Partial: 2D/3D preview RPC; styling not full “glassmorphic” spec.                       |
| UI           | **Interactive legend** (folders, date, search islands) | Partial: search + pulse prefs + timeline; deeper island UX may extend later.            |
| UI           | **Pulse** on recently edited notes                     | Shipped: top-% recency pulse (2D ring + 3D wireframe animation).                        |
| Functional   | **Local graph** view                                   | Shipped: local subgraph + `KmsLocalGraph3D`.                                            |
| Functional   | **Temporal** playback                                  | Shipped: time slider + Play in 2D/3D graphs.                                            |
| Functional   | **Pathfinding** two nodes                              | Shipped: BFS + highlight (disabled in paged view).                                      |
| Foundations  | DTO: node types, `last_modified`, folder paths         | Shipped: `KmsNodeDto` fields.                                                           |
| Foundations  | **Custom icons** by content type                       | Shipped: 2D per-type SVG paths + 3D shape (sphere/cone/box/octahedron) in main/local 3D. |


**Suggested sequencing (historical):** (1)-(6) shipped (including island legend follow-up); optional polish in [`kms-graph-island-legend-followup-scope.md`](./kms-graph-island-legend-followup-scope.md).

## Non-goals (unchanged)

- Spatial graph streaming; path sort by PageRank; structure-first IPC without product decision.

