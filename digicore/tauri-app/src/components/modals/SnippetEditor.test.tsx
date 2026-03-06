import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { SnippetEditor } from "./SnippetEditor";

const mockTaurpc = {
  test_snippet_logic: vi.fn(),
  copy_to_clipboard: vi.fn(),
  get_weather_location_suggestions: vi.fn(),
};

const mockOpen = vi.fn();

vi.mock("@/lib/taurpc", () => ({
  getTaurpc: () => mockTaurpc,
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: (...args: unknown[]) => mockOpen(...args),
}));

describe("SnippetEditor test script logic", () => {
  beforeEach(() => {
    mockTaurpc.test_snippet_logic.mockReset();
    mockTaurpc.copy_to_clipboard.mockReset();
    mockTaurpc.get_weather_location_suggestions.mockReset();
    mockOpen.mockReset();
  });

  it("renders Test Script Logic button between Save and Cancel", () => {
    render(
      <SnippetEditor
        visible
        mode="add"
        category="General"
        snippetIdx={-1}
        initialSnippet={null}
        onSave={vi.fn()}
        onCancel={vi.fn()}
      />
    );

    const save = screen.getByRole("button", { name: "Save" });
    const test = screen.getByRole("button", { name: "Test Script Logic" });
    const cancel = screen.getByRole("button", { name: "Cancel" });
    expect(save).toBeInTheDocument();
    expect(test).toBeInTheDocument();
    expect(cancel).toBeInTheDocument();
  });

  it("runs preview and renders simulated expansion result", async () => {
    mockTaurpc.test_snippet_logic.mockResolvedValue({
      result: "Hello, World!",
      requires_input: false,
      vars: [],
    });
    render(
      <SnippetEditor
        visible
        mode="add"
        category="General"
        snippetIdx={-1}
        initialSnippet={null}
        onSave={vi.fn()}
        onCancel={vi.fn()}
      />
    );

    fireEvent.change(screen.getByPlaceholderText("Snippet content..."), {
      target: { value: "{js: 2+2}" },
    });
    await userEvent.click(screen.getByRole("button", { name: "Test Script Logic" }));

    await waitFor(() =>
      expect(mockTaurpc.test_snippet_logic).toHaveBeenCalledWith("{js: 2+2}", null)
    );
    expect(screen.getByText("Simulated Expansion Result")).toBeInTheDocument();
    expect(screen.getByDisplayValue("Hello, World!")).toBeInTheDocument();
  });

  it("prompts for interactive vars then runs final test", async () => {
    mockTaurpc.test_snippet_logic
      .mockResolvedValueOnce({
        result: "",
        requires_input: true,
        vars: [
          {
            tag: "{choice:Tone|Formal|Casual}",
            label: "Tone",
            var_type: "choice",
            options: ["Formal", "Casual"],
          },
        ],
      })
      .mockResolvedValueOnce({
        result: "Final: Casual",
        requires_input: false,
        vars: [],
      });

    render(
      <SnippetEditor
        visible
        mode="add"
        category="General"
        snippetIdx={-1}
        initialSnippet={null}
        onSave={vi.fn()}
        onCancel={vi.fn()}
      />
    );

    fireEvent.change(screen.getByPlaceholderText("Snippet content..."), {
      target: { value: "Tone: {choice:Tone|Formal|Casual}" },
    });
    await userEvent.click(screen.getByRole("button", { name: "Test Script Logic" }));
    expect(await screen.findByText("Test Input Variables")).toBeInTheDocument();

    await userEvent.selectOptions(screen.getByRole("combobox"), "Casual");
    await userEvent.click(screen.getByRole("button", { name: "Run Test" }));

    await waitFor(() =>
      expect(mockTaurpc.test_snippet_logic).toHaveBeenLastCalledWith(
        "Tone: {choice:Tone|Formal|Casual}",
        { "{choice:Tone|Formal|Casual}": "Casual" }
      )
    );
    expect(await screen.findByDisplayValue("Final: Casual")).toBeInTheDocument();
  });

  it("copies simulated result to clipboard", async () => {
    mockTaurpc.test_snippet_logic.mockResolvedValue({
      result: "Copy me",
      requires_input: false,
      vars: [],
    });
    mockTaurpc.copy_to_clipboard.mockResolvedValue(null);

    render(
      <SnippetEditor
        visible
        mode="add"
        category="General"
        snippetIdx={-1}
        initialSnippet={null}
        onSave={vi.fn()}
        onCancel={vi.fn()}
      />
    );

    fireEvent.change(screen.getByPlaceholderText("Snippet content..."), {
      target: { value: "{js: 1+1}" },
    });
    await userEvent.click(screen.getByRole("button", { name: "Test Script Logic" }));
    await screen.findByDisplayValue("Copy me");

    await userEvent.click(screen.getByRole("button", { name: "Copy Result" }));
    await waitFor(() =>
      expect(mockTaurpc.copy_to_clipboard).toHaveBeenCalledWith("Copy me")
    );
    expect(screen.getByText("Result copied.")).toBeInTheDocument();
  });

  it("runs test via Ctrl+Enter keyboard shortcut", async () => {
    mockTaurpc.test_snippet_logic.mockResolvedValue({
      result: "Shortcut result",
      requires_input: false,
      vars: [],
    });

    render(
      <SnippetEditor
        visible
        mode="add"
        category="General"
        snippetIdx={-1}
        initialSnippet={null}
        onSave={vi.fn()}
        onCancel={vi.fn()}
      />
    );

    const contentInput = screen.getByPlaceholderText("Snippet content...");
    fireEvent.change(contentInput, {
      target: { value: "Hello {date}" },
    });
    fireEvent.keyDown(contentInput, { key: "Enter", ctrlKey: true });

    await waitFor(() =>
      expect(mockTaurpc.test_snippet_logic).toHaveBeenCalledWith(
        "Hello {date}",
        null
      )
    );
    expect(await screen.findByDisplayValue("Shortcut result")).toBeInTheDocument();
  });

  it("uses cached test result on repeated runs", async () => {
    mockTaurpc.test_snippet_logic.mockResolvedValue({
      result: "Cached value",
      requires_input: false,
      vars: [],
    });

    render(
      <SnippetEditor
        visible
        mode="add"
        category="General"
        snippetIdx={-1}
        initialSnippet={null}
        onSave={vi.fn()}
        onCancel={vi.fn()}
      />
    );

    fireEvent.change(screen.getByPlaceholderText("Snippet content..."), {
      target: { value: "Cache test" },
    });

    await userEvent.click(screen.getByRole("button", { name: "Test Script Logic" }));
    expect(await screen.findByDisplayValue("Cached value")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Test Script Logic" }));
    await waitFor(() =>
      expect(screen.getByText("Loaded from test cache.")).toBeInTheDocument()
    );
    expect(mockTaurpc.test_snippet_logic).toHaveBeenCalledTimes(1);
  });

  it("allows cancelling a long-running test run", async () => {
    type TestResultPayload = {
      result: string;
      requires_input: boolean;
      vars: Array<unknown>;
    };
    let resolveDeferred!: (value: TestResultPayload) => void;
    mockTaurpc.test_snippet_logic.mockImplementation(
      () =>
        new Promise<TestResultPayload>((resolve) => {
          resolveDeferred = resolve;
        })
    );

    render(
      <SnippetEditor
        visible
        mode="add"
        category="General"
        snippetIdx={-1}
        initialSnippet={null}
        onSave={vi.fn()}
        onCancel={vi.fn()}
      />
    );

    fireEvent.change(screen.getByPlaceholderText("Snippet content..."), {
      target: { value: "Long running" },
    });
    await userEvent.click(screen.getByRole("button", { name: "Test Script Logic" }));

    expect(screen.getByRole("button", { name: "Cancel Test Run" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Cancel Test Run" }));
    expect(screen.getByText("Test canceled.")).toBeInTheDocument();

    resolveDeferred({ result: "Late result", requires_input: false, vars: [] });
    await Promise.resolve();
    expect(screen.queryByDisplayValue("Late result")).not.toBeInTheDocument();
  });

  it("shows city suggestions in variable prompt", async () => {
    mockTaurpc.test_snippet_logic.mockResolvedValue({
      result: "",
      requires_input: true,
      vars: [
        { tag: "{var:City}", label: "City", var_type: "edit", options: [] },
        { tag: "{var:Country}", label: "Country", var_type: "edit", options: [] },
        { tag: "{var:State}", label: "State", var_type: "edit", options: [] },
      ],
    });
    mockTaurpc.get_weather_location_suggestions.mockResolvedValue([
      "Los Angeles, California, United States",
      "Los Angeles, Biobio, Chile",
    ]);

    render(
      <SnippetEditor
        visible
        mode="add"
        category="General"
        snippetIdx={-1}
        initialSnippet={null}
        onSave={vi.fn()}
        onCancel={vi.fn()}
      />
    );

    fireEvent.change(screen.getByPlaceholderText("Snippet content..."), {
      target: { value: "{weather:city={var:City}|country={var:Country}|state={var:State}}" },
    });
    await userEvent.click(screen.getByRole("button", { name: "Test Script Logic" }));
    expect(await screen.findByText("Test Input Variables")).toBeInTheDocument();

    const cityInput = screen.getByPlaceholderText("City");
    fireEvent.change(cityInput, { target: { value: "Los Angeles" } });
    await userEvent.click(screen.getByRole("button", { name: "Suggest" }));

    await waitFor(() =>
      expect(mockTaurpc.get_weather_location_suggestions).toHaveBeenCalledWith(
        "Los Angeles",
        null,
        null
      )
    );
  });

  it("auto-fills state and country from selected city suggestion", async () => {
    mockTaurpc.test_snippet_logic.mockResolvedValue({
      result: "",
      requires_input: true,
      vars: [
        { tag: "{var:City}", label: "City", var_type: "edit", options: [] },
        { tag: "{var:Country}", label: "Country", var_type: "edit", options: [] },
        { tag: "{var:State}", label: "State", var_type: "edit", options: [] },
      ],
    });
    mockTaurpc.get_weather_location_suggestions.mockResolvedValue([
      "Los Angeles, California, United States",
    ]);

    render(
      <SnippetEditor
        visible
        mode="add"
        category="General"
        snippetIdx={-1}
        initialSnippet={null}
        onSave={vi.fn()}
        onCancel={vi.fn()}
      />
    );

    fireEvent.change(screen.getByPlaceholderText("Snippet content..."), {
      target: { value: "{weather:city={var:City}|country={var:Country}|state={var:State}}" },
    });
    await userEvent.click(screen.getByRole("button", { name: "Test Script Logic" }));

    const cityInput = screen.getByPlaceholderText("City") as HTMLInputElement;
    fireEvent.change(cityInput, { target: { value: "Los Angeles" } });
    await userEvent.click(screen.getByRole("button", { name: "Suggest" }));
    await waitFor(() =>
      expect(mockTaurpc.get_weather_location_suggestions).toHaveBeenCalled()
    );

    fireEvent.change(cityInput, {
      target: { value: "Los Angeles, California, United States" },
    });

    expect((screen.getByPlaceholderText("City") as HTMLInputElement).value).toBe(
      "Los Angeles"
    );
    expect((screen.getByPlaceholderText("State") as HTMLInputElement).value).toBe(
      "California"
    );
    expect((screen.getByPlaceholderText("Country") as HTMLInputElement).value).toBe(
      "United States"
    );
  });
});

