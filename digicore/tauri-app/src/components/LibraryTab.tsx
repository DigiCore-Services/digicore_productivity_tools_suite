import React, { useCallback, useEffect, useRef, useState } from "react";
import { getTaurpc } from "@/lib/taurpc";
import { normalizeAppState } from "@/lib/normalizeState";
import { open, confirm } from "@tauri-apps/plugin-dialog";
import { useVirtualizer } from "@tanstack/react-virtual";
import { FolderOpen, Save, Search, Plus, Pencil, Trash2, FolderSearch, Pin } from "lucide-react";
import { showNativeContextMenu } from "@/lib/nativeContextMenu";
import { notify } from "@/lib/notifications";
import { syncLibraryToSqlite } from "@/lib/sqliteSync";
import {
  useSqliteRows,
  SQLITE_PARTIAL_THRESHOLD,
  SQLITE_PAGE_SIZE,
} from "@/lib/useSqliteRows";
import { formatLastModified, getCellValue, getRawField, COLUMN_KEYS } from "@/lib/libraryUtils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

const VIRTUALIZE_THRESHOLD = 500;
const ROW_HEIGHT = 40;

import type { AppState, Snippet } from "../types";

interface LibraryTabProps {
  appState: AppState | null;
  onAppStateChange: (state: AppState) => void;
  setStatus: (text: string, isError?: boolean) => void;
  libraryStatus?: string;
  libraryStatusError?: boolean;
  onOpenViewFull: (
    content: string,
    editMeta?: { category: string; snippetIdx: number }
  ) => void;
  onOpenSnippetEditor: (
    mode: "add" | "edit",
    category: string,
    snippetIdx: number
  ) => void;
  onOpenDeleteConfirm: (category: string, snippetIdx: number) => void;
  columnOrder: string[];
  sortColumn: string | null;
  sortAsc: boolean;
  onColumnOrderChange: (order: string[]) => void;
  onSortChange: (col: string, asc: boolean) => void;
  onColumnDrag: (col: string, targetCol: string) => void;
}

export function LibraryTab({
  appState,
  onAppStateChange,
  setStatus,
  onOpenViewFull,
  onOpenSnippetEditor,
  onOpenDeleteConfirm,
  columnOrder,
  sortColumn,
  sortAsc,
  onColumnOrderChange,
  onSortChange,
  onColumnDrag,
  libraryStatus = "",
  libraryStatusError = false,
}: LibraryTabProps) {
  const [libraryPath, setLibraryPath] = useState("");
  const [search, setSearch] = useState("");
  const [draggedCol, setDraggedCol] = useState<string | null>(null);

  const loadAppState = useCallback(async () => {
    try {
      const dto = await getTaurpc().get_app_state();
      const state = normalizeAppState(dto);
      setLibraryPath(state.library_path || "");
      onAppStateChange(state);
      return state;
    } catch (e) {
      setStatus("Error: " + String(e), true);
      return null;
    }
  }, [onAppStateChange, setStatus]);

  const handleLoad = async () => {
    if (libraryPath.trim()) {
      try {
        await getTaurpc().set_library_path(libraryPath.trim());
      } catch (e) {
        setStatus("set_library_path: " + String(e), true);
        return;
      }
    }
    try {
      const count = await getTaurpc().load_library();
      setStatus(`Loaded ${count} categories`);
      await getTaurpc().save_settings().catch(() => {});
      const state = await loadAppState();
      if (state?.library && Object.keys(state.library).length > 0) {
        await syncLibraryToSqlite(state.library);
      }
      await notify("DigiCore Text Expander", `Library loaded: ${count} categories`, {
        withViewLibrary: true,
      });
    } catch (e) {
      setStatus("Load failed: " + String(e), true);
    }
  };

  const handleBrowse = async () => {
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        title: "Select Library File",
        filters: [{ name: "JSON Library", extensions: ["json"] }],
        defaultPath: libraryPath.trim() || undefined,
      });
      if (selected && typeof selected === "string") {
        setLibraryPath(selected);
      }
    } catch (e) {
      setStatus("Browse failed: " + String(e), true);
    }
  };

  const library = appState?.library ?? {};

  const totalSnippetCount = Object.values(library).reduce(
    (sum, arr) => sum + arr.length,
    0
  );
  const useSqlitePartial =
    totalSnippetCount > SQLITE_PARTIAL_THRESHOLD;
  const {
    rows: sqliteRows,
    total: sqliteTotal,
    fetchPage: fetchSqlitePage,
  } = useSqliteRows(totalSnippetCount, search, library);

  const handleTogglePin = useCallback(
    async (cat: string, idx: number) => {
      const snips = library[cat];
      if (!snips || idx >= snips.length) return;
      const s = snips[idx];
      const isPinned = (s.pinned || "false").toLowerCase() === "true";
      if (isPinned) {
        const confirmed = await confirm(
          "Are you sure you want to unpin this snippet?",
          { title: "Confirm Unpin", kind: "warning" }
        );
        if (!confirmed) {
          setStatus("Unpin cancelled.");
          return;
        }
      }
      const newPinned = isPinned ? "false" : "true";
      try {
        await getTaurpc().update_snippet(cat, idx, {
          ...s,
          pinned: newPinned,
        });
        await getTaurpc().save_library();
        await getTaurpc().save_settings().catch(() => {});
        const state = await loadAppState();
        const lib = state?.library ?? {};
        if (Object.keys(lib).length > 0) {
          await syncLibraryToSqlite(lib);
        }
        setStatus(newPinned === "true" ? "Snippet pinned" : "Snippet unpinned");
      } catch (e) {
        setStatus("Pin failed: " + String(e), true);
      }
    },
    [library, loadAppState, setStatus]
  );

  const categories = appState?.categories ?? [];
  const handleSave = async () => {
    try {
      await getTaurpc().save_library();
      setStatus("Saved successfully");
      const state = await loadAppState();
      if (state?.library && Object.keys(state.library).length > 0) {
        await syncLibraryToSqlite(state.library);
      }
      await notify("DigiCore Text Expander", "Library saved successfully", {
        withViewLibrary: true,
      });
    } catch (e) {
      setStatus("Save failed: " + String(e), true);
    }
  };

  const searchLower = search.toLowerCase().split(/\s+/).filter(Boolean);

  let rows: { cat: string; s: Snippet; idx: number }[] = [];
  for (const [cat, snippets] of Object.entries(library)) {
    for (let idx = 0; idx < snippets.length; idx++) {
      const s = snippets[idx];
      const preview = (s.content || "")
        .replace(/\n/g, " ")
        .slice(0, 60) + (s.content?.length && s.content.length > 60 ? "..." : "");
      const target = (
        cat +
        " " +
        (s.trigger || "") +
        " " +
        preview +
        " " +
        (s.profile || "")
      ).toLowerCase();
      if (
        searchLower.length &&
        !searchLower.every((w) => target.includes(w))
      ) {
        continue;
      }
      rows.push({ cat, s, idx });
    }
  }

  const effectiveSortCol = sortColumn || "Trigger";
  const key = COLUMN_KEYS[effectiveSortCol];
  const getVal = (r: (typeof rows)[0]) => {
    if (!key) return "";
    if (key === "content") return (r.s.content || "").toLowerCase();
    const rec = r.s as unknown as Record<string, string | undefined>;
    const raw = getRawField(rec, key);
    return key === "last_modified" ? raw : raw.toLowerCase();
  };
  const isPinned = (r: (typeof rows)[0]) =>
    (r.s.pinned || "false").toLowerCase() === "true";
  rows.sort((a, b) => {
    const pinA = isPinned(a) ? 0 : 1;
    const pinB = isPinned(b) ? 0 : 1;
    if (pinA !== pinB) return pinA - pinB;
    const va = getVal(a);
    const vb = getVal(b);
    const cmp = va < vb ? -1 : va > vb ? 1 : 0;
    return sortAsc ? cmp : -cmp;
  });

  const handleColClick = (col: string) => {
    if (sortColumn === col) onSortChange(col, !sortAsc);
    else onSortChange(col, true);
  };

  const handleColDragStart = (col: string) => setDraggedCol(col);
  const handleColDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
  };
  const handleColDrop = (targetCol: string) => {
    if (!draggedCol || targetCol === draggedCol) return;
    const fromIdx = columnOrder.indexOf(draggedCol);
    const toIdx = columnOrder.indexOf(targetCol);
    if (fromIdx < 0 || toIdx < 0) return;
    const newOrder = [...columnOrder];
    newOrder.splice(fromIdx, 1);
    newOrder.splice(toIdx, 0, draggedCol);
    onColumnOrderChange(newOrder);
    onColumnDrag(draggedCol, targetCol);
    setDraggedCol(null);
  };
  const handleColDragEnd = () => setDraggedCol(null);

  const tableContainerRef = useRef<HTMLDivElement>(null);
  const hScrollContentRef = useRef<HTMLDivElement>(null);
  const hScrollBarRef = useRef<HTMLDivElement>(null);
  const spacerRef = useRef<HTMLDivElement>(null);
  const syncingRef = useRef(false);
  const effectiveRows = useSqlitePartial ? sqliteRows : rows;
  const effectiveCount = useSqlitePartial ? sqliteTotal : rows.length;
  const useVirtual =
    effectiveCount >= VIRTUALIZE_THRESHOLD || useSqlitePartial;

  const getScrollContent = useCallback(
    () => (useVirtual ? tableContainerRef.current : hScrollContentRef.current),
    [useVirtual]
  );

  const syncTableFromBar = useCallback(() => {
    const content = getScrollContent();
    if (syncingRef.current || !content || !hScrollBarRef.current) return;
    syncingRef.current = true;
    content.scrollLeft = hScrollBarRef.current.scrollLeft;
    requestAnimationFrame(() => { syncingRef.current = false; });
  }, [getScrollContent]);

  const syncBarFromTable = useCallback(() => {
    const content = getScrollContent();
    if (syncingRef.current || !content || !hScrollBarRef.current) return;
    syncingRef.current = true;
    hScrollBarRef.current.scrollLeft = content.scrollLeft;
    requestAnimationFrame(() => { syncingRef.current = false; });
  }, [getScrollContent]);

  useEffect(() => {
    if (!library || Object.keys(library).length === 0) return;
    const content = useVirtual ? tableContainerRef.current : hScrollContentRef.current;
    const spacer = spacerRef.current;
    if (!content || !spacer) return;
    spacer.style.width = `${content.scrollWidth}px`;
  }, [library, effectiveCount, useVirtual, columnOrder]);
  const rowVirtualizer = useVirtualizer({
    count: effectiveCount,
    getScrollElement: () => tableContainerRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 10,
    enabled: useVirtual,
  });

  const renderRow = (row: { cat: string; s: Snippet; idx: number }, rowClass: string) => (
    <>
      {columnOrder.map((col) => (
        <td key={col} className="border border-[var(--dc-border)] p-1.5">
          {col === "Category" ? row.cat : getCellValue(row.s, col)}
        </td>
      ))}
      <td className="border border-[var(--dc-border)] p-1.5">
        <Button
          variant="ghost"
          size="sm"
          onClick={() => onOpenSnippetEditor("edit", row.cat, row.idx)}
          className="h-7 px-2 mr-1"
          aria-label={`Edit snippet ${row.s.trigger}`}
        >
          <Pencil className="w-3 h-3 mr-1" aria-hidden />
          Edit
        </Button>
        <Button
          variant="ghost"
          size="sm"
          onClick={() => onOpenDeleteConfirm(row.cat, row.idx)}
          className="h-7 px-2 text-[var(--dc-error)] hover:text-[var(--dc-error)] hover:bg-red-500/10"
          aria-label={`Delete snippet ${row.s.trigger}`}
        >
          <Trash2 className="w-3 h-3 mr-1" aria-hidden />
          Delete
        </Button>
      </td>
    </>
  );

  return (
    <div className="p-6 border border-[var(--dc-border)] rounded-xl mt-2 bg-[var(--dc-bg-elevated)]/50">
      <h2 className="text-xl font-semibold mb-6">Text Expansion Library</h2>
      <div className="flex flex-wrap gap-3 mb-4">
        <div className="flex items-center gap-2">
          <Input
            value={libraryPath}
            onChange={(e) => setLibraryPath(e.target.value)}
            placeholder="path/to/library.json"
            className="w-[400px]"
            aria-label="Library file path"
          />
          <Button onClick={handleBrowse} variant="secondary" size="sm" aria-label="Browse for library file">
            <FolderSearch className="w-4 h-4 mr-1" aria-hidden />
            Browse
          </Button>
          <Button onClick={handleLoad} size="sm" aria-label="Load library">
            <FolderOpen className="w-4 h-4 mr-1" aria-hidden />
            Load
          </Button>
          <Button onClick={handleSave} variant="secondary" size="sm" aria-label="Save library">
            <Save className="w-4 h-4 mr-1" aria-hidden />
            Save
          </Button>
        </div>
      </div>
      <div className="flex flex-wrap gap-3 mb-4">
        <div className="flex items-center gap-2">
          <Search className="w-4 h-4 text-[var(--dc-text-muted)]" />
          <Input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search triggers/content..."
            className="w-[400px]"
            aria-label="Search snippets"
          />
          <Button
            onClick={() =>
              onOpenSnippetEditor("add", categories[0] || "General", -1)
            }
            size="sm"
            aria-label="Add snippet"
          >
            <Plus className="w-4 h-4 mr-1" aria-hidden />
            Add Snippet
          </Button>
        </div>
      </div>
      <div
        style={
          !library || Object.keys(library).length === 0
            ? undefined
            : useVirtual
              ? { display: "flex", flexDirection: "column", maxHeight: 420 }
              : undefined
        }
      >
        {!library || Object.keys(library).length === 0 ? (
          <p>No snippets loaded. Set library path and click Load.</p>
        ) : useVirtual ? (
          <>
          <div
            ref={tableContainerRef}
            className="border border-[var(--dc-border)] overflow-x-auto overflow-y-auto flex-1 min-h-0 hide-h-scrollbar"
            style={{ maxHeight: 420 }}
            onScroll={syncBarFromTable}
          >
            <div
              className="grid border-b border-[var(--dc-border)] bg-[var(--dc-bg-tertiary)] sticky top-0 z-10"
              style={{
                gridTemplateColumns: `28px repeat(${columnOrder.length}, minmax(80px, 1fr)) 120px`,
              }}
            >
              <div className="border-r border-[var(--dc-border)] p-1.5 flex items-center justify-center" title="Pinned">
                <Pin className="w-3.5 h-3.5 text-[var(--dc-text-muted)]" aria-hidden />
              </div>
              {columnOrder.map((col) => (
                <div
                  key={col}
                  data-col={col}
                  draggable
                  onDragStart={() => handleColDragStart(col)}
                  onDragOver={handleColDragOver}
                  onDrop={() => handleColDrop(col)}
                  onDragEnd={handleColDragEnd}
                  className={`border-r border-[var(--dc-border)] p-1.5 cursor-pointer select-none whitespace-nowrap last:border-r-0 ${
                    draggedCol === col ? "opacity-50" : ""
                  } ${(sortColumn || "Trigger") === col ? (sortAsc ? "sort-asc" : "sort-desc") : ""}`}
                  onClick={() => handleColClick(col)}
                >
                  {col}
                  {(sortColumn || "Trigger") === col && (
                    <span className="text-[0.7em] ml-1">
                      {sortAsc ? " \u25B2" : " \u25BC"}
                    </span>
                  )}
                </div>
              ))}
              <div className="p-1.5">Actions</div>
            </div>
            <div
              style={{
                height: `${rowVirtualizer.getTotalSize()}px`,
                width: "100%",
                position: "relative",
              }}
            >
              {rowVirtualizer.getVirtualItems().map((virtualRow) => {
                let row = effectiveRows[virtualRow.index];
                if (useSqlitePartial && !row) {
                  const pageIndex = Math.floor(
                    virtualRow.index / SQLITE_PAGE_SIZE
                  );
                  fetchSqlitePage(pageIndex);
                  return (
                    <div
                      key={`loading-${virtualRow.index}`}
                      className="grid border-b border-[var(--dc-border)] items-center"
                      style={{
                        gridTemplateColumns: `28px repeat(${columnOrder.length}, minmax(80px, 1fr)) 120px`,
                        position: "absolute",
                        top: 0,
                        left: 0,
                        width: "100%",
                        height: `${virtualRow.size}px`,
                        transform: `translateY(${virtualRow.start}px)`,
                        backgroundColor:
                          virtualRow.index % 2 === 0
                            ? "var(--dc-bg-alt)"
                            : "var(--dc-bg)",
                      }}
                    >
                      <div
                        className="col-span-full p-2 text-[var(--dc-text-muted)] text-sm"
                      >
                        Loading...
                      </div>
                    </div>
                  );
                }
                if (!row) return null;
                return (
                  <div
                    key={`${row.cat}-${row.idx}-${virtualRow.index}`}
                    className="grid border-b border-[var(--dc-border)] cursor-context-menu"
                    onContextMenu={(e) => {
                      e.preventDefault();
                      const isPinned = (row.s.pinned || "false") === "true";
                      showNativeContextMenu(e.clientX, e.clientY, [
                        {
                          id: "view-full",
                          icon: "👁",
                          text: "View Full Snippet Content",
                          onClick: () =>
                            onOpenViewFull(row.s.content || "", {
                              category: row.cat,
                              snippetIdx: row.idx,
                            }),
                        },
                        {
                          id: "pin",
                          icon: isPinned ? "📌" : "📍",
                          text: isPinned ? "Unpin Snippet" : "Pin Snippet",
                          onClick: () => handleTogglePin(row.cat, row.idx),
                        },
                        {
                          id: "copy",
                          icon: "⧉",
                          text: "Copy Full Content to Clipboard",
                          onClick: async () => {
                            try {
                              await getTaurpc().copy_to_clipboard(
                                row.s.content || ""
                              );
                              setStatus("Copied to clipboard");
                            } catch (err) {
                              setStatus("Copy failed: " + String(err), true);
                            }
                          },
                        },
                        {
                          id: "edit",
                          icon: "✎",
                          text: "Edit Snippet",
                          onClick: () =>
                            onOpenSnippetEditor("edit", row.cat, row.idx),
                        },
                        {
                          id: "delete",
                          icon: "🗑",
                          text: "Delete Snippet",
                          onClick: () =>
                            onOpenDeleteConfirm(row.cat, row.idx),
                        },
                      ]);
                    }}
                    style={{
                          gridTemplateColumns: `28px repeat(${columnOrder.length}, minmax(80px, 1fr)) 120px`,
                          position: "absolute",
                          top: 0,
                          left: 0,
                          width: "100%",
                          height: `${virtualRow.size}px`,
                          transform: `translateY(${virtualRow.start}px)`,
                          backgroundColor:
                            virtualRow.index % 2 === 0
                              ? "var(--dc-bg-alt)"
                              : "var(--dc-bg)",
                        }}
                      >
                        <div
                          className="border-r border-[var(--dc-border)] p-1.5 flex items-center justify-center cursor-pointer"
                          onClick={() => handleTogglePin(row.cat, row.idx)}
                          title={
                            (row.s.pinned || "false").toLowerCase() === "true"
                              ? "Unpin Snippet"
                              : "Pin Snippet"
                          }
                        >
                          {(row.s.pinned || "false").toLowerCase() === "true" ? (
                            <span title="Pinned"><Pin className="w-3.5 h-3.5 text-amber-500 fill-amber-500" aria-hidden /></span>
                          ) : (
                            <span className="w-3.5 h-3.5" aria-hidden />
                          )}
                        </div>
                        {columnOrder.map((col) => (
                          <div
                            key={col}
                            className="border-r border-[var(--dc-border)] p-1.5 last:border-r-0"
                          >
                            {col === "Category"
                              ? row.cat
                              : getCellValue(row.s, col)}
                          </div>
                        ))}
                        <div className="p-1.5 flex gap-1">
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() =>
                              onOpenSnippetEditor("edit", row.cat, row.idx)
                            }
                            className="h-7 px-2"
                          >
                            <Pencil className="w-3 h-3 mr-1" />
                            Edit
                          </Button>
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() =>
                              onOpenDeleteConfirm(row.cat, row.idx)
                            }
                            className="h-7 px-2 text-[var(--dc-error)]"
                          >
                            <Trash2 className="w-3 h-3 mr-1" />
                            Delete
                          </Button>
                        </div>
                      </div>
                );
              })}
            </div>
          </div>
          <div
            ref={hScrollBarRef}
            className="flex-shrink-0"
            style={{
              overflowX: "auto",
              overflowY: "hidden",
            }}
            onScroll={syncTableFromBar}
          >
            <div ref={spacerRef} style={{ height: 1, minWidth: "100%" }} />
          </div>
          </>
        ) : (
          <div
            className="border border-[var(--dc-border)]"
            style={{
              display: "flex",
              flexDirection: "column",
              maxHeight: 420,
            }}
          >
            <div
              ref={hScrollContentRef}
              className="hide-h-scrollbar"
              style={{
                flex: 1,
                overflowY: "auto",
                overflowX: "auto",
                minHeight: 0,
              }}
              onScroll={syncBarFromTable}
            >
            <table className="w-full border-collapse border border-[var(--dc-border)]">
              <thead>
                <tr>
                  <th
                    className="sticky top-0 z-10 border border-[var(--dc-border)] p-1.5 w-[28px] bg-[var(--dc-bg-tertiary)]"
                    style={{ backgroundColor: "var(--dc-bg-tertiary)" }}
                    title="Pinned"
                  >
                    <Pin className="w-3.5 h-3.5 mx-auto text-[var(--dc-text-muted)]" aria-hidden />
                  </th>
                  {columnOrder.map((col) => (
                    <th
                      key={col}
                      data-col={col}
                      draggable
                      onDragStart={() => handleColDragStart(col)}
                      onDragOver={handleColDragOver}
                      onDrop={() => handleColDrop(col)}
                      onDragEnd={handleColDragEnd}
                      className={`sticky top-0 z-10 border border-[var(--dc-border)] p-1.5 text-left bg-[var(--dc-bg-tertiary)] cursor-pointer select-none whitespace-nowrap ${
                        draggedCol === col ? "opacity-50" : ""
                      } ${sortColumn === col ? (sortAsc ? "sort-asc" : "sort-desc") : ""}`}
                      style={{ backgroundColor: "var(--dc-bg-tertiary)" }}
                    onClick={() => handleColClick(col)}
                  >
                    {col}
                    {(sortColumn || "Trigger") === col && (
                        <span className="text-[0.7em] ml-1">
                          {sortAsc ? " \u25B2" : " \u25BC"}
                        </span>
                      )}
                    </th>
                  ))}
                  <th
                    className="sticky top-0 z-10 border border-[var(--dc-border)] p-1.5 text-left bg-[var(--dc-bg-tertiary)]"
                    style={{ backgroundColor: "var(--dc-bg-tertiary)" }}
                  >
                    Actions
                  </th>
                </tr>
              </thead>
              <tbody>
              {rows.map(({ cat, s, idx }) => (
                <tr
                  key={`${cat}-${idx}`}
                  className="even:bg-[var(--dc-bg-alt)] odd:bg-[var(--dc-bg)] cursor-context-menu"
                  onContextMenu={(e) => {
                    e.preventDefault();
                    const isPinned = (s.pinned || "false") === "true";
                    showNativeContextMenu(e.clientX, e.clientY, [
                      {
                        id: "view-full",
                        icon: "👁",
                        text: "View Full Snippet Content",
                        onClick: () =>
                          onOpenViewFull(s.content || "", {
                            category: cat,
                            snippetIdx: idx,
                          }),
                      },
                      {
                        id: "pin",
                        icon: isPinned ? "📌" : "📍",
                        text: isPinned ? "Unpin Snippet" : "Pin Snippet",
                        onClick: () => handleTogglePin(cat, idx),
                      },
                      {
                        id: "copy",
                        icon: "⧉",
                        text: "Copy Full Content to Clipboard",
                        onClick: async () => {
                          try {
                            await getTaurpc().copy_to_clipboard(
                              s.content || ""
                            );
                            setStatus("Copied to clipboard");
                          } catch (err) {
                            setStatus("Copy failed: " + String(err), true);
                          }
                        },
                      },
                      {
                        id: "edit",
                        icon: "✎",
                        text: "Edit Snippet",
                        onClick: () => onOpenSnippetEditor("edit", cat, idx),
                      },
                      {
                        id: "delete",
                        icon: "🗑",
                        text: "Delete Snippet",
                        onClick: () => onOpenDeleteConfirm(cat, idx),
                      },
                    ]);
                  }}
                >
                  <td
                    className="border border-[var(--dc-border)] p-1.5 w-[28px] text-center cursor-pointer"
                    onClick={() => handleTogglePin(cat, idx)}
                    title={
                      (s.pinned || "false").toLowerCase() === "true"
                        ? "Unpin Snippet"
                        : "Pin Snippet"
                    }
                  >
                    {(s.pinned || "false").toLowerCase() === "true" ? (
                      <span title="Pinned"><Pin className="w-3.5 h-3.5 mx-auto text-amber-500 fill-amber-500" aria-hidden /></span>
                    ) : (
                      <span className="w-3.5 h-3.5 inline-block" aria-hidden />
                    )}
                  </td>
                  {renderRow({ cat, s, idx }, "")}
                </tr>
              ))}
            </tbody>
          </table>
            </div>
            <div
              ref={hScrollBarRef}
              className="flex-shrink-0"
              style={{
                overflowX: "auto",
                overflowY: "hidden",
              }}
              onScroll={syncTableFromBar}
            >
              <div ref={spacerRef} style={{ height: 1, minWidth: "100%" }} />
            </div>
          </div>
        )}
      </div>
      <p
        className={`text-sm mt-2 ${
          libraryStatusError ? "text-[var(--dc-error)]" : "text-[var(--dc-text-muted)]"
        }`}
      >
        {libraryStatus || appState?.status || "Ready"}
      </p>
    </div>
  );
}
