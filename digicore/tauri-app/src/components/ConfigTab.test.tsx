import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ConfigTab } from "./ConfigTab";
import type { AppState } from "@/types";

const mockTaurpc = {
  update_config: vi.fn(),
  save_settings: vi.fn(),
  get_app_state: vi.fn(),
  export_settings_bundle_to_file: vi.fn(),
  preview_settings_bundle_from_file: vi.fn(),
  import_settings_bundle_from_file: vi.fn(),
  ghost_follower_set_opacity: vi.fn(),
  get_copy_to_clipboard_config: vi.fn(),
  save_copy_to_clipboard_config: vi.fn(),
};

const mockOpen = vi.fn();
const mockSave = vi.fn();
const mockEmit = vi.fn();

vi.mock("@/lib/taurpc", () => ({
  getTaurpc: () => mockTaurpc,
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: (...args: unknown[]) => mockOpen(...args),
  save: (...args: unknown[]) => mockSave(...args),
}));

vi.mock("@tauri-apps/plugin-updater", () => ({
  check: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-process", () => ({
  relaunch: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: (...args: unknown[]) => Promise.resolve(mockEmit(...args)),
}));

const defaultState = {
  library_path: "",
  library: {},
  categories: [],
  selected_category: 0,
  status: "",
  sync_url: "",
  sync_status: "",
  expansion_paused: false,
  template_date_format: "%Y-%m-%d",
  template_time_format: "%H:%M",
  discovery_enabled: false,
  discovery_threshold: 3,
  discovery_lookback: 30,
  discovery_min_len: 5,
  discovery_max_len: 30,
  discovery_excluded_apps: "",
  discovery_excluded_window_titles: "",
  ghost_suggestor_enabled: false,
  ghost_suggestor_debounce_ms: 80,
  ghost_suggestor_display_secs: 10,
  ghost_suggestor_snooze_duration_mins: 5,
  ghost_suggestor_offset_x: 0,
  ghost_suggestor_offset_y: 0,
  ghost_follower_enabled: false,
  ghost_follower_edge_right: true,
  ghost_follower_monitor_anchor: 0,
  ghost_follower_search: "",
  ghost_follower_hover_preview: false,
  ghost_follower_collapse_delay_secs: 5,
  ghost_follower_opacity: 100,
  clip_history_max_depth: 20,
  script_library_run_disabled: false,
  script_library_run_allowlist: "",
} as unknown as AppState;

describe("ConfigTab import/export settings", () => {
  beforeEach(() => {
    mockOpen.mockReset();
    mockSave.mockReset();
    mockEmit.mockReset();
    mockEmit.mockResolvedValue(undefined);
    mockTaurpc.update_config.mockReset();
    mockTaurpc.save_settings.mockReset();
    mockTaurpc.get_app_state.mockReset();
    mockTaurpc.export_settings_bundle_to_file.mockReset();
    mockTaurpc.preview_settings_bundle_from_file.mockReset();
    mockTaurpc.import_settings_bundle_from_file.mockReset();
    mockTaurpc.ghost_follower_set_opacity.mockReset();
    mockTaurpc.get_copy_to_clipboard_config.mockReset();
    mockTaurpc.save_copy_to_clipboard_config.mockReset();
    mockTaurpc.get_app_state.mockResolvedValue(defaultState);
    mockTaurpc.export_settings_bundle_to_file.mockResolvedValue(9);
    mockTaurpc.preview_settings_bundle_from_file.mockResolvedValue({
      path: "C:\\temp\\settings.json",
      schema_version: "1.0.0",
      available_groups: ["ghost_follower", "appearance"],
      warnings: [],
      valid: true,
    });
    mockTaurpc.import_settings_bundle_from_file.mockResolvedValue({
      applied_groups: ["ghost_follower", "appearance"],
      skipped_groups: [],
      warnings: [],
      updated_keys: 10,
      appearance_rules_applied: 2,
      theme: "dark",
      autostart_enabled: false,
    });
    mockTaurpc.get_copy_to_clipboard_config.mockResolvedValue({
      enabled: true,
      min_log_length: 1,
      mask_cc: false,
      mask_ssn: false,
      mask_email: false,
      blacklist_processes: "",
      max_history_entries: 20,
      json_output_enabled: true,
      json_output_dir: "C:\\tmp\\clipboard-json",
      image_storage_dir: "C:\\tmp\\clipboard-images",
    });
    mockTaurpc.save_copy_to_clipboard_config.mockResolvedValue(null);
  });

  it("renders Appearance section and import/export section", () => {
    render(<ConfigTab appState={defaultState} onConfigLoaded={vi.fn()} />);
    expect(screen.getAllByText("Appearance").length).toBeGreaterThan(0);
    expect(
      screen.getByText(
        "NOTE: See 'Appearance' tab for detailed configurations and settings."
      )
    ).toBeInTheDocument();
    expect(screen.getByText("Import/Export Settings")).toBeInTheDocument();
  });

  it("exports settings bundle with selected groups", async () => {
    mockSave.mockResolvedValue("C:\\temp\\settings.json");
    render(<ConfigTab appState={defaultState} onConfigLoaded={vi.fn()} />);

    await userEvent.click(screen.getByRole("radio", { name: "Selected Groups" }));
    await userEvent.click(screen.getByRole("checkbox", { name: "Templates" }));
    await userEvent.click(screen.getByRole("checkbox", { name: "Sync" }));
    await userEvent.click(screen.getByRole("checkbox", { name: "Discovery" }));
    await userEvent.click(screen.getByRole("checkbox", { name: "Ghost Suggestor" }));
    await userEvent.click(screen.getByRole("checkbox", { name: "Clipboard History" }));
    await userEvent.click(screen.getByRole("checkbox", { name: "Copy-to-Clipboard" }));
    await userEvent.click(screen.getByRole("checkbox", { name: "Core" }));
    await userEvent.click(screen.getByRole("checkbox", { name: "Script Runtime" }));

    await userEvent.click(screen.getByRole("button", { name: "Export Settings JSON" }));

    await waitFor(() =>
      expect(mockTaurpc.export_settings_bundle_to_file).toHaveBeenCalledWith(
        "C:\\temp\\settings.json",
        ["ghost_follower", "appearance"],
        expect.any(String),
        expect.any(Boolean)
      )
    );
  });

  it("imports settings bundle and refreshes app state", async () => {
    const onConfigLoaded = vi.fn();
    mockOpen.mockResolvedValue("C:\\temp\\settings.json");
    render(<ConfigTab appState={defaultState} onConfigLoaded={onConfigLoaded} />);

    await userEvent.click(screen.getByRole("radio", { name: "Import" }));
    await userEvent.click(
      screen.getByRole("button", { name: "Select Import File (Preview)" })
    );
    await waitFor(() =>
      expect(mockTaurpc.preview_settings_bundle_from_file).toHaveBeenCalled()
    );
    await userEvent.click(
      screen.getByRole("button", { name: "Apply Import from Preview" })
    );

    await waitFor(() =>
      expect(mockTaurpc.import_settings_bundle_from_file).toHaveBeenCalled()
    );
    await waitFor(() => expect(mockTaurpc.get_app_state).toHaveBeenCalled());
    expect(onConfigLoaded).toHaveBeenCalled();
  });

  it("blocks apply until warnings are acknowledged", async () => {
    const onConfigLoaded = vi.fn();
    mockOpen.mockResolvedValue("C:\\temp\\settings.json");
    mockTaurpc.preview_settings_bundle_from_file.mockResolvedValue({
      path: "C:\\temp\\settings.json",
      schema_version: "1.0.0",
      available_groups: ["ghost_follower"],
      warnings: ["Unknown group 'legacy' will be ignored."],
      valid: true,
    });
    render(<ConfigTab appState={defaultState} onConfigLoaded={onConfigLoaded} />);

    await userEvent.click(screen.getByRole("radio", { name: "Import" }));
    await userEvent.click(
      screen.getByRole("button", { name: "Select Import File (Preview)" })
    );
    await waitFor(() =>
      expect(mockTaurpc.preview_settings_bundle_from_file).toHaveBeenCalled()
    );

    expect(screen.getByText("Review Warnings")).toBeInTheDocument();
    const applyBtn = screen.getByRole("button", {
      name: "Apply Import from Preview",
    });
    expect(applyBtn).toBeDisabled();

    await userEvent.click(
      screen.getByRole("checkbox", {
        name: "I reviewed and acknowledge the preview warnings before import.",
      })
    );
    expect(applyBtn).toBeEnabled();

    await userEvent.click(applyBtn);
    await waitFor(() =>
      expect(mockTaurpc.import_settings_bundle_from_file).toHaveBeenCalled()
    );
  });

  it("saves core pause toggle via update_config", async () => {
    render(<ConfigTab appState={defaultState} onConfigLoaded={vi.fn()} />);
    await userEvent.click(screen.getByRole("checkbox", { name: "Pause expansion (F7)" }));
    await userEvent.click(screen.getByRole("button", { name: "Save All Settings" }));

    await waitFor(() =>
      expect(mockTaurpc.update_config).toHaveBeenCalledWith(
        expect.objectContaining({ expansion_paused: true })
      )
    );
    await waitFor(() => expect(mockTaurpc.save_settings).toHaveBeenCalled());
  });

  it("saves copy-to-clipboard config subsection", async () => {
    render(<ConfigTab appState={defaultState} onConfigLoaded={vi.fn()} />);
    await userEvent.click(screen.getAllByText("Copy-to-Clipboard")[0]);
    await userEvent.click(
      screen.getByRole("checkbox", { name: "Mask email" })
    );
    await userEvent.click(
      screen.getByRole("button", { name: "Save Copy-to-Clipboard Settings" })
    );
    await waitFor(() =>
      expect(mockTaurpc.save_copy_to_clipboard_config).toHaveBeenCalledWith(
        expect.objectContaining({
          mask_email: true,
          json_output_dir: "C:\\tmp\\clipboard-json",
          image_storage_dir: "C:\\tmp\\clipboard-images",
        })
      )
    );
  });

  it("allows browsing and saving output/image directories", async () => {
    mockOpen.mockResolvedValueOnce("C:\\new\\json");
    mockOpen.mockResolvedValueOnce("C:\\new\\images");
    render(<ConfigTab appState={defaultState} onConfigLoaded={vi.fn()} />);
    await userEvent.click(screen.getAllByText("Copy-to-Clipboard")[0]);
    const browseButtons = screen.getAllByRole("button", { name: "Browse" });
    await userEvent.click(browseButtons[0]);
    await userEvent.click(browseButtons[1]);
    await userEvent.click(
      screen.getByRole("button", { name: "Save Copy-to-Clipboard Settings" })
    );
    await waitFor(() =>
      expect(mockTaurpc.save_copy_to_clipboard_config).toHaveBeenCalledWith(
        expect.objectContaining({
          json_output_dir: "C:\\new\\json",
          image_storage_dir: "C:\\new\\images",
        })
      )
    );
  });

  it("allows max history entries zero for unlimited", async () => {
    render(<ConfigTab appState={defaultState} onConfigLoaded={vi.fn()} />);
    await userEvent.click(screen.getAllByText("Copy-to-Clipboard")[0]);
    const maxDepthInput = screen.getByDisplayValue("20") as HTMLInputElement;
    fireEvent.change(maxDepthInput, { target: { value: "0" } });
    await userEvent.click(
      screen.getByRole("button", { name: "Save Copy-to-Clipboard Settings" })
    );
    await waitFor(() =>
      expect(mockTaurpc.save_copy_to_clipboard_config).toHaveBeenCalledWith(
        expect.objectContaining({
          max_history_entries: 0,
        })
      )
    );
    expect(screen.getByText("Note: 0 = Unlimited")).toBeInTheDocument();
  });
});
