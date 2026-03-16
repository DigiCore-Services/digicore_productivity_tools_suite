import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import { Label } from "../ui/label";
import { open } from "@tauri-apps/plugin-dialog";
import { getTaurpc } from "../../lib/taurpc";
import type { InteractiveVarDto, SnippetLogicTestResultDto } from "../../bindings";
import type { Snippet } from "../../types";

interface SnippetEditorProps {
  visible: boolean;
  mode: "add" | "edit";
  category: string;
  snippetIdx: number;
  initialSnippet?: Snippet | null;
  prefill?: { content: string; trigger: string };
  onSave: (snippet: Snippet) => void;
  onCancel: () => void;
}

export function SnippetEditor({
  visible,
  mode,
  category,
  snippetIdx,
  initialSnippet,
  prefill,
  onSave,
  onCancel,
}: SnippetEditorProps) {
  const [trigger, setTrigger] = useState("");
  const [profile, setProfile] = useState("Default");
  const [options, setOptions] = useState("*:");
  const [snippetCategory, setSnippetCategory] = useState("General");
  const [content, setContent] = useState("");
  const [appLock, setAppLock] = useState("");
  const [pinned, setPinned] = useState(false);
  const [testError, setTestError] = useState("");
  const [testResult, setTestResult] = useState("");
  const [testing, setTesting] = useState(false);
  const [copyStatus, setCopyStatus] = useState("");
  const [promptVars, setPromptVars] = useState<InteractiveVarDto[]>([]);
  const [promptValues, setPromptValues] = useState<Record<string, string>>({});
  const [showPrompt, setShowPrompt] = useState(false);
  const [testStatus, setTestStatus] = useState("");
  const [citySuggestionsByTag, setCitySuggestionsByTag] = useState<
    Record<string, string[]>
  >({});
  const [citySuggestLoadingTag, setCitySuggestLoadingTag] = useState("");
  const [citySuggestError, setCitySuggestError] = useState("");
  const resultPanelRef = useRef<HTMLDivElement | null>(null);
  const resultTextRef = useRef<HTMLTextAreaElement | null>(null);
  const activeRunIdRef = useRef(0);
  const lastIssuedRunIdRef = useRef(0);
  const testCacheRef = useRef<Map<string, SnippetLogicTestResultDto>>(new Map());
  const citySuggestTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const citySuggestCacheRef = useRef<Map<string, string[]>>(new Map());
  const latestCitySuggestReqRef = useRef(0);

  const TEST_TIMEOUT_MS = 30000;
  const SUGGEST_TIMEOUT_MS = 5000;

  useEffect(() => {
    if (visible) {
      if (mode === "edit" && initialSnippet) {
        setTrigger(initialSnippet.trigger || "");
        setProfile(initialSnippet.profile || "Default");
        setOptions(initialSnippet.options || "*:");
        setSnippetCategory(initialSnippet.category || category);
        setContent(initialSnippet.content || "");
        setAppLock(initialSnippet.app_lock || "");
        setPinned((initialSnippet.pinned || "").toLowerCase() === "true");
      } else if (mode === "add" && prefill) {
        setTrigger(prefill.trigger || "");
        setContent(prefill.content || "");
        setProfile("Default");
        setOptions("*:");
        setSnippetCategory(category || "General");
        setAppLock("");
        setPinned(false);
      } else {
        setTrigger("");
        setProfile("Default");
        setOptions("*:");
        setSnippetCategory(category || "General");
        setContent("");
        setAppLock("");
        setPinned(false);
      }
      setTestError("");
      setTestResult("");
      setPromptVars([]);
      setPromptValues({});
      setShowPrompt(false);
      setCopyStatus("");
      setTestStatus("");
      setCitySuggestionsByTag({});
      setCitySuggestLoadingTag("");
      setCitySuggestError("");
      citySuggestCacheRef.current.clear();
      activeRunIdRef.current = 0;
    }
  }, [visible, mode, category, initialSnippet, prefill]);

  useEffect(() => {
    return () => {
      if (citySuggestTimerRef.current) {
        clearTimeout(citySuggestTimerRef.current);
      }
    };
  }, []);

  useEffect(() => {
    if (!testResult) return;
    const panel = resultPanelRef.current as
      | (HTMLDivElement & {
          scrollIntoView?: (options?: ScrollIntoViewOptions) => void;
        })
      | null;
    if (panel && typeof panel.scrollIntoView === "function") {
      panel.scrollIntoView({
        behavior: "smooth",
        block: "nearest",
      });
    }
    resultTextRef.current?.focus();
  }, [testResult]);

  const handleSave = () => {
    const snippet: Snippet = {
      trigger: trigger.trim(),
      content,
      options: options.trim() || "*:",
      category: snippetCategory.trim(),
      profile: profile.trim() || "Default",
      app_lock: appLock.trim(),
      pinned: pinned ? "true" : "false",
      last_modified: "",
    };
    onSave(snippet);
  };

  const buildInitialPromptValues = (vars: InteractiveVarDto[]) => {
    const out: Record<string, string> = {};
    vars.forEach((v) => {
      if (v.var_type === "choice") out[v.tag] = v.options[0] || "";
      else out[v.tag] = "";
    });
    return out;
  };

  const normalizePromptValue = (v: InteractiveVarDto, raw: string) => {
    if (v.var_type === "date_picker") {
      return raw ? raw.replace(/-/g, "") : "";
    }
    if (v.var_type === "checkbox") {
      return raw === "true" ? v.options[0] || "true" : "";
    }
    return raw;
  };

  const runTest = async (providedValues?: Record<string, string>) => {
    const values = providedValues || null;
    const cacheKey = buildTestCacheKey(content, values);
    const currentRunId = ++lastIssuedRunIdRef.current;
    activeRunIdRef.current = currentRunId;
    setTesting(true);
    setTestError("");
    setTestStatus("Testing...");

    const cached = testCacheRef.current.get(cacheKey);
    if (cached) {
      applyTestResult(cached, providedValues);
      setTesting(false);
      setTestStatus("Loaded from test cache.");
      return;
    }

    try {
      const runPromise = getTaurpc().test_snippet_logic(content, values);
      const result = await withTimeout(runPromise, TEST_TIMEOUT_MS);
      if (activeRunIdRef.current !== currentRunId) {
        return;
      }
      testCacheRef.current.set(cacheKey, result);
      applyTestResult(result, providedValues);
      setTestStatus("Test completed.");
    } catch (e) {
      if (activeRunIdRef.current !== currentRunId) {
        return;
      }
      setTestError(
        String(e).includes("timed out")
          ? `Test timed out after ${Math.round(TEST_TIMEOUT_MS / 1000)}s. Adjust snippet or retry.`
          : "Script test failed: " + String(e)
      );
      setTestStatus("");
    } finally {
      if (activeRunIdRef.current === currentRunId) {
        setTesting(false);
      }
    }
  };

  const handleTest = async () => {
    if (!content.trim()) {
      setTestError("Enter snippet content to test.");
      setTestResult("");
      return;
    }
    await runTest();
  };

  const handlePromptSubmit = async () => {
    const values: Record<string, string> = {};
    promptVars.forEach((v) => {
      values[v.tag] = normalizePromptValue(v, promptValues[v.tag] || "");
    });
    await runTest(values);
  };

  const applyTestResult = (
    result: SnippetLogicTestResultDto,
    providedValues?: Record<string, string>
  ) => {
    if (result.requires_input && result.vars.length > 0 && !providedValues) {
      setPromptVars(result.vars);
      setPromptValues(buildInitialPromptValues(result.vars));
      setShowPrompt(true);
      setTestResult("");
      setCopyStatus("");
      return;
    }
    setShowPrompt(false);
    setPromptVars([]);
    setTestResult(result.result || "");
    setCopyStatus("");
  };

  const withTimeout = async <T,>(
    promise: Promise<T>,
    timeoutMs: number
  ): Promise<T> => {
    let timeoutId: ReturnType<typeof setTimeout> | undefined;
    const timeoutPromise = new Promise<T>((_, reject) => {
      timeoutId = setTimeout(() => {
        reject(new Error("test timed out"));
      }, timeoutMs);
    });
    try {
      return await Promise.race([promise, timeoutPromise]);
    } finally {
      if (timeoutId) clearTimeout(timeoutId);
    }
  };

  const buildTestCacheKey = (
    snippetContent: string,
    values: Record<string, string> | null
  ) => {
    const normalizedValues = values
      ? Object.keys(values)
          .sort()
          .map((k) => `${k}=${values[k]}`)
          .join("|")
      : "";
    return `${snippetContent}||${normalizedValues}`;
  };

  const handlePickFile = async (tag: string, title: string) => {
    const selected = await open({
      multiple: false,
      directory: false,
      title: title || "Select File",
    });
    if (selected && typeof selected === "string") {
      setPromptValues((prev) => ({ ...prev, [tag]: selected }));
    }
  };

  const handleCopyResult = async () => {
    if (!testResult) return;
    try {
      await getTaurpc().copy_to_clipboard(testResult);
      setCopyStatus("Result copied.");
    } catch (e) {
      setCopyStatus("Copy failed: " + String(e));
    }
  };

  const handleCancelRunningTest = () => {
    if (!testing) return;
    activeRunIdRef.current = 0;
    setTesting(false);
    setTestStatus("");
    setTestError("Test canceled.");
  };

  const handleEditorKeyDown = async (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== "Enter" || !e.ctrlKey) return;
    e.preventDefault();
    if (testing) return;
    if (showPrompt) {
      await handlePromptSubmit();
    } else {
      await handleTest();
    }
  };

  const promptVarText = (v: InteractiveVarDto) =>
    `${v.label || ""} ${v.tag || ""}`.toLowerCase();

  const isLikelyPromptVar = (
    v: InteractiveVarDto,
    kind: "city" | "country" | "state"
  ) => {
    const t = promptVarText(v);
    if (kind === "city") return t.includes("city") || t.includes("location");
    if (kind === "country") return t.includes("country");
    return t.includes("state") || t.includes("region") || t.includes("province");
  };

  const findPromptTagByKind = (kind: "country" | "state") =>
    promptVars.find((v) => v.var_type === "edit" && isLikelyPromptVar(v, kind))
      ?.tag;

  const fetchCitySuggestions = async (cityTag: string, cityQuery: string) => {
    const query = cityQuery.trim();
    if (query.length < 3) {
      setCitySuggestionsByTag((prev) => ({ ...prev, [cityTag]: [] }));
      return;
    }
    setCitySuggestError("");
    const countryTag = findPromptTagByKind("country");
    const stateTag = findPromptTagByKind("state");
    const country = countryTag ? promptValues[countryTag] || "" : "";
    const state = stateTag ? promptValues[stateTag] || "" : "";
    const cacheKey = `${query}|${country.trim()}|${state.trim()}`.toLowerCase();
    const cached = citySuggestCacheRef.current.get(cacheKey);
    if (cached) {
      setCitySuggestionsByTag((prev) => ({ ...prev, [cityTag]: cached }));
      return;
    }
    setCitySuggestLoadingTag(cityTag);
    const reqId = ++latestCitySuggestReqRef.current;
    try {
      const suggestions = await withTimeout(
        getTaurpc().get_weather_location_suggestions(
          query,
          country.trim() ? country.trim() : null,
          state.trim() ? state.trim() : null
        ),
        SUGGEST_TIMEOUT_MS
      );
      if (latestCitySuggestReqRef.current !== reqId) return;
      citySuggestCacheRef.current.set(cacheKey, suggestions);
      setCitySuggestionsByTag((prev) => ({
        ...prev,
        [cityTag]: suggestions,
      }));
    } catch (e) {
      if (latestCitySuggestReqRef.current !== reqId) return;
      setCitySuggestError(
        String(e).includes("timed out")
          ? "City suggestions timed out. Continue typing manually or retry Suggest."
          : "City suggestions unavailable right now."
      );
      setCitySuggestionsByTag((prev) => ({ ...prev, [cityTag]: [] }));
      console.warn("[SnippetEditor] weather city suggestions failed", e);
    } finally {
      if (latestCitySuggestReqRef.current === reqId) {
        setCitySuggestLoadingTag("");
      }
    }
  };

  const scheduleFetchCitySuggestions = (cityTag: string, cityQuery: string) => {
    if (citySuggestTimerRef.current) {
      clearTimeout(citySuggestTimerRef.current);
    }
    citySuggestTimerRef.current = setTimeout(() => {
      void fetchCitySuggestions(cityTag, cityQuery);
    }, 300);
  };

  const applyLocationSuggestionToPromptFields = (
    cityTag: string,
    selected: string
  ) => {
    const parts = selected
      .split(",")
      .map((p) => p.trim())
      .filter(Boolean);
    if (parts.length < 2) return;
    const cityOnly = parts[0];
    const countryTag = findPromptTagByKind("country");
    const stateTag = findPromptTagByKind("state");
    const countryValue = parts[parts.length - 1] || "";
    const stateValue = parts.length >= 3 ? parts[1] : "";
    setPromptValues((prev) => {
      const next = { ...prev, [cityTag]: cityOnly };
      if (countryTag && countryValue) {
        next[countryTag] = countryValue;
      }
      if (stateTag && stateValue) {
        next[stateTag] = stateValue;
      }
      return next;
    });
  };

  return (
    <AnimatePresence>
      {visible && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className="fixed inset-0 bg-black/50 z-[1000] flex items-center justify-center p-4"
          onClick={onCancel}
        >
          <motion.div
            initial={{ scale: 0.95, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            exit={{ scale: 0.95, opacity: 0 }}
            transition={{ type: "spring", duration: 0.3 }}
            className="bg-[var(--dc-bg-elevated)] text-[var(--dc-text)] p-6 rounded-xl max-w-[600px] w-full max-h-[90vh] overflow-y-auto border border-[var(--dc-border)] shadow-xl"
            onClick={(e) => e.stopPropagation()}
            onKeyDown={handleEditorKeyDown}
          >
            <h3 className="text-lg font-semibold mb-4">
              {mode === "add" ? "Add Snippet" : "Edit Snippet"}
            </h3>
            <div className="space-y-4">
              <div>
                <Label className="mb-1 block">Trigger</Label>
                <Input
                  value={trigger}
                  onChange={(e) => setTrigger(e.target.value)}
                  placeholder="e.g. /sig"
                />
              </div>
              <div>
                <Label className="mb-1 block">Profile</Label>
                <Input
                  value={profile}
                  onChange={(e) => setProfile(e.target.value)}
                  placeholder="Default"
                />
              </div>
              <div>
                <Label className="mb-1 block">Options</Label>
                <Input
                  value={options}
                  onChange={(e) => setOptions(e.target.value)}
                  placeholder="*:"
                />
              </div>
              <div>
                <Label className="mb-1 block">Category</Label>
                <Input
                  value={snippetCategory}
                  onChange={(e) => setSnippetCategory(e.target.value)}
                  placeholder="General"
                />
              </div>
              <div>
                <Label className="mb-1 block">Content</Label>
                <textarea
                  value={content}
                  onChange={(e) => setContent(e.target.value)}
                  placeholder="Snippet content..."
                  className="flex min-h-[120px] w-full rounded-md border border-[var(--dc-border)] bg-[var(--dc-bg)] px-3 py-2 text-sm"
                />
              </div>
              <div>
                <Label className="mb-1 block">AppLock</Label>
                <Input
                  value={appLock}
                  onChange={(e) => setAppLock(e.target.value)}
                  placeholder="comma-separated exe names"
                />
              </div>
              <label className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={pinned}
                  onChange={(e) => setPinned(e.target.checked)}
                  className="rounded"
                />
                <span>Pinned</span>
              </label>
            </div>
            <div className="mt-4 flex gap-2">
              <Button onClick={handleSave}>Save</Button>
              <Button variant="secondary" onClick={handleTest} disabled={testing}>
                {testing ? "Testing..." : "Test Script Logic"}
              </Button>
              {testing && (
                <Button variant="secondary" onClick={handleCancelRunningTest}>
                  Cancel Test Run
                </Button>
              )}
              <Button variant="secondary" onClick={onCancel}>
                Cancel
              </Button>
            </div>
            <p className="mt-2 text-xs text-[var(--dc-text-muted)]">
              Tip: Press Ctrl+Enter to run script logic test quickly.
            </p>
            {testStatus && (
              <p className="mt-2 text-xs text-[var(--dc-text-muted)]">{testStatus}</p>
            )}
            {showPrompt && (
              <div className="mt-4 border border-[var(--dc-border)] rounded p-3 space-y-3">
                <p className="text-sm font-medium">Test Input Variables</p>
                {promptVars.map((v) => (
                  <div key={v.tag}>
                    <Label className="mb-1 block">{v.label || v.tag}</Label>
                    {v.var_type === "choice" ? (
                      <select
                        value={promptValues[v.tag] || ""}
                        onChange={(e) =>
                          setPromptValues((prev) => ({
                            ...prev,
                            [v.tag]: e.target.value,
                          }))
                        }
                        className="flex h-9 w-full rounded-md border border-[var(--dc-border)] bg-[var(--dc-bg)] px-3 py-1 text-sm"
                      >
                        {v.options.map((opt) => (
                          <option key={opt} value={opt}>
                            {opt}
                          </option>
                        ))}
                      </select>
                    ) : v.var_type === "checkbox" ? (
                      <label className="flex items-center gap-2">
                        <input
                          type="checkbox"
                          checked={(promptValues[v.tag] || "") === "true"}
                          onChange={(e) =>
                            setPromptValues((prev) => ({
                              ...prev,
                              [v.tag]: e.target.checked ? "true" : "",
                            }))
                          }
                        />
                        <span className="text-sm">{v.options[0] || "Enabled value"}</span>
                      </label>
                    ) : v.var_type === "date_picker" ? (
                      <Input
                        type="date"
                        value={promptValues[v.tag] || ""}
                        onChange={(e) =>
                          setPromptValues((prev) => ({
                            ...prev,
                            [v.tag]: e.target.value,
                          }))
                        }
                      />
                    ) : v.var_type === "file_picker" ? (
                      <div className="flex gap-2">
                        <Input
                          value={promptValues[v.tag] || ""}
                          onChange={(e) =>
                            setPromptValues((prev) => ({
                              ...prev,
                              [v.tag]: e.target.value,
                            }))
                          }
                          placeholder="Select file path..."
                        />
                        <Button
                          type="button"
                          variant="secondary"
                          onClick={() => handlePickFile(v.tag, v.label)}
                        >
                          Browse
                        </Button>
                      </div>
                    ) : (
                      <>
                        <div className="flex gap-2">
                          <Input
                            list={
                              isLikelyPromptVar(v, "city")
                                ? `city-suggestions-${v.tag}`
                                : undefined
                            }
                            value={promptValues[v.tag] || ""}
                            onChange={(e) => {
                              const nextVal = e.target.value;
                              setPromptValues((prev) => ({
                                ...prev,
                                [v.tag]: nextVal,
                              }));
                              if (isLikelyPromptVar(v, "city")) {
                                const suggestionList = citySuggestionsByTag[v.tag] || [];
                                if (suggestionList.includes(nextVal)) {
                                  applyLocationSuggestionToPromptFields(v.tag, nextVal);
                                  return;
                                }
                                scheduleFetchCitySuggestions(v.tag, nextVal);
                              }
                            }}
                            onFocus={() => {
                              if (isLikelyPromptVar(v, "city")) {
                                const current = promptValues[v.tag] || "";
                                if (current.trim().length >= 2) {
                                  void fetchCitySuggestions(v.tag, current);
                                }
                              }
                            }}
                            placeholder={v.label || v.tag}
                          />
                          {isLikelyPromptVar(v, "city") && (
                            <Button
                              type="button"
                              variant="secondary"
                              onClick={() =>
                                fetchCitySuggestions(v.tag, promptValues[v.tag] || "")
                              }
                            >
                              Suggest
                            </Button>
                          )}
                        </div>
                        {isLikelyPromptVar(v, "city") && (
                          <>
                            <datalist id={`city-suggestions-${v.tag}`}>
                              {(citySuggestionsByTag[v.tag] || []).map((opt) => (
                                <option key={opt} value={opt} />
                              ))}
                            </datalist>
                            <p className="mt-1 text-xs text-[var(--dc-text-muted)]">
                              {citySuggestLoadingTag === v.tag
                                ? "Loading city suggestions..."
                                : `${(citySuggestionsByTag[v.tag] || []).length} suggestion(s). Selecting one auto-fills country/state when available.`}
                            </p>
                          </>
                        )}
                      </>
                    )}
                  </div>
                ))}
                <div className="flex gap-2">
                  <Button type="button" onClick={handlePromptSubmit} disabled={testing}>
                    {testing ? "Running..." : "Run Test"}
                  </Button>
                  <Button
                    type="button"
                    variant="secondary"
                    onClick={() => setShowPrompt(false)}
                    disabled={testing}
                  >
                    Cancel Test
                  </Button>
                </div>
              </div>
            )}
            {testError && (
              <p className="mt-3 text-sm text-red-500">{testError}</p>
            )}
            {citySuggestError && (
              <p className="mt-2 text-xs text-red-500">{citySuggestError}</p>
            )}
            {testResult && (
              <div
                ref={resultPanelRef}
                tabIndex={-1}
                className="mt-4 border border-[var(--dc-border)] rounded p-3"
              >
                <div className="mb-2 flex items-center justify-between gap-2">
                  <p className="text-sm font-medium">Simulated Expansion Result</p>
                  <Button
                    type="button"
                    variant="secondary"
                    size="sm"
                    onClick={handleCopyResult}
                    disabled={!testResult}
                  >
                    Copy Result
                  </Button>
                </div>
                <textarea
                  ref={resultTextRef}
                  value={testResult}
                  readOnly
                  className="flex min-h-[140px] w-full rounded-md border border-[var(--dc-border)] bg-[var(--dc-bg)] px-3 py-2 text-sm"
                />
                {copyStatus && (
                  <p className="mt-2 text-xs text-[var(--dc-text-muted)]">
                    {copyStatus}
                  </p>
                )}
              </div>
            )}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
