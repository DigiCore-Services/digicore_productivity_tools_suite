import { useCallback, useEffect, useMemo, useState } from "react";
import { getTaurpc } from "@/lib/taurpc";
import { confirm as confirmDialog } from "@tauri-apps/plugin-dialog";

type AppearanceTransparencyRule = {
  app_process: string;
  opacity: number;
  enabled: boolean;
};

function normalizeProcessKey(name: string): string {
  return (name || "")
    .trim()
    .toLowerCase()
    .replace(/\.exe$/i, "");
}

export function AppearanceTab() {
  const [appProcess, setAppProcess] = useState("");
  const [runningProcessNames, setRunningProcessNames] = useState<string[]>([]);
  const [opacity, setOpacity] = useState(255);
  const [rules, setRules] = useState<AppearanceTransparencyRule[]>([]);
  const [selectedApp, setSelectedApp] = useState<string | null>(null);
  const [status, setStatus] = useState("");
  const [statusError, setStatusError] = useState(false);

  const sortedRules = useMemo(
    () =>
      [...rules].sort((a, b) => {
        if (a.enabled !== b.enabled) {
          return a.enabled ? -1 : 1;
        }
        const keyCompare = normalizeProcessKey(a.app_process).localeCompare(
          normalizeProcessKey(b.app_process)
        );
        if (keyCompare !== 0) return keyCompare;
        return a.app_process.toLowerCase().localeCompare(b.app_process.toLowerCase());
      }),
    [rules]
  );

  const conflictingKeys = useMemo(() => {
    const counts = new Map<string, number>();
    for (const rule of sortedRules) {
      const key = normalizeProcessKey(rule.app_process);
      if (!key) continue;
      counts.set(key, (counts.get(key) ?? 0) + 1);
    }
    return Array.from(counts.entries())
      .filter(([, count]) => count > 1)
      .map(([key]) => `${key}.exe`);
  }, [sortedRules]);

  const loadRules = useCallback(async () => {
    try {
      const list = await getTaurpc().get_appearance_transparency_rules();
      setRules(list || []);
      setStatus("");
      setStatusError(false);
    } catch (e) {
      setStatus("Error loading transparency rules: " + String(e));
      setStatusError(true);
    }
  }, []);

  useEffect(() => {
    loadRules();
  }, [loadRules]);

  const loadRunningProcesses = useCallback(async () => {
    try {
      const names = await getTaurpc().get_running_process_names();
      setRunningProcessNames(Array.isArray(names) ? names : []);
    } catch {
      setRunningProcessNames([]);
    }
  }, []);

  useEffect(() => {
    loadRunningProcesses();
  }, [loadRunningProcesses]);

  const handleApplyNow = useCallback(
    async (value: number) => {
      const app = appProcess.trim();
      if (!app) return;
      try {
        const applied = await getTaurpc().apply_appearance_transparency_now(
          app,
          value
        );
        setStatus(
          `Applied preview transparency to ${applied} window${
            applied === 1 ? "" : "s"
          } for ${app}.`
        );
        setStatusError(false);
      } catch {
        /* ignore best-effort apply preview */
      }
    },
    [appProcess]
  );

  const handleToggleEnabled = useCallback(
    async (rule: AppearanceTransparencyRule, enabled: boolean) => {
      try {
        await getTaurpc().save_appearance_transparency_rule(
          rule.app_process,
          rule.opacity,
          enabled
        );
        await loadRules();
        setStatus(
          `Rule ${enabled ? "enabled" : "disabled"} for ${rule.app_process}.`
        );
        setStatusError(false);
      } catch (e) {
        setStatus("Toggle failed: " + String(e));
        setStatusError(true);
      }
    },
    [loadRules]
  );

  const handleApplyRuleNow = useCallback(async (rule: AppearanceTransparencyRule) => {
    try {
      const applied = await getTaurpc().apply_appearance_transparency_now(
        rule.app_process,
        rule.opacity
      );
      setStatus(
        `Applied ${rule.app_process} transparency to ${applied} window${
          applied === 1 ? "" : "s"
        }.`
      );
      setStatusError(false);
    } catch (e) {
      setStatus("Apply now failed: " + String(e));
      setStatusError(true);
    }
  }, []);

  const handleSaveRule = async () => {
    const app = appProcess.trim();
    if (!app) {
      setStatus("Validation: please specify an app process name.");
      setStatusError(true);
      return;
    }
    try {
      await getTaurpc().save_appearance_transparency_rule(app, opacity, true);
      await loadRules();
      setSelectedApp(app);
      setStatus(`Transparency rule saved for ${app}.`);
      setStatusError(false);
    } catch (e) {
      setStatus("Save failed: " + String(e));
      setStatusError(true);
    }
  };

  const handleDeleteRule = async () => {
    const app = selectedApp || appProcess.trim();
    if (!app) {
      setStatus("Validation: select a rule to delete.");
      setStatusError(true);
      return;
    }
    try {
      await getTaurpc().delete_appearance_transparency_rule(app);
      if (selectedApp?.toLowerCase() === app.toLowerCase()) {
        setSelectedApp(null);
      }
      if (appProcess.toLowerCase() === app.toLowerCase()) {
        setAppProcess("");
      }
      await loadRules();
      setStatus(`Transparency rule deleted for ${app}.`);
      setStatusError(false);
    } catch (e) {
      setStatus("Delete failed: " + String(e));
      setStatusError(true);
    }
  };

  const handleRestoreDefaults = useCallback(async () => {
    if (rules.length === 0) {
      setStatus("No Appearance rules to restore.");
      setStatusError(false);
      return;
    }
    const confirmed = await confirmDialog(
      "This will remove all Appearance transparency rules and reset transparency for currently running managed apps. Continue?",
      { title: "Restore Appearance Defaults", kind: "warning" }
    );
    if (!confirmed) {
      setStatus("Restore defaults cancelled.");
      setStatusError(false);
      return;
    }

    const removedRules = rules.length;
    try {
      const resetWindows = await getTaurpc().restore_appearance_defaults();
      setSelectedApp(null);
      setAppProcess("");
      await loadRules();
      setStatus(
        `Restored defaults: cleared ${removedRules} rule${
          removedRules === 1 ? "" : "s"
        } and reset ${resetWindows} window${resetWindows === 1 ? "" : "s"}.`
      );
      setStatusError(false);
    } catch (e) {
      setStatus("Restore defaults failed: " + String(e));
      setStatusError(true);
    }
  }, [loadRules, rules]);

  return (
    <div className="p-4 border border-[var(--dc-border)] rounded mt-2">
      <h2 className="text-xl font-semibold mb-4">Appearance</h2>

      <div className="border border-[var(--dc-border)] rounded p-3 mb-4">
        <h3 className="font-semibold mb-2">Add New Transparency Rule</h3>
        <label className="block mb-2">
          <span className="block mb-1">App Process (e.g., notepad.exe):</span>
          <div className="flex gap-2 items-center">
            <input
              type="text"
              list="appearance-running-processes"
              value={appProcess}
              onChange={(e) => setAppProcess(e.target.value)}
              className="w-full max-w-[320px] p-1 bg-[var(--dc-bg)] text-[var(--dc-text)] border border-[var(--dc-border)] rounded"
            />
            <button
              type="button"
              onClick={loadRunningProcesses}
              className="px-2 py-1 text-xs bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
              title="Refresh running process suggestions"
            >
              Refresh Apps
            </button>
          </div>
          <datalist id="appearance-running-processes">
            {runningProcessNames.map((name) => (
              <option key={name} value={name} />
            ))}
          </datalist>
          <p className="mt-1 text-xs text-[var(--dc-text-muted)]">
            Suggestions use currently running processes.
          </p>
        </label>
        <label className="block mb-1">
          Opacity Level (20-255): <span className="font-semibold">{opacity}</span>
        </label>
        <input
          type="range"
          min={20}
          max={255}
          value={opacity}
          onChange={(e) => {
            const next = Number.parseInt(e.target.value, 10);
            const safe = Number.isFinite(next) ? next : opacity;
            setOpacity(safe);
            handleApplyNow(safe);
          }}
          className="w-full max-w-[420px]"
        />
        <div className="mt-3 flex gap-2">
          <button
            onClick={handleSaveRule}
            className="px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
          >
            Add/Update Rule
          </button>
          <button
            onClick={handleDeleteRule}
            className="px-3 py-1.5 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
          >
            Delete Rule
          </button>
          <button
            onClick={loadRules}
            className="px-3 py-1.5 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
          >
            Refresh Rules
          </button>
          <button
            type="button"
            onClick={handleRestoreDefaults}
            className="px-3 py-1.5 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded text-[var(--dc-error)]"
            title="Clear all Appearance rules and reset managed transparency"
          >
            Restore All Defaults
          </button>
        </div>
      </div>

      <h3 className="font-semibold mb-2">Existing Transparency Rules</h3>
      {sortedRules.length === 0 ? (
        <p className="text-[var(--dc-text-muted)]">No transparency rules saved.</p>
      ) : (
        <table className="w-full border-collapse border border-[var(--dc-border)]">
          <thead>
            <tr>
              <th className="border border-[var(--dc-border)] p-1.5 text-left">Priority</th>
              <th className="border border-[var(--dc-border)] p-1.5 text-left">App Process</th>
              <th className="border border-[var(--dc-border)] p-1.5 text-left">Opacity</th>
              <th className="border border-[var(--dc-border)] p-1.5 text-left">Status</th>
              <th className="border border-[var(--dc-border)] p-1.5 text-left">Enabled</th>
              <th className="border border-[var(--dc-border)] p-1.5 text-left">Actions</th>
            </tr>
          </thead>
          <tbody>
            {sortedRules.map((rule, idx) => (
              <tr
                key={rule.app_process.toLowerCase()}
                className={`cursor-pointer ${
                  selectedApp?.toLowerCase() === rule.app_process.toLowerCase()
                    ? "bg-[var(--dc-bg-tertiary)]"
                    : "even:bg-[var(--dc-bg-alt)]"
                }`}
                onClick={() => setSelectedApp(rule.app_process)}
                onDoubleClick={() => {
                  setSelectedApp(rule.app_process);
                  setAppProcess(rule.app_process);
                  setOpacity(rule.opacity);
                }}
              >
                <td className="border border-[var(--dc-border)] p-1.5">{idx + 1}</td>
                <td className="border border-[var(--dc-border)] p-1.5">{rule.app_process}</td>
                <td className="border border-[var(--dc-border)] p-1.5">{rule.opacity}</td>
                <td className="border border-[var(--dc-border)] p-1.5">
                  {rule.enabled ? "Active" : "Disabled"}
                </td>
                <td className="border border-[var(--dc-border)] p-1.5">
                  <label className="inline-flex items-center gap-2">
                    <input
                      type="checkbox"
                      checked={rule.enabled}
                      onClick={(e) => e.stopPropagation()}
                      onChange={(e) => {
                        handleToggleEnabled(rule, e.target.checked);
                      }}
                    />
                    <span>{rule.enabled ? "On" : "Off"}</span>
                  </label>
                </td>
                <td className="border border-[var(--dc-border)] p-1.5">
                  <button
                    type="button"
                    className="px-2 py-1 text-xs bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
                    onClick={(e) => {
                      e.stopPropagation();
                      handleApplyRuleNow(rule);
                    }}
                    title="Apply this rule now and report windows affected"
                  >
                    Apply now
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {conflictingKeys.length > 0 && (
        <p className="text-sm mt-2 text-amber-500">
          Conflict detected for: {conflictingKeys.join(", ")}. Priority order
          determines which duplicate rule is applied first.
        </p>
      )}

      <p
        className={`text-sm mt-3 ${
          statusError ? "text-[var(--dc-error)]" : "text-[var(--dc-text-muted)]"
        }`}
      >
        {status}
      </p>
    </div>
  );
}
