/**
 * Web Worker for fuzzy search using Fuse.js.
 * Keeps search off the main thread for responsive UI.
 */
import Fuse from "fuse.js";

export interface SnippetForSearch {
  trigger: string;
  content: string;
  category: string;
  [key: string]: unknown;
}

export interface SearchRequest {
  id: number;
  snippets: SnippetForSearch[];
  query: string;
  limit?: number;
}

export interface SearchResponse {
  id: number;
  results: SnippetForSearch[];
}

let fuse: Fuse<SnippetForSearch> | null = null;
let lastSnippets: SnippetForSearch[] | null = null;

function ensureFuse(snippets: SnippetForSearch[]): Fuse<SnippetForSearch> {
  if (fuse && lastSnippets === snippets) return fuse;
  lastSnippets = snippets;
  fuse = new Fuse(snippets, {
    keys: ["trigger", "content", "category"],
    threshold: 0.3,
  });
  return fuse;
}

self.onmessage = (e: MessageEvent<SearchRequest>) => {
  const { id, snippets, query, limit = 50 } = e.data;
  try {
    if (!query.trim()) {
      const results = snippets.slice(0, limit);
      self.postMessage({ id, results } satisfies SearchResponse);
      return;
    }
    const f = ensureFuse(snippets);
    const searchResults = f.search(query);
    const results = searchResults.map((r) => r.item).slice(0, limit);
    self.postMessage({ id, results } satisfies SearchResponse);
  } catch (err) {
    console.error("[fuzzy-search.worker]", err);
    self.postMessage({ id, results: snippets.slice(0, limit) } satisfies SearchResponse);
  }
};
