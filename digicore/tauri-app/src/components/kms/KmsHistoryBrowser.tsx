import React, { useState, useEffect, useCallback } from "react";
import { History, RefreshCw, Clock, User, FileDiff } from "lucide-react";
import { Button } from "../ui/button";
import { getTaurpc } from "../../lib/taurpc";
import { KmsVersion } from "../../bindings";
import { formatDistanceToNow } from "date-fns";
import { formatIpcOrRaw } from "../../lib/ipcError";
import KmsVersionDiffModal from "./KmsVersionDiffModal";

interface KmsHistoryBrowserProps {
    /** Vault-relative path (Git index paths), forward slashes. */
    vaultRelPath: string;
    /** Absolute path for `kms_load_note` / revision IPC when the backend resolves from absolute. */
    absoluteNotePath: string;
    /** Panel width in px (optional UI + persistence from parent). */
    panelWidthPx?: number;
    onPanelWidthPxChange?: (px: number) => void;
    /** Latest markdown from the editor when available (falls back to disk). */
    getWorkingCopyMarkdown?: () => string;
    onRestore?: () => void;
}

export default function KmsHistoryBrowser({
    vaultRelPath,
    absoluteNotePath,
    panelWidthPx,
    onPanelWidthPxChange,
    getWorkingCopyMarkdown,
    onRestore,
}: KmsHistoryBrowserProps) {
    const [history, setHistory] = useState<KmsVersion[]>([]);
    const [loading, setLoading] = useState(false);
    const [restoring, setRestoring] = useState<string | null>(null);
    const [diffOpen, setDiffOpen] = useState(false);
    const [diffVersion, setDiffVersion] = useState<KmsVersion | null>(null);
    const [revisionContent, setRevisionContent] = useState("");
    const [loadingRevision, setLoadingRevision] = useState(false);
    const [revisionError, setRevisionError] = useState<string | null>(null);
    const [workingForDiff, setWorkingForDiff] = useState("");
    const [workingLabel, setWorkingLabel] = useState("Working copy");

    const fetchHistory = async () => {
        if (!vaultRelPath) return;
        setLoading(true);
        try {
            const data = await getTaurpc().kms_get_history(vaultRelPath);
            setHistory(data);
        } catch (error) {
            console.error("Failed to fetch history:", error);
        } finally {
            setLoading(false);
        }
    };

    const resolveWorkingMarkdown = useCallback(async () => {
        const fromEditor = getWorkingCopyMarkdown?.();
        if (fromEditor != null && fromEditor.trim().length > 0) {
            setWorkingLabel("Editor (unsaved changes included)");
            return fromEditor;
        }
        try {
            const disk = await getTaurpc().kms_load_note(absoluteNotePath);
            setWorkingLabel("Saved file on disk");
            return disk;
        } catch {
            setWorkingLabel("Working copy");
            return "";
        }
    }, [absoluteNotePath, getWorkingCopyMarkdown]);

    const openDiff = async (version: KmsVersion) => {
        setDiffVersion(version);
        setDiffOpen(true);
        setRevisionContent("");
        setRevisionError(null);
        setLoadingRevision(true);
        const working = await resolveWorkingMarkdown();
        setWorkingForDiff(working);
        try {
            const rev = await getTaurpc().kms_get_note_revision_content(version.hash, absoluteNotePath);
            setRevisionContent(rev);
        } catch (e) {
            setRevisionError(formatIpcOrRaw(e));
        } finally {
            setLoadingRevision(false);
        }
    };

    const handleConfirmRestore = async () => {
        if (!diffVersion) return;
        setRestoring(diffVersion.hash);
        try {
            await getTaurpc().kms_restore_version(diffVersion.hash, vaultRelPath);
            setDiffOpen(false);
            setDiffVersion(null);
            if (onRestore) onRestore();
            fetchHistory();
        } catch (error) {
            console.error("Failed to restore version:", error);
            alert("Failed to restore version: " + formatIpcOrRaw(error));
        } finally {
            setRestoring(null);
        }
    };

    useEffect(() => {
        fetchHistory();
    }, [vaultRelPath]);

    return (
        <div className="flex flex-col h-full bg-background/95 backdrop-blur-sm border-l border-border/50 overflow-hidden shadow-2xl">
            <div className="p-4 border-b border-border/50 shrink-0 bg-muted/20 space-y-1.5">
                <div className="flex items-center justify-between gap-2">
                    <div className="flex items-center gap-2 min-w-0">
                        <History className="w-5 h-5 text-primary shrink-0" />
                        <h2 className="text-sm font-semibold tracking-tight">Version History</h2>
                    </div>
                    <Button
                        variant="ghost"
                        size="icon"
                        onClick={fetchHistory}
                        disabled={loading}
                        className="h-8 w-8 shrink-0 hover:bg-primary/10 hover:text-primary transition-colors"
                        aria-label="Refresh version history"
                    >
                        <RefreshCw className={`w-4 h-4 ${loading ? "animate-spin" : ""}`} />
                    </Button>
                </div>
                <p className="text-[10px] text-muted-foreground leading-snug pr-1">
                    Newest snapshots are listed first. The top row is usually the same as your file right after a save; use{" "}
                    <span className="font-semibold text-foreground/80">View diff</span> on a <em>lower</em> row to see text from
                    before that change.
                </p>
                {typeof panelWidthPx === "number" && onPanelWidthPxChange ? (
                    <div className="flex items-center gap-2 pt-0.5">
                        <span className="text-[9px] font-semibold uppercase tracking-wide text-muted-foreground shrink-0">
                            Panel width
                        </span>
                        <input
                            type="range"
                            min={260}
                            max={920}
                            step={10}
                            value={panelWidthPx}
                            onChange={(ev) => onPanelWidthPxChange(Number(ev.target.value))}
                            className="flex-1 min-w-0 h-2 accent-primary"
                            aria-label="Version history panel width"
                        />
                        <span className="text-[9px] font-mono text-muted-foreground w-9 text-right tabular-nums shrink-0">
                            {panelWidthPx}
                        </span>
                    </div>
                ) : null}
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
                    history.map((version, index) => (
                        <div
                            key={version.hash}
                            className="group relative border border-border/30 rounded-lg p-3 bg-background/50 hover:border-primary/40 hover:bg-muted/10 transition-all duration-200"
                        >
                            <div className="flex flex-col gap-2">
                                <div className="flex items-center justify-between gap-2">
                                    <div className="flex items-center gap-1.5 text-[10px] font-medium text-muted-foreground uppercase tracking-wider min-w-0">
                                        <Clock className="w-3 h-3 shrink-0" />
                                        <span className="truncate">
                                            {formatDistanceToNow(new Date(Number(version.timestamp) * 1000), { addSuffix: true })}
                                        </span>
                                    </div>
                                    <div className="flex items-center gap-1.5 shrink-0">
                                        {index === 0 ? (
                                            <span className="text-[9px] font-semibold uppercase tracking-wide px-1.5 py-0.5 rounded bg-primary/15 text-primary border border-primary/25">
                                                Latest save
                                            </span>
                                        ) : null}
                                        <div className="flex items-center gap-1 text-[10px] font-mono opacity-40 group-hover:opacity-100 transition-opacity">
                                            {version.hash.substring(0, 7)}
                                        </div>
                                    </div>
                                </div>

                                <p className="text-xs text-foreground/90 font-medium leading-relaxed line-clamp-2 italic">
                                    &quot;{version.message || "Auto-save version"}&quot;
                                </p>

                                <Button
                                    type="button"
                                    variant="secondary"
                                    size="sm"
                                    onClick={() => openDiff(version)}
                                    disabled={restoring === version.hash}
                                    className="w-full h-8 gap-2 text-xs font-semibold"
                                >
                                    <FileDiff className="w-3.5 h-3.5 shrink-0" />
                                    View diff
                                </Button>

                                <div className="flex items-center gap-1.5 text-[10px] text-muted-foreground pt-0.5">
                                    <User className="w-3 h-3 opacity-60 shrink-0" />
                                    <span className="truncate">{version.author || "DigiCore"}</span>
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

            <KmsVersionDiffModal
                open={diffOpen}
                onOpenChange={(o) => {
                    if (restoring) return;
                    setDiffOpen(o);
                    if (!o) setDiffVersion(null);
                }}
                version={diffVersion}
                workingLabel={workingLabel}
                workingContent={workingForDiff}
                revisionContent={revisionContent}
                loadingRevision={loadingRevision}
                revisionError={revisionError}
                restoring={Boolean(diffVersion && restoring === diffVersion.hash)}
                onConfirmRestore={handleConfirmRestore}
            />
        </div>
    );
}
