import React, { useState, useEffect } from "react";
import { Button } from "../ui/button";
import { X, FolderOpen, Save, RefreshCw } from "lucide-react";
import { getTaurpc } from "../../lib/taurpc";
import { useToast } from "../ui/use-toast";
import { open } from "@tauri-apps/plugin-dialog";

interface VaultSettingsModalProps {
    isOpen: boolean;
    onClose: () => void;
    currentPath: string | null;
    onPathUpdated: (newPath: string) => void;
}

export default function VaultSettingsModal({ isOpen, onClose, currentPath, onPathUpdated }: VaultSettingsModalProps) {
    const { toast } = useToast();
    const [path, setPath] = useState(currentPath || "");
    const [migrate, setMigrate] = useState(false);
    const [saving, setSaving] = useState(false);

    useEffect(() => {
        if (currentPath) setPath(currentPath);
    }, [currentPath]);

    if (!isOpen) return null;

    const handleBrowse = async () => {
        try {
            const selected = await open({
                directory: true,
                multiple: false,
                defaultPath: path || undefined,
                title: "Select KMS Vault Directory"
            });

            if (selected && typeof selected === "string") {
                setPath(selected);
            }
        } catch (error) {
            console.error("Failed to open dialog:", error);
        }
    };

    const handleSave = async () => {
        if (!path) {
            toast({
                title: "Invalid Path",
                description: "Please select a valid directory for your vault.",
                variant: "destructive",
            });
            return;
        }

        setSaving(true);
        try {
            await getTaurpc().kms_set_vault_path(path, migrate);
            onPathUpdated(path);
            toast({
                title: "Vault Updated",
                description: "KMS vault path has been updated successfully.",
            });
            onClose();
        } catch (error) {
            toast({
                title: "Update Failed",
                description: String(error),
                variant: "destructive",
            });
        } finally {
            setSaving(false);
        }
    };

    return (
        <div className="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm animate-in fade-in duration-200">
            <div className="bg-dc-bg border border-dc-border shadow-2xl rounded-2xl w-full max-w-md overflow-hidden animate-in zoom-in-95 duration-200">
                {/* Header */}
                <div className="flex items-center justify-between p-4 border-b border-dc-border bg-dc-bg-secondary/50">
                    <div className="flex items-center gap-2">
                        <FolderOpen size={18} className="text-dc-accent" />
                        <h3 className="font-semibold text-dc-text">Vault Settings</h3>
                    </div>
                    <button
                        onClick={onClose}
                        className="p-1 hover:bg-dc-bg-hover rounded-full transition-colors text-dc-text-muted hover:text-dc-text"
                    >
                        <X size={18} />
                    </button>
                </div>

                {/* Body */}
                <div className="p-6 space-y-6">
                    <div className="space-y-2">
                        <label className="text-[10px] font-bold text-dc-text-muted uppercase tracking-wider px-1">
                            Local Vault Path
                        </label>
                        <div className="flex gap-2">
                            <input
                                type="text"
                                readOnly
                                className="flex-1 bg-dc-bg-secondary text-dc-text border border-dc-border rounded-xl py-2 px-3 text-xs focus:outline-none opacity-80"
                                value={path}
                            />
                            <Button
                                variant="secondary"
                                size="sm"
                                className="bg-dc-bg-secondary border-dc-border hover:bg-dc-bg-hover px-3 h-9"
                                onClick={handleBrowse}
                            >
                                <FolderOpen size={14} className="mr-2" />
                                Browse
                            </Button>
                        </div>
                        <p className="text-[10px] text-dc-text-muted px-1 italic">
                            This directory will store your Markdown notes and the AI search index.
                        </p>
                    </div>

                    <div className="flex items-start gap-3 p-3 bg-dc-accent/5 border border-dc-accent/10 rounded-xl">
                        <input
                            id="migrate-check"
                            type="checkbox"
                            className="mt-1 h-3.5 w-3.5 rounded border-dc-border text-dc-accent focus:ring-dc-accent bg-dc-bg-secondary cursor-pointer"
                            checked={migrate}
                            onChange={(e) => setMigrate(e.target.checked)}
                        />
                        <div className="space-y-1">
                            <label htmlFor="migrate-check" className="text-xs font-medium text-dc-text cursor-pointer select-none">
                                Migrate existing data
                            </label>
                            <p className="text-[10px] text-dc-text-muted leading-relaxed">
                                If checked, files and index will be moved from the old location to the new one.
                            </p>
                        </div>
                    </div>
                </div>

                {/* Footer */}
                <div className="p-4 bg-dc-bg-secondary/30 border-t border-dc-border flex justify-end gap-3">
                    <Button
                        variant="ghost"
                        size="sm"
                        onClick={onClose}
                        className="text-dc-text-muted hover:bg-dc-bg-hover"
                    >
                        Cancel
                    </Button>
                    <Button
                        size="sm"
                        className="bg-dc-accent hover:bg-dc-accent/90 text-white gap-2 px-6 min-w-[100px]"
                        onClick={handleSave}
                        disabled={saving || !path || path === currentPath}
                    >
                        {saving ? <RefreshCw size={14} className="animate-spin" /> : <Save size={14} />}
                        Save Changes
                    </Button>
                </div>
            </div>
        </div>
    );
}
