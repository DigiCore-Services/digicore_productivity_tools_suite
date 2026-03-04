# TauRPC Full Refactor Implementation Plan

**DigiCore Text Expander – Type-Safe IPC Migration**

**Version:** 2.1  
**Last Updated:** 2026-03-04  
**Status:** Implemented  
**Purpose:** Comprehensive implementation plan for migrating from string-based Tauri `invoke()` to TauRPC. Serves as a living document for progress tracking and future developer reference.

---

## 1. Executive Summary

### 1.1 Current Problem

The app uses **string-based IPC**: `invoke("command_name", { args })` with manual TypeScript types and casts. This causes:

- No compile-time validation of command names or argument shapes
- Manual interfaces in `src/types.ts` that can drift from Rust
- Runtime errors when commands or payloads change
- Typos in command names (e.g. `get_app_stae`) only surface at runtime

### 1.2 Solution: TauRPC

TauRPC replaces Tauri's invoke handler with a **trait-based API** and generates a **typed TypeScript proxy**. The frontend calls `await taurpc.get_app_state()` instead of `invoke("get_app_state")` with full end-to-end type safety.

### 1.3 Critical Constraint

**TauRPC completely replaces the invoke handler.** All IPC goes through TauRPC's protocol. The old `invoke("command_name", args)` will **not work** after migration. Every call site—including ghost overlays—must use the TauRPC proxy.

### 1.4 Out of Scope

- **tauri-plugin-sql**: `sqliteLoad.ts` and `sqliteSync.ts` use `Database.load()` and `db.select()` directly. These are plugin APIs, not Tauri commands. No migration needed.
- **Tauri events**: `listen()`, `emit()`, `emitTo()` remain unchanged. TauRPC can optionally add typed events later.

---

## 2. Quick Start (For Developers Resuming This Work)

1. Read Section 1 (Executive Summary) and Section 4 (Current State Audit).
2. Open Section 3 (Implementation Progress Tracker) and identify the next unchecked task.
3. Follow the corresponding section (5 Backend, 6 Frontend, 7 Ghost Overlays).
4. After each task, update the progress tracker checkbox.
5. Run tests after each phase (Section 9).

---

## 3. Implementation Progress Tracker

Use this section to track progress. Update checkboxes as work completes.

### 3.1 Phase 1: Backend Foundation

| # | Task | Status | Notes |
|---|------|--------|-------|
| 1.1 | Add TauRPC, Specta, Tokio to Cargo.toml | [x] | Pin Specta to `=2.0.0-rc.22` per TauRPC |
| 1.2 | Create SnippetDto (or add Type to digicore-core Snippet) | [x] | Snippet from digicore-core has serde; add specta/Type |
| 1.3 | Add #[taurpc::ipc_type] to all DTOs | [x] | See Section 4.2 |
| 1.4 | Define Api trait with all 37 procedures | [x] | See Section 4.3 |
| 1.5 | Implement ApiImpl with resolvers | [x] | Extract logic from current commands |
| 1.6 | Wire TauriAppState + AppHandle into ApiImpl | [x] | Pass in setup; use Arc<Mutex<AppState>> |
| 1.7 | Replace invoke_handler with taurpc::create_ipc_handler | [x] | Remove tauri::generate_handler![] |
| 1.8 | Remove #[tauri::command] and old command fns | [x] | Logic moved to resolvers |
| 1.9 | Run `cargo build` and `cargo test` | [x] | Backend must compile and pass tests |

### 3.2 Phase 2: Frontend – Main React App

| # | Task | Status | Notes |
|---|------|--------|-------|
| 2.1 | Add `taurpc` npm package | [x] | `pnpm add taurpc` |
| 2.2 | Run `pnpm tauri dev` to generate bindings | [x] | Types generated on first run |
| 2.3 | Create src/lib/taurpc.ts proxy singleton | [x] | getTaurpc() wrapping createTauRPCProxy() |
| 2.4 | Migrate App.tsx invoke calls | [x] | 12 call sites |
| 2.5 | Migrate LibraryTab.tsx invoke calls | [x] | 7 call sites |
| 2.6 | Migrate ConfigTab.tsx invoke calls | [x] | 2 call sites |
| 2.7 | Migrate ClipboardTab.tsx invoke calls | [x] | 3 call sites |
| 2.8 | Migrate ScriptTab.tsx invoke calls | [x] | 5 call sites |
| 2.9 | Migrate AnalyticsTab.tsx invoke calls | [x] | 2 call sites |
| 2.10 | Migrate LogTab.tsx invoke calls | [x] | 2 call sites |
| 2.11 | Migrate CommandPalette.tsx invoke calls | [x] | 1 call site |
| 2.12 | Remove or deprecate src/types.ts manual interfaces | [x] | normalizeState.ts converts DTOs; types.ts kept for UI types |
| 2.13 | Update Vitest mocks for taurpc | [x] | Mock getTaurpc in LibraryTab.test, LogTab.test |
| 2.14 | Run `npm run test` and `npm run build` | [x] | Frontend must pass |

### 3.3 Phase 3: Ghost Overlays

| # | Task | Status | Notes |
|---|------|--------|-------|
| 3.1 | Choose overlay strategy (bundle vs. script tag) | [x] | Vite multi-page (Option A) |
| 3.2 | Migrate ghost-follower.html to TauRPC proxy | [x] | 9 invoke call sites; src/ghost-follower.ts |
| 3.3 | Migrate ghost-suggestor.html to TauRPC proxy | [x] | 5 invoke call sites; src/ghost-suggestor.ts |
| 3.4 | Verify overlays load bindings correctly | [x] | Build outputs dist/ghost-follower.html, dist/ghost-suggestor.html |
| 3.5 | Manual test: Ghost Suggestor + Follower flows | [ ] | Recommended before release |

### 3.4 Phase 4: Verification & Documentation

| # | Task | Status | Notes |
|---|------|--------|-------|
| 4.1 | Full regression: Library, Config, Clipboard, Script, Analytics, Log | [ ] | Manual; recommended before release |
| 4.2 | Full regression: Variable Input (F11), Command Palette | [ ] | Manual; recommended before release |
| 4.3 | Full regression: Deep links, tray menu, single-instance | [ ] | Manual; recommended before release |
| 4.4 | Update TAURI_USER_GUIDE.md with TauRPC notes | [x] | See Section 6 Key Features |
| 4.5 | Update TAURI_IMPLEMENTATION_STATUS.md | [x] | Mark Type-safe IPC Done |

---

## 3.5 Implementation Complete (2026-03-04)

All code migration is complete. Zero `invoke()` calls remain in the codebase.

| Phase | Status | Summary |
|-------|--------|---------|
| **Phase 1** | Done | Api trait, ApiImpl, DTOs, taurpc::create_ipc_handler; bindings to src/bindings.ts |
| **Phase 2** | Done | getTaurpc(); App, LibraryTab, ConfigTab, ClipboardTab, ScriptTab, AnalyticsTab, LogTab, CommandPalette |
| **Phase 3** | Done | Vite multi-page; ghost-follower.ts, ghost-suggestor.ts; HTML in project root |
| **Phase 4** | Partial | Docs updated; manual regression (4.1–4.3) recommended before release |

**Deferred:** None.  
**Cancelled:** None.

**Proposed next steps:**
1. Manual regression: Library, Config, Clipboard, Script, Analytics, Log, Variable Input, Command Palette, Ghost Suggestor/Follower, deep links, tray, single-instance
2. Optional: Add E2E tests (Playwright/WebDriver) for critical flows
3. Optional: Run `tauri signer generate` and configure updater pubkey for production

---

## 4. Current State Audit

### 4.1 Backend Commands (37 total)

All in `digicore/tauri-app/src-tauri/src/lib.rs`. Registered via `tauri::generate_handler![]`.

| Category | Command | Args | Returns | Needs AppHandle |
|----------|---------|------|---------|-----------------|
| **App** | greet | name: String | String | No |
| **App** | get_app_state | — | Result<AppStateDto, String> | No |
| **App** | load_library | — | Result<usize, String> | Yes (emit ghost-follower-update) |
| **App** | save_library | — | Result<(), String> | No |
| **App** | set_library_path | path: String | Result<(), String> | No |
| **App** | save_settings | — | Result<(), String> | No |
| **UI** | get_ui_prefs | — | Result<UiPrefsDto, String> | No |
| **UI** | save_ui_prefs | last_tab, column_order | Result<(), String> | No |
| **Snippets** | add_snippet | category, snippet | Result<(), String> | Yes |
| **Snippets** | update_snippet | category, snippet_idx, snippet | Result<(), String> | Yes |
| **Snippets** | delete_snippet | category, snippet_idx | Result<(), String> | Yes |
| **Config** | update_config | config: ConfigUpdateDto | Result<(), String> | Yes |
| **Ghost Suggestor** | get_ghost_suggestor_state | — | Result<GhostSuggestorStateDto, String> | No |
| **Ghost Suggestor** | ghost_suggestor_accept | — | Result<Option<(String,String)>, String> | No |
| **Ghost Suggestor** | ghost_suggestor_dismiss | — | Result<(), String> | No |
| **Ghost Suggestor** | ghost_suggestor_create_snippet | — | Result<Option<(String,String)>, String> | No |
| **Ghost Suggestor** | ghost_suggestor_cycle_forward | — | Result<usize, String> | No |
| **Ghost Follower** | get_ghost_follower_state | search_filter: Option<String> | Result<GhostFollowerStateDto, String> | No |
| **Ghost Follower** | ghost_follower_insert | trigger, content | Result<(), String> | No |
| **Ghost Follower** | ghost_follower_set_search | filter, — | Result<(), String> | Yes |
| **Ghost Follower** | ghost_follower_request_view_full | content, — | Result<(), String> | Yes |
| **Ghost Follower** | ghost_follower_request_edit | category, snippet_idx, — | Result<(), String> | Yes |
| **Ghost Follower** | ghost_follower_request_promote | content, trigger, — | Result<(), String> | Yes |
| **Ghost Follower** | ghost_follower_toggle_pin | category, snippet_idx, — | Result<(), String> | Yes |
| **Variable** | get_pending_variable_input | — | Result<Option<PendingVariableInputDto>, String> | No |
| **Variable** | submit_variable_input | values: HashMap<String,String> | Result<(), String> | No |
| **Variable** | cancel_variable_input | — | Result<(), String> | No |
| **Analytics** | get_expansion_stats | — | Result<ExpansionStatsDto, String> | No |
| **Analytics** | reset_expansion_stats | — | Result<(), String> | No |
| **Log** | get_diagnostic_logs | — | Result<Vec<DiagnosticEntryDto>, String> | No |
| **Log** | clear_diagnostic_logs | — | Result<(), String> | No |
| **Clipboard** | get_clipboard_entries | — | Result<Vec<ClipEntryDto>, String> | No |
| **Clipboard** | delete_clip_entry | index: usize | Result<(), String> | No |
| **Clipboard** | clear_clipboard_history | — | Result<(), String> | No |
| **Clipboard** | copy_to_clipboard | text: String | Result<(), String> | No |
| **Script** | get_script_library_js | — | Result<String, String> | No |
| **Script** | save_script_library_js | content: String | Result<(), String> | No |

### 4.2 DTOs – Full Inventory

| DTO | Location | Serde | TauRPC ipc_type | Notes |
|-----|----------|-------|-----------------|-------|
| AppStateDto | lib.rs | Serialize | Add | 25 fields |
| UiPrefsDto | lib.rs | Serialize | Add | last_tab, column_order |
| ConfigUpdateDto | lib.rs | Deserialize | Add | 24 Option fields |
| Snippet | digicore-core | Serialize+Deserialize | Add in Tauri or core | trigger, content, options, category, profile, app_lock, pinned, last_modified |
| GhostSuggestorStateDto | lib.rs | Serialize | Add | has_suggestions, suggestions, selected_index, position, should_passthrough |
| SuggestionDto | lib.rs | Serialize | Add | trigger, content_preview, category |
| GhostFollowerStateDto | lib.rs | Serialize | Add | enabled, pinned, search_filter, position, edge_right, monitor_primary |
| PinnedSnippetDto | lib.rs | Serialize | Add | trigger, content, content_preview, category, snippet_idx |
| PendingVariableInputDto | lib.rs | Serialize | Add | content, vars, values, choice_indices, checkbox_checked |
| InteractiveVarDto | lib.rs | Serialize | Add | tag, label, var_type, options |
| ExpansionStatsDto | lib.rs | Serialize | Add | total_expansions, total_chars_saved, estimated_time_saved_secs, top_triggers |
| DiagnosticEntryDto | lib.rs | Serialize | Add | timestamp_ms, level, message |
| ClipEntryDto | lib.rs | Serialize | Add | content, process_name, window_title, length |

**Special types:**
- `HashMap<String, String>` – TauRPC/Specta supports; maps to `Record<string, string>`
- `Option<(i32, i32)>` – position tuple; verify Specta export
- `Option<(String, String)>` – ghost_suggestor_accept return; verify
- `Vec<(String, u64)>` – top_triggers; verify

### 4.3 Frontend Invoke Call Sites – Exact Count

| File | Commands | Count |
|------|----------|-------|
| App.tsx | get_app_state, add_snippet, update_snippet, delete_snippet, save_library, save_settings, clear_clipboard_history, save_ui_prefs, submit_variable_input, cancel_variable_input, get_pending_variable_input, get_ui_prefs | 12 |
| LibraryTab.tsx | get_app_state, set_library_path, load_library, save_settings, update_snippet, save_library, copy_to_clipboard | 7 |
| ConfigTab.tsx | update_config, save_settings | 2 |
| ClipboardTab.tsx | get_clipboard_entries, copy_to_clipboard, delete_clip_entry | 3 |
| ScriptTab.tsx | get_app_state, get_script_library_js, update_config, save_settings, save_script_library_js | 5 |
| AnalyticsTab.tsx | get_expansion_stats, reset_expansion_stats | 2 |
| LogTab.tsx | get_diagnostic_logs, clear_diagnostic_logs | 2 |
| CommandPalette.tsx | copy_to_clipboard | 1 |
| ghost-follower.html | ghost_follower_insert, ghost_follower_request_view_full, ghost_follower_toggle_pin, ghost_follower_request_edit, copy_to_clipboard, delete_snippet, save_library, ghost_follower_set_search, get_ghost_follower_state, get_clipboard_entries, delete_clip_entry, ghost_follower_request_promote | 12 (with duplicates) |
| ghost-suggestor.html | ghost_suggestor_cycle_forward, get_ghost_suggestor_state, ghost_suggestor_accept, ghost_suggestor_create_snippet, ghost_suggestor_dismiss | 5 |

### 4.4 Events (Unchanged by TauRPC)

Backend emits; frontend listens. These remain Tauri events. No TauRPC migration.

| Event | Emitter | Listener | Payload |
|-------|---------|----------|---------|
| show-command-palette | global-shortcut handler | App.tsx | () |
| show-variable-input | global-shortcut handler | App.tsx | () |
| tray-add-snippet | tray menu | App.tsx | () |
| ghost-follower-update | commands, tray | ghost-follower.html, App.tsx | () |
| ghost-follower-view-full | ghost_follower_request_view_full | App.tsx | content: string |
| ghost-follower-edit | ghost_follower_request_edit | App.tsx | { category, snippetIdx } |
| ghost-follower-promote | ghost_follower_request_promote | App.tsx | { content, trigger } |
| initial-cli-args | setup | App.tsx | args: string[] |
| secondary-instance-args | single-instance | App.tsx | args |
| notification-view-library | notifications | App.tsx | — |
| ghost-suggestor-update | ghost_suggestor callback | (suggestor overlay) | () |

---

## 5. Backend Implementation Details

### 5.1 Dependencies

```toml
# digicore/tauri-app/src-tauri/Cargo.toml
[dependencies]
taurpc = "0.7"
specta = { version = "=2.0.0-rc.22", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
# ... existing deps unchanged
```

**Note:** TauRPC requires Specta pinned. Check [TauRPC lib.rs](https://lib.rs/crates/taurpc) for current compatible Specta version.

### 5.2 DTO Annotations

```rust
#[taurpc::ipc_type]
#[derive(Clone, serde::Serialize)]  // or Deserialize for input DTOs
pub struct AppStateDto { ... }

#[taurpc::ipc_type]
#[derive(Clone, serde::Deserialize)]
pub struct ConfigUpdateDto { ... }
```

**Snippet:** `digicore-core::domain::Snippet` has `serde`. Add `specta::Type` in digicore-core (if feasible) or create `SnippetDto` in Tauri crate and convert at boundary.

### 5.3 Api Trait Structure

Use `#[taurpc::procedures(export_to = "../src/bindings.ts")]` to auto-generate TypeScript. Procedure names become camelCase on frontend by default (e.g. `get_app_state` → `getAppState` or stays `get_app_state`—verify TauRPC convention).

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
    async fn get_clipboard_entries() -> Result<Vec<ClipEntryDto>, String>;
    async fn delete_clip_entry(index: usize) -> Result<(), String>;
    async fn clear_clipboard_history() -> Result<(), String>;
    async fn copy_to_clipboard(text: String) -> Result<(), String>;
    async fn get_script_library_js() -> Result<String, String>;
    async fn save_script_library_js(content: String) -> Result<(), String>;
    async fn get_ghost_suggestor_state() -> Result<GhostSuggestorStateDto, String>;
    async fn ghost_suggestor_accept() -> Result<Option<(String, String)>, String>;
    async fn ghost_suggestor_dismiss() -> Result<(), String>;
    async fn ghost_suggestor_create_snippet() -> Result<Option<(String, String)>, String>;
    async fn ghost_suggestor_cycle_forward() -> Result<usize, String>;
    async fn get_ghost_follower_state(search_filter: Option<String>) -> Result<GhostFollowerStateDto, String>;
    async fn ghost_follower_insert(trigger: String, content: String) -> Result<(), String>;
    async fn ghost_follower_set_search(filter: String) -> Result<(), String>;
    async fn ghost_follower_request_view_full(content: String) -> Result<(), String>;
    async fn ghost_follower_request_edit(category: String, snippet_idx: usize) -> Result<(), String>;
    async fn ghost_follower_request_promote(content: String, trigger: String) -> Result<(), String>;
    async fn ghost_follower_toggle_pin(category: String, snippet_idx: usize) -> Result<(), String>;
    async fn get_pending_variable_input() -> Result<Option<PendingVariableInputDto>, String>;
    async fn submit_variable_input(values: HashMap<String, String>) -> Result<(), String>;
    async fn cancel_variable_input() -> Result<(), String>;
    async fn get_expansion_stats() -> Result<ExpansionStatsDto, String>;
    async fn reset_expansion_stats() -> Result<(), String>;
    async fn get_diagnostic_logs() -> Result<Vec<DiagnosticEntryDto>, String>;
    async fn clear_diagnostic_logs() -> Result<(), String>;
}
```

### 5.4 ApiImpl – State and AppHandle

```rust
#[derive(Clone)]
struct ApiImpl {
    state: Arc<Mutex<AppState>>,
    app: AppHandle,
}

#[taurpc::resolvers]
impl Api for ApiImpl {
    async fn get_app_state(self) -> Result<AppStateDto, String> {
        let guard = self.state.lock().map_err(|e| e.to_string())?;
        Ok(app_state_to_dto(&guard))
    }
    async fn load_library(self) -> Result<usize, String> {
        let mut guard = self.state.lock().map_err(|e| e.to_string())?;
        let count = guard.try_load_library().map_err(|e| e.to_string())?;
        update_library(guard.library.clone());
        let _ = self.app.emit("ghost-follower-update", ());
        Ok(count)
    }
    // ... all other resolvers; call existing logic, use self.app.emit where needed
}
```

### 5.5 Async Wrapper Pattern

Current commands are sync. TauRPC procedures are async. Use:

```rust
async fn load_library(self) -> Result<usize, String> {
    let mut guard = self.state.lock().map_err(|e| e.to_string())?;
    let count = guard.try_load_library().map_err(|e| e.to_string())?;
    update_library(guard.library.clone());
    let _ = self.app.emit("ghost-follower-update", ());
    Ok(count)
}
```

No `spawn_blocking` needed for fast, non-blocking logic. Mutex::lock is synchronous; keep it. If any command does heavy I/O, consider `tokio::task::spawn_blocking`.

### 5.6 Handler Registration

Replace:

```rust
.invoke_handler(tauri::generate_handler![...])
```

With:

```rust
.invoke_handler(taurpc::create_ipc_handler(
    ApiImpl {
        state: Arc::new(Mutex::new(app_state)),
        app: app.handle().clone(),
    }
    .into_handler(),
))
```

**Critical:** `app_state` must be moved into `ApiImpl`. The current `.manage(TauriAppState(...))` will be removed. Create `Arc<Mutex<AppState>>` from `app_state` before `tauri::Builder` and pass into `ApiImpl`. TauRPC does not use Tauri's `State<T>`.

### 5.7 Build and Type Generation

TauRPC generates TypeScript when the app runs. Run `pnpm tauri dev` to produce `src/bindings.ts`. The `export_to` path is relative to the crate root (`src-tauri/`), so `../src/bindings.ts` resolves to `tauri-app/src/bindings.ts`.

---

## 6. Frontend Implementation Details

### 6.1 Proxy Singleton

```typescript
// src/lib/taurpc.ts
import { createTauRPCProxy } from "@/bindings";

export const taurpc = createTauRPCProxy();
```

Use `taurpc` everywhere instead of `invoke`.

### 6.2 Migration Pattern

```typescript
// Before
const state = (await invoke("get_app_state")) as AppState;

// After
const state = await taurpc.get_app_state();
```

**Naming:** TauRPC proxy method names typically match Rust (snake_case). Verify generated `bindings.ts` for exact names. Use `#[taurpc(alias = "camelCaseName")]` if frontend convention differs.

### 6.3 Argument Shape

Tauri invoke uses `{ argName: value }`. TauRPC uses direct arguments. Example:

```typescript
// Before
await invoke("update_snippet", { category, snippetIdx: idx, snippet });

// After
await taurpc.update_snippet(category, idx, snippet);
```

Verify generated bindings for exact signatures.

### 6.4 Error Handling

TauRPC returns `Result<T, String>`. On error, the promise rejects. Existing `.catch()` patterns remain valid.

### 6.5 Vitest Mocks

```typescript
vi.mock("@/lib/taurpc", () => ({
  taurpc: {
    get_app_state: vi.fn().mockResolvedValue(mockAppState),
    load_library: vi.fn().mockResolvedValue(5),
    // ...
  },
}));
```

---

## 7. Ghost Overlay Strategy

### 7.1 Constraint

Ghost overlays (`ghost-follower.html`, `ghost-suggestor.html`) are loaded as standalone HTML in WebviewWindows. Each has its own JS context. They currently use `window.__TAURI__.core.invoke`. After TauRPC migration, `invoke` will not route to our commands—TauRPC uses a different IPC protocol. **Overlays must use the TauRPC proxy.**

### 7.2 Option A: Vite Multi-Page / Entry Points

Add overlay HTML as Vite entry points so they are bundled with the TauRPC bindings.

- `vite.config.ts`: Add `ghost-follower.html` and `ghost-suggestor.html` as input entries
- Each overlay gets its own bundle including `createTauRPCProxy` and bindings
- Output: `ghost-follower.html` + `ghost-follower-[hash].js`, etc.
- Update `tauri.conf.json` to load the built overlay paths

**Pros:** Full type safety, single build. **Cons:** Config changes, path updates.

### 7.3 Option B: Script Tag for Bindings

Keep overlays as static HTML but inject a script that loads the generated bindings.

- Build a small `taurpc-overlay.js` that imports `createTauRPCProxy` from bindings and assigns to `window.__taurpc`
- Overlays load this script before their own logic
- Overlays call `window.__taurpc.get_ghost_follower_state(...)` etc.

**Pros:** Minimal overlay changes. **Cons:** Need to ensure script loads from correct path (built assets).

### 7.4 Option C: Convert Overlays to React Components

Render overlays as React components inside WebviewWindows, sharing the main app's bundle.

- Create `GhostFollowerOverlay.tsx`, `GhostSuggestorOverlay.tsx`
- Load them in WebviewWindows via a data URL or separate HTML that loads the main bundle
- They import `taurpc` from `@/lib/taurpc`

**Pros:** Full type safety, shared code. **Cons:** Larger refactor, bundle size.

### 7.5 Recommendation

**Option A (Vite multi-page)** is the most straightforward: overlays become first-class build outputs with full access to bindings. Document the choice in this plan when decided.

---

## 8. Capabilities and Permissions

TauRPC uses Tauri's IPC layer. The default capability grants `core:default` which includes invoke. TauRPC's IPC may use a different internal command (e.g. `__taurpc__`). Verify that:

- `core:default` or equivalent allows TauRPC traffic
- No additional capability needed for TauRPC

If TauRPC requires explicit permissions, add them to `default.json` and `ghost-windows.json` (for overlay windows).

---

## 9. Testing Strategy

### 9.1 Backend

- `cargo test -p digicore-text-expander-tauri` – unit tests for resolvers if extracted
- `cargo test -p digicore-text-expander` – core lib tests unchanged
- Manual: `pnpm tauri dev` – app starts, no console errors

### 9.2 Frontend

- `npm run test` – Vitest with mocked taurpc
- `npm run build` – TypeScript compiles, no type errors
- Update `LibraryTab.test.tsx`, `sqliteLoad.test.ts`, etc. if they mock invoke

### 9.3 E2E / Manual

- Library: Load, Save, Add, Edit, Delete, Search, Sort
- Config: Update settings, save
- Clipboard: Copy, Delete, Promote
- Script: Load, Save JS library
- Analytics: View stats, Reset
- Log: View logs, Clear
- Variable Input: F11, submit, cancel
- Command Palette: Shift+Alt+Space, search, Enter, Ctrl+E
- Ghost Suggestor: Type trigger, Accept, Create, Dismiss
- Ghost Follower: Insert, View Full, Edit, Promote, Toggle Pin, context menus
- Deep links, tray menu, single-instance

---

## 10. Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| TauRPC replaces invoke; overlays break | Implement overlay strategy (Section 6) before or with main app migration |
| Snippet from digicore-core lacks Type | Add specta to digicore-core or create SnippetDto in Tauri crate |
| Specta/TauRPC version incompatibility | Pin Specta per TauRPC docs; test on first integration |
| HashMap, Option, tuples not supported | Verify Specta export for each; use wrapper types if needed |
| greet uses `&str` | Change to `name: String` for IPC (TauRPC uses owned types) |
| Procedures need Window/WebviewWindow | TauRPC supports these as args; use if needed for window-specific logic |

---

## 11. Effort Estimate

| Phase | Tasks | Estimate |
|-------|-------|----------|
| Phase 1: Backend | Dependencies, DTOs, trait, resolvers, handler | 6–8 h |
| Phase 2: Frontend React | Proxy, migrate 7 components, mocks | 4–6 h |
| Phase 3: Ghost Overlays | Strategy + migration | 3–5 h |
| Phase 4: Verification | Regression, docs | 2–3 h |
| **Total** | | **15–22 h** |

---

## 11. References

- [TauRPC (lib.rs)](https://lib.rs/crates/taurpc)
- [TauRPC (GitHub)](https://github.com/MatsDK/TauRPC)
- [TauRPC Example](https://github.com/MatsDK/TauRPC/tree/main/example)
- [Specta](https://github.com/oscartbeaumont/specta)
- [Tauri Commands](https://v2.tauri.app/develop/calling-rust/#commands)

---

## 13. Document History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2026-03-03 | Initial plan; Specta + TauRPC options |
| 2.0 | 2026-03-03 | Full TauRPC refactor focus; progress tracker; audit; overlay strategy; living document |
