import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { FileText, RotateCcw } from "lucide-react";
import { Button } from "./ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";

interface DiagnosticEntry {
  timestamp_ms: number;
  level: string;
  message: string;
}

function formatTimestamp(ms: number): string {
  const d = new Date(ms);
  const base = d.toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
  const frac = String(d.getMilliseconds()).padStart(3, "0");
  return `${base}.${frac}`;
}

export function LogTab() {
  const [entries, setEntries] = useState<DiagnosticEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [clearing, setClearing] = useState(false);
  const [autoRefresh, setAutoRefresh] = useState(true);

  const loadLogs = useCallback(async () => {
    try {
      setLoading(true);
      const e = (await invoke("get_diagnostic_logs")) as DiagnosticEntry[];
      setEntries(e);
    } catch (err) {
      console.error("Failed to load diagnostic logs:", err);
      setEntries([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadLogs();
  }, [loadLogs]);

  useEffect(() => {
    if (!autoRefresh) return;
    const id = setInterval(loadLogs, 2000);
    return () => clearInterval(id);
  }, [autoRefresh, loadLogs]);

  const handleClear = useCallback(async () => {
    try {
      setClearing(true);
      await invoke("clear_diagnostic_logs");
      await loadLogs();
    } catch (err) {
      console.error("Failed to clear logs:", err);
    } finally {
      setClearing(false);
    }
  }, [loadLogs]);

  const levelColor = (level: string): string => {
    switch (level) {
      case "error":
        return "text-[var(--dc-error)]";
      case "warn":
        return "text-amber-600 dark:text-amber-400";
      case "info":
        return "text-blue-600 dark:text-blue-400";
      default:
        return "text-[var(--dc-text-muted)]";
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold flex items-center gap-2">
          <FileText className="w-5 h-5" />
          Expansion Diagnostics
        </h2>
        <div className="flex items-center gap-2">
          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={autoRefresh}
              onChange={(e) => setAutoRefresh(e.target.checked)}
              className="rounded"
            />
            Auto-refresh (2s)
          </label>
          <Button
            variant="secondary"
            size="sm"
            onClick={handleClear}
            disabled={clearing}
          >
            <RotateCcw className="w-4 h-4 mr-1" />
            Clear
          </Button>
          <Button variant="secondary" size="sm" onClick={loadLogs}>
            Refresh
          </Button>
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="text-sm font-medium text-[var(--dc-text-muted)]">
            Why didn&apos;t my snippet expand? AppLock, no match, expansion paused.
          </CardTitle>
        </CardHeader>
        <CardContent>
          {loading && entries.length === 0 ? (
            <p className="text-sm text-[var(--dc-text-muted)] py-4">
              Loading...
            </p>
          ) : entries.length === 0 ? (
            <p className="text-sm text-[var(--dc-text-muted)] py-4">
              No diagnostic entries yet. Type a snippet trigger or trigger a
              non-match to see logs.
            </p>
          ) : (
            <pre className="text-xs font-mono overflow-auto max-h-[400px] bg-[var(--dc-bg-alt)] p-3 rounded border border-[var(--dc-border)]">
              {entries.map((e, i) => (
                <div
                  key={i}
                  className={`py-0.5 ${levelColor(e.level)}`}
                >{`[${formatTimestamp(e.timestamp_ms)}] [${e.level}] ${e.message}`}</div>
              ))}
            </pre>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
