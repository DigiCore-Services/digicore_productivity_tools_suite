import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AppState } from "../types";

interface ScriptTabProps {
  appState: AppState | null;
}

export function ScriptTab({ appState }: ScriptTabProps) {
  const [status, setStatus] = useState("");
  const [runDisabled, setRunDisabled] = useState(false);
  const [runAllowlist, setRunAllowlist] = useState("");
  const [jsContent, setJsContent] = useState("");

  useEffect(() => {
    if (appState) {
      setRunDisabled(!!appState.script_library_run_disabled);
      setRunAllowlist(appState.script_library_run_allowlist || "");
    }
  }, [appState]);

  const loadScriptTab = async () => {
    try {
      const state = (await invoke("get_app_state")) as AppState;
      setRunDisabled(!!state.script_library_run_disabled);
      setRunAllowlist(state.script_library_run_allowlist || "");
      const js = (await invoke("get_script_library_js")) as string;
      setJsContent(js || "");
    } catch (e) {
      setStatus("Error: " + String(e));
    }
  };

  useEffect(() => {
    loadScriptTab();
  }, []);

  const handleSaveRun = async () => {
    try {
      await invoke("update_config", {
        config: {
          script_library_run_disabled: runDisabled,
          script_library_run_allowlist: runAllowlist,
        },
      });
      await invoke("save_settings");
      setStatus("Run settings saved.");
    } catch (e) {
      setStatus("Error: " + String(e));
    }
  };

  const handleSaveJs = async () => {
    try {
      await invoke("save_script_library_js", { content: jsContent });
      setStatus("Global Library saved! JS hot-reloaded.");
    } catch (e) {
      setStatus("Error: " + String(e));
    }
  };

  return (
    <div className="p-4 border border-[var(--dc-border)] rounded mt-2">
      <h2 className="text-xl font-semibold mb-4">Scripting Engine Library</h2>
      <p className="text-sm text-[var(--dc-text-muted)] mb-4">{status}</p>

      <details className="my-3 border border-[var(--dc-border)] rounded p-2" open>
        <summary className="cursor-pointer font-bold">
          {"{run:}"} Security
        </summary>
        <label className="block mt-2 flex items-center gap-2">
          <input
            type="checkbox"
            checked={runDisabled}
            onChange={(e) => setRunDisabled(e.target.checked)}
          />
          Disable {"{run:command}"} (recommended for security)
        </label>
        <label className="block mt-2">Allowlist (when enabled):</label>
        <textarea
          value={runAllowlist}
          onChange={(e) => setRunAllowlist(e.target.value)}
          rows={3}
          placeholder="Comma-separated: python, cmd, C:\Scripts\, etc. Empty = block all."
          className="w-full max-w-[600px] p-1 bg-[var(--dc-bg)] text-[var(--dc-text)] border border-[var(--dc-border)] rounded mt-1"
        />
        <button
          onClick={handleSaveRun}
          className="mt-2 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
        >
          Save Run Settings
        </button>
      </details>

      <details className="my-3 border border-[var(--dc-border)] rounded p-2" open>
        <summary className="cursor-pointer font-bold">
          Global JavaScript Library
        </summary>
        <p className="text-sm text-[var(--dc-text-muted)] mt-1">
          Define reusable JS functions for use in all {"{js:...}"} tags.
        </p>
        <textarea
          value={jsContent}
          onChange={(e) => setJsContent(e.target.value)}
          rows={16}
          className="w-full max-w-[600px] p-2 font-mono text-sm bg-[var(--dc-bg)] text-[var(--dc-text)] border border-[var(--dc-border)] rounded mt-2"
        />
        <button
          onClick={handleSaveJs}
          className="mt-2 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
        >
          Save & Reload JS
        </button>
      </details>
    </div>
  );
}
