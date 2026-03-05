# Appearance UX + Reliability TODO

This tracks the iterative roadmap for the Tauri `Appearance` tab and runtime transparency manager.

## In Progress / Completed Core

- [x] Add `Appearance` tab to Tauri management console.
- [x] Add transparency rule CRUD (save, list, delete) with persistence.
- [x] Add runtime transparency apply for matching Windows process windows.
- [x] Add background enforcement loop for newly launched apps.
- [x] Reduce flicker with idempotent alpha application and per-window cache.

## Next UX Enhancements

- [x] Add per-rule enabled toggle in table (without deleting rule).
- [x] Add explicit per-rule `Apply now` action and applied-window count feedback.
- [x] Add running process picker/autocomplete to reduce process-name entry errors.
- [x] Add rule conflict detection and deterministic priority ordering.
- [x] Add global `Restore all defaults` action to clear transparency from all managed apps.
- [x] Add import/export for appearance rules as JSON (handled via centralized Import/Export Settings in Configurations and Settings using Appearance group selection; no separate Appearance-tab duplicate path).

## Reliability / Regression Testing

- [x] Frontend unit tests for Appearance tab basic rendering and validation.
- [x] Frontend tests for rule loading/sorting/double-click fill.
- [x] Frontend tests for save/delete happy paths and negative cases.
- [x] Frontend tests for live slider apply behavior.
- [x] Backend unit tests for process name matching edge cases.
- [x] Integration tests for end-to-end rule persistence and startup enforcement.
- [x] Stress tests for many open windows/processes with rapid enforcement cycles.
- [x] Add CI target to run Appearance-focused tests on Windows worker.

## Notes

- Current runtime application behavior is Windows-only; non-Windows remains safe no-op.
- Keep storage key schema backwards-compatible: `appearance_transparency_rules_json`.
