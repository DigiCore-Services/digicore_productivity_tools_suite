# Implementation Plan: Phase 76 - KMS Foundation & Vault Initialization

This phase focuses on the structural and architectural foundations for the DigiCore Knowledge Management Suite (KMS). We will implement the database schema, the multi-window launch system, and the vault initialization logic.

## Proposed Changes

### [Backend] Database & Internal Logic
#### [MODIFY] [lib.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/lib.rs)
- Add Migration v4 to `tauri-plugin-sql` setup.
- Define tables:
  - `kms_notes`: Metadata for Markdown files.
  - `kms_links`: Relationship graph (source/target).
  - `kms_tags`: Indexed tags for fast retrieval.
  - `kms_bookmarks`: URL management and content snapshots.

#### [MODIFY] [api.rs](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src-tauri/src/api.rs)
- Add `kms_launch` TauRPC command: Spawns a new Tauri window with label `kms`.
- Add `kms_initialize` TauRPC command: Handles vault directory creation (default: `Documents/DigiCore Notes`) and starts the initial indexer/watcher.
- Implement `kms_sync_vault` logic: Ensures the SQLite index matches the current state of the filesystem.

### [Frontend] Multi-Window Scaffolding
#### [MODIFY] [main.tsx](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/main.tsx)
- Detect `window.label()`.
- If `main`: Render existing `<App />`.
- If `kms`: Render new `<KmsApp />`.

#### [MODIFY] [App.tsx](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/App.tsx)
- Add a "Knowledge Base" (KMS) launch button. This will be a specialized action outside the standard tab navigation, triggering the backend `kms_launch` command.

#### [NEW] [KmsApp.tsx](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/KmsApp.tsx)
- Create the foundational shell for the Knowledge Management Suite.
- Implement the "Vault Setup" wizard (for first-time initialization).

## Verification Plan

### Automated Tests
- No existing tests for multi-window IPC.
- Will verify via manual smoke tests since this involves system-level window management and filesystem interaction.

### Manual Verification
1.  **DB Migration**: Check `digicore.db` (e.g., using a SQLite viewer or `sqlite3` CLI) to ensure the new KMS tables are created after startup.
2.  **Launch KMS**: Click the new "Knowledge Base" button in the main app. Verify a separate, styled window opens.
3.  **Window Differentiation**: Confirm that the "main" window remains the management console while the "kms" window shows the new KMS shell.
4.  **Vault Init**: Verify that `Documents/DigiCore Notes` is created automatically on first launch and populated with an `attachments/` subfolder.
