# DigiCore Text Expander - Clipboard History

**Version:** 1.0  
**Last Updated:** 2026-02-28  
**Product:** DigiCore Text Expander (Rust)

---

## Overview

Clipboard History provides real-time monitoring of your system clipboard. Each copied item is stored with metadata (source app, window title) and can be managed from the Clipboard History tab. This feature matches the AHK implementation (F38-F42).

## Enabling Clipboard History

1. Open the **Configuration** tab.
2. Enable **Clipboard History** in the Clipboard History section.
3. Set **Max Depth** (5-100) to control how many items are retained.
4. Save configuration.

## Clipboard History Tab

The tab displays a table with:

| Column | Description |
|--------|-------------|
| # | Item number (1 = most recent) |
| Content Preview | First 40 characters of the content |
| App | Source application process name |
| Window Title | Title of the active window when copied |
| Length | Character count |

### Actions

- **Refresh** - Rebuild the display from the current history.
- **Clear All History** - Opens a confirmation dialog to clear all entries.

## Right-Click Context Menu

Right-click any clipboard entry to access:

| Action | Description |
|--------|-------------|
| **Copy to Clipboard** | Copies the item's content to the system clipboard. Shows "Copied item #N to clipboard!" |
| **View Full Content** | Opens a modal with the full content. Includes a "Promote to Snippet" button. |
| **Delete Item** | Removes the item after confirmation ("Are you sure you want to delete this clipboard item?") |
| **Promote to Snippet** | Opens the Promote to Snippet modal to add the content as a new snippet. |
| **Clear All History** | Opens confirmation to clear all clipboard history. |

## Modals

### View Full Content

- Displays the full content in a scrollable, read-only view.
- **Promote to Snippet** - Opens the Promote modal with the content pre-filled.
- **Close** - Closes the modal.

### Delete Confirmation

- Message: "Are you sure you want to delete this clipboard item?"
- Shows a preview (first 30 characters).
- **Yes** - Deletes the item.
- **No** - Cancels.

### Clear All Confirmation

- Message: "Clear all clipboard history?"
- **Yes** - Clears all entries.
- **No** - Cancels.

## Promote to Snippet

When promoting from Clipboard History:

1. The Promote modal opens with content pre-filled.
2. Enter a trigger (shortcut) for the snippet.
3. Click **Save** to add to your library.
4. Status: "Clip promoted! Set your trigger and save."

## Configuration

- **Max Depth:** 5-100 (default: 20). Older entries are dropped when the limit is reached.
- **Enabled:** Toggle in Configuration tab. When off, the tab shows "Clipboard monitoring is off. Enable in Configuration tab."

## Testing

Clipboard history is covered by unit tests:

- `test_config_default`, `test_start_stop` - Config and lifecycle
- `test_add_entry_dedup`, `test_add_entry_max_depth`, `test_add_entry_with_metadata` - Entry handling and metadata
- `test_add_entry_when_disabled` - Disabled state
- `test_request_take_promote`, `test_take_promote_pending_none` - Promote flow
- `test_delete_entry_at`, `test_delete_entry_at_out_of_bounds` - Delete by index
- `test_clear_all` - Clear all history
- `test_update_config_max_depth_trims`, `test_update_config_disabled` - Config updates
- `test_suppress_for_duration_no_panic` - Suppression

Run with: `cargo test -p digicore-text-expander application::clipboard_history`

See [Implementation Plan](IMPLEMENTATION_PLAN.md) for full testing details.

## Snippet Placeholders

Clipboard history is available in snippets via `{clip:N}`:

- `{clip:1}` - Most recent clipboard item
- `{clip:2}` - Second most recent
- `{clip:3}` - Third most recent
- etc.

## Parity with AHK

This implementation matches the AHK Text Expansion Pro behavior:

- `ShowClipContextMenu` - Right-click menu
- `DeleteClipHistoryItem` - Delete with confirmation
- `ClearAllClipHistory` - Clear all with confirmation
- `CopyClipHistoryItem` - Copy with tooltip
- `ViewFullContentPreview` - Modal with Promote button
