import React, { useState, useEffect } from "react";
import { History, RefreshCw, RotateCcw, Clock, User, ChevronRight } from "lucide-react";
import { Button } from "../ui/button";
import { getTaurpc } from "../../lib/taurpc";
import { KmsVersion } from "../../bindings";
import { formatDistanceToNow } from "date-fns";

interface KmsHistoryBrowserProps {
    relPath: string;
    onRestore?: () => void;
}

export default function KmsHistoryBrowser({ relPath, onRestore }: KmsHistoryBrowserProps) {
    const [history, setHistory] = useState<KmsVersion[]>([]);
    const [loading, setLoading] = useState(false);
    const [restoring, setRestoring] = useState<string | null>(null);

    const fetchHistory = async () => {
        if (!relPath) return;
        setLoading(true);
        try {
            const data = await getTaurpc().kms_get_history(relPath);
            setHistory(data);
        } catch (error) {
            console.error("Failed to fetch history:", error);
        } finally {
            setLoading(false);
        }
    };

    const handleRestore = async (version: KmsVersion) => {
        if (!confirm(`Are you sure you want to restore the version from ${new Date(Number(version.timestamp) * 1000).toLocaleString()}? This will overwrite your current unsaved changes.`)) {
            return;
        }

        setRestoring(version.hash);
        try {
            await getTaurpc().kms_restore_version(version.hash, relPath);
            if (onRestore) onRestore();
            fetchHistory(); // Refresh history
        } catch (error) {
            console.error("Failed to restore version:", error);
            alert("Failed to restore version: " + error);
        } finally {
            setRestoring(null);
        }
    };

    useEffect(() => {
        fetchHistory();
    }, [relPath]);

    return (
        <div className="flex flex-col h-full bg-background/95 backdrop-blur-sm border-l border-border/50 overflow-hidden shadow-2xl">
            <div className="flex items-center justify-between p-4 border-b border-border/50 shrink-0 bg-muted/20">
                <div className="flex items-center gap-2">
                    <History className="w-5 h-5 text-primary" />
                    <h2 className="text-sm font-semibold tracking-tight">Version History</h2>
                </div>
                <Button
                    variant="ghost"
                    size="icon"
                    onClick={fetchHistory}
                    disabled={loading}
                    className="h-8 w-8 hover:bg-primary/10 hover:text-primary transition-colors"
                >
                    <RefreshCw className={`w-4 h-4 ${loading ? "animate-spin" : ""}`} />
                </Button>
            </div>

            <div className="flex-1 overflow-y-auto p-4 space-y-3 custom-scrollbar">
                {history.length === 0 ? (
                    <div className="flex flex-col items-center justify-center h-40 text-muted-foreground text-center space-y-2 opacity-60">
                        <Clock className="w-8 h-8 opacity-20" />
                        <p className="text-xs italic">
                            {loading ? "Loading history..." : "No version history found for this note."}
                        </p>
                    </div>
                ) : (
                    history.map((version) => (
                        <div
                            key={version.hash}
                            className="group relative border border-border/30 rounded-lg p-3 bg-background/50 hover:border-primary/40 hover:bg-muted/10 transition-all duration-200"
                        >
                            <div className="flex flex-col gap-2">
                                <div className="flex items-center justify-between">
                                    <div className="flex items-center gap-1.5 text-[10px] font-medium text-muted-foreground uppercase tracking-wider">
                                        <Clock className="w-3 h-3" />
                                        {formatDistanceToNow(new Date(Number(version.timestamp) * 1000), { addSuffix: true })}
                                    </div>
                                    <div className="flex items-center gap-1 text-[10px] font-mono opacity-40 group-hover:opacity-100 transition-opacity">
                                        {version.hash.substring(0, 7)}
                                    </div>
                                </div>

                                <p className="text-xs text-foreground/90 font-medium leading-relaxed line-clamp-2 italic">
                                    "{version.message || "Auto-save version"}"
                                </p>

                                <div className="flex items-center justify-between mt-1 pt-2 border-t border-border/10">
                                    <div className="flex items-center gap-1.5 text-[10px] text-muted-foreground">
                                        <User className="w-3 h-3 opacity-60" />
                                        <span className="truncate max-w-[80px]">{version.author || "DigiCore"}</span>
                                    </div>

                                    <Button
                                        variant="ghost"
                                        size="sm"
                                        onClick={() => handleRestore(version)}
                                        disabled={restoring === version.hash}
                                        className="h-7 px-2 text-[10px] gap-1.5 font-bold uppercase tracking-tighter text-primary/80 hover:text-primary hover:bg-primary/10 transition-colors"
                                    >
                                        <RotateCcw className={`w-3 h-3 ${restoring === version.hash ? "animate-spin" : ""}`} />
                                        Restore
                                    </Button>
                                </div>
                            </div>
                        </div>
                    ))
                )}
            </div>

            <div className="p-3 border-t border-border/30 bg-muted/10 flex items-center justify-center shrink-0">
                <p className="text-[10px] text-muted-foreground uppercase tracking-widest font-bold opacity-60">
                    Background Versioning Active
                </p>
            </div>
        </div>
    );
}
