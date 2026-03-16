# Settings Import/Export Implementation Plan

## Goal

Add a user-friendly **Import/Export Settings** section to the `Configurations and Settings` tab that supports:

- Export **all settings** or only **selected groups/categories**
- Import from JSON and apply either all or selected groups
- Team-sharing portability (consistent schema and validation)
- Graceful failures and visible diagnostics in the **Log** tab

Also add a new **Appearance** section in `Configurations and Settings` (above `Core`) with the note:

`NOTE: See 'Appearance' tab for detailed configurations and settings.`

---

## Audit Summary (Current State)

### Frontend (`ConfigTab`)

- `ConfigTab` currently has collapsible sections: Templates, Sync, Discovery, Ghost Suggestor, Ghost Follower, Clipboard History, Core, Updates.
- Core save path uses `update_config(...)` + `save_settings()` + `get_app_state()` refresh.
- `Appearance` is configured on a separate `Appearance` tab and persisted via dedicated Appearance APIs, not part of `AppState` payload in `ConfigTab`.
- `Theme` is currently stored via `localStorage` (`digicore-theme`) and `autostart` is controlled via plugin calls (not in backend persisted config keys).

### Backend (`api.rs`, `lib.rs`, storage keys)

- Config persistence is centralized through `persist_settings_to_storage(...)` and `storage_keys::*`.
- Appearance rules are persisted separately under `appearance_transparency_rules_json`.
- Diagnostics shown in Log tab come from `expansion_diagnostics` ring buffer, while some backend operations use `log::info!/warn!` only.

### Log Tab

- `LogTab` reads only `get_diagnostic_logs()` output (ring buffer), not arbitrary stdout logs.
- To ensure import/export troubleshooting is visible to users, import/export paths should also push entries into `expansion_diagnostics::push(...)`.

---

## UX + Placement Requirements

## Configurations and Settings tab layout updates

1. Add a new collapsed `Appearance` section **above** `Core`:
   - Text only (no complex controls duplicated from Appearance tab)
   - Exact note: `NOTE: See 'Appearance' tab for detailed configurations and settings.`

2. Add a new collapsed `Import/Export Settings` section **below `Core` and above `Updates`**:
   - Mode controls: `Export` / `Import`
   - Scope controls:
     - `All Settings`
     - `Selected Groups`
   - Group checklist (at minimum):
     - Templates
     - Sync
     - Discovery
     - Ghost Suggestor
     - Ghost Follower
     - Clipboard History
     - Core
     - Appearance
     - Script Runtime (allowlist + disable flag)
   - File actions:
     - Export: choose save location and write JSON
     - Import: choose file, preview summary, apply selected groups
   - Status messaging with counts and group names

---

## Data Contract (Portable JSON)

Use a versioned envelope so the schema can evolve safely.

```json
{
  "schema_version": "1.0.0",
  "exported_at_utc": "2026-03-05T18:00:00Z",
  "app": {
    "name": "DigiCore Text Expander",
    "format": "settings-bundle"
  },
  "selected_groups": ["ghost_follower", "appearance"],
  "groups": {
    "templates": { "template_date_format": "%Y-%m-%d", "template_time_format": "%H:%M" },
    "sync": { "sync_url": "..." },
    "discovery": { "...": "..." },
    "ghost_suggestor": { "...": "..." },
    "ghost_follower": { "...": "..." },
    "clipboard_history": { "clip_history_max_depth": 20 },
    "core": { "expansion_paused": false, "theme": "dark", "autostart": true },
    "script_runtime": { "script_library_run_disabled": false, "script_library_run_allowlist": "" },
    "appearance": { "rules": [{ "app_process": "cursor.exe", "opacity": 180, "enabled": true }] }
  }
}
```

Notes:

- `theme` and `autostart` are included in bundle for portability; import handler applies them with existing frontend/plugin behavior.
- Unknown groups are ignored with warning diagnostics (forward compatibility).

---

## Backend Design

Add IPC methods (suggested):

- `export_settings_bundle(selected_groups: Vec<String>) -> Result<String, String>`
  - Builds JSON string from persisted keys + current runtime config sources.
  - Pull Appearance from `appearance_transparency_rules_json`.
  - Validates group names and normalizes ordering.

- `import_settings_bundle(payload_json: String, selected_groups: Vec<String>) -> Result<ImportResultDto, String>`
  - Parses/validates schema.
  - Applies only selected groups (or all when selected list empty + mode=all).
  - Uses existing validation/clamping rules (same as `update_config`).
  - Persists safely.
  - Returns summary:
    - `applied_groups`
    - `skipped_groups`
    - `warnings`
    - `appearance_rules_count`

`ImportResultDto` (suggested):

- `applied_groups: string[]`
- `skipped_groups: string[]`
- `warnings: string[]`
- `updated_keys: number`
- `appearance_rules_applied: number`

---

## Frontend Flow

### Export

1. User chooses `All` or `Selected Groups`.
2. User picks save path via dialog.
3. App calls `export_settings_bundle(...)`.
4. Frontend writes returned JSON to selected file path.
5. Show success status with group count and file name.

### Import

1. User picks JSON file via dialog.
2. Frontend reads file contents.
3. Show import preview:
   - schema version
   - groups present
   - warnings (if any)
4. User chooses `All` or `Selected Groups` to apply.
5. App calls `import_settings_bundle(payload, selected_groups)`.
6. Refresh app state and Appearance data.
7. Show success summary (groups applied, warnings).

---

## Validation and Safety Rules

- Hard reject malformed JSON, missing `schema_version`, or invalid group payload types.
- Validate/clamp numeric values using existing rules:
  - ghost follower opacity, collapse delay, clip depth, discovery bounds, etc.
- Normalize process names for Appearance rules during import.
- Duplicate Appearance rule keys should follow deterministic priority behavior already implemented.
- Import should be **non-destructive by group**:
  - if user imports only `ghost_follower` + `appearance`, all other groups remain unchanged.

---

## Diagnostics and Logging Plan (Visible in Log Tab)

For each import/export operation, emit both:

1. `log::info!/warn!/error!` (system logs)
2. `expansion_diagnostics::push(level, message)` (user-visible Log tab)

Recommended messages:

- Export start/end with selected groups and output path
- Import start with file size and selected groups
- Parse failures and validation failures
- Per-group apply success/warnings
- Final summary counts

Example:

- `[info] settings_export_started groups=ghost_follower,appearance`
- `[info] settings_import_applied group=appearance rules=5`
- `[warn] settings_import_skipped group=sync reason=invalid_payload`
- `[info] settings_import_completed applied=2 skipped=1`

---

## Error Handling Strategy

- Dialog cancelled: no error state; status info only.
- File read/write failure: friendly status + diagnostic entry with exception detail.
- Schema mismatch: report expected vs actual version; skip apply.
- Partial import:
  - apply valid groups,
  - return warnings for invalid groups,
  - never fail entire operation unless payload is fundamentally invalid.

---

## Testing Plan

### Frontend (unit/integration-style)

- Section placement and headings:
  - `Appearance` note above `Core`
  - `Import/Export Settings` between `Core` and `Updates`
- Group multi-select behavior
- Export all vs selected groups
- Import preview rendering and apply action
- Cancel handling for dialogs
- Status message correctness

### Backend unit tests

- Export payload contains correct group keys and schema metadata
- Import applies only selected groups
- Import ignores unknown groups with warnings
- Numeric clamp validation on imported values
- Appearance rules import normalization and conflict ordering

### Negative/edge tests

- Invalid JSON
- Empty file
- Unsupported schema version
- Missing groups node
- Extremely large payload (bounded handling)

---

## Iterative Delivery Sequence

1. Add frontend sections and note placeholders (no side effects).
2. Add backend DTOs + import/export IPC contracts.
3. Implement export path (all + selected groups).
4. Implement import path with partial-group apply.
5. Add diagnostics pushes for all operations.
6. Add frontend/backend tests and stabilize.

---

## Out-of-Scope (for this slice)

- Encrypted settings bundles
- Cloud profile sync/merge conflict UI
- Secret vault handling for credentials beyond current scope

---

## Acceptance Criteria

- User can export all settings or selected groups to JSON.
- User can import all settings or selected groups from JSON.
- Importing only selected groups updates only those groups.
- `Appearance` section note is visible above `Core`.
- `Import/Export Settings` section is visible below `Core` and above `Updates`.
- Import/export errors are user-friendly and visible in `Log` tab diagnostics.
