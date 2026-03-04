import { useEffect, useState } from "react";
import { getTaurpc } from "@/lib/taurpc";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
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
  const [ghostSuggestorOffsetX, setGhostSuggestorOffsetX] = useState(0);
  const [ghostSuggestorOffsetY, setGhostSuggestorOffsetY] = useState(0);
  const [ghostFollowerEnabled, setGhostFollowerEnabled] = useState(false);
  const [ghostFollowerHover, setGhostFollowerHover] = useState(false);
  const [ghostFollowerCollapse, setGhostFollowerCollapse] = useState(5);
  const [ghostFollowerEdge, setGhostFollowerEdge] = useState<"right" | "left">(
    "right"
  );
  const [ghostFollowerMonitor, setGhostFollowerMonitor] = useState("0");
  const [ghostFollowerSearch, setGhostFollowerSearch] = useState("");
  const [clipMaxDepth, setClipMaxDepth] = useState(20);
  const [expansionPaused, setExpansionPaused] = useState(false);
  const [autostart, setAutostart] = useState(false);
  const [theme, setTheme] = useState("light");
  const [updateChecking, setUpdateChecking] = useState(false);
  const [updateStatus, setUpdateStatus] = useState("");

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
      setGhostSuggestorOffsetX(appState.ghost_suggestor_offset_x ?? 0);
      setGhostSuggestorOffsetY(appState.ghost_suggestor_offset_y ?? 0);
      setGhostFollowerEnabled(!!appState.ghost_follower_enabled);
      setGhostFollowerHover(!!appState.ghost_follower_hover_preview);
      setGhostFollowerCollapse(appState.ghost_follower_collapse_delay_secs ?? 5);
      setGhostFollowerEdge(appState.ghost_follower_edge_right ? "right" : "left");
      setGhostFollowerMonitor(
        String(appState.ghost_follower_monitor_anchor ?? 0)
      );
      setGhostFollowerSearch(appState.ghost_follower_search || "");
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

  const applyConfig = async (partial: Record<string, unknown>) => {
    try {
      await getTaurpc().update_config(partial as Parameters<ReturnType<typeof getTaurpc>["update_config"]>[0]);
      await getTaurpc().save_settings();
      setStatus("Settings saved.");
      setStatusError(false);
    } catch (e) {
      setStatus("Error: " + String(e));
      setStatusError(true);
    }
  };

  const applyTheme = (pref: string) => {
    const resolved =
      pref === "system"
        ? (window.matchMedia("(prefers-color-scheme: dark)").matches
            ? "dark"
            : "light")
        : pref;
    document.documentElement.dataset.theme = resolved;
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
            applyConfig({
              template_date_format: templateDate,
              template_time_format: templateTime,
            })
          }
          className="mt-2 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
        >
          Apply Templates
        </button>
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
          onClick={() => applyConfig({ sync_url: syncUrl })}
          className="mt-2 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
        >
          Save Sync URL
        </button>
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
          onChange={(e) => setDiscoveryThreshold(parseInt(e.target.value, 10))}
          min={2}
          max={10}
          className={inputCls}
        />
        <label className="block mt-2">Lookback (min):</label>
        <input
          type="number"
          value={discoveryLookback}
          onChange={(e) => setDiscoveryLookback(parseInt(e.target.value, 10))}
          min={5}
          max={240}
          className={inputCls}
        />
        <label className="block mt-2">Min phrase length:</label>
        <input
          type="number"
          value={discoveryMinLen}
          onChange={(e) => setDiscoveryMinLen(parseInt(e.target.value, 10))}
          min={2}
          max={20}
          className={inputCls}
        />
        <label className="block mt-2">Max phrase length:</label>
        <input
          type="number"
          value={discoveryMaxLen}
          onChange={(e) => setDiscoveryMaxLen(parseInt(e.target.value, 10))}
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
            applyConfig({
              discovery_enabled: discoveryEnabled,
              discovery_threshold: discoveryThreshold,
              discovery_lookback: discoveryLookback,
              discovery_min_len: discoveryMinLen,
              discovery_max_len: discoveryMaxLen,
              discovery_excluded_apps: discoveryExcludedApps,
              discovery_excluded_window_titles: discoveryExcludedTitles,
            })
          }
          className="mt-2 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
        >
          Apply Discovery
        </button>
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
            setGhostSuggestorDebounce(parseInt(e.target.value, 10))
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
            setGhostSuggestorDisplay(parseInt(e.target.value, 10))
          }
          min={0}
          max={120}
          className={inputCls}
        />
        <label className="block mt-2">Offset X:</label>
        <input
          type="number"
          value={ghostSuggestorOffsetX}
          onChange={(e) =>
            setGhostSuggestorOffsetX(parseInt(e.target.value, 10))
          }
          className={inputCls}
        />
        <label className="block mt-2">Offset Y:</label>
        <input
          type="number"
          value={ghostSuggestorOffsetY}
          onChange={(e) =>
            setGhostSuggestorOffsetY(parseInt(e.target.value, 10))
          }
          className={inputCls}
        />
        <button
          onClick={() =>
            applyConfig({
              ghost_suggestor_enabled: ghostSuggestorEnabled,
              ghost_suggestor_debounce_ms: ghostSuggestorDebounce,
              ghost_suggestor_display_secs: ghostSuggestorDisplay,
              ghost_suggestor_offset_x: ghostSuggestorOffsetX,
              ghost_suggestor_offset_y: ghostSuggestorOffsetY,
            })
          }
          className="mt-2 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
        >
          Apply Ghost Suggestor
        </button>
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
            setGhostFollowerCollapse(parseInt(e.target.value, 10))
          }
          min={0}
          max={60}
          className={inputCls}
        />
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
        <label className="block mt-2">Search filter:</label>
        <input
          type="text"
          value={ghostFollowerSearch}
          onChange={(e) => setGhostFollowerSearch(e.target.value)}
          className={inputCls}
        />
        <button
          onClick={() =>
            applyConfig({
              ghost_follower_enabled: ghostFollowerEnabled,
              ghost_follower_hover_preview: ghostFollowerHover,
              ghost_follower_collapse_delay_secs: ghostFollowerCollapse,
              ghost_follower_edge_right: ghostFollowerEdge === "right",
              ghost_follower_monitor_anchor: parseInt(ghostFollowerMonitor, 10),
              ghost_follower_search: ghostFollowerSearch,
            })
          }
          className="mt-2 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
        >
          Apply Ghost Follower
        </button>
      </details>

      <details className={sectionCls}>
        <summary className="cursor-pointer font-bold">
          Clipboard History (F38-F42)
        </summary>
        <label className="block mt-2">Max depth (5-100):</label>
        <input
          type="number"
          value={clipMaxDepth}
          onChange={(e) => setClipMaxDepth(parseInt(e.target.value, 10))}
          min={5}
          max={100}
          className={inputCls}
        />
        <button
          onClick={() =>
            applyConfig({ clip_history_max_depth: clipMaxDepth })
          }
          className="mt-2 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
        >
          Apply
        </button>
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
            await applyConfig({
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
              ghost_suggestor_offset_x: ghostSuggestorOffsetX,
              ghost_suggestor_offset_y: ghostSuggestorOffsetY,
              ghost_follower_enabled: ghostFollowerEnabled,
              ghost_follower_hover_preview: ghostFollowerHover,
              ghost_follower_collapse_delay_secs: ghostFollowerCollapse,
              ghost_follower_edge_right: ghostFollowerEdge === "right",
              ghost_follower_monitor_anchor: parseInt(ghostFollowerMonitor, 10),
              ghost_follower_search: ghostFollowerSearch,
              clip_history_max_depth: clipMaxDepth,
            });
          }}
          className="mt-2 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
        >
          Save All Settings
        </button>
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
