# KMS Frontend Diagnostics & Sync Status

Implement a dedicated UI for viewing KMS diagnostic logs and improve the visibility of the vault synchronization status. This builds upon the Phase 88 backend refactoring that added persistent logging and structured error handling.

## Proposed Changes

### [Frontend] Core UI & Components

#### [MODIFY] [bindings.ts](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/bindings.ts)
- Manually add `KmsLog` type to match `kms_repository::KmsLog`.
- Update `Router` interface to include `kms_get_logs` and `kms_clear_logs`.

#### [NEW] [KmsLogViewer.tsx](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/components/kms/KmsLogViewer.tsx)
- Create a clean log viewer component using the `KmsLog` type.
- Include level-based coloring (Error = red, Info = blue, Debug = gray).
- Add "Refresh" and "Clear All" buttons.
- Implement auto-scroll to bottom behavior.

#### [MODIFY] [KmsApp.tsx](file:///c:/Users/pinea/Scripts/AHK_AutoHotKey/digicore/tauri-app/src/KmsApp.tsx)
- Add `"logs"` to the `view` state enum.
- Add a new "Audit Logs" button in the sidebar under "Navigation".
- Implement `handleClearLogs` and `refreshLogs` (on demand or on view switch).
- Update the main content area with a conditional branch to render `KmsLogViewer` when `view === "logs"`.
- Enhance the bottom status bar to show more detail when `syncStatus` indicates an error or ongoing work.

## Verification Plan

### Automated Tests
- N/A (Frontend component testing not established in this environment, will focus on manual verification).

### Manual Verification
1. **Sidebar Navigation**:
   - Open KMS.
   - Click "Audit Logs" in the sidebar.
   - Verify the main area switches to the Log Viewer.
2. **Log Display**:
   - Save a note or rename a folder.
   - Verify that new entries appear in the Audit Logs (e.g., "Indexing note...", "Renamed folder...").
3. **Log Management**:
   - Click "Refresh" and verify new logs load.
   - Click "Clear All" and verify logs are removed from the view and backend.
4. **Resiliency**:
   - Manually trigger a "failed" sync (e.g. by deleting a file while it's being indexed if possible) and verify the "failed" status appears in logs with red text.
