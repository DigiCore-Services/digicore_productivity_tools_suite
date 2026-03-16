# Scripting Engine Library Review and Recommendations

## Purpose

This review documents the current scripting implementation status in DigiCore Tauri, identifies gaps (especially around multi-engine global libraries), and recommends a practical implementation roadmap to improve robustness, usability, and diagnostics.

## Current State (Implemented)

- **Registered script placeholders** include:
  - `{js:...}`
  - `{http:...}`
  - `{run:...}`
  - `{dsl:...}`
  - `{py:...}`
  - `{lua:...}`
  - `{weather:...}` (new)
- **JavaScript global library** is user-editable in the Scripting Engine Library tab and hot-reloaded.
- **Run security controls** are present in UI:
  - run disabled toggle
  - run allowlist
- **Weather placeholder** now supports:
  - city/country/state disambiguation
  - geocoding -> weather resolution
  - friendly weather text mapping
  - caching
- **Snippet test UX** now supports:
  - timeout
  - cancel test run
  - test-result cache for repeated runs
  - city suggestions for weather inputs

## Primary Gap Areas

1. **UI parity for non-JS global libraries**

- Current tab exposes only Global JavaScript Library editing.
- Python/Lua/DSL/HTTP/Run settings are not all represented as first-class editable sections.

1. **Cross-engine configuration discoverability**

- Some behavior is controlled by scripting config, but most users only see JS + run controls.
- Missing in-tab visibility for timeout/retry/debug settings per engine.

1. **Operational diagnostics visibility**

- Script errors are surfaced inline, but deeper execution metrics are not centralized in a scripting-focused diagnostics panel.

1. **Script testing breadth**

- Current test modal is strong for content simulation.
- No engine-specific test presets/templates in UI to help users verify py/lua/http quickly.

## Recommended Enhancements

## 1) Scripting Tab: Multi-Engine Sections

Add separate sections for:

- **Global JavaScript Library** (existing, keep)
- **Global Python Library** (new)
- **Global Lua Library** (new)
- **HTTP Engine Settings** (timeout, retries, allowlist, async toggle)
- **DSL Settings** (enabled, safe mode)
- **Run Settings** (existing, keep and expand descriptions)

Suggested UX:

- Shared layout and save workflow per section
- Per-section validation and status messages
- Inline examples for each engine

## 2) Add API Surface for Engine-Specific Libraries

Add backend TauRPC endpoints:

- `get_script_library_py() / save_script_library_py()`
- `get_script_library_lua() / save_script_library_lua()`
- Optional:
  - `get_scripting_config() / save_scripting_config()`

Behavior goals:

- Persist to configured script library paths
- Reload in-memory runtime immediately after save
- Return clear errors for invalid file path, parser issues, or runtime load failures

## 3) Better Diagnostics for Scripting

Add targeted diagnostic events:

- script type
- execution duration
- timeout/cancel reason
- error class and shortened message

Keep noisy internals at debug level; expose support-relevant entries to Log tab.

## 4) Improve Test Modal Reliability and Feedback

Already implemented:

- timeout
- cancel
- result cache

Recommended next:

- show timing badge (e.g., "Completed in 420ms")
- explicit network-attempt indicator for HTTP/weather
- optional "retry with extended timeout" action

## 5) Security and Hardening

- Keep `run` disabled by default.
- Add warning banners when enabling run/py/lua execution.
- Consider per-engine allowlist or policy toggles for production profiles.
- Validate and sanitize user-provided script library paths.

## 6) Documentation and Samples

Add quick-copy snippets per engine:

- JS utility usage
- HTTP JSON-path extraction
- Python and Lua sample wrappers
- Weather examples with and without interactive variables

## Proposed Implementation Sequence

1. Add API endpoints for Python/Lua global libraries.
2. Add UI sections for Python/Lua editing and save/reload.
3. Add scripting config read/write API and UI settings panels for HTTP/DSL/Run.
4. Add scripting diagnostics enrichment and log filtering.
5. Add sample snippet templates and in-tab docs/tooltips.
6. Add integration tests covering all script types with config permutations.

## Test Strategy (Recommended)

- **Unit tests**
  - parser/dispatch for all script prefixes
  - config validation and defaults
  - timeout and retry boundaries
- **Integration tests**
  - save + reload global library for JS/Python/Lua
  - script evaluation with real config values
  - weather and http fallback/error behavior
- **UI tests**
  - section render/save workflows
  - validation/error messaging
  - test modal result/cancel/timeout flow

## Immediate Conclusions

- The core scripting architecture already supports multi-engine execution.
- The major missing piece is **UI parity and user-visible configuration** for non-JS engines.
- The fastest high-value next step is to add Python/Lua global library editors and expose HTTP/DSL settings in the existing Scripting tab.

