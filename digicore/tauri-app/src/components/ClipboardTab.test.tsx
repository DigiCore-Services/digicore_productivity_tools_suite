import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ClipboardTab } from "./ClipboardTab";

const mockTaurpc = {
  get_clipboard_entries: vi.fn(),
  get_app_state: vi.fn(),
  search_clipboard_entries: vi.fn(),
  copy_to_clipboard: vi.fn(),
  copy_clipboard_image_by_id: vi.fn(),
  save_clipboard_image_by_id: vi.fn(),
  open_clipboard_image_by_id: vi.fn(),
};

vi.mock("@/lib/taurpc", () => ({
  getTaurpc: () => mockTaurpc,
}));

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => `file:///${path.replace(/\\/g, "/")}`,
}));

describe("ClipboardTab search operators", () => {
  beforeEach(() => {
    Object.values(mockTaurpc).forEach((fn) => fn.mockReset());
    mockTaurpc.get_clipboard_entries.mockResolvedValue([
      {
        id: 1,
        content: "hello",
        process_name: "Cursor.exe",
        window_title: "Editor",
        length: 5,
        word_count: 1,
        created_at: "1772745540000",
      },
    ]);
    mockTaurpc.search_clipboard_entries.mockResolvedValue([]);
    mockTaurpc.get_app_state.mockResolvedValue({ clip_history_max_depth: 20 });
    mockTaurpc.copy_to_clipboard.mockResolvedValue(null);
    mockTaurpc.copy_clipboard_image_by_id.mockResolvedValue(null);
    mockTaurpc.save_clipboard_image_by_id.mockResolvedValue(null);
    mockTaurpc.open_clipboard_image_by_id.mockResolvedValue(null);
  });

  it("calls backend search with selected operator", async () => {
    render(
      <ClipboardTab
        appState={null}
        onOpenViewFull={vi.fn()}
        onOpenSnippetEditor={vi.fn()}
        onOpenClipClearConfirm={vi.fn()}
        onOpenClipEntryDeleteConfirm={vi.fn()}
      />
    );
    await waitFor(() =>
      expect(mockTaurpc.get_clipboard_entries).toHaveBeenCalled()
    );
    await userEvent.type(
      screen.getByPlaceholderText("Search content/app/window..."),
      "test passing"
    );
    await userEvent.selectOptions(screen.getByTitle("Search operator"), "and");
    await userEvent.click(screen.getByRole("button", { name: "Refresh" }));
    await waitFor(() =>
      expect(mockTaurpc.search_clipboard_entries).toHaveBeenLastCalledWith(
        "test passing",
        "and",
        500
      )
    );
  });

  it("renders created timestamp in UTC column", async () => {
    render(
      <ClipboardTab
        appState={null}
        onOpenViewFull={vi.fn()}
        onOpenSnippetEditor={vi.fn()}
        onOpenClipClearConfirm={vi.fn()}
        onOpenClipEntryDeleteConfirm={vi.fn()}
      />
    );
    await waitFor(() =>
      expect(mockTaurpc.get_clipboard_entries).toHaveBeenCalled()
    );
    expect(screen.getByText(/UTC$/)).toBeInTheDocument();
  });

  it("uses image-specific copy action for image entries", async () => {
    mockTaurpc.get_clipboard_entries.mockResolvedValueOnce([
      {
        id: 42,
        content: "[Image] 320x200 image/png",
        process_name: "Cursor.exe",
        window_title: "Editor",
        length: 0,
        word_count: 0,
        created_at: "1772745540000",
        entry_type: "image",
        mime_type: "image/png",
        image_path: "C:/tmp/full.png",
        thumb_path: "C:/tmp/thumb.png",
        image_width: 320,
        image_height: 200,
        image_bytes: 1024,
      },
    ]);
    render(
      <ClipboardTab
        appState={null}
        onOpenViewFull={vi.fn()}
        onOpenSnippetEditor={vi.fn()}
        onOpenClipClearConfirm={vi.fn()}
        onOpenClipEntryDeleteConfirm={vi.fn()}
      />
    );
    await waitFor(() =>
      expect(mockTaurpc.get_clipboard_entries).toHaveBeenCalled()
    );
    await userEvent.click(screen.getByRole("button", { name: "Copy Image" }));
    await waitFor(() =>
      expect(mockTaurpc.copy_clipboard_image_by_id).toHaveBeenCalledWith(42)
    );
    expect(screen.queryByRole("button", { name: "Promote" })).not.toBeInTheDocument();
  });

  it("renders thumbnail and opens image when clicked", async () => {
    mockTaurpc.get_clipboard_entries.mockResolvedValueOnce([
      {
        id: 99,
        content: "[Image] 400x200 image/png",
        process_name: "Cursor.exe",
        window_title: "Editor",
        length: 0,
        word_count: 0,
        created_at: "1772745540000",
        entry_type: "image",
        mime_type: "image/png",
        image_path: "C:/tmp/full.png",
        thumb_path: "C:/tmp/thumb.png",
        image_width: 400,
        image_height: 200,
        image_bytes: 4096,
      },
    ]);
    render(
      <ClipboardTab
        appState={null}
        onOpenViewFull={vi.fn()}
        onOpenSnippetEditor={vi.fn()}
        onOpenClipClearConfirm={vi.fn()}
        onOpenClipEntryDeleteConfirm={vi.fn()}
      />
    );
    await waitFor(() =>
      expect(mockTaurpc.get_clipboard_entries).toHaveBeenCalled()
    );
    const thumbnail = screen.getByRole("img", {
      name: "Clipboard thumbnail 99",
    });
    await userEvent.click(thumbnail);
    await waitFor(() =>
      expect(mockTaurpc.open_clipboard_image_by_id).toHaveBeenCalledWith(99)
    );
  });

  it("shows recovered thumbnail status after refresh", async () => {
    mockTaurpc.get_clipboard_entries
      .mockResolvedValueOnce([
        {
          id: 7,
          content: "[Image] 300x120 image/png",
          process_name: "Cursor.exe",
          window_title: "Editor",
          length: 0,
          word_count: 0,
          created_at: "1772745540000",
          entry_type: "image",
          mime_type: "image/png",
          image_path: "C:/tmp/full.png",
          thumb_path: null,
          image_width: 300,
          image_height: 120,
          image_bytes: 1024,
        },
      ])
      .mockResolvedValueOnce([
        {
          id: 7,
          content: "[Image] 300x120 image/png",
          process_name: "Cursor.exe",
          window_title: "Editor",
          length: 0,
          word_count: 0,
          created_at: "1772745540000",
          entry_type: "image",
          mime_type: "image/png",
          image_path: "C:/tmp/full.png",
          thumb_path: "C:/tmp/thumb.png",
          image_width: 300,
          image_height: 120,
          image_bytes: 1024,
        },
      ]);
    render(
      <ClipboardTab
        appState={null}
        onOpenViewFull={vi.fn()}
        onOpenSnippetEditor={vi.fn()}
        onOpenClipClearConfirm={vi.fn()}
        onOpenClipEntryDeleteConfirm={vi.fn()}
      />
    );
    await waitFor(() =>
      expect(mockTaurpc.get_clipboard_entries).toHaveBeenCalledTimes(1)
    );
    await userEvent.click(screen.getByRole("button", { name: "Refresh" }));
    await waitFor(() =>
      expect(screen.getByText(/Recovered 1 thumbnail during refresh/i)).toBeInTheDocument()
    );
  });
});

