import React, { useState, useEffect, useRef } from "react";
import { Terminal, RefreshCw, Trash2, ChevronDown, ChevronRight, Info, AlertTriangle, Bug } from "lucide-react";
import { Button } from "../ui/button";
import { getTaurpc } from "../../lib/taurpc";
import { KmsLogDto } from "../../bindings";

export default function KmsLogViewer() {
    const [logs, setLogs] = useState<KmsLogDto[]>([]);
    const [loading, setLoading] = useState(false);
    const [expandedIds, setExpandedIds] = useState<Set<number>>(new Set());
    const scrollRef = useRef<HTMLDivElement>(null);

    const fetchLogs = async () => {
        setLoading(true);
        try {
            const data = await getTaurpc().kms_get_logs(100);
            setLogs(data);
        } catch (error) {
            console.error("Failed to fetch logs:", error);
        } finally {
            setLoading(false);
        }
    };

    const clearLogs = async () => {
        if (!confirm("Are you sure you want to clear all diagnostic logs?")) return;
        try {
            await getTaurpc().kms_clear_logs();
            setLogs([]);
        } catch (error) {
            console.error("Failed to clear logs:", error);
        }
    };

    useEffect(() => {
        fetchLogs();
    }, []);

    const toggleExpand = (id: number) => {
        const next = new Set(expandedIds);
        if (next.has(id)) next.delete(id);
        else next.add(id);
        setExpandedIds(next);
    };

    const getLevelClass = (level: string) => {
        switch (level.toLowerCase()) {
            case "error": return "bg-red-500/10 text-red-500 border-red-500/20";
            case "warn": return "bg-amber-500/10 text-amber-500 border-amber-500/20";
            case "info": return "bg-blue-500/10 text-blue-500 border-blue-500/20";
            default: return "bg-slate-500/10 text-slate-400 border-slate-500/20";
        }
    };

    return (
        <div className="flex flex-col h-full bg-background border-l border-border/50 overflow-hidden">
            <div className="flex items-center justify-between p-4 border-b border-border/50 shrink-0">
                <div className="flex items-center gap-2">
                    <Terminal className="w-5 h-5 text-primary" />
                    <h2 className="text-sm font-semibold tracking-tight">KMS Diagnostic Logs</h2>
                </div>
                <div className="flex items-center gap-2">
                    <Button variant="ghost" size="sm" onClick={fetchLogs} disabled={loading} className="gap-2 h-8">
                        <RefreshCw className={`w-3.5 h-3.5 ${loading ? "animate-spin" : ""}`} />
                        Refresh
                    </Button>
                    <Button variant="ghost" size="sm" onClick={clearLogs} className="gap-2 h-8 text-red-500 hover:text-red-500 hover:bg-red-500/10">
                        <Trash2 className="w-3.5 h-3.5" />
                        Clear All
                    </Button>
                </div>
            </div>

            <div className="flex-1 overflow-y-auto p-4 font-mono text-xs leading-relaxed space-y-2 bg-black/5 dark:bg-white/5">
                {logs.length === 0 ? (
                    <div className="flex flex-col items-center justify-center h-40 text-muted-foreground italic">
                        {loading ? "Loading logs..." : "No logs recorded yet."}
                    </div>
                ) : (
                    logs.map((log) => (
                        <div key={log.id} className="group border border-border/30 rounded-md overflow-hidden bg-background/50 hover:border-border/60 transition-colors">
                            <div
                                className="flex items-start gap-3 p-2 cursor-pointer hover:bg-muted/30 transition-colors"
                                onClick={() => log.details && toggleExpand(log.id)}
                            >
                                <span className="text-muted-foreground whitespace-nowrap pt-0.5 opacity-60">
                                    {new Date(log.timestamp).toLocaleTimeString([], { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' })}
                                </span>

                                <span className={`px-1.5 py-0.5 rounded border text-[10px] font-bold uppercase tracking-wider shrink-0 ${getLevelClass(log.level)}`}>
                                    {log.level}
                                </span>

                                <div className="flex-1 min-w-0">
                                    <p className="text-foreground/90 break-words">{log.message}</p>
                                </div>

                                {log.details && (
                                    <div className="shrink-0 text-muted-foreground group-hover:text-primary transition-colors">
                                        {expandedIds.has(log.id) ? <ChevronDown className="w-4 h-4" /> : <ChevronRight className="w-4 h-4" />}
                                    </div>
                                )}
                            </div>

                            {log.details && expandedIds.has(log.id) && (
                                <div className="px-3 pb-3 pt-1 border-t border-border/30 bg-muted/20">
                                    <pre className="whitespace-pre-wrap break-all text-muted-foreground bg-black/10 dark:bg-black/40 p-2 rounded border border-border/20 mt-1">
                                        {log.details}
                                    </pre>
                                </div>
                            )}
                        </div>
                    ))
                )}
                <div ref={scrollRef} />
            </div>

            <div className="p-2 border-t border-border/50 bg-muted/10 flex items-center justify-between shrink-0">
                <div className="text-[10px] text-muted-foreground uppercase tracking-widest font-bold px-2">
                    Showing latest {logs.length} entries
                </div>
                <div className="flex items-center gap-1.5 px-2">
                    <div className="w-1.5 h-1.5 rounded-full bg-green-500 animate-pulse" />
                    <span className="text-[10px] text-muted-foreground uppercase">System: Operational</span>
                </div>
            </div>
        </div>
    );
}
