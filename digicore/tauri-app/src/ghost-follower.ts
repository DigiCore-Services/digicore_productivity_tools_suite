/**
 * Ghost Follower overlay - pill (collapsed) / ribbon (expanded).
 * Pill expands on hover; collapses after delay when idle.
 * Uses TauRPC proxy for type-safe IPC.
 *
 * SMOKE_TEST_CENTER: When true, always show and center for smoke testing.
 * Set to false for production edge-anchored behavior.
 */
const SMOKE_TEST_CENTER = false;

const PILL_WIDTH = 64;
const PILL_HEIGHT = 36;
const RIBBON_WIDTH = 280;
const RIBBON_HEIGHT = 420;

import { getTaurpc } from "@/lib/taurpc";
import { emit, listen } from "@tauri-apps/api/event";
import { resolveTheme, applyThemeToDocument } from "@/lib/theme";
import { LogicalPosition, PhysicalPosition } from "@tauri-apps/api/dpi";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { confirm } from "@tauri-apps/plugin-dialog";

declare const window: Window & {
  __TAURI__?: { plugin?: { positioner?: { moveWindow: (pos: unknown) => Promise<void>; Position?: { TopRight?: unknown; TopLeft?: unknown } } } };
};

const positioner = (window as unknown as { __TAURI__?: { plugin?: { positioner?: { moveWindow: (pos: unknown) => Promise<void>; Position?: { TopRight?: unknown; TopLeft?: unknown } } } } }).__TAURI__?.plugin?.positioner;

let fallbackInterval: ReturnType<typeof setInterval> | null = null;
let hoverKeepAliveInterval: ReturnType<typeof setInterval> | null = null;
let lastSearch = "";
let currentTheme: "dark" | "light" = "light";
let collapsed = true;
let pinnedItems: { trigger: string; content: string; content_preview: string; category: string; snippet_idx: number }[] = [];
let clipEntries: { content: string; process_name?: string }[] = [];
let promotedContentSet = new Set<string>();
let isPointerOverFollower = false;
let isPointerOverContextMenu = false;
let ghostFollowerMode = "EdgeAnchored";
let ghostFollowerExpandTrigger = "Click";

function normalizeContentForMatch(value: string): string {
  return (value || "").replace(/\r\n/g, "\n").trim();
}

function escapeHtml(s: string): string {
  const div = document.createElement("div");
  div.textContent = s || "";
  return div.innerHTML;
}

function showContextMenu(
  x: number,
  y: number,
  items: { text: string; icon?: string; onClick: () => void; danger?: boolean; disabled?: boolean }[]
) {
  const menu = document.getElementById("ctx-menu");
  if (!menu) return;
  menu.innerHTML = "";
  items.forEach(({ text, icon, onClick, danger, disabled }) => {
    const el = document.createElement("div");
    el.className = "ctx-menu-item" + (danger ? " danger" : "") + (disabled ? " disabled" : "");
    const iconSpan = document.createElement("span");
    iconSpan.className = "ctx-menu-item-icon";
    iconSpan.textContent = icon || "";
    const textSpan = document.createElement("span");
    textSpan.className = "ctx-menu-item-text";
    textSpan.textContent = text;
    el.appendChild(iconSpan);
    el.appendChild(textSpan);
    if (!disabled) {
      el.onclick = () => {
        hideContextMenu();
        onClick();
      };
    }
    menu.appendChild(el);
  });
  menu.style.left = x + "px";
  menu.style.top = y + "px";
  menu.style.display = "block";
  isPointerOverContextMenu = true;
  updateHoverKeepAlive();
}

function hideContextMenu() {
  const menu = document.getElementById("ctx-menu");
  if (menu) menu.style.display = "none";
  isPointerOverContextMenu = false;
  updateHoverKeepAlive();
}

document.addEventListener("click", hideContextMenu);

const api = getTaurpc();

/** Capture target window when mouse enters and we don't have focus (user moving from Sublime/Outlook). */
function onPointerEnter() {
  if (!document.hasFocus()) {
    api.ghost_follower_capture_target_window().catch(() => { });
  }
}

if (document.body) {
  document.body.addEventListener("mouseenter", onPointerEnter);
}

function updateHoverKeepAlive() {
  const shouldKeepAlive =
    !collapsed && (isPointerOverFollower || isPointerOverContextMenu);
  if (shouldKeepAlive && !hoverKeepAliveInterval) {
    hoverKeepAliveInterval = setInterval(() => {
      api.ghost_follower_touch().catch(() => { });
    }, 500);
    return;
  }
  if (!shouldKeepAlive && hoverKeepAliveInterval) {
    clearInterval(hoverKeepAliveInterval);
    hoverKeepAliveInterval = null;
  }
}

async function setCollapsed(c: boolean) {
  collapsed = c;
  hideContextMenu();
  updateHoverKeepAlive();
  await api.ghost_follower_set_collapsed(c);
  const pill = document.getElementById("pill-container");
  const pillWrapper = document.getElementById("pill-wrapper");
  const ribbon = document.getElementById("ribbon-container");
  document.body?.classList.toggle("pill-mode", c);
  document.documentElement.classList.toggle("pill-mode", c);
  if (pill && ribbon) {
    pill.style.display = c ? "flex" : "none";
    ribbon.classList.toggle("expanded", !c);
  }
  if (pillWrapper) {
    pillWrapper.classList.toggle("ribbon-active", !c);
    if (c) {
      // Ensure collapsed mode always renders as pure pill (no header ribbon remnants).
      pillWrapper.classList.remove("title-bar-visible");
    }
  }
  try {
    if (c) {
      await api.ghost_follower_set_size(PILL_WIDTH, PILL_HEIGHT);
    } else {
      await api.ghost_follower_set_size(RIBBON_WIDTH, RIBBON_HEIGHT);
    }
  } catch {
    /* ignore */
  }
}

async function expand() {
  if (collapsed) {
    api.ghost_follower_touch().catch(() => { });
    await setCollapsed(false);
  }
}

async function collapse() {
  if (!collapsed) {
    await setCollapsed(true);
  }
}

async function handleClose() {
  await api.ghost_follower_hide();
}

function render(state: { enabled?: boolean; pinned?: typeof pinnedItems; clip_history_max_depth?: number; should_collapse?: boolean; collapse_delay_secs?: number; opacity?: number; mode?: string; expand_trigger?: string } | null) {
  const list = document.getElementById("pinned-list");
  const clipList = document.getElementById("clip-list");
  const clipHeader = document.getElementById("clip-header");
  if (!list || !clipList) return;
  const effectiveEnabled = SMOKE_TEST_CENTER || (state?.enabled ?? false);
  const opacity = state?.opacity ?? 1;
  if (document.body) {
    document.body.style.opacity = String(opacity);
    if (opacity < 1) {
      const a = Math.max(0.15, opacity);
      document.documentElement.classList.add("glass-mode");
      document.body.style.setProperty("--body-bg", "transparent");
      if (currentTheme === "dark") {
        document.body.style.setProperty("--card-bg", `rgba(31, 41, 55, ${a})`);
        document.body.style.setProperty("--list-bg", `rgba(31, 41, 55, ${a * 0.9})`);
        document.body.style.setProperty("--input-bg", `rgba(55, 65, 81, ${a * 0.95})`);
        document.body.style.setProperty("--hover-bg", `rgba(75, 85, 99, ${a})`);
        document.body.style.setProperty("--ctx-bg", `rgba(55, 65, 81, ${a})`);
      } else {
        document.body.style.setProperty("--card-bg", `rgba(255, 255, 255, ${a})`);
        document.body.style.setProperty("--list-bg", `rgba(255, 255, 255, ${a * 0.9})`);
        document.body.style.setProperty("--input-bg", `rgba(255, 255, 255, ${a * 0.95})`);
        document.body.style.setProperty("--hover-bg", `rgba(243, 244, 246, ${a})`);
        document.body.style.setProperty("--ctx-bg", `rgba(255, 255, 255, ${a})`);
      }
    } else {
      document.documentElement.classList.remove("glass-mode");
      document.body.style.removeProperty("--body-bg");
      document.body.style.removeProperty("--card-bg");
      document.body.style.removeProperty("--list-bg");
      document.body.style.removeProperty("--input-bg");
      document.body.style.removeProperty("--hover-bg");
      document.body.style.removeProperty("--ctx-bg");
    }
  }
  if (!effectiveEnabled) {
    list.innerHTML = "";
    clipList.innerHTML = "";
    if (clipHeader) clipHeader.textContent = "Clipboard History";
    return;
  }
  const clipMax = state?.clip_history_max_depth ?? 20;
  if (clipHeader) clipHeader.textContent = `Clipboard History: ${clipEntries.length} of ${clipMax} entries`;
  pinnedItems = (state?.pinned ?? []) as typeof pinnedItems;
  let html = "";
  pinnedItems.forEach((p, i) => {
    html += `<div class="pinned-item" data-idx="${i}">
      <span class="trigger">${escapeHtml(p.trigger)}</span>
      <div class="preview">${escapeHtml(p.content_preview)}</div>
    </div>`;
  });
  list.innerHTML = html || '<div class="empty-msg">No pinned snippets</div>';
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
        {
          icon: "👁",
          text: "View Full Snippet Content",
          onClick: async () => {
            await api.bring_main_window_to_foreground();
            emit("ghost-follower-view-full", {
              source: "pinned",
              content: p.content,
              category: p.category,
              snippetIdx: p.snippet_idx,
            });
          },
        },
        {
          icon: "📌",
          text: "Unpin Snippet",
          onClick: async () => {
            const confirmed = await confirm(
              "Are you sure you want to unpin this snippet?",
              { title: "Confirm Unpin", kind: "warning" }
            );
            if (!confirmed) {
              emit("ghost-follower-status", { text: "Unpin cancelled." });
              return;
            }
            await api.ghost_follower_toggle_pin(p.category, p.snippet_idx);
          },
        },
        {
          icon: "✎",
          text: "Edit Snippet",
          onClick: () => api.ghost_follower_request_edit(p.category, p.snippet_idx),
        },
        { icon: "⧉", text: "Copy Full Content to Clipboard", onClick: () => api.copy_to_clipboard(p.content) },
        {
          icon: "🗑",
          text: "Delete Snippet",
          onClick: async () => {
            await api.bring_main_window_to_foreground();
            emit("ghost-follower-delete-snippet", { category: p.category, snippetIdx: p.snippet_idx });
          },
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
  clipList.innerHTML = clipHtml || '<div class="empty-msg">No clipboard history</div>';
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
      const promoted = promotedContentSet.has(normalizeContentForMatch(c.content || ""));
      // TODO(ghost-menu-order): Keep right-click order in sync with Clipboard tab and View Full actions.
      showContextMenu(ev.clientX, ev.clientY, [
        {
          icon: "👁",
          text: "View Full Content",
          onClick: async () => {
            await api.bring_main_window_to_foreground();
            emit("ghost-follower-view-full", {
              source: "clipboard",
              content: c.content,
              index: idx,
              trigger,
            });
          },
        },
        promoted
          ? { icon: "✓", text: "Promoted", disabled: true, onClick: () => { } }
          : { icon: "⬆", text: "Promote to Snippet", onClick: () => api.ghost_follower_request_promote(c.content, trigger) },
        { icon: "⧉", text: "Copy to Clipboard", onClick: () => api.copy_to_clipboard(c.content) },
        {
          icon: "🗑",
          text: "Delete",
          onClick: async () => {
            await api.bring_main_window_to_foreground();
            emit("ghost-follower-delete-clip", { index: idx });
          },
          danger: true,
        },
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
    const [state, clips, appState] = await Promise.all([
      api.get_ghost_follower_state(search || null).catch(() => null),
      api.get_clipboard_entries().catch(() => []),
      api.get_app_state().catch(() => null),
    ]);
    clipEntries = clips || [];
    const set = new Set<string>();
    const library = appState?.library ?? {};
    Object.values(library).forEach((snippets) => {
      if (!Array.isArray(snippets)) return;
      snippets.forEach((snippet) => {
        const normalized = normalizeContentForMatch(snippet.content || "");
        if (normalized) set.add(normalized);
      });
    });
    promotedContentSet = set;
    ghostFollowerMode = state?.mode || "EdgeAnchored";
    ghostFollowerExpandTrigger = state?.expand_trigger || "Click";
    render(state);

    if (
      state?.should_collapse &&
      state.collapse_delay_secs &&
      state.collapse_delay_secs > 0 &&
      !collapsed &&
      !isPointerOverFollower &&
      !isPointerOverContextMenu
    ) {
      await collapse();
    }

    const effectiveEnabled = SMOKE_TEST_CENTER || (state?.enabled ?? false);
    const w = getCurrentWebviewWindow();
    if (w) {
      if (SMOKE_TEST_CENTER) {
        await w.center();
        await w.show();
      } else {
        let positioned = false;
        const isSaved = (state as { saved_position?: boolean }).saved_position;
        if (positioner && state?.enabled && (state as { monitor_primary?: boolean }).monitor_primary && ghostFollowerMode === "EdgeAnchored" && !isSaved) {
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
              await w.setPosition(new PhysicalPosition(x, y));
            } else if (state?.enabled && !isSaved) {
              // Only auto-center if enabled and NO saved position exists (to avoid jumping)
              await w.center();
            }
          } catch {
            if (!isSaved) await w.center();
          }
        }
        if (state?.enabled) {
          await w.show();
        } else if (state && !state.enabled) {
          await w.hide();
        }
      }
    }
  } catch (e) {
    console.error("[GhostFollower] refresh error:", e);
  }
}

function applyTheme(resolved: "dark" | "light") {
  currentTheme = resolved;
  applyThemeToDocument(resolved);
}

async function init() {
  const win = getCurrentWebviewWindow();
  if (win) {
    try {
      await win.setBackgroundColor("rgba(0,0,0,0)");
    } catch {
      /* ignore - transparency may not be supported on all platforms */
    }
  }

  const pref =
    (typeof localStorage !== "undefined" &&
      localStorage.getItem("digicore-theme")) ||
    "light";
  applyTheme(resolveTheme(pref));
  listen<{ theme: "dark" | "light" }>("digicore-theme-changed", (e) => {
    applyTheme(e.payload.theme);
  }).catch(() => { });

  const pill = document.getElementById("pill-container");
  const pillWrapper = document.getElementById("pill-wrapper");
  const ribbon = document.getElementById("ribbon-container");
  const pillClose = document.getElementById("pill-close");
  const pillCloseBar = document.getElementById("pill-close-bar");
  const pillMinimize = document.getElementById("pill-minimize");
  const pillMaximize = document.getElementById("pill-maximize");
  const ribbonClose = document.getElementById("ribbon-close");
  const searchInput = document.getElementById("search-input");

  if (pill) {
    pill.style.display = collapsed ? "flex" : "none";
    pill.addEventListener("mouseenter", () => {
      if (ghostFollowerExpandTrigger === "Hover") {
        api.ghost_follower_touch().catch(() => { });
        expand();
      }
    });
    pill.addEventListener("click", () => {
      if (ghostFollowerExpandTrigger === "Click") {
        api.ghost_follower_touch().catch(() => { });
        expand();
      }
    });
  }
  if (pillWrapper) {
    // Defensive reset in case class persisted from prior UI state.
    pillWrapper.classList.remove("title-bar-visible");
    pillWrapper.addEventListener("mouseenter", () => {
      isPointerOverFollower = true;
      updateHoverKeepAlive();
    });
    pillWrapper.addEventListener("mouseleave", () => {
      isPointerOverFollower = false;
      updateHoverKeepAlive();
    });
  }
  if (pillMinimize) {
    pillMinimize.addEventListener("click", async (e) => {
      e.stopPropagation();
      const w = getCurrentWebviewWindow();
      if (w) await w.minimize();
    });
  }
  if (pillMaximize) {
    pillMaximize.addEventListener("click", async (e) => {
      e.stopPropagation();
      const w = getCurrentWebviewWindow();
      if (w) await w.toggleMaximize();
    });
  }
  if (pillCloseBar) {
    pillCloseBar.addEventListener("click", (e) => {
      e.stopPropagation();
      handleClose();
    });
  }
  if (ribbon) {
    ribbon.classList.toggle("expanded", !collapsed);
    ribbon.addEventListener("mouseenter", () => {
      isPointerOverFollower = true;
      api.ghost_follower_touch().catch(() => { });
      updateHoverKeepAlive();
    });
    ribbon.addEventListener("mouseleave", () => {
      isPointerOverFollower = false;
      updateHoverKeepAlive();
    });
  }
  if (pillClose) {
    pillClose.addEventListener("click", (e) => {
      e.stopPropagation();
      handleClose();
    });
  }
  if (ribbonClose) {
    ribbonClose.addEventListener("click", (e) => {
      e.stopPropagation();
      handleClose();
    });
  }

  if (searchInput) {
    searchInput.addEventListener("input", () => {
      lastSearch = "";
      refresh();
    });
    searchInput.addEventListener("focus", () => api.ghost_follower_touch().catch(() => { }));
  }

  document.getElementById("pinned-list")?.addEventListener("scroll", () => api.ghost_follower_touch().catch(() => { }));
  document.getElementById("clip-list")?.addEventListener("scroll", () => api.ghost_follower_touch().catch(() => { }));
  document.getElementById("ctx-menu")?.addEventListener("mouseenter", () => {
    isPointerOverContextMenu = true;
    updateHoverKeepAlive();
  });
  document.getElementById("ctx-menu")?.addEventListener("mouseleave", () => {
    isPointerOverContextMenu = false;
    updateHoverKeepAlive();
  });

  await listen("ghost-follower-update", () => refresh());
  await refresh();

  const w = getCurrentWebviewWindow();
  if (w) {
    w.onMoved(({ payload }) => {
      const x = Math.round(payload.x);
      const y = Math.round(payload.y);
      api.ghost_follower_save_position(x, y).catch(() => { });
    }).catch(() => { /* ignore */ });
  }

  await setCollapsed(true);
  fallbackInterval = setInterval(refresh, 3000);
}

init();
