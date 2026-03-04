/**
 * Ghost Follower overlay entry.
 * Uses TauRPC proxy for type-safe IPC (no invoke).
 */
import { getTaurpc } from "@/lib/taurpc";
import { listen } from "@tauri-apps/api/event";
import { LogicalPosition } from "@tauri-apps/api/dpi";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

declare const window: Window & {
  __TAURI__?: { plugin?: { positioner?: { moveWindow: (pos: unknown) => Promise<void>; Position?: { TopRight?: unknown; TopLeft?: unknown } } } };
};

const positioner = (window as unknown as { __TAURI__?: { plugin?: { positioner?: { moveWindow: (pos: unknown) => Promise<void>; Position?: { TopRight?: unknown; TopLeft?: unknown } } } } }).__TAURI__?.plugin?.positioner;

let fallbackInterval: ReturnType<typeof setInterval> | null = null;
let lastSearch = "";
let pinnedItems: { trigger: string; content: string; content_preview: string; category: string; snippet_idx: number }[] = [];
let clipEntries: { content: string; process_name?: string }[] = [];

function escapeHtml(s: string): string {
  const div = document.createElement("div");
  div.textContent = s || "";
  return div.innerHTML;
}

function showContextMenu(
  x: number,
  y: number,
  items: { text: string; onClick: () => void; danger?: boolean }[]
) {
  const menu = document.getElementById("ctx-menu");
  if (!menu) return;
  menu.innerHTML = "";
  items.forEach(({ text, onClick, danger }) => {
    const el = document.createElement("div");
    el.className = "ctx-menu-item" + (danger ? " danger" : "");
    el.textContent = text;
    el.onclick = () => {
      hideContextMenu();
      onClick();
    };
    menu.appendChild(el);
  });
  menu.style.left = x + "px";
  menu.style.top = y + "px";
  menu.style.display = "block";
}

function hideContextMenu() {
  const menu = document.getElementById("ctx-menu");
  if (menu) menu.style.display = "none";
}

document.addEventListener("click", hideContextMenu);

const api = getTaurpc();

function render(state: { enabled?: boolean; pinned?: typeof pinnedItems } | null) {
  const list = document.getElementById("pinned-list");
  const clipList = document.getElementById("clip-list");
  if (!list || !clipList) return;
  if (!state || !state.enabled) {
    list.innerHTML = "";
    clipList.innerHTML = "";
    return;
  }
  pinnedItems = state.pinned || [];
  let html = "";
  pinnedItems.forEach((p, i) => {
    html += `<div class="pinned-item" data-idx="${i}">
      <span class="trigger">${escapeHtml(p.trigger)}</span>
      <div class="preview">${escapeHtml(p.content_preview)}</div>
    </div>`;
  });
  list.innerHTML = html || '<div style="padding:8px;color:#6b7280">No pinned snippets</div>';
  list.querySelectorAll(".pinned-item").forEach((el) => {
    el.addEventListener("dblclick", () => {
      const idx = parseInt((el as HTMLElement).dataset.idx ?? "", 10);
      const p = pinnedItems[idx];
      if (p) api.ghost_follower_insert(p.trigger, p.content);
    });
        el.addEventListener("contextmenu", (e) => {
          e.preventDefault();
          const ev = e as MouseEvent;
          const idx = parseInt((el as HTMLElement).dataset.idx ?? "", 10);
          const p = pinnedItems[idx];
          if (!p) return;
          showContextMenu(ev.clientX, ev.clientY, [
        { text: "View Full Snippet Content", onClick: () => api.ghost_follower_request_view_full(p.content) },
        { text: "Unpin Snippet", onClick: () => api.ghost_follower_toggle_pin(p.category, p.snippet_idx) },
        { text: "Edit Snippet", onClick: () => api.ghost_follower_request_edit(p.category, p.snippet_idx) },
        { text: "Copy Full Content to Clipboard", onClick: () => api.copy_to_clipboard(p.content) },
        {
          text: "Delete Snippet",
          onClick: () =>
            api.delete_snippet(p.category, p.snippet_idx).then(() => api.save_library()),
          danger: true,
        },
      ]);
    });
  });

  let clipHtml = "";
  clipEntries.forEach((e, i) => {
    const preview = (e.content || "").slice(0, 40) + (e.content?.length && e.content.length > 40 ? "..." : "");
    clipHtml += `<div class="clip-item" data-idx="${i}">
      <span class="preview">${escapeHtml(preview.replace(/\n/g, " "))}</span>
      <div class="meta">${escapeHtml((e.process_name || "").slice(0, 20) || "(unknown)")}</div>
    </div>`;
  });
  clipList.innerHTML = clipHtml || '<div style="padding:8px;color:#6b7280">No clipboard history</div>';
  clipList.querySelectorAll(".clip-item").forEach((el) => {
    el.addEventListener("dblclick", () => {
      const idx = parseInt((el as HTMLElement).dataset.idx ?? "", 10);
      const c = clipEntries[idx];
      if (c) api.ghost_follower_insert("", c.content);
    });
    el.addEventListener("contextmenu", (e) => {
      e.preventDefault();
      const ev = e as MouseEvent;
      const idx = parseInt((el as HTMLElement).dataset.idx ?? "", 10);
      const c = clipEntries[idx];
      if (!c) return;
      const trigger = (c.content || "").slice(0, 20).replace(/\s/g, "").trim() || "clip";
      showContextMenu(ev.clientX, ev.clientY, [
        { text: "Copy to Clipboard", onClick: () => api.copy_to_clipboard(c.content) },
        { text: "View Full Content", onClick: () => api.ghost_follower_request_view_full(c.content) },
        { text: "Delete", onClick: () => api.delete_clip_entry(idx), danger: true },
        { text: "Promote to Snippet", onClick: () => api.ghost_follower_request_promote(c.content, trigger) },
      ]);
    });
  });
}

async function refresh() {
  try {
    const searchInput = document.getElementById("search-input") as HTMLInputElement;
    const search = searchInput?.value || "";
    if (search !== lastSearch) {
      lastSearch = search;
      await api.ghost_follower_set_search(search);
    }
    const [state, clips] = await Promise.all([
      api.get_ghost_follower_state(search || null),
      api.get_clipboard_entries().catch(() => []),
    ]);
    clipEntries = clips || [];
    render(state);
    const w = getCurrentWebviewWindow();
    if (w) {
      let positioned = false;
      if (positioner && state?.enabled && (state as { monitor_primary?: boolean }).monitor_primary) {
        try {
          const edgeRight = (state as { edge_right?: boolean }).edge_right;
          const pos = edgeRight ? positioner.Position?.TopRight : positioner.Position?.TopLeft;
          if (pos !== undefined) {
            await positioner.moveWindow(pos);
            positioned = true;
          }
        } catch {
          /* ignore */
        }
      }
      if (!positioned && state?.enabled) {
        const pos = (state as { position?: [number, number] }).position;
        const x = pos ? pos[0] : null;
        const y = pos ? pos[1] : null;
        const sane =
          typeof x === "number" &&
          typeof y === "number" &&
          x >= -20000 &&
          x <= 20000 &&
          y >= -20000 &&
          y <= 20000;
        try {
          if (sane && x !== null && y !== null) {
            await w.setPosition(new LogicalPosition(x, y));
          } else {
            await w.center();
          }
        } catch {
          await w.center();
        }
      }
      if (state?.enabled) {
        await w.show();
        try {
          await w.setFocus();
        } catch {
          /* ignore */
        }
      } else if (state && !state.enabled) {
        await w.hide();
      }
    }
  } catch {
    /* ignore */
  }
}

async function init() {
  const searchInput = document.getElementById("search-input");
  if (searchInput) {
    searchInput.addEventListener("input", () => {
      lastSearch = "";
      refresh();
    });
  }

  await listen("ghost-follower-update", () => refresh());
  await refresh();
  fallbackInterval = setInterval(refresh, 3000);
}

init();
