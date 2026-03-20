/**
 * Hook for loading snippet rows from SQLite on demand (partial loading).
 * Use when library exceeds SQLITE_PARTIAL_THRESHOLD to avoid loading all into memory.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { loadSnippetsPage, type SnippetRow } from "./sqliteLoad";
import type { Snippet } from "../types";

export const SQLITE_PARTIAL_THRESHOLD = 5000;
export const SQLITE_PAGE_SIZE = 100;

export type LibraryRow = { cat: string; s: Snippet; idx: number };

function rowToSnippet(r: SnippetRow): Snippet {
  return {
    trigger: r.trigger,
    trigger_type: r.trigger_type,
    content: r.content,
    htmlContent: r.html_content,
    rtfContent: r.rtf_content,
    options: r.options,
    category: r.category,
    profile: r.profile,
    appLock: r.app_lock,
    pinned: r.pinned,
    lastModified: r.last_modified,
  };
}

function findSnippetIdx(
  library: Record<string, Snippet[]>,
  cat: string,
  trigger: string,
  content: string
): number {
  const snips = library[cat];
  if (!snips) return 0;
  const idx = snips.findIndex(
    (s) => s.trigger === trigger && s.content === content
  );
  return idx >= 0 ? idx : 0;
}

export function useSqliteRows(
  totalCount: number,
  search: string,
  library: Record<string, Snippet[]>
) {
  const [rows, setRows] = useState<(LibraryRow | undefined)[]>([]);
  const [total, setTotal] = useState(0);
  const loadingRef = useRef<Set<number>>(new Set());
  const cacheRef = useRef<Map<number, LibraryRow[]>>(new Map());

  const fetchPage = useCallback(
    async (pageIndex: number) => {
      if (loadingRef.current.has(pageIndex)) return;
      loadingRef.current.add(pageIndex);
      try {
        const { rows: pageRows, total: t } = await loadSnippetsPage(
          pageIndex * SQLITE_PAGE_SIZE,
          SQLITE_PAGE_SIZE,
          search.trim() || undefined
        );
        setTotal((prev) => (t > 0 ? t : prev));
        const mapped: LibraryRow[] = pageRows.map((r) => ({
          cat: r.category,
          s: rowToSnippet(r),
          idx: findSnippetIdx(library, r.category, r.trigger, r.content),
        }));
        cacheRef.current.set(pageIndex, mapped);
        setRows((prev) => {
          const next = [...prev];
          const start = pageIndex * SQLITE_PAGE_SIZE;
          for (let i = 0; i < mapped.length; i++) {
            next[start + i] = mapped[i];
          }
          return next;
        });
      } finally {
        loadingRef.current.delete(pageIndex);
      }
    },
    [search, library]
  );

  useEffect(() => {
    if (
      totalCount <= SQLITE_PARTIAL_THRESHOLD ||
      Object.keys(library).length === 0
    )
      return;
    setRows([]);
    setTotal(0);
    cacheRef.current.clear();
    loadSnippetsPage(0, 1, search.trim() || undefined).then(({ total: t }) => {
      setTotal(t);
      setRows(Array(Math.max(0, t)).fill(undefined));
      fetchPage(0);
    });
  }, [totalCount, search, fetchPage, library]);

  const getRow = useCallback(
    (index: number): LibraryRow | undefined => {
      if (totalCount <= SQLITE_PARTIAL_THRESHOLD) return undefined;
      const pageIndex = Math.floor(index / SQLITE_PAGE_SIZE);
      if (!cacheRef.current.has(pageIndex)) {
        fetchPage(pageIndex);
      }
      return rows[index];
    },
    [totalCount, rows, fetchPage]
  );

  return { rows, total, getRow, fetchPage };
}
