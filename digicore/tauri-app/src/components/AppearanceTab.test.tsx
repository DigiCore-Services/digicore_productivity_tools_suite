import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { AppearanceTab } from "./AppearanceTab";

const mockTaurpc = {
  get_appearance_transparency_rules: vi.fn(),
  get_running_process_names: vi.fn(),
  save_appearance_transparency_rule: vi.fn(),
  delete_appearance_transparency_rule: vi.fn(),
  apply_appearance_transparency_now: vi.fn(),
  restore_appearance_defaults: vi.fn(),
};
const mockConfirm = vi.fn();

vi.mock("@/lib/taurpc", () => ({
  getTaurpc: () => mockTaurpc,
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: (...args: unknown[]) => mockConfirm(...args),
}));

describe("AppearanceTab", () => {
  beforeEach(() => {
    mockTaurpc.get_appearance_transparency_rules.mockReset();
    mockTaurpc.get_running_process_names.mockReset();
    mockTaurpc.save_appearance_transparency_rule.mockReset();
    mockTaurpc.delete_appearance_transparency_rule.mockReset();
    mockTaurpc.apply_appearance_transparency_now.mockReset();
    mockTaurpc.restore_appearance_defaults.mockReset();
    mockTaurpc.get_appearance_transparency_rules.mockResolvedValue([]);
    mockTaurpc.get_running_process_names.mockResolvedValue([]);
    mockTaurpc.save_appearance_transparency_rule.mockResolvedValue(undefined);
    mockTaurpc.delete_appearance_transparency_rule.mockResolvedValue(undefined);
    mockTaurpc.apply_appearance_transparency_now.mockResolvedValue(0);
    mockTaurpc.restore_appearance_defaults.mockResolvedValue(0);
    mockConfirm.mockReset();
    mockConfirm.mockResolvedValue(true);
  });

  it("renders heading and base actions", async () => {
    render(<AppearanceTab />);
    expect(screen.getByText("Appearance")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Add/Update Rule" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Delete Rule" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Refresh Rules" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Restore All Defaults" })).toBeInTheDocument();
    await waitFor(() =>
      expect(mockTaurpc.get_appearance_transparency_rules).toHaveBeenCalledTimes(1)
    );
    await waitFor(() =>
      expect(mockTaurpc.get_running_process_names).toHaveBeenCalledTimes(1)
    );
  });

  it("loads running process suggestions into autocomplete", async () => {
    mockTaurpc.get_running_process_names.mockResolvedValue([
      "cursor.exe",
      "notepad.exe",
    ]);
    render(<AppearanceTab />);
    await waitFor(() =>
      expect(
        document.querySelector(
          '#appearance-running-processes option[value="cursor.exe"]'
        )
      ).toBeTruthy()
    );
    expect(
      document.querySelector(
        '#appearance-running-processes option[value="notepad.exe"]'
      )
    ).toBeTruthy();
  });

  it("refreshes running process suggestions on demand", async () => {
    mockTaurpc.get_running_process_names
      .mockResolvedValueOnce(["cursor.exe"])
      .mockResolvedValueOnce(["cursor.exe", "code.exe"]);
    render(<AppearanceTab />);
    await userEvent.click(await screen.findByRole("button", { name: "Refresh Apps" }));
    await waitFor(() =>
      expect(
        document.querySelector('#appearance-running-processes option[value="code.exe"]')
      ).toBeTruthy()
    );
  });

  it("shows validation on save without app process", async () => {
    render(<AppearanceTab />);
    await userEvent.click(screen.getByRole("button", { name: "Add/Update Rule" }));
    expect(
      screen.getByText("Validation: please specify an app process name.")
    ).toBeInTheDocument();
    expect(mockTaurpc.save_appearance_transparency_rule).not.toHaveBeenCalled();
  });

  it("saves rule and shows success status", async () => {
    render(<AppearanceTab />);
    const input = screen.getByRole("combobox");
    await userEvent.type(input, "cursor.exe");
    await userEvent.click(screen.getByRole("button", { name: "Add/Update Rule" }));
    await waitFor(() =>
      expect(mockTaurpc.save_appearance_transparency_rule).toHaveBeenCalledWith(
        "cursor.exe",
        255,
        true
      )
    );
    expect(screen.getByText("Transparency rule saved for cursor.exe.")).toBeInTheDocument();
  });

  it("loads and sorts rules alphabetically", async () => {
    mockTaurpc.get_appearance_transparency_rules.mockResolvedValue([
      { app_process: "zeta.exe", opacity: 140, enabled: true },
      { app_process: "alpha.exe", opacity: 200, enabled: true },
    ]);
    render(<AppearanceTab />);
    await waitFor(() => expect(screen.getByText("alpha.exe")).toBeInTheDocument());
    const rows = screen.getAllByRole("row");
    expect(rows[1]).toHaveTextContent("alpha.exe");
    expect(rows[2]).toHaveTextContent("zeta.exe");
  });

  it("applies deterministic priority ordering with enabled rules first", async () => {
    mockTaurpc.get_appearance_transparency_rules.mockResolvedValue([
      { app_process: "zeta.exe", opacity: 140, enabled: true },
      { app_process: "alpha.exe", opacity: 200, enabled: true },
      { app_process: "cursor.exe", opacity: 180, enabled: false },
    ]);
    render(<AppearanceTab />);
    await waitFor(() => expect(screen.getByText("alpha.exe")).toBeInTheDocument());
    const rows = screen.getAllByRole("row");
    expect(rows[1]).toHaveTextContent("alpha.exe");
    expect(rows[2]).toHaveTextContent("zeta.exe");
    expect(rows[3]).toHaveTextContent("cursor.exe");
  });

  it("shows conflict warning for duplicate process keys", async () => {
    mockTaurpc.get_appearance_transparency_rules.mockResolvedValue([
      { app_process: "cursor", opacity: 180, enabled: true },
      { app_process: "cursor.exe", opacity: 200, enabled: false },
    ]);
    render(<AppearanceTab />);
    expect(
      await screen.findByText(
        "Conflict detected for: cursor.exe. Priority order determines which duplicate rule is applied first."
      )
    ).toBeInTheDocument();
  });

  it("double click rule fills form fields", async () => {
    mockTaurpc.get_appearance_transparency_rules.mockResolvedValue([
      { app_process: "cursor.exe", opacity: 180, enabled: true },
    ]);
    render(<AppearanceTab />);
    const row = await screen.findByText("cursor.exe");
    await userEvent.dblClick(row);
    expect(screen.getByRole("combobox")).toHaveValue("cursor.exe");
    expect(screen.getByRole("slider")).toHaveValue("180");
  });

  it("applies transparency preview when slider changes and app is set", async () => {
    render(<AppearanceTab />);
    await userEvent.type(screen.getByRole("combobox"), "cursor.exe");
    mockTaurpc.apply_appearance_transparency_now.mockResolvedValue(2);
    const slider = screen.getByRole("slider");
    fireEvent.change(slider, { target: { value: "240" } });
    await waitFor(() =>
      expect(mockTaurpc.apply_appearance_transparency_now).toHaveBeenCalledWith("cursor.exe", 240)
    );
    expect(
      screen.getByText("Applied preview transparency to 2 windows for cursor.exe.")
    ).toBeInTheDocument();
  });

  it("shows validation on delete with no selection or app", async () => {
    render(<AppearanceTab />);
    await userEvent.click(screen.getByRole("button", { name: "Delete Rule" }));
    expect(screen.getByText("Validation: select a rule to delete.")).toBeInTheDocument();
    expect(mockTaurpc.delete_appearance_transparency_rule).not.toHaveBeenCalled();
  });

  it("toggles enabled state for a rule", async () => {
    mockTaurpc.get_appearance_transparency_rules
      .mockResolvedValueOnce([{ app_process: "cursor.exe", opacity: 180, enabled: true }])
      .mockResolvedValueOnce([{ app_process: "cursor.exe", opacity: 180, enabled: false }]);
    render(<AppearanceTab />);
    const checkbox = await screen.findByRole("checkbox");
    await userEvent.click(checkbox);
    await waitFor(() =>
      expect(mockTaurpc.save_appearance_transparency_rule).toHaveBeenCalledWith(
        "cursor.exe",
        180,
        false
      )
    );
    expect(screen.getByText("Rule disabled for cursor.exe.")).toBeInTheDocument();
  });

  it("applies selected rule now and shows affected window count", async () => {
    mockTaurpc.get_appearance_transparency_rules.mockResolvedValue([
      { app_process: "cursor.exe", opacity: 180, enabled: true },
    ]);
    mockTaurpc.apply_appearance_transparency_now.mockResolvedValue(3);
    render(<AppearanceTab />);
    await userEvent.click(await screen.findByRole("button", { name: "Apply now" }));
    await waitFor(() =>
      expect(mockTaurpc.apply_appearance_transparency_now).toHaveBeenCalledWith(
        "cursor.exe",
        180
      )
    );
    expect(
      screen.getByText("Applied cursor.exe transparency to 3 windows.")
    ).toBeInTheDocument();
  });

  it("restores all defaults after confirmation with count feedback", async () => {
    mockTaurpc.get_appearance_transparency_rules
      .mockResolvedValueOnce([{ app_process: "cursor.exe", opacity: 180, enabled: true }])
      .mockResolvedValueOnce([]);
    mockTaurpc.restore_appearance_defaults.mockResolvedValue(4);
    render(<AppearanceTab />);
    await userEvent.click(
      await screen.findByRole("button", { name: "Restore All Defaults" })
    );
    await waitFor(() => expect(mockConfirm).toHaveBeenCalled());
    expect(mockTaurpc.restore_appearance_defaults).toHaveBeenCalledTimes(1);
    expect(
      await screen.findByText("Restored defaults: cleared 1 rule and reset 4 windows.")
    ).toBeInTheDocument();
  });

  it("shows cancellation feedback when restore defaults is cancelled", async () => {
    mockConfirm.mockResolvedValue(false);
    mockTaurpc.get_appearance_transparency_rules.mockResolvedValue([
      { app_process: "cursor.exe", opacity: 180, enabled: true },
    ]);
    render(<AppearanceTab />);
    await userEvent.click(
      await screen.findByRole("button", { name: "Restore All Defaults" })
    );
    await waitFor(() => expect(mockConfirm).toHaveBeenCalled());
    expect(mockTaurpc.restore_appearance_defaults).not.toHaveBeenCalled();
    expect(await screen.findByText("Restore defaults cancelled.")).toBeInTheDocument();
  });
});
