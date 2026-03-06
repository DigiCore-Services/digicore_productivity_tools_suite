# Copy-to-Clipboard SQLite Migration Audit

## Scope Delivered

- Added persistent `clipboard_history` storage in SQLite with migration version `2`.
- Added backend repository adapter for clipboard CRUD/search/trim/count operations.
- Wired runtime clipboard capture to persist through a storage-filter pipeline.
- Added new Management Console tab: `Copy-to-Clipboard`.
- Added Config tab subsection: `Copy-to-Clipboard`.
- Added JSON-only control flag (`json_output_enabled`) and disabled TXT-centric pathing in this feature area.

## Storage and Data Model

- DB file: `digicore.db` under DigiCore config directory.
- Table: `clipboard_history`
  - `id` (PK autoincrement)
  - `content`
  - `process_name`
  - `window_title`
  - `char_count`
  - `word_count`
  - `content_hash`
  - `created_at_unix_ms`
- Indexes:
  - `idx_clipboard_history_created_at`
  - `idx_clipboard_history_content_hash`

## Runtime Behavior

- Clipboard capture still uses existing listener path.
- New observer hook persists accepted entries to SQLite.
- Filtering/guardrails applied before insert:
  - enabled toggle
  - json output enabled toggle
  - min log length
  - process blacklist
  - optional masking (`cc`, `ssn`, `email`)
- Dedup via latest hash comparison.
- Post-insert max-depth trim enforced.

## API Surface Added

- `search_clipboard_entries(search, limit)`
- `delete_clip_entry_by_id(id)`
- `get_copy_to_clipboard_config()`
- `save_copy_to_clipboard_config(config)`
- `get_copy_to_clipboard_stats()`

Compatibility retained:

- `get_clipboard_entries()`
- `delete_clip_entry(index)` (legacy wrapper path)
- `clear_clipboard_history()`

## UX Changes

- New tab: `Copy-to-Clipboard`
  - search/filter
  - persisted list view
  - copy/view/delete actions
  - clear-all action
  - inline config controls
  - total persisted count
- Existing `Clipboard History` tab now receives SQLite-backed rows through API.
- Configurations tab now includes dedicated `Copy-to-Clipboard` subsection.

## Diagnostics

- Added structured clipboard diagnostics in capture and command paths:
  - accepted/skipped capture events
  - persistence write errors
  - delete/clear/config/copy events

## Validation and Guardrails

- `min_log_length`: clamped `1..2000`
- `max_history_entries`: `0 = Unlimited`, otherwise any positive integer
- blacklist parsing is normalized and case-insensitive

## Testing and Verification

Executed successfully:

- `cargo check` in `tauri-app/src-tauri`
- `npm run build` in `tauri-app`
- `npm run test -- ConfigTab CopyToClipboardTab` in `tauri-app`

Known environment issue:

- `cargo test -p digicore-text-expander-tauri --lib` currently fails to execute test binary with Windows OS lock (`Access is denied`), consistent with prior lock behavior in this workspace.

## Risks and Mitigations

- High clipboard event frequency:
  - mitigated with lightweight insert and bounded trim.
- Long-term DB growth:
  - mitigated by max-entry clamp and trim policy; archival/purge is next phase.
- PII leak risk:
  - mitigated by optional masking toggles and process blacklist.

## Follow-up Recommendations

- Add archival table + scheduled purge policy (age + size based).
- Add explicit whitelist mode in addition to blacklist.
- Add detached JSON export command from DB snapshots for audit handoff.
- Add Windows integration test lane that runs when clipboard listeners are not locked by live process.

