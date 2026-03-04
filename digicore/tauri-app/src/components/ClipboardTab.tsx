import { useCallback, useEffect, useState } from "react";
import { getTaurpc } from "@/lib/taurpc";
import { showNativeContextMenu } from "@/lib/nativeContextMenu";
import type { AppState, ClipEntry } from "../types";

interface ClipboardTabProps {
  appState: AppState | null;
  refreshTrigger?: number;
  onOpenViewFull: (content: string) => void;
  onOpenSnippetEditor: (
    mode: "add",
    category: string,
    snippetIdx: number,
    prefill?: { content: string; trigger: string }
  ) => void;
  onOpenClipClearConfirm: () => void;
}

export function ClipboardTab({
  appState,
  refreshTrigger = 0,
  onOpenViewFull,
  onOpenSnippetEditor,
  onOpenClipClearConfirm,
}: ClipboardTabProps) {
  const [entries, setEntries] = useState<ClipEntry[]>([]);
  const [status, setStatus] = useState("");
  const [loading, setLoading] = useState(false);

  const loadEntries = useCallback(async () => {
    setLoading(true);
    try {
      const data = await getTaurpc().get_clipboard_entries();
      setEntries(data);
    } catch (e) {
      setStatus("Error: " + String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadEntries();
  }, [loadEntries, refreshTrigger]);

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
    if (entry) onOpenViewFull(entry.content);
  };

  const handleDelete = async (idx: number) => {
    try {
      await getTaurpc().delete_clip_entry(idx);
      await loadEntries();
      setStatus("Entry deleted.");
    } catch (e) {
      setStatus("Error: " + String(e));
    }
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

  const depth = appState?.clip_history_max_depth ?? 20;

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
          Real-time Clipboard History (Last {depth})
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
                return (
                  <tr
                    key={i}
                    className="even:bg-[var(--dc-bg-alt)] hover:bg-[var(--dc-bg-tertiary)] cursor-context-menu"
                    onContextMenu={(e) => {
                      e.preventDefault();
                      const entry = entries[i];
                      if (!entry) return;
                      showNativeContextMenu(e.clientX, e.clientY, [
                        {
                          id: "copy",
                          text: "Copy to Clipboard",
                          onClick: () => handleCopy(i),
                        },
                        {
                          id: "view-full",
                          text: "View Full Content",
                          onClick: () => handleView(i),
                        },
                        {
                          id: "promote",
                          text: "Promote to Snippet",
                          onClick: () => handlePromote(i),
                        },
                        {
                          id: "delete",
                          text: "Delete",
                          onClick: () => handleDelete(i),
                        },
                      ]);
                    }}
                  >
                    <td className="border border-[var(--dc-border)] p-1.5">
                      {i + 1}
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
                        className="px-2.5 py-1 text-sm mr-1"
                      >
                        Copy
                      </button>
                      <button
                        onClick={() => handleView(i)}
                        className="px-2.5 py-1 text-sm mr-1"
                      >
                        View
                      </button>
                      <button
                        onClick={() => handleDelete(i)}
                        className="px-2.5 py-1 text-sm mr-1"
                      >
                        Delete
                      </button>
                      <button
                        onClick={() => handlePromote(i)}
                        className="px-2.5 py-1 text-sm"
                      >
                        Promote
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
