import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ScriptTab } from "./ScriptTab";

const mockApi = {
  get_app_state: vi.fn(),
  get_script_library_js: vi.fn(),
  get_script_library_py: vi.fn(),
  get_script_library_lua: vi.fn(),
  save_script_library_js: vi.fn(),
  save_script_library_py: vi.fn(),
  save_script_library_lua: vi.fn(),
  get_scripting_engine_config: vi.fn(),
  save_scripting_engine_config: vi.fn(),
  update_config: vi.fn(),
  save_settings: vi.fn(),
  test_snippet_logic: vi.fn(),
  export_scripting_profile_to_file: vi.fn(),
  export_scripting_profile_with_detached_signature_to_file: vi.fn(),
  preview_scripting_profile_from_file: vi.fn(),
  dry_run_import_scripting_profile_from_file: vi.fn(),
  import_scripting_profile_from_file: vi.fn(),
  get_scripting_signer_registry: vi.fn(),
  save_scripting_signer_registry: vi.fn(),
  get_diagnostic_logs: vi.fn(),
};

const mockOpen = vi.fn();
const mockSave = vi.fn();

vi.mock("@/lib/taurpc", () => ({
  getTaurpc: () => mockApi,
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: (...args: unknown[]) => mockOpen(...args),
  save: (...args: unknown[]) => mockSave(...args),
}));

describe("ScriptTab sub-tabs", () => {
  beforeEach(() => {
    Object.values(mockApi).forEach((fn) => fn.mockReset());
    mockOpen.mockReset();
    mockSave.mockReset();
    mockApi.get_app_state.mockResolvedValue({
      script_library_run_disabled: true,
      script_library_run_allowlist: "python,cmd",
    });
    mockApi.get_script_library_js.mockResolvedValue("function greet(){ return 'hi'; }");
    mockApi.get_script_library_py.mockResolvedValue("def py_greet(name): return name");
    mockApi.get_script_library_lua.mockResolvedValue("function lua_greet(name) return name end");
    mockApi.save_script_library_js.mockResolvedValue(null);
    mockApi.save_script_library_py.mockResolvedValue(null);
    mockApi.save_script_library_lua.mockResolvedValue(null);
    mockApi.get_scripting_engine_config.mockResolvedValue({
      dsl: { enabled: true },
      http: { timeout_secs: 5, retry_count: 3, retry_delay_ms: 500, use_async: false },
      py: { enabled: false, path: "", library_path: "scripts/global_library.py" },
      lua: { enabled: false, path: "", library_path: "scripts/global_library.lua" },
    });
    mockApi.save_scripting_engine_config.mockResolvedValue(null);
    mockApi.update_config.mockResolvedValue(null);
    mockApi.save_settings.mockResolvedValue(null);
    mockApi.test_snippet_logic.mockResolvedValue({
      result: "ok",
      requires_input: false,
      vars: [],
    });
    mockApi.export_scripting_profile_to_file.mockResolvedValue(6);
    mockApi.export_scripting_profile_with_detached_signature_to_file.mockResolvedValue({
      profile_path: "C:\\temp\\scripting_profile.json",
      signature_path: "C:\\temp\\scripting_profile.sig.json",
      key_id: "abc123",
      signer_fingerprint: "abc123def456",
      payload_sha256: "sha256value",
    });
    mockApi.preview_scripting_profile_from_file.mockResolvedValue({
      path: "C:\\temp\\scripting_profile.json",
      schema_version: "2.0.0",
      available_groups: ["javascript", "python", "run"],
      warnings: [],
      valid: true,
      signed_bundle: true,
      signature_valid: true,
      migrated_from_schema: null,
      signature_key_id: "abc123",
      signer_fingerprint: "abc123def456",
      signer_trusted: false,
    });
    mockApi.dry_run_import_scripting_profile_from_file.mockResolvedValue({
      path: "C:\\temp\\scripting_profile.json",
      selected_groups: ["javascript"],
      changed_groups: ["javascript"],
      estimated_updates: 1,
      warnings: [],
      diff_entries: [
        {
          group: "javascript",
          field: "library",
          current_value: "old",
          incoming_value: "new",
        },
      ],
      schema_version_used: "2.0.0",
      signature_valid: true,
      migrated_from_schema: null,
      signer_fingerprint: "abc123def456",
      signer_trusted: false,
    });
    mockApi.import_scripting_profile_from_file.mockResolvedValue({
      applied_groups: ["javascript"],
      skipped_groups: [],
      warnings: [],
      updated_keys: 2,
      schema_version_used: "2.0.0",
      signature_valid: true,
      migrated_from_schema: null,
      signer_fingerprint: "abc123def456",
      signer_trusted: false,
    });
    mockApi.get_scripting_signer_registry.mockResolvedValue({
      allow_unknown_signers: true,
      trust_on_first_use: false,
      trusted_fingerprints: [],
      blocked_fingerprints: [],
    });
    mockApi.save_scripting_signer_registry.mockResolvedValue(null);
    mockApi.get_diagnostic_logs.mockResolvedValue([
      {
        timestamp_ms: Date.now(),
        level: "info",
        message:
          "[ScriptingSignerTOFU][AUDIT] First trust established signer=abc123 at=123 source=preview",
      },
      {
        timestamp_ms: Date.now(),
        level: "info",
        message: "[Scripting][Config] Saved dsl_enabled=true http_async=false py_enabled=false lua_enabled=false",
      },
    ]);
  });

  it("renders engine sub-tabs and loads script libraries", async () => {
    render(<ScriptTab appState={null} />);
    expect(await screen.findByText("Scripting Engine Library")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "JavaScript" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Python" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Lua" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "HTTP/Weather" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Run Security" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Diagnostics" })).toBeInTheDocument();
    await waitFor(() => expect(mockApi.get_script_library_py).toHaveBeenCalled());
  });

  it("saves python library from Python sub-tab", async () => {
    render(<ScriptTab appState={null} />);
    await screen.findByText("Scripting Engine Library");
    await userEvent.click(screen.getByRole("button", { name: "Python" }));
    const textarea = screen.getByDisplayValue(
      "def py_greet(name): return name"
    ) as HTMLTextAreaElement;
    fireEvent.change(textarea, { target: { value: "def helper(): return 42" } });
    await userEvent.click(screen.getByRole("button", { name: "Save Python Library" }));
    await waitFor(() =>
      expect(mockApi.save_script_library_py).toHaveBeenCalledWith(
        "def helper(): return 42"
      )
    );
  });

  it("runs quick test from HTTP/Weather tab", async () => {
    render(<ScriptTab appState={null} />);
    await screen.findByText("Scripting Engine Library");
    await userEvent.click(screen.getByRole("button", { name: "HTTP/Weather" }));
    await userEvent.click(screen.getByRole("button", { name: "Run Quick Test" }));
    await waitFor(() => expect(mockApi.test_snippet_logic).toHaveBeenCalled());
    expect(await screen.findByText("ok")).toBeInTheDocument();
  });

  it("saves HTTP/Weather engine settings", async () => {
    render(<ScriptTab appState={null} />);
    await screen.findByText("Scripting Engine Library");
    await userEvent.click(screen.getByRole("button", { name: "HTTP/Weather" }));
    await userEvent.click(
      screen.getByRole("button", { name: "Save HTTP/Weather Settings" })
    );
    await waitFor(() =>
      expect(mockApi.save_scripting_engine_config).toHaveBeenCalled()
    );
  });

  it("inserts JavaScript starter template into library editor", async () => {
    render(<ScriptTab appState={null} />);
    await screen.findByText("Scripting Engine Library");
    await userEvent.click(screen.getByRole("button", { name: "JavaScript" }));
    await userEvent.click(screen.getByRole("button", { name: "Insert Template" }));
    const jsEditor = screen.getAllByRole("textbox")[0] as HTMLTextAreaElement;
    expect(jsEditor.value).toContain("function greet(name)");
  });

  it("blocks saving when validation errors exist", async () => {
    render(<ScriptTab appState={null} />);
    await screen.findByText("Scripting Engine Library");
    await userEvent.click(screen.getByRole("button", { name: "Python" }));
    const libPathInput = screen.getByPlaceholderText(
      "Python global library path"
    ) as HTMLInputElement;
    fireEvent.change(libPathInput, { target: { value: "scripts/library.txt" } });
    const savePy = screen.getByRole("button", { name: "Save Python Library" });
    expect(savePy).toBeDisabled();
  });

  it("exports engine profile json with selected groups", async () => {
    mockSave.mockResolvedValue("C:\\temp\\scripting_profile.json");
    render(<ScriptTab appState={null} />);
    await screen.findByText("Scripting Engine Library");
    await userEvent.click(screen.getByRole("radio", { name: "Selected Groups" }));
    await userEvent.click(screen.getByRole("checkbox", { name: "Python" }));
    await userEvent.click(
      screen.getByRole("button", { name: "Export Engine Profile JSON" })
    );
    await waitFor(() =>
      expect(mockApi.export_scripting_profile_to_file).toHaveBeenCalledWith(
        "C:\\temp\\scripting_profile.json",
        expect.arrayContaining(["javascript", "lua", "http", "dsl", "run"])
      )
    );
  });

  it("imports engine profile from preview", async () => {
    mockOpen.mockResolvedValue("C:\\temp\\scripting_profile.json");
    render(<ScriptTab appState={null} />);
    await screen.findByText("Scripting Engine Library");
    await userEvent.click(screen.getByRole("radio", { name: "Import" }));
    await userEvent.click(
      screen.getByRole("button", { name: "Select Import File (Preview)" })
    );
    await waitFor(() =>
      expect(mockApi.preview_scripting_profile_from_file).toHaveBeenCalled()
    );
    await userEvent.click(
      screen.getByRole("button", { name: "Apply Import from Preview" })
    );
    await waitFor(() =>
      expect(mockApi.import_scripting_profile_from_file).toHaveBeenCalled()
    );
  });

  it("runs engine profile dry-run diff preview", async () => {
    mockOpen.mockResolvedValue("C:\\temp\\scripting_profile.json");
    render(<ScriptTab appState={null} />);
    await screen.findByText("Scripting Engine Library");
    await userEvent.click(screen.getByRole("radio", { name: "Import" }));
    await userEvent.click(
      screen.getByRole("button", { name: "Select Import File (Preview)" })
    );
    await waitFor(() =>
      expect(mockApi.preview_scripting_profile_from_file).toHaveBeenCalled()
    );
    await userEvent.click(screen.getByRole("button", { name: "Run Dry-Run Diff" }));
    await waitFor(() =>
      expect(mockApi.dry_run_import_scripting_profile_from_file).toHaveBeenCalled()
    );
    expect(await screen.findByText("Estimated Updates:")).toBeInTheDocument();
  });

  it("saves signer registry and supports trust button", async () => {
    mockOpen.mockResolvedValue("C:\\temp\\scripting_profile.json");
    render(<ScriptTab appState={null} />);
    await screen.findByText("Scripting Engine Library");
    await userEvent.click(screen.getByRole("radio", { name: "Import" }));
    await userEvent.click(
      screen.getByRole("button", { name: "Select Import File (Preview)" })
    );
    await waitFor(() =>
      expect(mockApi.preview_scripting_profile_from_file).toHaveBeenCalled()
    );
    await userEvent.click(screen.getByRole("button", { name: "Trust This Signer" }));
    await userEvent.click(screen.getByRole("button", { name: "Save Signer Registry" }));
    await waitFor(() =>
      expect(mockApi.save_scripting_signer_registry).toHaveBeenCalled()
    );
    expect(mockApi.save_scripting_signer_registry).toHaveBeenCalledWith(
      expect.objectContaining({ trust_on_first_use: false })
    );
  });

  it("shows TOFU-only audit history in diagnostics sub-tab", async () => {
    render(<ScriptTab appState={null} />);
    await screen.findByText("Scripting Engine Library");
    await userEvent.click(screen.getByRole("button", { name: "Diagnostics" }));
    await waitFor(() => expect(mockApi.get_diagnostic_logs).toHaveBeenCalled());
    expect(await screen.findByText(/TOFU Audit History/)).toBeInTheDocument();
    expect(
      screen.getByText(/First trust established signer=abc123/)
    ).toBeInTheDocument();
    expect(screen.queryByText(/\[Scripting\]\[Config\] Saved/)).not.toBeInTheDocument();
  });
});

