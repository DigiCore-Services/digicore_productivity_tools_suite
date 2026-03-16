/**
 * Hook to run fuzzy search in a Web Worker.
 * Falls back to main-thread search if Worker is unavailable.
 */
import { useEffect, useState } from "react";
import Fuse from "fuse.js";
export interface SnippetWithCategory {
  trigger: string;
  content: string;
  category: string;
  [key: string]: unknown;
}

let worker: Worker | null = null;
let workerReqId = 0;
const pending = new Map<number, (results: SnippetWithCategory[]) => void>();

function getWorker(): Worker | null {
  if (worker) return worker;
  try {
    worker = new Worker(
      new URL("../workers/fuzzy-search.worker.ts", import.meta.url),
      { type: "module" }
    );
    worker.onmessage = (e: MessageEvent<{ id: number; results: SnippetWithCategory[] }>) => {
      const resolve = pending.get(e.data.id);
      if (resolve) {
        pending.delete(e.data.id);
        resolve(e.data.results);
      }
    };
    return worker;
  } catch {
    return null;
  }
}

function searchInWorker(
  snippets: SnippetWithCategory[],
  query: string,
  limit: number
): Promise<SnippetWithCategory[]> {
  const w = getWorker();
  if (!w) return Promise.resolve(searchMainThread(snippets, query, limit));
  return new Promise((resolve) => {
    const id = ++workerReqId;
    pending.set(id, resolve);
    w.postMessage({ id, snippets, query, limit });
  });
}

function searchMainThread(
  snippets: SnippetWithCategory[],
  query: string,
  limit: number
): SnippetWithCategory[] {
  if (!query.trim()) return snippets.slice(0, limit);
  const fuse = new Fuse(snippets, {
    keys: ["trigger", "content", "category"],
    threshold: 0.3,
  });
  return fuse.search(query).map((r) => r.item).slice(0, limit);
}

export function useFuzzySearch(
  snippets: SnippetWithCategory[],
  query: string
): { results: SnippetWithCategory[]; loading: boolean } {
  const [results, setResults] = useState<SnippetWithCategory[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    searchInWorker(snippets, query, 50).then((r) => {
      if (!cancelled) {
        setResults(r);
        setLoading(false);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [snippets, query]);

  return { results, loading };
}
