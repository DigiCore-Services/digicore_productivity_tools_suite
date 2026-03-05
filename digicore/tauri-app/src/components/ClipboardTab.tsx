import { useCallback, useEffect, useMemo, useState } from "react";
import { getTaurpc } from "@/lib/taurpc";
import { showNativeContextMenu } from "@/lib/nativeContextMenu";
import { ArrowUpToLine, Check, Copy, Eye, Trash2 } from "lucide-react";
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
  const normalizeContentForMatch = useCallback((value: string) => {
    return (value || "").replace(/\r\n/g, "\n").trim();
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
      const [data, state] = await Promise.all([
        getTaurpc().get_clipboard_entries(),
        appState ? Promise.resolve(null) : getTaurpc().get_app_state().catch(() => null),
      ]);
      setEntries(data);
      if (state) setMaxDepth(state.clip_history_max_depth ?? 20);
    } catch (e) {
      setStatus("Error: " + String(e));
    } finally {
      setLoading(false);
    }
  }, [appState]);

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
      await getTaurpc().copy_to_clipboard(entry.content);
      setStatus("Copied to clipboard!");
    } catch (e) {
      setStatus("Error: " + String(e));
    }
  };

  const handleView = (idx: number) => {
    const entry = entries[idx];
    if (!entry) return;
    const snippetCreated = snippetContentSet.has(
      normalizeContentForMatch(entry.content || "")
    );
    onOpenViewFull(entry.content, { index: idx, canPromote: !snippetCreated });
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

  return (
    <div className="p-4 border border-[var(--dc-border)] rounded mt-2">
      <h2 className="text-xl font-semibold mb-4">Clipboard History</h2>
      <p className="mb-2">
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
          Real-time Clipboard History: {entries.length} of {depth} entries
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
                  Actions
                </th>
              </tr>
            </thead>
            <tbody>
              {entries.map((e, i) => {
                const preview =
                  (e.content || "").slice(0, 40) +
                  (e.content?.length && e.content.length > 40 ? "..." : "");
                const app = (e.process_name || "(unknown)").slice(0, 20);
                const title = (e.window_title || "(unknown)").slice(0, 30);
                const snippetCreated = snippetContentSet.has(
                  normalizeContentForMatch(e.content || "")
                );
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
                          text: "View Full Content",
                          onClick: () => handleView(i),
                        },
                        snippetCreated
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
                          text: "Copy to Clipboard",
                          onClick: () => handleCopy(i),
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
                      {snippetCreated ? (
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
                      {preview}
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
                    <td className="border border-[var(--dc-border)] p-1.5">
                      <button
                        onClick={() => handleCopy(i)}
                        className="inline-flex items-center px-2.5 py-1 text-sm mr-1"
                      >
                        <Copy className="w-3.5 h-3.5 mr-1" aria-hidden />
                        Copy
                      </button>
                      <button
                        onClick={() => handleView(i)}
                        className="inline-flex items-center px-2.5 py-1 text-sm mr-1"
                      >
                        <Eye className="w-3.5 h-3.5 mr-1" aria-hidden />
                        View
                      </button>
                      <button
                        onClick={() => handleDeleteClick(i)}
                        className="inline-flex items-center px-2.5 py-1 text-sm mr-1 text-[var(--dc-error)]"
                      >
                        <Trash2 className="w-3.5 h-3.5 mr-1" aria-hidden />
                        Delete
                      </button>
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
