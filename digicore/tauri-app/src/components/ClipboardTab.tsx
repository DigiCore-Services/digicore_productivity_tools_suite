import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getTaurpc } from "@/lib/taurpc";
import { showNativeContextMenu } from "@/lib/nativeContextMenu";
import { ArrowUpToLine, Check, Copy, Eye, Search, Trash2 } from "lucide-react";
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
          `Search loaded (${searchOperator.toUpperCase()}): ${data.length} match${
            data.length === 1 ? "" : "es"
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
                  Snippet Created
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
                <th className="border border-[var(--dc-border)] p-1.5 text-left">
                  Actions
                </th>
              </tr>
            </thead>
            <tbody>
              {entries.map((e, i) => {
                const preview =
                  e.entry_type === "image"
                    ? `Image ${e.image_width || "?"}x${e.image_height || "?"}${
                        e.mime_type ? ` (${e.mime_type})` : ""
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
                              onClick: () => {},
                            }
                          : {
                              id: "promote",
                              icon: "⬆",
                              text: "Promote to Snippet",
                              onClick: () => handlePromote(i),
                            },
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
                      ];
                      showNativeContextMenu(e.clientX, e.clientY, actions);
                    }}
                  >
                    <td className="border border-[var(--dc-border)] p-1.5">
                      {i + 1}
                    </td>
                    <td className="border border-[var(--dc-border)] p-1.5 text-center">
                      {isImage ? (
                        <span className="w-4 h-4 inline-block" aria-hidden />
                      ) : snippetCreated ? (
                        <span title="Snippet already created">
                          <Check
                            className="w-4 h-4 mx-auto text-emerald-500"
                            aria-hidden
                          />
                        </span>
                      ) : (
                        <span className="w-4 h-4 inline-block" aria-hidden />
                      )}
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
                        preview
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
                    <td className="border border-[var(--dc-border)] p-1.5">
                      <button
                        onClick={() => void handleCopy(i)}
                        className="inline-flex items-center px-2.5 py-1 text-sm mr-1"
                      >
                        <Copy className="w-3.5 h-3.5 mr-1" aria-hidden />
                        {isImage ? "Copy Image" : "Copy"}
                      </button>
                      <button
                        onClick={() => void handleView(i)}
                        className="inline-flex items-center px-2.5 py-1 text-sm mr-1"
                      >
                        <Eye className="w-3.5 h-3.5 mr-1" aria-hidden />
                        {isImage ? "Open Image" : "View"}
                      </button>
                      {isImage && (
                        <button
                          onClick={() => void handleSaveImageAs(i)}
                          className="inline-flex items-center px-2.5 py-1 text-sm mr-1"
                        >
                          Save As
                        </button>
                      )}
                      <button
                        onClick={() => handleDeleteClick(i)}
                        className="inline-flex items-center px-2.5 py-1 text-sm mr-1 text-[var(--dc-error)]"
                      >
                        <Trash2 className="w-3.5 h-3.5 mr-1" aria-hidden />
                        Delete
                      </button>
                      {!isImage && (
                        <button
                          onClick={() => handlePromote(i)}
                          className={`inline-flex items-center px-2.5 py-1 text-sm ${
                            snippetCreated ? "opacity-50 cursor-not-allowed" : ""
                          }`}
                          disabled={snippetCreated}
                        >
                          <ArrowUpToLine className="w-3.5 h-3.5 mr-1" aria-hidden />
                          {snippetCreated ? "Promoted" : "Promote"}
                        </button>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}
