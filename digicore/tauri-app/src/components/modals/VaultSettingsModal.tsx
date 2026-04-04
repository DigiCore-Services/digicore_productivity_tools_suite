import React, { useState, useEffect, useMemo } from "react";
import { Button } from "../ui/button";
import { X, FolderOpen, Save, RefreshCw, ChevronDown } from "lucide-react";
import { getTaurpc } from "../../lib/taurpc";
import { formatIpcOrRaw } from "../../lib/ipcError";
import { useToast } from "../ui/use-toast";
import { open } from "@tauri-apps/plugin-dialog";
import type { AppStateDto } from "../../bindings";
import { KMS_GRAPH_DEFAULT_WARN_NOTE_THRESHOLD } from "../../lib/kmsGraphPaging";

interface VaultSettingsModalProps {
    isOpen: boolean;
    onClose: () => void;
    currentPath: string | null;
    onPathUpdated: (newPath: string) => void;
}

function parseIntOr(raw: string, fallback: number): number {
    const parsed = Number.parseInt(raw, 10);
    return Number.isFinite(parsed) ? parsed : fallback;
}

function clampInt(value: number, min: number, max: number): number {
    return Math.min(max, Math.max(min, value));
}

function isPlainObject(v: unknown): v is Record<string, unknown> {
    return typeof v === "object" && v !== null && !Array.isArray(v);
}

export default function VaultSettingsModal({ isOpen, onClose, currentPath, onPathUpdated }: VaultSettingsModalProps) {
    const { toast } = useToast();
    const [path, setPath] = useState(currentPath || "");
    const [migrate, setMigrate] = useState(false);
    const [saving, setSaving] = useState(false);
    const [globalGraph, setGlobalGraph] = useState<AppStateDto | null>(null);
    const [vaultPatch, setVaultPatch] = useState<Record<string, unknown>>({});
    const [overridesLoading, setOverridesLoading] = useState(false);
    const [overridesSaving, setOverridesSaving] = useState(false);

    useEffect(() => {
        if (currentPath) setPath(currentPath);
    }, [currentPath]);

    useEffect(() => {
        if (!isOpen) return;
        let cancelled = false;
        (async () => {
            setOverridesLoading(true);
            try {
                const [j, app] = await Promise.all([
                    getTaurpc().kms_get_vault_graph_overrides_json(),
                    getTaurpc().get_app_state(),
                ]);
                if (cancelled) return;
                setGlobalGraph(app);
                try {
                    const parsed: unknown = JSON.parse(j || "{}");
                    setVaultPatch(isPlainObject(parsed) ? { ...parsed } : {});
                } catch {
                    setVaultPatch({});
                }
            } catch (e) {
                if (!cancelled) {
                    setVaultPatch({});
                    console.error(e);
                }
            } finally {
                if (!cancelled) setOverridesLoading(false);
            }
        })();
        return () => {
            cancelled = true;
        };
    }, [isOpen, currentPath]);

    const globalHint = useMemo(() => {
        const g = globalGraph;
        if (!g) return null;
        return {
            kms_graph_enable_semantic_clustering: g.kms_graph_enable_semantic_clustering ?? true,
            kms_graph_enable_leiden_communities: g.kms_graph_enable_leiden_communities ?? true,
            kms_graph_enable_ai_beams: g.kms_graph_enable_ai_beams ?? true,
            kms_graph_k_means_max_k: g.kms_graph_k_means_max_k ?? 10,
            kms_graph_k_means_iterations: g.kms_graph_k_means_iterations ?? 15,
            kms_graph_ai_beam_max_nodes: g.kms_graph_ai_beam_max_nodes ?? 400,
            kms_graph_ai_beam_similarity_threshold: g.kms_graph_ai_beam_similarity_threshold ?? 0.9,
            kms_graph_ai_beam_max_edges: g.kms_graph_ai_beam_max_edges ?? 20,
            kms_graph_semantic_max_notes: g.kms_graph_semantic_max_notes ?? 2500,
            kms_graph_warn_note_threshold:
                g.kms_graph_warn_note_threshold ?? KMS_GRAPH_DEFAULT_WARN_NOTE_THRESHOLD,
            kms_graph_beam_max_pair_checks: g.kms_graph_beam_max_pair_checks ?? 200000,
            kms_graph_enable_semantic_knn_edges: g.kms_graph_enable_semantic_knn_edges ?? true,
            kms_graph_semantic_knn_per_note: g.kms_graph_semantic_knn_per_note ?? 5,
            kms_graph_semantic_knn_min_similarity: g.kms_graph_semantic_knn_min_similarity ?? 0.82,
            kms_graph_semantic_knn_max_edges: g.kms_graph_semantic_knn_max_edges ?? 8000,
            kms_graph_semantic_knn_max_pair_checks: g.kms_graph_semantic_knn_max_pair_checks ?? 400000,
            kms_graph_pagerank_iterations: g.kms_graph_pagerank_iterations ?? 48,
            kms_graph_pagerank_local_iterations: g.kms_graph_pagerank_local_iterations ?? 32,
            kms_graph_pagerank_damping: g.kms_graph_pagerank_damping ?? 0.85,
            kms_graph_pagerank_scope: g.kms_graph_pagerank_scope ?? "auto",
            kms_graph_background_wiki_pagerank_enabled:
                g.kms_graph_background_wiki_pagerank_enabled !== false,
            kms_search_min_similarity: g.kms_search_min_similarity ?? 0,
            kms_embedding_chunk_enabled: g.kms_embedding_chunk_enabled ?? false,
            kms_embedding_chunk_max_chars: g.kms_embedding_chunk_max_chars ?? 2048,
            kms_embedding_chunk_overlap_chars: g.kms_embedding_chunk_overlap_chars ?? 128,
        };
    }, [globalGraph]);

    const patchJsonPreview = useMemo(() => {
        try {
            return JSON.stringify(vaultPatch, null, 2);
        } catch {
            return "{}";
        }
    }, [vaultPatch]);

    const updatePatch = (key: string, value: unknown | undefined) => {
        setVaultPatch(prev => {
            const next = { ...prev };
            if (value === undefined || value === "") {
                delete next[key];
            } else {
                next[key] = value;
            }
            return next;
        });
    };

    if (!isOpen) return null;

    const handleBrowse = async () => {
        try {
            const selected = await open({
                directory: true,
                multiple: false,
                defaultPath: path || undefined,
                title: "Select KMS Vault Directory",
            });

            if (selected && typeof selected === "string") {
                setPath(selected);
            }
        } catch (error) {
            console.error("Failed to open dialog:", error);
        }
    };

    const handleSaveVaultPath = async () => {
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

    type VaultPatchKey =
        | "kms_graph_enable_semantic_clustering"
        | "kms_graph_enable_leiden_communities"
        | "kms_graph_enable_ai_beams"
        | "kms_graph_k_means_max_k"
        | "kms_graph_k_means_iterations"
        | "kms_graph_ai_beam_max_nodes"
        | "kms_graph_ai_beam_similarity_threshold"
        | "kms_graph_ai_beam_max_edges"
        | "kms_graph_semantic_max_notes"
        | "kms_graph_warn_note_threshold"
        | "kms_graph_beam_max_pair_checks"
        | "kms_graph_enable_semantic_knn_edges"
        | "kms_graph_semantic_knn_per_note"
        | "kms_graph_semantic_knn_min_similarity"
        | "kms_graph_semantic_knn_max_edges"
        | "kms_graph_semantic_knn_max_pair_checks"
        | "kms_graph_pagerank_iterations"
        | "kms_graph_pagerank_local_iterations"
        | "kms_graph_pagerank_damping"
        | "kms_graph_pagerank_scope"
        | "kms_graph_background_wiki_pagerank_enabled"
        | "kms_search_min_similarity"
        | "kms_embedding_chunk_enabled"
        | "kms_embedding_chunk_max_chars"
        | "kms_embedding_chunk_overlap_chars";

    const boolSelect = (key: VaultPatchKey, label: string) => {
        const raw = vaultPatch[key];
        let sel = "";
        if (typeof raw === "boolean") sel = raw ? "true" : "false";
        const gh = globalHint?.[key as keyof typeof globalHint];
        const hint =
            typeof gh === "boolean"
                ? gh
                    ? "on"
                    : "off"
                : gh !== undefined
                  ? String(gh)
                  : "";
        return (
            <label className="flex flex-col gap-1 text-xs">
                <span className="font-semibold text-dc-text">{label}</span>
                <span className="text-[10px] text-dc-text-muted">Global default: {hint}</span>
                <select
                    className="border border-dc-border rounded px-2 py-1 bg-dc-bg-secondary text-dc-text max-w-xs"
                    value={sel}
                    disabled={overridesLoading}
                    onChange={e => {
                        const v = e.target.value;
                        if (v === "") updatePatch(key, undefined);
                        else updatePatch(key, v === "true");
                    }}
                >
                    <option value="">Inherit (use global)</option>
                    <option value="true">On (true)</option>
                    <option value="false">Off (false)</option>
                </select>
            </label>
        );
    };

    const numField = (
        key: VaultPatchKey,
        label: string,
        min: number,
        max: number,
        step: number | "int",
        globalKey: keyof NonNullable<typeof globalHint>
    ) => {
        const raw = vaultPatch[key];
        const str =
            typeof raw === "number" && Number.isFinite(raw)
                ? step === "int"
                    ? String(Math.round(raw))
                    : String(raw)
                : "";
        const g = globalHint?.[globalKey];
        const gStr = g !== undefined && g !== null ? String(g) : "";
        return (
            <label className="flex flex-col gap-1 text-xs">
                <span className="font-semibold text-dc-text">{label}</span>
                <span className="text-[10px] text-dc-text-muted">Global default: {gStr}</span>
                <input
                    type="number"
                    min={min}
                    max={max}
                    step={step === "int" ? 1 : step}
                    placeholder="Leave empty to inherit"
                    className="border border-dc-border rounded px-2 py-1 bg-dc-bg-secondary text-dc-text max-w-xs"
                    value={str}
                    disabled={overridesLoading}
                    onChange={e => {
                        const t = e.target.value.trim();
                        if (t === "") {
                            updatePatch(key, undefined);
                            return;
                        }
                        if (step === "int") {
                            updatePatch(key, clampInt(parseIntOr(t, 0), min, max));
                        } else {
                            const n = Number.parseFloat(t);
                            if (!Number.isFinite(n)) return;
                            updatePatch(key, Math.min(max, Math.max(min, n)));
                        }
                    }}
                />
            </label>
        );
    };

    return (
        <div className="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm animate-in fade-in duration-200">
            <div className="bg-dc-bg border border-dc-border shadow-2xl rounded-2xl w-full max-w-4xl max-h-[90vh] overflow-y-auto animate-in zoom-in-95 duration-200">
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
                            onChange={e => setMigrate(e.target.checked)}
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

                    <div className="space-y-3 border-t border-dc-border pt-4">
                        <label className="text-[10px] font-bold text-dc-text-muted uppercase tracking-wider px-1">
                            Knowledge graph overrides (this vault)
                        </label>
                        <p className="text-[10px] text-dc-text-muted px-1 leading-relaxed">
                            Same tunables as Config &gt; Knowledge Graph. Leave a field empty to use the global default. Paged graph
                            (auto-paging) stays global-only.
                        </p>

                        {overridesLoading ? (
                            <p className="text-xs text-dc-text-muted px-1">Loading overrides...</p>
                        ) : (
                            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                {boolSelect("kms_graph_enable_semantic_clustering", "Semantic clustering")}
                                {boolSelect("kms_graph_enable_leiden_communities", "Leiden communities (experimental)")}
                                {boolSelect("kms_graph_enable_ai_beams", "AI insight beams")}
                                {boolSelect("kms_graph_enable_semantic_knn_edges", "Semantic kNN edges")}
                                {numField("kms_graph_k_means_max_k", "K-means max K", 2, 200, "int", "kms_graph_k_means_max_k")}
                                {numField("kms_graph_k_means_iterations", "K-means iterations", 1, 500, "int", "kms_graph_k_means_iterations")}
                                {numField(
                                    "kms_graph_ai_beam_max_nodes",
                                    "AI beam max nodes",
                                    2,
                                    50000,
                                    "int",
                                    "kms_graph_ai_beam_max_nodes"
                                )}
                                {numField(
                                    "kms_graph_ai_beam_similarity_threshold",
                                    "AI beam similarity (0-1)",
                                    0,
                                    1,
                                    0.01,
                                    "kms_graph_ai_beam_similarity_threshold"
                                )}
                                {numField("kms_graph_ai_beam_max_edges", "Max AI beam edges", 0, 500, "int", "kms_graph_ai_beam_max_edges")}
                                {numField(
                                    "kms_graph_semantic_max_notes",
                                    "Max notes for semantics (0 = no cap)",
                                    0,
                                    1000000,
                                    "int",
                                    "kms_graph_semantic_max_notes"
                                )}
                                {numField(
                                    "kms_graph_warn_note_threshold",
                                    "Large-vault warning at note count (0 = off)",
                                    0,
                                    1000000,
                                    "int",
                                    "kms_graph_warn_note_threshold"
                                )}
                                {numField(
                                    "kms_graph_beam_max_pair_checks",
                                    "AI beam max pair checks (0 = unlimited)",
                                    0,
                                    50000000,
                                    "int",
                                    "kms_graph_beam_max_pair_checks"
                                )}
                                {numField(
                                    "kms_graph_semantic_knn_per_note",
                                    "kNN neighbors per note",
                                    1,
                                    30,
                                    "int",
                                    "kms_graph_semantic_knn_per_note"
                                )}
                                {numField(
                                    "kms_graph_semantic_knn_min_similarity",
                                    "kNN min similarity (0.5-0.999)",
                                    0.5,
                                    0.999,
                                    0.01,
                                    "kms_graph_semantic_knn_min_similarity"
                                )}
                                {numField(
                                    "kms_graph_semantic_knn_max_edges",
                                    "Max kNN edges per build",
                                    0,
                                    500000,
                                    "int",
                                    "kms_graph_semantic_knn_max_edges"
                                )}
                                {numField(
                                    "kms_graph_semantic_knn_max_pair_checks",
                                    "kNN max pair checks (0 = unlimited)",
                                    0,
                                    50000000,
                                    "int",
                                    "kms_graph_semantic_knn_max_pair_checks"
                                )}
                                {numField(
                                    "kms_graph_pagerank_iterations",
                                    "PageRank iterations (global graph)",
                                    4,
                                    500,
                                    "int",
                                    "kms_graph_pagerank_iterations"
                                )}
                                {numField(
                                    "kms_graph_pagerank_local_iterations",
                                    "PageRank iterations (local graph)",
                                    4,
                                    500,
                                    "int",
                                    "kms_graph_pagerank_local_iterations"
                                )}
                                {numField(
                                    "kms_graph_pagerank_damping",
                                    "PageRank damping (0.5-0.99)",
                                    0.5,
                                    0.99,
                                    0.01,
                                    "kms_graph_pagerank_damping"
                                )}
                                <label className="flex flex-col gap-1 text-xs md:col-span-2">
                                    <span className="font-medium text-dc-text">PageRank scope (global graph)</span>
                                    <span className="text-[10px] text-dc-text-muted">
                                        Empty = use app default. auto / full_vault / page_subgraph / off
                                    </span>
                                    <select
                                        className="rounded border border-dc-border bg-dc-bg px-2 py-1 text-xs text-dc-text max-w-md"
                                        value={
                                            typeof vaultPatch.kms_graph_pagerank_scope === "string"
                                                ? (vaultPatch.kms_graph_pagerank_scope as string)
                                                : ""
                                        }
                                        onChange={(e) => {
                                            const v = e.target.value.trim();
                                            updatePatch("kms_graph_pagerank_scope", v === "" ? undefined : v);
                                        }}
                                    >
                                        <option value="">(inherit global)</option>
                                        <option value="auto">auto</option>
                                        <option value="full_vault">full_vault</option>
                                        <option value="page_subgraph">page_subgraph</option>
                                        <option value="off">off</option>
                                    </select>
                                </label>
                                {boolSelect(
                                    "kms_graph_background_wiki_pagerank_enabled",
                                    "Background materialized wiki PageRank after vault sync"
                                )}
                            </div>
                        )}

                        <div className="space-y-3 border-t border-dc-border pt-4">
                            <label className="text-[10px] font-bold text-dc-text-muted uppercase tracking-wider px-1">
                                Search and embedding overrides (this vault)
                            </label>
                            <p className="text-[10px] text-dc-text-muted px-1 leading-relaxed">
                                Hybrid/semantic search minimum vector similarity and optional chunking for queries and note indexing. Empty fields inherit global Config values.
                            </p>
                            {overridesLoading ? null : (
                                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                    {numField(
                                        "kms_search_min_similarity",
                                        "Min vector similarity (0-1, 0=off)",
                                        0,
                                        1,
                                        0.01,
                                        "kms_search_min_similarity"
                                    )}
                                    {boolSelect("kms_embedding_chunk_enabled", "Chunk long text before embedding")}
                                    {numField(
                                        "kms_embedding_chunk_max_chars",
                                        "Max chars per chunk (256-8192)",
                                        256,
                                        8192,
                                        "int",
                                        "kms_embedding_chunk_max_chars"
                                    )}
                                    {numField(
                                        "kms_embedding_chunk_overlap_chars",
                                        "Chunk overlap (chars)",
                                        0,
                                        4096,
                                        "int",
                                        "kms_embedding_chunk_overlap_chars"
                                    )}
                                </div>
                            )}
                        </div>

                        <details className="rounded-lg border border-dc-border bg-dc-bg-secondary/40 px-3 py-2 text-xs">
                            <summary className="cursor-pointer font-medium text-dc-text-muted flex items-center gap-1 list-none">
                                <ChevronDown size={14} className="inline" /> JSON preview (saved payload)
                            </summary>
                            <pre className="mt-2 text-[10px] font-mono text-dc-text/90 whitespace-pre-wrap break-all max-h-40 overflow-y-auto">
                                {patchJsonPreview}
                            </pre>
                        </details>

                        <div className="flex flex-wrap gap-2 pt-1">
                            <Button
                                type="button"
                                variant="secondary"
                                size="sm"
                                className="text-xs"
                                disabled={overridesLoading || overridesSaving}
                                onClick={async () => {
                                    setOverridesSaving(true);
                                    try {
                                        await getTaurpc().kms_set_vault_graph_overrides_json(patchJsonPreview);
                                        toast({
                                            title: "Graph overrides saved",
                                            description: "Applied for this vault on the next graph build.",
                                        });
                                    } catch (err) {
                                        toast({
                                            title: "Invalid or save failed",
                                            description: formatIpcOrRaw(err),
                                            variant: "destructive",
                                        });
                                    } finally {
                                        setOverridesSaving(false);
                                    }
                                }}
                            >
                                Save graph overrides
                            </Button>
                            <Button
                                type="button"
                                variant="ghost"
                                size="sm"
                                className="text-xs text-dc-text-muted"
                                disabled={overridesLoading || overridesSaving}
                                onClick={() => setVaultPatch({})}
                            >
                                Reset form (inherit all)
                            </Button>
                            <Button
                                type="button"
                                variant="ghost"
                                size="sm"
                                className="text-xs text-dc-text-muted"
                                disabled={overridesLoading || overridesSaving}
                                onClick={async () => {
                                    setOverridesSaving(true);
                                    try {
                                        await getTaurpc().kms_clear_vault_graph_overrides_json();
                                        setVaultPatch({});
                                        toast({
                                            title: "Saved overrides removed",
                                            description: "This vault now uses global Knowledge Graph settings.",
                                        });
                                    } catch (err) {
                                        toast({
                                            title: "Clear failed",
                                            description: formatIpcOrRaw(err),
                                            variant: "destructive",
                                        });
                                    } finally {
                                        setOverridesSaving(false);
                                    }
                                }}
                            >
                                Remove saved overrides
                            </Button>
                        </div>
                    </div>
                </div>

                <div className="p-4 bg-dc-bg-secondary/30 border-t border-dc-border flex justify-end gap-3">
                    <Button variant="ghost" size="sm" onClick={onClose} className="text-dc-text-muted hover:bg-dc-bg-hover">
                        Cancel
                    </Button>
                    <Button
                        size="sm"
                        className="bg-dc-accent hover:bg-dc-accent/90 text-white gap-2 px-6 min-w-[100px]"
                        onClick={handleSaveVaultPath}
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
