/**
 * Ghost Suggestor overlay entry.
 * Uses TauRPC proxy for type-safe IPC (no invoke).
 */
import { getTaurpc } from "@/lib/taurpc";
import { listen } from "@tauri-apps/api/event";
import { resolveTheme, applyThemeToDocument } from "@/lib/theme";
import { PhysicalPosition } from "@tauri-apps/api/dpi";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

let fallbackInterval: ReturnType<typeof setInterval> | null = null;

function escapeHtml(s: string): string {
  const div = document.createElement("div");
  div.textContent = s || "";
  return div.innerHTML;
}

const api = getTaurpc();

function render(state: {
  has_suggestions?: boolean;
  suggestions?: { trigger: string; content_preview: string }[];
  selected_index?: number;
} | null) {
  const list = document.getElementById("suggestions-list");
  if (!list) return;
  if (!state || !state.has_suggestions) {
    list.innerHTML = "";
    return;
  }
  let html = "";
  (state.suggestions || []).forEach((s, i) => {
    const cls = i === (state.selected_index ?? 0) ? "suggestion selected" : "suggestion";
    html += `<div class="${cls}" data-idx="${i}">[${escapeHtml(s.trigger)}] -> ${escapeHtml(s.content_preview)}</div>`;
  });
  list.innerHTML = html;
  list.querySelectorAll(".suggestion").forEach((el) => {
    el.addEventListener("click", () =>
      api.ghost_suggestor_cycle_forward().then(() => refresh())
    );
  });
}

async function refresh() {
  try {
    const state = await api.get_ghost_suggestor_state();
    const w = getCurrentWebviewWindow();
    console.log(
      "[GhostSuggestor] refresh: has_suggestions=",
      state?.has_suggestions,
      "suggestions=",
      state?.suggestions?.length ?? 0,
      "position=",
      state?.position,
      "window=",
      w ? "exists" : "null"
    );
    render(state);
    if (w) {
      if (state?.has_suggestions && state.position) {
        console.log("[GhostSuggestor] refresh: setting position", state.position);
        await w.setPosition(new PhysicalPosition(state.position[0], state.position[1]));
      }
      if (state?.has_suggestions) {
        console.log("[GhostSuggestor] refresh: calling w.show()");
        await w.show();
        try {
          await w.setFocus();
        } catch (e) {
          console.warn("[GhostSuggestor] refresh: setFocus failed", e);
        }
      } else {
        await w.hide();
      }
    } else {
      console.warn("[GhostSuggestor] refresh: getCurrentWebviewWindow returned null");
    }
  } catch (e) {
    console.error("[GhostSuggestor] refresh error:", e);
  }
}

async function init() {
  const pref =
    (typeof localStorage !== "undefined" &&
      localStorage.getItem("digicore-theme")) ||
    "light";
  applyThemeToDocument(resolveTheme(pref));
  listen<{ theme: "dark" | "light" }>("digicore-theme-changed", (e) => {
    applyThemeToDocument(e.payload.theme);
  }).catch(() => {});

  const btnSnooze = document.getElementById("btn-snooze");
  const btnPromote = document.getElementById("btn-promote");
  const btnIgnore = document.getElementById("btn-ignore");

  if (!btnSnooze || !btnPromote || !btnIgnore) {
    console.error("[GhostSuggestor] Button elements not found");
    return;
  }

  btnSnooze.addEventListener("click", async (e) => {
    e.preventDefault();
    e.stopPropagation();
    try {
      await api.ghost_suggestor_snooze();
      const w = getCurrentWebviewWindow();
      if (w) await w.hide();
      await refresh();
    } catch (err) {
      console.error("[GhostSuggestor] Snooze error:", err);
    }
  });

  btnPromote.addEventListener("click", async (e) => {
    e.preventDefault();
    e.stopPropagation();
    try {
      await api.ghost_suggestor_create_snippet();
      const w = getCurrentWebviewWindow();
      if (w) await w.hide();
      await refresh();
    } catch (err) {
      console.error("[GhostSuggestor] Promote error:", err);
    }
  });

  btnIgnore.addEventListener("click", async (e) => {
    e.preventDefault();
    e.stopPropagation();
    try {
      const state = await api.get_ghost_suggestor_state();
      const idx = state?.selected_index ?? 0;
      const phrase =
        state?.suggestions?.[idx]?.trigger ?? state?.suggestions?.[0]?.trigger ?? "";
      await api.ghost_suggestor_ignore(phrase);
      const w = getCurrentWebviewWindow();
      if (w) await w.hide();
      await refresh();
    } catch (err) {
      console.error("[GhostSuggestor] Ignore error:", err);
    }
  });

  await listen("ghost-suggestor-update", () => {
    console.log("[GhostSuggestor] received ghost-suggestor-update event");
    refresh();
  });
  console.log("[GhostSuggestor] init: listener registered, doing initial refresh");
  await refresh();
  fallbackInterval = setInterval(refresh, 3000);
  console.log("[GhostSuggestor] init: complete, fallback poll every 3s");
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", () => init());
} else {
  init();
}
