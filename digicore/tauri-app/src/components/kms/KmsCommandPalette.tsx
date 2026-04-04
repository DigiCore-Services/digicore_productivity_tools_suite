import React, { useEffect, useMemo, useState } from "react";
import {
    Dialog,
    DialogContent,
    DialogHeader,
    DialogTitle,
} from "../ui/dialog";
import { KmsNoteDto } from "../../bindings";
import { Button } from "../ui/button";
import { formatIpcOrRaw } from "../../lib/ipcError";
import { getTaurpc } from "../../lib/taurpc";
import { useToast } from "../ui/use-toast";
import { Search, Network, FolderOpen, Cpu, Activity, RefreshCw } from "lucide-react";

export type KmsCommandPaletteView =
    | "explorer"
    | "search"
    | "favorites"
    | "recents"
    | "logs"
    | "skills"
    | "graph";

type KmsCommandPaletteProps = {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    notes: KmsNoteDto[];
    onSelectNote: (note: KmsNoteDto) => void;
    onJumpView: (view: KmsCommandPaletteView) => void;
};

function norm(s: string): string {
    return s.replace(/\\/g, "/").toLowerCase();
}

export default function KmsCommandPalette({
    open,
    onOpenChange,
    notes,
    onSelectNote,
    onJumpView,
}: KmsCommandPaletteProps) {
    const { toast } = useToast();
    const [q, setQ] = useState("");

    useEffect(() => {
        if (!open) setQ("");
    }, [open]);

    const noteHits = useMemo(() => {
        const t = q.trim().toLowerCase();
        if (!t) return notes.slice(0, 12);
        return notes
            .filter((n) => norm(n.path).includes(t) || n.title.toLowerCase().includes(t))
            .slice(0, 20);
    }, [notes, q]);

    const actions = useMemo(
        () => [
            { id: "v-ex", label: "Vault Explorer", view: "explorer" as const, icon: FolderOpen },
            { id: "v-se", label: "Knowledge Search", view: "search" as const, icon: Search },
            { id: "v-gr", label: "Knowledge Graph", view: "graph" as const, icon: Network },
            { id: "v-sk", label: "Skill Hub", view: "skills" as const, icon: Cpu },
            { id: "v-lo", label: "Operational Logs", view: "logs" as const, icon: Activity },
        ],
        []
    );

    const filteredActions = useMemo(() => {
        const t = q.trim().toLowerCase();
        if (!t) return actions;
        return actions.filter((a) => a.label.toLowerCase().includes(t));
    }, [actions, q]);

    const triggerReindex = async () => {
        try {
            await getTaurpc().kms_reindex_all();
            toast({ title: "Reindex started", description: "Vault indexing has been triggered." });
            onOpenChange(false);
        } catch (e) {
            toast({ title: "Reindex failed", description: formatIpcOrRaw(e), variant: "destructive" });
        }
    };

    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent className="max-w-lg p-0 gap-0 overflow-hidden">
                <DialogHeader className="px-4 pt-4 pb-2">
                    <DialogTitle className="text-sm">KMS command palette</DialogTitle>
                </DialogHeader>
                <div className="px-4 pb-2">
                    <div className="relative">
                        <Search
                            size={14}
                            className="absolute left-2.5 top-1/2 -translate-y-1/2 text-dc-text-muted pointer-events-none"
                        />
                        <input
                            autoFocus
                            className="w-full rounded-lg border border-dc-border bg-dc-bg-secondary py-2 pl-8 pr-3 text-xs"
                            placeholder="Search notes or jump to a view..."
                            value={q}
                            onChange={(e) => setQ(e.target.value)}
                        />
                    </div>
                </div>
                <div className="max-h-[min(60vh,420px)] overflow-y-auto border-t border-dc-border/60 px-2 py-2 space-y-3">
                    <div>
                        <div className="text-[9px] font-bold uppercase tracking-wider text-dc-text-muted px-2 mb-1">
                            Views
                        </div>
                        <div className="flex flex-col gap-0.5">
                            {filteredActions.map((a) => (
                                <button
                                    key={a.id}
                                    type="button"
                                    className="flex items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs hover:bg-dc-bg-hover"
                                    onClick={() => {
                                        onJumpView(a.view);
                                        onOpenChange(false);
                                    }}
                                >
                                    <a.icon size={14} className="text-dc-text-muted shrink-0" />
                                    {a.label}
                                </button>
                            ))}
                        </div>
                    </div>
                    <div>
                        <div className="text-[9px] font-bold uppercase tracking-wider text-dc-text-muted px-2 mb-1">
                            Notes
                        </div>
                        <div className="flex flex-col gap-0.5">
                            {noteHits.map((n) => (
                                <button
                                    key={n.path}
                                    type="button"
                                    className="rounded-md px-2 py-1.5 text-left text-xs hover:bg-dc-bg-hover"
                                    onClick={() => {
                                        onSelectNote(n);
                                        onOpenChange(false);
                                    }}
                                >
                                    <div className="font-medium text-dc-text truncate">{n.title}</div>
                                    <div className="text-[10px] text-dc-text-muted font-mono truncate">{n.path}</div>
                                </button>
                            ))}
                        </div>
                    </div>
                    <div className="px-2 pt-1 border-t border-dc-border/40">
                        <Button
                            type="button"
                            variant="secondary"
                            size="sm"
                            className="w-full justify-start gap-2 h-8 text-xs"
                            onClick={() => void triggerReindex()}
                        >
                            <RefreshCw size={14} />
                            Reindex vault (advanced)
                        </Button>
                    </div>
                </div>
            </DialogContent>
        </Dialog>
    );
}
