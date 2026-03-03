import React, { useCallback, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useVirtualizer } from "@tanstack/react-virtual";
import { FolderOpen, Save, Search, Plus, Pencil, Trash2 } from "lucide-react";
import * as ContextMenu from "@radix-ui/react-context-menu";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

const VIRTUALIZE_THRESHOLD = 500;
const ROW_HEIGHT = 40;

async function notify(title: string, body: string): Promise<void> {
  try {
    const { isPermissionGranted, requestPermission, sendNotification } =
      await import("@tauri-apps/plugin-notification");
    let granted = await isPermissionGranted();
    if (!granted) {
      const perm = await requestPermission();
      granted = perm === "granted";
    }
    if (granted) sendNotification({ title, body });
  } catch {
    /* ignore */
  }
}
import type { AppState, Snippet } from "../types";

const COLUMN_KEYS: Record<string, string> = {
  Profile: "profile",
  Category: "category",
  Trigger: "trigger",
  "Content Preview": "content",
  AppLock: "app_lock",
  Options: "options",
  "Last Modified": "last_modified",
};

function formatLastModified(val: string): string {
  if (!val) return "";
  if (val.length >= 14) {
    const y = val.slice(0, 4),
      m = val.slice(4, 6),
      d = val.slice(6, 8);
    const h = val.slice(8, 10),
      min = val.slice(10, 12),
      sec = val.slice(12, 14);
    return `${y}-${m}-${d} ${h}:${min}:${sec}`;
  }
  return val;
}

function getCellValue(s: Snippet, col: string): string {
  const key = COLUMN_KEYS[col];
  if (!key) return "";
  if (key === "content") {
    const content = (s.content || "").replace(/\n/g, " ").slice(0, 60);
    return content + (s.content?.length && s.content.length > 60 ? "..." : "");
  }
  if (key === "last_modified") return formatLastModified(s.last_modified || "");
  return ((s as unknown as Record<string, string>)[key] || "").toString();
}

interface LibraryTabProps {
  appState: AppState | null;
  onAppStateChange: (state: AppState) => void;
  setStatus: (text: string, isError?: boolean) => void;
  libraryStatus?: string;
  libraryStatusError?: boolean;
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
      const state = (await invoke("get_app_state")) as AppState;
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
        await invoke("set_library_path", { path: libraryPath.trim() });
      } catch (e) {
        setStatus("set_library_path: " + String(e), true);
        return;
      }
    }
    try {
      const count = (await invoke("load_library")) as number;
      setStatus(`Loaded ${count} categories`);
      await invoke("save_settings").catch(() => {});
      await loadAppState();
      await notify("DigiCore Text Expander", `Library loaded: ${count} categories`);
    } catch (e) {
      setStatus("Load failed: " + String(e), true);
    }
  };

  const handleSave = async () => {
    try {
      await invoke("save_library");
      setStatus("Saved successfully");
      await notify("DigiCore Text Expander", "Library saved successfully");
    } catch (e) {
      setStatus("Save failed: " + String(e), true);
    }
  };

  const library = appState?.library ?? {};
  const categories = appState?.categories ?? [];
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

  if (sortColumn && COLUMN_KEYS[sortColumn]) {
    const key = COLUMN_KEYS[sortColumn];
      const getVal = (r: (typeof rows)[0]) => {
      if (key === "content") return (r.s.content || "").toLowerCase();
      if (key === "last_modified") return r.s.last_modified || "";
      return ((r.s as unknown as Record<string, string>)[key] || "").toLowerCase();
    };
    rows.sort((a, b) => {
      const va = getVal(a),
        vb = getVal(b);
      const cmp = va < vb ? -1 : va > vb ? 1 : 0;
      return sortAsc ? cmp : -cmp;
    });
  }

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
  const useVirtual = rows.length >= VIRTUALIZE_THRESHOLD;
  const rowVirtualizer = useVirtualizer({
    count: rows.length,
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
        >
          <Pencil className="w-3 h-3 mr-1" />
          Edit
        </Button>
        <Button
          variant="ghost"
          size="sm"
          onClick={() => onOpenDeleteConfirm(row.cat, row.idx)}
          className="h-7 px-2 text-[var(--dc-error)] hover:text-[var(--dc-error)] hover:bg-red-500/10"
        >
          <Trash2 className="w-3 h-3 mr-1" />
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
          />
          <Button onClick={handleLoad} size="sm">
            <FolderOpen className="w-4 h-4 mr-1" />
            Load
          </Button>
          <Button onClick={handleSave} variant="secondary" size="sm">
            <Save className="w-4 h-4 mr-1" />
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
          />
          <Button
            onClick={() =>
              onOpenSnippetEditor("add", categories[0] || "General", -1)
            }
            size="sm"
          >
            <Plus className="w-4 h-4 mr-1" />
            Add Snippet
          </Button>
        </div>
      </div>
      <div
        className="overflow-x-auto"
        ref={useVirtual ? tableContainerRef : undefined}
        style={useVirtual ? { maxHeight: 420, overflowY: "auto" } : undefined}
      >
        {!library || Object.keys(library).length === 0 ? (
          <p>No snippets loaded. Set library path and click Load.</p>
        ) : useVirtual ? (
          <div className="border border-[var(--dc-border)]">
            <div
              className="grid border-b border-[var(--dc-border)] bg-[var(--dc-bg-tertiary)] sticky top-0 z-10"
              style={{
                gridTemplateColumns: `repeat(${columnOrder.length}, minmax(80px, 1fr)) 120px`,
              }}
            >
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
                  } ${sortColumn === col ? (sortAsc ? "sort-asc" : "sort-desc") : ""}`}
                  onClick={() => handleColClick(col)}
                >
                  {col}
                  {sortColumn === col && (
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
                const row = rows[virtualRow.index];
                return (
                  <ContextMenu.Root key={`${row.cat}-${row.idx}`}>
                    <ContextMenu.Trigger asChild>
                      <div
                        className="grid border-b border-[var(--dc-border)] cursor-context-menu"
                        style={{
                          gridTemplateColumns: `repeat(${columnOrder.length}, minmax(80px, 1fr)) 120px`,
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
                    </ContextMenu.Trigger>
                    <ContextMenu.Portal>
                      <ContextMenu.Content
                        className="min-w-[140px] rounded-lg border border-[var(--dc-border)] bg-[var(--dc-bg-elevated)] p-1 shadow-lg"
                        onCloseAutoFocus={(e) => e.preventDefault()}
                      >
                        <ContextMenu.Item
                          className="flex cursor-default items-center gap-2 rounded px-2 py-1.5 text-sm outline-none hover:bg-[var(--dc-bg-tertiary)] focus:bg-[var(--dc-bg-tertiary)]"
                          onSelect={() =>
                            onOpenSnippetEditor("edit", row.cat, row.idx)
                          }
                        >
                          <Pencil className="w-3.5 h-3.5" />
                          Edit
                        </ContextMenu.Item>
                        <ContextMenu.Separator className="my-1 h-px bg-[var(--dc-border)]" />
                        <ContextMenu.Item
                          className="flex cursor-default items-center gap-2 rounded px-2 py-1.5 text-sm text-[var(--dc-error)] outline-none hover:bg-red-500/10 focus:bg-red-500/10"
                          onSelect={() =>
                            onOpenDeleteConfirm(row.cat, row.idx)
                          }
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                          Delete
                        </ContextMenu.Item>
                      </ContextMenu.Content>
                    </ContextMenu.Portal>
                  </ContextMenu.Root>
                );
              })}
            </div>
          </div>
        ) : (
          <table className="w-full border-collapse border border-[var(--dc-border)]">
            <thead>
              <tr>
                {columnOrder.map((col) => (
                  <th
                    key={col}
                    data-col={col}
                    draggable
                    onDragStart={() => handleColDragStart(col)}
                    onDragOver={handleColDragOver}
                    onDrop={() => handleColDrop(col)}
                    onDragEnd={handleColDragEnd}
                    className={`border border-[var(--dc-border)] p-1.5 text-left bg-[var(--dc-bg-tertiary)] cursor-pointer select-none whitespace-nowrap ${
                      draggedCol === col ? "opacity-50" : ""
                    } ${sortColumn === col ? (sortAsc ? "sort-asc" : "sort-desc") : ""}`}
                    onClick={() => handleColClick(col)}
                  >
                    {col}
                    {sortColumn === col && (
                      <span className="text-[0.7em] ml-1">
                        {sortAsc ? " \u25B2" : " \u25BC"}
                      </span>
                    )}
                  </th>
                ))}
                <th className="border border-[var(--dc-border)] p-1.5 text-left bg-[var(--dc-bg-tertiary)]">
                  Actions
                </th>
              </tr>
            </thead>
            <tbody>
              {rows.map(({ cat, s, idx }) => (
                <ContextMenu.Root key={`${cat}-${idx}`}>
                  <ContextMenu.Trigger asChild>
                    <tr className="even:bg-[var(--dc-bg-alt)] odd:bg-[var(--dc-bg)] cursor-context-menu">
                      {renderRow({ cat, s, idx }, "")}
                    </tr>
                  </ContextMenu.Trigger>
                  <ContextMenu.Portal>
                    <ContextMenu.Content
                      className="min-w-[140px] rounded-lg border border-[var(--dc-border)] bg-[var(--dc-bg-elevated)] p-1 shadow-lg"
                      onCloseAutoFocus={(e) => e.preventDefault()}
                    >
                      <ContextMenu.Item
                        className="flex cursor-default items-center gap-2 rounded px-2 py-1.5 text-sm outline-none hover:bg-[var(--dc-bg-tertiary)] focus:bg-[var(--dc-bg-tertiary)]"
                        onSelect={() =>
                          onOpenSnippetEditor("edit", cat, idx)
                        }
                      >
                        <Pencil className="w-3.5 h-3.5" />
                        Edit
                      </ContextMenu.Item>
                      <ContextMenu.Separator className="my-1 h-px bg-[var(--dc-border)]" />
                      <ContextMenu.Item
                        className="flex cursor-default items-center gap-2 rounded px-2 py-1.5 text-sm text-[var(--dc-error)] outline-none hover:bg-red-500/10 focus:bg-red-500/10"
                        onSelect={() => onOpenDeleteConfirm(cat, idx)}
                      >
                        <Trash2 className="w-3.5 h-3.5" />
                        Delete
                      </ContextMenu.Item>
                    </ContextMenu.Content>
                  </ContextMenu.Portal>
                </ContextMenu.Root>
              ))}
            </tbody>
          </table>
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
