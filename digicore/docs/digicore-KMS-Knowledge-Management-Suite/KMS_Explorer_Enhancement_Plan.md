# Implementation Plan: KMS Knowledge Hub Hierarchical Explorer & Notebook Views

## Overview
Transform the current flat "Recent Notes" list into a robust, hierarchical Explorer similar to Obsidian, Notion, and Joplin. This will allow users to organize notes into Notebooks (folders) and sub-folders, providing a more intuitive navigation experience for large knowledge bases.

## Audit of Existing Implementation
- **Data Model**: `kms_notes` stores flat records with full paths. `KmsNoteDto` is also flat.
- **Discovery**: `sync_vault_files_to_db_internal` recursively scans the filesystem but only flattens the result into the database.
- **UI**: `KmsApp.tsx` renders a single list of notes sorted by modification date.

## Proposed Architecture
Following Hexagonal, SOLID, and SRP principles:
- **Domains**: The filesystem is the primary source of truth for structure. The DB is a cache for metadata and embeddings.
- **Configuration**: Vault path remains the root. Hierarchy is relative to this root.

## Proposed Changes

### 1. Backend: Data Structures & API
#### [DTO] `KmsFileSystemItemDto`
A recursive structure to represent the vault tree.
```typescript
export type KmsFileSystemItemDto = {
    name: string;
    path: string; // Absolute path for UI/Editor
    rel_path: string; // Relative path for internal logic
    item_type: 'file' | 'directory';
    children?: KmsFileSystemItemDto[];
    note?: KmsNoteDto; // Metadata from DB if file
}
```

#### [API] `kms_get_vault_structure`
- A new command that performs a recursive scan and merges DB metadata.
- **Optimization**: Use a cache or incremental updates if the vault is massive (>10k notes).

#### [KMS Repository]
- Enhance `kms_repository.rs` with folder-aware operations.
- Ensure `delete_note` and `rename_note` handle parent path changes correctly (refactoring backlinks).

### 2. Frontend: UI/UX Enhancements
#### [Components]
- `KmsExplorer`: Main container for the navigation.
- `KmsTreeItem`: Recursive component for folders and notes.
- `VaultActions`: Buttons for "New Note", "New Folder" at the current level.

#### [State Management]
- Expansion state (collapsed/expanded) persisted in `localStorage`.
- Active note highlighting within the tree.

#### [UX/Interactions]
- **Context Menus**: Right-click on folders to "New Note here", "Rename", "Delete".
- **Visuals**: Use `lucide-react` icons (ChevronRight, ChevronDown, Folder, FileText).
- **Notebook View**: Special treatment for top-level folders as "Notebooks" with distinct icons.

## Alternative Implementation Options

| Option | Pros | Cons |
| :--- | :--- | :--- |
| **A: Pure Filesystem Hierarchy** | Zero drift from disk; compatible with all Markdown editors. | More IO-intensive to scan. |
| **B: Database-Only Folders** | Faster retrieval; virtual "collections" across folders. | Drifts from disk; confusing if files move externally. |
| **C: Hybrid (Recommended)** | Folders mirror disk; DB caches metadata for speed. | Complexity in keeping sync perfect. |

## SWOT Analysis

- **Strengths**: Robust file-backed persistence; AI search integrated.
- **Weaknesses**: Current flat list doesn't scale well for large vaults.
- **Opportunities**: "Notebook" branding can appeal to Joplin/Evernote switchers.
- **Threats**: Rapid filesystem changes might cause UI jitter during sync.

## Key Decisions Required
1. **Empty Folders**: Should empty folders be shown in the Explorer? (Recommendation: Yes, as placeholders for new notes).
2. **Hidden Files**: Should we hide `.git` or `.obsidian` folders? (Recommendation: Yes, use an ignore list).
3. **Drafts**: Should we have a special "Drafts" notebook that isn't a folder? (Recommendation: Defer to Phase 86).

## Verification Plan

### Automated Tests
- **Unit (Rust)**: Test `build_tree` logic with mock filesystem paths.
- **Integration (Tauri)**: Verify `kms_get_vault_structure` returns correct depth for nested folders.

### Manual Verification
1. Create a 3-level deep folder structure on disk; verify UI reflects it.
2. Rename a top-level folder; verify all nested notes remain accessible and functional.
3. Right-click folder -> New Note; verify file is created in the correct subdirectory.
4. Verify "Semantic Search" still navigates correctly to nested notes.

## Technical Details: Diagnostic Logging
- Log recursion depth and item counts during vault scans.
- Detailed error logging for "Access Denied" or "Path Too Long" scenarios on Windows.
- Trace logs for path normalization (slashes/case-sensitivity).
