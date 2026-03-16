# Changelog

All notable changes to the DigiCore project.

## [Unreleased]

### Added

- **Tauri enhancements (2026-03-03):**
  - Build script: `scripts/build.ps1` now uses `npm run tauri build` for full Tauri build (frontend + Rust + bundle)
  - Native window decorations: `decorations: true`; custom TitleBar removed to avoid dual header
  - SQLite: tauri-plugin-sql with categories/snippets schema; sync from JSON on Load/Save/startup
  - Web Workers: Fuzzy search (Fuse.js) in worker; CommandPalette off main thread
  - Rich notifications: Actionable toasts with "View Library" action
  - Accessibility: ARIA labels, tab roles, prefers-reduced-motion, prefers-contrast
  - [TAURI_USER_GUIDE.md](docs/digicore-text-expander/TAURI_USER_GUIDE.md) – Build, dev, SQLite sync, troubleshooting
  - **FileDialogPort:** tauri-plugin-dialog; Browse button in Library tab for native file picker (JSON library path)
  - **Phase 3 plugins:** tauri-plugin-prevent-default, positioner, persisted-scope, http
  - **SQLite partial loading:** loadSnippetsPage() in sqliteLoad.ts for large libraries
  - **Tests:** libraryUtils, sqliteSync, LibraryTab (Browse, formatLastModified, getCellValue)

- **Ghost Suggestor enhancements (AHK parity+):**
  - Configurable display duration (sec, 0=no auto-hide) in Configuration tab
  - Create Snippet, Ignore, Cancel buttons on overlay
  - WindowLevel::AlwaysOnTop for visibility over all apps
  - Create Snippet opens Snippet Editor with selected content

- **Clipboard History tab** - Real-time clipboard history with table (#, Content Preview, App, Window Title, Length)
- **Right-click context menu** on clipboard entries:
  - Copy to Clipboard
  - View Full Content
  - Delete Item
  - Promote to Snippet
  - Clear All History
- **View Full Content modal** - Read-only full content view with Promote to Snippet button
- **Delete confirmation dialog** - "Are you sure you want to delete this clipboard item?" with preview
- **Clear All confirmation dialog** - "Clear all clipboard history?"
- **Utils module** - `truncate_for_display()` for display truncation (moved from clipboard_history_tab)

### Changed

- Clipboard History max depth range extended to 5-100 (Configuration tab)
- `truncate_for_display` extracted to `digicore_text_expander::utils` for reuse
- `load_config()` no longer overwrites SCRIPTING_CONFIG when already set (fixes integration_run_allowed)

### Testing

- `test_delete_entry_at` - Clipboard history delete by index
- `test_delete_entry_at_out_of_bounds` - Out-of-bounds index handling
- `test_clear_all` - Clear all clipboard history
- `test_add_entry_with_metadata` - process_name and window_title stored correctly
- `test_update_config_max_depth_trims` - Reducing max_depth trims entries
- `test_update_config_disabled` - update_config disables and blocks add
- `test_suppress_for_duration_no_panic` - suppress_for_duration does not panic
- `test_truncate_for_display_short`, `_long`, `_edge` - Utils truncation
- Doc-tests for `truncate_for_display`
- Fixed `integration_run_allowed` - load_config no longer overwrites test-set config

**Test status (2026-02-28):** 90 unit + 30 integration tests passing in digicore-text-expander.
