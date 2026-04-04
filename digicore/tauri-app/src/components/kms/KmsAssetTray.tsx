import React, { useCallback, useEffect, useState } from "react";
import { getTaurpc } from "../../lib/taurpc";
import { formatIpcOrRaw } from "../../lib/ipcError";
import { useToast } from "../ui/use-toast";
import { Button } from "../ui/button";
import { Image as ImageIcon, Clipboard, Link2, RefreshCw, AlertTriangle } from "lucide-react";

type KmsAssetTrayProps = {
    vaultPath: string | null;
    activeNotePath: string | null;
    onInsertMarkdown: (markdown: string) => void;
};

function isImageRel(p: string): boolean {
    return /\.(png|jpe?g|gif|webp|svg|bmp)$/i.test(p);
}

export default function KmsAssetTray({ vaultPath, activeNotePath, onInsertMarkdown }: KmsAssetTrayProps) {
    const { toast } = useToast();
    const [paths, setPaths] = useState<string[]>([]);
    const [unused, setUnused] = useState<string[] | null>(null);
    const [loading, setLoading] = useState(false);

    const load = useCallback(async () => {
        if (!vaultPath) return;
        setLoading(true);
        try {
            const list = await getTaurpc().kms_list_vault_media();
            setPaths(list);
        } catch (e) {
            toast({ title: "Media list failed", description: formatIpcOrRaw(e), variant: "destructive" });
        } finally {
            setLoading(false);
        }
    }, [vaultPath, toast]);

    useEffect(() => {
        void load();
    }, [load]);

    const runUnused = async () => {
        if (!vaultPath) return;
        setLoading(true);
        try {
            const u = await getTaurpc().kms_list_unused_vault_media();
            setUnused(u);
            if (u.length === 0) {
                toast({ title: "Unused scan", description: "No unused media files found (heuristic)." });
            }
        } catch (e) {
            toast({ title: "Unused report failed", description: formatIpcOrRaw(e), variant: "destructive" });
        } finally {
            setLoading(false);
        }
    };

    const insertLink = (rel: string) => {
        const md = isImageRel(rel) ? `![image](${rel.replace(/\\/g, "/")})` : `[file](${rel.replace(/\\/g, "/")})`;
        onInsertMarkdown(md);
        toast({ title: "Inserted", description: "Markdown link added at the cursor." });
    };

    const copyPath = async (rel: string) => {
        const norm = rel.replace(/\\/g, "/");
        try {
            await navigator.clipboard.writeText(norm);
            toast({ title: "Copied", description: norm });
        } catch {
            toast({ title: "Copy failed", variant: "destructive" });
        }
    };

    if (!vaultPath) return null;

    const list = unused ?? paths;

    return (
        <div className="border border-dc-border rounded-xl bg-dc-bg-secondary/30 p-3 space-y-2">
            <div className="flex items-center justify-between gap-2">
                <div className="text-[10px] font-bold uppercase tracking-wider text-dc-text-muted">Attachments</div>
                <div className="flex items-center gap-1">
                    <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        className="h-7 px-2 text-[10px]"
                        title="Refresh media list"
                        onClick={() => void load()}
                        disabled={loading}
                    >
                        <RefreshCw size={12} className={loading ? "animate-spin" : ""} />
                    </Button>
                    <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        className="h-7 px-2 text-[10px] gap-1"
                        title="List media not referenced in any markdown body (substring heuristic)"
                        onClick={() => void runUnused()}
                        disabled={loading || !activeNotePath}
                    >
                        <AlertTriangle size={12} />
                        Unused report
                    </Button>
                </div>
            </div>
            {!activeNotePath ? (
                <p className="text-[10px] text-dc-text-muted">Open a note to insert asset links at the cursor.</p>
            ) : null}
            {unused != null ? (
                <button
                    type="button"
                    className="text-[10px] text-dc-accent hover:underline"
                    onClick={() => setUnused(null)}
                >
                    Show all media ({paths.length})
                </button>
            ) : null}
            <div className="max-h-40 overflow-y-auto space-y-1 pr-1">
                {list.length === 0 ? (
                    <p className="text-[10px] text-dc-text-muted italic">
                        {unused != null ? "No unused files detected." : "No media files found under the vault."}
                    </p>
                ) : (
                    list.map((rel) => (
                        <div
                            key={rel}
                            className="flex items-start gap-2 rounded-md border border-dc-border/60 bg-dc-bg/40 px-2 py-1.5 text-[10px]"
                        >
                            <ImageIcon size={12} className="mt-0.5 shrink-0 text-dc-text-muted" />
                            <span className="flex-1 font-mono break-all text-dc-text leading-snug">{rel}</span>
                            <div className="flex flex-col gap-0.5 shrink-0">
                                <Button
                                    type="button"
                                    variant="secondary"
                                    size="sm"
                                    className="h-6 px-1.5 text-[9px]"
                                    disabled={!activeNotePath}
                                    onClick={() => insertLink(rel)}
                                >
                                    <Link2 size={10} />
                                </Button>
                                <Button
                                    type="button"
                                    variant="ghost"
                                    size="sm"
                                    className="h-6 px-1.5 text-[9px]"
                                    onClick={() => void copyPath(rel)}
                                >
                                    <Clipboard size={10} />
                                </Button>
                            </div>
                        </div>
                    ))
                )}
            </div>
        </div>
    );
}
