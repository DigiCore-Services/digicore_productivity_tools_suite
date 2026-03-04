import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { LibraryTab } from "./LibraryTab";

const mockInvoke = vi.fn();
const mockOpen = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: (...args: unknown[]) => mockOpen(...args),
}));

vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: vi.fn().mockResolvedValue(true),
  requestPermission: vi.fn().mockResolvedValue(true),
  sendNotification: vi.fn(),
}));

vi.mock("@/lib/sqliteSync", () => ({
  syncLibraryToSqlite: vi.fn().mockResolvedValue(undefined),
}));

const defaultProps = {
  appState: null,
  onAppStateChange: vi.fn(),
  setStatus: vi.fn(),
  onOpenViewFull: vi.fn(),
  onOpenSnippetEditor: vi.fn(),
  onOpenDeleteConfirm: vi.fn(),
  columnOrder: ["Profile", "Category", "Trigger", "Content Preview"],
  sortColumn: "Trigger",
  sortAsc: true,
  onColumnOrderChange: vi.fn(),
  onSortChange: vi.fn(),
  onColumnDrag: vi.fn(),
};

describe("LibraryTab", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockOpen.mockReset();
    mockInvoke.mockResolvedValue({
      library_path: "",
      library: {},
      categories: [],
      status: "",
    });
  });

  it("renders Text Expansion Library heading", () => {
    render(<LibraryTab {...defaultProps} />);
    expect(screen.getByText("Text Expansion Library")).toBeInTheDocument();
  });

  it("renders Load, Save, and Browse buttons", () => {
    render(<LibraryTab {...defaultProps} />);
    expect(screen.getByRole("button", { name: /Load library/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Save library/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Browse for library file/i })).toBeInTheDocument();
  });

  it("calls open with JSON filter when Browse is clicked", async () => {
    mockOpen.mockResolvedValue(null);
    render(<LibraryTab {...defaultProps} />);
    await userEvent.click(screen.getByRole("button", { name: /Browse for library file/i }));
    await waitFor(() => {
      expect(mockOpen).toHaveBeenCalledWith(
        expect.objectContaining({
          filters: [{ name: "JSON Library", extensions: ["json"] }],
          multiple: false,
          directory: false,
        })
      );
    });
  });

  it("updates library path when Browse returns a path", async () => {
    mockOpen.mockResolvedValue("C:\\path\\to\\library.json");
    render(<LibraryTab {...defaultProps} />);
    await userEvent.click(screen.getByRole("button", { name: /Browse for library file/i }));
    await waitFor(() => {
      const input = screen.getByLabelText("Library file path");
      expect(input).toHaveValue("C:\\path\\to\\library.json");
    });
  });

  it("calls setStatus on Browse error", async () => {
    mockOpen.mockRejectedValue(new Error("Dialog failed"));
    render(<LibraryTab {...defaultProps} />);
    await userEvent.click(screen.getByRole("button", { name: /Browse for library file/i }));
    await waitFor(() => {
      expect(defaultProps.setStatus).toHaveBeenCalledWith(
        "Browse failed: Error: Dialog failed",
        true
      );
    });
  });
});
