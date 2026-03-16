import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getTaurpc } from "@/lib/taurpc";
import { showNativeContextMenu } from "@/lib/nativeContextMenu";
import { ArrowUpToLine, Check, Copy, Eye, Image as ImageIcon, Search, Trash2 } from "lucide-react";
import { save } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { NativeContextMenuAction } from "@/lib/nativeContextMenu";
import type { AppState, ClipEntry } from "../types";

interface ClipboardTabProps {
  appState: AppState | null;
  refreshTrigger?: number;
  onOpenViewFull: (
    content: string,
    clipMeta?: { index: number; canPromote: boolean }
  ) => void;
  onOpenSnippetEditor: (
    mode: "add",
    category: string,
    snippetIdx: number,
    prefill?: { content: string; trigger: string }
  ) => void;
  onOpenClipClearConfirm: () => void;
  onOpenClipEntryDeleteConfirm: (idx: number) => void;
}

export function ClipboardTab({
  appState,
  refreshTrigger = 0,
  onOpenViewFull,
  onOpenSnippetEditor,
  onOpenClipClearConfirm,
  onOpenClipEntryDeleteConfirm,
}: ClipboardTabProps) {
  const [entries, setEntries] = useState<ClipEntry[]>([]);
  const [status, setStatus] = useState("");
  const [loading, setLoading] = useState(false);
  const [maxDepth, setMaxDepth] = useState<number | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchOperator, setSearchOperator] = useState<"or" | "and" | "regex">(
    "or"
  );
  const [thumbLoadFailed, setThumbLoadFailed] = useState<Record<number, boolean>>({});
  const [copiedIdx, setCopiedIdx] = useState<number | null>(null);
  const prevThumbByIdRef = useRef<Record<number, string>>({});
  const normalizeContentForMatch = useCallback((value: string) => {
    return (value || "").replace(/\r\n/g, "\n").trim();
  }, []);
  const formatUtcTimestamp = useCallback((value: string) => {
    const unixMs = Number.parseInt(value || "", 10);
    if (!Number.isFinite(unixMs) || unixMs <= 0) return "-";
    const date = new Date(unixMs);
    if (Number.isNaN(date.getTime())) return "-";
    const yyyy = date.getUTCFullYear();
    const mm = String(date.getUTCMonth() + 1).padStart(2, "0");
    const dd = String(date.getUTCDate()).padStart(2, "0");
    let hh = date.getUTCHours();
    const min = String(date.getUTCMinutes()).padStart(2, "0");
    const sec = String(date.getUTCSeconds()).padStart(2, "0");
    const ampm = hh >= 12 ? "PM" : "AM";
    hh = hh % 12;
    if (hh === 0) hh = 12;
    const hour = String(hh).padStart(2, "0");
    return `${yyyy}-${mm}-${dd} ${hour}:${min}:${sec} ${ampm} UTC`;
  }, []);
  const toFileUrl = useCallback((path: string | null | undefined) => {
    if (!path || !path.trim()) return "";
    const normalized = path.trim().replace(/\\/g, "/");
    if (normalized.startsWith("http://") || normalized.startsWith("https://")) {
      return normalized;
    }
    try {
      return convertFileSrc(normalized);
    } catch {
      // Fallback for non-Tauri test environments.
    }
    if (/^[a-zA-Z]:\//.test(normalized)) {
      return `file:///${normalized}`;
    }
    if (normalized.startsWith("/")) {
      return `file://${normalized}`;
    }
    return normalized;
  }, []);
  const snippetContentSet = useMemo(() => {
    const set = new Set<string>();
    const library = appState?.library ?? {};
    Object.values(library).forEach((snippets) => {
      snippets.forEach((snippet) => {
        const normalized = normalizeContentForMatch(snippet.content || "");
        if (normalized) {
          set.add(normalized);
        }
      });
    });
    return set;
  }, [appState?.library, normalizeContentForMatch]);

  const loadEntries = useCallback(async () => {
    setLoading(true);
    try {
      const query = searchQuery.trim();
      const [data, state] = await Promise.all([
        query
          ? getTaurpc().search_clipboard_entries(query, searchOperator, 500)
          : getTaurpc().get_clipboard_entries(),
        appState ? Promise.resolve(null) : getTaurpc().get_app_state().catch(() => null),
      ]);
      const recoveredThumbs = data.reduce((count, row) => {
        if (row.entry_type !== "image") return count;
        const prevThumb = prevThumbByIdRef.current[row.id] || "";
        const nextThumb = (row.thumb_path || "").trim();
        if (!prevThumb && !!nextThumb) return count + 1;
        return count;
      }, 0);
      prevThumbByIdRef.current = data.reduce<Record<number, string>>((acc, row) => {
        if (row.entry_type === "image") {
          acc[row.id] = (row.thumb_path || "").trim();
        }
        return acc;
      }, {});
      setEntries(data);
      setThumbLoadFailed({});
      const statusParts: string[] = [];
      if (query) {
        statusParts.push(
          `Search loaded (${searchOperator.toUpperCase()}): ${data.length} match${data.length === 1 ? "" : "es"
          }.`
        );
      }
      if (recoveredThumbs > 0) {
        statusParts.push(
          `Recovered ${recoveredThumbs} thumbnail${recoveredThumbs === 1 ? "" : "s"} during refresh.`
        );
      }
      if (statusParts.length > 0) {
        setStatus(statusParts.join(" "));
      }
      if (state) setMaxDepth(state.clip_history_max_depth ?? 20);
    } catch (e) {
      setStatus("Error: " + String(e));
    } finally {
      setLoading(false);
    }
  }, [appState, searchOperator, searchQuery]);

  useEffect(() => {
    loadEntries();
  }, [loadEntries, refreshTrigger]);

  useEffect(() => {
    if (appState?.clip_history_max_depth != null) setMaxDepth(appState.clip_history_max_depth);
  }, [appState?.clip_history_max_depth]);

  const handleCopy = async (idx: number) => {
    const entry = entries[idx];
    if (!entry) return;
    try {
      if (entry.entry_type === "image") {
        await getTaurpc().copy_clipboard_image_by_id(entry.id);
        setStatus("Copied image to clipboard!");
      } else {
        await getTaurpc().copy_to_clipboard(entry.content);
        setStatus("Copied to clipboard!");
      }
      setCopiedIdx(idx);
      setTimeout(() => {
        setCopiedIdx((prev) => (prev === idx ? null : prev));
      }, 2000);
    } catch (e) {
      setStatus("Error: " + String(e));
    }
  };

  const handleView = async (idx: number) => {
    const entry = entries[idx];
    if (!entry) return;
    if (entry.entry_type === "image") {
      try {
        await getTaurpc().open_clipboard_image_by_id(entry.id);
        setStatus("Opened image.");
      } catch (e) {
        setStatus("Error: " + String(e));
      }
      return;
    }
    const snippetCreated = snippetContentSet.has(
      normalizeContentForMatch(entry.content || "")
    );
    onOpenViewFull(entry.content, { index: idx, canPromote: !snippetCreated });
  };

  const handleViewParentImage = async (parentId: number) => {
    try {
      await getTaurpc().open_clipboard_image_by_id(parentId);
      setStatus("Opened source image.");
    } catch (e) {
      setStatus("Error: " + String(e));
    }
  };

  const handleSaveImageAs = async (idx: number) => {
    const entry = entries[idx];
    if (!entry || entry.entry_type !== "image") return;
    const path = await save({
      title: "Save Clipboard Image As",
      defaultPath: "clipboard_image.png",
      filters: [{ name: "PNG", extensions: ["png"] }],
    });
    if (!path) return;
    try {
      await getTaurpc().save_clipboard_image_by_id(entry.id, String(path));
      setStatus(`Saved image to ${String(path)}`);
    } catch (e) {
      setStatus("Error: " + String(e));
    }
  };

  const handleDeleteClick = (idx: number) => {
    onOpenClipEntryDeleteConfirm(idx);
  };

  const handlePromote = async (idx: number) => {
    const entry = entries[idx];
    if (!entry) return;
    const categories = appState?.categories || ["General"];
    const trigger = (entry.content || "")
      .slice(0, 20)
      .replace(/\s/g, "")
      .trim() || "clip";
    onOpenSnippetEditor("add", categories[0] || "General", -1, {
      content: entry.content,
      trigger,
    });
  };

  const depth = appState?.clip_history_max_depth ?? maxDepth ?? 20;
  const depthLabel = depth === 0 ? "Unlimited" : String(depth);

  return (
    <div className="p-4 border border-[var(--dc-border)] rounded mt-2">
      <h2 className="text-xl font-semibold mb-4">Clipboard History</h2>
      <p className="mb-2">
        <span className="inline-flex items-center gap-2 px-2 py-1 border border-[var(--dc-border)] rounded mr-2 align-middle">
          <Search className="w-4 h-4" aria-hidden />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search content/app/window..."
            className="bg-transparent outline-none min-w-[280px]"
          />
        </span>
        <select
          value={searchOperator}
          onChange={(e) =>
            setSearchOperator((e.target.value as "or" | "and" | "regex") || "or")
          }
          className="px-2 py-1.5 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded mr-2"
          title="Search operator"
        >
          <option value="or">OR</option>
          <option value="and">AND</option>
          <option value="regex">REGEX</option>
        </select>
        <button
          onClick={loadEntries}
          className="px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
        >
          Refresh
        </button>
        <button
          onClick={onOpenClipClearConfirm}
          className="ml-2 px-3 py-1.5 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
        >
          Clear All History
        </button>
        <span className="ml-2 text-sm text-[var(--dc-text-muted)]">
          {status}
        </span>
      </p>
      <div>
        <p className="mb-2">
          Real-time Clipboard History: {entries.length} of {depthLabel} entries
        </p>
        {loading ? (
          <p>Loading...</p>
        ) : entries.length === 0 ? (
          <p>No clipboard history.</p>
        ) : (
          <table className="w-full border-collapse border border-[var(--dc-border)]">
            <thead>
              <tr>
                <th className="border border-[var(--dc-border)] p-1.5 text-left">
                  #
                </th>
                <th className="border border-[var(--dc-border)] p-1.5 text-left">
                  Actions
                </th>
                <th className="border border-[var(--dc-border)] p-1.5 text-left">
                  Content Preview
                </th>
                <th className="border border-[var(--dc-border)] p-1.5 text-left">
                  App
                </th>
                <th className="border border-[var(--dc-border)] p-1.5 text-left">
                  Window Title
                </th>
                <th className="border border-[var(--dc-border)] p-1.5 text-left">
                  Length
                </th>
                <th className="border border-[var(--dc-border)] p-1.5 text-left">
                  Created (UTC)
                </th>
              </tr>
            </thead>
            <tbody>
              {entries.map((e, i) => {
                const preview =
                  e.entry_type === "image"
                    ? `Image ${e.image_width || "?"}x${e.image_height || "?"}${e.mime_type ? ` (${e.mime_type})` : ""
                    }`
                    : (e.content || "").slice(0, 40) +
                    (e.content?.length && e.content.length > 40 ? "..." : "");
                const app = (e.process_name || "(unknown)").slice(0, 20);
                const title = (e.window_title || "(unknown)").slice(0, 30);
                const isImage = e.entry_type === "image";
                const thumbUrl = toFileUrl(e.thumb_path || e.image_path || "");
                const canRenderThumb = isImage && !!thumbUrl && !thumbLoadFailed[e.id];
                const snippetCreated = !isImage
                  ? snippetContentSet.has(normalizeContentForMatch(e.content || ""))
                  : false;
                return (
                  <tr
                    key={i}
                    className="even:bg-[var(--dc-bg-alt)] hover:bg-[var(--dc-bg-tertiary)] cursor-context-menu"
                    onContextMenu={(e) => {
                      e.preventDefault();
                      const entry = entries[i];
                      if (!entry) return;
                      const actions: NativeContextMenuAction[] = [
                        {
                          id: "view-full",
                          icon: "👁",
                          text: isImage ? "Open Image" : "View Full Content",
                          onClick: () => void handleView(i),
                        },
                        isImage
                          ? {
                            id: "save-image-as",
                            icon: "💾",
                            text: "Save Image As",
                            onClick: () => void handleSaveImageAs(i),
                          }
                          : snippetCreated
                            ? {
                              id: "promoted",
                              icon: "✓",
                              text: "Promoted",
                              enabled: false,
                              onClick: () => { },
                            }
                            : {
                              id: "promote",
                              icon: "⬆",
                              text: "Promote to Snippet",
                              onClick: () => handlePromote(i),
                            },
                        entry.parent_id
                          ? {
                            id: "view-source-image",
                            icon: "🖼",
                            text: "View Source Image",
                            onClick: () => void handleViewParentImage(entry.parent_id!),
                          }
                          : null,
                        {
                          id: "copy",
                          icon: "⧉",
                          text: isImage ? "Copy Image" : "Copy to Clipboard",
                          onClick: () => void handleCopy(i),
                        },
                        {
                          id: "delete",
                          icon: "🗑",
                          text: "Delete",
                          onClick: () => handleDeleteClick(i),
                        },
                      ].filter(Boolean) as NativeContextMenuAction[];
                      showNativeContextMenu(e.clientX, e.clientY, actions);
                    }}
                  >
                    <td className="border border-[var(--dc-border)] p-1.5">
                      {i + 1}
                    </td>
                    <td className="border border-[var(--dc-border)] p-1.5">
                      <div className="flex flex-wrap gap-1">
                        <button
                          onClick={() => void handleCopy(i)}
                          className="inline-flex items-center px-2 py-0.5 text-xs bg-[var(--dc-bg-alt)] hover:bg-[var(--dc-bg-tertiary)] border border-[var(--dc-border)] rounded transition-colors"
                          title={isImage ? "Copy Image" : "Copy to Clipboard"}
                        >
                          <Copy className="w-3 h-3 mr-1" aria-hidden />
                          {isImage ? "Copy" : "Copy"}
                        </button>
                        {copiedIdx === i && (
                          <span className="text-emerald-500 text-[10px] font-medium flex items-center animate-in fade-in duration-300">
                            Copied!
                          </span>
                        )}
                        <button
                          onClick={() => void handleView(i)}
                          className="inline-flex items-center px-2 py-0.5 text-xs bg-[var(--dc-bg-alt)] hover:bg-[var(--dc-bg-tertiary)] border border-[var(--dc-border)] rounded transition-colors"
                          title={isImage ? "Open Image" : "View Full Content"}
                        >
                          <Eye className="w-3 h-3 mr-1" aria-hidden />
                          {isImage ? "Open" : "View"}
                        </button>
                        {e.parent_id && (
                          <button
                            onClick={() => void handleViewParentImage(e.parent_id!)}
                            className="inline-flex items-center px-2 py-0.5 text-xs bg-[var(--dc-bg-alt)] hover:bg-[var(--dc-bg-tertiary)] border border-[var(--dc-border)] rounded transition-colors"
                            title="Open Source Image"
                          >
                            <ImageIcon className="w-3 h-3 mr-1" aria-hidden />
                            Image
                          </button>
                        )}
                        {isImage && (
                          <button
                            onClick={() => void handleSaveImageAs(i)}
                            className="inline-flex items-center px-2 py-0.5 text-xs bg-[var(--dc-bg-alt)] hover:bg-[var(--dc-bg-tertiary)] border border-[var(--dc-border)] rounded transition-colors"
                            title="Save Image As"
                          >
                            Save As
                          </button>
                        )}
                        <button
                          onClick={() => handleDeleteClick(i)}
                          className="inline-flex items-center px-2 py-0.5 text-xs bg-[var(--dc-bg-alt)] hover:bg-red-500/10 text-[var(--dc-error)] border border-[var(--dc-border)] rounded transition-colors"
                          title="Delete Entry"
                        >
                          <Trash2 className="w-3 h-3 mr-1" aria-hidden />
                          Delete
                        </button>
                        {!isImage && (
                          <button
                            onClick={() => handlePromote(i)}
                            className={`inline-flex items-center px-2 py-0.5 text-xs border border-[var(--dc-border)] rounded transition-colors ${snippetCreated
                              ? "bg-emerald-500/10 text-emerald-500 cursor-default font-medium"
                              : "bg-[var(--dc-bg-alt)] hover:bg-[var(--dc-bg-tertiary)]"
                              }`}
                            disabled={snippetCreated}
                            title={snippetCreated ? "Snippet already created" : "Promote to Snippet"}
                          >
                            {snippetCreated ? (
                              <Check className="w-3 h-3 mr-1" aria-hidden />
                            ) : (
                              <ArrowUpToLine className="w-3 h-3 mr-1" aria-hidden />
                            )}
                            {snippetCreated ? "Promoted" : "Promote"}
                          </button>
                        )}
                      </div>
                    </td>
                    <td className="border border-[var(--dc-border)] p-1.5">
                      {canRenderThumb ? (
                        <button
                          type="button"
                          onClick={() => void handleView(i)}
                          className="border border-[var(--dc-border)] rounded overflow-hidden hover:opacity-90"
                          title="Open full image"
                        >
                          <img
                            src={thumbUrl}
                            alt={`Clipboard thumbnail ${e.id}`}
                            className="block max-h-16 max-w-28 object-cover bg-[var(--dc-bg-alt)]"
                            loading="lazy"
                            onError={() =>
                              setThumbLoadFailed((prev) => ({ ...prev, [e.id]: true }))
                            }
                          />
                        </button>
                      ) : (
                        <div className="flex items-center gap-2">
                          {e.parent_id && (
                            <button
                              type="button"
                              onClick={(ev) => {
                                ev.stopPropagation();
                                handleViewParentImage(e.parent_id!);
                              }}
                              className="text-[var(--dc-accent)] hover:text-[var(--dc-accent-hover)] transition-colors p-0.5"
                              title="View Source Image"
                            >
                              <ImageIcon className="w-4 h-4" />
                            </button>
                          )}
                          <span className="truncate">
                            {preview}
                          </span>
                        </div>
                      )}
                    </td>
                    <td className="border border-[var(--dc-border)] p-1.5">
                      {app}
                    </td>
                    <td className="border border-[var(--dc-border)] p-1.5">
                      {title}
                    </td>
                    <td className="border border-[var(--dc-border)] p-1.5">
                      {e.length ?? 0}
                    </td>
                    <td className="border border-[var(--dc-border)] p-1.5 whitespace-nowrap">
                      {formatUtcTimestamp(e.created_at)}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>
    </div >
  );
}
