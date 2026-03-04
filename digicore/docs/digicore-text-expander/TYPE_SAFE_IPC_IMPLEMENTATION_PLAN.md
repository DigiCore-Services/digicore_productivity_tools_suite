# Type-Safe IPC Implementation Plan

**DigiCore Text Expander – Specta / TauRPC Refactoring**

**Version:** 1.0  
**Last Updated:** 2026-03-03  
**Purpose:** Refactoring and implementation plan for migrating from string-based Tauri `invoke()` to type-safe IPC using Specta or TauRPC.

---

## 1. Executive Summary

The current DigiCore Text Expander Tauri app uses **string-based IPC**: frontend calls `invoke("command_name", { args })` with manually typed payloads and casts. This leads to:

- No compile-time validation of command names or argument shapes
- Manual TypeScript interfaces that can drift from Rust
- Runtime errors when command names or payloads change

**Goal:** Migrate to type-safe IPC so that:

- Command names and argument/return types are derived from Rust (Single Source of Truth)
- TypeScript types are generated automatically
- IDE autocomplete and compile-time checks catch mismatches

**Options:** Two main approaches are viable:

| Approach | Pros | Cons |
|----------|------|------|
| **Tauri Specta** | Minimal change; add `#[specta::specta]` to existing commands; export types | No runtime proxy; frontend still uses `invoke()` with generated types |
| **TauRPC** | Full typed proxy; `await taurpc.command_name(args)`; events typed; trait-based API | Larger refactor; async-only; different handler registration |

This document outlines both paths and recommends a phased approach.

---

## 2. Current State

### 2.1 Backend Commands (37 total)

All commands live in `digicore/tauri-app/src-tauri/src/lib.rs` and are registered via `tauri::generate_handler![]`.

| Category | Commands |
|----------|----------|
| **App / Library** | `greet`, `get_app_state`, `load_library`, `save_library`, `set_library_path`, `save_settings` |
| **UI Prefs** | `get_ui_prefs`, `save_ui_prefs` |
| **Snippets** | `add_snippet`, `update_snippet`, `delete_snippet` |
| **Config** | `update_config` |
| **Ghost Suggestor** | `get_ghost_suggestor_state`, `ghost_suggestor_accept`, `ghost_suggestor_dismiss`, `ghost_suggestor_create_snippet`, `ghost_suggestor_cycle_forward` |
| **Ghost Follower** | `get_ghost_follower_state`, `ghost_follower_insert`, `ghost_follower_set_search`, `ghost_follower_request_view_full`, `ghost_follower_request_edit`, `ghost_follower_request_promote`, `ghost_follower_toggle_pin` |
| **Variable Input** | `get_pending_variable_input`, `submit_variable_input`, `cancel_variable_input` |
| **Analytics** | `get_expansion_stats`, `reset_expansion_stats` |
| **Log** | `get_diagnostic_logs`, `clear_diagnostic_logs` |
| **Clipboard** | `get_clipboard_entries`, `delete_clip_entry`, `clear_clipboard_history`, `copy_to_clipboard` |
| **Script** | `get_script_library_js`, `save_script_library_js` |

### 2.2 DTOs Requiring Type Generation

| DTO | Used By | Fields (summary) |
|-----|--------|------------------|
| `AppStateDto` | `get_app_state` | library_path, library, categories, selected_category, status, sync_url, sync_status, expansion_paused, template_*, discovery_*, ghost_*, clip_history_max_depth, script_library_* |
| `UiPrefsDto` | `get_ui_prefs` | last_tab, column_order |
| `ConfigUpdateDto` | `update_config` | Optional fields for all config keys |
| `Snippet` | `add_snippet`, `update_snippet` | trigger, content, profile, options, category, appLock, pinned (from digicore-core) |
| `GhostSuggestorStateDto` | `get_ghost_suggestor_state` | has_suggestions, suggestions, selected_index, position, should_passthrough |
| `SuggestionDto` | nested | trigger, content_preview, category |
| `GhostFollowerStateDto` | `get_ghost_follower_state` | enabled, pinned, search_filter, position, edge_right, monitor_primary |
| `PinnedSnippetDto` | nested | trigger, content, content_preview, category, snippet_idx |
| `PendingVariableInputDto` | `get_pending_variable_input` | content, vars, values, choice_indices, checkbox_checked |
| `InteractiveVarDto` | nested | tag, label, var_type, options |
| `ExpansionStatsDto` | `get_expansion_stats` | total_expansions, total_chars_saved, estimated_time_saved_secs, top_triggers |
| `DiagnosticEntryDto` | `get_diagnostic_logs` | timestamp_ms, level, message |
| `ClipEntryDto` | `get_clipboard_entries` | content, process_name, window_title, length |

### 2.3 Frontend Invoke Call Sites

| File | Commands Used |
|------|---------------|
| `App.tsx` | get_app_state, add_snippet, update_snippet, delete_snippet, save_library, save_settings, clear_clipboard_history, save_ui_prefs, submit_variable_input, cancel_variable_input, get_pending_variable_input, get_ui_prefs |
| `LibraryTab.tsx` | get_app_state, set_library_path, load_library, save_settings, update_snippet, save_library, copy_to_clipboard |
| `ConfigTab.tsx` | update_config, save_settings |
| `ClipboardTab.tsx` | get_clipboard_entries, copy_to_clipboard, delete_clip_entry |
| `ScriptTab.tsx` | get_app_state, get_script_library_js, update_config, save_settings, save_script_library_js |
| `AnalyticsTab.tsx` | get_expansion_stats, reset_expansion_stats |
| `LogTab.tsx` | get_diagnostic_logs, clear_diagnostic_logs |
| `CommandPalette.tsx` | copy_to_clipboard |
| `ghost-follower.html` | ghost_follower_insert, ghost_follower_request_view_full, ghost_follower_toggle_pin, ghost_follower_request_edit, copy_to_clipboard, delete_snippet, save_library, ghost_follower_insert, ghost_follower_request_view_full, delete_clip_entry, ghost_follower_request_promote, ghost_follower_set_search, get_ghost_follower_state, get_clipboard_entries |
| `ghost-suggestor.html` | ghost_suggestor_cycle_forward, get_ghost_suggestor_state, ghost_suggestor_accept, ghost_suggestor_create_snippet, ghost_suggestor_dismiss |

**Note:** `ghost-follower.html` and `ghost-suggestor.html` are standalone HTML pages loaded in WebviewWindows. They use `invoke` via the Tauri preload script. Type-safe usage there would require either (a) importing generated bindings in a bundled context, or (b) keeping string-based invoke for overlays and only migrating the main React app.

---

## 3. Option A: Tauri Specta (Incremental)

### 3.1 Overview

Tauri Specta adds `#[specta::specta]` to commands and `#[derive(specta::Type)]` (or `#[derive(Serialize, Type)]`) to DTOs. It generates TypeScript types at build time. The frontend continues to use `invoke()` but with **generated types** instead of manual casts.

### 3.2 Dependencies

```toml
# src-tauri/Cargo.toml
[dependencies]
specta = { version = "2.0", features = ["derive"] }
tauri-specta = { version = "2", features = ["typescript"] }
```

### 3.3 Backend Changes

1. **Add Specta to DTOs**

   ```rust
   use specta::Type;

   #[derive(serde::Serialize, Type)]
   pub struct AppStateDto { ... }

   #[derive(serde::Serialize, Type)]
   pub struct UiPrefsDto { ... }

   #[derive(serde::Deserialize, Type)]
   pub struct ConfigUpdateDto { ... }

   // Repeat for all DTOs used in command signatures
   ```

2. **Add Specta to Commands**

   ```rust
   #[tauri::command]
   #[specta::specta]
   fn get_app_state(state: State<TauriAppState>) -> Result<AppStateDto, String> { ... }

   #[tauri::command]
   #[specta::specta]
   fn load_library(state: State<TauriAppState>, app: tauri::AppHandle) -> Result<usize, String> { ... }
   ```

3. **Export Types**

   In `lib.rs` or a dedicated module:

   ```rust
   use specta::collect_types;
   use tauri_specta::ts;

   fn export_bindings() {
       ts::export(
           collect_types![
               greet, get_app_state, load_library, save_library, set_library_path,
               save_settings, get_ui_prefs, save_ui_prefs, add_snippet, update_snippet,
               delete_snippet, update_config, get_clipboard_entries, delete_clip_entry,
               clear_clipboard_history, copy_to_clipboard, get_script_library_js,
               save_script_library_js, get_ghost_suggestor_state, ghost_suggestor_accept,
               ghost_suggestor_dismiss, ghost_suggestor_create_snippet, ghost_suggestor_cycle_forward,
               get_ghost_follower_state, ghost_follower_insert, ghost_follower_set_search,
               ghost_follower_request_view_full, ghost_follower_request_edit,
               ghost_follower_request_promote, ghost_follower_toggle_pin,
               get_pending_variable_input, submit_variable_input, cancel_variable_input,
               get_expansion_stats, reset_expansion_stats, get_diagnostic_logs, clear_diagnostic_logs,
           ],
           "../src/bindings.ts",
       )
       .unwrap();
   }
   ```

   Call `export_bindings()` from `build.rs` or from `fn run()` when `#[cfg(debug_assertions)]`.

### 3.4 Frontend Changes

1. **Create typed invoke wrapper**

   ```typescript
   // src/lib/tauri.ts
   import { invoke } from "@tauri-apps/api/core";
   import type * as Bindings from "@/bindings";

   export async function invokeTyped<T extends keyof Bindings.Commands>(
     cmd: T,
     args?: Bindings.CommandArgs<T>
   ): Promise<Bindings.CommandResult<T>> {
     return invoke(cmd, args) as Promise<Bindings.CommandResult<T>>;
   }
   ```

   (Exact shape of `Commands` / `CommandArgs` / `CommandResult` depends on Specta export format.)

2. **Replace invoke calls**

   ```typescript
   // Before
   const state = (await invoke("get_app_state")) as AppState;

   // After
   const state = await invokeTyped("get_app_state");
   ```

### 3.5 Specta Considerations

- **Snippet type:** `Snippet` comes from `digicore-core`. Either re-export a DTO in the Tauri crate with `#[derive(Serialize, Deserialize, Type)]`, or add Specta support to the core crate.
- **HashMap:** Specta supports `HashMap<String, String>`; verify `HashMap<String, Vec<Snippet>>` and similar.
- **Option, Result:** Specta handles these; `Result<T, String>` maps to `T | { error: string }` or similar.
- **Tuples:** `(i32, i32)` and `Option<(String, String)>` should work; verify export.

### 3.6 Effort Estimate

| Task | Effort |
|------|--------|
| Add Specta deps, annotate DTOs | 2–3 h |
| Annotate all commands | 1–2 h |
| Export + build integration | 1 h |
| Typed invoke wrapper | 1 h |
| Migrate React components | 2–3 h |
| Ghost HTML overlays (optional) | 1–2 h |
| **Total** | **~8–12 h** |

---

## 4. Option B: TauRPC (Full Refactor)

### 4.1 Overview

TauRPC replaces Tauri's invoke handler with a trait-based API. Procedures are async, and a TypeScript proxy is generated so the frontend calls `await taurpc.get_app_state()` with full type safety.

### 4.2 Dependencies

```toml
# src-tauri/Cargo.toml
[dependencies]
taurpc = "0.7"
specta = { version = "=2.0.0-rc.22", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
```

### 4.3 Backend Architecture

1. **Define procedures trait**

   ```rust
   #[taurpc::procedures(export_to = "../src/bindings.ts")]
   trait Api {
       async fn greet(name: String) -> String;
       async fn get_app_state() -> Result<AppStateDto, String>;
       async fn load_library() -> Result<usize, String>;
       async fn save_library() -> Result<(), String>;
       async fn set_library_path(path: String) -> Result<(), String>;
       async fn save_settings() -> Result<(), String>;
       async fn get_ui_prefs() -> Result<UiPrefsDto, String>;
       async fn save_ui_prefs(last_tab: usize, column_order: Vec<String>) -> Result<(), String>;
       async fn add_snippet(category: String, snippet: Snippet) -> Result<(), String>;
       async fn update_snippet(category: String, snippet_idx: usize, snippet: Snippet) -> Result<(), String>;
       async fn delete_snippet(category: String, snippet_idx: usize) -> Result<(), String>;
       async fn update_config(config: ConfigUpdateDto) -> Result<(), String>;
       // ... all other commands
   }
   ```

2. **Annotate DTOs with `#[taurpc::ipc_type]`**

   ```rust
   #[taurpc::ipc_type]
   #[derive(Clone)]
   pub struct AppStateDto { ... }
   ```

3. **Implement resolvers**

   ```rust
   #[derive(Clone)]
   struct ApiImpl {
       state: std::sync::Arc<std::sync::Mutex<AppState>>,
   }

   #[taurpc::resolvers]
   impl Api for ApiImpl {
       async fn get_app_state(self) -> Result<AppStateDto, String> {
           let guard = self.state.lock().map_err(|e| e.to_string())?;
           Ok(app_state_to_dto(&guard))
       }
       // ...
   }
   ```

4. **Replace invoke_handler**

   ```rust
   .invoke_handler(taurpc::create_ipc_handler(
       ApiImpl { state: app_state_arc }.into_handler(),
   ))
   ```

### 4.4 Accessing Tauri Context

TauRPC supports `Window`, `AppHandle`, `WebviewWindow` as procedure arguments. For `State<TauriAppState>`, use **managed state** on the implementation struct:

```rust
#[derive(Clone)]
struct ApiImpl {
    state: Arc<Mutex<AppState>>,
    app: AppHandle,  // Passed in setup
}
```

Commands that need `app.emit()` will use `self.app.emit(...)`.

### 4.5 Async Conversion

Current commands are sync. TauRPC procedures are async. Options:

- **Wrap in `async {}`:** `async fn load_library(self) -> Result<usize, String> { tokio::task::spawn_blocking(|| { ... }).await.unwrap() }` (overkill for fast ops)
- **Use `tauri::async_runtime::block_on`** inside the resolver (not recommended)
- **Keep logic sync, make resolver async:** `async fn load_library(self) -> Result<usize, String> { Ok(load_library_sync(self.state)?) }` – simplest

### 4.6 Frontend Changes

1. **Install TauRPC client**

   ```bash
   pnpm add taurpc
   ```

2. **Create proxy**

   ```typescript
   import { createTauRPCProxy } from "@/bindings";

   const taurpc = createTauRPCProxy();
   ```

3. **Replace invoke**

   ```typescript
   // Before
   const state = (await invoke("get_app_state")) as AppState;

   // After
   const state = await taurpc.get_app_state();
   ```

### 4.7 Ghost Overlays

`ghost-follower.html` and `ghost-suggestor.html` are loaded as separate HTML files. They need access to the TauRPC proxy. Options:

- **Bundle with Vite:** Convert overlays to React/Vue components and include in the main bundle (larger refactor).
- **Expose proxy globally:** In the main window, expose `window.__taurpc = createTauRPCProxy()` and have overlays use it if available.
- **Keep invoke for overlays:** Use TauRPC only in the main React app; overlays continue with `invoke()` (no type safety there).

### 4.8 Effort Estimate

| Task | Effort |
|------|--------|
| Add TauRPC deps, define trait | 2–3 h |
| Implement all resolvers, state wiring | 4–6 h |
| Replace invoke_handler, test | 2 h |
| Migrate React components | 2–3 h |
| Ghost overlays (bundle or hybrid) | 2–4 h |
| **Total** | **~12–18 h** |

---

## 5. Recommended Phased Approach

### Phase 1: Specta Only (Low Risk)

1. Add Specta + tauri-specta.
2. Annotate all DTOs with `#[derive(Type)]` (or `#[taurpc::ipc_type]` if going TauRPC later).
3. Annotate all commands with `#[specta::specta]`.
4. Export types to `src/bindings.ts` in debug builds.
5. Create a typed `invokeTyped` wrapper.
6. Migrate React components to use `invokeTyped`.
7. **Leave ghost overlays on raw `invoke`** for now.

**Outcome:** Type-safe calls in the main app; no change to Tauri's invoke handler; minimal risk.

### Phase 2 (Optional): TauRPC Migration

If Phase 1 is successful and you want the full proxy experience:

1. Introduce TauRPC trait and resolvers.
2. Migrate handler registration.
3. Replace `invokeTyped` with `taurpc` proxy in React.
4. Decide on ghost overlays (bundle vs. hybrid vs. keep invoke).

---

## 6. Implementation Checklist

### 6.1 Specta (Phase 1)

- [ ] Add `specta` and `tauri-specta` to `Cargo.toml`
- [ ] Add `#[derive(Serialize, Type)]` or `#[derive(Deserialize, Type)]` to all DTOs
- [ ] Add `#[specta::specta]` to all 37 commands
- [ ] Handle `Snippet` (re-export DTO or add Type to digicore-core)
- [ ] Add `export_bindings()` and call from build or run
- [ ] Create `src/lib/tauriInvoke.ts` with typed wrapper
- [ ] Migrate `App.tsx` invoke calls
- [ ] Migrate `LibraryTab.tsx` invoke calls
- [ ] Migrate `ConfigTab.tsx` invoke calls
- [ ] Migrate `ClipboardTab.tsx` invoke calls
- [ ] Migrate `ScriptTab.tsx` invoke calls
- [ ] Migrate `AnalyticsTab.tsx` invoke calls
- [ ] Migrate `LogTab.tsx` invoke calls
- [ ] Migrate `CommandPalette.tsx` invoke calls
- [ ] Run `npm run test` and `cargo test` to verify
- [ ] Document bindings generation in README / TAURI_USER_GUIDE

### 6.2 TauRPC (Phase 2, Optional)

- [ ] Add `taurpc`, `specta`, `tokio` to `Cargo.toml`
- [ ] Define `Api` trait with all procedures
- [ ] Add `#[taurpc::ipc_type]` to DTOs
- [ ] Implement `ApiImpl` with resolvers
- [ ] Wire `TauriAppState` and `AppHandle` into `ApiImpl`
- [ ] Replace `invoke_handler` with `taurpc::create_ipc_handler`
- [ ] Add `taurpc` npm package
- [ ] Replace `invokeTyped` with `createTauRPCProxy()` in React
- [ ] Handle ghost overlays (bundle / hybrid / keep invoke)
- [ ] Full regression test

---

## 7. Testing Strategy

1. **Unit tests:** Ensure Rust commands still pass `cargo test`.
2. **Frontend tests:** Vitest mocks for `invoke` / `taurpc`; update mocks to match new signatures.
3. **E2E / manual:** Load library, add/edit/delete snippets, use Ghost Suggestor/Follower, Variable Input, Clipboard, Config, Analytics, Log.
4. **Type check:** `npm run build` and `tsc --noEmit` must pass.

---

## 8. Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Specta export format differs from expected | Inspect generated `bindings.ts`; adjust wrapper or Specta config |
| `Snippet` from digicore-core lacks Type | Create `SnippetDto` in Tauri crate, convert at boundary |
| TauRPC async vs sync commands | Use trivial `async fn` that calls sync logic |
| Ghost overlays can't use TauRPC | Keep `invoke` for overlays; type safety only in main app |
| Breaking change for existing users | No user-facing change; internal refactor only |

---

## 9. References

- [Tauri Specta (docs.rs)](https://docs.rs/tauri-specta/latest/tauri_specta/)
- [TauRPC (lib.rs)](https://lib.rs/crates/taurpc)
- [Specta](https://github.com/oscartbeaumont/specta)
- [Tauri Commands](https://v2.tauri.app/develop/calling-rust/#commands)

---

## 10. Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-03-03 | — | Initial plan; Specta + TauRPC options; phased approach |
