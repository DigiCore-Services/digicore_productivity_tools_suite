import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { BarChart3, RotateCcw } from "lucide-react";
import { Button } from "./ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";

interface ExpansionStats {
  total_expansions: number;
  total_chars_saved: number;
  estimated_time_saved_secs: number;
  top_triggers: [string, number][];
}

function formatTimeSaved(secs: number): string {
  if (secs < 60) return `${Math.round(secs)} sec`;
  const mins = Math.floor(secs / 60);
  const s = Math.round(secs % 60);
  if (mins < 60) return `${mins} min ${s} sec`;
  const hrs = Math.floor(mins / 60);
  const m = mins % 60;
  return `${hrs} hr ${m} min`;
}

function formatNumber(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

export function AnalyticsTab() {
  const [stats, setStats] = useState<ExpansionStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [resetting, setResetting] = useState(false);

  const loadStats = useCallback(async () => {
    try {
      setLoading(true);
      const s = (await invoke("get_expansion_stats")) as ExpansionStats;
      setStats(s);
    } catch (e) {
      console.error("Failed to load expansion stats:", e);
      setStats(null);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadStats();
  }, [loadStats]);

  const handleReset = useCallback(async () => {
    try {
      setResetting(true);
      await invoke("reset_expansion_stats");
      await loadStats();
    } catch (e) {
      console.error("Failed to reset stats:", e);
    } finally {
      setResetting(false);
    }
  }, [loadStats]);

  if (loading) {
    return (
      <div className="flex items-center justify-center py-12">
        <p className="text-[var(--dc-text-muted)]">Loading statistics...</p>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold flex items-center gap-2">
          <BarChart3 className="w-5 h-5" />
          Expansion Statistics
        </h2>
        <Button
          variant="secondary"
          size="sm"
          onClick={handleReset}
          disabled={resetting}
        >
          <RotateCcw className="w-4 h-4 mr-1" />
          Reset
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-[var(--dc-text-muted)]">
              Total Expansions
            </CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold">
              {stats ? formatNumber(stats.total_expansions) : "0"}
            </p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-[var(--dc-text-muted)]">
              Characters Saved
            </CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold">
              {stats ? formatNumber(stats.total_chars_saved) : "0"}
            </p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-[var(--dc-text-muted)]">
              Estimated Time Saved
            </CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold">
              {stats
                ? formatTimeSaved(stats.estimated_time_saved_secs)
                : "0 sec"}
            </p>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Most Used Triggers</CardTitle>
          <p className="text-sm text-[var(--dc-text-muted)]">
            Top 10 snippets by expansion count
          </p>
        </CardHeader>
        <CardContent>
          {stats?.top_triggers && stats.top_triggers.length > 0 ? (
            <ul className="space-y-2">
              {stats.top_triggers.map(([trigger, count], i) => (
                <li
                  key={`${trigger}-${i}`}
                  className="flex justify-between items-center py-2 border-b border-[var(--dc-border)] last:border-0"
                >
                  <code className="text-sm bg-[var(--dc-bg-alt)] px-2 py-0.5 rounded">
                    {trigger === "ghost_follower" ? "Ghost Follower" : trigger}
                  </code>
                  <span className="text-sm font-medium">{count} expansions</span>
                </li>
              ))}
            </ul>
          ) : (
            <p className="text-sm text-[var(--dc-text-muted)] py-4">
              No expansions recorded yet. Type a snippet trigger to see stats.
            </p>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
