import React, { useMemo, useState } from "react";
import {
    Dialog,
    DialogContent,
    DialogHeader,
    DialogTitle,
    DialogDescription,
    DialogFooter,
} from "../ui/dialog";
import { Button } from "../ui/button";
import { KMS_TEMPLATE_GALLERY, kmsDailyNoteRelPath, type KmsTemplateGalleryEntry } from "../../lib/kmsTemplateGallery";

type KmsTemplateGalleryModalProps = {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    vaultPath: string;
    onCreate: (absolutePath: string, content: string) => Promise<void>;
};

export default function KmsTemplateGalleryModal({
    open,
    onOpenChange,
    vaultPath,
    onCreate,
}: KmsTemplateGalleryModalProps) {
    const [selectedId, setSelectedId] = useState<string>(KMS_TEMPLATE_GALLERY[0]?.id ?? "");
    const [useDailyPath, setUseDailyPath] = useState(false);
    const [busy, setBusy] = useState(false);

    const selected = useMemo(
        () => KMS_TEMPLATE_GALLERY.find((t) => t.id === selectedId) ?? KMS_TEMPLATE_GALLERY[0],
        [selectedId]
    );

    const previewPath = useMemo(() => {
        if (!selected) return "";
        if (selected.suggestDailyPath && useDailyPath) {
            const rel = kmsDailyNoteRelPath();
            return `${vaultPath.replace(/[/\\]+$/, "")}\\${rel.replace(/\//g, "\\")}`;
        }
        const title = selected.title.replace(/\s+/g, "_");
        return `${vaultPath.replace(/[/\\]+$/, "")}\\notes\\${title}.md`;
    }, [selected, useDailyPath, vaultPath]);

    const handleCreate = async () => {
        if (!selected) return;
        setBusy(true);
        try {
            await onCreate(previewPath, selected.body);
            onOpenChange(false);
        } finally {
            setBusy(false);
        }
    };

    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent className="max-w-lg max-h-[85vh] overflow-y-auto">
                <DialogHeader>
                    <DialogTitle>Template gallery</DialogTitle>
                    <DialogDescription>
                        Create a new markdown file from a curated template. Paths use your vault folder.
                    </DialogDescription>
                </DialogHeader>
                <div className="space-y-3">
                    <div className="flex flex-col gap-1.5">
                        {KMS_TEMPLATE_GALLERY.map((t: KmsTemplateGalleryEntry) => (
                            <button
                                key={t.id}
                                type="button"
                                onClick={() => {
                                    setSelectedId(t.id);
                                    setUseDailyPath(Boolean(t.suggestDailyPath));
                                }}
                                className={`text-left rounded-lg border px-3 py-2 text-xs transition-colors ${
                                    selectedId === t.id
                                        ? "border-dc-accent bg-dc-accent/10"
                                        : "border-dc-border hover:bg-dc-bg-hover"
                                }`}
                            >
                                <div className="font-semibold text-dc-text">{t.title}</div>
                                <div className="text-dc-text-muted mt-0.5">{t.description}</div>
                            </button>
                        ))}
                    </div>
                    {selected?.suggestDailyPath ? (
                        <label className="flex items-center gap-2 text-xs text-dc-text cursor-pointer">
                            <input
                                type="checkbox"
                                checked={useDailyPath}
                                onChange={(e) => setUseDailyPath(e.target.checked)}
                            />
                            Use dated path ({kmsDailyNoteRelPath().replace(/\//g, "\\")})
                        </label>
                    ) : null}
                    <div>
                        <div className="text-[10px] uppercase text-dc-text-muted font-bold mb-1">Save as</div>
                        <div className="text-[10px] font-mono break-all text-dc-text-muted bg-dc-bg-secondary/50 border border-dc-border rounded-md p-2">
                            {previewPath}
                        </div>
                    </div>
                </div>
                <DialogFooter className="gap-2">
                    <Button type="button" variant="ghost" onClick={() => onOpenChange(false)}>
                        Cancel
                    </Button>
                    <Button type="button" onClick={() => void handleCreate()} disabled={busy || !selected}>
                        {busy ? "Creating..." : "Create note"}
                    </Button>
                </DialogFooter>
            </DialogContent>
        </Dialog>
    );
}
