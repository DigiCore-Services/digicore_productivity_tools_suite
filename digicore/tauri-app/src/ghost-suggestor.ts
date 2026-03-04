/**
 * Ghost Suggestor overlay entry.
 * Uses TauRPC proxy for type-safe IPC (no invoke).
 */
import { getTaurpc } from "@/lib/taurpc";
import { listen } from "@tauri-apps/api/event";
import { LogicalPosition } from "@tauri-apps/api/dpi";
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
    render(state);
    const w = getCurrentWebviewWindow();
    if (w) {
      if (state?.should_passthrough) {
        try {
          await w.setIgnoreCursorEvents(true);
        } catch {
          /* ignore */
        }
      } else {
        try {
          await w.setIgnoreCursorEvents(false);
        } catch {
          /* ignore */
        }
      }
      if (state?.has_suggestions && state.position) {
        await w.setPosition(new LogicalPosition(state.position[0], state.position[1]));
      }
      if (state?.has_suggestions) {
        await w.show();
      } else {
        await w.hide();
      }
    }
  } catch {
    /* ignore */
  }
}

async function init() {
  const btnAccept = document.getElementById("btn-accept");
  const btnCreate = document.getElementById("btn-create");
  const btnIgnore = document.getElementById("btn-ignore");

  if (btnAccept) {
    btnAccept.addEventListener("click", async () => {
      const r = await api.ghost_suggestor_accept();
      if (r) {
        const w = getCurrentWebviewWindow();
        if (w) await w.hide();
      }
      await refresh();
    });
  }

  if (btnCreate) {
    btnCreate.addEventListener("click", async () => {
      await api.ghost_suggestor_create_snippet();
      const w = getCurrentWebviewWindow();
      if (w) await w.hide();
      await refresh();
    });
  }

  if (btnIgnore) {
    btnIgnore.addEventListener("click", async () => {
      await api.ghost_suggestor_dismiss();
      const w = getCurrentWebviewWindow();
      if (w) await w.hide();
      await refresh();
    });
  }

  await listen("ghost-suggestor-update", () => refresh());
  await refresh();
  fallbackInterval = setInterval(refresh, 3000);
}

init();
