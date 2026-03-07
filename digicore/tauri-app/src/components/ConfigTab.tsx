import { useEffect, useState } from "react";
import { emit } from "@tauri-apps/api/event";
import { getTaurpc } from "@/lib/taurpc";
import { resolveTheme, applyThemeToDocument } from "@/lib/theme";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { open, save } from "@tauri-apps/plugin-dialog";
import { ShieldAlert, ShieldCheck, ShieldX } from "lucide-react";
import { normalizeAppState } from "@/lib/normalizeState";
import type { AppState } from "../types";

type TauriAutostart = {
  isEnabled: () => Promise<boolean>;
  enable: () => Promise<void>;
  disable: () => Promise<void>;
};

function getAutostart(): TauriAutostart | undefined {
  return (window as unknown as { __TAURI__?: { autostart?: TauriAutostart } })
    .__TAURI__?.autostart;
}

interface ConfigTabProps {
  appState: AppState | null;
  onConfigLoaded: (state: AppState) => void;
}

type SettingsBundlePreview = {
  path: string;
  schema_version: string;
  available_groups: string[];
  warnings: string[];
  valid: boolean;
};

type CopyToClipboardConfig = {
  enabled: boolean;
  image_capture_enabled: boolean;
  min_log_length: number;
  mask_cc: boolean;
  mask_ssn: boolean;
  mask_email: boolean;
  blacklist_processes: string;
  max_history_entries: number;
  json_output_enabled: boolean;
  json_output_dir: string;
  image_storage_dir: string;
};

type PreviewBadge = {
  label: "Ready" | "Review Warnings" | "Blocked";
  className: string;
  icon: "check" | "alert" | "blocked";
};

function getPreviewBadge(preview: SettingsBundlePreview): PreviewBadge {
  if (!preview.valid) {
    return { label: "Blocked", className: "text-red-500", icon: "blocked" };
  }
  if (preview.warnings.length > 0) {
    return {
      label: "Review Warnings",
      className: "text-amber-500",
      icon: "alert",
    };
  }
  return { label: "Ready", className: "text-emerald-500", icon: "check" };
}

const SETTINGS_GROUP_OPTIONS = [
  { id: "templates", label: "Templates" },
  { id: "sync", label: "Sync" },
  { id: "discovery", label: "Discovery" },
  { id: "ghost_suggestor", label: "Ghost Suggestor" },
  { id: "ghost_follower", label: "Ghost Follower" },
  { id: "clipboard_history", label: "Clipboard History" },
  { id: "copy_to_clipboard", label: "Copy-to-Clipboard" },
  { id: "core", label: "Core" },
  { id: "script_runtime", label: "Script Runtime" },
  { id: "appearance", label: "Appearance" },
] as const;

export function ConfigTab({ appState, onConfigLoaded }: ConfigTabProps) {
  const [status, setStatus] = useState("");
  const [statusError, setStatusError] = useState(false);
  const [templateDate, setTemplateDate] = useState("%Y-%m-%d");
  const [templateTime, setTemplateTime] = useState("%H:%M");
  const [syncUrl, setSyncUrl] = useState("");
  const [discoveryEnabled, setDiscoveryEnabled] = useState(false);
  const [discoveryThreshold, setDiscoveryThreshold] = useState(3);
  const [discoveryLookback, setDiscoveryLookback] = useState(30);
  const [discoveryMinLen, setDiscoveryMinLen] = useState(5);
  const [discoveryMaxLen, setDiscoveryMaxLen] = useState(30);
  const [discoveryExcludedApps, setDiscoveryExcludedApps] = useState("");
  const [discoveryExcludedTitles, setDiscoveryExcludedTitles] = useState("");
  const [ghostSuggestorEnabled, setGhostSuggestorEnabled] = useState(false);
  const [ghostSuggestorDebounce, setGhostSuggestorDebounce] = useState(80);
  const [ghostSuggestorDisplay, setGhostSuggestorDisplay] = useState(10);
  const [ghostSuggestorSnooze, setGhostSuggestorSnooze] = useState(5);
  const [ghostSuggestorOffsetX, setGhostSuggestorOffsetX] = useState(0);
  const [ghostSuggestorOffsetY, setGhostSuggestorOffsetY] = useState(0);
  const [ghostFollowerEnabled, setGhostFollowerEnabled] = useState(false);
  const [ghostFollowerHover, setGhostFollowerHover] = useState(false);
  const [ghostFollowerCollapse, setGhostFollowerCollapse] = useState(5);
  const [ghostFollowerEdge, setGhostFollowerEdge] = useState<"right" | "left">(
    "right"
  );
  const [ghostFollowerMonitor, setGhostFollowerMonitor] = useState("0");
  const [ghostFollowerOpacity, setGhostFollowerOpacity] = useState(100);
  const [clipMaxDepth, setClipMaxDepth] = useState(20);
  const [copyEnabled, setCopyEnabled] = useState(true);
  const [copyImageEnabled, setCopyImageEnabled] = useState(true);
  const [copyMinLogLength, setCopyMinLogLength] = useState(1);
  const [copyMaskCc, setCopyMaskCc] = useState(false);
  const [copyMaskSsn, setCopyMaskSsn] = useState(false);
  const [copyMaskEmail, setCopyMaskEmail] = useState(false);
  const [copyBlacklistProcesses, setCopyBlacklistProcesses] = useState("");
  const [copyJsonOutputEnabled, setCopyJsonOutputEnabled] = useState(true);
  const [copyJsonOutputDir, setCopyJsonOutputDir] = useState("");
  const [copyImageStorageDir, setCopyImageStorageDir] = useState("");
  const [loadedImageStorageDir, setLoadedImageStorageDir] = useState("");
  const [expansionPaused, setExpansionPaused] = useState(false);
  const [autostart, setAutostart] = useState(false);
  const [theme, setTheme] = useState("light");
  const [updateChecking, setUpdateChecking] = useState(false);
  const [updateStatus, setUpdateStatus] = useState("");
  const [settingsTransferMode, setSettingsTransferMode] = useState<
    "export" | "import"
  >("export");
  const [settingsTransferScope, setSettingsTransferScope] = useState<
    "all" | "selected"
  >("all");
  const [selectedSettingsGroups, setSelectedSettingsGroups] = useState<string[]>(
    SETTINGS_GROUP_OPTIONS.map((g) => g.id)
  );
  const [importPreview, setImportPreview] = useState<SettingsBundlePreview | null>(
    null
  );
  const [importWarningsAcknowledged, setImportWarningsAcknowledged] =
    useState(false);
  const [lastSavedGroup, setLastSavedGroup] = useState<string | null>(null);

  const showFeedback = (groupName: string) => {
    setLastSavedGroup(groupName);
    setTimeout(() => {
      setLastSavedGroup((prev) => (prev === groupName ? null : prev));
    }, 3000);
  };

  useEffect(() => {
    if (appState) {
      setTemplateDate(appState.template_date_format || "%Y-%m-%d");
      setTemplateTime(appState.template_time_format || "%H:%M");
      setSyncUrl(appState.sync_url || "");
      setDiscoveryEnabled(!!appState.discovery_enabled);
      setDiscoveryThreshold(appState.discovery_threshold ?? 3);
      setDiscoveryLookback(appState.discovery_lookback ?? 30);
      setDiscoveryMinLen(appState.discovery_min_len ?? 5);
      setDiscoveryMaxLen(appState.discovery_max_len ?? 30);
      setDiscoveryExcludedApps(appState.discovery_excluded_apps || "");
      setDiscoveryExcludedTitles(appState.discovery_excluded_window_titles || "");
      setGhostSuggestorEnabled(!!appState.ghost_suggestor_enabled);
      setGhostSuggestorDebounce(appState.ghost_suggestor_debounce_ms ?? 80);
      setGhostSuggestorDisplay(appState.ghost_suggestor_display_secs ?? 10);
      setGhostSuggestorSnooze(appState.ghost_suggestor_snooze_duration_mins ?? 5);
      setGhostSuggestorOffsetX(appState.ghost_suggestor_offset_x ?? 0);
      setGhostSuggestorOffsetY(appState.ghost_suggestor_offset_y ?? 0);
      setGhostFollowerEnabled(!!appState.ghost_follower_enabled);
      setGhostFollowerHover(!!appState.ghost_follower_hover_preview);
      setGhostFollowerCollapse(appState.ghost_follower_collapse_delay_secs ?? 5);
      setGhostFollowerEdge(appState.ghost_follower_edge_right ? "right" : "left");
      setGhostFollowerMonitor(
        String(appState.ghost_follower_monitor_anchor ?? 0)
      );
      setGhostFollowerOpacity(appState.ghost_follower_opacity ?? 100);
      setClipMaxDepth(appState.clip_history_max_depth ?? 20);
      setExpansionPaused(!!appState.expansion_paused);
      setTheme(
        (typeof localStorage !== "undefined" &&
          localStorage.getItem("digicore-theme")) ||
        "light"
      );
      onConfigLoaded(appState);
    }
  }, [appState]);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const cfg: CopyToClipboardConfig =
          await getTaurpc().get_copy_to_clipboard_config();
        if (cancelled) return;
        setCopyEnabled(!!cfg.enabled);
        setCopyImageEnabled(!!cfg.image_capture_enabled);
        setCopyMinLogLength(cfg.min_log_length ?? 1);
        setCopyMaskCc(!!cfg.mask_cc);
        setCopyMaskSsn(!!cfg.mask_ssn);
        setCopyMaskEmail(!!cfg.mask_email);
        setCopyBlacklistProcesses(cfg.blacklist_processes || "");
        setCopyJsonOutputEnabled(cfg.json_output_enabled ?? true);
        setCopyJsonOutputDir(cfg.json_output_dir || "");
        setCopyImageStorageDir(cfg.image_storage_dir || "");
        setLoadedImageStorageDir(cfg.image_storage_dir || "");
        if (typeof cfg.max_history_entries === "number") {
          setClipMaxDepth(cfg.max_history_entries);
        }
      } catch {
        /* ignore load failure */
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const loadAutostart = async () => {
    try {
      const autostartPlugin = getAutostart();
      if (autostartPlugin) {
        const enabled = await autostartPlugin.isEnabled();
        setAutostart(enabled);
      }
    } catch {
      /* ignore */
    }
  };

  useEffect(() => {
    loadAutostart();
  }, []);

  const parseIntOr = (raw: string, fallback: number): number => {
    const parsed = Number.parseInt(raw, 10);
    return Number.isFinite(parsed) ? parsed : fallback;
  };

  const clampInt = (value: number, min: number, max: number): number =>
    Math.min(max, Math.max(min, value));

  const monitorAnchorSafe = clampInt(
    parseIntOr(ghostFollowerMonitor, 0),
    0,
    2
  );

  const applyConfig = async (
    partial: Record<string, unknown>,
    groupName: string
  ) => {
    try {
      await getTaurpc().update_config(
        partial as Parameters<ReturnType<typeof getTaurpc>["update_config"]>[0]
      );
      await getTaurpc().save_settings();
      const dto = await getTaurpc().get_app_state();
      onConfigLoaded(normalizeAppState(dto));
      setStatus("Settings saved.");
      setStatusError(false);
      showFeedback(groupName);
    } catch (e) {
      setStatus("Error: " + String(e));
      setStatusError(true);
    }
  };

  const applyCopyToClipboardConfig = async () => {
    const minLen = clampInt(copyMinLogLength, 1, 2000);
    const maxEntries = Math.max(0, Number.isFinite(clipMaxDepth) ? clipMaxDepth : 20);
    const imagePathChanged =
      copyImageStorageDir.trim() !== loadedImageStorageDir.trim();
    try {
      await getTaurpc().save_copy_to_clipboard_config({
        enabled: copyEnabled,
        image_capture_enabled: copyImageEnabled,
        min_log_length: minLen,
        mask_cc: copyMaskCc,
        mask_ssn: copyMaskSsn,
        mask_email: copyMaskEmail,
        blacklist_processes: copyBlacklistProcesses,
        max_history_entries: maxEntries,
        json_output_enabled: copyJsonOutputEnabled,
        json_output_dir: copyJsonOutputDir,
        image_storage_dir: copyImageStorageDir,
      });
      setStatus(
        imagePathChanged
          ? "Copy-to-Clipboard settings saved. Image assets migration completed."
          : "Copy-to-Clipboard settings saved."
      );
      setStatusError(false);
      setClipMaxDepth(maxEntries);
      setLoadedImageStorageDir(copyImageStorageDir);
      await applyConfig({ clip_history_max_depth: maxEntries }, "core");
      showFeedback("copy_to_clipboard");
    } catch (e) {
      setStatus("Copy-to-Clipboard save failed: " + String(e));
      setStatusError(true);
    }
  };

  const chooseDirectory = async (
    currentValue: string,
    setter: (value: string) => void
  ) => {
    const selected = await open({
      directory: true,
      multiple: false,
      defaultPath: currentValue || undefined,
      title: "Select folder",
    });
    if (typeof selected === "string" && selected.trim()) {
      setter(selected);
    }
  };

  const handleClearClipboardHistory = async () => {
    try {
      await getTaurpc().clear_clipboard_history();
      setStatus("Clipboard history cleared.");
      setStatusError(false);
    } catch (e) {
      setStatus("Clipboard history clear failed: " + String(e));
      setStatusError(true);
    }
  };

  const applyTheme = (pref: string) => {
    const resolved = resolveTheme(pref);
    applyThemeToDocument(resolved);
    emit("digicore-theme-changed", { theme: resolved }).catch(() => { });
  };

  useEffect(() => {
    applyTheme(theme);
    if (theme !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = () => applyTheme("system");
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, [theme]);

  const inputCls =
    "w-full max-w-[400px] p-1 bg-[var(--dc-bg)] text-[var(--dc-text)] border border-[var(--dc-border)] rounded";

  const sectionCls =
    "my-3 border border-[var(--dc-border)] rounded p-2";

  const getTargetSettingsGroups = (): string[] => {
    if (settingsTransferScope === "all") {
      return SETTINGS_GROUP_OPTIONS.map((g) => g.id);
    }
    return selectedSettingsGroups;
  };

  const toggleSettingsGroup = (groupId: string, checked: boolean) => {
    setSelectedSettingsGroups((prev) => {
      if (checked) {
        return prev.includes(groupId) ? prev : [...prev, groupId];
      }
      return prev.filter((g) => g !== groupId);
    });
  };

  const handleExportSettingsBundle = async () => {
    const groups = getTargetSettingsGroups();
    if (groups.length === 0) {
      setStatus("Validation: choose at least one settings group to export.");
      setStatusError(true);
      return;
    }
    const path = await save({
      title: "Export DigiCore Settings",
      defaultPath: "digicore_settings_bundle.json",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path) {
      setStatus("Export cancelled.");
      setStatusError(false);
      return;
    }
    try {
      const count = await getTaurpc().export_settings_bundle_to_file(
        path,
        groups,
        theme,
        autostart
      );
      setStatus(
        `Exported settings bundle with ${count} group${count === 1 ? "" : "s"
        } to ${String(path)}`
      );
      setStatusError(false);
    } catch (e) {
      setStatus("Export failed: " + String(e));
      setStatusError(true);
    }
  };

  const handleImportSettingsBundle = async () => {
    const pathSelection = await open({
      title: "Preview DigiCore Settings Bundle",
      multiple: false,
      directory: false,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    const path = Array.isArray(pathSelection) ? pathSelection[0] : pathSelection;
    if (!path) {
      setStatus("Import preview cancelled.");
      setStatusError(false);
      return;
    }
    try {
      const preview = await getTaurpc().preview_settings_bundle_from_file(path);
      setImportPreview(preview);
      setImportWarningsAcknowledged(false);
      setStatus(
        `Preview loaded: schema ${preview.schema_version}, groups ${preview.available_groups.length}, warnings ${preview.warnings.length}.`
      );
      setStatusError(!preview.valid);
    } catch (e) {
      setImportPreview(null);
      setImportWarningsAcknowledged(false);
      setStatus("Preview failed: " + String(e));
      setStatusError(true);
    }
  };

  const handleApplyImportSettingsBundle = async () => {
    const groups = getTargetSettingsGroups();
    if (groups.length === 0) {
      setStatus("Validation: choose at least one settings group to import.");
      setStatusError(true);
      return;
    }
    if (!importPreview?.path) {
      setStatus("Validation: preview a settings bundle file first.");
      setStatusError(true);
      return;
    }
    if (!importPreview.valid) {
      setStatus("Validation: cannot import from an invalid preview bundle.");
      setStatusError(true);
      return;
    }
    if (importPreview.warnings.length > 0 && !importWarningsAcknowledged) {
      setStatus("Validation: acknowledge preview warnings before import.");
      setStatusError(true);
      return;
    }
    try {
      const result = await getTaurpc().import_settings_bundle_from_file(
        importPreview.path,
        groups
      );
      if (result.theme) {
        setTheme(result.theme);
        localStorage.setItem("digicore-theme", result.theme);
        applyTheme(result.theme);
      }
      if (typeof result.autostart_enabled === "boolean") {
        try {
          const autostartPlugin = getAutostart();
          if (autostartPlugin) {
            if (result.autostart_enabled) await autostartPlugin.enable();
            else await autostartPlugin.disable();
            setAutostart(result.autostart_enabled);
          }
        } catch {
          /* ignore plugin apply failure */
        }
      }
      const dto = await getTaurpc().get_app_state();
      onConfigLoaded(normalizeAppState(dto));
      setStatus(
        `Import complete: applied ${result.applied_groups.length} group${result.applied_groups.length === 1 ? "" : "s"
        }, warnings ${result.warnings.length}.`
      );
      setStatusError(false);
      setImportPreview(null);
      setImportWarningsAcknowledged(false);
    } catch (e) {
      setStatus("Import failed: " + String(e));
      setStatusError(true);
    }
  };

  return (
    <div className="p-4 border border-[var(--dc-border)] rounded mt-2">
      <h2 className="text-xl font-semibold mb-4">
        Configurations and Settings
      </h2>
      <p className={`text-sm mt-2 ${statusError ? "text-[var(--dc-error)]" : "text-[var(--dc-text-muted)]"}`}>
        {status}
      </p>

      <details className={sectionCls}>
        <summary className="cursor-pointer font-bold">Templates (F16-F20)</summary>
        <p className="text-sm text-[var(--dc-text-muted)] mt-1">
          Placeholders: {"{date}"}, {"{time}"}, {"{time:fmt}"}, {"{clipboard}"},{" "}
          {"{clip:1}"}-{"{clip:N}"}, {"{env:VAR}"}
        </p>
        <label className="block mt-2">Date format:</label>
        <input
          type="text"
          value={templateDate}
          onChange={(e) => setTemplateDate(e.target.value)}
          placeholder="%Y-%m-%d"
          className={inputCls}
        />
        <label className="block mt-2">Time format:</label>
        <input
          type="text"
          value={templateTime}
          onChange={(e) => setTemplateTime(e.target.value)}
          placeholder="%H:%M"
          className={inputCls}
        />
        <button
          onClick={() =>
            applyConfig(
              {
                template_date_format: templateDate,
                template_time_format: templateTime,
              },
              "templates"
            )
          }
          className="mt-2 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
        >
          Save Templates Settings
        </button>
        {lastSavedGroup === "templates" && (
          <span className="ml-2 text-emerald-500 font-medium animate-in fade-in duration-300">
            Saved!
          </span>
        )}
      </details>

      <details className={sectionCls}>
        <summary className="cursor-pointer font-bold">Sync (WebDAV)</summary>
        <label className="block mt-2">WebDAV URL:</label>
        <input
          type="text"
          value={syncUrl}
          onChange={(e) => setSyncUrl(e.target.value)}
          placeholder="https://webdav.example.com/library.json"
          className={inputCls}
        />
        <button
          onClick={() => applyConfig({ sync_url: syncUrl }, "sync")}
          className="mt-2 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
        >
          Save Sync URL Settings
        </button>
        {lastSavedGroup === "sync" && (
          <span className="ml-2 text-emerald-500 font-medium animate-in fade-in duration-300">
            Saved!
          </span>
        )}
      </details>

      <details className={sectionCls}>
        <summary className="cursor-pointer font-bold">Discovery (F60-F69)</summary>
        <label className="block mt-2 flex items-center gap-2">
          <input
            type="checkbox"
            checked={discoveryEnabled}
            onChange={(e) => setDiscoveryEnabled(e.target.checked)}
          />
          Enable Discovery
        </label>
        <label className="block mt-2">Threshold (repeats):</label>
        <input
          type="number"
          value={discoveryThreshold}
          onChange={(e) =>
            setDiscoveryThreshold((prev) =>
              clampInt(parseIntOr(e.target.value, prev), 2, 10)
            )
          }
          min={2}
          max={10}
          className={inputCls}
        />
        <label className="block mt-2">Lookback (min):</label>
        <input
          type="number"
          value={discoveryLookback}
          onChange={(e) =>
            setDiscoveryLookback((prev) =>
              clampInt(parseIntOr(e.target.value, prev), 5, 240)
            )
          }
          min={5}
          max={240}
          className={inputCls}
        />
        <label className="block mt-2">Min phrase length:</label>
        <input
          type="number"
          value={discoveryMinLen}
          onChange={(e) =>
            setDiscoveryMinLen((prev) =>
              clampInt(parseIntOr(e.target.value, prev), 2, 20)
            )
          }
          min={2}
          max={20}
          className={inputCls}
        />
        <label className="block mt-2">Max phrase length:</label>
        <input
          type="number"
          value={discoveryMaxLen}
          onChange={(e) =>
            setDiscoveryMaxLen((prev) =>
              clampInt(parseIntOr(e.target.value, prev), 10, 100)
            )
          }
          min={10}
          max={100}
          className={inputCls}
        />
        <label className="block mt-2">Excluded apps (comma-separated):</label>
        <input
          type="text"
          value={discoveryExcludedApps}
          onChange={(e) => setDiscoveryExcludedApps(e.target.value)}
          className={inputCls}
        />
        <label className="block mt-2">Excluded window titles:</label>
        <input
          type="text"
          value={discoveryExcludedTitles}
          onChange={(e) => setDiscoveryExcludedTitles(e.target.value)}
          className={inputCls}
        />
        <button
          onClick={() =>
            applyConfig(
              {
                discovery_enabled: discoveryEnabled,
                discovery_threshold: discoveryThreshold,
                discovery_lookback: discoveryLookback,
                discovery_min_len: discoveryMinLen,
                discovery_max_len: discoveryMaxLen,
                discovery_excluded_apps: discoveryExcludedApps,
                discovery_excluded_window_titles: discoveryExcludedTitles,
              },
              "discovery"
            )
          }
          className="mt-2 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
        >
          Save Discovery Settings
        </button>
        {lastSavedGroup === "discovery" && (
          <span className="ml-2 text-emerald-500 font-medium animate-in fade-in duration-300">
            Saved!
          </span>
        )}
      </details>

      <details className={sectionCls}>
        <summary className="cursor-pointer font-bold">
          Ghost Suggestor (F43-F47)
        </summary>
        <label className="block mt-2 flex items-center gap-2">
          <input
            type="checkbox"
            checked={ghostSuggestorEnabled}
            onChange={(e) => setGhostSuggestorEnabled(e.target.checked)}
          />
          Enable Ghost Suggestor
        </label>
        <label className="block mt-2">Debounce (ms):</label>
        <input
          type="number"
          value={ghostSuggestorDebounce}
          onChange={(e) =>
            setGhostSuggestorDebounce((prev) =>
              clampInt(parseIntOr(e.target.value, prev), 20, 200)
            )
          }
          min={20}
          max={200}
          className={inputCls}
        />
        <label className="block mt-2">
          Display duration (sec, 0=no auto-hide):
        </label>
        <input
          type="number"
          value={ghostSuggestorDisplay}
          onChange={(e) =>
            setGhostSuggestorDisplay((prev) =>
              clampInt(parseIntOr(e.target.value, prev), 0, 120)
            )
          }
          min={0}
          max={120}
          className={inputCls}
        />
        <label className="block mt-2">Snooze duration (min, 1-120):</label>
        <input
          type="number"
          value={ghostSuggestorSnooze}
          onChange={(e) =>
            setGhostSuggestorSnooze((prev) =>
              clampInt(parseIntOr(e.target.value, prev), 1, 120)
            )
          }
          min={1}
          max={120}
          className={inputCls}
        />
        <label className="block mt-2">Offset X:</label>
        <input
          type="number"
          value={ghostSuggestorOffsetX}
          onChange={(e) =>
            setGhostSuggestorOffsetX((prev) =>
              parseIntOr(e.target.value, prev)
            )
          }
          className={inputCls}
        />
        <label className="block mt-2">Offset Y:</label>
        <input
          type="number"
          value={ghostSuggestorOffsetY}
          onChange={(e) =>
            setGhostSuggestorOffsetY((prev) =>
              parseIntOr(e.target.value, prev)
            )
          }
          className={inputCls}
        />
        <button
          onClick={() =>
            applyConfig(
              {
                ghost_suggestor_enabled: ghostSuggestorEnabled,
                ghost_suggestor_debounce_ms: ghostSuggestorDebounce,
                ghost_suggestor_display_secs: ghostSuggestorDisplay,
                ghost_suggestor_snooze_duration_mins: ghostSuggestorSnooze,
                ghost_suggestor_offset_x: ghostSuggestorOffsetX,
                ghost_suggestor_offset_y: ghostSuggestorOffsetY,
              },
              "ghost_suggestor"
            )
          }
          className="mt-2 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
        >
          Save Ghost Suggestor Settings
        </button>
        {lastSavedGroup === "ghost_suggestor" && (
          <span className="ml-2 text-emerald-500 font-medium animate-in fade-in duration-300">
            Saved!
          </span>
        )}
      </details>

      <details className={sectionCls}>
        <summary className="cursor-pointer font-bold">
          Ghost Follower (F48-F59)
        </summary>
        <label className="block mt-2 flex items-center gap-2">
          <input
            type="checkbox"
            checked={ghostFollowerEnabled}
            onChange={(e) => setGhostFollowerEnabled(e.target.checked)}
          />
          Enable Ghost Follower
        </label>
        <label className="block mt-2 flex items-center gap-2">
          <input
            type="checkbox"
            checked={ghostFollowerHover}
            onChange={(e) => setGhostFollowerHover(e.target.checked)}
          />
          Hover preview
        </label>
        <label className="block mt-2">Collapse delay (s):</label>
        <input
          type="number"
          value={ghostFollowerCollapse}
          onChange={(e) =>
            setGhostFollowerCollapse((prev) =>
              clampInt(parseIntOr(e.target.value, prev), 0, 60)
            )
          }
          min={0}
          max={60}
          className={inputCls}
        />
        <p className="mt-1 text-xs text-[var(--dc-text-muted)]">
          NOTE: value "0" keeps open always, otherwise change duration to 1-n
          seconds before collapsing to pill.
        </p>
        <label className="block mt-2">Edge:</label>
        <select
          value={ghostFollowerEdge}
          onChange={(e) =>
            setGhostFollowerEdge(e.target.value as "right" | "left")
          }
          className={inputCls}
        >
          <option value="right">Right</option>
          <option value="left">Left</option>
        </select>
        <label className="block mt-2">Monitor:</label>
        <select
          value={ghostFollowerMonitor}
          onChange={(e) => setGhostFollowerMonitor(e.target.value)}
          className={inputCls}
        >
          <option value="0">Primary</option>
          <option value="1">Secondary</option>
          <option value="2">Current</option>
        </select>
        <label className="block mt-2">Transparency: {ghostFollowerOpacity}%</label>
        <input
          type="range"
          min={10}
          max={100}
          value={ghostFollowerOpacity}
          onChange={(e) => {
            const v = clampInt(parseIntOr(e.target.value, ghostFollowerOpacity), 10, 100);
            setGhostFollowerOpacity(v);
            getTaurpc().ghost_follower_set_opacity(v).catch(() => { });
          }}
          className="w-full mt-1"
        />
        <button
          onClick={() =>
            applyConfig(
              {
                ghost_follower_enabled: ghostFollowerEnabled,
                ghost_follower_hover_preview: ghostFollowerHover,
                ghost_follower_collapse_delay_secs: ghostFollowerCollapse,
                ghost_follower_edge_right: ghostFollowerEdge === "right",
                ghost_follower_monitor_anchor: monitorAnchorSafe,
                ghost_follower_opacity: ghostFollowerOpacity,
              },
              "ghost_follower"
            )
          }
          className="mt-2 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
        >
          Save Ghost Follower Settings
        </button>
        {lastSavedGroup === "ghost_follower" && (
          <span className="ml-2 text-emerald-500 font-medium animate-in fade-in duration-300">
            Saved!
          </span>
        )}
      </details>

      <details className={sectionCls}>
        <summary className="cursor-pointer font-bold">
          Clipboard History (F38-F42)
        </summary>
        <p className="text-sm text-[var(--dc-text-muted)] mt-1">
          Clipboard entry depth is managed from the Copy-to-Clipboard section to keep
          one single source of truth.
        </p>
        <p className="text-sm mt-2">
          Current max depth: <strong>{clipMaxDepth}</strong>
        </p>
      </details>

      <details className={sectionCls}>
        <summary className="cursor-pointer font-bold">Copy-to-Clipboard</summary>
        <p className="text-sm text-[var(--dc-text-muted)] mt-1">
          Controls capture, filtering, masking, and JSON-only persistence behavior.
        </p>
        <label className="block mt-2 flex items-center gap-2">
          <input
            type="checkbox"
            checked={copyEnabled}
            onChange={(e) => setCopyEnabled(e.target.checked)}
          />
          Enable Copy-to-Clipboard (Text) Capture
        </label>
        <label className="block mt-2 flex items-center gap-2">
          <input
            type="checkbox"
            checked={copyImageEnabled}
            onChange={(e) => setCopyImageEnabled(e.target.checked)}
          />
          Enable Copy-to-Clipboard (Image) Capture
        </label>
        <label className="block mt-2 flex items-center gap-2">
          <input
            type="checkbox"
            checked={copyJsonOutputEnabled}
            onChange={(e) => setCopyJsonOutputEnabled(e.target.checked)}
          />
          Enable JSON Output
        </label>
        <label className="block mt-2">JSON Output Directory:</label>
        <div className="mt-1 flex gap-2">
          <input
            type="text"
            value={copyJsonOutputDir}
            onChange={(e) => setCopyJsonOutputDir(e.target.value)}
            placeholder="C:\\Users\\...\\DigiCore\\clipboard-json"
            className={`${inputCls} flex-1`}
          />
          <button
            type="button"
            onClick={() => void chooseDirectory(copyJsonOutputDir, setCopyJsonOutputDir)}
            className="px-3 py-1.5 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
          >
            Browse
          </button>
        </div>
        <label className="block mt-2">Image Storage Directory:</label>
        <div className="mt-1 flex gap-2">
          <input
            type="text"
            value={copyImageStorageDir}
            onChange={(e) => setCopyImageStorageDir(e.target.value)}
            placeholder="C:\\Users\\...\\DigiCore\\clipboard-images"
            className={`${inputCls} flex-1`}
          />
          <button
            type="button"
            onClick={() =>
              void chooseDirectory(copyImageStorageDir, setCopyImageStorageDir)
            }
            className="px-3 py-1.5 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
          >
            Browse
          </button>
        </div>
        <label className="block mt-2">Min logged content length (1-2000):</label>
        <input
          type="number"
          value={copyMinLogLength}
          onChange={(e) =>
            setCopyMinLogLength((prev) =>
              clampInt(parseIntOr(e.target.value, prev), 1, 2000)
            )
          }
          min={1}
          max={2000}
          className={inputCls}
        />
        <label className="block mt-2">Max history entries:</label>
        <input
          type="number"
          value={clipMaxDepth}
          onChange={(e) =>
            setClipMaxDepth((prev) =>
              Math.max(0, parseIntOr(e.target.value, prev))
            )
          }
          min={0}
          className={inputCls}
        />
        <p className="text-xs text-[var(--dc-text-muted)] mt-1">
          Note: 0 = Unlimited
        </p>
        <label className="block mt-2">Blacklist process names (comma-separated):</label>
        <input
          type="text"
          value={copyBlacklistProcesses}
          onChange={(e) => setCopyBlacklistProcesses(e.target.value)}
          placeholder="KeePassXC.exe, 1Password.exe"
          className={inputCls}
        />
        <div className="mt-2 flex flex-wrap gap-3">
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={copyMaskCc}
              onChange={(e) => setCopyMaskCc(e.target.checked)}
            />
            Mask credit cards
          </label>
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={copyMaskSsn}
              onChange={(e) => setCopyMaskSsn(e.target.checked)}
            />
            Mask SSN
          </label>
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={copyMaskEmail}
              onChange={(e) => setCopyMaskEmail(e.target.checked)}
            />
            Mask email
          </label>
        </div>
        <button
          onClick={applyCopyToClipboardConfig}
          className="mt-2 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
        >
          Save Copy-to-Clipboard Settings
        </button>
        {lastSavedGroup === "copy_to_clipboard" && (
          <span className="ml-2 text-emerald-500 font-medium animate-in fade-in duration-300">
            Saved!
          </span>
        )}
        <button
          onClick={handleClearClipboardHistory}
          className="mt-2 ml-2 px-3 py-1.5 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
        >
          Clear All
        </button>
      </details>

      <details className={sectionCls}>
        <summary className="cursor-pointer font-bold">Appearance</summary>
        <p className="text-sm text-[var(--dc-text-muted)] mt-1">
          NOTE: See &apos;Appearance&apos; tab for detailed configurations and
          settings.
        </p>
      </details>

      <details className={sectionCls}>
        <summary className="cursor-pointer font-bold">Core</summary>
        <label className="block mt-2 flex items-center gap-2">
          <input
            type="checkbox"
            checked={expansionPaused}
            onChange={(e) => setExpansionPaused(e.target.checked)}
          />
          Pause expansion (F7)
        </label>
        <label className="block mt-2 flex items-center gap-2">
          <input
            type="checkbox"
            checked={autostart}
            onChange={async (e) => {
              try {
                const autostartPlugin = getAutostart();
                if (autostartPlugin) {
                  if (e.target.checked) await autostartPlugin.enable();
                  else await autostartPlugin.disable();
                  setAutostart(e.target.checked);
                }
              } catch {
                /* ignore */
              }
            }}
          />
          Start with Windows
        </label>
        <label className="block mt-2">
          Theme:{" "}
          <select
            value={theme}
            onChange={(e) => {
              const v = e.target.value;
              setTheme(v);
              localStorage.setItem("digicore-theme", v);
              applyTheme(v);
            }}
            className={inputCls}
          >
            <option value="light">Light</option>
            <option value="dark">Dark</option>
            <option value="system">System</option>
          </select>
        </label>
        <button
          onClick={async () => {
            try {
              const autostartPlugin = getAutostart();
              if (autostartPlugin) {
                if (autostart) await autostartPlugin.enable();
                else await autostartPlugin.disable();
              }
            } catch {
              /* ignore */
            }
            await applyConfig(
              {
                expansion_paused: expansionPaused,
                template_date_format: templateDate,
                template_time_format: templateTime,
                sync_url: syncUrl,
                discovery_enabled: discoveryEnabled,
                discovery_threshold: discoveryThreshold,
                discovery_lookback: discoveryLookback,
                discovery_min_len: discoveryMinLen,
                discovery_max_len: discoveryMaxLen,
                discovery_excluded_apps: discoveryExcludedApps,
                discovery_excluded_window_titles: discoveryExcludedTitles,
                ghost_suggestor_enabled: ghostSuggestorEnabled,
                ghost_suggestor_debounce_ms: ghostSuggestorDebounce,
                ghost_suggestor_display_secs: ghostSuggestorDisplay,
                ghost_suggestor_snooze_duration_mins: ghostSuggestorSnooze,
                ghost_suggestor_offset_x: ghostSuggestorOffsetX,
                ghost_suggestor_offset_y: ghostSuggestorOffsetY,
                ghost_follower_enabled: ghostFollowerEnabled,
                ghost_follower_hover_preview: ghostFollowerHover,
                ghost_follower_collapse_delay_secs: ghostFollowerCollapse,
                ghost_follower_edge_right: ghostFollowerEdge === "right",
                ghost_follower_monitor_anchor: monitorAnchorSafe,
                ghost_follower_opacity: ghostFollowerOpacity,
                clip_history_max_depth: clipMaxDepth,
              },
              "all"
            );
          }}
          className="mt-2 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
        >
          Save All Settings
        </button>
        {lastSavedGroup === "all" && (
          <span className="ml-2 text-emerald-500 font-medium animate-in fade-in duration-300">
            Saved!
          </span>
        )}
      </details>

      <details className={sectionCls}>
        <summary className="cursor-pointer font-bold">
          Import/Export Settings
        </summary>
        <p className="text-sm text-[var(--dc-text-muted)] mt-1">
          Export or import all settings, or only selected categories for team
          sharing and backups.
        </p>

        <label className="block mt-2 font-medium">Mode:</label>
        <div className="mt-1 flex items-center gap-4">
          <label className="flex items-center gap-2">
            <input
              type="radio"
              name="settings-transfer-mode"
              checked={settingsTransferMode === "export"}
              onChange={() => {
                setSettingsTransferMode("export");
                setImportPreview(null);
                setImportWarningsAcknowledged(false);
              }}
            />
            Export
          </label>
          <label className="flex items-center gap-2">
            <input
              type="radio"
              name="settings-transfer-mode"
              checked={settingsTransferMode === "import"}
              onChange={() => setSettingsTransferMode("import")}
            />
            Import
          </label>
        </div>

        <label className="block mt-3 font-medium">Scope:</label>
        <div className="mt-1 flex items-center gap-4">
          <label className="flex items-center gap-2">
            <input
              type="radio"
              name="settings-transfer-scope"
              checked={settingsTransferScope === "all"}
              onChange={() => setSettingsTransferScope("all")}
            />
            All Settings
          </label>
          <label className="flex items-center gap-2">
            <input
              type="radio"
              name="settings-transfer-scope"
              checked={settingsTransferScope === "selected"}
              onChange={() => setSettingsTransferScope("selected")}
            />
            Selected Groups
          </label>
        </div>

        <div className="mt-3 grid grid-cols-1 md:grid-cols-2 gap-2">
          {SETTINGS_GROUP_OPTIONS.map((group) => (
            <label
              key={group.id}
              className="flex items-center gap-2 text-sm"
            >
              <input
                type="checkbox"
                checked={selectedSettingsGroups.includes(group.id)}
                disabled={settingsTransferScope === "all"}
                onChange={(e) =>
                  toggleSettingsGroup(group.id, e.target.checked)
                }
              />
              {group.label}
            </label>
          ))}
        </div>

        <div className="mt-3 flex gap-2">
          <button
            type="button"
            onClick={handleExportSettingsBundle}
            className="px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
            disabled={settingsTransferMode !== "export"}
          >
            Export Settings JSON
          </button>
          <button
            type="button"
            onClick={handleImportSettingsBundle}
            className="px-3 py-1.5 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
            disabled={settingsTransferMode !== "import"}
          >
            Select Import File (Preview)
          </button>
          <button
            type="button"
            onClick={handleApplyImportSettingsBundle}
            className="px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
            disabled={
              settingsTransferMode !== "import" ||
              !importPreview ||
              !importPreview.valid ||
              (importPreview.warnings.length > 0 && !importWarningsAcknowledged)
            }
          >
            Apply Import from Preview
          </button>
        </div>

        {settingsTransferMode === "import" && importPreview && (
          <div className="mt-3 p-2 border border-[var(--dc-border)] rounded text-sm">
            {(() => {
              const badge = getPreviewBadge(importPreview);
              const icon =
                badge.icon === "check" ? (
                  <ShieldCheck className="w-3.5 h-3.5" aria-hidden />
                ) : badge.icon === "alert" ? (
                  <ShieldAlert className="w-3.5 h-3.5" aria-hidden />
                ) : (
                  <ShieldX className="w-3.5 h-3.5" aria-hidden />
                );
              return (
                <p>
                  <strong>Preview Status:</strong>{" "}
                  <span
                    className={`inline-flex items-center gap-1 ${badge.className}`}
                    title={`Preview status: ${badge.label}`}
                  >
                    {icon}
                    {badge.label}
                  </span>
                </p>
              );
            })()}
            <p>
              <strong>Preview File:</strong> {importPreview.path}
            </p>
            <p>
              <strong>Schema:</strong> {importPreview.schema_version} (
              {importPreview.valid ? "valid" : "invalid"})
            </p>
            <p>
              <strong>Available Groups:</strong>{" "}
              {importPreview.available_groups.join(", ") || "(none)"}
            </p>
            <p>
              <strong>Target Groups:</strong> {getTargetSettingsGroups().join(", ")}
            </p>
            {importPreview.warnings.length > 0 && (
              <>
                <p className="text-amber-500">
                  <strong>Warnings:</strong> {importPreview.warnings.join(" | ")}
                </p>
                <label className="mt-2 flex items-center gap-2">
                  <input
                    type="checkbox"
                    checked={importWarningsAcknowledged}
                    onChange={(e) =>
                      setImportWarningsAcknowledged(e.target.checked)
                    }
                  />
                  I reviewed and acknowledge the preview warnings before import.
                </label>
              </>
            )}

            <p className="mt-2 text-xs text-[var(--dc-text-muted)]">
              <strong>Legend:</strong>{" "}
              <span className="inline-flex items-center gap-1 text-emerald-500 mr-2">
                <ShieldCheck className="w-3.5 h-3.5" aria-hidden />
                Ready
              </span>
              <span className="inline-flex items-center gap-1 text-amber-500 mr-2">
                <ShieldAlert className="w-3.5 h-3.5" aria-hidden />
                Review
              </span>
              <span className="inline-flex items-center gap-1 text-red-500">
                <ShieldX className="w-3.5 h-3.5" aria-hidden />
                Blocked
              </span>
            </p>
          </div>
        )}
      </details>

      <details className={sectionCls}>
        <summary className="cursor-pointer font-bold">Updates</summary>
        <p className="text-sm text-[var(--dc-text-muted)] mt-1">
          Check for app updates. Configure update endpoints in tauri.conf.json to enable.
        </p>
        <button
          onClick={async () => {
            setUpdateChecking(true);
            setUpdateStatus("");
            try {
              const update = await check();
              if (update) {
                setUpdateStatus(`Update available: ${update.version}. Downloading...`);
                await update.downloadAndInstall();
                setUpdateStatus("Update installed. Restarting...");
                await relaunch();
              } else {
                setUpdateStatus("You are on the latest version.");
              }
            } catch (e) {
              setUpdateStatus("Error: " + String(e));
            } finally {
              setUpdateChecking(false);
            }
          }}
          disabled={updateChecking}
          className="mt-2 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded disabled:opacity-50"
        >
          {updateChecking ? "Checking..." : "Check for Updates"}
        </button>
        {updateStatus && (
          <p className="text-sm mt-2 text-[var(--dc-text-muted)]">{updateStatus}</p>
        )}
      </details>
    </div>
  );
}
