import { describe, it, expect, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { LogTab } from "./LogTab";

vi.mock("@/lib/taurpc", () => ({
  getTaurpc: () => ({
    get_diagnostic_logs: vi.fn().mockResolvedValue([]),
    clear_diagnostic_logs: vi.fn().mockResolvedValue(undefined),
  }),
}));

describe("LogTab", () => {
  it("renders Expansion Diagnostics heading", () => {
    render(<LogTab />);
    expect(screen.getByText("Expansion Diagnostics")).toBeInTheDocument();
  });

  it("shows empty state when no entries", async () => {
    render(<LogTab />);
    await waitFor(() => {
      expect(screen.getByText(/No diagnostic entries yet/)).toBeInTheDocument();
    });
  });

  it("renders Clear and Refresh buttons", () => {
    render(<LogTab />);
    expect(screen.getByRole("button", { name: /Clear/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Refresh/i })).toBeInTheDocument();
  });
});
