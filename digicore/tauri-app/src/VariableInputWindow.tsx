import { useEffect, useState, useCallback } from "react";
import { getTaurpc } from "./lib/taurpc";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { open } from "@tauri-apps/plugin-dialog";
import { Calendar, FileSearch, CheckCircle2, ChevronRight } from "lucide-react";
import { applyThemeToDocument, resolveTheme } from "./lib/theme";
import type { PendingVariableInput } from "./types";
import { normalizePendingInput } from "./lib/normalizeState";

const api = getTaurpc();
const win = getCurrentWebviewWindow();

export default function VariableInputWindow() {
  const [data, setData] = useState<PendingVariableInput | null>(null);
  const [values, setValues] = useState<Record<string, string>>({});

  const loadData = useCallback(async (retryCount = 0) => {
    try {
      console.log(`[VariableInput] Loading data, attempt ${retryCount + 1}`);
      const pending = await api.get_pending_variable_input();
      console.log(`[VariableInput] Pending result:`, pending);

      if (pending) {
        const normalized = normalizePendingInput(pending);
        setData(normalized);

        const initial: Record<string, string> = {};
        for (const v of (normalized.vars || [])) {
          if (v.var_type === "checkbox") {
            initial[v.tag] = (normalized.checkbox_checked && normalized.checkbox_checked[v.tag])
              ? (v.options?.[0] || "yes")
              : "";
          } else if (v.var_type === "choice") {
            const idx = (normalized.choice_indices && normalized.choice_indices[v.tag]) ?? 0;
            initial[v.tag] = (v.options && v.options[idx]) ?? (normalized.values && normalized.values[v.tag]) ?? "";
          } else {
            initial[v.tag] = (normalized.values && normalized.values[v.tag]) ?? "";
          }
        }
        setValues(initial);
      } else if (retryCount < 10) {
        console.log(`[VariableInput] No pending data (attempt ${retryCount + 1}), retrying in 250ms...`);
        setTimeout(() => loadData(retryCount + 1), 250);
        console.warn("[VariableInput] No pending data after 10 retries, hiding...");
        try { await win.hide(); } catch (e) { console.error("Hide failed:", e); }
      }
    } catch (e) {
      console.error("[VariableInput] Failed to load pending input:", e);
      if (retryCount < 5) {
        setTimeout(() => loadData(retryCount + 1), 250);
      } else {
        try { await win.hide(); } catch (e) { console.error("Hide failed:", e); }
      }
    }
  }, []);

  useEffect(() => {
    const pref = (typeof localStorage !== "undefined" && localStorage.getItem("digicore-theme")) || "light";
    applyThemeToDocument(resolveTheme(pref));

    loadData();

    let unlistenFn: (() => void) | null = null;
    const setupListener = async () => {
      try {
        const unlisten = await listen("variable-input-refresh", () => {
          console.log("[VariableInput] Refresh event received");
          loadData();
        });
        unlistenFn = unlisten;
      } catch (e) {
        console.warn("[VariableInput] Failed to setup refresh listener:", e);
      }
    };

    setupListener();

    return () => {
      if (unlistenFn) unlistenFn();
    };
  }, [loadData]);

  const handleChange = (tag: string, value: string) => {
    setValues((prev) => ({ ...prev, [tag]: value }));
  };

  const handleFileBrowse = async (tag: string, filter?: string) => {
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: filter ? [{ name: "Requested Files", extensions: filter.split(",").map(e => e.trim().replace("*.", "")) }] : []
      });
      if (selected && typeof selected === "string") {
        handleChange(tag, selected);
      }
    } catch (e) {
      console.error("File browse error:", e);
    }
  };

  const formatDateForInput = (val: string) => {
    if (!val || val.length !== 8) return "";
    const y = val.substring(0, 4);
    const m = val.substring(4, 6);
    const d = val.substring(6, 8);
    return `${y}-${m}-${d}`;
  };

  const formatDateFromInput = (val: string) => {
    return val.replace(/-/g, "");
  };

  const setToday = (tag: string) => {
    const now = new Date();
    const y = now.getFullYear();
    const m = String(now.getMonth() + 1).padStart(2, '0');
    const d = String(now.getDate()).padStart(2, '0');
    handleChange(tag, `${y}${m}${d}`);
  };

  const handleOk = async () => {
    try {
      await api.submit_variable_input(values);
      await win.hide();
    } catch (e) {
      console.error("Submit error:", e);
    }
  };

  const handleCancel = async () => {
    try {
      await api.cancel_variable_input();
      await win.hide();
    } catch (e) {
      console.error("Cancel error:", e);
      try { await win.hide(); } catch (ex) { }
    }
  };

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
        handleOk();
      } else if (e.key === "Escape") {
        handleCancel();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [values]);

  if (!data) {
    return (
      <div className="h-screen bg-[var(--dc-bg)] flex items-center justify-center text-[var(--dc-text-muted)] text-sm">
        <div className="flex flex-col items-center gap-3">
          <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-[var(--dc-accent)]" />
          <span>Syncing with expansion engine...</span>
        </div>
      </div>
    );
  }

  return (
    <div className="h-screen bg-[var(--dc-bg)] text-[var(--dc-text)] p-6 overflow-y-auto flex flex-col border-t border-[var(--dc-border)] shadow-xl">
      <div className="flex-1">
        <h3 className="mt-0 mb-4 text-lg font-semibold flex items-center justify-between gap-2" data-tauri-drag-region>
          <span className="flex items-center gap-2 pointer-events-none">
            Snippet Input Required
          </span>
          <span className="text-[var(--dc-text-muted)] text-xs font-normal pointer-events-none">DigiCore Expander</span>
        </h3>
        <p className="mb-6 text-sm text-[var(--dc-text-muted)]">
          Fill in the placeholders for your expansion:
        </p>

        <div className="space-y-5">
          {data.vars.map((v) => (
            <div key={v.tag} className="space-y-1.5">
              <label className="block text-sm font-medium">
                {v.label}
              </label>

              {v.var_type === "choice" ? (
                <div className="relative">
                  <select
                    value={values[v.tag] ?? v.options[0]}
                    onChange={(e) => handleChange(v.tag, e.target.value)}
                    className="w-full p-2.5 bg-[var(--dc-bg-elevated)] text-[var(--dc-text)] border border-[var(--dc-border)] rounded-md focus:ring-2 focus:ring-[var(--dc-accent)] outline-none transition-all appearance-none cursor-pointer pr-10"
                  >
                    {v.options.map((opt, i) => (
                      <option key={i} value={opt}>
                        {opt}
                      </option>
                    ))}
                  </select>
                  <div className="absolute right-3 top-1/2 -translate-y-1/2 pointer-events-none text-[var(--dc-text-muted)]">
                    <ChevronRight className="w-4 h-4 rotate-90" />
                  </div>
                </div>
              ) : v.var_type === "checkbox" ? (
                <div
                  className="flex items-center gap-3 py-2 px-3 bg-[var(--dc-bg-elevated)] border border-[var(--dc-border)] rounded-md cursor-pointer hover:border-[var(--dc-accent)] transition-colors"
                  onClick={() => handleChange(v.tag, values[v.tag] ? "" : (v.options?.[0] || "yes"))}
                >
                  <div className={`w-5 h-5 rounded border flex items-center justify-center transition-all ${values[v.tag] ? 'bg-[var(--dc-accent)] border-[var(--dc-accent)]' : 'border-[var(--dc-border)] bg-transparent'}`}>
                    {values[v.tag] && <CheckCircle2 className="w-4 h-4 text-white" />}
                  </div>
                  <label htmlFor={v.tag} className="text-sm cursor-pointer select-none">
                    Enable {v.label}
                  </label>
                </div>
              ) : v.var_type === "date_picker" ? (
                <div className="flex gap-2">
                  <div className="relative flex-1">
                    <input
                      type="date"
                      value={formatDateForInput(values[v.tag] ?? "")}
                      onChange={(e) => handleChange(v.tag, formatDateFromInput(e.target.value))}
                      className="w-full p-2 bg-[var(--dc-bg-elevated)] text-[var(--dc-text)] border border-[var(--dc-border)] rounded-md focus:ring-2 focus:ring-[var(--dc-accent)] outline-none transition-all pr-10"
                    />
                    <Calendar className="absolute right-3 top-1/2 -translate-y-1/2 w-4 h-4 text-[var(--dc-text-muted)] pointer-events-none" />
                  </div>
                  <button
                    onClick={() => setToday(v.tag)}
                    className="px-3 text-xs font-medium bg-[var(--dc-bg-alt)] hover:bg-[var(--dc-hover)] border border-[var(--dc-border)] rounded-md transition-colors"
                  >
                    Today
                  </button>
                </div>
              ) : v.var_type === "file_picker" ? (
                <div className="flex gap-2">
                  <div className="relative flex-1">
                    <input
                      type="text"
                      value={values[v.tag] ?? ""}
                      onChange={(e) => handleChange(v.tag, e.target.value)}
                      placeholder="Enter or browse for file path..."
                      className="w-full p-2 bg-[var(--dc-bg-elevated)] text-[var(--dc-text)] border border-[var(--dc-border)] rounded-md focus:ring-2 focus:ring-[var(--dc-accent)] outline-none transition-all pr-10 text-sm"
                    />
                    <FileSearch className="absolute right-3 top-1/2 -translate-y-1/2 w-4 h-4 text-[var(--dc-text-muted)] pointer-events-none" />
                  </div>
                  <button
                    onClick={() => handleFileBrowse(v.tag, v.options?.[0])}
                    className="px-4 py-2 text-sm font-medium bg-[var(--dc-bg-alt)] hover:bg-[var(--dc-hover)] border border-[var(--dc-border)] rounded-md transition-colors whitespace-nowrap"
                  >
                    Browse...
                  </button>
                </div>
              ) : (
                <input
                  type="text"
                  autoFocus={data.vars.indexOf(v) === 0}
                  value={values[v.tag] ?? ""}
                  onChange={(e) => handleChange(v.tag, e.target.value)}
                  className="w-full p-2 bg-[var(--dc-bg-elevated)] text-[var(--dc-text)] border border-[var(--dc-border)] rounded-md focus:ring-2 focus:ring-[var(--dc-accent)] outline-none transition-all"
                />
              )}
            </div>
          ))}
        </div>
      </div>

      <div className="mt-8 pt-4 border-t border-[var(--dc-border)] flex justify-end gap-3 sticky bottom-0 bg-[var(--dc-bg)]">
        <button
          onClick={handleCancel}
          className="px-4 py-2 text-sm font-medium bg-[var(--dc-bg-alt)] hover:bg-[var(--dc-hover)] border border-[var(--dc-border)] rounded-md transition-colors"
        >
          Cancel
        </button>
        <button
          onClick={handleOk}
          className="px-6 py-2 text-sm font-medium bg-[var(--dc-accent)] hover:bg-[var(--dc-accent-hover)] text-white rounded-md shadow-sm transition-all active:scale-95"
        >
          Insert Expansion
        </button>
      </div>
    </div>
  );
}
