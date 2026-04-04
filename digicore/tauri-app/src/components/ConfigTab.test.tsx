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
  kms_get_indexing_status: vi.fn(),
  kms_request_note_embedding_migration: vi.fn(),
  kms_get_embedding_policy_diagnostics: vi.fn(),
  kms_get_embedding_diagnostic_log_path: vi.fn(),
  kms_cancel_note_embedding_migration: vi.fn(),
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
  listen: vi.fn(() => Promise.resolve(() => {})),
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
  kms_graph_k_means_max_k: 10,
  kms_graph_k_means_iterations: 15,
  kms_graph_ai_beam_max_nodes: 400,
  kms_graph_ai_beam_similarity_threshold: 0.9,
  kms_graph_ai_beam_max_edges: 20,
  kms_graph_enable_ai_beams: true,
  kms_graph_enable_semantic_clustering: true,
  kms_graph_enable_leiden_communities: true,
  kms_graph_semantic_max_notes: 2500,
  kms_graph_warn_note_threshold: 1500,
  kms_graph_auto_paging_enabled: true,
  kms_graph_auto_paging_note_threshold: 2000,
  kms_graph_beam_max_pair_checks: 200000,
  kms_graph_enable_semantic_knn_edges: true,
  kms_graph_semantic_knn_per_note: 5,
  kms_graph_semantic_knn_min_similarity: 0.82,
  kms_graph_semantic_knn_max_edges: 8000,
  kms_graph_semantic_knn_max_pair_checks: 400000,
  kms_graph_pagerank_iterations: 48,
  kms_graph_pagerank_local_iterations: 32,
  kms_graph_pagerank_damping: 0.85,
  kms_graph_pagerank_scope: "auto",
  kms_graph_background_wiki_pagerank_enabled: true,
  kms_graph_temporal_window_enabled: false,
  kms_graph_temporal_default_days: 0,
  kms_graph_temporal_include_notes_without_mtime: true,
  kms_graph_temporal_edge_recency_enabled: false,
  kms_graph_temporal_edge_recency_strength: 1.0,
  kms_graph_temporal_edge_recency_half_life_days: 30.0,
  kms_search_min_similarity: 0.0,
  kms_search_include_embedding_diagnostics: true,
  kms_search_default_mode: "Hybrid",
  kms_search_default_limit: 20,
  kms_embedding_model_id: "",
  kms_embedding_batch_notes_per_tick: 8,
  kms_embedding_chunk_enabled: false,
  kms_embedding_chunk_max_chars: 2048,
  kms_embedding_chunk_overlap_chars: 128,
  kms_graph_sprite_label_max_dpr_scale: 2.5,
  kms_graph_sprite_label_min_res_scale: 1.25,
  kms_graph_webworker_layout_threshold: 800,
  kms_graph_webworker_layout_max_ticks: 450,
  kms_graph_webworker_layout_alpha_min: 0.02,
} as unknown as AppState;

describe("ConfigTab import/export settings", () => {
  beforeEach(() => {
    localStorage.removeItem("digicore-config-subtab");
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
    mockTaurpc.kms_get_indexing_status.mockReset();
    mockTaurpc.kms_get_indexing_status.mockResolvedValue([]);
    mockTaurpc.kms_request_note_embedding_migration.mockReset();
    mockTaurpc.kms_request_note_embedding_migration.mockResolvedValue(1);
    mockTaurpc.kms_get_embedding_policy_diagnostics.mockReset();
    mockTaurpc.kms_get_embedding_policy_diagnostics.mockResolvedValue({
      indexed_note_count: 0,
      stale_embedding_note_count: 0,
      expected_policy_signature: "",
      total_notes_in_index: 0,
      pending_note_count: 0,
      failed_sync_note_count: 0,
      embedding_aligned_note_count: 0,
      other_sync_status_note_count: 0,
      vault_markdown_files_on_disk: 0,
      vault_all_files_on_disk: 0,
    });
    mockTaurpc.kms_get_embedding_diagnostic_log_path.mockReset();
    mockTaurpc.kms_get_embedding_diagnostic_log_path.mockResolvedValue(
      "C:\\fake\\DigiCore\\logs\\kms_embedding.log"
    );
    mockTaurpc.kms_cancel_note_embedding_migration.mockReset();
    mockTaurpc.kms_cancel_note_embedding_migration.mockResolvedValue(undefined);
    mockTaurpc.get_app_state.mockResolvedValue(defaultState);
    mockTaurpc.export_settings_bundle_to_file.mockResolvedValue(9);
    mockTaurpc.preview_settings_bundle_from_file.mockResolvedValue({
      path: "C:\\temp\\settings.json",
      schema_version: "1.1.0",
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

  it("renders Appearance section and import/export section", async () => {
    const user = userEvent.setup();
    render(<ConfigTab appState={defaultState} onConfigLoaded={vi.fn()} />);
    await user.click(screen.getByRole("button", { name: "Appearance" }));
    expect(await screen.findByRole("heading", { name: "Appearance" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Import/Export" }));
    expect(screen.getByRole("heading", { name: "Import/Export Settings" })).toBeInTheDocument();
  });

  it("exports settings bundle with selected groups", async () => {
    const user = userEvent.setup();
    mockSave.mockResolvedValue("C:\\temp\\settings.json");
    render(<ConfigTab appState={defaultState} onConfigLoaded={vi.fn()} />);

    await user.click(screen.getByRole("button", { name: "Import/Export" }));
    await user.click(screen.getByRole("radio", { name: "Selected Groups" }));
    await user.click(screen.getByRole("checkbox", { name: "Templates" }));
    await user.click(screen.getByRole("checkbox", { name: "Sync" }));
    await user.click(screen.getByRole("checkbox", { name: "Discovery" }));
    await user.click(screen.getByRole("checkbox", { name: "Ghost Suggestor" }));
    await user.click(screen.getByRole("checkbox", { name: "Clipboard History" }));
    await user.click(screen.getByRole("checkbox", { name: "Copy-to-Clipboard" }));
    await user.click(screen.getByRole("checkbox", { name: "Text Expansion" }));
    await user.click(screen.getByRole("checkbox", { name: "Core" }));
    await user.click(screen.getByRole("checkbox", { name: "Script Runtime" }));
    await user.click(screen.getByRole("checkbox", { name: "Corpus Generation" }));
    await user.click(screen.getByRole("checkbox", { name: "Extraction Engine" }));
    await user.click(screen.getByRole("checkbox", { name: "Statistics" }));
    await user.click(screen.getByRole("checkbox", { name: "Log" }));
    await user.click(screen.getByRole("checkbox", { name: "Semantic Search" }));
    await user.click(screen.getByRole("checkbox", { name: "Knowledge Graph" }));
    await user.click(screen.getByRole("checkbox", { name: "KMS Search and embeddings" }));

    await user.click(screen.getByRole("button", { name: "Export Settings JSON" }));

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
    const user = userEvent.setup();
    const onConfigLoaded = vi.fn();
    mockOpen.mockResolvedValue("C:\\temp\\settings.json");
    render(<ConfigTab appState={defaultState} onConfigLoaded={onConfigLoaded} />);

    await user.click(screen.getByRole("button", { name: "Import/Export" }));
    await user.click(screen.getByRole("radio", { name: "Import" }));
    await user.click(
      screen.getByRole("button", { name: "Select Import File (Preview)" })
    );
    await waitFor(() =>
      expect(mockTaurpc.preview_settings_bundle_from_file).toHaveBeenCalled()
    );
    await user.click(
      screen.getByRole("button", { name: "Apply Import from Preview" })
    );

    await waitFor(() =>
      expect(mockTaurpc.import_settings_bundle_from_file).toHaveBeenCalled()
    );
    await waitFor(() => expect(mockTaurpc.get_app_state).toHaveBeenCalled());
    expect(onConfigLoaded).toHaveBeenCalled();
  });

  it("blocks apply until warnings are acknowledged", async () => {
    const user = userEvent.setup();
    const onConfigLoaded = vi.fn();
    mockOpen.mockResolvedValue("C:\\temp\\settings.json");
    mockTaurpc.preview_settings_bundle_from_file.mockResolvedValue({
      path: "C:\\temp\\settings.json",
      schema_version: "1.1.0",
      available_groups: ["ghost_follower"],
      warnings: ["Unknown group 'legacy' will be ignored."],
      valid: true,
    });
    render(<ConfigTab appState={defaultState} onConfigLoaded={onConfigLoaded} />);

    await user.click(screen.getByRole("button", { name: "Import/Export" }));
    await user.click(screen.getByRole("radio", { name: "Import" }));
    await user.click(
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

    await user.click(
      screen.getByRole("checkbox", {
        name: "I reviewed and acknowledge the preview warnings before import.",
      })
    );
    expect(applyBtn).toBeEnabled();

    await user.click(applyBtn);
    await waitFor(() =>
      expect(mockTaurpc.import_settings_bundle_from_file).toHaveBeenCalled()
    );
  });

  it("saves core pause toggle via update_config", async () => {
    const user = userEvent.setup();
    render(<ConfigTab appState={defaultState} onConfigLoaded={vi.fn()} />);
    await user.click(screen.getByRole("button", { name: "Core" }));
    await user.click(screen.getByRole("checkbox", { name: "Pause expansion (F7)" }));
    await user.click(screen.getByRole("button", { name: "Save All Settings" }));

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

describe("ConfigTab KMS embeddings", () => {
  beforeEach(() => {
    localStorage.removeItem("digicore-config-subtab");
    localStorage.removeItem("digicore-kms-embedding-health-report-v1");
    mockTaurpc.update_config.mockReset();
    mockTaurpc.save_settings.mockReset();
    mockTaurpc.get_app_state.mockReset();
    mockTaurpc.kms_request_note_embedding_migration.mockReset();
    mockTaurpc.kms_request_note_embedding_migration.mockResolvedValue(1);
    mockTaurpc.kms_get_embedding_policy_diagnostics.mockReset();
    mockTaurpc.kms_get_embedding_policy_diagnostics.mockResolvedValue({
      indexed_note_count: 0,
      stale_embedding_note_count: 0,
      expected_policy_signature: "",
      total_notes_in_index: 0,
      pending_note_count: 0,
      failed_sync_note_count: 0,
      embedding_aligned_note_count: 0,
      other_sync_status_note_count: 0,
      vault_markdown_files_on_disk: 0,
      vault_all_files_on_disk: 0,
    });
    mockTaurpc.kms_get_embedding_diagnostic_log_path.mockReset();
    mockTaurpc.kms_get_embedding_diagnostic_log_path.mockResolvedValue(
      "C:\\fake\\DigiCore\\logs\\kms_embedding.log"
    );
    mockTaurpc.kms_cancel_note_embedding_migration.mockReset();
    mockTaurpc.kms_cancel_note_embedding_migration.mockResolvedValue(undefined);
    mockTaurpc.get_app_state.mockResolvedValue(defaultState);
  });

  it("queues vault re-embed from KMS Search and embeddings", async () => {
    const user = userEvent.setup();
    render(<ConfigTab appState={defaultState} onConfigLoaded={vi.fn()} />);
    await user.click(screen.getByRole("button", { name: "KMS Search and embeddings" }));
    await user.click(screen.getByTestId("kms-reembed-vault-btn"));
    await waitFor(() =>
      expect(mockTaurpc.kms_request_note_embedding_migration).toHaveBeenCalled()
    );
  });

  it("saves embedding model id and batch size with search settings", async () => {
    const user = userEvent.setup();
    const onLoaded = vi.fn();
    render(<ConfigTab appState={defaultState} onConfigLoaded={onLoaded} />);
    await user.click(screen.getByRole("button", { name: "KMS Search and embeddings" }));
    const modelInput = screen.getByPlaceholderText("BGESmallENV15");
    await user.clear(modelInput);
    await user.type(modelInput, "CustomModel");
    await user.click(
      screen.getByRole("button", { name: "Save search and embedding settings" })
    );
    await waitFor(() =>
      expect(mockTaurpc.update_config).toHaveBeenCalledWith(
        expect.objectContaining({
          kms_embedding_model_id: "CustomModel",
          kms_embedding_batch_notes_per_tick: 8,
        })
      )
    );
  });
});
