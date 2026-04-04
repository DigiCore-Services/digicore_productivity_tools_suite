# KMS: Embeddings, Temporal Graph (Seq 12), Semantic Search, and Hexagonal Ports

> Doc governance status: Authoritative companion (embeddings/semantic-search/seq-12 scope)
> Canonical roadmap authority: `knowledge-graph-comprehensive-audit-and-roadmap-2026-03.md`
> Governance map: `kms-graph-doc-governance-map-2026-04.md`

**Document purpose:** Standalone planning artifact for the **next engineering sprint** and follow-on work: **seq 12 (temporal / time-aware graph on the server)**, **`LoadEmbeddingsPort` (and related hexagonal boundaries)**, **optional materialized PageRank when graph settings change**, and a **configuration-first** path for **embedding generation** that serves both the **Knowledge Graph** (clustering, kNN edges, beams) and **semantic / hybrid search** in the **Tauri** app (Search, Explorer, KMS surfaces).

**Primary code references (current implementation):**

| Concern | Location |
|--------|----------|
| Graph build, semantic layer | `digicore/tauri-app/src-tauri/src/kms_graph_service.rs` |
| Note embedding load (all / scoped) | `kms_repository.rs` (`get_all_note_embeddings`, `get_note_embeddings_for_paths` for local graph hood) |
| Hybrid / semantic search | `digicore/tauri-app/src-tauri/src/kms_repository.rs` (`search_hybrid`), `api.rs` (`kms_search_semantic`) |
| Embedding generation (index time) | `embedding_pipeline.rs` (normalize, optional chunk mean-pool, `FastembedTextGenerator` / `KmsSqliteNoteEmbeddingStore`), `api.rs` (`sync_note_index_internal` spawn_blocking) |
| Hybrid search query vector | `embedding_pipeline::embed_kms_query_text_blocking` (same normalization + chunk policy as notes) from `kms_search_semantic` |
| Graph ports (partial) | `digicore/tauri-app/src-tauri/src/kms_graph_ports.rs` |
| Effective graph params + vault overrides | `digicore/tauri-app/src-tauri/src/kms_graph_effective_params.rs` |
| Materialized wiki PageRank, background job | `kms_graph_service.rs`, `api.rs` (`spawn_background_wiki_pagerank_after_vault_sync`) |
| App settings / Config UI | `digicore/crates/digicore-text-expander/src/application/app_state.rs`, `tauri-app/src/components/ConfigTab.tsx`, `VaultSettingsModal.tsx` |

**Related roadmap (do not duplicate):** `knowledge-graph-comprehensive-audit-and-roadmap-2026-03.md` (seq 12, seq 10 embeddings note, Phase 4 temporal bullet, section 7.2 scale policy).

---

## Stakeholder decisions (recorded 2026-03-28)

| ID | Decision |
|----|----------|
| **D1** | **Temporal MVP:** Ship **both** Option **A** (time window filter) and Option **B** (edge recency weighting for visualization), each behind its **own flag** (Config + RPC where applicable). |
| **D2** | **Default temporal behavior:** When the client sends **no** time window and no default window is configured, server behavior stays **unchanged** (no implicit filter). |
| **D3** | **PageRank + time:** For time-filtered graphs, **PageRank ignores the time filter** (compute centrality on the **full** wiki topology used for PR today, or a documented full-vault note set), while **layout / display** may still reflect A/B. Exact split is implementation detail; PR must not be restricted to the time-induced subgraph. |
| **D4** | **Port location:** Extract **graph and embedding port traits** to a **shared workspace crate** now (implement as `digicore/crates/digicore-kms-ports` or name agreed in PR; Tauri implements adapters calling `kms_repository`). |
| **D5** | **Search similarity floor:** **Yes** - enforce **minimum cosine similarity** (drop weak vector hits) in semantic/hybrid paths. Support **global AppState** and **per-vault overrides** (multi-vault). |
| **D6** | **Model change policy:** On embedding model switch: **background re-embed / migrate** with **progress UI** (no hard block of the whole app; clear status and completion). |
| **D7** | **PR on settings:** **Track C is in scope for this sprint** (debounced background materialize when pagerank-affecting settings change). |
| **D8** | **Config UI:** Add a **sibling** area **KMS Search & embeddings** (alongside Knowledge Graph), not nested only under Knowledge Graph. |
| **Embedding architecture** | **O3 - Central `EmbeddingPipeline` application service:** normalize text, chunk, call generator via **`EmbeddingGeneratorPort`**, persist via **`EmbeddingStorePort`**, bump **`embedding_model_version`** on note row or side table. |

---

## 1. Sprint scope (single next sprint, three tracks)

### 1.1 Track A - Seq 12: Temporal graph (server) **[MVP delivered; Option C future]**

**Goal (met for MVP):** Server-side **D1**: (**A**) optional `last_modified` window on graph RPCs + config defaults; (**B**) optional edge-level recency in DTOs **separately toggled**. **D2:** defaults off unless explicitly configured. **D3:** PageRank path does **not** apply the time window to centrality computation. See **section 11** checklist.

**Still future:** Option **C** (versioned snapshots / valid-from edge schema); extra temporal unit tests / UX polish as needed.

### 1.2 Track B - Hexagonal: shared crate + `LoadEmbeddingsPort` + O3 trajectory **[structural]**

**Goal (sprint minimum):** **`LoadEmbeddingsPort`** (and existing note/link ports as needed) defined in **shared crate** (**D4**); Tauri **repository adapters** implement them; `kms_graph_service` uses the port in production build paths.

**O3 alignment:** Sprint may **introduce trait boundaries** (`EmbeddingGeneratorPort`, `EmbeddingStorePort`) and a thin **EmbeddingPipeline** facade used from **one** call site (e.g. sync path) if time permits; full chunking + version column + migrate job may **span sprint +1** but **design to O3**, not O1/O2-only.

### 1.3 Track C - PR on settings change **[in sprint per D7]**

**Goal:** When **global** or **effective** graph knobs that affect materialized wiki PageRank **iterations** or **damping** change (and `pagerank_scope` is not `off`, effective background PR enabled), **queue** a debounced background materialize (same as post-sync).

**Guardrails:** Debounce **500 ms - 2 s**, single-flight / coalesce with in-flight job, skip if fingerprint unchanged, respect effective `kms_graph_background_wiki_pagerank_enabled`.

---

## 2. Current embedding data flow (fact base)

**Generation (note, text):** On note sync, content is embedded via `embedding_service::generate_text_embedding` (currently **fastembed** `BGESmallENV15`) and stored through `kms_repository::upsert_embedding` into SQLite vec tables (`kms_embeddings_text`, `kms_vector_map`).

**Graph consumption:** `build_full_graph` / local graph paths call `get_all_note_embeddings()` (full table join for notes) when semantic clustering and/or semantic kNN edges are enabled; failures degrade with DTO `warnings`.

**Search consumption:** `kms_search_semantic` embeds the **query** with the same generator, then `search_hybrid` runs vec search + optional FTS5 + RRF-style blending for `"Hybrid"`.

**Implication:** Graph and search **share the same vector space** today (same model for note and query). Any change to **model, dimension, or preprocessing** is a **cross-cutting** migration (re-embed all notes + document search behavior).

---

## 3. Seq 12 (temporal server): implementation options

### 3.1 Option A - Filter edges/nodes by `last_modified` window (MVP)

**Idea:** Accept optional `time_from_utc` / `time_to_utc` (or `recency_days`) on `kms_get_graph` / `kms_get_local_graph` (and effective build params or RPC args). Server drops nodes outside window and wiki edges incident to dropped nodes; semantic layer runs on **remaining** notes only.

| Pros | Cons |
|------|------|
| No new tables; uses existing columns | Not true "edge history"; moving a note jumps across cutoffs |
| Predictable perf (smaller induced graph) | Client and server must agree on timezone semantics |
| Fits "timeline filter exists on client" story | kNN edges become time-ambiguous (computed on filtered set) |

### 3.2 Option B - Derived edge weight from endpoint recency

**Idea:** Keep topology; multiply or threshold edge draw weight / force strength from `min(last_modified_src, last_modified_tgt)` or similar; expose in DTO as optional `edge_weight` or reuse a channel.

| Pros | Cons |
|------|------|
| Preserves full topology for path tools | Harder to explain in UI; PageRank semantics change if you weight edges |
| Good for "glow recent links" | Requires clear product rule vs undirected PR |

### 3.3 Option C - Schema: versioned snapshots or edge valid-from/to

**Idea:** Store graph snapshots or temporal edges for true historical views.

| Pros | Cons |
|------|------|
| Correct for audits / "as of" queries | Migration cost, storage, invalidation complexity |
| Supports serious temporal analytics | Likely **post-MVP** for seq 12 |

### 3.4 SWOT (temporal MVP: A vs B)

| | **A (window filter)** | **B (edge weight)** |
|--|----------------------|----------------------|
| **Strengths** | Simple; large vault friendly | Rich visualization; no node deletion surprises |
| **Weaknesses** | Abrupt cutoff; may confuse path finders | Tuning burden; interacts with PR and layout |
| **Opportunities** | Combine with paging for fixed budgets | Pair with legend "recency scale" |
| **Threats** | Users misread "missing" nodes as data loss | Performance if every edge gets custom weight pass |

**Recorded decision (D1):** Implement **both A and B** for seq 12, each behind **separate flags** (see section 9.1). Option **A** remains the primary **payload reduction** lever; **B** is optional **visual emphasis** without removing nodes.

---

## 4. Shared crate + `LoadEmbeddingsPort` (hexagonal)

### 4.1 Shared crate (**D4**)

- Add a workspace crate (proposed name: **`digicore-kms-ports`**) under `digicore/crates/`, depending only on **std / serde / thiserror** (or minimal deps) - **no** Tauri, **no** SQLite in the crate.
- Move or redefine: `LoadNotesMinimalPort`, `LoadWikiLinksPort`, **`LoadEmbeddingsPort`**, and (as O3 lands) **`EmbeddingGeneratorPort`**, **`EmbeddingStorePort`** trait objects or generics.
- **`digicore-text-expander-tauri`** implements adapters that call `kms_repository` and `embedding_service`.

### 4.2 Suggested `LoadEmbeddingsPort` shape (Rust)

- `load_note_embeddings_all() -> Result<Vec<(String, Vec<f32>)>, E>` (parity with today), and/or  
- `load_note_embeddings_for_paths(paths: &[String]) -> Result<...>` for future scoped loads.

**Consumer:** `kms_graph_service` accepts `&dyn LoadEmbeddingsPort` in `build_full_graph_with_ports`; thin wrapper keeps `build_full_graph` stable.

### 4.3 O3: `EmbeddingPipeline` (application service)

**Responsibilities:** normalize input text, **chunk** (policy from config), invoke **`EmbeddingGeneratorPort`**, write vectors through **`EmbeddingStorePort`**, update **`embedding_model_version`** (note column or `kms_vector_map` / side table - schema task).

**Callers:** note sync (`api.rs` today), future **bulk re-embed**, query embedding for search (same generator).

### 4.4 SWOT (embeddings port + shared crate)

| Strengths | Weaknesses |
|-----------|------------|
| Testability without SQLite | One more indirection |
| Clear seam for **temporal filter** (load after path filter) | Local graph still loads "all minimal notes" separately today |
| Aligns with seq 10 ports story | Does not by itself fix full-table embedding scan cost |

| Opportunities | Threats |
|---------------|---------|
| Swap in **chunked** or **mmap** loader later | Over-abstracting before second adapter exists |
| Unify with **search** read path later | Scope creep into image embeddings |

---

## 5. Materialized PageRank when settings change (**Track C, D7**)

**Trigger points:** `update_config` (or dedicated graph save) when any of: `kms_graph_pagerank_iterations`, `kms_graph_pagerank_damping`, `kms_graph_pagerank_scope` (if moving off `off`), or vault override equivalents change.

**Behavior:** Same as post-`sync_vault_files_to_db_internal` background job: respect effective `background_wiki_pagerank_enabled` and `pagerank_scope != off`; debounce **500 ms - 2 s** to collapse slider drags.

**Rejected for this sprint as default policy:** Invalidate-only (no auto-recompute); may remain an emergency manual path if ever needed.

---

## 6. Advanced embedding generation (graph + search)

This section frames **follow-on** work beyond the single sprint; it informs **Config** and **ports**.

### 6.1 Dimensions of evolution

| Theme | Graph impact | Search impact |
|-------|--------------|---------------|
| **Model choice** (e.g. BGE-small vs larger) | Cluster geometry, kNN density | Query/note space must match |
| **Model versioning** in DB | Select correct vec table or metadata | Avoid mixed-dimension queries |
| **Chunking** (long notes) | Multiple vec rows per note; graph edges? | Snippet-level hits |
| **Batch / queue** embed on sync | Throughput, backpressure | Stale search until indexed |
| **Image modality** | Optional image nodes in graph | Unified search already has `modality` param |
| **CPU vs GPU / provider** | Index latency | Same |

### 6.2 Implementation options (embedding pipeline) - **O3 selected**

**O1 / O2** - Superseded as **primary** direction; O3 **includes** generator and store ports internally.

**O3 - Central `EmbeddingPipeline` application service (RECORDED CHOICE)**  
Orchestrates: normalize text, chunk, call **`EmbeddingGeneratorPort`**, write via **`EmbeddingStorePort`**, bump **`embedding_model_version`** on note row or side table. Single place for retries, metrics, and **D6** background migration orchestration.

### 6.3 Rationale (why O3)

| Theme | Benefit |
|-------|---------|
| **D6** | Progress UI + background migrate naturally hook into pipeline + job state |
| **Graph + search** | One code path for "text in / vectors out + stored" |
| **Config-first** | Chunk size, model id, batch ticks live in AppState and vault overrides |

### 6.4 SWOT (embedding strategy, whole program)

**Strengths:** Local-first; single SQLite; hybrid search already combines FTS + vectors.  
**Weaknesses:** Full-note embedding scan for **full** graph builds; **no explicit `embedding_model_version` / dimension guard** in search yet; RAM cost if many fastembed models are initialized.  
**Opportunities:** Shared ports for graph + search; batch re-embed job from Config; quality toggles per vault.  
**Threats:** Model switch without migration breaks search; large vault + sync storm pins CPU.

---

## 7. Semantic search (Tauri) alignment

**Today:** `kms_search_semantic` embeds query and calls `search_hybrid`.  

**Planned alignment:**

1. **Same `EmbeddingGeneratorPort`** (via **O3 pipeline** or direct port) as index path so query and notes stay in one vector space.  
2. **D5:** Apply **minimum cosine similarity** after vector retrieval (and define interaction with **Hybrid** RRF - e.g. floor applies to vector leg before fuse, documented in impl).  
3. **D5:** **`kms_search_min_similarity`** global + **per-vault override** key (e.g. `kms_search_min_similarity` in vault graph/search patch JSON alongside existing `kms_graph_*` keys, or dedicated `kms_search_*` map - choose one namespace in impl PR).  
4. **Explorer / KMS** UIs call RPC only; no client-side embedding.

**Implemented:** Search DTO diagnostics: **`kms_effective_embedding_model_id`**, **`kms_query_embedding_ms`**, gated by **`kms_search_include_embedding_diagnostics`** (Config: KMS Search & embeddings). Optional **`vector_k`** not exposed.

---

## 8. Stakeholder decisions (archive)

All items **D1-D8** and **O3** are **recorded** at the top of this document (2026-03-28). Future questions get new IDs in **Appendix A**.

---

## 9. Configuration-first: AppState keys and UI map

**Principle:** New behavior **off by default** (**D2**); persist via JSON storage + `ConfigUpdateDto` + settings bundle groups **`kms_graph`** and **`kms_search`** (and vault overrides).

### 9.1 Seq 12 (temporal) - keys (**D1** two levers)

| Key | Type | Default | UI surface |
|-----|------|---------|------------|
| `kms_graph_temporal_window_enabled` | bool | `false` | Config: **Knowledge Graph** - "Time window filter (Option A)" |
| `kms_graph_temporal_default_days` | u32 | `0` | Same (0 = no default window unless RPC sends bounds) |
| `kms_graph_temporal_include_notes_without_mtime` | bool | `true` | Advanced / vault override |
| `kms_graph_temporal_edge_recency_enabled` | bool | `false` | Config: **Knowledge Graph** - "Edge recency emphasis (Option B)" |
| `kms_graph_temporal_edge_recency_*` | f32 / enum | TBD in impl | Tunables for B (e.g. decay half-life); document in PR |

**D2:** If client sends **no** window and defaults are off, **no** temporal filtering.  
**D3:** Document in UI help that **link_centrality / materialized PR** reflect **vault-wide** policy, not the time slice.

RPC: explicit `time_from_utc` / `time_to_utc` (or `recency_days`) overrides for one-shot views.

### 9.2 Embeddings / search - keys (**D5**, **D6**, **O3**)

| Key | Phase | Notes |
|-----|-------|------|
| `kms_embedding_model_id` | 1+ | When multiple models; drives **D6** migrate |
| `kms_embedding_batch_notes_per_tick` | 1+ | Backpressure for sync + migrate |
| `kms_embedding_chunk_*` | 1+ | Chunk size / overlap under O3 |
| `kms_search_default_mode` | 1 | `"Hybrid"` \| `"Semantic"` \| `"Keyword"` |
| `kms_search_default_limit` | 1 | e.g. 20 |
| `kms_search_min_similarity` | 1 | **D5** global; `0` = disable floor |
| `kms_search_include_embedding_diagnostics` | 1 | When false, semantic search omits query-embed timing + effective model id on each **`SearchResultDto`** row |
| Vault override | 1 | **D5** `kms_search_min_similarity` (and optionally mode/limit) per vault |

### 9.3 **Configurations and Settings** layout (**D8**)

- **Sibling** top-level section (or tab): **KMS Search & embeddings**  
  - Embeddings: model display/selector, chunk/batch, **re-embed / migrate** progress (D6), link to O3 pipeline behavior.  
  - Search: default mode, limit, **minimum similarity** (global).  
- **Knowledge Graph** (existing): temporal **A** and **B** toggles + tunables; cross-link to Search tab for "same embedding model".

Vault **Settings** modal: overrides for **temporal A/B**, **search floor**, and embedding-related keys as needed (same JSON patch pattern; extend `VaultPatchKey` / merge map).

---

## 10. Risks and mitigations

| Risk | Mitigation |
|------|------------|
| Temporal filter hides nodes and breaks user mental model | Toolbar / DTO `warnings` + "time filter active" badge |
| Double `get_all_note_embeddings` in local graph path | Port + later **single load** refactor (documented tech debt) |
| Settings-triggered PR storms | Debounce + single-flight guard |
| Model migration half-done | Version column + search rejects mismatched dims with clear error |

---

## 11. Deliverables checklist (sprint exit criteria)

- [x] **Seq 12 D1:** RPC + service for **Option A** (window) **and** **Option B** (edge recency), **separate flags**; **D3** PR on full graph before temporal filter; `KmsEdgeDto.edge_recency`; `kms_get_graph` optional `time_from_utc` / `time_to_utc`. (Extra unit tests for temporal parsing/filter optional follow-up.)  
- [x] **D4:** Workspace crate **`digicore-kms-ports`** with load ports including **`LoadNoteEmbeddingsPort`**; Tauri adapters; `build_full_graph_with_ports` wired.  
- [ ] **O3 (incremental OK):** Trait-only stub `embedding_pipeline.rs`; sync still uses `embedding_service` / `kms_repository`. **D6** job + progress UI still open.  
- [x] **D5:** `kms_search_min_similarity` global + vault JSON key; applied in `search_hybrid` (vector leg, after FTS, before RRF).  
- [x] **Track C (D7):** Debounced background wiki PageRank when global pagerank iterations/damping/scope change via `update_config`.  
- [x] **D8:** Config **KMS Search & embeddings** tab; temporal A/B under **Knowledge Graph**.  
- [x] **Config:** `app_state.rs`, storage keys, `lib.rs` / `api.rs` persist, `ConfigUpdateDto`, `ConfigTab`, types/bindings; settings bundle `kms_graph` export/import extended. Vault overrides JSON supports same keys (dedicated Vault UI fields optional).  

---

## 12. Implementation status (engineering log)

| Date | Area | Notes |
|------|------|--------|
| 2026-03-30 | **D4** | `digicore-kms-ports` + `kms_graph_ports` adapters. |
| 2026-03-30 | **Seq 12** | Temporal window + edge recency in `kms_graph_service`; IPC on `kms_get_graph`. |
| 2026-03-30 | **D5** | `effective_kms_search_min_similarity` + `search_hybrid` floor. |
| 2026-03-30 | **D7** | `schedule_debounced_background_wiki_pagerank_on_settings`. |
| 2026-03-30 | **O3** | `embedding_pipeline.rs` ports + note path. |
| 2026-03-30 | **O3+** | Chunking keys + mean-pooled multi-chunk notes; query path uses same pipeline; settings bundle + Config UI. |
| 2026-03-30 | **Embeddings** | Multi-model **`embedding_service`** (`EmbeddingModel::from_str` + per-id cache); query path passes effective model id into **`embed_kms_query_text_blocking`**. |
| 2026-03-30 | **Search UX** | **`kms_search_include_embedding_diagnostics`** (storage + Config + `SearchResultDto` **Option** fields when disabled). |
| 2026-03-30 | **Build** | `#![recursion_limit = "256"]` on Tauri lib (large settings-bundle `json!`). |

---

## Appendix A - Decision log

| Date | Decision | Outcome |
|------|----------|---------|
| 2026-03-28 | D1 | Both temporal **A** (window) and **B** (edge recency); **separate flags**. |
| 2026-03-28 | D2 | No client window + no default => **unchanged** server behavior (no implicit filter). |
| 2026-03-28 | D3 | **PageRank ignores** time filter; do not restrict PR to time-induced subgraph. |
| 2026-03-28 | D4 | **Shared workspace crate** for ports now; Tauri implements adapters. |
| 2026-03-28 | D5 | **Min cosine similarity** enforced; **global + per-vault** support. |
| 2026-03-28 | D6 | Model switch => **background migrate** + **progress UI**. |
| 2026-03-28 | D7 | **Track C** (PR on settings) **in sprint**. |
| 2026-03-28 | D8 | **Sibling** Config section **KMS Search & embeddings**. |
| 2026-03-28 | Embedding | **O3** `EmbeddingPipeline` + `EmbeddingGeneratorPort` + `EmbeddingStorePort` + `embedding_model_version`. |
| 2026-03-28 | Sprint seq 12 | **Option A** as primary MVP recommendation **extended** by stakeholder to **A+B** both in scope. |

---

## Appendix B - Glossary

- **Induced subgraph:** Nodes in set S plus edges whose both endpoints are in S.  
- **Materialized PageRank:** `wiki_pagerank` column + fingerprint in `kms_graph_meta`.  
- **Effective params:** Global `AppState` merged with per-vault JSON overrides (`kms_graph_effective_params.rs`).

---

**Document control**

| Version | Date | Summary |
|---------|------|---------|
| 1.0 | 2026-03-28 | Initial: sprint scope (seq 12, embeddings port, optional PR-on-settings), embedding/search alignment, SWOT, options, stakeholder decisions, Config UI map |
| 1.1 | 2026-03-28 | Stakeholder decisions D1-D8 + O3 recorded; Track C in sprint; shared crate D4; temporal A+B; PR ignores time filter D3; search floor global+vault D5; Config sibling D8; checklist + keys updated |
| 1.2 | 2026-03-30 | First implementation pass: ports crate, temporal graph + IPC, search floor, PR-on-settings debounce, Config UI, `embedding_pipeline` stub; checklist section 11 updated |
| 1.3 | 2026-03-30 | O3 note+query alignment, `kms_embedding_chunk_*` AppState/storage/DTOs, D6 migrate passes chunk cfg; doc checklist synced |
| 1.4 | 2026-03-30 | Doc refresh: §2 flow, Track A status, O3/D6 checklist, multi-model + diagnostics done; remaining follow-ups listed |

**Owner:** DigiCore KMS / platform (align with `knowledge-graph-comprehensive-audit-and-roadmap-2026-03.md`).

### Open follow-ups (embedding / search track)

- **Optional:** persist a distinct **`embedding_model_version`** string from the fastembed crate (beyond normalized id + fingerprint) if product needs finer upgrade detection.  
- **D6 progress UX (optional):** dedicated cancel is shipped; further polish (multi-vault queue UI, persist ETA model) if needed.

**Done (through 2026-03-31):** `EmbeddingGeneratorPort` / `EmbeddingStorePort` / **`KmsTextEmbeddingChunkConfig`** in **`digicore-kms-ports`**; **`embedding_pipeline`** for notes + queries. **Per-vault** search floor + chunk keys via **`effective_kms_*`**, **Vault Settings**. **Local graph** scoped embedding load via **`LoadNoteEmbeddingsPort::load_note_embeddings_for_paths`**. **Multi-model fastembed** per **`kms_embedding_model_id`**. **Search diagnostics** optional via **`kms_search_include_embedding_diagnostics`**; **`SearchResultDto`** uses **`Option`** for timing and model id when gated off. **Config:** **`kms_search_default_mode`**, **`kms_search_default_limit`**. **Note embedding fingerprint:** **`kms_notes.embedding_policy_sig`** (`v1|model|chunk_enabled|max|overlap|vec_dim`) written on every successful note embed; D6 migration and **`kms_get_embedding_policy_diagnostics`** compare against the effective policy. **Auto re-embed:** **`update_config`** queues D6 when **effective** chunk policy changes (merged vault overrides), not only when the model id changes. **D6 UI:** Config tab modal (progress, linear ETA, **Cancel migration** via **`kms_cancel_note_embedding_migration`**), chunk controls, stale-count banner.
