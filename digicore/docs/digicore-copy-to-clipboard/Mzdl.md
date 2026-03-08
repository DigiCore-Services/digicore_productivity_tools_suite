# Implementation Plan - Phase 52: "One-Click" Corpus Generation Utility

This phase introduces a friction-free way to expand our regression test suite. When developers encounter a problematic document or a new format, they can simply capture it to their clipboard and hit a global hotkey to automatically ingest it into the corpus.

## Proposed Changes

### [digicore-text-expander]

#### [MODIFY] [windows_keyboard.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/src/platform/windows_keyboard.rs)
- Ensure `is_shift_pressed` and `is_alt_pressed` helpers exist (or add them alongside `is_ctrl_pressed`).

#### [MODIFY] [hotstring.rs (Keyboard Hook)](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/src/drivers/hotstring.rs)
- **Hotkey Listener**: Intercept a chosen developer hotkey (e.g., `Ctrl+Alt+Shift+S`) inside `on_key`.
- **Action**: When triggered, execute a headless capture utility that:
    - Reads the current image from the clipboard (using `arboard` if possible) or triggers a screenshot.
    - Prompts the user (via OS toast) for a short name, or auto-generates a timestamp-based filename.
    - Saves the image to `docs/sample-ocr-images/<timestamp>.png`.

#### [NEW] [corpus_generator.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/crates/digicore-text-expander/src/application/corpus_generator.rs)
- **Capture Logic**: Create a new application service that handles the ingestion.
- **Baseline Auto-Generation**: Immediately run the Windows OCR adapter against the new image.
    - Save the output as a `.snap` file in `tests/snapshots` to establish the initial "Golden Master" baseline.
- **Notification**: Show a Windows Toast `winrt_toast_reborn` confirming "Added to OCR Corpus: [filename]".

## Verification Plan

### Automated Tests
- Future regression runs will automatically include the new sample.

### Manual Verification
1. Run the `digicore-text-expander` app.
2. Snipping-tool a random region of the screen (saving to clipboard).
3. Press `Ctrl+Alt+Shift+S`.
4. Check that a new image appears in `docs/sample-ocr-images/`.
5. Check that a new snapshot baseline appears in `tests/snapshots/`.
6. Verify a Windows Toast Notification confirms the action.
