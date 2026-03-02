# DigiCore Text Expander - Implementation Plan

**Version:** 1.0  
**Last Updated:** 2026-02-28  
**Product:** DigiCore Text Expander (Rust)

---

## Overview

This document tracks implementation status, recent integrations, enhancements, and testing coverage for the DigiCore Text Expander project.

---

## Recent Integrations and Enhancements (2026-02-28)

### Clipboard History (F38-F42)

| Feature | Implementation | Status |
|---------|----------------|--------|
| Real-time clipboard monitoring | `application/clipboard_history.rs` | Done |
| App and Window Title metadata | Windows clipboard listener (WM_CLIPBOARDUPDATE) | Done |
| Right-click context menu | `ui/clipboard_history_tab.rs` | Done |
| Copy to Clipboard | arboard::Clipboard::set_text | Done |
| View Full Content modal | `ui/modals.rs` clip_view_content_modal | Done |
| Delete Item with confirmation | `ui/modals.rs` clip_delete_confirm_dialog | Done |
| Promote to Snippet | `ui/modals.rs` promote_modal | Done |
| Clear All History with confirmation | `ui/modals.rs` clip_clear_confirm_dialog | Done |
| Max depth config (5-100) | ClipboardHistoryConfig | Done |

### Windows Clipboard Listener

| Component | Implementation | Status |
|-----------|----------------|--------|
| WM_CLIPBOARDUPDATE listener | `platform/windows_clipboard_listener.rs` | Done |
| AddClipboardFormatListener | Message-only window | Done |
| Foreground window context | WindowsWindowAdapter::get_active | Done |
| Fallback to poll loop | On listener failure | Done |
| Suppress during paste | suppress_for_duration | Done |

### Ghost Suggestor (F43-F47)

| Feature | AHK Parity | Status |
|---------|------------|--------|
| AlwaysOnTop overlay | +AlwaysOnTop | Done |
| Tab accept, Ctrl+Tab cycle | Tab/Ctrl+Tab | Done |
| Configurable display duration | New (0=no auto-hide) | Done |
| Create Snippet button | Enhancement | Done |
| Ignore button | HideGhost | Done |
| Cancel button | Esc | Done |
| Debounce, offset | ghostOverlayOffsetX/Y | Done |

### Modals

| Modal | Purpose | Status |
|-------|---------|--------|
| Promote to Snippet | Add clip content as snippet | Done |
| View Full Content | Read-only full content + Promote | Done |
| Delete confirmation | "Are you sure?" for single item | Done |
| Clear All confirmation | "Clear all clipboard history?" | Done |

---

## Implementation Details

### Clipboard History Architecture

- **On Windows:** Uses `AddClipboardFormatListener` with a message-only window. On `WM_CLIPBOARDUPDATE`, reads clipboard, queries foreground window via `WindowsWindowAdapter`, and calls `add_entry(text, process_name, window_title)`. AHK parity for App/Window Title.
- **On other platforms:** Poll loop (500ms) in background thread. App/Window Title remain empty.
- **Suppression:** `suppress_for_duration` prevents adding to history during our own paste (e.g. Copy to Clipboard from context menu). Checked in listener callback before `add_entry`.

### Config Loading (Scripting)

- `load_config()` no longer overwrites `SCRIPTING_CONFIG` if already set. Enables tests to call `set_scripting_config` before `process_with_config` without being overwritten by `get_registry()` -> `load_config()`.

---

## Testing Status

### Test Counts (2026-02-28)

| Crate | Unit Tests | Integration Tests | Doc-Tests | Total |
|-------|------------|------------------|-----------|-------|
| digicore-text-expander | 90 | 30 | 1 | 121 |

### Clipboard History Tests

| Test | Purpose |
|------|---------|
| `test_config_default` | ClipboardHistoryConfig defaults |
| `test_start_stop` | Start/stop lifecycle |
| `test_add_entry_dedup` | Deduplication of consecutive same content |
| `test_add_entry_max_depth` | Max depth trimming |
| `test_add_entry_with_metadata` | process_name and window_title stored correctly |
| `test_add_entry_when_disabled` | No add when disabled |
| `test_take_promote_pending_none` | take_promote_pending returns None when empty |
| `test_request_take_promote` | request_promote / take_promote_pending flow |
| `test_get_entries_when_stopped` | Empty when stopped |
| `test_delete_entry_at` | Delete by index |
| `test_delete_entry_at_out_of_bounds` | Out-of-range index no-op |
| `test_clear_all` | Clear all entries and last_content |
| `test_update_config_max_depth_trims` | Reducing max_depth trims entries |
| `test_update_config_disabled` | update_config disables and blocks add |
| `test_suppress_for_duration_no_panic` | suppress_for_duration does not panic |

### Utils Tests

| Test | Purpose |
|------|---------|
| `test_truncate_for_display_short` | Short strings unchanged |
| `test_truncate_for_display_long` | Long strings truncated with "..." |
| `test_truncate_for_display_edge` | Edge cases (exact length, zero) |

### Integration Tests

| Test | Purpose |
|------|---------|
| `integration_js_clipboard_clip_history` | {clip:1}, {clip:2} with clip history |
| `integration_run_allowed` | {run:cmd} when run.disabled=false, allowlist=cmd |
| `integration_js_and_clipboard` | {clipboard}, {js:} |
| `integration_full_template` | Combined placeholders |
| `integration_js_http_clipboard_with_mock` | HTTP mock + clipboard |

### Run Command

```bash
cargo test -p digicore-text-expander
```

---

## Known Limitations

- **Windows clipboard listener:** No explicit shutdown path; runs until process exit.
- **Non-Windows:** Poll loop used; App/Window Title remain empty.
- **Windows clipboard listener:** Not unit-tested (requires real message loop); verified manually.

---

## Related Documentation

- [UI Decoupling Implementation Plan (Phase 0/1)](UI_DECOUPLING_IMPLEMENTATION_PLAN.md) - Framework-agnostic ports (StoragePort, WindowPort), AppState extraction, feature flags for multi-GUI support
- [egui to Azul Migration Proposal](EGUI_TO_AZUL_MIGRATION_PROPOSAL.md)
- [Clipboard History Guide](CLIPBOARD_HISTORY.md)
- [Scripting User Guide](SCRIPTING_USER_GUIDE.md)
- [Changelog](../../CHANGELOG.md)
