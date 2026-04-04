# PRD stub (optional epics): Paged graph view & per-vault graph overrides

**Status:** Draft / optional  
**Product:** DigiCore KMS Knowledge Graph  
**Audience:** Product, engineering  
**Related:** `knowledge-graph-features-audit-and-implementation-plan.md`, `kms_graph_service.rs` (`kms_get_graph(offset, limit)`, `limit == 0` = full graph)

---

## 1. Problem statements

| Epic | Problem |
|------|---------|
| **Paged graph view** | Large vaults produce heavy IPC payloads and slow or janky 2D/3D rendering. Users need a predictable way to browse the graph in chunks without assuming the whole vault fits one view. |
| **Per-vault graph overrides** | Graph tuning (clustering, beams, caps, warnings) is global. Users with multiple vaults (small vs large, work vs personal) cannot tune behavior per vault without changing settings every time they switch. |

---

## 2. Epic A: Paged graph view

### 2.1 Goal

Let users load the global Knowledge Graph in **path-ordered pages** (backend already slices nodes and filters edges/beams; `pagination` metadata exists), with clear UX so users understand what they are seeing.

### 2.2 User stories

| ID | As a… | I want… | So that… |
|----|---------|---------|----------|
| A1 | KMS user with a large vault | to open the graph in **paged mode** (or have the app suggest it when the vault is large) | I can explore links without freezing the UI |
| A2 | KMS user | to see **which page** I am on and whether **more pages** exist | I know the view is partial |
| A3 | KMS user | to change **page size** (or pick a preset) within reason | I can balance detail vs performance |
| A4 | KMS user | to switch to **full graph** when my vault is small enough | I keep today’s “whole picture” behavior |
| A5 | KMS user in paged graph view | **pathfinding disabled** with a clear notice and a path to **full graph** or **local graph** | I am not confused by partial or missing path highlights |

### 2.3 Configuration (Configurations and Settings)

Paging behavior is **user-configurable** under the existing **Knowledge Graph** sub-tab (same area as other `kms_graph_*` keys), not hardcoded. Use the **exact field names** below in `AppState`, `ConfigUpdateDto`, `storage_keys`, bundle export, and TypeScript bindings.

| Key | Type | Purpose |
|-----|------|---------|
| `kms_graph_auto_paging_enabled` | `bool` | When **true**, opening the global graph uses **paged** mode if indexed note count **≥** `kms_graph_auto_paging_note_threshold`. When **false**, default open is **full graph** (`limit === 0`); optional graph-toolbar control may still switch to paged mode. **Permanent opt-out** from automatic paging-by-size = set to **false**. |
| `kms_graph_auto_paging_note_threshold` | `u32` | Indexed note count at or above which auto-paged mode applies (only when `kms_graph_auto_paging_enabled` is true). Default should align with `kms_graph_warn_note_threshold` unless product sets a different initial value. |

**Client session (not persisted in global JSON app state):** store pagination position in `localStorage` (or equivalent) so leaving and re-entering the graph view does not reset the page. Illustrative keys:

| Key | Type | Purpose |
|-----|------|---------|
| `kms_graph_session_paged_offset` | `u32` | Current page offset passed to `kms_get_graph(offset, limit)`. |
| `kms_graph_session_paged_limit` | `u32` | Current page size (`limit`); `0` means “use full graph” for this session view if user switched manually. |

Global **auto** behavior only affects **initial** mode when entering the graph view; session keys track the user’s current page while they stay in the app.

### 2.4 Acceptance criteria

1. **Config:** `kms_graph_auto_paging_enabled` and `kms_graph_auto_paging_note_threshold` exist in **Configurations and Settings > Knowledge Graph**, load/save with the rest of app config (`update_config` / `get_app_state`), and are documented in tooltip/help text (threshold + permanent opt-out behavior).
2. **Load:** When paged mode is active, the client calls `kms_get_graph(offset, limit)` with `limit > 0` and persists `kms_graph_session_paged_offset` / `kms_graph_session_paged_limit` across the session (see 2.3).
3. **Metadata:** UI displays `pagination.total_nodes`, current range, and `has_more` (or equivalent) when returned by IPC.
4. **Navigation:** User can go to **next** and **previous** page; **invalid** `offset` (e.g. past end) shows an empty graph and a clear message, not a silent failure.
5. **Ordering:** Copy or tooltip states that pages are ordered by **note path** (stable sort), not by relevance or centrality.
6. **Full graph:** User can select **full graph** (`limit === 0`) when offered; behavior matches current production for appropriately sized vaults.
7. **Warnings:** Existing `warnings` (large vault, semantic skipped, etc.) still surface in the same surface as today (toast/banner/graph chrome).
8. **Pathfinding / preview (resolved):** **Shortest-path mode and related path UI are disabled** while the global graph is in **paged** mode. Show a short inline notice (e.g. “Pathfinding is unavailable in paged view; switch to full graph or use local graph.”). Hover preview may follow the same rule or be limited to nodes on the current page—**engineering to match pathfinding** for consistency. A **follow-up epic** may define cross-page pathfinding and re-enable controls.

### 2.5 Non-goals (this epic)

- Spatial “load more as I pan” graph streaming.
- Changing backend sort order to PageRank or relevance (unless explicitly added later).
- Guaranteeing faster **first** IPC if full semantic build still runs on full data (call out in release notes; future epic may add “structure-first” load).

---

## 3. Epic B: Per-vault graph overrides

### 3.1 Goal

Allow **Knowledge Graph** settings (same keys as global config: k-means, beams, caps, toggles, warning thresholds—see implementation plan) to be **overridden per vault**, with predictable precedence and discoverable UI.

### 3.2 User stories

| ID | As a… | I want… | So that… |
|----|---------|---------|----------|
| B1 | User with multiple vaults | **per-vault** graph settings | my small vault can use rich semantics and my huge vault can stay fast |
| B2 | User | to see whether I am using **global defaults** or **vault-specific** values | I can reason about behavior |
| B3 | User | to **reset** vault overrides to global defaults | I can undo experiments |
| B4 | Admin / power user | overrides included in **backup/export** story (if product supports it) | I can move machines without re-tuning |

### 3.3 Acceptance criteria

1. **Precedence:** `effective_value = vault_override[key] ?? global_app_state[key]` for each supported graph key. Leaf keys **must match** existing `kms_graph_*` field names (see **section 7**); omit a key in the per-vault object to inherit global.
2. **Persistence:** Overrides are stored durably and associated with a **stable vault identity**; see **section 6.3** (canonical path, Windows case-insensitivity, single helper for keys).
3. **UI:** Vault Settings (or equivalent per-vault surface) exposes a **Knowledge Graph** subsection: fields mirror global Config where possible; “Use global default” toggle per field or section.
4. **Reset:** “Reset all vault graph overrides” restores global behavior for that vault.
5. **Runtime:** `kms_get_graph` (and any graph RPCs that read these settings) resolve **effective** values for the **currently active vault**.
6. **Migration:** Existing users: **no change** until they set an override; global behavior remains default.
7. **Export/import (resolved):** Per-vault graph overrides are **included in v1** settings bundle export/import (see section 6). Bundle format **version** must increment or include a nested schema so older imports remain valid; import path should **merge** or **replace** per-vault map per explicit product rule (recommend **replace** for named vault keys on restore for predictability).

### 3.4 Non-goals (this epic)

- Per-vault overrides for **non-graph** KMS settings (unless product expands scope explicitly).
- Cloud sync of overrides (local-first unless another product initiative owns it).

---

## 4. Shared assumptions & dependencies

- IPC/bindings already expose `kms_get_graph(offset, limit)` and `pagination` DTO on success; frontend work for Epic A assumes types are aligned.
- Per-vault Epic B depends on a **single authoritative** “current vault” context in the app when graph RPCs run.

---

## 5. Success signals (lightweight)

| Epic | Signal |
|------|--------|
| A | Fewer OOM / long-frame reports on graph tab for large fixtures; qualitative “usable” on N≥2k notes (N chosen by product). |
| B | Multi-vault testers can set different beam caps without touching global Config each time; support tickets about “wrong graph behavior after vault switch” decrease. |

---

## 6. Resolved product decisions

### 6.1 Epic A: Full graph vs auto-paged (threshold + opt-out)

**Decision:** **Configurable** in **Configurations and Settings**, **Knowledge Graph** sub-tab using keys **`kms_graph_auto_paging_enabled`** and **`kms_graph_auto_paging_note_threshold`** (see **2.3**).

- **`kms_graph_auto_paging_enabled`:** when **true**, indexed note count **≥** **`kms_graph_auto_paging_note_threshold`** opens the global graph in **paged** mode by default; when **false**, default is **full graph**—permanent opt-out from automatic paging-by-size.
- Session pagination position uses **`kms_graph_session_paged_offset`** / **`kms_graph_session_paged_limit`** (client-only storage).

See **section 2.3** and **2.4** acceptance criteria.

### 6.2 Epic A: Pathfinding in paged mode

**Decision:** **Disable** shortest-path / pathfinding UI in **paged** global graph mode until a **follow-up epic** defines cross-page behavior (partial highlight, server-side path with highlight only for on-page segments, etc.). Surface a clear **inline notice** pointing users to **full graph** or **local graph** for path exploration.

### 6.3 Epic B: Vault identity key (normalization, Windows)

**Decision / engineering standard:**

1. **Primary key:** The KMS **vault root path** already used at runtime (same string as initialization / `vaultPath` in UI), stored after **canonicalization** where supported (e.g. resolve `.`, symlinks via platform APIs) so the same folder opened two ways maps to one key.
2. **Windows:** Persist and compare vault keys using **case-insensitive** equality for the path string (drive letter and path segments per Windows norms). Optionally normalize to a single casing (e.g. canonical path from the OS) in one place when writing the override map.
3. **Implementation rule:** One shared helper `vault_graph_settings_key(vault_root: &Path) -> String` (or equivalent) used by load/save and bundle export so behavior is consistent.

### 6.4 Epic B: Bundle export of overrides (v1 recommendation)

**Decision:** **Include** per-vault graph overrides in **v1** of the enhanced bundle for **robustness and UX**:

- **Why:** Restoring settings on a new machine or after reinstall should restore **both** global graph defaults **and** per-vault tweaks; omitting overrides causes confusing “works on old PC, wrong on new PC” support issues.
- **How:** Extend the settings bundle JSON with **`kms_graph_vault_overrides`**: a map from **`vault_graph_settings_key`** (output of the canonical helper in **6.3**) to a **partial object** whose leaf keys are the same **`kms_graph_*`** names as in **section 7.2** (omit keys to mean “use global”). Bump **`settings_bundle_version`** (or equivalent) when introducing this field.
- **Import:** Prefer **replace** semantics for imported keys for that vault (simplest mental model for “restore backup”); document merge behavior if product chooses merge.
- **Privacy:** Same sensitivity as other local settings (no cloud unless a separate feature).

---

## 7. Engineering key registry (ticket reference)

Use these names consistently in Rust (`AppState`, `storage_keys`, bundle serde), generated bindings, and `ConfigTab` / Vault Settings UI.

### 7.1 Epic A – new persisted global keys (add to app state + storage)

| Key | Type |
|-----|------|
| `kms_graph_auto_paging_enabled` | `bool` |
| `kms_graph_auto_paging_note_threshold` | `u32` |

### 7.2 Epic A – client session only (localStorage / front-end; not in `AppState`)

| Key | Type |
|-----|------|
| `kms_graph_session_paged_offset` | `u32` |
| `kms_graph_session_paged_limit` | `u32` |

### 7.3 Existing global graph keys (already in codebase; per-vault overrides may include any subset)

These exist today in `api.rs` / settings bundles; per-vault objects reuse **the same leaf key strings**:

| Key | Type |
|-----|------|
| `kms_graph_k_means_max_k` | `u32` |
| `kms_graph_k_means_iterations` | `u32` |
| `kms_graph_ai_beam_max_nodes` | `u32` |
| `kms_graph_ai_beam_similarity_threshold` | `f32` |
| `kms_graph_ai_beam_max_edges` | `u32` |
| `kms_graph_enable_ai_beams` | `bool` |
| `kms_graph_enable_semantic_clustering` | `bool` |
| `kms_graph_semantic_max_notes` | `u32` |
| `kms_graph_warn_note_threshold` | `u32` |
| `kms_graph_beam_max_pair_checks` | `u32` |

**Note:** `kms_graph_auto_paging_*` and `kms_graph_session_*` are **global / UX** concerns; v1 per-vault overrides **may** omit them unless product later adds per-vault paging defaults (default: **global only** for auto-paging keys).

### 7.4 Bundle root field for Epic B

| Key | Shape |
|-----|--------|
| `kms_graph_vault_overrides` | `Record<string, Partial<{ keys from 7.3 }>>` — outer key = canonical vault key from **6.3** |

---

*End of one-page PRD stub.*
