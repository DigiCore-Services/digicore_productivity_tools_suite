# KMS graph: Island behaviors & richer legend (follow-up scope)

**Status:** Phases A-C + light Phase D polish **implemented** in app (2026-03); this doc remains the spec reference.  
**Source vision:** `digicore/docs/kms_graph_3.0_roadmap.md` -- *Interactive Legend*: toggle folder visibility, filter by date, search for specific **"islands"** of notes.

This document defines **island** for product purposes, what is already done, and a **separate** follow-up scope so work can be estimated and sequenced without mixing it with shipped pulse / icons / text search.

---

## 1. Terms

| Term | Meaning in this scope |
|------|------------------------|
| **Island** | A **weakly connected component** in the graph **after** all active filters are applied (timeline, edge-type toggles, text search, and any future folder visibility). Nodes in different islands have **no path** through currently visible wiki/AI edges. |
| **Legend island** | UI that surfaces **which islands exist**, how many nodes each has, and optional actions (focus, dim others, reset). |
| **Folder slice** | The set of nodes whose `folder_path` maps to the same legend folder key (palette row). Not always equal to an island (one folder can span multiple components if links do not connect them). |

---

## 2. Baseline (already in repo)

- **Timeline / date:** Time slider + play (full + local 3D).
- **Text filter:** Title / path / folder substring search; visible node and link sets shrink accordingly (`kmsGraphGraphFilter.ts`, `vizData`).
- **Edge toggles:** Wiki vs AI beam (and 3D cluster edges where applicable).
- **Folder coloring:** By-folder mode + deterministic palette; legend can **show** folder rows (swatch + label) but rows are **informational only** -- they do **not** hide/show nodes yet.
- **Pulse / icons:** Recency pulse and type-based 2D icons / 3D shapes.

**Gap vs roadmap wording:** *"toggle visibility of folders"* is **not** fully met until per-folder (or per-folder-key) visibility affects the rendered graph.

---

## 3. Proposed follow-up scope (phased)

Work can ship in slices; later phases depend on earlier ones only where noted.

### Phase A -- Legend-driven visibility (folders & types)

**Goal:** The legend becomes an **active filter surface**, not only a key.

- **A1. Per-folder visibility (folder color mode)**  
  - Each folder legend row gets a checkbox (or click-to-cycle: show / dim / hide).  
  - Hidden folders: nodes excluded from `vizData` (and incident edges dropped).  
  - Persist in session (extend `kmsGraphLegendPrefs.ts` with a structure such as `folderVisibility: Record<folderKey, "show" | "dim" | "hide">` or a compact allowlist).

- **A2. Optional: per-type visibility (type color mode)**  
  - Same pattern for note / skill / image / asset when `colorMode === "type"`.  
  - Reuse one prefs pattern for consistency.

- **A3. "Reset visibility"**  
  - One control to clear folder/type overrides and match "show all" defaults.

**Acceptance:** Toggling legend rows changes the graph immediately in **2D and 3D**; local 3D uses the same prefs where applicable.

---

### Phase B -- Island detection & legend panel

**Goal:** After all filters (including Phase A), users see **structure**, not just a flat count.

- **B1. Compute connected components**  
  - On the current visible node set and visible undirected edge set, compute weakly connected components (standard Union-Find or BFS).  
  - O(nodes + edges) per filter change; acceptable for typical vault subgraph sizes; document a soft cap or debounce if needed.

- **B2. Legend / panel: island summary**  
  - Show **island count** and optional list: e.g. "3 islands | 12 / 5 / 2 nodes".  
  - When text search is active, copy can read: "Search matches span N islands" to reinforce mental model.

- **B3. Optional: largest island first**  
  - Sort island list by size; default selection or highlight the largest component.

**Acceptance:** With a disconnected filtered graph, the UI reports correct island count and sizes; changing folder visibility or search updates the summary.

---

### Phase C -- Island-focused navigation (richer interactions)

**Goal:** *"Search for islands"* becomes **navigate and emphasize** an island, not only filter text.

- **C1. Select island from list**  
  - Clicking an island in the summary **dims** nodes/edges not in that island (preferred) or **hides** them (stronger mode, optional toggle).  
  - Clear selection restores full visible set.

- **C2. "Island containing selection"**  
  - Context action or button: from focused/hovered/selected node, jump to **that** component (same dim/hide behavior).

- **C3. 3D camera assist (main + local 3D)**  
  - Optional: "Frame island" -- adjust camera to bounding sphere of island nodes (approximate); fall back gracefully if layout unstable.

- **C4. Bridge awareness (optional / later)**  
  - If a future feature restores a single hidden folder or edge type and merges two components, show a short hint ("Merged 2 islands") -- only if low effort.

**Acceptance:** User can answer "which cluster am I in?" and "show only this cluster" without retyping search queries.

---

### Phase D -- Polish & glassmorphic alignment (optional)

- Restyle legend + island panel toward roadmap **glassmorphic** hover/legend aesthetic (blur, borders, motion) consistent with hover preview cards.  
- Can be combined with a separate **hover preview** visual pass from the same roadmap.

---

## 4. Out of scope for this follow-up (unless reprioritized)

- **Semantic / embedding "continents"** as islands (that is clustering product logic, not graph-theoretic components).  
- **Backend changes** for islands (all of the above can be client-side on the current DTO).  
- **Persisted** per-vault "saved island views" (session-only is enough for v1).  
- **Spatial graph streaming** (already a non-goal in PRD).

---

## 5. Suggested task IDs (for Cursor / tracking)

| ID | Description |
|----|-------------|
| `roadmap-6-legend-folder-visibility` | Phase A: per-folder (+ optional per-type) visibility + prefs |
| `roadmap-7-island-metrics` | Phase B: component computation + legend summary |
| `roadmap-8-island-navigation` | Phase C: select island, dim/hide, frame in 3D |

---

## 6. Open decisions (product)

1. **Dim vs hide** for non-selected islands: dim keeps context; hide maximizes clarity. Default recommendation: **dim** with optional "hide others".  
2. **Paged graph mode:** Either disable island actions when the graph is incomplete, or show a disclaimer that island counts are **page-local** only.  
3. **Local 3D:** Full parity vs simplified panel (minimum: same prefs; island list can be compact).

---

## 7. References

- `digicore/docs/kms_graph_3.0_roadmap.md` -- Interactive Legend, islands wording.  
- `digicore/docs/kms-graph-prd-progress.md` -- shipped vs pending summary.  
- Implementation touchpoints (current): `KmsGraph.tsx`, `KmsGraph3D.tsx`, `KmsLocalGraph3D.tsx`, `kmsGraphLegendPrefs.ts`, `kmsGraphGraphFilter.ts`.
