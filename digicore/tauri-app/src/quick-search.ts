import { getTaurpc } from "@/lib/taurpc";
import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { confirm } from "@tauri-apps/plugin-dialog";
import { applyThemeToDocument, resolveTheme } from "@/lib/theme";
import type { Snippet } from "@/bindings";

type QuickSnippet = {
  category: string;
  snippetIdx: number;
  snippet: Snippet;
};

const api = getTaurpc();
const win = getCurrentWebviewWindow();

let allSnippets: QuickSnippet[] = [];
let filtered: QuickSnippet[] = [];
let selectedIndex = 0;

function onPointerEnter(): void {
  // Capture target app before this webview gains focus.
  if (!document.hasFocus()) {
    api.ghost_follower_capture_target_window().catch(() => {});
  }
}

function escapeHtml(value: string): string {
  const div = document.createElement("div");
  div.textContent = value || "";
  return div.innerHTML;
}

function normalize(value: string): string {
  return (value || "").toLowerCase().trim();
}

function normalizeContentForMatch(value: string): string {
  return (value || "").replace(/\r\n/g, "\n").trim();
}

function matchesQuery(entry: QuickSnippet, query: string): boolean {
  const q = normalize(query);
  if (!q) return true;
  const terms = q.split(/\s+/).filter(Boolean);
  const haystack = normalize(
    `${entry.snippet.trigger || ""} ${entry.snippet.content || ""} ${entry.category || ""}`
  );
  return terms.every((term) => haystack.includes(term));
}

function hideContextMenu(): void {
  const menu = document.getElementById("ctx-menu");
  if (!menu) return;
  menu.style.display = "none";
  menu.innerHTML = "";
}

function showContextMenu(
  x: number,
  y: number,
  items: Array<{ text?: string; icon?: string; onClick?: () => void; danger?: boolean; separator?: boolean }>
): void {
  const menu = document.getElementById("ctx-menu");
  if (!menu) return;
  menu.innerHTML = "";
  items.forEach((item) => {
    if (item.separator) {
      const sep = document.createElement("div");
      sep.className = "ctx-sep";
      menu.appendChild(sep);
      return;
    }
    const row = document.createElement("div");
    row.className = `ctx-item${item.danger ? " danger" : ""}`;
    const icon = document.createElement("span");
    icon.className = "ctx-icon";
    icon.textContent = item.icon || "";
    const text = document.createElement("span");
    text.textContent = item.text || "";
    row.append(icon, text);
    row.addEventListener("click", () => {
      hideContextMenu();
      item.onClick?.();
    });
    menu.appendChild(row);
  });
  menu.style.left = `${x}px`;
  menu.style.top = `${y}px`;
  menu.style.display = "block";
}

function getPreview(content: string): string {
  const normalized = normalizeContentForMatch(content).replace(/\n/g, " ");
  return normalized.length > 140 ? `${normalized.slice(0, 140)}...` : normalized;
}

async function loadData(): Promise<void> {
  const state = await api.get_app_state();
  const results: QuickSnippet[] = [];
  const cats = state.categories || [];
  for (const category of cats) {
    const snippets = state.library?.[category] || [];
    snippets.forEach((snippet, snippetIdx) => {
      results.push({ category, snippetIdx, snippet });
    });
  }
  allSnippets = results;
}

function applyFilter(): void {
  const input = document.getElementById("search") as HTMLInputElement | null;
  const query = input?.value || "";
  filtered = allSnippets.filter((entry) => matchesQuery(entry, query));
  if (selectedIndex >= filtered.length) selectedIndex = 0;
  renderList();
}

function renderList(): void {
  const list = document.getElementById("list");
  if (!list) return;
  if (filtered.length === 0) {
    list.innerHTML = '<div class="empty">No snippets match your search.</div>';
    return;
  }
  let html = "";
  filtered.forEach((entry, idx) => {
    const snippet = entry.snippet;
    const pinned = String(snippet.pinned || "").toLowerCase() === "true";
    html += `<div class="row${idx === selectedIndex ? " selected" : ""}" data-idx="${idx}">
      <div class="row-top">
        <span class="trigger">${escapeHtml(snippet.trigger || "(no trigger)")}</span>
        <span class="badge">${escapeHtml(entry.category)}</span>
        ${pinned ? '<span class="badge">📌 Pinned</span>' : ""}
      </div>
      <div class="preview">${escapeHtml(getPreview(snippet.content || ""))}</div>
    </div>`;
  });
  list.innerHTML = html;

  list.querySelectorAll(".row").forEach((el) => {
    el.addEventListener("click", () => {
      const idx = Number((el as HTMLElement).dataset.idx || "0");
      selectedIndex = Number.isFinite(idx) ? idx : 0;
      renderList();
    });
    el.addEventListener("dblclick", async () => {
      const idx = Number((el as HTMLElement).dataset.idx || "0");
      await insertAtIndex(idx);
    });
    el.addEventListener("contextmenu", (event) => {
      event.preventDefault();
      const mouseEvent = event as MouseEvent;
      const idx = Number((el as HTMLElement).dataset.idx || "0");
      const target = filtered[idx];
      if (!target) return;
      const isPinned = String(target.snippet.pinned || "").toLowerCase() === "true";
      showContextMenu(mouseEvent.clientX, mouseEvent.clientY, [
        {
          icon: "👁",
          text: "View Full Snippet Content",
          onClick: async () => {
            await win.hide();
            await api.bring_main_window_to_foreground();
            await emit("quick-search-view-full", {
              category: target.category,
              snippetIdx: target.snippetIdx,
              content: target.snippet.content || "",
            });
          },
        },
        {
          icon: isPinned ? "📌" : "📍",
          text: isPinned ? "Unpin Snippet" : "Pin Snippet",
          onClick: async () => {
            if (isPinned) {
              const ok = await confirm("Unpin this snippet?", {
                title: "Confirm Unpin",
                kind: "warning",
              });
              if (!ok) return;
            }
            await api.ghost_follower_toggle_pin(target.category, target.snippetIdx);
            await emit("quick-search-library-refresh", {});
            await refreshAndKeepSelection(target.category, target.snippet.trigger || "");
          },
        },
        {
          icon: "📋",
          text: "Copy Full Content to Clipboard",
          onClick: async () => {
            await api.copy_to_clipboard(target.snippet.content || "");
          },
        },
        { separator: true },
        {
          icon: "✏️",
          text: "Edit Snippet",
          onClick: async () => {
            await win.hide();
            await api.bring_main_window_to_foreground();
            await emit("quick-search-edit-snippet", {
              category: target.category,
              snippetIdx: target.snippetIdx,
            });
          },
        },
        {
          icon: "🗑",
          text: "Delete Snippet",
          danger: true,
          onClick: async () => {
            const ok = await confirm("Delete this snippet?", {
              title: "Confirm Delete",
              kind: "warning",
            });
            if (!ok) return;
            await api.delete_snippet(target.category, target.snippetIdx);
            await emit("quick-search-library-refresh", {});
            await refreshAndKeepSelection(target.category, target.snippet.trigger || "");
          },
        },
      ]);
    });
  });
}

async function refreshAndKeepSelection(category: string, trigger: string): Promise<void> {
  await loadData();
  applyFilter();
  const idx = filtered.findIndex(
    (x) => x.category === category && x.snippet.trigger === trigger
  );
  selectedIndex = idx >= 0 ? idx : Math.min(selectedIndex, Math.max(0, filtered.length - 1));
  renderList();
}

async function insertAtIndex(index: number): Promise<void> {
  const target = filtered[index];
  if (!target) return;
  await win.hide();
  // Give focus a moment to return before invoking insert.
  // Do not recapture here: this can overwrite a valid stored target with
  // transient helper windows during focus transitions.
  await new Promise((resolve) => setTimeout(resolve, 40));
  await api.ghost_follower_insert(target.snippet.trigger || "", target.snippet.content || "");
}

async function refresh(): Promise<void> {
  await loadData();
  applyFilter();
}

async function init(): Promise<void> {
  const pref =
    (typeof localStorage !== "undefined" &&
      localStorage.getItem("digicore-theme")) ||
    "light";
  applyThemeToDocument(resolveTheme(pref));
  listen<{ theme: "dark" | "light" }>("digicore-theme-changed", (e) => {
    applyThemeToDocument(e.payload.theme);
  }).catch(() => {});

  const search = document.getElementById("search") as HTMLInputElement | null;
  const btnClose = document.getElementById("btn-close");

  if (search) {
    search.addEventListener("input", () => {
      selectedIndex = 0;
      applyFilter();
    });
    search.addEventListener("keydown", async (event) => {
      if (event.key === "ArrowDown") {
        event.preventDefault();
        selectedIndex = Math.min(selectedIndex + 1, Math.max(0, filtered.length - 1));
        renderList();
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        selectedIndex = Math.max(selectedIndex - 1, 0);
        renderList();
      } else if (event.key === "Enter") {
        event.preventDefault();
        await insertAtIndex(selectedIndex);
      } else if (event.key === "Escape") {
        event.preventDefault();
        await win.hide();
      }
    });
  }

  btnClose?.addEventListener("click", async () => {
    await win.hide();
  });

  document.addEventListener("click", () => hideContextMenu());
  document.body.addEventListener("mouseenter", onPointerEnter);
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      hideContextMenu();
    }
  });

  await listen("quick-search-refresh", async () => {
    await refresh();
    if (search) {
      search.value = "";
      search.focus();
      selectedIndex = 0;
      applyFilter();
    }
  });

  await refresh();
  search?.focus();
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", () => {
    init().catch(() => {});
  });
} else {
  init().catch(() => {});
}
