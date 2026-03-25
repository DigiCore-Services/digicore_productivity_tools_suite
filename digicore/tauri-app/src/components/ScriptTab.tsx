import { useEffect, useMemo, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { getTaurpc } from "@/lib/taurpc";
import type { AppState } from "../types";
import { ScriptCodeEditor } from "./ScriptCodeEditor";

interface ScriptTabProps {
  appState: AppState | null;
}

type TemplateItem = {
  label: string;
  content: string;
};

type EngineProfileGroupId =
  | "javascript"
  | "python"
  | "lua"
  | "http"
  | "dsl"
  | "run";

type EngineProfilePreview = {
  path: string;
  schema_version: string;
  available_groups: string[];
  warnings: string[];
  valid: boolean;
  signed_bundle: boolean;
  signature_valid: boolean;
  migrated_from_schema: string | null;
  signature_key_id: string | null;
  signer_fingerprint: string | null;
  signer_trusted: boolean;
};

type EngineProfileImportResult = {
  applied_groups: string[];
  skipped_groups: string[];
  warnings: string[];
  updated_keys: number;
  schema_version_used: string;
  signature_valid: boolean;
  migrated_from_schema: string | null;
  signer_fingerprint: string | null;
  signer_trusted: boolean;
};

type EngineProfileDiffEntry = {
  group: string;
  field: string;
  current_value: string;
  incoming_value: string;
};

type EngineProfileDryRun = {
  path: string;
  selected_groups: string[];
  changed_groups: string[];
  estimated_updates: number;
  warnings: string[];
  diff_entries: EngineProfileDiffEntry[];
  schema_version_used: string;
  signature_valid: boolean;
  migrated_from_schema: string | null;
  signer_fingerprint: string | null;
  signer_trusted: boolean;
};

type EngineSignerRegistry = {
  allow_unknown_signers: boolean;
  trust_on_first_use: boolean;
  trusted_fingerprints: string[];
  blocked_fingerprints: string[];
};

type DetachedSignatureExportResult = {
  profile_path: string;
  signature_path: string;
  key_id: string;
  signer_fingerprint: string;
  payload_sha256: string;
};

type DiagnosticEntry = {
  timestamp_ms: number;
  level: string;
  message: string;
};

const ENGINE_PROFILE_GROUP_OPTIONS: Array<{ id: EngineProfileGroupId; label: string }> = [
  { id: "javascript", label: "JavaScript" },
  { id: "python", label: "Python" },
  { id: "lua", label: "Lua" },
  { id: "http", label: "HTTP" },
  { id: "dsl", label: "DSL" },
  { id: "run", label: "Run Security" },
];

const JS_LIBRARY_TEMPLATES: TemplateItem[] = [
  {
    label: "Greeting Helper",
    content:
      "function greet(name) {\n  return \"Hello, \" + name + \"!\";\n}\n",
  },
  {
    label: "Clipboard Cleaner",
    content:
      "function clipClean(str) {\n  if (!str) return \"\";\n  return str.replace(/\\s+/g, \" \").trim();\n}\n",
  },
];

const PY_LIBRARY_TEMPLATES: TemplateItem[] = [
  {
    label: "Greeting Function",
    content:
      "def py_greet(name: str) -> str:\n    return f\"Hello, {name}!\"\n",
  },
  {
    label: "Title Case Helper",
    content:
      "def title_case(value: str) -> str:\n    return (value or \"\").title()\n",
  },
];

const LUA_LIBRARY_TEMPLATES: TemplateItem[] = [
  {
    label: "Greeting Function",
    content:
      "function lua_greet(name)\n  return \"Hello, \" .. tostring(name) .. \"!\"\nend\n",
  },
  {
    label: "Trim Helper",
    content:
      "function trim(s)\n  return (s:gsub(\"^%s*(.-)%s*$\", \"%1\"))\nend\n",
  },
];

const EXPR_TEMPLATES: Record<"http" | "dsl" | "run", TemplateItem[]> = {
  http: [
    {
      label: "Weather Summary",
      content: "{weather:city=London|country=GB|state=England|format=summary}",
    },
    {
      label: "HTTP JSON Path",
      content: "{http:https://api.ipify.org?format=json|ip}",
    },
  ],
  dsl: [
    { label: "Simple Math", content: "{dsl:(2 + 3) * 4}" },
    { label: "Conditional", content: "{dsl:if(5 > 2, \"yes\", \"no\")}" },
  ],
  run: [
    { label: "Hostname", content: "{run:hostname}" },
    { label: "Date (PowerShell)", content: "{run:powershell -NoProfile -Command Get-Date}" },
  ],
};

export function ScriptTab({ appState }: ScriptTabProps) {
  const [status, setStatus] = useState("");
  const [activeSubTab, setActiveSubTab] = useState<
    "javascript" | "python" | "lua" | "http" | "dsl" | "run" | "diagnostics"
  >(() => {
    const saved = localStorage.getItem("digicore-script-subtab");
    const validTabs = [
      "javascript",
      "python",
      "lua",
      "http",
      "dsl",
      "run",
      "diagnostics",
    ];
    if (saved && validTabs.includes(saved)) {
      return saved as any;
    }
    return "javascript";
  });
  const [runDisabled, setRunDisabled] = useState(false);
  const [runAllowlist, setRunAllowlist] = useState("");
  const [jsContent, setJsContent] = useState("");
  const [pyContent, setPyContent] = useState("");
  const [luaContent, setLuaContent] = useState("");
  const [quickTestExpr, setQuickTestExpr] = useState("{js: greet('World')}");
  const [quickTestResult, setQuickTestResult] = useState("");
  const [quickTesting, setQuickTesting] = useState(false);
  const [statusHistory, setStatusHistory] = useState<string[]>([]);
  const [tofuAuditHistory, setTofuAuditHistory] = useState<string[]>([]);
  const [tofuAuditLoading, setTofuAuditLoading] = useState(false);
  const [httpTimeoutSecs, setHttpTimeoutSecs] = useState(5);
  const [httpRetryCount, setHttpRetryCount] = useState(3);
  const [httpRetryDelayMs, setHttpRetryDelayMs] = useState(500);
  const [httpUseAsync, setHttpUseAsync] = useState(false);
  const [dslEnabled, setDslEnabled] = useState(true);
  const [pyEnabled, setPyEnabled] = useState(false);
  const [pyPath, setPyPath] = useState("");
  const [pyLibraryPath, setPyLibraryPath] = useState("");
  const [luaEnabled, setLuaEnabled] = useState(false);
  const [luaPath, setLuaPath] = useState("");
  const [luaLibraryPath, setLuaLibraryPath] = useState("");
  const [selectedJsTemplate, setSelectedJsTemplate] = useState(JS_LIBRARY_TEMPLATES[0].label);
  const [selectedPyTemplate, setSelectedPyTemplate] = useState(PY_LIBRARY_TEMPLATES[0].label);
  const [selectedLuaTemplate, setSelectedLuaTemplate] = useState(LUA_LIBRARY_TEMPLATES[0].label);
  const [selectedExprTemplate, setSelectedExprTemplate] = useState(EXPR_TEMPLATES.http[0].label);
  const [engineProfileMode, setEngineProfileMode] = useState<"export" | "import">("export");
  const [engineProfileScope, setEngineProfileScope] = useState<"all" | "selected">("all");
  const [selectedEngineProfileGroups, setSelectedEngineProfileGroups] = useState<EngineProfileGroupId[]>(
    ENGINE_PROFILE_GROUP_OPTIONS.map((g) => g.id)
  );
  const [engineProfilePreview, setEngineProfilePreview] = useState<EngineProfilePreview | null>(null);
  const [engineProfileWarningsAcknowledged, setEngineProfileWarningsAcknowledged] = useState(false);
  const [engineProfileDryRun, setEngineProfileDryRun] = useState<EngineProfileDryRun | null>(null);
  const [allowUnknownSigners, setAllowUnknownSigners] = useState(true);
  const [trustOnFirstUse, setTrustOnFirstUse] = useState(false);
  const [trustedSignerListText, setTrustedSignerListText] = useState("");
  const [blockedSignerListText, setBlockedSignerListText] = useState("");

  const [savedFeedback, setSavedFeedback] = useState<string | null>(null);

  useEffect(() => {
    localStorage.setItem("digicore-script-subtab", activeSubTab);
  }, [activeSubTab]);

  const triggerSavedFeedback = (id: string) => {
    setSavedFeedback(id);
    setTimeout(() => setSavedFeedback(null), 3000);
  };

  useEffect(() => {
    if (appState) {
      setRunDisabled(!!appState.script_library_run_disabled);
      setRunAllowlist(appState.script_library_run_allowlist || "");
    }
  }, [appState]);

  const loadScriptTab = async () => {
    try {
      const state = await getTaurpc().get_app_state();
      setRunDisabled(!!state.script_library_run_disabled);
      setRunAllowlist(state.script_library_run_allowlist || "");
      const [js, py, lua] = await Promise.all([
        getTaurpc().get_script_library_js(),
        getTaurpc().get_script_library_py(),
        getTaurpc().get_script_library_lua(),
      ]);
      const scriptingCfg = await getTaurpc().get_scripting_engine_config();
      const signerRegistry = (await getTaurpc().get_scripting_signer_registry()) as EngineSignerRegistry;
      setJsContent(js || "");
      setPyContent(py || "");
      setLuaContent(lua || "");
      setHttpTimeoutSecs(scriptingCfg.http.timeout_secs || 5);
      setHttpRetryCount(scriptingCfg.http.retry_count || 3);
      setHttpRetryDelayMs(scriptingCfg.http.retry_delay_ms || 500);
      setHttpUseAsync(!!scriptingCfg.http.use_async);
      setDslEnabled(!!scriptingCfg.dsl.enabled);
      setPyEnabled(!!scriptingCfg.py.enabled);
      setPyPath(scriptingCfg.py.path || "");
      setPyLibraryPath(scriptingCfg.py.library_path || "");
      setLuaEnabled(!!scriptingCfg.lua.enabled);
      setLuaPath(scriptingCfg.lua.path || "");
      setLuaLibraryPath(scriptingCfg.lua.library_path || "");
      setAllowUnknownSigners(signerRegistry.allow_unknown_signers);
      setTrustOnFirstUse(!!signerRegistry.trust_on_first_use);
      setTrustedSignerListText((signerRegistry.trusted_fingerprints || []).join("\n"));
      setBlockedSignerListText((signerRegistry.blocked_fingerprints || []).join("\n"));
      pushStatus("Scripting libraries loaded.");
    } catch (e) {
      pushStatus("Error: " + String(e));
    }
  };

  useEffect(() => {
    loadScriptTab();
  }, []);

  const pushStatus = (message: string) => {
    const stamp = new Date().toLocaleTimeString();
    const line = `[${stamp}] ${message}`;
    setStatus(line);
    setStatusHistory((prev) => [line, ...prev].slice(0, 30));
  };

  const refreshTofuAuditHistory = async () => {
    try {
      setTofuAuditLoading(true);
      const entries = (await getTaurpc().get_diagnostic_logs()) as DiagnosticEntry[];
      const auditLines = entries
        .filter((e) => e.message.includes("[ScriptingSignerTOFU][AUDIT]"))
        .slice(0, 30)
        .map((e) => `[${new Date(e.timestamp_ms).toLocaleTimeString()}] ${e.message}`);
      setTofuAuditHistory(auditLines);
    } catch (e) {
      setTofuAuditHistory([`[Error] Failed to load TOFU audit events: ${String(e)}`]);
    } finally {
      setTofuAuditLoading(false);
    }
  };

  useEffect(() => {
    if (activeSubTab !== "diagnostics") return;
    refreshTofuAuditHistory();
    const id = setInterval(() => {
      refreshTofuAuditHistory();
    }, 4000);
    return () => clearInterval(id);
  }, [activeSubTab]);

  const handleSaveRun = async () => {
    if (!validation.canSave) {
      pushStatus("Fix validation errors before saving run settings.");
      return;
    }
    try {
      await getTaurpc().update_config({
        script_library_run_disabled: runDisabled,
        script_library_run_allowlist: runAllowlist,
      } as Parameters<ReturnType<typeof getTaurpc>["update_config"]>[0]);
      await getTaurpc().save_settings();
      pushStatus("Run settings saved.");
      triggerSavedFeedback("run");
    } catch (e) {
      pushStatus("Error: " + String(e));
    }
  };

  const handleSaveJs = async () => {
    if (!validation.canSave) {
      pushStatus("Fix validation errors before saving JavaScript library.");
      return;
    }
    try {
      await getTaurpc().save_script_library_js(jsContent);
      pushStatus("Global JavaScript library saved and hot-reloaded.");
      triggerSavedFeedback("js");
    } catch (e) {
      pushStatus("Error: " + String(e));
    }
  };

  const handleSavePy = async () => {
    if (!validation.canSave) {
      pushStatus("Fix validation errors before saving Python settings.");
      return;
    }
    try {
      await Promise.all([
        getTaurpc().save_script_library_py(pyContent),
        getTaurpc().save_scripting_engine_config({
          dsl: { enabled: dslEnabled },
          http: {
            timeout_secs: clampNum(httpTimeoutSecs, 1, 60),
            retry_count: clampNum(httpRetryCount, 0, 10),
            retry_delay_ms: clampNum(httpRetryDelayMs, 50, 20000),
            use_async: httpUseAsync,
          },
          py: {
            enabled: pyEnabled,
            path: pyPath,
            library_path: pyLibraryPath,
          },
          lua: {
            enabled: luaEnabled,
            path: luaPath,
            library_path: luaLibraryPath,
          },
        }),
      ]);
      pushStatus("Global Python library + Python engine settings saved.");
      triggerSavedFeedback("py");
    } catch (e) {
      pushStatus("Error: " + String(e));
    }
  };

  const handleSaveLua = async () => {
    if (!validation.canSave) {
      pushStatus("Fix validation errors before saving Lua settings.");
      return;
    }
    try {
      await Promise.all([
        getTaurpc().save_script_library_lua(luaContent),
        getTaurpc().save_scripting_engine_config({
          dsl: { enabled: dslEnabled },
          http: {
            timeout_secs: clampNum(httpTimeoutSecs, 1, 60),
            retry_count: clampNum(httpRetryCount, 0, 10),
            retry_delay_ms: clampNum(httpRetryDelayMs, 50, 20000),
            use_async: httpUseAsync,
          },
          py: {
            enabled: pyEnabled,
            path: pyPath,
            library_path: pyLibraryPath,
          },
          lua: {
            enabled: luaEnabled,
            path: luaPath,
            library_path: luaLibraryPath,
          },
        }),
      ]);
      pushStatus("Global Lua library + Lua engine settings saved.");
      triggerSavedFeedback("lua");
    } catch (e) {
      pushStatus("Error: " + String(e));
    }
  };

  const handleSaveAllLibraries = async () => {
    if (!validation.canSave) {
      pushStatus("Fix validation errors before saving all libraries/settings.");
      return;
    }
    try {
      await Promise.all([
        getTaurpc().save_script_library_js(jsContent),
        getTaurpc().save_script_library_py(pyContent),
        getTaurpc().save_script_library_lua(luaContent),
        getTaurpc().save_scripting_engine_config({
          dsl: { enabled: dslEnabled },
          http: {
            timeout_secs: clampNum(httpTimeoutSecs, 1, 60),
            retry_count: clampNum(httpRetryCount, 0, 10),
            retry_delay_ms: clampNum(httpRetryDelayMs, 50, 20000),
            use_async: httpUseAsync,
          },
          py: {
            enabled: pyEnabled,
            path: pyPath,
            library_path: pyLibraryPath,
          },
          lua: {
            enabled: luaEnabled,
            path: luaPath,
            library_path: luaLibraryPath,
          },
        }),
      ]);
      pushStatus("All script libraries and engine settings saved.");
      triggerSavedFeedback("all");
    } catch (e) {
      pushStatus("Error: " + String(e));
    }
  };

  const handleSaveHttpConfig = async () => {
    if (!validation.canSave) {
      pushStatus("Fix validation errors before saving HTTP settings.");
      return;
    }
    try {
      await getTaurpc().save_scripting_engine_config({
        dsl: { enabled: dslEnabled },
        http: {
          timeout_secs: clampNum(httpTimeoutSecs, 1, 60),
          retry_count: clampNum(httpRetryCount, 0, 10),
          retry_delay_ms: clampNum(httpRetryDelayMs, 50, 20000),
          use_async: httpUseAsync,
        },
        py: {
          enabled: pyEnabled,
          path: pyPath,
          library_path: pyLibraryPath,
        },
        lua: {
          enabled: luaEnabled,
          path: luaPath,
          library_path: luaLibraryPath,
        },
      });
      pushStatus("HTTP/Weather engine settings saved.");
      triggerSavedFeedback("http");
    } catch (e) {
      pushStatus("Error: " + String(e));
    }
  };

  const handleSaveDslConfig = async () => {
    if (!validation.canSave) {
      pushStatus("Fix validation errors before saving DSL settings.");
      return;
    }
    try {
      await getTaurpc().save_scripting_engine_config({
        dsl: { enabled: dslEnabled },
        http: {
          timeout_secs: clampNum(httpTimeoutSecs, 1, 60),
          retry_count: clampNum(httpRetryCount, 0, 10),
          retry_delay_ms: clampNum(httpRetryDelayMs, 50, 20000),
          use_async: httpUseAsync,
        },
        py: {
          enabled: pyEnabled,
          path: pyPath,
          library_path: pyLibraryPath,
        },
        lua: {
          enabled: luaEnabled,
          path: luaPath,
          library_path: luaLibraryPath,
        },
      });
      pushStatus("DSL engine setting saved.");
      triggerSavedFeedback("dsl");
    } catch (e) {
      pushStatus("Error: " + String(e));
    }
  };

  const runQuickTest = async () => {
    const expr = quickTestExpr.trim();
    if (!expr) {
      pushStatus("Enter a quick test expression first.");
      return;
    }
    setQuickTesting(true);
    try {
      const out = await getTaurpc().test_snippet_logic(expr, null);
      setQuickTestResult(out.result || "");
      pushStatus("Quick test completed.");
    } catch (e) {
      setQuickTestResult("");
      pushStatus("Quick test failed: " + String(e));
    } finally {
      setQuickTesting(false);
    }
  };

  const libraryStats = useMemo(
    () => ({
      jsLines: jsContent.split("\n").length,
      pyLines: pyContent.split("\n").length,
      luaLines: luaContent.split("\n").length,
    }),
    [jsContent, pyContent, luaContent]
  );

  const clampNum = (value: number, min: number, max: number) =>
    Number.isFinite(value) ? Math.min(max, Math.max(min, value)) : min;

  const normalizeCodeText = (value: string) =>
    value
      .replace(/\r\n/g, "\n")
      .split("\n")
      .map((line) => line.replace(/\s+$/g, ""))
      .join("\n")
      .replace(/\n{3,}/g, "\n\n")
      .trimEnd()
      .concat("\n");

  const applyCodeFormat = (engine: "js" | "py" | "lua") => {
    if (engine === "js") {
      setJsContent(normalizeCodeText(jsContent));
      pushStatus("JavaScript library formatted.");
    } else if (engine === "py") {
      setPyContent(normalizeCodeText(pyContent));
      pushStatus("Python library formatted.");
    } else {
      setLuaContent(normalizeCodeText(luaContent));
      pushStatus("Lua library formatted.");
    }
  };

  const insertLibraryTemplate = (engine: "js" | "py" | "lua") => {
    if (engine === "js") {
      const tpl = JS_LIBRARY_TEMPLATES.find((t) => t.label === selectedJsTemplate);
      if (!tpl) return;
      setJsContent((prev) => {
        const base = prev.trim() ? `${normalizeCodeText(prev)}\n` : "";
        return `${base}${tpl.content}`;
      });
      pushStatus(`Inserted JavaScript template: ${tpl.label}`);
      return;
    }
    if (engine === "py") {
      const tpl = PY_LIBRARY_TEMPLATES.find((t) => t.label === selectedPyTemplate);
      if (!tpl) return;
      setPyContent((prev) => {
        const base = prev.trim() ? `${normalizeCodeText(prev)}\n` : "";
        return `${base}${tpl.content}`;
      });
      pushStatus(`Inserted Python template: ${tpl.label}`);
      return;
    }
    const tpl = LUA_LIBRARY_TEMPLATES.find((t) => t.label === selectedLuaTemplate);
    if (!tpl) return;
    setLuaContent((prev) => {
      const base = prev.trim() ? `${normalizeCodeText(prev)}\n` : "";
      return `${base}${tpl.content}`;
    });
    pushStatus(`Inserted Lua template: ${tpl.label}`);
  };

  const insertExprTemplate = (group: "http" | "dsl" | "run") => {
    const tpl = EXPR_TEMPLATES[group].find((t) => t.label === selectedExprTemplate);
    if (!tpl) return;
    setQuickTestExpr(tpl.content);
    pushStatus(`Inserted ${group.toUpperCase()} template: ${tpl.label}`);
  };

  const getTargetEngineProfileGroups = (): EngineProfileGroupId[] => {
    if (engineProfileScope === "all") {
      return ENGINE_PROFILE_GROUP_OPTIONS.map((g) => g.id);
    }
    return selectedEngineProfileGroups;
  };

  const toggleEngineProfileGroup = (groupId: EngineProfileGroupId, checked: boolean) => {
    setSelectedEngineProfileGroups((prev) => {
      if (checked) {
        return prev.includes(groupId) ? prev : [...prev, groupId];
      }
      return prev.filter((g) => g !== groupId);
    });
  };

  const handleExportEngineProfile = async () => {
    const groups = getTargetEngineProfileGroups();
    if (groups.length === 0) {
      pushStatus("Validation: choose at least one engine group to export.");
      return;
    }
    const path = await save({
      title: "Export Scripting Engine Profile",
      defaultPath: "digicore_scripting_engine_profile.json",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path) {
      pushStatus("Engine profile export cancelled.");
      return;
    }
    try {
      setEngineProfileDryRun(null);
      const count = await getTaurpc().export_scripting_profile_to_file(path, groups);
      pushStatus(
        `Exported scripting profile with ${count} group${count === 1 ? "" : "s"} to ${String(path)}`
      );
    } catch (e) {
      pushStatus("Engine profile export failed: " + String(e));
    }
  };

  const handlePreviewImportEngineProfile = async () => {
    const pathSelection = await open({
      title: "Preview Scripting Engine Profile",
      multiple: false,
      directory: false,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    const path = Array.isArray(pathSelection) ? pathSelection[0] : pathSelection;
    if (!path) {
      pushStatus("Engine profile preview cancelled.");
      return;
    }
    try {
      const preview = (await getTaurpc().preview_scripting_profile_from_file(
        path
      )) as EngineProfilePreview;
      setEngineProfilePreview(preview);
      setEngineProfileDryRun(null);
      setEngineProfileWarningsAcknowledged(false);
      pushStatus(
        `Engine profile preview loaded: schema ${preview.schema_version}, groups ${preview.available_groups.length}, warnings ${preview.warnings.length}, signed=${preview.signed_bundle}, signature_valid=${preview.signature_valid}.`
      );
    } catch (e) {
      setEngineProfilePreview(null);
      setEngineProfileWarningsAcknowledged(false);
      pushStatus("Engine profile preview failed: " + String(e));
    }
  };

  const handleApplyImportEngineProfile = async () => {
    const groups = getTargetEngineProfileGroups();
    if (groups.length === 0) {
      pushStatus("Validation: choose at least one engine group to import.");
      return;
    }
    if (!engineProfilePreview?.path) {
      pushStatus("Validation: preview an engine profile file first.");
      return;
    }
    if (!engineProfilePreview.valid) {
      pushStatus("Validation: cannot import from an invalid engine profile preview.");
      return;
    }
    if (
      engineProfilePreview.warnings.length > 0 &&
      !engineProfileWarningsAcknowledged
    ) {
      pushStatus("Validation: acknowledge engine profile preview warnings first.");
      return;
    }
    try {
      const result = (await getTaurpc().import_scripting_profile_from_file(
        engineProfilePreview.path,
        groups
      )) as EngineProfileImportResult;
      await loadScriptTab();
      setEngineProfilePreview(null);
      setEngineProfileDryRun(null);
      setEngineProfileWarningsAcknowledged(false);
      pushStatus(
        `Engine profile import complete: applied ${result.applied_groups.length} group${result.applied_groups.length === 1 ? "" : "s"
        }, warnings ${result.warnings.length}, schema=${result.schema_version_used}, signature_valid=${result.signature_valid}.`
      );
    } catch (e) {
      pushStatus("Engine profile import failed: " + String(e));
    }
  };

  const handleDryRunEngineProfile = async () => {
    const groups = getTargetEngineProfileGroups();
    if (groups.length === 0) {
      pushStatus("Validation: choose at least one engine group for dry-run.");
      return;
    }
    if (!engineProfilePreview?.path) {
      pushStatus("Validation: preview an engine profile file first.");
      return;
    }
    try {
      const dryRun = (await getTaurpc().dry_run_import_scripting_profile_from_file(
        engineProfilePreview.path,
        groups
      )) as EngineProfileDryRun;
      setEngineProfileDryRun(dryRun);
      pushStatus(
        `Dry-run completed: ${dryRun.changed_groups.length} group(s) changed, ${dryRun.estimated_updates} field update(s) estimated.`
      );
    } catch (e) {
      setEngineProfileDryRun(null);
      pushStatus("Engine profile dry-run failed: " + String(e));
    }
  };

  const parseSignerList = (raw: string): string[] =>
    raw
      .split(/\r?\n/g)
      .map((line) => line.trim().toLowerCase().replace(/[^a-f0-9]/g, ""))
      .filter((line) => line.length > 0);

  const handleSaveSignerRegistry = async () => {
    try {
      await getTaurpc().save_scripting_signer_registry({
        allow_unknown_signers: allowUnknownSigners,
        trust_on_first_use: trustOnFirstUse,
        trusted_fingerprints: parseSignerList(trustedSignerListText),
        blocked_fingerprints: parseSignerList(blockedSignerListText),
      } as EngineSignerRegistry);
      pushStatus("Signer registry saved.");
      triggerSavedFeedback("signer");
    } catch (e) {
      pushStatus("Signer registry save failed: " + String(e));
    }
  };

  const handleTrustPreviewSigner = async () => {
    const fp = engineProfilePreview?.signer_fingerprint?.trim().toLowerCase();
    if (!fp) {
      pushStatus("No preview signer fingerprint available to trust.");
      return;
    }
    const current = new Set(parseSignerList(trustedSignerListText));
    current.add(fp.replace(/[^a-f0-9]/g, ""));
    setTrustedSignerListText(Array.from(current).join("\n"));
    pushStatus("Added preview signer fingerprint to trusted list (save to persist).");
  };

  const handleBlockPreviewSigner = async () => {
    const fp = engineProfilePreview?.signer_fingerprint?.trim().toLowerCase();
    if (!fp) {
      pushStatus("No preview signer fingerprint available to block.");
      return;
    }
    const current = new Set(parseSignerList(blockedSignerListText));
    current.add(fp.replace(/[^a-f0-9]/g, ""));
    setBlockedSignerListText(Array.from(current).join("\n"));
    pushStatus("Added preview signer fingerprint to blocked list (save to persist).");
  };

  const handleCopySignerFingerprint = async () => {
    const fp = engineProfilePreview?.signer_fingerprint || engineProfileDryRun?.signer_fingerprint;
    if (!fp) {
      pushStatus("No signer fingerprint available to copy.");
      return;
    }
    try {
      await getTaurpc().copy_to_clipboard(fp);
      pushStatus("Signer fingerprint copied to clipboard.");
    } catch (e) {
      pushStatus("Failed to copy signer fingerprint: " + String(e));
    }
  };

  const handleExportWithDetachedSignature = async () => {
    const groups = getTargetEngineProfileGroups();
    if (groups.length === 0) {
      pushStatus("Validation: choose at least one engine group to export.");
      return;
    }
    const path = await save({
      title: "Export Signed Scripting Profile + Detached Signature",
      defaultPath: "digicore_scripting_engine_profile.json",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path) {
      pushStatus("Detached-signature export cancelled.");
      return;
    }
    try {
      const out = (await getTaurpc().export_scripting_profile_with_detached_signature_to_file(
        path,
        groups
      )) as DetachedSignatureExportResult;
      pushStatus(
        `Exported signed profile + detached signature. key_id=${out.key_id}, profile=${out.profile_path}, signature=${out.signature_path}`
      );
    } catch (e) {
      pushStatus("Detached-signature export failed: " + String(e));
    }
  };

  const validation = useMemo(() => {
    const errors: string[] = [];
    const warnings: string[] = [];

    if (httpTimeoutSecs < 1 || httpTimeoutSecs > 60) {
      errors.push("HTTP timeout must be between 1 and 60 seconds.");
    }
    if (httpRetryCount < 0 || httpRetryCount > 10) {
      errors.push("HTTP retry count must be between 0 and 10.");
    }
    if (httpRetryDelayMs < 50 || httpRetryDelayMs > 20000) {
      errors.push("HTTP retry delay must be between 50 and 20000 ms.");
    }
    if (pyLibraryPath.trim() && !pyLibraryPath.trim().toLowerCase().endsWith(".py")) {
      errors.push("Python library path should end with .py");
    }
    if (luaLibraryPath.trim() && !luaLibraryPath.trim().toLowerCase().endsWith(".lua")) {
      errors.push("Lua library path should end with .lua");
    }
    if (!runDisabled && !runAllowlist.trim()) {
      warnings.push("Run is enabled without allowlist; this is unsafe.");
    }
    if (pyEnabled && !pyPath.trim()) {
      warnings.push("Python enabled with default executable lookup (python in PATH).");
    }
    if (luaEnabled && !luaPath.trim()) {
      warnings.push("Lua enabled with default executable lookup (lua in PATH).");
    }
    if (jsContent.trim().length === 0) {
      warnings.push("JavaScript global library is empty.");
    }

    return { errors, warnings, canSave: errors.length === 0 };
  }, [
    httpRetryCount,
    httpRetryDelayMs,
    httpTimeoutSecs,
    jsContent,
    luaEnabled,
    luaLibraryPath,
    luaPath,
    pyEnabled,
    pyLibraryPath,
    pyPath,
    runAllowlist,
    runDisabled,
  ]);

  const subTabs: Array<{
    id: "javascript" | "python" | "lua" | "http" | "dsl" | "run" | "diagnostics";
    label: string;
  }> = [
      { id: "javascript", label: "JavaScript" },
      { id: "python", label: "Python" },
      { id: "lua", label: "Lua" },
      { id: "http", label: "HTTP/Weather" },
      { id: "dsl", label: "DSL" },
      { id: "run", label: "Run Security" },
      { id: "diagnostics", label: "Diagnostics" },
    ];

  return (
    <div className="p-4 border border-[var(--dc-border)] rounded mt-2">
      <div className="mb-4 flex items-center justify-between gap-2">
        <h2 className="text-xl font-semibold">Scripting Engine Library</h2>
        <button
          onClick={handleSaveAllLibraries}
          className="px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
          disabled={!validation.canSave}
        >
          Save All Libraries
          {savedFeedback === "all" && (
            <span className="text-green-300 text-sm ml-2 font-medium">Saved!</span>
          )}
        </button>
      </div>
      <p className="text-sm text-[var(--dc-text-muted)] mb-2">{status}</p>
      <p className="text-xs text-[var(--dc-text-muted)] mb-4">
        Engines available: JS, Python, Lua, HTTP, DSL, Run, Weather.
      </p>
      {validation.errors.length > 0 && (
        <div className="mb-3 rounded border border-red-500/50 bg-red-500/10 p-2 text-sm text-red-400">
          {validation.errors.map((e) => (
            <div key={e}>- {e}</div>
          ))}
        </div>
      )}
      {validation.warnings.length > 0 && (
        <div className="mb-3 rounded border border-amber-500/50 bg-amber-500/10 p-2 text-sm text-amber-300">
          {validation.warnings.map((w) => (
            <div key={w}>- {w}</div>
          ))}
        </div>
      )}

      <div className="mb-4 flex flex-wrap gap-2">
        {subTabs.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveSubTab(tab.id)}
            className={`px-3 py-1.5 rounded border border-[var(--dc-border)] ${activeSubTab === tab.id
              ? "bg-[var(--dc-accent)] text-white"
              : "bg-[var(--dc-bg)] text-[var(--dc-text)]"
              }`}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {activeSubTab === "javascript" && (
        <div className="my-3 border border-[var(--dc-border)] rounded p-3">
          <h3 className="font-semibold mb-2">Global JavaScript Library</h3>
          <p className="text-sm text-[var(--dc-text-muted)]">
            Reusable functions for all {"{js:...}"} tags.
          </p>
          <div className="mt-2 flex flex-wrap gap-2">
            <select
              value={selectedJsTemplate}
              onChange={(e) => setSelectedJsTemplate(e.target.value)}
              className="px-2 py-1 text-sm bg-[var(--dc-bg)] border border-[var(--dc-border)] rounded"
            >
              {JS_LIBRARY_TEMPLATES.map((t) => (
                <option key={t.label} value={t.label}>
                  {t.label}
                </option>
              ))}
            </select>
            <button
              onClick={() => insertLibraryTemplate("js")}
              className="px-3 py-1.5 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
            >
              Insert Template
            </button>
            <button
              onClick={() => applyCodeFormat("js")}
              className="px-3 py-1.5 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
            >
              Format JS
            </button>
          </div>
          <div className="mt-2">
            <ScriptCodeEditor
              value={jsContent}
              onChange={setJsContent}
              language="javascript"
              minHeight="320px"
            />
          </div>
          <div className="mt-2 flex items-center justify-between gap-2">
            <span className="text-xs text-[var(--dc-text-muted)]">
              {libraryStats.jsLines} line(s)
            </span>
            <button
              onClick={handleSaveJs}
              className="px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
              disabled={!validation.canSave}
            >
              Save & Reload JS
              {savedFeedback === "js" && (
                <span className="text-green-500 text-sm ml-2 font-medium animate-pulse">Saved!</span>
              )}
            </button>
          </div>
          <pre className="mt-2 text-xs whitespace-pre-wrap bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded p-2">
            {`Syntax Guide:
Use {js: greet("World")}
Global library functions become available in all snippets.`}
          </pre>
        </div>
      )}

      {activeSubTab === "python" && (
        <div className="my-3 border border-[var(--dc-border)] rounded p-3">
          <h3 className="font-semibold mb-2">Global Python Library</h3>
          <p className="text-sm text-[var(--dc-text-muted)]">
            Shared helpers prepended for all {"{py:...}"} expressions.
          </p>
          <div className="mt-2 grid grid-cols-1 md:grid-cols-2 gap-2">
            <label className="text-sm flex items-center gap-2">
              <input
                type="checkbox"
                checked={pyEnabled}
                onChange={(e) => setPyEnabled(e.target.checked)}
              />
              Python enabled
            </label>
            <input
              value={pyPath}
              onChange={(e) => setPyPath(e.target.value)}
              placeholder="Python executable path (blank = python)"
              className="w-full p-2 text-sm bg-[var(--dc-bg)] text-[var(--dc-text)] border border-[var(--dc-border)] rounded"
            />
            <input
              value={pyLibraryPath}
              onChange={(e) => setPyLibraryPath(e.target.value)}
              placeholder="Python global library path"
              className="w-full p-2 text-sm bg-[var(--dc-bg)] text-[var(--dc-text)] border border-[var(--dc-border)] rounded md:col-span-2"
            />
          </div>
          <div className="mt-2 flex flex-wrap gap-2">
            <select
              value={selectedPyTemplate}
              onChange={(e) => setSelectedPyTemplate(e.target.value)}
              className="px-2 py-1 text-sm bg-[var(--dc-bg)] border border-[var(--dc-border)] rounded"
            >
              {PY_LIBRARY_TEMPLATES.map((t) => (
                <option key={t.label} value={t.label}>
                  {t.label}
                </option>
              ))}
            </select>
            <button
              onClick={() => insertLibraryTemplate("py")}
              className="px-3 py-1.5 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
            >
              Insert Template
            </button>
            <button
              onClick={() => applyCodeFormat("py")}
              className="px-3 py-1.5 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
            >
              Format Python
            </button>
          </div>
          <div className="mt-2">
            <ScriptCodeEditor
              value={pyContent}
              onChange={setPyContent}
              language="python"
              minHeight="320px"
            />
          </div>
          <div className="mt-2 flex items-center justify-between gap-2">
            <span className="text-xs text-[var(--dc-text-muted)]">
              {libraryStats.pyLines} line(s)
            </span>
            <button
              onClick={handleSavePy}
              className="px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
              disabled={!validation.canSave}
            >
              Save Python Library
              {savedFeedback === "py" && (
                <span className="text-green-500 text-sm ml-2 font-medium animate-pulse">Saved!</span>
              )}
            </button>
          </div>
          <pre className="mt-2 text-xs whitespace-pre-wrap bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded p-2">
            {`Syntax Guide:
Use {py: py_greet("World")}
Library is prepended before expression/script execution.`}
          </pre>
        </div>
      )}

      {activeSubTab === "lua" && (
        <div className="my-3 border border-[var(--dc-border)] rounded p-3">
          <h3 className="font-semibold mb-2">Global Lua Library</h3>
          <p className="text-sm text-[var(--dc-text-muted)]">
            Shared helpers prepended for all {"{lua:...}"} scripts.
          </p>
          <div className="mt-2 grid grid-cols-1 md:grid-cols-2 gap-2">
            <label className="text-sm flex items-center gap-2">
              <input
                type="checkbox"
                checked={luaEnabled}
                onChange={(e) => setLuaEnabled(e.target.checked)}
              />
              Lua enabled
            </label>
            <input
              value={luaPath}
              onChange={(e) => setLuaPath(e.target.value)}
              placeholder="Lua executable path (blank = lua)"
              className="w-full p-2 text-sm bg-[var(--dc-bg)] text-[var(--dc-text)] border border-[var(--dc-border)] rounded"
            />
            <input
              value={luaLibraryPath}
              onChange={(e) => setLuaLibraryPath(e.target.value)}
              placeholder="Lua global library path"
              className="w-full p-2 text-sm bg-[var(--dc-bg)] text-[var(--dc-text)] border border-[var(--dc-border)] rounded md:col-span-2"
            />
          </div>
          <div className="mt-2 flex flex-wrap gap-2">
            <select
              value={selectedLuaTemplate}
              onChange={(e) => setSelectedLuaTemplate(e.target.value)}
              className="px-2 py-1 text-sm bg-[var(--dc-bg)] border border-[var(--dc-border)] rounded"
            >
              {LUA_LIBRARY_TEMPLATES.map((t) => (
                <option key={t.label} value={t.label}>
                  {t.label}
                </option>
              ))}
            </select>
            <button
              onClick={() => insertLibraryTemplate("lua")}
              className="px-3 py-1.5 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
            >
              Insert Template
            </button>
            <button
              onClick={() => applyCodeFormat("lua")}
              className="px-3 py-1.5 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
            >
              Format Lua
            </button>
          </div>
          <div className="mt-2">
            <ScriptCodeEditor
              value={luaContent}
              onChange={setLuaContent}
              language="lua"
              minHeight="320px"
            />
          </div>
          <div className="mt-2 flex items-center justify-between gap-2">
            <span className="text-xs text-[var(--dc-text-muted)]">
              {libraryStats.luaLines} line(s)
            </span>
            <button
              onClick={handleSaveLua}
              className="px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
              disabled={!validation.canSave}
            >
              Save Lua Library
              {savedFeedback === "lua" && (
                <span className="text-green-500 text-sm ml-2 font-medium animate-pulse">Saved!</span>
              )}
            </button>
          </div>
          <pre className="mt-2 text-xs whitespace-pre-wrap bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded p-2">
            {`Syntax Guide:
Use {lua: lua_greet("World")}
Library code is prepended before snippet Lua code.`}
          </pre>
        </div>
      )}

      {activeSubTab === "http" && (
        <div className="my-3 border border-[var(--dc-border)] rounded p-3">
          <h3 className="font-semibold mb-2">HTTP / Weather Quick Validation</h3>
          <p className="text-sm text-[var(--dc-text-muted)]">
            Validate HTTP and weather placeholders quickly. Example:
            {" {weather:city=London|country=GB|state=England|format=summary}"}
          </p>
          <div className="mt-2 grid grid-cols-1 md:grid-cols-2 gap-2">
            <label className="text-sm">
              Timeout (secs)
              <input
                type="number"
                value={httpTimeoutSecs}
                min={1}
                max={60}
                onChange={(e) => setHttpTimeoutSecs(Number(e.target.value) || 5)}
                className="w-full mt-1 p-2 text-sm bg-[var(--dc-bg)] text-[var(--dc-text)] border border-[var(--dc-border)] rounded"
              />
            </label>
            <label className="text-sm">
              Retry count
              <input
                type="number"
                value={httpRetryCount}
                min={0}
                max={10}
                onChange={(e) => setHttpRetryCount(Number(e.target.value) || 0)}
                className="w-full mt-1 p-2 text-sm bg-[var(--dc-bg)] text-[var(--dc-text)] border border-[var(--dc-border)] rounded"
              />
            </label>
            <label className="text-sm">
              Retry delay (ms)
              <input
                type="number"
                value={httpRetryDelayMs}
                min={50}
                max={20000}
                onChange={(e) => setHttpRetryDelayMs(Number(e.target.value) || 500)}
                className="w-full mt-1 p-2 text-sm bg-[var(--dc-bg)] text-[var(--dc-text)] border border-[var(--dc-border)] rounded"
              />
            </label>
            <label className="text-sm flex items-end gap-2">
              <input
                type="checkbox"
                checked={httpUseAsync}
                onChange={(e) => setHttpUseAsync(e.target.checked)}
              />
              Use async HTTP fetcher
            </label>
          </div>
          <div className="mt-2">
            <ScriptCodeEditor
              value={quickTestExpr}
              onChange={setQuickTestExpr}
              language="http"
              minHeight="160px"
            />
          </div>
          <div className="mt-2 flex gap-2">
            <select
              value={selectedExprTemplate}
              onChange={(e) => setSelectedExprTemplate(e.target.value)}
              className="px-2 py-1 text-sm bg-[var(--dc-bg)] border border-[var(--dc-border)] rounded"
            >
              {EXPR_TEMPLATES.http.map((t) => (
                <option key={t.label} value={t.label}>
                  {t.label}
                </option>
              ))}
            </select>
            <button
              onClick={() => insertExprTemplate("http")}
              className="px-3 py-1.5 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
            >
              Insert HTTP/Weather Template
            </button>
            <button
              onClick={runQuickTest}
              className="px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
              disabled={quickTesting}
            >
              {quickTesting ? "Testing..." : "Run Quick Test"}
            </button>
            <button
              onClick={handleSaveHttpConfig}
              className="px-3 py-1.5 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
              disabled={!validation.canSave}
            >
              Save HTTP/Weather Settings
              {savedFeedback === "http" && (
                <span className="text-green-500 text-sm ml-2 font-medium animate-pulse">Saved!</span>
              )}
            </button>
          </div>
          {!!quickTestResult && (
            <pre className="mt-2 whitespace-pre-wrap text-sm bg-[var(--dc-bg)] border border-[var(--dc-border)] rounded p-2">
              {quickTestResult}
            </pre>
          )}
          <pre className="mt-2 text-xs whitespace-pre-wrap bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded p-2">
            {`Syntax Guide:
HTTP: {http:https://api.example.com/data|path.to.value}
Weather: {weather:city=Tokyo|country=JP|state=Tokyo|format=summary}`}
          </pre>
        </div>
      )}

      {activeSubTab === "dsl" && (
        <div className="my-3 border border-[var(--dc-border)] rounded p-3">
          <h3 className="font-semibold mb-2">DSL Engine</h3>
          <p className="text-sm text-[var(--dc-text-muted)]">
            Toggle the DSL evaluator used by {"{dsl:...}"} expressions.
          </p>
          <label className="mt-2 text-sm flex items-center gap-2">
            <input
              type="checkbox"
              checked={dslEnabled}
              onChange={(e) => setDslEnabled(e.target.checked)}
            />
            DSL enabled
          </label>
          <div className="mt-2 flex gap-2">
            <select
              value={selectedExprTemplate}
              onChange={(e) => setSelectedExprTemplate(e.target.value)}
              className="px-2 py-1 text-sm bg-[var(--dc-bg)] border border-[var(--dc-border)] rounded"
            >
              {EXPR_TEMPLATES.dsl.map((t) => (
                <option key={t.label} value={t.label}>
                  {t.label}
                </option>
              ))}
            </select>
            <button
              onClick={() => insertExprTemplate("dsl")}
              className="px-3 py-1.5 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
            >
              Insert DSL Template
            </button>
          </div>
          <button
            onClick={handleSaveDslConfig}
            className="mt-2 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
            disabled={!validation.canSave}
          >
            Save DSL Setting
            {savedFeedback === "dsl" && (
              <span className="text-green-500 text-sm ml-2 font-medium animate-pulse">Saved!</span>
            )}
          </button>
          <pre className="mt-2 text-xs whitespace-pre-wrap bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded p-2">
            {`Syntax Guide:
Use {dsl: expression}
Example: {dsl:(10 + 5) * 2}`}
          </pre>
        </div>
      )}

      {activeSubTab === "run" && (
        <div className="my-3 border border-[var(--dc-border)] rounded p-3">
          <h3 className="font-semibold mb-2">{"{run:}"} Security</h3>
          <label className="block mt-2 flex items-center gap-2">
            <input
              type="checkbox"
              checked={runDisabled}
              onChange={(e) => setRunDisabled(e.target.checked)}
            />
            Disable {"{run:command}"} (recommended)
          </label>
          <label className="block mt-2">Allowlist (when enabled):</label>
          <textarea
            value={runAllowlist}
            onChange={(e) => setRunAllowlist(e.target.value)}
            rows={3}
            placeholder="Comma-separated: python, cmd, C:\Scripts\, etc. Empty = block all."
            className="w-full p-1 bg-[var(--dc-bg)] text-[var(--dc-text)] border border-[var(--dc-border)] rounded mt-1"
          />
          <button
            onClick={handleSaveRun}
            className="mt-2 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
            disabled={!validation.canSave}
          >
            Save Run Settings
            {savedFeedback === "run" && (
              <span className="text-green-500 text-sm ml-2 font-medium animate-pulse">Saved!</span>
            )}
          </button>
          <div className="mt-2 flex gap-2">
            <select
              value={selectedExprTemplate}
              onChange={(e) => setSelectedExprTemplate(e.target.value)}
              className="px-2 py-1 text-sm bg-[var(--dc-bg)] border border-[var(--dc-border)] rounded"
            >
              {EXPR_TEMPLATES.run.map((t) => (
                <option key={t.label} value={t.label}>
                  {t.label}
                </option>
              ))}
            </select>
            <button
              onClick={() => insertExprTemplate("run")}
              className="px-3 py-1.5 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
            >
              Insert Run Template
            </button>
          </div>
          <pre className="mt-2 text-xs whitespace-pre-wrap bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded p-2">
            {`Syntax Guide:
Use {run: command}
Always prefer allowlisted commands only.`}
          </pre>
        </div>
      )}

      <div className="my-3 border border-[var(--dc-border)] rounded p-3">
        <h3 className="font-semibold mb-2">Import/Export Engine Profiles</h3>
        <p className="text-sm text-[var(--dc-text-muted)]">
          Export or import scripting engine profiles per group or all groups for team sharing and backups.
        </p>

        <label className="block mt-2 font-medium">Mode:</label>
        <div className="mt-1 flex items-center gap-4">
          <label className="flex items-center gap-2">
            <input
              type="radio"
              name="engine-profile-mode"
              checked={engineProfileMode === "export"}
              onChange={() => {
                setEngineProfileMode("export");
                setEngineProfilePreview(null);
                setEngineProfileDryRun(null);
                setEngineProfileWarningsAcknowledged(false);
              }}
            />
            Export
          </label>
          <label className="flex items-center gap-2">
            <input
              type="radio"
              name="engine-profile-mode"
              checked={engineProfileMode === "import"}
              onChange={() => {
                setEngineProfileMode("import");
                setEngineProfileDryRun(null);
              }}
            />
            Import
          </label>
        </div>

        <label className="block mt-3 font-medium">Scope:</label>
        <div className="mt-1 flex items-center gap-4">
          <label className="flex items-center gap-2">
            <input
              type="radio"
              name="engine-profile-scope"
              checked={engineProfileScope === "all"}
              onChange={() => setEngineProfileScope("all")}
            />
            All Groups
          </label>
          <label className="flex items-center gap-2">
            <input
              type="radio"
              name="engine-profile-scope"
              checked={engineProfileScope === "selected"}
              onChange={() => setEngineProfileScope("selected")}
            />
            Selected Groups
          </label>
        </div>

        <div className="mt-3 grid grid-cols-1 md:grid-cols-2 gap-2">
          {ENGINE_PROFILE_GROUP_OPTIONS.map((group) => (
            <label key={group.id} className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={selectedEngineProfileGroups.includes(group.id)}
                disabled={engineProfileScope === "all"}
                onChange={(e) => toggleEngineProfileGroup(group.id, e.target.checked)}
              />
              {group.label}
            </label>
          ))}
        </div>

        <div className="mt-3 flex gap-2">
          <button
            type="button"
            onClick={handleExportEngineProfile}
            className="px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
            disabled={engineProfileMode !== "export"}
          >
            Export Engine Profile JSON
          </button>
          <button
            type="button"
            onClick={handleExportWithDetachedSignature}
            className="px-3 py-1.5 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
            disabled={engineProfileMode !== "export"}
          >
            Export Signed + Detached Signature
          </button>
          <button
            type="button"
            onClick={handlePreviewImportEngineProfile}
            className="px-3 py-1.5 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
            disabled={engineProfileMode !== "import"}
          >
            Select Import File (Preview)
          </button>
          <button
            type="button"
            onClick={handleDryRunEngineProfile}
            className="px-3 py-1.5 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
            disabled={engineProfileMode !== "import" || !engineProfilePreview}
          >
            Run Dry-Run Diff
          </button>
          <button
            type="button"
            onClick={handleApplyImportEngineProfile}
            className="px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
            disabled={
              engineProfileMode !== "import" ||
              !engineProfilePreview ||
              !engineProfilePreview.valid ||
              (engineProfilePreview.warnings.length > 0 &&
                !engineProfileWarningsAcknowledged)
            }
          >
            Apply Import from Preview
          </button>
        </div>

        {engineProfileMode === "import" && engineProfilePreview && (
          <div className="mt-3 p-2 border border-[var(--dc-border)] rounded text-sm">
            <p>
              <strong>Preview File:</strong> {engineProfilePreview.path}
            </p>
            <p>
              <strong>Schema:</strong> {engineProfilePreview.schema_version} (
              {engineProfilePreview.valid ? "valid" : "invalid"})
            </p>
            <p>
              <strong>Bundle Signed:</strong>{" "}
              {engineProfilePreview.signed_bundle ? "yes" : "no"}
            </p>
            <p>
              <strong>Signature Valid:</strong>{" "}
              {engineProfilePreview.signature_valid ? "yes" : "no"}
            </p>
            <p>
              <strong>Signature Key ID:</strong>{" "}
              {engineProfilePreview.signature_key_id || "(none)"}
            </p>
            <p>
              <strong>Signer Fingerprint:</strong>{" "}
              {engineProfilePreview.signer_fingerprint || "(none)"}
            </p>
            <p>
              <strong>Signer Trusted:</strong>{" "}
              {engineProfilePreview.signer_trusted ? "yes" : "no"}
            </p>
            {engineProfilePreview.migrated_from_schema && (
              <p>
                <strong>Migration:</strong> migrated from{" "}
                {engineProfilePreview.migrated_from_schema}
              </p>
            )}
            <p>
              <strong>Available Groups:</strong>{" "}
              {engineProfilePreview.available_groups.join(", ") || "(none)"}
            </p>
            <p>
              <strong>Target Groups:</strong>{" "}
              {getTargetEngineProfileGroups().join(", ")}
            </p>
            {engineProfilePreview.warnings.length > 0 && (
              <>
                <p className="text-amber-500">
                  <strong>Warnings:</strong>{" "}
                  {engineProfilePreview.warnings.join(" | ")}
                </p>
                <label className="mt-2 flex items-center gap-2">
                  <input
                    type="checkbox"
                    checked={engineProfileWarningsAcknowledged}
                    onChange={(e) =>
                      setEngineProfileWarningsAcknowledged(e.target.checked)
                    }
                  />
                  I reviewed and acknowledge the preview warnings before import.
                </label>
              </>
            )}
            <div className="mt-2 flex gap-2">
              <button
                type="button"
                onClick={handleCopySignerFingerprint}
                className="px-2 py-1 text-xs bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
              >
                Copy Signer Fingerprint
              </button>
              <button
                type="button"
                onClick={handleTrustPreviewSigner}
                className="px-2 py-1 text-xs bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
              >
                Trust This Signer
              </button>
              <button
                type="button"
                onClick={handleBlockPreviewSigner}
                className="px-2 py-1 text-xs bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
              >
                Block This Signer
              </button>
            </div>
          </div>
        )}

        {engineProfileMode === "import" && engineProfileDryRun && (
          <div className="mt-3 p-2 border border-[var(--dc-border)] rounded text-sm">
            <p>
              <strong>Dry-run Schema:</strong> {engineProfileDryRun.schema_version_used}
            </p>
            <p>
              <strong>Dry-run Signature Valid:</strong>{" "}
              {engineProfileDryRun.signature_valid ? "yes" : "no"}
            </p>
            <p>
              <strong>Dry-run Signer Fingerprint:</strong>{" "}
              {engineProfileDryRun.signer_fingerprint || "(none)"}
            </p>
            <p>
              <strong>Dry-run Signer Trusted:</strong>{" "}
              {engineProfileDryRun.signer_trusted ? "yes" : "no"}
            </p>
            {engineProfileDryRun.migrated_from_schema && (
              <p>
                <strong>Dry-run Migration:</strong>{" "}
                {engineProfileDryRun.migrated_from_schema} -&gt;{" "}
                {engineProfileDryRun.schema_version_used}
              </p>
            )}
            <p>
              <strong>Changed Groups:</strong>{" "}
              {engineProfileDryRun.changed_groups.join(", ") || "(none)"}
            </p>
            <p>
              <strong>Estimated Updates:</strong>{" "}
              {engineProfileDryRun.estimated_updates}
            </p>
            {engineProfileDryRun.warnings.length > 0 && (
              <p className="text-amber-500">
                <strong>Dry-run Warnings:</strong>{" "}
                {engineProfileDryRun.warnings.join(" | ")}
              </p>
            )}
            {engineProfileDryRun.diff_entries.length > 0 && (
              <div className="mt-2 max-h-[220px] overflow-auto border border-[var(--dc-border)] rounded">
                {engineProfileDryRun.diff_entries.map((entry, idx) => (
                  <div
                    key={`${entry.group}-${entry.field}-${idx}`}
                    className="p-2 text-xs border-b border-[var(--dc-border)]"
                  >
                    <strong>{entry.group}</strong> - {entry.field}
                    <div className="text-[var(--dc-text-muted)]">
                      current: {entry.current_value || "(empty)"}
                    </div>
                    <div>incoming: {entry.incoming_value || "(empty)"}</div>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        <div className="mt-3 p-2 border border-[var(--dc-border)] rounded text-sm">
          <h4 className="font-semibold">Trusted Signer Registry</h4>
          <label className="mt-2 flex items-center gap-2">
            <input
              type="checkbox"
              checked={allowUnknownSigners}
              onChange={(e) => setAllowUnknownSigners(e.target.checked)}
            />
            Allow unknown signing keys
          </label>
          <label className="mt-2 flex items-center gap-2">
            <input
              type="checkbox"
              checked={trustOnFirstUse}
              onChange={(e) => setTrustOnFirstUse(e.target.checked)}
            />
            Trust on first use (TOFU) for unknown signers
          </label>
          <div className="mt-2 grid grid-cols-1 md:grid-cols-2 gap-2">
            <label className="text-xs">
              Trusted signer fingerprints (one per line)
              <textarea
                value={trustedSignerListText}
                onChange={(e) => setTrustedSignerListText(e.target.value)}
                rows={4}
                className="w-full mt-1 p-2 font-mono text-xs bg-[var(--dc-bg)] border border-[var(--dc-border)] rounded"
              />
            </label>
            <label className="text-xs">
              Blocked signer fingerprints (one per line)
              <textarea
                value={blockedSignerListText}
                onChange={(e) => setBlockedSignerListText(e.target.value)}
                rows={4}
                className="w-full mt-1 p-2 font-mono text-xs bg-[var(--dc-bg)] border border-[var(--dc-border)] rounded"
              />
            </label>
          </div>
          <button
            type="button"
            onClick={handleSaveSignerRegistry}
            className="mt-2 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
          >
            Save Signer Registry
            {savedFeedback === "signer" && (
              <span className="text-green-500 text-sm ml-2 font-medium animate-pulse">Saved!</span>
            )}
          </button>
        </div>
      </div>

      {activeSubTab === "diagnostics" && (
        <div className="my-3 border border-[var(--dc-border)] rounded p-3">
          <h3 className="font-semibold mb-2">Scripting Diagnostics</h3>
          <p className="text-sm text-[var(--dc-text-muted)]">
            Recent status events from script library operations and quick tests.
          </p>
          <div className="mt-3 border border-emerald-500/40 bg-emerald-500/5 rounded p-2">
            <div className="flex items-center justify-between gap-2">
              <h4 className="font-semibold text-sm">
                TOFU Audit History ({tofuAuditHistory.length})
              </h4>
              <button
                type="button"
                onClick={refreshTofuAuditHistory}
                className="px-2 py-1 text-xs bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
              >
                {tofuAuditLoading ? "Refreshing..." : "Refresh TOFU Audit"}
              </button>
            </div>
            <p className="text-xs text-[var(--dc-text-muted)] mt-1">
              Filtered signer trust events from backend diagnostics.
            </p>
            <div className="mt-2 max-h-[180px] overflow-auto border border-[var(--dc-border)] rounded">
              {tofuAuditHistory.length === 0 ? (
                <p className="p-2 text-xs text-[var(--dc-text-muted)]">
                  No TOFU audit events found yet.
                </p>
              ) : (
                tofuAuditHistory.map((line, idx) => (
                  <div
                    key={`${line}-${idx}`}
                    className="px-2 py-1 text-xs border-b border-[var(--dc-border)] bg-emerald-500/10"
                  >
                    {line}
                  </div>
                ))
              )}
            </div>
          </div>
          <div className="mt-2 max-h-[260px] overflow-auto border border-[var(--dc-border)] rounded">
            {statusHistory.length === 0 ? (
              <p className="p-2 text-sm text-[var(--dc-text-muted)]">
                No events yet.
              </p>
            ) : (
              statusHistory.map((line, idx) => (
                <div
                  key={`${line}-${idx}`}
                  className="px-2 py-1 text-xs border-b border-[var(--dc-border)]"
                >
                  {line}
                </div>
              ))
            )}
          </div>
        </div>
      )}
    </div>
  );
}
