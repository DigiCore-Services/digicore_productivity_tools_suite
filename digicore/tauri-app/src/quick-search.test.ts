import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent, screen, waitFor } from "@testing-library/react";

const mockApi = {
  get_app_state: vi.fn(),
  ghost_follower_insert: vi.fn(),
  bring_main_window_to_foreground: vi.fn(),
  ghost_follower_toggle_pin: vi.fn(),
  copy_to_clipboard: vi.fn(),
  delete_snippet: vi.fn(),
  ghost_follower_capture_target_window: vi.fn(),
};

const mockEmit = vi.fn();
const mockHide = vi.fn();
const mockShow = vi.fn();
const listeners = new Map<string, ((event: { payload?: unknown }) => void)[]>();

vi.mock("@/lib/taurpc", () => ({
  getTaurpc: () => mockApi,
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: (...args: unknown[]) => Promise.resolve(mockEmit(...args)),
  listen: (name: string, cb: (event: { payload?: unknown }) => void) => {
    const list = listeners.get(name) ?? [];
    list.push(cb);
    listeners.set(name, list);
    return Promise.resolve(() => {});
  },
}));

vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({
    hide: (...args: unknown[]) => Promise.resolve(mockHide(...args)),
    show: (...args: unknown[]) => Promise.resolve(mockShow(...args)),
  }),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: vi.fn(async () => true),
}));

vi.mock("@/lib/theme", () => ({
  applyThemeToDocument: vi.fn(),
  resolveTheme: (x: string) => x,
}));

function setQuickSearchDom() {
  document.body.innerHTML = `
    <div class="frame">
      <div class="titlebar"><button id="btn-close" type="button">Close</button></div>
      <input id="search" type="text" />
      <div id="list"></div>
    </div>
    <div id="ctx-menu"></div>
  `;
}

async function loadModule() {
  vi.resetModules();
  await import("./quick-search");
}

describe("quick-search event behavior", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
    listeners.clear();
    mockEmit.mockReset();
    mockHide.mockReset();
    mockShow.mockReset();
    mockApi.get_app_state.mockReset();
    mockApi.ghost_follower_insert.mockReset();
    mockApi.bring_main_window_to_foreground.mockReset();
    mockApi.ghost_follower_toggle_pin.mockReset();
    mockApi.copy_to_clipboard.mockReset();
    mockApi.delete_snippet.mockReset();
    mockApi.ghost_follower_capture_target_window.mockReset();

    mockApi.get_app_state.mockResolvedValue({
      categories: ["General"],
      library: {
        General: [
          {
            trigger: "/cota",
            content: "pay careful attention to details",
            pinned: "false",
          },
        ],
      },
    });
    mockApi.bring_main_window_to_foreground.mockResolvedValue(undefined);
    mockApi.ghost_follower_toggle_pin.mockResolvedValue(undefined);
    mockApi.delete_snippet.mockResolvedValue(undefined);
    mockApi.copy_to_clipboard.mockResolvedValue(undefined);
    mockApi.ghost_follower_insert.mockResolvedValue(undefined);
    mockApi.ghost_follower_capture_target_window.mockResolvedValue(undefined);
  });

  it("emits quick-search-library-refresh after pin action", async () => {
    setQuickSearchDom();
    await loadModule();

    const row = await screen.findByText("/cota");
    fireEvent.contextMenu(row);
    fireEvent.click(screen.getByText("Pin Snippet"));

    await waitFor(() =>
      expect(mockApi.ghost_follower_toggle_pin).toHaveBeenCalledWith("General", 0)
    );
    await waitFor(() =>
      expect(mockEmit).toHaveBeenCalledWith("quick-search-library-refresh", {})
    );
  });

  it("hides quick-search before view/edit emits", async () => {
    const order: string[] = [];
    mockHide.mockImplementation(() => {
      order.push("hide");
      return undefined;
    });
    mockEmit.mockImplementation((name: string) => {
      order.push(`emit:${name}`);
      return undefined;
    });

    setQuickSearchDom();
    await loadModule();

    const row = await screen.findByText("/cota");

    fireEvent.contextMenu(row);
    fireEvent.click(screen.getByText("View Full Snippet Content"));
    await waitFor(() =>
      expect(mockEmit).toHaveBeenCalledWith(
        "quick-search-view-full",
        expect.objectContaining({ category: "General", snippetIdx: 0 })
      )
    );
    expect(order.indexOf("hide")).toBeLessThan(
      order.indexOf("emit:quick-search-view-full")
    );

    fireEvent.contextMenu(row);
    fireEvent.click(screen.getByText("Edit Snippet"));
    await waitFor(() =>
      expect(mockEmit).toHaveBeenCalledWith("quick-search-edit-snippet", {
        category: "General",
        snippetIdx: 0,
      })
    );
    const secondHideIdx = order.lastIndexOf("hide");
    const editEmitIdx = order.lastIndexOf("emit:quick-search-edit-snippet");
    expect(secondHideIdx).toBeLessThan(editEmitIdx);
  });

  it("does not auto-show quick-search on module init", async () => {
    setQuickSearchDom();
    await loadModule();

    await waitFor(() => expect(mockApi.get_app_state).toHaveBeenCalled());
    expect(mockShow).not.toHaveBeenCalled();
  });

  it("double-click insert hides then inserts", async () => {
    const order: string[] = [];
    mockHide.mockImplementation(() => {
      order.push("hide");
      return undefined;
    });
    mockApi.ghost_follower_insert.mockImplementation(() => {
      order.push("insert");
      return undefined;
    });

    setQuickSearchDom();
    await loadModule();

    const row = await screen.findByText("/cota");
    fireEvent.doubleClick(row);
    await new Promise((resolve) => setTimeout(resolve, 80));

    await waitFor(() => expect(mockApi.ghost_follower_insert).toHaveBeenCalled());
    expect(order).toEqual(["hide", "insert"]);
  });
});

