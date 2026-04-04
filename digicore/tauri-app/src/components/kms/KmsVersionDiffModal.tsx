import React, { useMemo } from "react";
import {
    Dialog,
    DialogContent,
    DialogHeader,
    DialogTitle,
    DialogFooter,
} from "../ui/dialog";
import { Button } from "../ui/button";
import { Loader2, RotateCcw } from "lucide-react";
import type { KmsVersion } from "../../bindings";

export interface KmsVersionDiffModalProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    version: KmsVersion | null;
    workingLabel: string;
    /** Current markdown (editor working copy when available). */
    workingContent: string;
    /** Content at selected Git revision. */
    revisionContent: string;
    loadingRevision: boolean;
    revisionError: string | null;
    restoring: boolean;
    onConfirmRestore: () => void;
}

export default function KmsVersionDiffModal({
    open,
    onOpenChange,
    version,
    workingLabel,
    workingContent,
    revisionContent,
    loadingRevision,
    revisionError,
    restoring,
    onConfirmRestore,
}: KmsVersionDiffModalProps) {
    const identical = useMemo(
        () => workingContent === revisionContent,
        [workingContent, revisionContent]
    );

    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent
                className="max-w-[min(96vw,1200px)] w-full max-h-[min(90vh,900px)] flex flex-col gap-3 p-4 sm:p-6"
                showClose
            >
                <DialogHeader className="shrink-0 space-y-1">
                    <DialogTitle className="text-base">Compare before restore</DialogTitle>
                    {version && (
                        <p className="text-xs text-muted-foreground font-mono">
                            {version.hash.slice(0, 7)} &middot;{" "}
                            {new Date(Number(version.timestamp) * 1000).toLocaleString()}
                            {!identical ? (
                                <span className="text-dc-text ml-2">Working copy and revision differ.</span>
                            ) : (
                                <span className="text-muted-foreground ml-2">Working copy matches revision text.</span>
                            )}
                        </p>
                    )}
                    {identical ? (
                        <p className="text-[11px] text-amber-600/90 dark:text-amber-400/90 bg-amber-500/10 border border-amber-500/20 rounded-md px-2 py-1.5">
                            This Git revision is the same text as the left side. That is normal if you opened{" "}
                            <strong>View diff</strong> on the <strong>Latest save</strong> row right after saving. Scroll the
                            history list and open diff on an <strong>older</strong> row to see the note without your newest
                            edits.
                        </p>
                    ) : null}
                    <p className="text-[11px] text-muted-foreground">
                        Left: {workingLabel}. Right: selected revision from Git. Restoring overwrites the file on disk
                        and reloads the note.
                    </p>
                </DialogHeader>

                <div className="flex-1 min-h-0 grid grid-cols-1 md:grid-cols-2 gap-2 border border-dc-border rounded-lg overflow-hidden bg-dc-bg-secondary/30">
                    <div className="flex flex-col min-h-0 border-b md:border-b-0 md:border-r border-dc-border">
                        <div className="text-[10px] uppercase tracking-wider font-bold text-dc-text-muted px-2 py-1 bg-dc-bg-secondary border-b border-dc-border">
                            Working copy
                        </div>
                        <pre className="flex-1 overflow-auto text-[11px] leading-snug p-2 m-0 font-mono whitespace-pre-wrap break-words max-h-[50vh] md:max-h-[55vh]">
                            {workingContent || "(empty)"}
                        </pre>
                    </div>
                    <div className="flex flex-col min-h-0">
                        <div className="text-[10px] uppercase tracking-wider font-bold text-dc-text-muted px-2 py-1 bg-dc-bg-secondary border-b border-dc-border">
                            Revision
                        </div>
                        <pre className="flex-1 overflow-auto text-[11px] leading-snug p-2 m-0 font-mono whitespace-pre-wrap break-words max-h-[50vh] md:max-h-[55vh]">
                            {loadingRevision ? (
                                <span className="inline-flex items-center gap-2 text-muted-foreground">
                                    <Loader2 className="w-4 h-4 animate-spin" />
                                    Loading revision…
                                </span>
                            ) : revisionError ? (
                                <span className="text-red-500">{revisionError}</span>
                            ) : (
                                revisionContent || "(empty)"
                            )}
                        </pre>
                    </div>
                </div>

                <DialogFooter className="shrink-0 flex flex-row justify-end gap-2 sm:gap-2">
                    <Button type="button" variant="secondary" onClick={() => onOpenChange(false)} disabled={restoring}>
                        Cancel
                    </Button>
                    <Button
                        type="button"
                        className="gap-2"
                        disabled={loadingRevision || Boolean(revisionError) || restoring || !version}
                        onClick={onConfirmRestore}
                    >
                        {restoring ? (
                            <Loader2 className="w-4 h-4 animate-spin" />
                        ) : (
                            <RotateCcw className="w-4 h-4" />
                        )}
                        Restore this version
                    </Button>
                </DialogFooter>
            </DialogContent>
        </Dialog>
    );
}
