import React, { useState, useEffect, useRef, lazy, Suspense, useCallback, useMemo } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { listen } from "@tauri-apps/api/event";
import { getTaurpc } from "./lib/taurpc";
import { resolveTheme, applyThemeToDocument } from "./lib/theme";
import { Toaster } from "./components/ui/toaster";
import { useToast } from "./components/ui/use-toast";
import { Book, FolderOpen, Search, Settings, Plus, Star, FileText, Sun, Moon, AlertCircle, RefreshCw, Check, Terminal, Activity, Cpu, Maximize2, Minimize2, Network, PanelLeft, Clock, GripVertical, Filter, LayoutTemplate } from "lucide-react";
import { Button } from "./components/ui/button";
import KmsEditor from "./components/kms/KmsEditor";
import KmsLogViewer from "./components/kms/KmsLogViewer";
import KmsHistoryBrowser from "./components/kms/KmsHistoryBrowser";
import VaultSettingsModal from "./components/modals/VaultSettingsModal";
import { ViewFull } from "./components/modals/ViewFull";
const ImageViewerModal = lazy(() =>
    import("./components/modals/ImageViewerModal").then((m) => ({ default: m.ImageViewerModal }))
);
import FileExplorer from "./components/kms/FileExplorer";
import KmsTemplateGalleryModal from "./components/kms/KmsTemplateGalleryModal";
import KmsAssetTray from "./components/kms/KmsAssetTray";
import KmsCommandPalette, { type KmsCommandPaletteView } from "./components/kms/KmsCommandPalette";
import SkillHub from "./components/kms/SkillHub";
import SkillEditor from "./components/kms/SkillEditor";
import KmsReindexProgressBadge from "./components/kms/KmsReindexProgressBadge";
const KmsGraph = lazy(() => import("./components/kms/KmsGraph"));
const KmsGraph3D = lazy(() => import("./components/kms/KmsGraph3D"));
import { KmsNoteDto, KmsFileSystemItemDto, KmsLogDto, SkillDto, AppStateDto, SearchResultDto } from "./bindings";
import { ClipEntry } from "./types";
import { formatIpcOrRaw } from "./lib/ipcError";
import {
    estimateWeightedEtaMs,
    loadProviderDurationHistory,
    recordProviderDuration,
    vaultSizeTierFromNoteCount,
} from "./lib/kmsReindexEta";
import {
    readLegacyRecentNotePathsFromLocalStorage,
    clearLegacyRecentNotePathsFromLocalStorage,
    recordRecentNotePath,
    normalizeKmsNotePathForLookup,
    KMS_RECENT_NOTES_MAX,
} from "./lib/kmsRecentNotes";
import {
    readLegacyFavoritePathOrderFromLocalStorage,
    clearLegacyFavoritePathOrderFromLocalStorage,
    sortFavoriteNotes,
    pruneFavoriteOrder,
} from "./lib/kmsFavoriteManualOrder";
import {
    fetchKmsRecentPathsFromDb,
    fetchKmsFavoritePathOrderFromDb,
    persistKmsRecentPaths,
    persistKmsFavoritePathOrder,
} from "./lib/kmsSidebarStateDb";
import {
    defaultKmsSearchClientFilters,
    filterSearchResults,
    parseInputDateToDay,
    searchEmbeddingDiagFromResults,
    type KmsSearchClientFilters,
} from "./lib/kmsSearchResultFilter";
import { kmsVaultRelativePath } from "./lib/kmsVaultRelPath";
import { resolveNoteFromWikiTarget } from "./lib/kmsWikiResolve";

export default function KmsApp() {
    const { toast } = useToast();
    const [vaultPath, setVaultPath] = useState<string | null>(null);
    const [initializing, setInitializing] = useState(true);
    const [notes, setNotes] = useState<KmsNoteDto[]>([]);
    const [activeNote, setActiveNote] = useState<KmsNoteDto | null>(null);
    const [activeContent, setActiveContent] = useState<string>("");
    /** Read-only split pane: second note while editing the active one. */
    const [referenceNote, setReferenceNote] = useState<KmsNoteDto | null>(null);
    const [referenceContent, setReferenceContent] = useState<string | null>(null);
    const [theme, setTheme] = useState<"light" | "dark">("light");
    const [themeOverride, setThemeOverride] = useState<"light" | "dark" | null>(null);
    const [view, setView] = useState<"explorer" | "search" | "favorites" | "recents" | "logs" | "skills" | "graph">("explorer");
    const [activeSkill, setActiveSkill] = useState<SkillDto | null>(null);
    const [isSkillEditorOpen, setIsSkillEditorOpen] = useState(false);
    const [isSkillDirty, setIsSkillDirty] = useState(false);
    const [skillRefreshKey, setSkillRefreshKey] = useState(0);
    const [searchQuery, setSearchQuery] = useState("");
    const [searchResults, setSearchResults] = useState<SearchResultDto[]>([]);
    const [searchLoading, setSearchLoading] = useState(false);
    const [searchMode, setSearchMode] = useState<"Hybrid" | "Semantic" | "Keyword">("Hybrid");
    const [kmsSearchDefaultLimit, setKmsSearchDefaultLimit] = useState(20);
    const [kmsSearchIncludeEmbeddingDiagnostics, setKmsSearchIncludeEmbeddingDiagnostics] = useState(false);
    const [searchClientFilters, setSearchClientFilters] = useState<KmsSearchClientFilters>(() =>
        defaultKmsSearchClientFilters()
    );
    const [searchDateFromInput, setSearchDateFromInput] = useState("");
    const [searchDateToInput, setSearchDateToInput] = useState("");
    const [isSettingsOpen, setIsSettingsOpen] = useState(false);
    const [syncStatus, setSyncStatus] = useState<string>("Idle");
    const [viewFullVisible, setViewFullVisible] = useState(false);
    const [viewFullContent, setViewFullContent] = useState("");
    const [viewFullEditMeta, setViewFullEditMeta] = useState<{ category: string; snippetIdx: number } | null>(null);
    const [viewFullClipboardMeta, setViewFullClipboardMeta] = useState<{ id: number; canPromote: boolean; trigger?: string } | null>(null);
    const [imageViewerVisible, setImageViewerVisible] = useState(false);
    const [imageViewerCurrent, setImageViewerCurrent] = useState<ClipEntry | null>(null);
    const [imageViewerContext, setImageViewerContext] = useState<ClipEntry[]>([]);
    const searchAbortController = useRef<AbortController | null>(null);
    const indexedNoteCountRef = useRef(0);
    const [vaultStructure, setVaultStructure] = useState<KmsFileSystemItemDto | null>(null);
    const [explorerTreeFilter, setExplorerTreeFilter] = useState("");
    const [explorerTagFilter, setExplorerTagFilter] = useState("");
    const [templateGalleryOpen, setTemplateGalleryOpen] = useState(false);
    const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);
    const [explorerBulkMode, setExplorerBulkMode] = useState(false);
    const [explorerBulkPaths, setExplorerBulkPaths] = useState<Set<string>>(() => new Set());
    const editorInsertTokenRef = useRef(0);
    const [editorMarkdownInsert, setEditorMarkdownInsert] = useState<{ id: number; text: string } | null>(null);
    const [recentPaths, setRecentPaths] = useState<string[]>([]);
    const [favoritePathOrder, setFavoritePathOrder] = useState<string[]>([]);
    const [sidebarWidth, setSidebarWidth] = useState(() => {
        const saved = localStorage.getItem("kms-sidebar-width");
        return saved ? Number.parseInt(saved) : 280;
    });
    const [isResizing, setIsResizing] = useState(false);
    const [isZenMode, setIsZenMode] = useState(() => {
        const saved = localStorage.getItem("kms-zen-mode");
        return saved === "true";
    });
    const [isHistoryOpen, setIsHistoryOpen] = useState(false);
    const [historyPanelWidth, setHistoryPanelWidth] = useState(() => {
        const saved = localStorage.getItem("kms-history-panel-width");
        const n = saved ? Number.parseInt(saved, 10) : 320;
        return Number.isFinite(n) ? Math.max(260, Math.min(920, n)) : 320;
    });
    const [isResizingHistory, setIsResizingHistory] = useState(false);
    const [graphMode, setGraphMode] = useState<"2d" | "3d">("3d");
    const [graphResetKey, setGraphResetKey] = useState(0);
    const [graphNavigateRequest, setGraphNavigateRequest] = useState<{ token: number; path: string } | null>(null);
    const graphNavigateTokenRef = useRef(0);
    const editorWorkingCopyMarkdownRef = useRef<(() => string) | null>(null);
    const [indexedNoteCount, setIndexedNoteCount] = useState(0);
    const [reindexProgress, setReindexProgress] = useState<{
        requestId: string;
        providerId: string;
        phase: "start" | "progress" | "end";
        startedAtMs: number;
        emittedAtMs: number;
        elapsedMs: number;
        etaRemainingMs: number | null;
        providerIndex: number;
        providerTotal: number;
        providerIndexedCount: number;
        indexedTotalSoFar: number;
        succeeded: boolean | null;
        error: string | null;
    } | null>(null);
    const providerStartElapsedRef = useRef<Record<string, number>>({});

    const applyKmsSearchDefaultsFromDto = useCallback((app: AppStateDto) => {
        const rawMode = (app.kms_search_default_mode ?? "Hybrid").trim();
        if (rawMode === "Semantic" || rawMode === "Keyword") {
            setSearchMode(rawMode);
        } else {
            setSearchMode("Hybrid");
        }
        setKmsSearchDefaultLimit(
            Math.min(200, Math.max(1, app.kms_search_default_limit ?? 20))
        );
        setKmsSearchIncludeEmbeddingDiagnostics(Boolean(app.kms_search_include_embedding_diagnostics));
    }, []);

    const currentTheme = themeOverride || theme;
    const formatKmsError = useCallback((error: unknown) => formatIpcOrRaw(error), []);

    useEffect(() => {
        indexedNoteCountRef.current = indexedNoteCount;
    }, [indexedNoteCount]);

    const toggleTheme = useCallback(() => {
        setThemeOverride(currentTheme === "dark" ? "light" : "dark");
    }, [currentTheme]);

    const toggleZenMode = useCallback(() => {
        setIsZenMode(prev => {
            const next = !prev;
            localStorage.setItem("kms-zen-mode", String(next));
            return next;
        });
    }, []);

    useEffect(() => {
        const init = async () => {
            try {
                const path = await getTaurpc().kms_initialize();
                setVaultPath(path);
                refreshNotes();
                refreshStructure();
                refreshIndexedNoteCount();

                try {
                    const app = await getTaurpc().get_app_state();
                    applyKmsSearchDefaultsFromDto(app);
                } catch {
                    /* keep defaults */
                }

                try {
                    const [recentFromDb, favoriteOrderFromDb] = await Promise.all([
                        fetchKmsRecentPathsFromDb(),
                        fetchKmsFavoritePathOrderFromDb(),
                    ]);

                    let recentInit = recentFromDb;
                    if (recentInit.length === 0) {
                        const legacy = readLegacyRecentNotePathsFromLocalStorage();
                        if (legacy.length > 0) {
                            recentInit = legacy.slice(0, KMS_RECENT_NOTES_MAX);
                            await persistKmsRecentPaths(recentInit);
                            clearLegacyRecentNotePathsFromLocalStorage();
                        }
                    }
                    setRecentPaths(recentInit);

                    let favInit = favoriteOrderFromDb;
                    if (favInit.length === 0) {
                        const legacyFav = readLegacyFavoritePathOrderFromLocalStorage();
                        if (legacyFav.length > 0) {
                            favInit = legacyFav;
                            await persistKmsFavoritePathOrder(favInit);
                            clearLegacyFavoritePathOrderFromLocalStorage();
                        }
                    }
                    setFavoritePathOrder(favInit);
                } catch {
                    /* keep empty sidebar lists */
                }

                // Initialize theme from global settings
                const globalThemePref = localStorage.getItem("digicore-theme") || "light";
                setTheme(resolveTheme(globalThemePref));
            } catch (error) {
                toast({
                    title: "KMS Initialization Error",
                    description: formatKmsError(error),
                    variant: "destructive",
                });
            } finally {
                setInitializing(false);
            }
        };
        init();

        // Listen for global theme changes
        const unlistenPromise = listen("digicore-theme-changed", (event: any) => {
            const { theme: newResolvedTheme } = event.payload;
            if (newResolvedTheme) {
                setTheme(newResolvedTheme);
            }
        });

        // Listen for sync events
        const unlistenSyncStatus = listen("kms-sync-status", (event: any) => {
            setSyncStatus(event.payload as string);
        });
        const unlistenSyncComplete = listen("kms-sync-complete", () => {
            refreshNotes();
            refreshStructure();
            refreshIndexedNoteCount();
            setReindexProgress(null);
        });
        const unlistenReindexProviderProgress = listen("kms-reindex-provider-progress", (event: any) => {
            const payload = (event.payload ?? {}) as {
                request_id?: string;
                provider_id?: string;
                phase?: string;
                started_at_ms?: number;
                emitted_at_ms?: number;
                elapsed_ms?: number;
                eta_remaining_ms?: number | null;
                provider_index?: number;
                provider_total?: number;
                provider_indexed_count?: number;
                indexed_total_so_far?: number;
                succeeded?: boolean | null;
                error?: string | null;
            };
            const phase = payload.phase === "start" || payload.phase === "progress" || payload.phase === "end"
                ? payload.phase
                : "progress";
            const providerId = payload.provider_id ?? "unknown";
            const runElapsedMs = typeof payload.elapsed_ms === "number" ? payload.elapsed_ms : 0;
            if (phase === "start") {
                providerStartElapsedRef.current[providerId] = runElapsedMs;
            }
            const providerStartElapsed = providerStartElapsedRef.current[providerId] ?? runElapsedMs;
            const providerElapsedMs = Math.max(0, runElapsedMs - providerStartElapsed);
            let history = loadProviderDurationHistory();
            const tier = vaultSizeTierFromNoteCount(indexedNoteCountRef.current);
            if (phase === "end") {
                history = recordProviderDuration(providerId, providerElapsedMs, tier);
                delete providerStartElapsedRef.current[providerId];
            }
            const weightedEtaMs = estimateWeightedEtaMs(
                {
                    providerId,
                    phase,
                    providerIndex: typeof payload.provider_index === "number" ? payload.provider_index : 0,
                    providerTotal: typeof payload.provider_total === "number" ? payload.provider_total : 0,
                    elapsedMs: runElapsedMs,
                },
                providerElapsedMs,
                history,
                tier
            );
            setReindexProgress({
                requestId: payload.request_id ?? "",
                providerId,
                phase,
                startedAtMs: typeof payload.started_at_ms === "number" ? payload.started_at_ms : 0,
                emittedAtMs: typeof payload.emitted_at_ms === "number" ? payload.emitted_at_ms : 0,
                elapsedMs: typeof payload.elapsed_ms === "number" ? payload.elapsed_ms : 0,
                etaRemainingMs: weightedEtaMs,
                providerIndex: typeof payload.provider_index === "number" ? payload.provider_index : 0,
                providerTotal: typeof payload.provider_total === "number" ? payload.provider_total : 0,
                providerIndexedCount:
                    typeof payload.provider_indexed_count === "number" ? payload.provider_indexed_count : 0,
                indexedTotalSoFar:
                    typeof payload.indexed_total_so_far === "number" ? payload.indexed_total_so_far : 0,
                succeeded:
                    typeof payload.succeeded === "boolean" ? payload.succeeded : null,
                error: typeof payload.error === "string" ? payload.error : null,
            });
        });
        const unlistenReindexComplete = listen("kms-reindex-complete", (event: any) => {
            const payload = (event.payload ?? {}) as {
                request_id?: string;
                indexed_total?: number;
                provider_failures?: string[];
                succeeded?: boolean;
            };
            const failures = Array.isArray(payload.provider_failures) ? payload.provider_failures : [];
            const indexed = typeof payload.indexed_total === "number" ? payload.indexed_total : 0;
            if (payload.succeeded) {
                toast({
                    title: "Reindex Complete",
                    description: `Indexed ${indexed} items.`,
                });
            } else {
                toast({
                    title: "Reindex Completed with Warnings",
                    description:
                        failures.length > 0
                            ? `Indexed ${indexed} items. Failures: ${failures.join("; ")}`
                            : `Indexed ${indexed} items with one or more provider issues.`,
                    variant: "destructive",
                });
            }
            refreshNotes();
            refreshStructure();
            refreshIndexedNoteCount();
            providerStartElapsedRef.current = {};
            setReindexProgress(null);
        });
        const unlistenWikiPr = listen("kms-wiki-pagerank-ready", (event) => {
            window.dispatchEvent(new Event("kms-wiki-pagerank-ready"));
            const raw = (event as { payload?: unknown }).payload;
            const n =
                typeof raw === "number"
                    ? raw
                    : typeof raw === "bigint"
                      ? Number(raw)
                      : typeof raw === "string"
                        ? parseInt(raw, 10)
                        : NaN;
            toast({
                title: "Wiki PageRank ready",
                description:
                    Number.isFinite(n) && n > 0
                        ? `Updated materialized scores for ${n} notes.`
                        : "Materialized link centrality is up to date.",
            });
        });

        const unlistenAppState = listen("digicore-app-state-changed", () => {
            getTaurpc()
                .get_app_state()
                .then(applyKmsSearchDefaultsFromDto)
                .catch(() => {});
        });

        return () => {
            unlistenPromise.then(f => f());
            unlistenSyncStatus.then(f => f());
            unlistenSyncComplete.then(f => f());
            unlistenReindexProviderProgress.then(f => f());
            unlistenReindexComplete.then(f => f());
            unlistenWikiPr.then(f => f());
            unlistenAppState.then(f => f());
        };
    }, [toast, applyKmsSearchDefaultsFromDto, formatKmsError]);

    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            if (e.altKey && e.key.toLowerCase() === 'z') {
                e.preventDefault();
                toggleZenMode();
            }
        };
        window.addEventListener('keydown', handleKeyDown);
        return () => window.removeEventListener('keydown', handleKeyDown);
    }, [toggleZenMode]);

    const refreshNotes = async () => {
        try {
            const list = await getTaurpc().kms_list_notes();
            setNotes(list);
        } catch (error) {
            console.error("Failed to list notes:", error);
        }
    };

    const favoriteNotes = useMemo(
        () => sortFavoriteNotes(notes, favoritePathOrder),
        [notes, favoritePathOrder]
    );

    const recentNotes = useMemo(() => {
        const byKey = new Map(notes.map((n) => [normalizeKmsNotePathForLookup(n.path), n]));
        const out: KmsNoteDto[] = [];
        const seen = new Set<string>();
        for (const p of recentPaths) {
            const n = byKey.get(normalizeKmsNotePathForLookup(p));
            if (n && !seen.has(n.path)) {
                seen.add(n.path);
                out.push(n);
            }
        }
        return out;
    }, [recentPaths, notes]);

    const noteByPathForSearch = useMemo(() => new Map(notes.map((n) => [n.path, n])), [notes]);

    const mergedSearchFilters = useMemo(
        () => ({
            ...searchClientFilters,
            dateFromDay: parseInputDateToDay(searchDateFromInput),
            dateToDay: parseInputDateToDay(searchDateToInput),
        }),
        [searchClientFilters, searchDateFromInput, searchDateToInput]
    );

    const filteredSearchResults = useMemo(
        () => filterSearchResults(searchResults, mergedSearchFilters, noteByPathForSearch),
        [searchResults, mergedSearchFilters, noteByPathForSearch]
    );

    const searchEmbeddingDiagBanner = useMemo(() => {
        if (!kmsSearchIncludeEmbeddingDiagnostics) return null;
        return searchEmbeddingDiagFromResults(searchResults);
    }, [kmsSearchIncludeEmbeddingDiagnostics, searchResults]);

    useEffect(() => {
        if (notes.length === 0) return;
        const byKey = new Map(
            notes.map((n) => [normalizeKmsNotePathForLookup(n.path), n.path])
        );
        setRecentPaths((prev) => {
            const next = prev
                .map((p) => byKey.get(normalizeKmsNotePathForLookup(p)))
                .filter((p): p is string => p != null);
            if (next.length !== prev.length) void persistKmsRecentPaths(next);
            return next;
        });
    }, [notes]);

    useEffect(() => {
        if (notes.length === 0) return;
        const favSet = new Set(notes.filter((n) => n.is_favorite).map((n) => n.path));
        setFavoritePathOrder((prev) => {
            const next = pruneFavoriteOrder(prev, favSet);
            if (next.length !== prev.length) void persistKmsFavoritePathOrder(next);
            return next;
        });
    }, [notes]);

    useEffect(() => {
        if (view !== "favorites" && view !== "recents") return;
        void (async () => {
            try {
                const list = await getTaurpc().kms_list_notes();
                setNotes(list);
            } catch (error) {
                console.error("Failed to list notes for sidebar list view:", error);
            }
        })();
    }, [view]);

    const applyNoteFavoriteAtPath = useCallback(
        async (path: string, wantFavorite: boolean) => {
            try {
                await getTaurpc().kms_set_note_favorite(path, wantFavorite);
                const list = await getTaurpc().kms_list_notes();
                setNotes(list);
                setFavoritePathOrder((prev) => {
                    let nextOrder: string[];
                    if (wantFavorite) {
                        nextOrder = prev.includes(path) ? prev : [...prev, path];
                    } else {
                        nextOrder = prev.filter((p) => p !== path);
                    }
                    void persistKmsFavoritePathOrder(nextOrder);
                    return nextOrder;
                });
                setActiveNote((prev) => {
                    if (!prev || prev.path !== path) return prev;
                    const updated = list.find((n) => n.path === path);
                    return updated ?? { ...prev, is_favorite: wantFavorite };
                });
            } catch (err) {
                toast({
                    title: "Favorite update failed",
                    description: formatKmsError(err),
                    variant: "destructive",
                });
            }
        },
        [formatKmsError, toast]
    );

    const handleFavoriteReorder = useCallback(
        (fromIndex: number, toIndex: number) => {
            if (fromIndex === toIndex) return;
            const paths = sortFavoriteNotes(notes, favoritePathOrder).map((n) => n.path);
            const next = [...paths];
            const [removed] = next.splice(fromIndex, 1);
            next.splice(toIndex, 0, removed);
            setFavoritePathOrder(next);
            void persistKmsFavoritePathOrder(next);
        },
        [notes, favoritePathOrder]
    );

    const handleNoteFavoriteChange = useCallback(
        async (next: boolean) => {
            if (!activeNote) return;
            await applyNoteFavoriteAtPath(activeNote.path, next);
        },
        [activeNote, applyNoteFavoriteAtPath]
    );

    const refreshIndexedNoteCount = async () => {
        try {
            const diag = await getTaurpc().kms_get_diagnostics();
            setIndexedNoteCount(diag.note_count);
        } catch (error) {
            console.error("Failed to load KMS diagnostics for note count:", error);
        }
    };

    useEffect(() => {
        if (!isResizing) return;

        const handleMouseMove = (e: MouseEvent) => {
            const newWidth = Math.max(200, Math.min(600, e.clientX));
            setSidebarWidth(newWidth);
        };

        const handleMouseUp = () => {
            setIsResizing(false);
            localStorage.setItem("kms-sidebar-width", sidebarWidth.toString());
        };

        window.addEventListener("mousemove", handleMouseMove);
        window.addEventListener("mouseup", handleMouseUp);
        return () => {
            window.removeEventListener("mousemove", handleMouseMove);
            window.removeEventListener("mouseup", handleMouseUp);
        };
    }, [isResizing, sidebarWidth]);

    const historyPanelWidthRef = useRef(historyPanelWidth);
    historyPanelWidthRef.current = historyPanelWidth;

    const endHistoryPanelResize = useCallback(() => {
        setIsResizingHistory(false);
        document.body.style.cursor = "";
        document.body.style.userSelect = "";
        localStorage.setItem("kms-history-panel-width", String(historyPanelWidthRef.current));
    }, []);

    const applyHistoryPanelWidth = useCallback((w: number) => {
        const n = Math.round(Number(w));
        const clamped = Number.isFinite(n) ? Math.max(260, Math.min(920, n)) : historyPanelWidthRef.current;
        setHistoryPanelWidth(clamped);
        historyPanelWidthRef.current = clamped;
    }, []);

    const persistHistoryPanelWidth = useCallback(
        (w: number) => {
            applyHistoryPanelWidth(w);
            localStorage.setItem("kms-history-panel-width", String(historyPanelWidthRef.current));
        },
        [applyHistoryPanelWidth]
    );

    /**
     * React onPointerDown only (no useLayoutEffect): effect cleanup was removing listeners mid-drag and
     * historyResizeHandleRef was never attached to the DOM, so node was always null. Native listeners
     * added from pointerdown use applyHistoryPanelWidth (not persist) until pointerup.
     */
    const handleHistoryResizePointerDown = useCallback(
        (e: React.PointerEvent<HTMLDivElement>) => {
            if (e.button !== 0) return;
            e.preventDefault();
            e.stopPropagation();

            const node = e.currentTarget;
            const startX = e.clientX;
            const startW = historyPanelWidthRef.current;
            if (!Number.isFinite(startX) || !Number.isFinite(startW)) return;

            const attachWindowDrag = () => {
                setIsResizingHistory(true);
                document.body.style.cursor = "col-resize";
                document.body.style.userSelect = "none";
                const onMove = (ev: MouseEvent) => {
                    ev.preventDefault();
                    const delta = startX - ev.clientX;
                    applyHistoryPanelWidth(startW + delta);
                };
                const onUp = () => {
                    window.removeEventListener("mousemove", onMove, true);
                    window.removeEventListener("mouseup", onUp, true);
                    endHistoryPanelResize();
                };
                window.addEventListener("mousemove", onMove, true);
                window.addEventListener("mouseup", onUp, true);
            };

            let captureOk = false;
            try {
                node.setPointerCapture(e.pointerId);
                captureOk = true;
            } catch {
                captureOk = false;
            }

            if (!captureOk) {
                attachWindowDrag();
                return;
            }

            setIsResizingHistory(true);
            document.body.style.cursor = "col-resize";
            document.body.style.userSelect = "none";

            const onPointerMove = (ev: PointerEvent) => {
                ev.preventDefault();
                const delta = startX - ev.clientX;
                applyHistoryPanelWidth(startW + delta);
            };

            const onPointerEnd = (ev: PointerEvent) => {
                node.removeEventListener("pointermove", onPointerMove);
                node.removeEventListener("pointerup", onPointerEnd);
                node.removeEventListener("pointercancel", onPointerEnd);
                try {
                    if (node.hasPointerCapture(ev.pointerId)) {
                        node.releasePointerCapture(ev.pointerId);
                    }
                } catch {
                    /* ignore */
                }
                endHistoryPanelResize();
            };

            node.addEventListener("pointermove", onPointerMove);
            node.addEventListener("pointerup", onPointerEnd);
            node.addEventListener("pointercancel", onPointerEnd);
        },
        [applyHistoryPanelWidth, endHistoryPanelResize]
    );

    const refreshStructure = async () => {
        try {
            const structure = await getTaurpc().kms_get_vault_structure();
            setVaultStructure(structure);
        } catch (error) {
            console.error("Failed to get vault structure:", error);
        }
    };

    useEffect(() => {
        if (notes.length > indexedNoteCount) {
            setIndexedNoteCount(notes.length);
        }
    }, [notes, indexedNoteCount]);

    const checkUnsavedSkillChanges = () => {
        if (isSkillEditorOpen && isSkillDirty) {
            return window.confirm("You have unsaved changes in the Skill Editor. Discard changes and continue?");
        }
        return true;
    };

    const handleCommandPaletteJump = (v: KmsCommandPaletteView) => {
        if (!checkUnsavedSkillChanges()) return;
        setIsSkillEditorOpen(false);
        setActiveNote(null);
        setView(v);
    };

    useEffect(() => {
        const onKey = (e: KeyboardEvent) => {
            if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
                const t = e.target as HTMLElement | null;
                if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable)) {
                    return;
                }
                e.preventDefault();
                setCommandPaletteOpen(true);
            }
        };
        window.addEventListener("keydown", onKey);
        return () => window.removeEventListener("keydown", onKey);
    }, []);

    const handleSelectNote = async (note: KmsNoteDto) => {
        if (!checkUnsavedSkillChanges()) return;
        try {
            const content = await getTaurpc().kms_load_note(note.path);
            setActiveContent(content);
            setActiveNote(note);
            setIsSkillEditorOpen(false);
            setRecentPaths((prev) => {
                const next = recordRecentNotePath(prev, note.path);
                void persistKmsRecentPaths(next);
                return next;
            });
        } catch (error) {
            toast({
                title: "Error Loading Note",
                description: formatKmsError(error),
                variant: "destructive",
            });
        }
    };

    const resolveNoteForWikiTarget = async (wikiTarget: string): Promise<KmsNoteDto | null> => {
        let note = resolveNoteFromWikiTarget(notes, wikiTarget);
        if (!note) {
            try {
                const latest = await getTaurpc().kms_list_notes();
                setNotes(latest);
                note = resolveNoteFromWikiTarget(latest, wikiTarget);
            } catch {
                /* ignore */
            }
        }
        if (note) return note;
        const title =
            wikiTarget.split(/[\\/]/).pop()?.replace(/\.md$/i, "") || wikiTarget;
        return { path: wikiTarget, title, tags: [] } as unknown as KmsNoteDto;
    };

    const handleWikiNavigateFromEditor = async (target: string) => {
        const note = await resolveNoteForWikiTarget(target);
        if (note) await handleSelectNote(note);
    };

    const handleOpenReferenceFromWiki = async (wikiTarget: string) => {
        const note = await resolveNoteForWikiTarget(wikiTarget);
        if (!note) return;
        if (activeNote && note.path === activeNote.path) {
            toast({
                title: "Already editing",
                description: "This note is already open in the editor.",
            });
            return;
        }
        try {
            const content = await getTaurpc().kms_load_note(note.path);
            setReferenceNote(note);
            setReferenceContent(content);
        } catch (error) {
            toast({
                title: "Reference note failed to load",
                description: formatKmsError(error),
                variant: "destructive",
            });
        }
    };

    const handleClearReferenceNote = () => {
        setReferenceNote(null);
        setReferenceContent(null);
    };

    const handleOpenNoteAsReference = async (note: KmsNoteDto) => {
        if (activeNote && note.path === activeNote.path) {
            toast({
                title: "Already editing",
                description: "Pick a different note for the reference pane.",
            });
            return;
        }
        try {
            const content = await getTaurpc().kms_load_note(note.path);
            setReferenceNote(note);
            setReferenceContent(content);
        } catch (error) {
            toast({
                title: "Reference note failed to load",
                description: formatKmsError(error),
                variant: "destructive",
            });
        }
    };

    useEffect(() => {
        if (!activeNote || !referenceNote) return;
        if (activeNote.path === referenceNote.path) {
            setReferenceNote(null);
            setReferenceContent(null);
        }
    }, [activeNote, referenceNote]);

    const openGlobalGraphForActiveNote = useCallback(() => {
        if (!activeNote) return;
        graphNavigateTokenRef.current += 1;
        setGraphNavigateRequest({ token: graphNavigateTokenRef.current, path: activeNote.path });
        setView("graph");
    }, [activeNote]);

    const handleOpenSkillEditor = async (filePath: string) => {
        // Path should be like .../skills/skill-name/SKILL.md
        const parts = filePath.split(/[\\/]/);
        // Find "skills" folder in the path to identify the skill name correctly
        const skillsIdx = parts.findIndex(p => p.toLowerCase() === "skills");

        if (skillsIdx !== -1 && skillsIdx + 1 < parts.length) {
            const skillName = parts[skillsIdx + 1];
            try {
                const skill = await getTaurpc().kms_get_skill(skillName);
                if (skill) {
                    setActiveSkill(skill);
                    setView("skills");
                    setIsSkillEditorOpen(true);
                } else {
                    toast({
                        title: "Skill Not Found",
                        description: `Could not find managed metadata for skill "${skillName}"`,
                        variant: "destructive"
                    });
                }
            } catch (error) {
                console.error("Failed to open skill editor:", error);
                toast({
                    title: "Navigation Error",
                    description: "Failed to load skill metadata from the database",
                    variant: "destructive"
                });
            }
        } else {
            toast({
                title: "Invalid Skill Path",
                description: "This SKILL.md does not appear to be within a managed skills directory.",
                variant: "default"
            });
        }
    };

    const handleCreateNote = async (parentPath?: string) => {
        if (!checkUnsavedSkillChanges()) return;
        if (!vaultPath) return;

        const targetDir = parentPath || `${vaultPath}\\notes`;
        const title = `Untitled Note ${notes.length + 1}`;
        const fileName = `${title}.md`;
        const path = `${targetDir}\\${fileName}`;
        const initialContent = `# ${title}\n\nStart writing here...`;

        try {
            await getTaurpc().kms_save_note(path, initialContent);
            await refreshStructure();
            await refreshNotes();
            // Find the newly created note in the list
            const newList = await getTaurpc().kms_list_notes();
            const newNote = newList.find(n => n.path === path);
            if (newNote) {
                handleSelectNote(newNote);
            }
        } catch (error) {
            toast({
                title: "Error Creating Note",
                description: formatKmsError(error),
                variant: "destructive",
            });
        }
    };

    const handleCreateFolder = async (parentPath?: string) => {
        if (!checkUnsavedSkillChanges()) return;
        if (!vaultPath) return;
        const targetParent = parentPath || vaultPath;
        const name = window.prompt("Enter notebook name:");
        if (!name) return;

        const path = `${targetParent}\\${name}`;
        try {
            await getTaurpc().kms_create_folder(path);
            refreshStructure();
            toast({
                title: "Folder Created",
                description: `Created notebook "${name}"`,
            });
        } catch (error) {
            toast({
                title: "Error Creating Folder",
                description: formatKmsError(error),
                variant: "destructive",
            });
        }
    };

    const handleSaveNote = async (content: string) => {
        if (!activeNote) return;
        try {
            await getTaurpc().kms_save_note(activeNote.path, content);
            setActiveContent(content);
            refreshNotes();
            refreshStructure();
            toast({
                title: "Note Saved",
                description: "Your changes have been persisted locally.",
            });
        } catch (error) {
            toast({
                title: "Save Failed",
                description: formatKmsError(error),
                variant: "destructive",
            });
        }
    };

    const handleDeleteNote = async (path?: string, options?: { skipConfirm?: boolean }) => {
        const targetPath = path || activeNote?.path;
        if (!targetPath) return;

        if (!options?.skipConfirm) {
            const confirmed = window.confirm(`Are you sure you want to delete this note?`);
            if (!confirmed) return;
        }

        try {
            await getTaurpc().kms_delete_note(targetPath);
            if (activeNote?.path === targetPath) {
                setActiveNote(null);
                setActiveContent("");
            }
            if (referenceNote?.path === targetPath) {
                setReferenceNote(null);
                setReferenceContent(null);
            }
            refreshNotes();
            refreshStructure();
            toast({
                title: "Note Deleted",
                description: "The note and its local file have been removed.",
            });
        } catch (error) {
            toast({
                title: "Delete Failed",
                description: formatKmsError(error),
                variant: "destructive",
            });
        }
    };

    const handleRenameNote = async (newName: string, oldPath?: string) => {
        const targetPath = oldPath || activeNote?.path;
        if (!targetPath) return;

        try {
            const newPath = await getTaurpc().kms_rename_note(targetPath, newName);
            if (activeNote?.path === targetPath) {
                const updatedNote = { ...activeNote, path: newPath, title: newName.replace(/\.md$/i, "") };
                setActiveNote(updatedNote);
            }
            if (referenceNote?.path === targetPath) {
                setReferenceNote({
                    ...referenceNote,
                    path: newPath,
                    title: newName.replace(/\.md$/i, ""),
                });
            }
            await refreshNotes();
            await refreshStructure();
            toast({
                title: "Note Renamed",
                description: `Successfully renamed to ${newName}`,
            });
            return newPath;
        } catch (error) {
            toast({
                title: "Rename Failed",
                description: formatKmsError(error),
                variant: "destructive",
            });
            throw error;
        }
    };

    const handleSearch = async () => {
        if (!searchQuery.trim()) {
            setSearchResults([]);
            return;
        }

        if (searchAbortController.current) {
            searchAbortController.current.abort();
        }
        const abortController = new AbortController();
        searchAbortController.current = abortController;

        setSearchLoading(true);
        try {
            const results = await getTaurpc().kms_search_semantic(
                searchQuery,
                "text",
                kmsSearchDefaultLimit,
                searchMode
            );
            if (abortController.signal.aborted) return;
            setSearchResults(results);
        } catch (error) {
            if (abortController.signal.aborted) return;
            toast({
                title: "Search Failed",
                description: formatKmsError(error),
                variant: "destructive",
            });
        } finally {
            if (!abortController.signal.aborted) {
                setSearchLoading(false);
            }
        }
    };

    const cancelSearch = () => {
        if (searchAbortController.current) {
            searchAbortController.current.abort();
            searchAbortController.current = null;
        }
        setSearchLoading(false);
    };

    const handleRenameFolder = async (oldPath: string) => {
        const currentName = oldPath.split(/[/\\]/).pop() || "";
        const newName = window.prompt("Rename notebook/folder:", currentName);
        if (!newName || newName === currentName) return;

        if (!window.confirm(`Are you sure you want to rename '${currentName}' to '${newName}'? This will update all notes within this folder.`)) {
            return;
        }

        try {
            await getTaurpc().kms_rename_folder(oldPath, newName);
            toast({ title: "Folder Renamed", description: `Renamed to ${newName}` });
            refreshStructure();
            refreshNotes();
        } catch (err) {
            toast({ title: "Rename Failed", description: formatKmsError(err), variant: "destructive" });
        }
    };

    const handleDeleteFolder = async (path: string) => {
        const name = path.split(/[/\\]/).pop() || "folder";
        if (!window.confirm(`CRITICAL: Are you sure you want to delete '${name}' and ALL its contents? This cannot be undone.`)) {
            return;
        }

        try {
            await getTaurpc().kms_delete_folder(path);
            toast({ title: "Folder Deleted", description: name });
            refreshStructure();
            refreshNotes();
            if (activeNote && activeNote.path.startsWith(path)) {
                setActiveNote(null);
            }
        } catch (err) {
            toast({ title: "Delete Failed", description: formatKmsError(err), variant: "destructive" });
        }
    };

    const handleMoveItem = async (path: string, newParentPath: string) => {
        const itemName = path.split(/[/\\]/).pop() || "item";
        const folderName = newParentPath.split(/[/\\]/).pop() || "root";

        if (!window.confirm(`Move '${itemName}' to '${folderName}'?`)) {
            return;
        }

        try {
            await getTaurpc().kms_move_item(path, newParentPath);
            toast({ title: "Item Moved", description: `Moved ${itemName} to ${folderName}` });
            refreshStructure();
            refreshNotes();
        } catch (err) {
            toast({ title: "Move Failed", description: formatKmsError(err), variant: "destructive" });
        }
    };

    const toggleExplorerBulkPath = useCallback((path: string) => {
        setExplorerBulkPaths((prev) => {
            const next = new Set(prev);
            if (next.has(path)) next.delete(path);
            else next.add(path);
            return next;
        });
    }, []);

    const handleExplorerBulkDelete = async () => {
        const paths = [...explorerBulkPaths];
        if (paths.length === 0) return;
        if (!window.confirm(`Delete ${paths.length} note(s)? This cannot be undone.`)) return;
        for (const p of paths) {
            await handleDeleteNote(p, { skipConfirm: true });
        }
        setExplorerBulkPaths(new Set());
        setExplorerBulkMode(false);
    };

    const handleExplorerBulkMove = async () => {
        const paths = [...explorerBulkPaths];
        if (paths.length === 0) return;
        const dest = window.prompt("Move selected notes into this folder (full path):");
        if (!dest?.trim()) return;
        const target = dest.trim();
        try {
            for (const p of paths) {
                await getTaurpc().kms_move_item(p, target);
            }
            toast({ title: "Bulk move complete", description: `${paths.length} item(s) moved.` });
            refreshStructure();
            refreshNotes();
            setExplorerBulkPaths(new Set());
            setExplorerBulkMode(false);
        } catch (err) {
            toast({ title: "Bulk move failed", description: formatKmsError(err), variant: "destructive" });
        }
    };

    const queueEditorMarkdownInsert = useCallback((text: string) => {
        editorInsertTokenRef.current += 1;
        setEditorMarkdownInsert({ id: editorInsertTokenRef.current, text });
    }, []);

    const handleNavigateToResult = async (result: any) => {
        if (!checkUnsavedSkillChanges()) return;
        if (result.entity_type === "note") {
            const note = notes.find(n => n.path === result.entity_id);
            if (note) handleSelectNote(note);
        } else if (result.entity_type === "snippet") {
            try {
                // Try from snippet field first (newly populated), then metadata
                const content = result.snippet || (() => {
                    const meta = JSON.parse(result.metadata || "{}");
                    return meta.content || result.metadata || "";
                })();

                const meta = JSON.parse(result.metadata || "{}");
                setViewFullContent(content);
                setViewFullEditMeta({
                    category: meta.category || "General",
                    snippetIdx: typeof meta.snippetIdx === "number" ? meta.snippetIdx : -1
                });
                setViewFullVisible(true);
            } catch (e) {
                // Fallback for older or malformed metadata
                const content = result.snippet || result.metadata || "";
                setViewFullContent(content);
                setViewFullEditMeta(null);
                setViewFullVisible(true);
            }
        } else if (result.entity_type === "clipboard") {
            try {
                setSearchLoading(true);
                const id = Number.parseInt(result.entity_id);
                const entry = await getTaurpc().get_clip_entry_by_id(id);

                if (result.modality === "image" && entry) {
                    setImageViewerCurrent(entry);
                    setImageViewerContext([entry]);
                    setImageViewerVisible(true);
                    return;
                }

                // Fallback to text modal
                const content = entry?.content || result.snippet || (() => {
                    const meta = JSON.parse(result.metadata || "{}");
                    return meta.content || result.metadata || "";
                })();

                const meta = JSON.parse(result.metadata || "{}");
                setViewFullContent(content);
                setViewFullClipboardMeta({
                    id: entry?.id || (typeof meta.id === "number" ? meta.id : Number.parseInt(result.entity_id)),
                    canPromote: entry ? (entry.entry_type !== "image" && entry.entry_type !== "extracted_text") : (meta.entry_type !== "image"),
                    trigger: (content || "").slice(0, 20).replace(/\s/g, "").trim() || "clip"
                });
                setViewFullEditMeta(null);
                setViewFullVisible(true);
            } catch (err) {
                console.error("Failed to handle clipboard navigation:", err);
                toast({ title: "Error", description: "Failed to open clipboard entry", variant: "destructive" });
            } finally {
                setSearchLoading(false);
            }
        }
    };

    const handleRepairDatabase = async () => {
        if (!window.confirm("This will surgically reset the AI search index. Your actual notes and snippets will NOT be deleted. Proceed?")) {
            return;
        }
        try {
            await getTaurpc().kms_repair_database();
            toast({
                title: "KMS Index Reset",
                description: "AI tables cleared. Please RESTART the app now to finish the repair.",
            });
        } catch (error) {
            toast({
                title: "Repair Failed",
                description: formatKmsError(error),
                variant: "destructive",
            });
        }
    };

    const handleSelectGraphNode = async (path: string) => {
        let note = notes.find((n) => n.path === path);
        if (!note) {
            try {
                const latest = await getTaurpc().kms_list_notes();
                setNotes(latest);
                note = latest.find((n) => n.path === path);
            } catch (error) {
                console.warn("Failed to refresh note list before graph navigation:", error);
            }
        }

        if (note) {
            await handleSelectNote(note);
        } else {
            const title = path.split(/[\\/]/).pop()?.replace(".md", "") || path;
            await handleSelectNote({ path, title, tags: [] } as unknown as KmsNoteDto);
        }
        setView("explorer");
    };

    useEffect(() => {
        applyThemeToDocument(currentTheme);
    }, [currentTheme]);

    if (initializing) {
        return (
            <div className="flex items-center justify-center h-screen bg-dc-bg text-dc-text">
                <div className="flex flex-col items-center gap-4">
                    <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-dc-accent" />
                    <p className="text-sm font-medium animate-pulse">Initializing Knowledge Suite...</p>
                </div>
            </div>
        );
    }

    return (
        <div className="flex h-screen bg-dc-bg text-dc-text font-sans overflow-hidden select-none" data-theme={currentTheme}>
            {/* Sidebar */}
            {!isZenMode && (
                <aside
                    className="border-r border-dc-border flex flex-col bg-dc-bg-secondary/30 backdrop-blur-md relative"
                    style={{ width: sidebarWidth }}
                >
                    <div className="p-4 border-b border-dc-border flex items-center gap-2">
                        <div className="p-1.5 bg-dc-accent rounded-lg text-white">
                            < Book size={18} />
                        </div>
                        <span className="font-semibold tracking-tight">DigiCore KMS</span>
                    </div>
                    <div className="flex items-center gap-4 px-4 py-2">
                        <div className="flex items-center gap-1.5 px-3 py-1 bg-dc-bg-secondary/50 rounded-full border border-dc-border">
                            <div className={`w-1.5 h-1.5 rounded-full ${syncStatus === "Idle" ? "bg-dc-green" : syncStatus.toLowerCase().includes("error") || syncStatus.toLowerCase().includes("failed") ? "bg-dc-red" : "bg-dc-amber animate-pulse"}`} />
                            <span className="text-[10px] text-dc-text-muted uppercase tracking-wider font-bold">
                                Sync: <span className={syncStatus !== "Idle" ? "text-dc-text" : ""}>{syncStatus}</span>
                            </span>
                        </div>
                    </div>
                    {reindexProgress && (
                        <div className="px-4 pb-2">
                            <KmsReindexProgressBadge
                                variant="panel"
                                providerId={reindexProgress.providerId}
                                providerIndex={reindexProgress.providerIndex}
                                providerTotal={reindexProgress.providerTotal}
                                elapsedMs={reindexProgress.elapsedMs}
                                etaRemainingMs={reindexProgress.etaRemainingMs}
                                indexedTotalSoFar={reindexProgress.indexedTotalSoFar}
                            />
                            {(reindexProgress.phase !== "start" || reindexProgress.error) && (
                                <div className="mt-1 px-1 text-[10px] text-dc-text-muted">
                                    {reindexProgress.phase !== "start" && (
                                        <span>
                                            Provider indexed: {reindexProgress.providerIndexedCount.toLocaleString()}
                                            {reindexProgress.succeeded === false ? " (with errors)" : ""}
                                        </span>
                                    )}
                                    {reindexProgress.error && (
                                        <div className="text-dc-red mt-1 truncate" title={reindexProgress.error}>
                                            {reindexProgress.error}
                                        </div>
                                    )}
                                </div>
                            )}
                        </div>
                    )}

                    <div className="flex-1 overflow-y-auto p-4 space-y-6">
                        <div>
                            <div className="text-[10px] font-bold text-dc-text-muted uppercase tracking-wider mb-2 px-2">Navigation</div>
                            <nav className="space-y-1">
                                <Button
                                    variant={view === "explorer" ? "secondary" : "ghost"}
                                    size="sm"
                                    className={`w-full justify-start gap-2 h-9 px-2 ${view === "explorer" ? "bg-dc-bg-hover text-dc-accent font-medium" : "text-dc-text-muted hover:bg-dc-bg-hover"}`}
                                    onClick={() => {
                                        if (!checkUnsavedSkillChanges()) return;
                                        setView("explorer");
                                        setIsSkillEditorOpen(false);
                                        setActiveNote(null);
                                    }}
                                >
                                    <FolderOpen size={16} className={view === "explorer" ? "text-dc-accent" : "text-dc-text-muted"} />
                                    <span className="text-sm">Explorer</span>
                                </Button>
                                <Button
                                    variant={view === "search" ? "secondary" : "ghost"}
                                    size="sm"
                                    className={`w-full justify-start gap-2 h-9 px-2 ${view === "search" ? "bg-dc-bg-hover text-dc-accent font-medium" : "text-dc-text-muted hover:bg-dc-bg-hover"}`}
                                    onClick={() => {
                                        if (!checkUnsavedSkillChanges()) return;
                                        setView("search");
                                        setIsSkillEditorOpen(false);
                                    }}
                                >
                                    <Search size={16} className={view === "search" ? "text-dc-accent" : "text-dc-text-muted"} />
                                    <span className="text-sm">Semantic Search</span>
                                </Button>
                                <Button
                                    variant={view === "favorites" ? "secondary" : "ghost"}
                                    size="sm"
                                    className={`w-full justify-start gap-2 h-9 px-2 ${view === "favorites" ? "bg-dc-bg-hover text-dc-accent font-medium" : "text-dc-text-muted hover:bg-dc-bg-hover"}`}
                                    onClick={() => {
                                        if (!checkUnsavedSkillChanges()) return;
                                        setView("favorites");
                                        setIsSkillEditorOpen(false);
                                    }}
                                >
                                    <Star size={16} className={view === "favorites" ? "text-dc-accent" : "text-dc-text-muted"} />
                                    <span className="text-sm">Favorites</span>
                                </Button>
                                <Button
                                    variant={view === "recents" ? "secondary" : "ghost"}
                                    size="sm"
                                    className={`w-full justify-start gap-2 h-9 px-2 ${view === "recents" ? "bg-dc-bg-hover text-dc-accent font-medium" : "text-dc-text-muted hover:bg-dc-bg-hover"}`}
                                    onClick={() => {
                                        if (!checkUnsavedSkillChanges()) return;
                                        setView("recents");
                                        setIsSkillEditorOpen(false);
                                    }}
                                >
                                    <Clock size={16} className={view === "recents" ? "text-dc-accent" : "text-dc-text-muted"} />
                                    <span className="text-sm">Recents</span>
                                </Button>
                                <Button
                                    variant={view === "graph" ? "secondary" : "ghost"}
                                    size="sm"
                                    className={`w-full justify-start gap-2 h-9 px-2 ${view === "graph" ? "bg-dc-bg-hover text-dc-accent font-medium" : "text-dc-text-muted hover:bg-dc-bg-hover"}`}
                                    onClick={() => {
                                        if (!checkUnsavedSkillChanges()) return;
                                        setView("graph");
                                        setIsSkillEditorOpen(false);
                                        setActiveNote(null);
                                    }}
                                >
                                    <Network size={16} className={view === "graph" ? "text-dc-accent" : "text-dc-text-muted"} />
                                    <span className="text-sm">Knowledge Graph</span>
                                </Button>
                                <Button
                                    variant={view === "skills" ? "secondary" : "ghost"}
                                    size="sm"
                                    className={`w-full justify-start gap-2 h-9 px-2 ${view === "skills" ? "bg-dc-bg-hover text-dc-accent font-medium" : "text-dc-text-muted hover:bg-dc-bg-hover"}`}
                                    onClick={() => {
                                        if (!checkUnsavedSkillChanges()) return;
                                        setView("skills");
                                        setIsSkillEditorOpen(false);
                                        setActiveNote(null);
                                    }}
                                >
                                    <Cpu size={16} className={view === "skills" ? "text-dc-accent" : "text-dc-text-muted"} />
                                    <span className="text-sm">Skill Hub</span>
                                </Button>
                                <Button
                                    variant={view === "logs" ? "secondary" : "ghost"}
                                    size="sm"
                                    className={`w-full justify-start gap-2 h-9 px-2 ${view === "logs" ? "bg-dc-bg-hover text-dc-accent font-medium" : "text-dc-text-muted hover:bg-dc-bg-hover"}`}
                                    onClick={() => {
                                        if (!checkUnsavedSkillChanges()) return;
                                        setView("logs");
                                        setIsSkillEditorOpen(false);
                                    }}
                                >
                                    <Activity size={16} className={view === "logs" ? "text-dc-accent" : "text-dc-text-muted"} />
                                    <span className="text-sm">Operational Logs</span>
                                </Button>
                            </nav>
                        </div>

                        <div className="flex-1 overflow-y-auto pt-0">
                            {view === "explorer" ? (
                                <div className="p-4 space-y-4">
                                    <div>
                                        <div className="text-[10px] font-bold text-dc-text-muted uppercase tracking-wider mb-2 px-2 flex justify-between items-center">
                                            Vault Explorer
                                            <div className="flex items-center gap-2">
                                                <div title="New Notebook/Folder">
                                                    <FolderOpen
                                                        size={14}
                                                        className="cursor-pointer hover:text-dc-accent transition-colors"
                                                        onClick={() => handleCreateFolder()}
                                                    />
                                                </div>
                                                <div title="New Note">
                                                    <Plus
                                                        size={14}
                                                        className="cursor-pointer hover:text-dc-accent transition-colors"
                                                        onClick={() => handleCreateNote()}
                                                    />
                                                </div>
                                                <div title="Force Reindex Vault">
                                                    <RefreshCw
                                                        size={14}
                                                        className={`cursor-pointer transition-colors ${syncStatus !== "Idle" ? "text-dc-amber animate-spin" : "hover:text-dc-accent"}`}
                                                        onClick={async () => {
                                                            try {
                                                                await getTaurpc().kms_reindex_all();
                                                                refreshNotes();
                                                                refreshStructure();
                                                                toast({ title: "Reindex Triggered", description: "Indexing vault contents." });
                                                            } catch (err) {
                                                                toast({ title: "Reindex Failed", description: formatKmsError(err), variant: "destructive" });
                                                            }
                                                        }}
                                                    />
                                                </div>
                                                <div title="Template gallery">
                                                    <LayoutTemplate
                                                        size={14}
                                                        className="cursor-pointer hover:text-dc-accent transition-colors"
                                                        onClick={() => setTemplateGalleryOpen(true)}
                                                    />
                                                </div>
                                            </div>
                                        </div>
                                        <div className="flex flex-wrap items-center gap-2 px-1 mb-2">
                                            <Button
                                                type="button"
                                                variant={explorerBulkMode ? "secondary" : "ghost"}
                                                size="sm"
                                                className="h-7 text-[10px]"
                                                onClick={() => {
                                                    setExplorerBulkMode((v) => !v);
                                                    setExplorerBulkPaths(new Set());
                                                }}
                                            >
                                                Bulk select
                                            </Button>
                                            {explorerBulkMode ? (
                                                <>
                                                    <Button
                                                        type="button"
                                                        variant="secondary"
                                                        size="sm"
                                                        className="h-7 text-[10px]"
                                                        disabled={explorerBulkPaths.size === 0}
                                                        onClick={() => void handleExplorerBulkMove()}
                                                    >
                                                        Move ({explorerBulkPaths.size})
                                                    </Button>
                                                    <Button
                                                        type="button"
                                                        variant="destructive"
                                                        size="sm"
                                                        className="h-7 text-[10px]"
                                                        disabled={explorerBulkPaths.size === 0}
                                                        onClick={() => void handleExplorerBulkDelete()}
                                                    >
                                                        Delete ({explorerBulkPaths.size})
                                                    </Button>
                                                </>
                                            ) : null}
                                        </div>
                                        <div className="relative group px-1 mb-2 space-y-2">
                                            <div className="relative">
                                                <Filter size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-dc-text-muted group-focus-within:text-dc-accent transition-colors pointer-events-none" />
                                                <input
                                                    placeholder="Filter tree by name or path..."
                                                    className="w-full bg-dc-bg-secondary text-dc-text border border-dc-border rounded-lg py-1.5 pl-9 pr-3 text-xs focus:outline-none focus:border-dc-accent/50 focus:bg-dc-bg-hover/50 transition-all placeholder:text-dc-text-muted/50"
                                                    value={explorerTreeFilter}
                                                    onChange={(e) => setExplorerTreeFilter(e.target.value)}
                                                />
                                            </div>
                                            <input
                                                placeholder="Filter by tag (comma or space)..."
                                                title="Uses indexed YAML tags from frontmatter. Any token matches any tag."
                                                className="w-full bg-dc-bg-secondary text-dc-text border border-dc-border rounded-lg py-1.5 px-3 text-xs focus:outline-none focus:border-dc-accent/50 focus:bg-dc-bg-hover/50 transition-all placeholder:text-dc-text-muted/50"
                                                value={explorerTagFilter}
                                                onChange={(e) => setExplorerTagFilter(e.target.value)}
                                            />
                                        </div>
                                        <div className="flex-1 overflow-y-auto min-h-0">
                                            <FileExplorer
                                                structure={vaultStructure}
                                                notes={notes}
                                                activeNote={activeNote}
                                                onSelectNote={handleSelectNote}
                                                onOpenNoteAsReference={handleOpenNoteAsReference}
                                                onCreateNote={handleCreateNote}
                                                onCreateFolder={handleCreateFolder}
                                                onRenameNote={async (oldPath: string, newName: string) => {
                                                    if (window.confirm(`Rename note to '${newName}'?`)) {
                                                        await handleRenameNote(newName, oldPath);
                                                    }
                                                }}
                                                onDeleteNote={async (path: string) => {
                                                    if (window.confirm("Are you sure you want to delete this note?")) {
                                                        await handleDeleteNote(path);
                                                    }
                                                }}
                                                onRenameFolder={handleRenameFolder}
                                                onDeleteFolder={handleDeleteFolder}
                                                onMoveItem={handleMoveItem}
                                                onSetNoteFavorite={applyNoteFavoriteAtPath}
                                                filterQuery={explorerTreeFilter}
                                                tagFilter={explorerTagFilter}
                                                bulkSelectMode={explorerBulkMode}
                                                bulkSelectedPaths={explorerBulkPaths}
                                                onToggleBulkPath={toggleExplorerBulkPath}
                                            />
                                        </div>
                                    </div>
                                </div>
                            ) : view === "search" ? (
                                <div className="p-4 space-y-4">
                                    <div className="text-[10px] font-bold text-dc-text-muted uppercase tracking-wider mb-2 px-2">Knowledge Search</div>
                                    <div className="relative group px-1">
                                        <Search size={14} className="absolute left-4 top-1/2 -translate-y-1/2 text-dc-text-muted group-focus-within:text-dc-accent transition-colors" />
                                        <input
                                            autoFocus
                                            placeholder="Recall anything..."
                                            className="w-full bg-dc-bg-secondary text-dc-text border border-dc-border rounded-xl py-2 pl-10 pr-4 text-xs focus:outline-none focus:border-dc-accent/50 focus:bg-dc-bg-hover/50 transition-all font-medium placeholder:text-dc-text-muted/50"
                                            value={searchQuery}
                                            onChange={(e) => setSearchQuery(e.target.value)}
                                            onKeyDown={(e) => e.key === "Enter" && handleSearch()}
                                        />
                                        <div className="absolute right-3 top-1/2 -translate-y-1/2 flex gap-1">
                                            {searchQuery && (
                                                <div className="text-[8px] bg-dc-bg font-mono border border-dc-border rounded px-1 text-dc-text-muted opacity-50 shadow-sm">ENTER</div>
                                            )}
                                        </div>
                                    </div>

                                    {/* Search Mode Selector */}
                                    <div className="flex bg-dc-bg-secondary/50 rounded-lg p-0.5 border border-dc-border mx-1">
                                        {(["Hybrid", "Semantic", "Keyword"] as const).map((mode) => (
                                            <button
                                                key={mode}
                                                onClick={() => setSearchMode(mode)}
                                                className={`flex-1 py-1 text-[10px] font-bold uppercase tracking-tight rounded-md transition-all ${searchMode === mode
                                                    ? "bg-dc-accent text-white shadow-sm"
                                                    : "text-dc-text-muted hover:text-dc-text hover:bg-dc-bg-hover"
                                                    }`}
                                            >
                                                {mode}
                                            </button>
                                        ))}
                                    </div>

                                    <details className="mx-1 mt-2 border border-dc-border rounded-lg bg-dc-bg-secondary/30 group">
                                        <summary className="px-3 py-2 text-[10px] font-bold text-dc-text-muted cursor-pointer select-none uppercase tracking-wider list-none flex items-center justify-between">
                                            <span>Scope and filters</span>
                                            <span className="text-[9px] opacity-60 normal-case font-normal">Client-side</span>
                                        </summary>
                                        <div className="px-3 pb-3 pt-1 space-y-3 border-t border-dc-border/40">
                                            <div>
                                                <label className="text-[9px] text-dc-text-muted uppercase font-bold block mb-1">Path contains</label>
                                                <input
                                                    placeholder="e.g. notes/project"
                                                    className="w-full bg-dc-bg-secondary text-dc-text border border-dc-border rounded-lg py-1.5 px-2 text-xs"
                                                    value={searchClientFilters.pathPrefix}
                                                    onChange={(e) =>
                                                        setSearchClientFilters((f) => ({ ...f, pathPrefix: e.target.value }))
                                                    }
                                                />
                                            </div>
                                            <div>
                                                <label className="text-[9px] text-dc-text-muted uppercase font-bold block mb-1">Note tags</label>
                                                <input
                                                    placeholder="e.g. project, review"
                                                    title="Indexed YAML tags. Any token matches any tag (substring)."
                                                    className="w-full bg-dc-bg-secondary text-dc-text border border-dc-border rounded-lg py-1.5 px-2 text-xs"
                                                    value={searchClientFilters.tagsFilter}
                                                    onChange={(e) =>
                                                        setSearchClientFilters((f) => ({ ...f, tagsFilter: e.target.value }))
                                                    }
                                                />
                                            </div>
                                            <div className="grid grid-cols-2 gap-2">
                                                <div>
                                                    <label className="text-[9px] text-dc-text-muted uppercase font-bold block mb-1">Modified from</label>
                                                    <input
                                                        type="date"
                                                        className="w-full bg-dc-bg-secondary text-dc-text border border-dc-border rounded-lg py-1.5 px-2 text-[10px]"
                                                        value={searchDateFromInput}
                                                        onChange={(e) => setSearchDateFromInput(e.target.value)}
                                                    />
                                                </div>
                                                <div>
                                                    <label className="text-[9px] text-dc-text-muted uppercase font-bold block mb-1">Modified to</label>
                                                    <input
                                                        type="date"
                                                        className="w-full bg-dc-bg-secondary text-dc-text border border-dc-border rounded-lg py-1.5 px-2 text-[10px]"
                                                        value={searchDateToInput}
                                                        onChange={(e) => setSearchDateToInput(e.target.value)}
                                                    />
                                                </div>
                                            </div>
                                            <p className="text-[9px] text-dc-text-muted leading-snug">
                                                Date range applies to indexed notes only (uses note last modified from the vault index).
                                            </p>
                                            <div>
                                                <label className="text-[9px] text-dc-text-muted uppercase font-bold block mb-1">Notes</label>
                                                <select
                                                    className="w-full bg-dc-bg-secondary text-dc-text border border-dc-border rounded-lg py-1.5 px-2 text-xs"
                                                    value={searchClientFilters.noteScope}
                                                    onChange={(e) =>
                                                        setSearchClientFilters((f) => ({
                                                            ...f,
                                                            noteScope: e.target.value as KmsSearchClientFilters["noteScope"],
                                                        }))
                                                    }
                                                >
                                                    <option value="all">All note files</option>
                                                    <option value="standard_only">Hide skill files</option>
                                                    <option value="skills_only">Skills only</option>
                                                </select>
                                            </div>
                                            <div className="flex flex-wrap gap-x-4 gap-y-2 text-[10px] text-dc-text">
                                                <label className="flex items-center gap-2 cursor-pointer">
                                                    <input
                                                        type="checkbox"
                                                        checked={searchClientFilters.includeNotes}
                                                        onChange={(e) =>
                                                            setSearchClientFilters((f) => ({
                                                                ...f,
                                                                includeNotes: e.target.checked,
                                                            }))
                                                        }
                                                    />
                                                    Notes
                                                </label>
                                                <label className="flex items-center gap-2 cursor-pointer">
                                                    <input
                                                        type="checkbox"
                                                        checked={searchClientFilters.includeSnippets}
                                                        onChange={(e) =>
                                                            setSearchClientFilters((f) => ({
                                                                ...f,
                                                                includeSnippets: e.target.checked,
                                                            }))
                                                        }
                                                    />
                                                    Snippets
                                                </label>
                                                <label className="flex items-center gap-2 cursor-pointer">
                                                    <input
                                                        type="checkbox"
                                                        checked={searchClientFilters.includeClipboard}
                                                        onChange={(e) =>
                                                            setSearchClientFilters((f) => ({
                                                                ...f,
                                                                includeClipboard: e.target.checked,
                                                            }))
                                                        }
                                                    />
                                                    Clipboard
                                                </label>
                                                <label className="flex items-center gap-2 cursor-pointer">
                                                    <input
                                                        type="checkbox"
                                                        checked={searchClientFilters.includeImages}
                                                        onChange={(e) =>
                                                            setSearchClientFilters((f) => ({
                                                                ...f,
                                                                includeImages: e.target.checked,
                                                            }))
                                                        }
                                                    />
                                                    Images
                                                </label>
                                            </div>
                                            <Button
                                                type="button"
                                                variant="ghost"
                                                size="sm"
                                                className="h-7 text-[10px] border border-dc-border"
                                                onClick={() => {
                                                    setSearchClientFilters(defaultKmsSearchClientFilters());
                                                    setSearchDateFromInput("");
                                                    setSearchDateToInput("");
                                                }}
                                            >
                                                Reset filters
                                            </Button>
                                        </div>
                                    </details>

                                    {!kmsSearchIncludeEmbeddingDiagnostics && (
                                        <p className="text-[9px] text-dc-text-muted px-2 leading-snug">
                                            Turn on &quot;Include embedding diagnostics in KMS search&quot; in Config to see query embed time and model id here.
                                        </p>
                                    )}

                                    <div className="space-y-1 pb-10">
                                        {searchLoading ? (
                                            <div className="flex flex-col items-center justify-center py-12 gap-3 opacity-50">
                                                <div className="animate-spin rounded-full h-5 w-5 border-b-2 border-dc-accent" />
                                                <span className="text-[10px] uppercase font-bold tracking-[0.2em] text-dc-accent">Thinking...</span>
                                                <Button variant="ghost" size="sm" onClick={cancelSearch} className="mt-2 text-xs border border-dc-border hover:bg-dc-bg-hover">
                                                    Cancel Search
                                                </Button>
                                            </div>
                                        ) : searchResults.length === 0 && searchQuery ? (
                                            <div className="text-center py-12 px-6">
                                                <div className="w-10 h-10 bg-dc-bg-hover rounded-full flex items-center justify-center mx-auto mb-3 opacity-50">
                                                    <Search size={16} />
                                                </div>
                                                <p className="text-[10px] text-dc-text-muted italic leading-relaxed">We couldn't find any direct or semantic matches for your query.</p>
                                            </div>
                                        ) : searchResults.length > 0 && filteredSearchResults.length === 0 ? (
                                            <div className="text-center py-12 px-6">
                                                <p className="text-[10px] text-dc-text-muted leading-relaxed">
                                                    {searchResults.length} hit(s) from search, but none match your filters. Adjust scope and filters or reset.
                                                </p>
                                            </div>
                                        ) : (
                                            <>
                                                {searchEmbeddingDiagBanner ? (
                                                    <div
                                                        className="mx-1 mb-2 px-2 py-1.5 rounded-md bg-dc-bg-secondary/50 border border-dc-border text-[9px] text-dc-text-muted font-mono break-all"
                                                        title={searchEmbeddingDiagBanner.modelId}
                                                    >
                                                        Query embedding: {searchEmbeddingDiagBanner.ms.toFixed(1)} ms |{" "}
                                                        {searchEmbeddingDiagBanner.modelId.length > 56
                                                            ? `${searchEmbeddingDiagBanner.modelId.slice(0, 54)}...`
                                                            : searchEmbeddingDiagBanner.modelId}
                                                    </div>
                                                ) : null}
                                                {filteredSearchResults.map((result, idx) => (
                                                <Button
                                                    key={`${result.entity_id}-${idx}`}
                                                    variant="ghost"
                                                    size="sm"
                                                    className="w-full justify-start gap-2 py-3 px-3 h-auto hover:bg-dc-bg-hover group border border-transparent hover:border-dc-accent/20 rounded-xl transition-all mb-1"
                                                    onClick={() => handleNavigateToResult(result)}
                                                >
                                                    <div className="flex flex-col items-start text-left w-full gap-1">
                                                        <div className="flex items-center justify-between w-full">
                                                            <div className="flex items-center gap-1.5 overflow-hidden flex-1 mr-2">
                                                                <span className="text-[9px] font-bold text-dc-accent uppercase tracking-tighter opacity-80 whitespace-nowrap">
                                                                    {result.entity_type === "clipboard" ? `CLIPBOARD (${result.modality})` : result.entity_type}
                                                                </span>
                                                                {result.modality === "image" && <div className="p-0.5 bg-dc-accent/10 rounded text-dc-accent"><Book size={10} strokeWidth={3} /></div>}
                                                                {result.modality === "text" && result.entity_type === "clipboard" && <div className="p-0.5 bg-dc-accent/5 rounded text-dc-accent/60"><FileText size={10} strokeWidth={3} /></div>}
                                                            </div>
                                                            <span className="text-[8px] opacity-40 font-mono italic shrink-0">{Math.round((1 - result.distance) * 100)}% Match</span>
                                                        </div>

                                                        <span className="text-sm font-medium truncate group-hover:text-dc-accent transition-colors">
                                                            {result.entity_type === "note"
                                                                ? result.entity_id.split(/[\\/]/).pop()?.replace(".md", "")
                                                                : result.entity_type === "snippet"
                                                                    ? `Snippet: ${result.entity_id}`
                                                                    : (() => {
                                                                        if (result.entity_type === "clipboard") {
                                                                            try {
                                                                                const meta = JSON.parse(result.metadata || "{}");
                                                                                return meta.process_name || `Clipboard ${result.entity_id}`;
                                                                            } catch {
                                                                                return `Clipboard ${result.entity_id}`;
                                                                            }
                                                                        }
                                                                        return result.entity_id;
                                                                    })()
                                                            }
                                                        </span>

                                                        {(result.snippet || result.metadata) && (
                                                            <span className="text-[10px] text-dc-text-muted mt-1 leading-normal line-clamp-3 opacity-70 group-hover:opacity-100 transition-opacity">
                                                                {result.snippet || (() => {
                                                                    if (result.entity_type === "snippet") {
                                                                        try {
                                                                            const meta = JSON.parse(result.metadata || "{}");
                                                                            return meta.content || result.metadata;
                                                                        } catch {
                                                                            return result.metadata;
                                                                        }
                                                                    }
                                                                    if (result.entity_type === "clipboard" && result.metadata) {
                                                                        try {
                                                                            const meta = JSON.parse(result.metadata);
                                                                            return (meta.content || "").substring(0, 150);
                                                                        } catch {
                                                                            return result.metadata.substring(0, 150);
                                                                        }
                                                                    }
                                                                    return result.metadata;
                                                                })()}
                                                            </span>
                                                        )}
                                                    </div>
                                                </Button>
                                                ))}
                                            </>
                                        )}
                                    </div>
                                </div>
                            ) : view === "skills" ? (
                                null

                            ) : view === "favorites" ? (
                                <div className="p-4 space-y-2 flex-1 flex flex-col min-h-0">
                                    <div className="text-[10px] font-bold text-dc-text-muted uppercase tracking-wider mb-1 px-2 shrink-0">
                                        Favorites
                                    </div>
                                    <p className="text-[9px] text-dc-text-muted px-2 leading-snug">
                                        Drag the grip to reorder. Order is saved in this browser only.
                                    </p>
                                    <div className="flex-1 overflow-y-auto space-y-1 min-h-0">
                                        {favoriteNotes.length === 0 ? (
                                            <div className="text-center py-12 px-4 text-[10px] text-dc-text-muted leading-relaxed">
                                                No starred notes yet. Open a note and use the star in the editor toolbar.
                                            </div>
                                        ) : (
                                            favoriteNotes.map((note, favIndex) => (
                                                <div
                                                    key={note.path}
                                                    className="flex items-stretch gap-0.5 rounded-lg border border-transparent hover:border-dc-accent/15"
                                                    onDragOver={(e) => {
                                                        e.preventDefault();
                                                        e.dataTransfer.dropEffect = "move";
                                                    }}
                                                    onDrop={(e) => {
                                                        e.preventDefault();
                                                        const from = Number.parseInt(
                                                            e.dataTransfer.getData("application/x-kms-fav-idx"),
                                                            10
                                                        );
                                                        if (Number.isNaN(from)) return;
                                                        handleFavoriteReorder(from, favIndex);
                                                    }}
                                                >
                                                    <button
                                                        type="button"
                                                        title="Drag to reorder favorites"
                                                        draggable
                                                        onDragStart={(e) => {
                                                            e.dataTransfer.setData(
                                                                "application/x-kms-fav-idx",
                                                                String(favIndex)
                                                            );
                                                            e.dataTransfer.effectAllowed = "move";
                                                        }}
                                                        className="shrink-0 px-1 flex items-center text-dc-text-muted hover:text-dc-accent cursor-grab active:cursor-grabbing rounded-l-lg hover:bg-dc-bg-hover/80"
                                                    >
                                                        <GripVertical size={14} />
                                                    </button>
                                                    <Button
                                                        variant="ghost"
                                                        size="sm"
                                                        className={`flex-1 justify-start gap-2 py-2 px-2 h-auto hover:bg-dc-bg-hover rounded-r-lg rounded-l-none ${activeNote?.path === note.path ? "bg-dc-bg-hover border-dc-accent/30" : ""}`}
                                                        onClick={() => handleSelectNote(note)}
                                                    >
                                                        <Star size={14} className="shrink-0 text-dc-accent fill-dc-accent" />
                                                        <div className="flex flex-col items-start text-left min-w-0">
                                                            <span className="text-sm font-medium text-dc-text truncate w-full">
                                                                {note.title || note.path.split(/[\\/]/).pop()}
                                                            </span>
                                                            {note.folder_path ? (
                                                                <span className="text-[9px] text-dc-text-muted truncate w-full font-mono opacity-70">
                                                                    {note.folder_path}
                                                                </span>
                                                            ) : null}
                                                        </div>
                                                    </Button>
                                                </div>
                                            ))
                                        )}
                                    </div>
                                </div>
                            ) : view === "recents" ? (
                                <div className="p-4 space-y-2 flex-1 flex flex-col min-h-0">
                                    <div className="text-[10px] font-bold text-dc-text-muted uppercase tracking-wider mb-1 px-2 shrink-0">
                                        Recent notes
                                    </div>
                                    <p className="text-[9px] text-dc-text-muted px-2 leading-snug">
                                        Last {KMS_RECENT_NOTES_MAX} opened notes on this device.
                                    </p>
                                    <div className="flex-1 overflow-y-auto space-y-1 min-h-0">
                                        {recentNotes.length === 0 ? (
                                            <div className="text-center py-12 px-4 text-[10px] text-dc-text-muted leading-relaxed">
                                                Open a note from Explorer or Search to build your recent list.
                                            </div>
                                        ) : (
                                            recentNotes.map((note) => (
                                                <Button
                                                    key={note.path}
                                                    variant="ghost"
                                                    size="sm"
                                                    className={`w-full justify-start gap-2 py-2 px-3 h-auto hover:bg-dc-bg-hover rounded-lg border border-transparent hover:border-dc-accent/20 ${activeNote?.path === note.path ? "bg-dc-bg-hover border-dc-accent/30" : ""}`}
                                                    onClick={() => handleSelectNote(note)}
                                                >
                                                    <Clock size={14} className="shrink-0 text-dc-text-muted" />
                                                    <div className="flex flex-col items-start text-left min-w-0">
                                                        <span className="text-sm font-medium text-dc-text truncate w-full">
                                                            {note.title || note.path.split(/[\\/]/).pop()}
                                                        </span>
                                                        {note.folder_path ? (
                                                            <span className="text-[9px] text-dc-text-muted truncate w-full font-mono opacity-70">
                                                                {note.folder_path}
                                                            </span>
                                                        ) : null}
                                                    </div>
                                                </Button>
                                            ))
                                        )}
                                    </div>
                                </div>
                            ) : view === "logs" ? (
                                <KmsLogViewer />
                            ) : null}
                        </div>
                    </div >

                    <div className="p-4 border-t border-dc-border bg-dc-bg-secondary/50 space-y-2">
                        <div className="flex flex-col gap-1">
                            <Button
                                variant="ghost"
                                size="sm"
                                className="w-full justify-start gap-2 h-9 px-2 text-dc-text-muted hover:text-dc-text"
                                onClick={() => setIsSettingsOpen(true)}
                            >
                                <Settings size={16} className="text-dc-text-muted" />
                                <span className="text-sm">Vault Settings</span>
                            </Button>
                            <Button
                                variant="ghost"
                                size="sm"
                                className="w-full justify-start gap-2 h-7 px-2 text-[10px] text-dc-accent hover:text-dc-accent hover:bg-dc-accent/10"
                                onClick={handleRepairDatabase}
                            >
                                <AlertCircle size={12} />
                                <span>Repair KMS Index</span>
                            </Button>
                        </div>
                        <div className="px-2 pb-1 flex items-center justify-between text-[10px] text-dc-text-muted opacity-50">
                            <span className="truncate flex-1" title={vaultPath || ""}>{vaultPath}</span>
                            {syncStatus !== "Idle" && (
                                <div className="flex items-center gap-1.5 text-dc-accent animate-pulse">
                                    <RefreshCw size={10} className="animate-spin" />
                                    <span>{syncStatus}</span>
                                </div>
                            )}
                        </div>
                    </div>

                    {/* Resize Handle */}
                    <div
                        className={`absolute top-0 right-0 w-1 h-full cursor-col-resize transition-colors z-50 ${isResizing ? "bg-dc-accent" : "hover:bg-dc-accent/30"}`}
                        onMouseDown={(e) => {
                            e.preventDefault();
                            setIsResizing(true);
                        }}
                    />
                </aside >
            )}

            {/* Main Content Area */}
            < main className="flex-1 flex flex-col bg-dc-bg relative" >

                {/* Persistent Skills View */}
                <div
                    className="absolute inset-x-0 bottom-0 top-0 transition-opacity duration-300"
                    style={{
                        opacity: view === "skills" ? 1 : 0,
                        pointerEvents: view === "skills" ? "auto" : "none",
                        zIndex: view === "skills" ? 10 : 0
                    }}
                >
                    <SkillHub
                        refreshKey={skillRefreshKey}
                        onSelectSkill={(s) => { setActiveSkill(s); setIsSkillEditorOpen(true); }}
                        onCreateNew={() => { setActiveSkill(null); setIsSkillEditorOpen(true); }}
                    />
                </div>

                {/* Persistent Graph View */}
                <div
                    className="absolute inset-x-0 bottom-0 top-0 flex flex-col transition-opacity duration-300"
                    style={{
                        opacity: view === "graph" ? 1 : 0,
                        pointerEvents: view === "graph" ? "auto" : "none",
                        zIndex: view === "graph" ? 10 : 0
                    }}
                >
                    <Suspense fallback={<div className="flex-1 flex items-center justify-center bg-dc-bg"><div className="animate-spin rounded-full h-8 w-8 border-b-2 border-dc-accent" /></div>}>
                        <div className="absolute top-6 right-20 z-10 flex items-center gap-2">
                            {reindexProgress && (
                                <KmsReindexProgressBadge
                                    variant="toolbar"
                                    providerId={reindexProgress.providerId}
                                    providerIndex={reindexProgress.providerIndex}
                                    providerTotal={reindexProgress.providerTotal}
                                    elapsedMs={reindexProgress.elapsedMs}
                                    etaRemainingMs={reindexProgress.etaRemainingMs}
                                    indexedTotalSoFar={reindexProgress.indexedTotalSoFar}
                                />
                            )}
                            <div className="flex bg-dc-bg-secondary/40 backdrop-blur-md rounded-xl p-0.5 border border-dc-border">
                                <button
                                    onClick={() => setGraphMode("2d")}
                                    className={`px-3 py-1.5 text-[10px] font-bold uppercase tracking-wider rounded-lg transition-all ${graphMode === "2d" ? "bg-dc-accent text-white shadow-lg" : "text-dc-text-muted hover:text-dc-text"}`}
                                >
                                    2D
                                </button>
                                <button
                                    onClick={() => setGraphMode("3d")}
                                    className={`px-3 py-1.5 text-[10px] font-bold uppercase tracking-wider rounded-lg transition-all ${graphMode === "3d" ? "bg-dc-accent text-white shadow-lg" : "text-dc-text-muted hover:text-dc-text"}`}
                                >
                                    3D
                                </button>
                            </div>

                            <Button
                                variant="secondary"
                                size="sm"
                                onClick={() => setGraphResetKey(prev => prev + 1)}
                                className="h-[34px] rounded-xl bg-dc-bg-secondary/40 backdrop-blur-md border-dc-border text-dc-text-muted hover:text-dc-accent hover:border-dc-accent transition-all gap-2 px-3"
                                title="Reset Camera View"
                            >
                                <Maximize2 size={14} />
                                <span className="text-[10px] font-bold uppercase tracking-widest hidden sm:inline">Reset</span>
                            </Button>

                            <Button
                                variant="secondary"
                                size="sm"
                                onClick={() => window.dispatchEvent(new Event("kms-graph-toggle-tools-dock"))}
                                className="h-[34px] rounded-xl bg-dc-bg-secondary/40 backdrop-blur-md border-dc-border text-dc-text-muted hover:text-dc-accent hover:border-dc-accent transition-all gap-2 px-3"
                                title="Toggle graph tools dock (legend, search, shortest path). Same as the Tools tab on the left. Shortcut: Ctrl+Shift+G."
                            >
                                <PanelLeft size={14} />
                                <span className="text-[10px] font-bold uppercase tracking-widest hidden sm:inline">Tools</span>
                            </Button>
                        </div>

                        {graphMode === "2d" ? (
                            <KmsGraph
                                indexedNoteCount={indexedNoteCount}
                                indexedNotes={notes}
                                onSelectNote={handleSelectGraphNode}
                                activeNotePath={activeNote?.path}
                                isVisible={view === "graph"}
                                resetKey={graphResetKey}
                                graphNavigateRequest={graphNavigateRequest}
                            />
                        ) : (
                            <KmsGraph3D
                                indexedNoteCount={indexedNoteCount}
                                indexedNotes={notes}
                                onSelectNote={handleSelectGraphNode}
                                activeNotePath={activeNote?.path}
                                isVisible={view === "graph"}
                                resetKey={graphResetKey}
                                graphNavigateRequest={graphNavigateRequest}
                            />
                        )}
                    </Suspense>
                </div>

                {/* Explorer / Favorites / Recents / Editor View */}
                {(view === "explorer" || view === "favorites" || view === "recents" || view === "logs") && (
                    <div className="flex-1 h-full overflow-hidden flex flex-col">
                        {view === "logs" ? (
                            <KmsLogViewer />
                        ) : activeNote ? (
                            <div
                                className="flex-1 min-h-0 min-w-0 grid h-full overflow-hidden [grid-template-rows:minmax(0,1fr)]"
                                style={{
                                    gridTemplateColumns: isHistoryOpen
                                        ? `minmax(0, 1fr) ${historyPanelWidth}px`
                                        : `minmax(0, 1fr)`,
                                }}
                            >
                                <div className="min-h-0 min-w-0 h-full overflow-hidden flex flex-col">
                                    <KmsEditor
                                        path={activeNote.path}
                                        initialContent={activeContent}
                                        onSave={handleSaveNote}
                                        onDelete={handleDeleteNote}
                                        onRename={(newName) => handleRenameNote(newName).then(() => { })}
                                        onSelectNote={handleWikiNavigateFromEditor}
                                        onOpenReferenceWikiTarget={handleOpenReferenceFromWiki}
                                        referenceNote={
                                            referenceNote
                                                ? { path: referenceNote.path, title: referenceNote.title }
                                                : null
                                        }
                                        referenceContent={referenceContent}
                                        onClearReference={handleClearReferenceNote}
                                        onOpenSkillEditor={() => handleOpenSkillEditor(activeNote.path)}
                                        isFavorite={activeNote.is_favorite}
                                        onFavoriteChange={handleNoteFavoriteChange}
                                        isZenMode={isZenMode}
                                        onToggleZenMode={toggleZenMode}
                                        onToggleHistory={() => setIsHistoryOpen(!isHistoryOpen)}
                                        isHistoryOpen={isHistoryOpen}
                                        onToggleTheme={toggleTheme}
                                        currentTheme={currentTheme}
                                        vaultPath={vaultPath}
                                        workingCopyMarkdownRef={editorWorkingCopyMarkdownRef}
                                        onOpenGlobalGraph={openGlobalGraphForActiveNote}
                                    />
                                </div>
                                {isHistoryOpen && (
                                    <div className="flex h-full min-h-0 min-w-0 flex-row border-l border-dc-border bg-dc-bg-secondary/30 backdrop-blur-sm shadow-2xl z-40 overflow-hidden">
                                        <div
                                            role="separator"
                                            aria-orientation="vertical"
                                            aria-label="Resize version history panel"
                                            title="Drag left or right to resize version history"
                                            draggable={false}
                                            onDragStart={(ev) => ev.preventDefault()}
                                            onPointerDown={handleHistoryResizePointerDown}
                                            className={`relative shrink-0 w-4 flex items-center justify-center cursor-col-resize z-[60] touch-none select-none border-0 pointer-events-auto ${
                                                isResizingHistory ? "bg-dc-accent/25" : "hover:bg-dc-accent/15"
                                            }`}
                                        >
                                            <div
                                                className="absolute inset-y-2 left-1/2 w-1 -translate-x-1/2 rounded-full bg-dc-border border border-dc-border/80 shadow-sm pointer-events-none"
                                                aria-hidden
                                            />
                                            <GripVertical
                                                className="relative h-10 w-4 text-dc-text-muted opacity-70 pointer-events-none"
                                                strokeWidth={2}
                                                aria-hidden
                                            />
                                        </div>
                                        <div className="flex-1 min-w-0 min-h-0 flex flex-col overflow-hidden">
                                            <KmsHistoryBrowser
                                                vaultRelPath={kmsVaultRelativePath(vaultPath, activeNote.path)}
                                                absoluteNotePath={activeNote.path}
                                                panelWidthPx={historyPanelWidth}
                                                onPanelWidthPxChange={persistHistoryPanelWidth}
                                                getWorkingCopyMarkdown={() =>
                                                    editorWorkingCopyMarkdownRef.current?.() ?? ""
                                                }
                                                onRestore={() => {
                                                    void (async () => {
                                                        const content = await getTaurpc().kms_load_note(activeNote.path);
                                                        setActiveContent(content);
                                                        toast({
                                                            title: "Version Restored",
                                                            description: "The selected version has been restored and loaded.",
                                                        });
                                                    })();
                                                }}
                                            />
                                        </div>
                                    </div>
                                )}
                            </div>
                        ) : (
                            <div className="flex-1 flex flex-col items-center justify-center p-8">
                                <div className="max-w-md w-full text-center space-y-4">
                                    <div className="mx-auto w-16 h-16 bg-dc-accent/10 rounded-2xl flex items-center justify-center text-dc-accent mb-6">
                                        <Book size={32} />
                                    </div>
                                    <h2 className="text-2xl font-bold tracking-tight text-dc-text">Select a note to get started</h2>
                                    <p className="text-dc-text-muted text-sm leading-relaxed">
                                        {isZenMode ? (
                                            <>
                                                Zen mode is hiding the navigation sidebar (Explorer, Recents, Favorites, Knowledge Graph, Search). Use{" "}
                                                <span className="text-dc-text font-medium">Show sidebar</span> (bottom-left) or press{" "}
                                                <kbd className="px-1.5 py-0.5 rounded bg-dc-bg-secondary border border-dc-border text-[11px] font-mono">Alt+Z</kbd>{" "}
                                                to restore the full Knowledge Hub.
                                            </>
                                        ) : (
                                            <>
                                                Every note you create is a local Markdown file stored securely in your vault.
                                                Use the sidebar to explore your knowledge graph.
                                            </>
                                        )}
                                    </p>
                                    <div className="pt-6 flex justify-center flex-wrap gap-3">
                                        {isZenMode && (
                                            <Button
                                                size="sm"
                                                variant="secondary"
                                                className="gap-2 px-6 border border-dc-border"
                                                onClick={toggleZenMode}
                                                title="Exit Zen mode (Alt+Z)"
                                            >
                                                <PanelLeft size={16} />
                                                Show sidebar
                                            </Button>
                                        )}
                                        <Button
                                            size="sm"
                                            className="bg-dc-accent hover:bg-dc-accent/90 text-white gap-2 px-6"
                                            onClick={() => handleCreateNote()}
                                        >
                                            <Plus size={16} />
                                            Create New Note
                                        </Button>
                                    </div>
                                </div>
                            </div>
                        )}
                    </div>
                )}

                <AnimatePresence>
                    {isSkillEditorOpen && (
                        <SkillEditor
                            key={activeSkill ? `${activeSkill.metadata.name}-${skillRefreshKey}` : "new-skill"}
                            skill={activeSkill}
                            onClose={() => {
                                if (!checkUnsavedSkillChanges()) return;
                                setIsSkillEditorOpen(false);
                            }}
                            onDirtyChange={setIsSkillDirty}
                            onSaved={() => {
                                setIsSkillEditorOpen(false);
                                setIsSkillDirty(false);
                                setSkillRefreshKey(prev => prev + 1);
                            }}
                        />
                    )}
                </AnimatePresence>

                {/* Visual Accent */}
                {!activeNote && view !== "skills" && (
                    <>
                        <div className="absolute top-0 right-0 w-64 h-64 bg-dc-accent/5 blur-[120px] pointer-events-none rounded-full" />
                        <div className="absolute bottom-0 left-0 w-96 h-96 bg-dc-accent/5 blur-[160px] pointer-events-none rounded-full" />
                    </>
                )}

                {isZenMode && (
                    <div className="fixed bottom-5 left-5 z-[200] pointer-events-auto">
                        <Button
                            type="button"
                            variant="secondary"
                            size="sm"
                            className="shadow-lg border border-dc-border bg-dc-bg-secondary/95 backdrop-blur-md gap-2"
                            onClick={toggleZenMode}
                            title="Exit Zen mode (Alt+Z)"
                        >
                            <PanelLeft size={16} />
                            Show sidebar
                        </Button>
                    </div>
                )}
            </main>

            <Toaster />
            {vaultPath ? (
                <KmsTemplateGalleryModal
                    open={templateGalleryOpen}
                    onOpenChange={setTemplateGalleryOpen}
                    vaultPath={vaultPath}
                    onCreate={async (absolutePath, content) => {
                        await getTaurpc().kms_save_note(absolutePath, content);
                        await refreshStructure();
                        await refreshNotes();
                        const list = await getTaurpc().kms_list_notes();
                        const note = list.find((n) => n.path === absolutePath);
                        if (note) await handleSelectNote(note);
                        toast({
                            title: "Note created",
                            description: note?.title ?? absolutePath,
                        });
                    }}
                />
            ) : null}
            <KmsCommandPalette
                open={commandPaletteOpen}
                onOpenChange={setCommandPaletteOpen}
                notes={notes}
                onSelectNote={handleSelectNote}
                onJumpView={handleCommandPaletteJump}
            />
            <VaultSettingsModal
                isOpen={isSettingsOpen}
                onClose={() => setIsSettingsOpen(false)}
                currentPath={vaultPath}
                onPathUpdated={(newPath) => {
                    setVaultPath(newPath);
                    refreshNotes();
                }}
            />
            <ViewFull
                visible={viewFullVisible}
                content={viewFullContent}
                onClose={() => setViewFullVisible(false)}
                onEdit={(cat, idx) => {
                    getTaurpc().ghost_follower_request_edit(cat, idx as any);
                    setViewFullVisible(false);
                }}
                editMeta={viewFullEditMeta}
                onPromote={viewFullClipboardMeta ? async () => {
                    if (viewFullClipboardMeta.canPromote) {
                        const trigger = viewFullClipboardMeta.trigger || "clip";
                        await getTaurpc().ghost_follower_request_promote(viewFullContent, trigger);
                        setViewFullVisible(false);
                    }
                } : undefined}
                onCopy={async () => {
                    try {
                        await getTaurpc().copy_to_clipboard(viewFullContent);
                        toast({ title: "Snippet Copied", description: "Content copied to clipboard." });
                    } catch (err) {
                        toast({ title: "Copy Failed", description: String(err), variant: "destructive" });
                    }
                }}
                onDelete={viewFullEditMeta ? async () => {
                    if (window.confirm("Are you sure you want to delete this snippet?")) {
                        try {
                            await getTaurpc().delete_snippet(viewFullEditMeta.category, viewFullEditMeta.snippetIdx as any);
                            setViewFullVisible(false);
                            toast({ title: "Snippet Deleted", description: "The snippet has been removed." });
                        } catch (err) {
                            toast({ title: "Delete Failed", description: String(err), variant: "destructive" });
                        }
                    }
                } : viewFullClipboardMeta ? async () => {
                    if (window.confirm("Are you sure you want to delete this clipboard entry?")) {
                        try {
                            await getTaurpc().delete_clip_entry_by_id(viewFullClipboardMeta.id);
                            setViewFullVisible(false);
                            toast({ title: "Entry Deleted", description: "Clipboard entry removed." });
                        } catch (err) {
                            toast({ title: "Delete Failed", description: String(err), variant: "destructive" });
                        }
                    }
                } : undefined}
            />
            <Suspense fallback={null}>
                <ImageViewerModal
                    isOpen={imageViewerVisible}
                    onClose={() => setImageViewerVisible(false)}
                    currentImage={imageViewerCurrent}
                    allImages={imageViewerContext}
                    onNavigate={(img) => setImageViewerCurrent(img)}
                    onDeleteSuccess={() => {
                        // Update search results if needed
                    }}
                />
            </Suspense>
        </div >
    );
}
