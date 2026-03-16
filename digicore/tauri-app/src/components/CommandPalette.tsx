import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { getTaurpc } from "@/lib/taurpc";
import { Search } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import { useFuzzySearch, type SnippetWithCategory } from "@/lib/useFuzzySearch";
import type { AppState, Snippet } from "../types";

function flattenSnippets(library: Record<string, Snippet[]>): SnippetWithCategory[] {
  const out: SnippetWithCategory[] = [];
  for (const [cat, snippets] of Object.entries(library || {})) {
    for (const s of snippets) {
      out.push({ ...s, category: cat });
    }
  }
  return out;
}

interface CommandPaletteProps {
  visible: boolean;
  appState: AppState | null;
  onClose: () => void;
  onOpenSnippetEditor: (
    mode: "edit",
    category: string,
    snippetIdx: number
  ) => void;
}

export function CommandPalette({
  visible,
  appState,
  onClose,
  onOpenSnippetEditor,
}: CommandPaletteProps) {
  const [query, setQuery] = useState("");
  const [selectedIdx, setSelectedIdx] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  const allSnippets = useMemo(
    () => flattenSnippets(appState?.library ?? {}),
    [appState?.library]
  );

  const { results, loading } = useFuzzySearch(allSnippets, query);

  const selectedItem = results[selectedIdx] ?? null;

  const copyToClipboard = useCallback(async (content: string) => {
    try {
      await getTaurpc().copy_to_clipboard(content);
    } catch {
      /* ignore */
    }
  }, []);

  const handleSelect = useCallback(
    (item: SnippetWithCategory, edit: boolean) => {
      if (edit) {
        const snippets = appState?.library?.[item.category] ?? [];
        const idx = snippets.findIndex((s) => s.trigger === item.trigger);
        if (idx >= 0) {
          onOpenSnippetEditor("edit", item.category, idx);
        }
      } else {
        copyToClipboard(item.content || "");
      }
      onClose();
    },
    [appState?.library, onOpenSnippetEditor, copyToClipboard, onClose]
  );

  useEffect(() => {
    if (visible) {
      setQuery("");
      setSelectedIdx(0);
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [visible]);

  useEffect(() => {
    setSelectedIdx(0);
  }, [query]);

  useEffect(() => {
    if (!visible) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
        return;
      }
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIdx((i) => Math.min(i + 1, results.length - 1));
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIdx((i) => Math.max(i - 1, 0));
        return;
      }
      if (e.key === "Enter") {
        e.preventDefault();
        if (e.ctrlKey) {
          if (selectedItem) handleSelect(selectedItem, true);
        } else {
          if (selectedItem) handleSelect(selectedItem, false);
        }
        return;
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [visible, results.length, selectedItem, handleSelect, onClose]);

  useEffect(() => {
    if (!listRef.current || selectedIdx < 0) return;
    const el = listRef.current.children[selectedIdx] as HTMLElement;
    el?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [selectedIdx]);

  if (!visible) return null;

  return (
    <AnimatePresence>
      <motion.div
        className="fixed inset-0 z-[100] flex items-start justify-center pt-[15vh] bg-black/40"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        onClick={onClose}
        role="dialog"
        aria-modal="true"
        aria-label="Command palette - search snippets"
      >
        <motion.div
          className="w-full max-w-xl rounded-lg border border-[var(--dc-border)] bg-[var(--dc-bg-elevated)] shadow-xl overflow-hidden"
          initial={{ opacity: 0, scale: 0.96, y: -10 }}
          animate={{ opacity: 1, scale: 1, y: 0 }}
          exit={{ opacity: 0, scale: 0.96, y: -10 }}
          transition={{ duration: 0.15 }}
          onClick={(e) => e.stopPropagation()}
        >
          <div className="flex items-center gap-2 px-4 py-3 border-b border-[var(--dc-border)]">
            <Search className="h-4 w-4 text-[var(--dc-text-muted)] shrink-0" />
            <input
              ref={inputRef}
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search snippets..."
              className="flex-1 bg-transparent text-[var(--dc-text)] placeholder:text-[var(--dc-text-muted)] outline-none text-sm"
              autoComplete="off"
              aria-label="Search snippets"
              aria-describedby="command-palette-hint"
            />
            <span id="command-palette-hint" className="text-xs text-[var(--dc-text-muted)]">
              Enter: copy | Ctrl+E: edit
            </span>
          </div>
          <div
            ref={listRef}
            className="max-h-[60vh] overflow-y-auto py-2"
            role="listbox"
            aria-label="Search results"
          >
            {loading ? (
              <div className="px-4 py-8 text-center text-sm text-[var(--dc-text-muted)]">
                Searching...
              </div>
            ) : results.length === 0 ? (
              <div className="px-4 py-8 text-center text-sm text-[var(--dc-text-muted)]">
                No snippets found
              </div>
            ) : (
              results.map((item, i) => (
                <button
                  key={`${item.category}-${item.trigger}-${i}`}
                  type="button"
                  role="option"
                  aria-selected={i === selectedIdx}
                  className={`w-full flex flex-col items-start gap-0.5 px-4 py-2.5 text-left text-sm transition-colors ${
                    i === selectedIdx
                      ? "bg-[var(--dc-accent)]/20 text-[var(--dc-text)]"
                      : "hover:bg-[var(--dc-bg-tertiary)] text-[var(--dc-text)]"
                  }`}
                  onClick={() => handleSelect(item, false)}
                  onDoubleClick={() => handleSelect(item, true)}
                >
                  <span className="font-medium">{item.trigger}</span>
                  <span className="text-xs text-[var(--dc-text-muted)] truncate w-full">
                    {item.category} · {(item.content || "").slice(0, 60)}
                    {(item.content?.length ?? 0) > 60 ? "..." : ""}
                  </span>
                </button>
              ))
            )}
          </div>
        </motion.div>
      </motion.div>
    </AnimatePresence>
  );
}
