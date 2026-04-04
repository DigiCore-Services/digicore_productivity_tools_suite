import React, {
    useCallback,
    useEffect,
    useLayoutEffect,
    useMemo,
    useRef,
    useState,
} from "react";
import * as d3 from "d3";
import { getTaurpc } from "../../lib/taurpc";
import {
    KmsGraphDto,
    KmsAiBeamDto,
    KmsGraphPathDto,
    KmsNoteGraphPreviewDto,
    KmsDiagnosticsDto,
    KmsNoteDto,
} from "../../bindings";
import { parseTagFilterTokens, tagsMatchFilterTokens } from "../../lib/kmsTagFilter";
import {
    readGraphSession,
    writeGraphSession,
    shouldUsePagedGraph,
    DEFAULT_PAGE_LIMIT,
    pageSizeSelectOptions,
    clampPageLimit,
    KMS_GRAPH_DEFAULT_AUTO_PAGING_ENABLED,
    KMS_GRAPH_DEFAULT_AUTO_PAGING_NOTE_THRESHOLD,
} from "../../lib/kmsGraphPaging";
import { kmsGraphLog } from "../../lib/kmsGraphLog";
import { normalizeKmsGraphWarnings } from "../../lib/kmsGraphWarnings";

const PATH_SORT_HELP =
    "Notes are ordered by vault-relative path (lexicographic). Pagination slices that list; it is not ranked by importance.";
import { Loader2, RefreshCw, Activity, Play, Route, ChevronLeft, ChevronRight, ClipboardList } from "lucide-react";
import { Button } from "../ui/button";
import { cn } from "../../lib/utils";
import { useToast } from "../ui/use-toast";
import {
    pathEdgeSetFromDto,
    pathNodeSetFromDto,
    undirectedEdgeKey,
} from "../../lib/kmsGraphHelpers";
import { formatIpcOrRaw } from "../../lib/ipcError";
import { copyKmsGraphDebugToClipboard } from "../../lib/kmsGraphDebugClipboard";
import {
    buildFolderColorMap,
    colorForFolderKey,
    folderLegendLabel,
    type GraphColorMode,
} from "../../lib/kmsGraphFolderPalette";
import {
    readGraphColorMode,
    writeGraphColorMode,
    readShowWikiEdges,
    writeShowWikiEdges,
    readShowAiBeamEdges,
    writeShowAiBeamEdges,
    readShowSemanticKnnEdges,
    writeShowSemanticKnnEdges,
    readLegendPanelTypes,
    writeLegendPanelTypes,
    readLegendPanelFolders,
    writeLegendPanelFolders,
    readLegendPanelEdgeToggles,
    writeLegendPanelEdgeToggles,
    readPulseEnabled,
    writePulseEnabled,
    readPulseTopPercent,
    writePulseTopPercent,
    readLegendFilterQuery,
    writeLegendFilterQuery,
    readHiddenFolderKeys,
    writeHiddenFolderKeys,
    readHiddenNodeTypes,
    writeHiddenNodeTypes,
    resetLegendVisibilityFilters,
    readGraphPanelsCollapsed,
    writeGraphPanelsCollapsed,
} from "../../lib/kmsGraphLegendPrefs";
import {
    graphLastModifiedRange,
    lastModifiedMs,
    recency01,
    shouldPulseRecent,
} from "../../lib/kmsGraphPulse";
import { kmsNodeIconPath } from "../../lib/kmsGraphNodeIcons";
import { nodeMatchesGraphFilter } from "../../lib/kmsGraphGraphFilter";
import {
    applyLegendVisibilityFilter,
    LEGEND_TYPE_ROWS,
} from "../../lib/kmsGraphLegendVisibility";
import {
    computeWeaklyConnectedComponents,
    componentIndexContaining,
} from "../../lib/kmsGraphIslands";
import { useIslandBridgeMergeToast } from "../../lib/kmsGraphIslandBridgeToast";
import {
    KmsGraphConstellationBackdrop,
    KMS_CONSTELLATION_ISLAND_COLORS,
} from "./KmsGraphConstellationBackdrop";
import { useKmsGraphVisualPrefs } from "../../lib/useKmsGraphVisualPrefs";
import {
    graphNodeCollisionRadius,
    graphNodeSizeWeight,
} from "../../lib/kmsGraphNodeLayoutMath";
import { runKmsGraphForceLayoutWorker } from "../../lib/runKmsGraphForceLayoutWorker";

type GraphPaging =
    | null
    | { kind: "full" }
    | { kind: "paged"; offset: number; limit: number };

interface KmsGraphProps {
    onSelectNote: (path: string) => void;
    activeNotePath?: string | null;
    isVisible?: boolean;
    resetKey?: number;
    /** Indexed notes in vault (for auto-paging threshold). */
    indexedNoteCount?: number;
    /** When set, enables optional tag filter in the legend (paths joined to `tags` from index). */
    indexedNotes?: KmsNoteDto[];
    graphNavigateRequest?: { token: number; path: string } | null;
}

interface GraphNode extends d3.SimulationNodeDatum {
    id: string; // path
    title: string;
    numLinks: number;
    /** Server PageRank-style score 0..1; blended with degree for radius. */
    linkCentrality: number;
    nodeType: string;
    folderPath: string;
    lastModified: string;
    /** 0..1 within graph last_modified range (newest = 1). */
    recency01: number;
    /** Visual pulse for recently edited (within top % of range). */
    pulseHighlight: boolean;
    clusterId?: number | null;
    /** Dimmed when another island is focused (non-isolate mode). */
    islandDimmed?: boolean;
}

interface GraphLink extends d3.SimulationLinkDatum<GraphNode> {
    source: string | GraphNode;
    target: string | GraphNode;
    isAiBeam?: boolean;
    /** True when DTO edge kind is semantic_knn (embedding kNN). */
    isSemanticKnn?: boolean;
    summary?: string;
}

export default function KmsGraph({
    onSelectNote,
    activeNotePath,
    isVisible,
    resetKey,
    indexedNoteCount = 0,
    indexedNotes,
    graphNavigateRequest = null,
}: KmsGraphProps) {
    const graphVisualPrefs = useKmsGraphVisualPrefs();
    const svgRef = useRef<SVGSVGElement>(null);
    const containerRef = useRef<HTMLDivElement>(null);
    const [data, setData] = useState<KmsGraphDto | null>(null);
    const [vaultDiag, setVaultDiag] = useState<KmsDiagnosticsDto | null>(null);
    const [paging, setPaging] = useState<GraphPaging>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [dimensions, setDimensions] = useState({ width: 0, height: 0 });
    const [workerLayoutBusy, setWorkerLayoutBusy] = useState(false);
    const [timeFilter, setTimeFilter] = useState(100); // 0-100%
    const [playingTimeline, setPlayingTimeline] = useState(false);
    const [pathFrom, setPathFrom] = useState("");
    const [pathTo, setPathTo] = useState("");
    const [pathResult, setPathResult] = useState<KmsGraphPathDto | null>(null);
    const [pathError, setPathError] = useState<string | null>(null);
    const [hoverPreview, setHoverPreview] = useState<
        (KmsNoteGraphPreviewDto & { x: number; y: number }) | null
    >(null);
    const previewTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    const zoomBehaviorRef = useRef<d3.ZoomBehavior<SVGSVGElement, unknown> | null>(null);
    const simulationNodesRef = useRef<GraphNode[]>([]);
    const islandFrameIdsRef = useRef<Set<string> | null>(null);
    const lastGraphNavTokenRef = useRef(0);
    const { toast } = useToast();

    const [colorMode, setColorMode] = useState<GraphColorMode>(() => readGraphColorMode());
    const [showWikiEdges, setShowWikiEdges] = useState(() => readShowWikiEdges());
    const [showSemanticKnnEdges, setShowSemanticKnnEdges] = useState(() =>
        readShowSemanticKnnEdges()
    );
    const [showAiBeams, setShowAiBeams] = useState(() => readShowAiBeamEdges());
    const [legendShowTypes, setLegendShowTypes] = useState(() => readLegendPanelTypes());
    const [legendShowFolders, setLegendShowFolders] = useState(() => readLegendPanelFolders());
    const [legendShowEdgeToggles, setLegendShowEdgeToggles] = useState(() =>
        readLegendPanelEdgeToggles()
    );
    const [pulseEnabled, setPulseEnabled] = useState(() => readPulseEnabled());
    const [pulseTopPercent, setPulseTopPercent] = useState(() => readPulseTopPercent());
    const [legendFilterText, setLegendFilterText] = useState(() => readLegendFilterQuery());
    const [legendTagFilterText, setLegendTagFilterText] = useState("");
    const [hiddenFolderKeys, setHiddenFolderKeys] = useState(() => readHiddenFolderKeys());
    const [hiddenNodeTypes, setHiddenNodeTypes] = useState(() => readHiddenNodeTypes());
    const [focusedIslandIdx, setFocusedIslandIdx] = useState<number | null>(null);
    const [islandIsolateHide, setIslandIsolateHide] = useState(false);
    const [graphPanelsCollapsed, setGraphPanelsCollapsed] = useState(() => readGraphPanelsCollapsed());

    const setColorModePersist = useCallback((m: GraphColorMode) => {
        setColorMode(m);
        writeGraphColorMode(m);
    }, []);

    const folderColorMap = useMemo(() => {
        if (!data) return new Map<string, string>();
        return buildFolderColorMap(data.nodes.map(n => n.folder_path ?? ""));
    }, [data]);

    const folderLegendRows = useMemo(() => {
        return Array.from(folderColorMap.entries())
            .sort((a, b) => a[0].localeCompare(b[0]))
            .map(([key, color]) => ({
                key,
                label: folderLegendLabel(key),
                color,
            }));
    }, [folderColorMap]);

    const handleResize = useCallback(() => {
        if (containerRef.current) {
            setDimensions({
                width: containerRef.current.clientWidth,
                height: containerRef.current.clientHeight
            });
        }
    }, []);

    useEffect(() => {
        handleResize(); // Initial size (often 0 while the graph container is not mounted yet)
        window.addEventListener("resize", handleResize);
        return () => window.removeEventListener("resize", handleResize);
    }, [handleResize]);

    useEffect(() => {
        if (isVisible) {
            handleResize();
        }
    }, [isVisible, handleResize]);

    // While loading we render a spinner without `containerRef`, so the first window resize pass
    // leaves dimensions at 0x0 forever unless we measure after the real graph shell mounts.
    useLayoutEffect(() => {
        if (!paging || loading || error) return;
        const el = containerRef.current;
        if (!el) return;
        const ro = new ResizeObserver(() => handleResize());
        ro.observe(el);
        handleResize();
        return () => ro.disconnect();
    }, [paging, loading, error, handleResize]);

    useEffect(() => {
        if (!isVisible) return;
        let cancelled = false;
        (async () => {
            try {
                const d = await getTaurpc().kms_get_diagnostics();
                if (!cancelled) setVaultDiag(d);
            } catch {
                /* ignore */
            }
        })();
        return () => {
            cancelled = true;
        };
    }, [isVisible]);

    useEffect(() => {
        if (!playingTimeline) return;
        let v = 0;
        const id = window.setInterval(() => {
            v += 2;
            setTimeFilter(Math.min(100, v));
            if (v >= 100) {
                setPlayingTimeline(false);
                window.clearInterval(id);
            }
        }, 90);
        return () => window.clearInterval(id);
    }, [playingTimeline]);

    const isPagedView = paging?.kind === "paged";

    const toggleGraphPanelsCollapsed = useCallback(() => {
        setGraphPanelsCollapsed(prev => {
            const next = !prev;
            writeGraphPanelsCollapsed(next);
            return next;
        });
    }, []);

    useEffect(() => {
        if (!isVisible) return;
        const onKey = (e: KeyboardEvent) => {
            if (e.ctrlKey && e.shiftKey && (e.key === "g" || e.key === "G")) {
                e.preventDefault();
                toggleGraphPanelsCollapsed();
            }
        };
        const onToggleDock = () => toggleGraphPanelsCollapsed();
        window.addEventListener("keydown", onKey);
        window.addEventListener("kms-graph-toggle-tools-dock", onToggleDock);
        return () => {
            window.removeEventListener("keydown", onKey);
            window.removeEventListener("kms-graph-toggle-tools-dock", onToggleDock);
        };
    }, [isVisible, toggleGraphPanelsCollapsed]);

    useEffect(() => {
        let cancelled = false;
        (async () => {
            try {
                const app = await getTaurpc().get_app_state();
                const sess = readGraphSession();
                const usePaged = shouldUsePagedGraph(
                    app.kms_graph_auto_paging_enabled ?? KMS_GRAPH_DEFAULT_AUTO_PAGING_ENABLED,
                    app.kms_graph_auto_paging_note_threshold ?? KMS_GRAPH_DEFAULT_AUTO_PAGING_NOTE_THRESHOLD,
                    indexedNoteCount,
                    sess.viewMode
                );
                if (cancelled) return;
                if (usePaged) {
                    setPaging({
                        kind: "paged",
                        offset: sess.offset,
                        limit: sess.limit || DEFAULT_PAGE_LIMIT,
                    });
                } else {
                    setPaging({ kind: "full" });
                }
            } catch {
                if (!cancelled) setPaging({ kind: "full" });
            }
        })();
        return () => {
            cancelled = true;
        };
    }, [indexedNoteCount]);

    const fetchData = useCallback(async () => {
        if (!paging) return;
        setLoading(true);
        try {
            let graphData: KmsGraphDto;
            if (paging.kind === "full") {
                graphData = await getTaurpc().kms_get_graph(0, 0, null, null);
            } else {
                graphData = await getTaurpc().kms_get_graph(paging.offset, paging.limit, null, null);
            }
            setData(graphData);
            setError(null);
        } catch (err) {
            kmsGraphLog.error("Failed to fetch graph data:", err);
            setError(formatIpcOrRaw(err));
        } finally {
            setLoading(false);
        }
    }, [paging]);

    const copyGraphDebugInfo = useCallback(() => {
        void copyKmsGraphDebugToClipboard(
            {
                graphView: "2d",
                data,
                error,
                paging,
                indexedNoteCount,
                vaultDiag,
                pathFrom,
                pathTo,
                pathResult,
                pathError,
                hoverPreviewPath: hoverPreview?.path ?? null,
            },
            { toast }
        );
    }, [
        data,
        error,
        paging,
        indexedNoteCount,
        vaultDiag,
        pathFrom,
        pathTo,
        pathResult,
        pathError,
        hoverPreview?.path,
        toast,
    ]);

    useEffect(() => {
        if (!paging) return;
        fetchData();
    }, [paging, fetchData]);

    useEffect(() => {
        const onWikiPrReady = () => {
            void fetchData();
        };
        window.addEventListener("kms-wiki-pagerank-ready", onWikiPrReady);
        return () => window.removeEventListener("kms-wiki-pagerank-ready", onWikiPrReady);
    }, [fetchData]);

    useEffect(() => {
        const svg = svgRef.current;
        if (!svg || !data || isPagedView) return;
        const onMove = (ev: MouseEvent) => {
            const g = (ev.target as Element | null)?.closest?.("g.node");
            if (!g) {
                setHoverPreview(null);
                return;
            }
            const p = g.getAttribute("data-path");
            if (!p) return;
            setHoverPreview(prev => {
                if (prev && prev.path === p) {
                    return { ...prev, x: ev.clientX, y: ev.clientY };
                }
                return {
                    path: p,
                    title: "",
                    excerpt: "",
                    last_modified: null,
                    x: ev.clientX,
                    y: ev.clientY,
                };
            });
            if (previewTimerRef.current) clearTimeout(previewTimerRef.current);
            previewTimerRef.current = setTimeout(async () => {
                try {
                    const prev = await getTaurpc().kms_get_note_graph_preview(p, 320);
                    setHoverPreview(h =>
                        h && h.path === p ? { ...prev, x: h.x, y: h.y } : null
                    );
                } catch {
                    setHoverPreview(null);
                }
            }, 240);
        };
        const onLeave = () => {
            if (previewTimerRef.current) clearTimeout(previewTimerRef.current);
            setHoverPreview(null);
        };
        svg.addEventListener("mousemove", onMove);
        svg.addEventListener("mouseleave", onLeave);
        return () => {
            if (previewTimerRef.current) clearTimeout(previewTimerRef.current);
            svg.removeEventListener("mousemove", onMove);
            svg.removeEventListener("mouseleave", onLeave);
        };
    }, [data, isPagedView]);

    const timeRange = useMemo(() => {
        if (!data) return { min: 0, max: Date.now() };
        const timestamps = data.nodes
            .filter(n => n.last_modified)
            .map(n => new Date(n.last_modified).getTime())
            .filter(t => !isNaN(t));
        if (timestamps.length === 0) return { min: 0, max: Date.now() };
        return { min: Math.min(...timestamps), max: Math.max(...timestamps) };
    }, [data]);

    const filteredNodes = useMemo(() => {
        if (!data) return [];
        if (timeFilter === 100) return data.nodes;

        const threshold = timeRange.min + (timeRange.max - timeRange.min) * (timeFilter / 100);
        return data.nodes.filter(n => {
            const t = new Date(n.last_modified).getTime();
            return isNaN(t) || t <= threshold;
        });
    }, [data, timeFilter, timeRange]);

    const filteredLinks = useMemo(() => {
        if (!data) return [];
        const nodeIds = new Set(filteredNodes.map(n => n.path));

        const structural = data.edges
            .filter(e => nodeIds.has(e.source) && nodeIds.has(e.target))
            .map(e => ({
                source: e.source,
                target: e.target,
                isAiBeam: false,
                isSemanticKnn: (e.kind ?? "wiki") === "semantic_knn",
                summary: undefined as string | undefined,
            }));
        const aiBeams = (data.ai_beams ?? [])
            .filter(
                (b: KmsAiBeamDto) =>
                    nodeIds.has(b.source_path) && nodeIds.has(b.target_path)
            )
            .map((b: KmsAiBeamDto) => ({
                source: b.source_path,
                target: b.target_path,
                isAiBeam: true,
                isSemanticKnn: false,
                summary: b.summary,
            }));

        const combined = [...structural, ...aiBeams];
        return combined.filter(l => {
            if (l.isAiBeam) return showAiBeams;
            if (l.isSemanticKnn) return showSemanticKnnEdges;
            return showWikiEdges;
        });
    }, [data, filteredNodes, showWikiEdges, showSemanticKnnEdges, showAiBeams]);

    const tagPathToTags = useMemo(() => {
        const m = new Map<string, string[]>();
        for (const n of indexedNotes ?? []) {
            if (n.path && n.tags?.length) m.set(n.path, n.tags);
        }
        return m;
    }, [indexedNotes]);

    const textFilteredNodes = useMemo(() => {
        const q = legendFilterText;
        if (!q.trim()) return filteredNodes;
        return filteredNodes.filter(n =>
            nodeMatchesGraphFilter(q, {
                title: n.title,
                path: n.path,
                folder_path: n.folder_path,
            })
        );
    }, [filteredNodes, legendFilterText]);

    const tagFilteredNodes = useMemo(() => {
        const tokens = parseTagFilterTokens(legendTagFilterText);
        if (tokens.length === 0) return textFilteredNodes;
        return textFilteredNodes.filter(n =>
            tagsMatchFilterTokens(tagPathToTags.get(n.path), tokens)
        );
    }, [textFilteredNodes, legendTagFilterText, tagPathToTags]);

    const textFilteredLinks = useMemo(() => {
        const ids = new Set(tagFilteredNodes.map(n => n.path));
        return filteredLinks.filter(l => ids.has(l.source) && ids.has(l.target));
    }, [filteredLinks, tagFilteredNodes]);

    const legendFilteredBundle = useMemo(
        () =>
            applyLegendVisibilityFilter({
                nodes: tagFilteredNodes.map(n => ({
                    id: n.path,
                    folder_path: n.folder_path ?? "",
                    node_type: n.node_type,
                })),
                links: textFilteredLinks,
                colorMode,
                hiddenFolderKeys,
                hiddenNodeTypes,
            }),
        [tagFilteredNodes, textFilteredLinks, colorMode, hiddenFolderKeys, hiddenNodeTypes]
    );

    const legendVisibleIdSet = useMemo(
        () => new Set(legendFilteredBundle.nodes.map(n => n.id)),
        [legendFilteredBundle.nodes]
    );

    const legendFilteredNodes = useMemo(
        () => tagFilteredNodes.filter(n => legendVisibleIdSet.has(n.path)),
        [tagFilteredNodes, legendVisibleIdSet]
    );

    const legendFilteredLinks = legendFilteredBundle.links;

    const islandComponents = useMemo(
        () =>
            computeWeaklyConnectedComponents(
                legendFilteredNodes.map(n => n.path),
                legendFilteredLinks
            ),
        [legendFilteredNodes, legendFilteredLinks]
    );

    const maxIslandBarDenominator = useMemo(
        () => Math.max(1, ...islandComponents.map(c => c.length)),
        [islandComponents]
    );

    useEffect(() => {
        if (focusedIslandIdx !== null && focusedIslandIdx >= islandComponents.length) {
            setFocusedIslandIdx(null);
        }
    }, [focusedIslandIdx, islandComponents.length]);

    const layoutNodes = useMemo(() => {
        if (
            islandIsolateHide &&
            focusedIslandIdx !== null &&
            islandComponents[focusedIslandIdx]
        ) {
            const s = new Set(islandComponents[focusedIslandIdx]);
            return legendFilteredNodes.filter(n => s.has(n.path));
        }
        return legendFilteredNodes;
    }, [islandIsolateHide, focusedIslandIdx, islandComponents, legendFilteredNodes]);

    const layoutLinks = useMemo(() => {
        const ids = new Set(layoutNodes.map(n => n.path));
        return legendFilteredLinks.filter(l => ids.has(l.source) && ids.has(l.target));
    }, [layoutNodes, legendFilteredLinks]);

    const islandFocusMemberSet = useMemo(() => {
        if (focusedIslandIdx === null || islandIsolateHide) return null;
        const comp = islandComponents[focusedIslandIdx];
        return comp ? new Set(comp) : null;
    }, [focusedIslandIdx, islandIsolateHide, islandComponents]);

    const markIslandBridgeExpand = useIslandBridgeMergeToast(islandComponents.length, toast);

    useEffect(() => {
        if (
            focusedIslandIdx !== null &&
            islandComponents[focusedIslandIdx]?.length
        ) {
            islandFrameIdsRef.current = new Set(islandComponents[focusedIslandIdx]);
        } else {
            islandFrameIdsRef.current = null;
        }
    }, [focusedIslandIdx, islandComponents]);

    const frameFocusedIsland2D = useCallback(() => {
        const ids = islandFrameIdsRef.current;
        if (!ids?.size || !svgRef.current || !zoomBehaviorRef.current) return;
        const { width, height } = dimensions;
        if (width < 32 || height < 32) return;
        const pad = 64;
        const subset = simulationNodesRef.current.filter(
            n => ids.has(n.id) && n.x != null && n.y != null
        );
        if (subset.length === 0) return;
        const nodePad = 48;
        const xs = subset.map(n => n.x!);
        const ys = subset.map(n => n.y!);
        const minX = Math.min(...xs) - nodePad;
        const maxX = Math.max(...xs) + nodePad;
        const minY = Math.min(...ys) - nodePad;
        const maxY = Math.max(...ys) + nodePad;
        const dx = Math.max(maxX - minX, 80);
        const dy = Math.max(maxY - minY, 80);
        const midX = (minX + maxX) / 2;
        const midY = (minY + maxY) / 2;
        const scale = Math.min(
            (width - 2 * pad) / dx,
            (height - 2 * pad) / dy,
            4
        );
        const tx = width / 2 - scale * midX;
        const ty = height / 2 - scale * midY;
        d3.select(svgRef.current)
            .transition()
            .duration(550)
            .call(
                zoomBehaviorRef.current.transform as any,
                d3.zoomIdentity.translate(tx, ty).scale(scale)
            );
    }, [dimensions]);

    const frameNodePaths2D = useCallback(
        (paths: string[]) => {
            const idSet = new Set(paths);
            if (!svgRef.current || !zoomBehaviorRef.current) return;
            const { width, height } = dimensions;
            if (width < 32 || height < 32) return;
            const pad = 64;
            const subset = simulationNodesRef.current.filter(
                n => idSet.has(n.id) && n.x != null && n.y != null
            );
            if (subset.length === 0) return;
            const nodePad = 48;
            const xs = subset.map(n => n.x!);
            const ys = subset.map(n => n.y!);
            const minX = Math.min(...xs) - nodePad;
            const maxX = Math.max(...xs) + nodePad;
            const minY = Math.min(...ys) - nodePad;
            const maxY = Math.max(...ys) + nodePad;
            const dx = Math.max(maxX - minX, 80);
            const dy = Math.max(maxY - minY, 80);
            const midX = (minX + maxX) / 2;
            const midY = (minY + maxY) / 2;
            const scale = Math.min(
                (width - 2 * pad) / dx,
                (height - 2 * pad) / dy,
                4
            );
            const tx = width / 2 - scale * midX;
            const ty = height / 2 - scale * midY;
            d3.select(svgRef.current)
                .transition()
                .duration(550)
                .call(
                    zoomBehaviorRef.current.transform as any,
                    d3.zoomIdentity.translate(tx, ty).scale(scale)
                );
        },
        [dimensions]
    );

    const pathEdgeSet = useMemo(
        () => pathEdgeSetFromDto(Boolean(pathResult?.found), pathResult?.edges),
        [pathResult]
    );

    const pathNodeSet = useMemo(
        () => pathNodeSetFromDto(Boolean(pathResult?.found), pathResult?.node_paths),
        [pathResult]
    );

    const findPath = useCallback(async () => {
        if (!pathFrom || !pathTo || pathFrom === pathTo) {
            setPathError("Pick two different notes.");
            return;
        }
        setPathError(null);
        try {
            const r = await getTaurpc().kms_get_graph_shortest_path(pathFrom, pathTo);
            setPathResult(r);
            setPathError(r.found ? null : (r.message ?? "No path found."));
        } catch (e) {
            setPathResult(null);
            setPathError(formatIpcOrRaw(e));
        }
    }, [pathFrom, pathTo]);

    const clearPath = useCallback(() => {
        setPathResult(null);
        setPathError(null);
    }, []);

    useEffect(() => {
        if (!data || !svgRef.current || !containerRef.current) return;

        const { width, height } = dimensions;
        if (width === 0 || height === 0) return;

        let cancelled = false;
        let simulation: d3.Simulation<GraphNode, undefined> | undefined;

        void (async () => {
        const svg = d3.select(svgRef.current!);
        svg.selectAll("*").remove(); // Clear previous rendering

        const g = svg.append("g");

        const rootDefs = svg.append("defs");
        rootDefs.append("style").text(`
            @keyframes pulse-ring {
                0% { transform: scale(0.8); opacity: 0.8; stroke-width: 2px; }
                50% { transform: scale(1.5); opacity: 0; stroke-width: 1px; }
                100% { transform: scale(0.8); opacity: 0; stroke-width: 0px; }
            }
            .pulse-circle {
                animation: pulse-ring 2s cubic-bezier(0.24, 0, 0.38, 1) infinite;
                transform-origin: center;
            }
            @keyframes recent-edit-pulse {
                0% { transform: scale(1); opacity: 0.35; stroke-width: 1.5px; }
                50% { transform: scale(1.35); opacity: 0.85; stroke-width: 2.5px; }
                100% { transform: scale(1); opacity: 0.35; stroke-width: 1.5px; }
            }
            .recent-edit-pulse {
                animation: recent-edit-pulse 2.4s ease-in-out infinite;
                transform-origin: center;
            }
        `);
        const linkGlowFilter = rootDefs
            .append("filter")
            .attr("id", "kms-link-glow")
            .attr("x", "-80%")
            .attr("y", "-80%")
            .attr("width", "260%")
            .attr("height", "260%");
        linkGlowFilter
            .append("feGaussianBlur")
            .attr("in", "SourceGraphic")
            .attr("stdDeviation", "1.15")
            .attr("result", "blur");
        const glowMerge = linkGlowFilter.append("feMerge");
        glowMerge.append("feMergeNode").attr("in", "blur");
        glowMerge.append("feMergeNode").attr("in", "SourceGraphic");

        // Zoom helper
        const zoom = d3.zoom<SVGSVGElement, unknown>()
            .scaleExtent([0.1, 4])
            .on("zoom", (event) => {
                g.attr("transform", event.transform);
            });

        zoomBehaviorRef.current = zoom;
        svg.call(zoom as any);

        const clusterLabelById = new Map<number, string>();
        for (const row of data.cluster_labels ?? []) {
            clusterLabelById.set(row.cluster_id, row.label);
        }

        const lmRange = graphLastModifiedRange(layoutNodes);
        const nodes: GraphNode[] = layoutNodes.map(n => {
            const ms = lastModifiedMs(n.last_modified);
            const r01 = recency01(ms, lmRange.min, lmRange.max);
            const pulseHighlight = shouldPulseRecent(r01, pulseTopPercent, pulseEnabled);
            const islandDimmed = Boolean(
                islandFocusMemberSet && !islandFocusMemberSet.has(n.path)
            );
            return {
                id: n.path,
                title: n.title,
                nodeType: n.node_type || "note",
                folderPath: n.folder_path || "",
                lastModified: n.last_modified || "",
                numLinks: data.edges.filter(e => e.source === n.path || e.target === n.path).length,
                linkCentrality: n.link_centrality ?? 0,
                recency01: r01,
                pulseHighlight,
                clusterId: n.cluster_id,
                islandDimmed,
            };
        });

        const links: GraphLink[] = layoutLinks
            .map(e => ({
                source: e.source,
                target: e.target,
                isAiBeam: e.isAiBeam,
                isSemanticKnn: e.isSemanticKnn,
                summary: e.summary
            }));

        // Clustering logic: Calculate centers for folders
        const folders = Array.from(new Set(nodes.map(n => n.folderPath)));
        const folderCenters = new Map<string, { x: number, y: number }>();
        folders.forEach((f, i) => {
            const angle = (i / (folders.length || 1)) * 2 * Math.PI;
            const radius = Math.min(width, height) * 0.3;
            folderCenters.set(f, {
                x: width / 2 + Math.cos(angle) * radius,
                y: height / 2 + Math.sin(angle) * radius
            });
        });

        // Clustering logic: Calculate centers for semantic clusters (one slot per cluster id present)
        const clusterCenters = new Map<number, { x: number, y: number }>();
        let maxClusterId = -1;
        for (const n of nodes) {
            if (n.clusterId !== undefined && n.clusterId !== null) {
                maxClusterId = Math.max(maxClusterId, n.clusterId);
            }
        }
        const numClusters = Math.max(1, maxClusterId + 1);
        for (let i = 0; i < numClusters; i++) {
            const angle = (i / numClusters) * 2 * Math.PI;
            const radius = Math.min(width, height) * 0.28;
            clusterCenters.set(i, {
                x: width / 2 + Math.cos(angle) * (radius * 1.2),
                y: height / 2 + Math.sin(angle) * (radius * 1.2)
            });
        }

        const wwThreshold = graphVisualPrefs.webworkerLayoutThreshold;
        const useWorker = wwThreshold > 0 && nodes.length >= wwThreshold;
        let workerLayoutApplied = false;

        if (useWorker) {
            const userTickCap = graphVisualPrefs.webworkerLayoutMaxTicks;
            const scaledTicks = 100 + Math.floor(nodes.length * 0.35);
            const maxTicks = Math.max(40, Math.min(userTickCap, scaledTicks));
            const alphaMin = graphVisualPrefs.webworkerLayoutAlphaMin;

            setWorkerLayoutBusy(true);
            try {
                const positions = await runKmsGraphForceLayoutWorker({
                    width,
                    height,
                    linkDistance: 100,
                    chargeStrength: -400,
                    xyStrength: 0.08,
                    alphaMin,
                    maxTicks,
                    nodes: nodes.map(n => ({
                        id: n.id,
                        folderPath: n.folderPath,
                        clusterId:
                            n.clusterId === undefined || n.clusterId === null ? null : n.clusterId,
                        collisionRadius: graphNodeCollisionRadius(n.numLinks, n.linkCentrality),
                    })),
                    links: links.map(l => ({
                        source: l.source as string,
                        target: l.target as string,
                    })),
                });
                if (cancelled) return;
                if (positions && positions.length > 0) {
                    const posMap = new Map(positions.map(p => [p.id, p]));
                    let matched = 0;
                    for (const n of nodes) {
                        const p = posMap.get(n.id);
                        if (p) {
                            n.x = p.x;
                            n.y = p.y;
                            matched++;
                        }
                    }
                    if (matched > 0) workerLayoutApplied = true;
                }
            } finally {
                if (!cancelled) setWorkerLayoutBusy(false);
            }
        }

        if (cancelled) return;

        // Color scale for node types
        const colorScale = d3.scaleOrdinal<string>()
            .domain(["note", "skill", "image", "asset"])
            .range(["var(--dc-accent)", "#f59e0b", "#ec4899", "#8b5cf6"]);

        const nodeColor = (d: GraphNode) =>
            colorMode === "folder"
                ? colorForFolderKey(d.folderPath, folderColorMap)
                : colorScale(d.nodeType);

        simulation = d3.forceSimulation<GraphNode>(nodes)
            .force("link", d3.forceLink<GraphNode, GraphLink>(links).id(d => d.id).distance(100))
            .force("charge", d3.forceManyBody().strength(-400))
            .force("center", d3.forceCenter(width / 2, height / 2))
            .force("x", d3.forceX<GraphNode>(d => {
                if (d.clusterId !== undefined && d.clusterId !== null) return clusterCenters.get(d.clusterId)?.x || width / 2;
                return folderCenters.get(d.folderPath)?.x || width / 2;
            }).strength(0.08))
            .force("y", d3.forceY<GraphNode>(d => {
                if (d.clusterId !== undefined && d.clusterId !== null) return clusterCenters.get(d.clusterId)?.y || height / 2;
                return folderCenters.get(d.folderPath)?.y || height / 2;
            }).strength(0.08))
            .force("collision", d3.forceCollide<GraphNode>().radius(d =>
                graphNodeCollisionRadius(d.numLinks, d.linkCentrality)));

        // Arrow head definition
        svg.append("defs").append("marker")
            .attr("id", "arrowhead")
            .attr("viewBox", "-0 -5 10 10")
            .attr("refX", 25)
            .attr("refY", 0)
            .attr("orient", "auto")
            .attr("markerWidth", 6)
            .attr("markerHeight", 6)
            .attr("xoverflow", "visible")
            .append("svg:path")
            .attr("d", "M 0,-5 L 10 ,0 L 0,5")
            .attr("fill", "var(--dc-text-muted)")
            .style("opacity", 0.5)
            .style("stroke", "none");

        svg.append("defs").append("marker")
            .attr("id", "arrowhead-ai")
            .attr("viewBox", "-0 -5 10 10")
            .attr("refX", 25)
            .attr("refY", 0)
            .attr("orient", "auto")
            .attr("markerWidth", 8)
            .attr("markerHeight", 8)
            .attr("xoverflow", "visible")
            .append("svg:path")
            .attr("d", "M 0,-5 L 10 ,0 L 0,5")
            .attr("fill", "#a855f7")
            .style("stroke", "none");

        // Edges
        const link = g.append("g")
            .attr("class", "links")
            .selectAll("line")
            .data(links)
            .enter().append("line")
            .attr("stroke", d => {
                const s = d.source as string;
                const t = d.target as string;
                if (pathEdgeSet.has(undirectedEdgeKey(s, t))) return "#10b981";
                if (d.isAiBeam) return "#c084fc";
                if (d.isSemanticKnn) return "rgba(251, 191, 36, 0.92)";
                return "rgba(125, 211, 252, 0.85)";
            })
            .attr("stroke-width", d => {
                const s = d.source as string;
                const t = d.target as string;
                if (pathEdgeSet.has(undirectedEdgeKey(s, t))) return 4;
                if (d.isAiBeam) return 3;
                if (d.isSemanticKnn) return 1.4;
                return 1.25;
            })
            .attr("stroke-opacity", d => {
                const s = d.source as string;
                const t = d.target as string;
                if (pathEdgeSet.has(undirectedEdgeKey(s, t))) return 1;
                const base = d.isAiBeam ? 0.85 : d.isSemanticKnn ? 0.62 : 0.5;
                const ns = nodes.find(x => x.id === s);
                const nt = nodes.find(x => x.id === t);
                const dim = Boolean(ns?.islandDimmed || nt?.islandDimmed);
                return dim ? base * 0.12 : base;
            })
            .attr("stroke-dasharray", d =>
                d.isAiBeam ? "5,5" : d.isSemanticKnn ? "2,4" : "none"
            )
            .attr("marker-end", d => d.isAiBeam ? "url(#arrowhead-ai)" : "url(#arrowhead)")
            .attr("filter", d => {
                const s = d.source as string;
                const t = d.target as string;
                if (pathEdgeSet.has(undirectedEdgeKey(s, t))) return null;
                return "url(#kms-link-glow)";
            });

        link.filter(d => !!d.isAiBeam)
            .append("title")
            .text(d => `AI Summary: ${d.summary}`);

        // Nodes
        // Node icons (simplified paths)
        const node = g.append("g")
            .attr("class", "nodes")
            .selectAll(".node")
            .data(nodes)
            .enter().append("g")
            .attr("class", "node")
            .attr("data-path", d => d.id)
            .call(d3.drag<SVGGElement, GraphNode>()
                .on("start", dragstarted)
                .on("drag", dragged)
                .on("end", dragended))
            .on("click", (event, d) => {
                if (event.defaultPrevented) return;
                onSelectNote(d.id);
            });

        node.filter(d => d.pulseHighlight && d.id !== activeNotePath)
            .append("circle")
            .attr("r", d => Math.sqrt(graphNodeSizeWeight(d.numLinks, d.linkCentrality)) * 3 + 14)
            .attr("fill", "none")
            .attr("stroke", "#2dd4bf")
            .attr("class", "recent-edit-pulse")
            .style("pointer-events", "none");

        // Pulsing ring for active node
        node.filter(d => d.id === activeNotePath)
            .append("circle")
            .attr("r", d => Math.sqrt(graphNodeSizeWeight(d.numLinks, d.linkCentrality)) * 3 + 12)
            .attr("fill", "none")
            .attr("stroke", d => nodeColor(d))
            .attr("class", "pulse-circle")
            .style("pointer-events", "none");

        // Node circles
        node.append("circle")
            .attr("r", d => Math.sqrt(graphNodeSizeWeight(d.numLinks, d.linkCentrality)) * 3 + 8)
            .attr("fill", d => nodeColor(d))
            .attr("fill-opacity", d =>
                d.islandDimmed ? 0.04 : d.id === activeNotePath ? 0.4 : 0.15
            )
            .attr("stroke", d =>
                pathNodeSet.has(d.id) ? "#10b981" : nodeColor(d)
            )
            .attr("stroke-width", d => {
                if (d.id === activeNotePath) return 4;
                if (pathNodeSet.has(d.id)) return 4;
                return 2;
            })
            .style("cursor", "pointer")
            .style("transition", "all 0.2s ease")
            .on("mouseover", function (event, d) {
                d3.select(this)
                    .attr("stroke-width", 4)
                    .attr("fill-opacity", d.islandDimmed ? 0.08 : 0.3);
            })
            .on("mouseout", function (event, d) {
                const w =
                    d.id === activeNotePath ? 4 : pathNodeSet.has(d.id) ? 4 : 2;
                const fo = d.islandDimmed
                    ? 0.04
                    : d.id === activeNotePath
                      ? 0.4
                      : 0.15;
                d3.select(this).attr("stroke-width", w).attr("fill-opacity", fo);
            });

        // Icon path
        node.append("path")
            .attr("d", d => kmsNodeIconPath(d.nodeType))
            .attr("transform", "scale(0.6) translate(-12, -12)")
            .attr("fill", d => nodeColor(d))
            .attr("fill-opacity", d => (d.islandDimmed ? 0.12 : 0.9))
            .style("pointer-events", "none");

        // Inner glows for types
        node.append("circle")
            .attr("r", d => Math.sqrt(graphNodeSizeWeight(d.numLinks, d.linkCentrality)) * 3 + 2)
            .attr("fill", d => nodeColor(d))
            .style("filter", "blur(6px)")
            .style("opacity", d => (d.islandDimmed ? 0.08 : 0.52))
            .style("pointer-events", "none");

        // Tooltip (simple SVG title for now)
        node.append("title")
            .text(d => `${d.title}\nType: ${d.nodeType}\nFolder: ${d.folderPath || 'Root'}\nModified: ${d.lastModified}`);

        // Node labels
        node.append("text")
            .attr("dy", d => Math.sqrt(graphNodeSizeWeight(d.numLinks, d.linkCentrality)) * 3 + 20)
            .attr("text-anchor", "middle")
            .text(d => d.title)
            .attr("fill", "var(--dc-text)")
            .style("font-size", "10px")
            .style("pointer-events", "none")
            .style("opacity", d => (d.islandDimmed ? 0.2 : 1))
            .style("text-shadow", "0 1px 2px rgba(0,0,0,0.5)");

        const topicLayer = g.append("g").attr("class", "cluster-topic-labels");

        const onSimTick = () => {
            link
                .attr("x1", d => (d.source as GraphNode).x!)
                .attr("y1", d => (d.source as GraphNode).y!)
                .attr("x2", d => (d.target as GraphNode).x!)
                .attr("y2", d => (d.target as GraphNode).y!);

            node.attr("transform", d => `translate(${d.x},${d.y})`);
            simulationNodesRef.current = nodes;

            const byC = new Map<number, { sx: number; sy: number; w: number; dim: number }>();
            for (const d of nodes) {
                if (
                    d.clusterId === null ||
                    d.clusterId === undefined ||
                    d.x == null ||
                    d.y == null
                ) {
                    continue;
                }
                const cid = d.clusterId;
                const cur = byC.get(cid);
                const dim = d.islandDimmed ? 1 : 0;
                if (!cur) {
                    byC.set(cid, { sx: d.x, sy: d.y, w: 1, dim });
                } else {
                    cur.sx += d.x;
                    cur.sy += d.y;
                    cur.w += 1;
                    cur.dim += dim;
                }
            }
            const topicRows: {
                cid: number;
                x: number;
                y: number;
                t: string;
                op: number;
            }[] = [];
            for (const [cid, v] of byC) {
                const text = clusterLabelById.get(cid);
                if (!text) continue;
                const dimFrac = v.dim / Math.max(1, v.w);
                topicRows.push({
                    cid,
                    x: v.sx / v.w,
                    y: v.sy / v.w - 32,
                    t: text,
                    op: 0.12 + 0.82 * (1 - dimFrac * 0.9),
                });
            }
            topicLayer
                .selectAll<SVGTextElement, (typeof topicRows)[0]>("text")
                .data(topicRows, d => String(d.cid))
                .join(
                    enter =>
                        enter
                            .append("text")
                            .attr("text-anchor", "middle")
                            .style("font-size", "15px")
                            .style("font-weight", "800")
                            .style("letter-spacing", "0.06em")
                            .attr("fill", "rgba(186, 230, 253, 0.96)")
                            .style("paint-order", "stroke")
                            .attr("stroke", "rgba(2, 8, 22, 0.9)")
                            .attr("stroke-width", "5px")
                            .style("pointer-events", "none")
                            .style(
                                "filter",
                                "drop-shadow(0 0 12px rgba(56, 189, 248, 0.35))"
                            ),
                    update => update,
                    exit => exit.remove()
                )
                .attr("x", d => d.x)
                .attr("y", d => d.y)
                .style("opacity", d => d.op)
                .text(d => d.t);
        };

        simulation!.on("tick", onSimTick);

        if (workerLayoutApplied) {
            simulation!.stop();
            simulation!.alpha(0);
            simulation!.tick();
            onSimTick();
        }

        function dragstarted(event: any, d: any) {
            if (!event.active) simulation!.alphaTarget(0.3).restart();
            d.fx = d.x;
            d.fy = d.y;
        }

        function dragged(event: any, d: any) {
            d.fx = event.x;
            d.fy = event.y;
        }

        function dragended(event: any, d: any) {
            if (!event.active) simulation!.alphaTarget(0);
            d.fx = null;
            d.fy = null;
        }

        })();

        return () => {
            cancelled = true;
            setWorkerLayoutBusy(false);
            simulation?.stop();
        };
    }, [
        data,
        layoutNodes,
        layoutLinks,
        filteredNodes,
        onSelectNote,
        dimensions,
        pathEdgeSet,
        pathNodeSet,
        activeNotePath,
        colorMode,
        folderColorMap,
        pulseEnabled,
        pulseTopPercent,
        islandFocusMemberSet,
        graphVisualPrefs.webworkerLayoutThreshold,
        graphVisualPrefs.webworkerLayoutMaxTicks,
        graphVisualPrefs.webworkerLayoutAlphaMin,
    ]);

    useEffect(() => {
        if (svgRef.current && zoomBehaviorRef.current && resetKey !== undefined) {
            d3.select(svgRef.current)
                .transition()
                .duration(750)
                .call(zoomBehaviorRef.current.transform as any, d3.zoomIdentity);
        }
    }, [resetKey]);

    useEffect(() => {
        if (!graphNavigateRequest || !isVisible) return;
        if (graphNavigateRequest.token === lastGraphNavTokenRef.current) return;
        if (loading || !data) return;

        const path = graphNavigateRequest.path;
        const inGraph = data.nodes.some(n => n.path === path);
        if (!inGraph) {
            lastGraphNavTokenRef.current = graphNavigateRequest.token;
            const base = path.split(/[/\\]/).pop() ?? path;
            setLegendFilterText(base);
            writeLegendFilterQuery(base);
            toast({
                title: "Note not on this graph page",
                description:
                    "Try full graph view, change pagination, or clear filters. A filename filter was applied to help locate the note.",
                variant: "destructive",
            });
            return;
        }

        lastGraphNavTokenRef.current = graphNavigateRequest.token;
        const idx = componentIndexContaining(islandComponents, path);
        if (idx >= 0) setFocusedIslandIdx(idx);
        const t = window.setTimeout(() => frameNodePaths2D([path]), 900);
        return () => clearTimeout(t);
    }, [
        graphNavigateRequest,
        isVisible,
        loading,
        data,
        islandComponents,
        toast,
        frameNodePaths2D,
    ]);

    if (!paging || loading) {
        return (
            <div className="flex-1 flex flex-col items-center justify-center bg-dc-bg/50 backdrop-blur-sm">
                <Loader2 className="h-8 w-8 animate-spin text-dc-accent mb-4" />
                <span className="text-sm font-medium text-dc-text-muted">Loading Knowledge Graph...</span>
            </div>
        );
    }

    if (error) {
        return (
            <div className="flex-1 flex flex-col items-center justify-center bg-dc-bg">
                <div className="bg-red-500/10 p-6 rounded-2xl border border-red-500/20 text-center max-w-sm">
                    <p className="text-sm text-red-500 font-medium mb-4">{error}</p>
                    <Button variant="secondary" size="sm" onClick={fetchData} className="gap-2">
                        <RefreshCw size={14} /> Retry
                    </Button>
                </div>
            </div>
        );
    }

    const pagedEmpty =
        isPagedView && data && data.nodes.length === 0 && data.pagination;

    return (
        <div ref={containerRef} className="flex-1 relative overflow-hidden bg-[#050814] select-none">
            <KmsGraphConstellationBackdrop
                hex={{
                    cellRadius: graphVisualPrefs.hexCellRadius,
                    layerOpacity: graphVisualPrefs.hexLayerOpacity,
                    strokeWidth: graphVisualPrefs.hexStrokeWidth,
                    strokeOpacity: graphVisualPrefs.hexStrokeOpacity,
                }}
            />
            <svg ref={svgRef} className="relative z-10 h-full w-full" />

            {workerLayoutBusy && (
                <div
                    className="pointer-events-none absolute top-3 left-1/2 z-20 flex -translate-x-1/2 items-center gap-2 rounded-full border border-dc-border/80 bg-dc-bg-secondary/95 px-3 py-1.5 text-xs font-medium text-dc-text-muted shadow-lg backdrop-blur-sm"
                    role="status"
                    aria-live="polite"
                >
                    <Loader2 className="h-3.5 w-3.5 shrink-0 animate-spin text-dc-accent" aria-hidden />
                    <span>Layout running...</span>
                </div>
            )}

            {pagedEmpty && paging?.kind === "paged" && (
                <div className="absolute inset-0 z-40 flex items-center justify-center bg-dc-bg/85 backdrop-blur-sm pointer-events-auto px-4">
                    <div className="max-w-md rounded-2xl border border-dc-border bg-dc-bg-secondary/95 p-6 text-center shadow-xl">
                        <p className="text-sm font-semibold text-dc-text mb-2">No notes on this page</p>
                        <p className="text-xs text-dc-text-muted mb-4">
                            The offset may be past the end of the vault, or there are no indexed notes matching the
                            current graph build. Try the first page or load the full graph.
                        </p>
                        <div className="flex flex-wrap justify-center gap-2">
                            <Button
                                type="button"
                                variant="secondary"
                                size="sm"
                                onClick={() => {
                                    writeGraphSession("paged", 0, paging.limit);
                                    setPaging({ kind: "paged", offset: 0, limit: paging.limit });
                                }}
                            >
                                First page
                            </Button>
                            <Button
                                type="button"
                                variant="secondary"
                                size="sm"
                                className="border-dc-accent/40"
                                onClick={() => {
                                    writeGraphSession("full", 0, DEFAULT_PAGE_LIMIT);
                                    setPaging({ kind: "full" });
                                }}
                            >
                                Full graph
                            </Button>
                        </div>
                    </div>
                </div>
            )}

            {isPagedView && data?.pagination && (
                <div className="absolute top-20 left-1/2 -translate-x-1/2 z-30 flex flex-wrap items-center gap-2 rounded-xl border border-dc-border bg-dc-bg-secondary/90 backdrop-blur-md px-3 py-2 pointer-events-auto shadow-lg max-w-[95vw]">
                    <span
                        className="text-[10px] text-dc-text-muted font-mono cursor-help"
                        title={PATH_SORT_HELP}
                    >
                        Page: {data.pagination.returned_nodes === 0
                            ? "0"
                            : `${data.pagination.offset + 1}-${data.pagination.offset + data.pagination.returned_nodes}`}{" "}
                        of {data.pagination.total_nodes} notes (path-sorted)
                    </span>
                    {paging?.kind === "paged" && (
                        <label className="flex items-center gap-1 text-[10px] text-dc-text-muted">
                            <span className="sr-only">Page size</span>
                            <select
                                className="rounded border border-dc-border bg-dc-bg px-1.5 py-0.5 text-[10px] font-mono text-dc-text"
                                title="Notes per page (resets to first page)"
                                aria-label="Notes per page"
                                value={String(clampPageLimit(paging.limit))}
                                onChange={e => {
                                    const newLimit = clampPageLimit(parseInt(e.target.value, 10));
                                    writeGraphSession("paged", 0, newLimit);
                                    setPaging({ kind: "paged", offset: 0, limit: newLimit });
                                }}
                            >
                                {pageSizeSelectOptions(paging.limit).map(n => (
                                    <option key={n} value={String(n)}>
                                        {n} / page
                                    </option>
                                ))}
                            </select>
                        </label>
                    )}
                    <Button
                        type="button"
                        variant="secondary"
                        size="sm"
                        className="h-7 text-[10px]"
                        disabled={data.pagination.offset === 0}
                        onClick={() => {
                            if (paging?.kind !== "paged") return;
                            const off = Math.max(0, paging.offset - paging.limit);
                            writeGraphSession("paged", off, paging.limit);
                            setPaging({ kind: "paged", offset: off, limit: paging.limit });
                        }}
                    >
                        Prev
                    </Button>
                    <Button
                        type="button"
                        variant="secondary"
                        size="sm"
                        className="h-7 text-[10px]"
                        disabled={!data.pagination.has_more}
                        onClick={() => {
                            if (paging?.kind !== "paged") return;
                            const off = paging.offset + paging.limit;
                            writeGraphSession("paged", off, paging.limit);
                            setPaging({ kind: "paged", offset: off, limit: paging.limit });
                        }}
                    >
                        Next
                    </Button>
                    <Button
                        type="button"
                        variant="secondary"
                        size="sm"
                        className="h-7 text-[10px] border-dc-accent/40"
                        onClick={() => {
                            writeGraphSession("full", 0, DEFAULT_PAGE_LIMIT);
                            setPaging({ kind: "full" });
                        }}
                    >
                        Full graph
                    </Button>
                </div>
            )}

            {data && !isPagedView && indexedNoteCount > 0 && (
                <div className="absolute top-20 right-6 z-20">
                    <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        className="h-7 text-[10px] text-dc-text-muted"
                        onClick={() => {
                            const lim = clampPageLimit(readGraphSession().limit);
                            writeGraphSession("paged", 0, lim);
                            setPaging({ kind: "paged", offset: 0, limit: lim });
                        }}
                    >
                        Paged view
                    </Button>
                </div>
            )}

            {isPagedView && (
                <div className="absolute bottom-24 right-6 z-20 max-w-sm rounded-xl border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-[10px] text-amber-100 pointer-events-none">
                    Shortest path and hover preview are disabled in paged view. Use Full graph or Local graph for path exploration.
                </div>
            )}

            {data?.warnings && data.warnings.length > 0 && (
                <div className="absolute top-6 right-6 z-10 max-w-lg pointer-events-auto">
                    <div className="rounded-xl border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-xs text-amber-100 shadow-lg backdrop-blur-md">
                        <p className="font-semibold text-amber-200 mb-1">Graph notices</p>
                        <ul className="list-disc pl-4 space-y-1 text-amber-100/90">
                            {normalizeKmsGraphWarnings(data.warnings).map((w, i) => (
                                <li key={`${w.code ?? "raw"}-${i}`}>
                                    {w.message}
                                    {w.code && <span className="ml-1 align-middle text-[10px] text-amber-200/70">[{w.code}]</span>}
                                </li>
                            ))}
                        </ul>
                        {isPagedView && (
                            <p className="mt-2 border-t border-amber-500/25 pt-2 text-[10px] leading-snug text-amber-200/80">
                                Paged view runs clustering and AI beams only on the notes in this slice, so colors and beams are not comparable across pages. Messages about skipped semantics or caps refer to your Knowledge Graph settings (global and per-vault overrides).
                            </p>
                        )}
                    </div>
                </div>
            )}

            {/* Legend + color / edge toggles (collapsible dock for focus view) */}
            <div
                className={cn(
                    "absolute left-0 top-0 z-30 flex h-full max-h-full items-stretch pb-36 pl-6 pt-6 transition-transform duration-300 ease-out pointer-events-none will-change-transform",
                    graphPanelsCollapsed ? "-translate-x-[calc(100%-2.75rem)]" : "translate-x-0"
                )}
                style={{ width: "min(100vw - 1.5rem, 332px)" }}
            >
                <div className="pointer-events-auto flex min-h-0 w-[min(100vw-3rem,300px)] max-w-[min(100vw-3rem,300px)] shrink-0 flex-col gap-3">
                    <div className="relative min-h-0 max-h-[calc(100vh-10rem)] w-full overflow-y-auto overflow-x-hidden rounded-3xl border border-white/15 bg-gradient-to-br from-dc-bg-secondary/55 via-dc-bg-secondary/40 to-dc-bg/30 p-4 shadow-[0_16px_56px_rgba(0,0,0,0.45)] ring-1 ring-sky-400/20 backdrop-blur-2xl">
                <div className="pointer-events-none absolute inset-0 rounded-3xl bg-[radial-gradient(ellipse_at_top_left,rgba(56,189,248,0.12),transparent_58%)]" />
                <div className="relative">
                <div className="mb-3 flex items-start gap-3">
                    <div className="mt-0.5 h-3 w-3 shrink-0 rounded-full bg-sky-400 shadow-[0_0_14px_rgba(56,189,248,0.75)]" />
                    <div className="min-w-0">
                        <span className="block text-[10px] font-bold uppercase tracking-[0.18em] text-sky-200/95">
                            Knowledge constellation
                        </span>
                        <span className="mt-0.5 block text-[9px] tracking-wide text-dc-text-muted/85">
                            AI knowledge graph
                        </span>
                    </div>
                </div>

                <div className="mb-3 space-y-1.5">
                    <span className="text-[9px] uppercase tracking-wider text-dc-text-muted">Node colors</span>
                    <div className="flex rounded-lg border border-dc-border p-0.5 bg-dc-bg/50">
                        <button
                            type="button"
                            className={`flex-1 rounded-md px-2 py-1 text-[10px] font-medium transition-colors ${
                                colorMode === "type"
                                    ? "bg-dc-accent text-white"
                                    : "text-dc-text-muted hover:text-dc-text"
                            }`}
                            onClick={() => setColorModePersist("type")}
                        >
                            By type
                        </button>
                        <button
                            type="button"
                            className={`flex-1 rounded-md px-2 py-1 text-[10px] font-medium transition-colors ${
                                colorMode === "folder"
                                    ? "bg-dc-accent text-white"
                                    : "text-dc-text-muted hover:text-dc-text"
                            }`}
                            onClick={() => setColorModePersist("folder")}
                        >
                            By folder
                        </button>
                    </div>
                </div>

                <div className="mb-3 space-y-1">
                    <span className="text-[9px] uppercase tracking-wider text-dc-text-muted">
                        Search / filter
                    </span>
                    <input
                        type="search"
                        value={legendFilterText}
                        onChange={e => {
                            const v = e.target.value;
                            setLegendFilterText(v);
                            writeLegendFilterQuery(v);
                        }}
                        placeholder="Title, path, folder..."
                        className="w-full rounded-md border border-dc-border bg-dc-bg px-2 py-1.5 text-[10px] text-dc-text placeholder:text-dc-text-muted/60"
                        aria-label="Filter graph nodes"
                    />
                    {(indexedNotes?.length ?? 0) > 0 ? (
                        <input
                            type="search"
                            value={legendTagFilterText}
                            onChange={e => setLegendTagFilterText(e.target.value)}
                            placeholder="Tags (comma or space)..."
                            className="mt-1.5 w-full rounded-md border border-dc-border bg-dc-bg px-2 py-1.5 text-[10px] text-dc-text placeholder:text-dc-text-muted/60"
                            aria-label="Filter graph nodes by indexed YAML tags"
                            title="Uses tags from the vault note index (frontmatter). Any token can match any tag."
                        />
                    ) : null}
                </div>

                <div className="mb-3 space-y-2">
                    <span className="text-[9px] uppercase tracking-wider text-dc-text-muted">
                        Recent edits (pulse)
                    </span>
                    <label className="flex items-center gap-2 cursor-pointer">
                        <input
                            type="checkbox"
                            className="rounded border-dc-border"
                            checked={pulseEnabled}
                            onChange={e => {
                                const v = e.target.checked;
                                setPulseEnabled(v);
                                writePulseEnabled(v);
                            }}
                        />
                        <span className="text-[10px] text-dc-text-muted">Highlight newest notes</span>
                    </label>
                    {pulseEnabled && (
                        <div className="pl-1">
                            <label className="text-[9px] text-dc-text-muted block mb-1">
                                Top {pulseTopPercent}% by date in view
                            </label>
                            <input
                                type="range"
                                min={5}
                                max={50}
                                step={5}
                                value={pulseTopPercent}
                                onChange={e => {
                                    const v = Number.parseInt(e.target.value, 10);
                                    setPulseTopPercent(v);
                                    writePulseTopPercent(v);
                                }}
                                className="w-full h-1.5 accent-teal-500"
                            />
                        </div>
                    )}
                    <div className="flex items-center gap-2 pl-1">
                        <div className="w-2 h-2 rounded-full border-2 border-teal-400" />
                        <span className="text-[10px] text-dc-text-muted">Pulse ring (recent)</span>
                    </div>
                </div>

                <label className="flex items-center gap-2 mb-2 cursor-pointer">
                    <input
                        type="checkbox"
                        className="rounded border-dc-border"
                        checked={legendShowTypes}
                        onChange={e => {
                            const v = e.target.checked;
                            setLegendShowTypes(v);
                            writeLegendPanelTypes(v);
                        }}
                    />
                    <span className="text-[9px] text-dc-text-muted">Show type legend</span>
                </label>
                {legendShowTypes && colorMode === "type" && (
                    <div className="flex flex-col gap-1.5 mb-3 pl-1 max-h-36 overflow-y-auto pr-1">
                        {LEGEND_TYPE_ROWS.map(row => (
                            <label
                                key={row.type}
                                className="flex items-center gap-2 cursor-pointer min-w-0"
                            >
                                <input
                                    type="checkbox"
                                    className="rounded border-dc-border shrink-0"
                                    checked={!hiddenNodeTypes.has(row.type)}
                                    onChange={() => {
                                        const wasHidden = hiddenNodeTypes.has(row.type);
                                        const next = new Set(hiddenNodeTypes);
                                        if (next.has(row.type)) next.delete(row.type);
                                        else next.add(row.type);
                                        if (wasHidden) markIslandBridgeExpand();
                                        setHiddenNodeTypes(next);
                                        writeHiddenNodeTypes(next);
                                        setFocusedIslandIdx(null);
                                    }}
                                />
                                <span className="text-[10px] text-dc-text-muted truncate">
                                    {row.label}
                                </span>
                            </label>
                        ))}
                    </div>
                )}

                <label className="flex items-center gap-2 mb-2 cursor-pointer">
                    <input
                        type="checkbox"
                        className="rounded border-dc-border"
                        checked={legendShowFolders}
                        onChange={e => {
                            const v = e.target.checked;
                            setLegendShowFolders(v);
                            writeLegendPanelFolders(v);
                        }}
                    />
                    <span className="text-[9px] text-dc-text-muted">Show folder palette</span>
                </label>
                {legendShowFolders && colorMode === "folder" && folderLegendRows.length > 0 && (
                    <div className="max-h-32 overflow-y-auto flex flex-col gap-1 mb-2 pl-1 pr-1">
                        {folderLegendRows.map(row => (
                            <label
                                key={row.key}
                                className="flex items-center gap-2 min-w-0 cursor-pointer"
                            >
                                <input
                                    type="checkbox"
                                    className="rounded border-dc-border shrink-0"
                                    checked={!hiddenFolderKeys.has(row.key)}
                                    onChange={() => {
                                        const wasHidden = hiddenFolderKeys.has(row.key);
                                        const next = new Set(hiddenFolderKeys);
                                        if (next.has(row.key)) next.delete(row.key);
                                        else next.add(row.key);
                                        if (wasHidden) markIslandBridgeExpand();
                                        setHiddenFolderKeys(next);
                                        writeHiddenFolderKeys(next);
                                        setFocusedIslandIdx(null);
                                    }}
                                />
                                <div
                                    className="w-2 h-2 rounded-full shrink-0"
                                    style={{ backgroundColor: row.color }}
                                />
                                <span
                                    className="text-[10px] text-dc-text-muted truncate"
                                    title={row.key}
                                >
                                    {row.label}
                                </span>
                            </label>
                        ))}
                    </div>
                )}

                <div className="mb-3 flex flex-wrap gap-2">
                    <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        className="h-7 text-[9px] px-2 text-dc-text-muted border border-dc-border/60"
                        onClick={() => {
                            markIslandBridgeExpand();
                            resetLegendVisibilityFilters();
                            setHiddenFolderKeys(new Set());
                            setHiddenNodeTypes(new Set());
                            setFocusedIslandIdx(null);
                        }}
                    >
                        Reset legend visibility
                    </Button>
                </div>

                <div className="mb-3 space-y-2 rounded-2xl border border-white/10 bg-black/20 px-2.5 py-2.5 shadow-inner backdrop-blur-sm">
                    <span className="block text-[9px] font-semibold uppercase tracking-[0.14em] text-sky-200/80">
                        Semantic clusters
                    </span>
                    <p className="text-[9px] leading-snug text-dc-text-muted/90">
                        {islandComponents.length} connected group
                        {islandComponents.length !== 1 ? "s" : ""}
                        {islandComponents.length > 0
                            ? ` (${islandComponents.map(c => c.length).join(" / ")})`
                            : ""}
                        {legendFilterText.trim()
                            ? " - search may span multiple groups."
                            : ""}
                        {isPagedView ? " Page-local counts." : ""}
                    </p>
                    {islandComponents.length > 0 && (
                        <div className="flex max-h-32 flex-col gap-1.5 overflow-y-auto pr-0.5">
                            {islandComponents.map((comp, idx) => {
                                const hue = KMS_CONSTELLATION_ISLAND_COLORS[idx % KMS_CONSTELLATION_ISLAND_COLORS.length];
                                const pct = Math.round((100 * comp.length) / maxIslandBarDenominator);
                                return (
                                    <button
                                        key={idx}
                                        type="button"
                                        className={`flex w-full min-w-0 items-center gap-2 rounded-lg border px-2 py-1.5 text-left transition-colors ${
                                            focusedIslandIdx === idx
                                                ? "border-sky-400/50 bg-sky-500/15 text-dc-text"
                                                : "border-white/10 text-dc-text-muted hover:bg-white/5"
                                        }`}
                                        onClick={() =>
                                            setFocusedIslandIdx(focusedIslandIdx === idx ? null : idx)
                                        }
                                    >
                                        <span
                                            className="h-2 w-2 shrink-0 rounded-full shadow-[0_0_6px_currentColor]"
                                            style={{ backgroundColor: hue, color: hue }}
                                        />
                                        <span className="min-w-0 flex-1 truncate text-[9px] font-medium">
                                            Cluster {idx + 1}
                                        </span>
                                        <span className="flex h-1.5 w-14 shrink-0 overflow-hidden rounded-full bg-white/10">
                                            <span
                                                className="h-full rounded-full transition-all"
                                                style={{
                                                    width: `${pct}%`,
                                                    backgroundColor: hue,
                                                    boxShadow: `0 0 8px ${hue}88`,
                                                }}
                                            />
                                        </span>
                                        <span className="w-7 shrink-0 text-right text-[9px] tabular-nums text-dc-text-muted">
                                            {comp.length}
                                        </span>
                                    </button>
                                );
                            })}
                        </div>
                    )}
                    <label className="flex items-center gap-2 cursor-pointer">
                        <input
                            type="checkbox"
                            className="rounded border-dc-border"
                            checked={islandIsolateHide}
                            onChange={e => setIslandIsolateHide(e.target.checked)}
                        />
                        <span className="text-[9px] text-dc-text-muted">
                            Hide other islands (else dim)
                        </span>
                    </label>
                    <div className="flex flex-wrap gap-1.5">
                        <Button
                            type="button"
                            variant="secondary"
                            size="sm"
                            className="h-7 text-[9px] px-2"
                            disabled={legendFilteredNodes.length === 0}
                            onClick={() => {
                                const p = activeNotePath ?? hoverPreview?.path ?? null;
                                if (!p) return;
                                const idx = componentIndexContaining(islandComponents, p);
                                if (idx >= 0) setFocusedIslandIdx(idx);
                            }}
                        >
                            Focus island of active / hover
                        </Button>
                        {focusedIslandIdx !== null && (
                            <>
                                <Button
                                    type="button"
                                    variant="secondary"
                                    size="sm"
                                    className="h-7 text-[9px] px-2"
                                    onClick={() => frameFocusedIsland2D()}
                                >
                                    Frame in 2D
                                </Button>
                                <Button
                                    type="button"
                                    variant="ghost"
                                    size="sm"
                                    className="h-7 text-[9px] px-2 text-dc-text-muted"
                                    onClick={() => setFocusedIslandIdx(null)}
                                >
                                    Clear island focus
                                </Button>
                            </>
                        )}
                    </div>
                </div>

                <label className="flex items-center gap-2 mb-2 cursor-pointer">
                    <input
                        type="checkbox"
                        className="rounded border-dc-border"
                        checked={legendShowEdgeToggles}
                        onChange={e => {
                            const v = e.target.checked;
                            setLegendShowEdgeToggles(v);
                            writeLegendPanelEdgeToggles(v);
                        }}
                    />
                    <span className="text-[9px] text-dc-text-muted">Show edge toggles</span>
                </label>
                {legendShowEdgeToggles && (
                    <div className="flex flex-col gap-2 mb-3 pl-1">
                        <label className="flex items-center gap-2 cursor-pointer">
                            <input
                                type="checkbox"
                                className="rounded border-dc-border"
                                checked={showWikiEdges}
                                onChange={e => {
                                    const v = e.target.checked;
                                    if (v) markIslandBridgeExpand();
                                    setShowWikiEdges(v);
                                    writeShowWikiEdges(v);
                                }}
                            />
                            <span className="text-[10px] text-dc-text-muted">Wiki links</span>
                        </label>
                        <label className="flex items-center gap-2 cursor-pointer">
                            <input
                                type="checkbox"
                                className="rounded border-dc-border"
                                checked={showSemanticKnnEdges}
                                onChange={e => {
                                    const v = e.target.checked;
                                    if (v) markIslandBridgeExpand();
                                    setShowSemanticKnnEdges(v);
                                    writeShowSemanticKnnEdges(v);
                                }}
                            />
                            <span className="text-[10px] text-dc-text-muted">Semantic kNN</span>
                        </label>
                        <label className="flex items-center gap-2 cursor-pointer">
                            <input
                                type="checkbox"
                                className="rounded border-dc-border"
                                checked={showAiBeams}
                                onChange={e => {
                                    const v = e.target.checked;
                                    if (v) markIslandBridgeExpand();
                                    setShowAiBeams(v);
                                    writeShowAiBeamEdges(v);
                                }}
                            />
                            <span className="text-[10px] text-dc-text-muted">AI beams</span>
                        </label>
                    </div>
                )}

                <div className="flex flex-col gap-1.5 mb-2">
                    <div className="flex items-center gap-2">
                        <div className="w-2 h-2 rounded-full bg-[#a855f7]" />
                        <span className="text-[10px] text-dc-text-muted">AI beam (edge style)</span>
                    </div>
                    <div className="flex items-center gap-2">
                        <div className="w-6 h-0.5 rounded bg-amber-300/90 border border-dashed border-amber-200/40" />
                        <span className="text-[10px] text-dc-text-muted">Semantic kNN</span>
                    </div>
                    <div className="flex items-center gap-2">
                        <div className="w-3 h-0.5 rounded bg-[#10b981]" />
                        <span className="text-[10px] text-dc-text-muted">Shortest path</span>
                    </div>
                </div>

                <div className="mt-4 pt-4 border-t border-dc-border/50 text-[10px] text-dc-text-muted space-y-1">
                    <div>
                        {layoutNodes.length} nodes - {layoutLinks.length} links
                        {textFilteredNodes.length < filteredNodes.length
                            ? ` (search: ${textFilteredNodes.length} of ${filteredNodes.length})`
                            : ""}
                        {tagFilteredNodes.length < textFilteredNodes.length
                            ? ` (tags: ${tagFilteredNodes.length} of ${textFilteredNodes.length})`
                            : ""}
                        {legendFilteredNodes.length < tagFilteredNodes.length
                            ? ` (legend: ${legendFilteredNodes.length} shown)`
                            : ""}
                        {islandIsolateHide && focusedIslandIdx !== null
                            ? " (one island)"
                            : ""}
                    </div>
                    {data != null && (data.build_time_ms ?? 0) > 0 && (
                        <div title="Server-side graph build time for the last fetch">
                            Build: {data.build_time_ms} ms
                        </div>
                    )}
                    {vaultDiag != null && (
                        <div
                            title="Vault-wide counts from kms_get_diagnostics"
                            className="text-[9px] opacity-90 leading-snug"
                        >
                            Vault: {vaultDiag.note_count} notes indexed, {vaultDiag.vector_count} vectors
                            {vaultDiag.error_log_count > 0
                                ? `, ${vaultDiag.error_log_count} warn/error log rows`
                                : ""}
                        </div>
                    )}
                </div>
                </div>
                    </div>

            {data && !isPagedView && (
                <div className="max-w-full shrink-0 rounded-2xl border border-dc-border bg-dc-bg-secondary/85 backdrop-blur-xl shadow-2xl p-3">
                    <div className="flex items-center gap-2 mb-2 text-dc-text">
                        <Route size={14} className="text-dc-accent shrink-0" />
                        <span className="text-[10px] font-bold uppercase tracking-wider">Shortest path</span>
                    </div>
                    <div className="flex flex-col gap-2">
                        <select
                            value={pathFrom}
                            onChange={e => setPathFrom(e.target.value)}
                            className="text-[11px] rounded-lg border border-dc-border bg-dc-bg px-2 py-1.5 text-dc-text max-w-full"
                            aria-label="From note"
                        >
                            <option value="">From note...</option>
                            {data.nodes.map(n => (
                                <option key={n.path} value={n.path}>
                                    {n.title || n.path}
                                </option>
                            ))}
                        </select>
                        <select
                            value={pathTo}
                            onChange={e => setPathTo(e.target.value)}
                            className="text-[11px] rounded-lg border border-dc-border bg-dc-bg px-2 py-1.5 text-dc-text max-w-full"
                            aria-label="To note"
                        >
                            <option value="">To note...</option>
                            {data.nodes.map(n => (
                                <option key={`to-${n.path}`} value={n.path}>
                                    {n.title || n.path}
                                </option>
                            ))}
                        </select>
                        <div className="flex gap-2">
                            <Button
                                type="button"
                                variant="secondary"
                                size="sm"
                                className="flex-1 text-[11px] h-8"
                                onClick={findPath}
                            >
                                Find path
                            </Button>
                            <Button
                                type="button"
                                variant="ghost"
                                size="sm"
                                className="text-[11px] h-8 px-2"
                                onClick={clearPath}
                            >
                                Clear
                            </Button>
                        </div>
                        {pathError && (
                            <p className="text-[10px] text-amber-500/90 leading-snug">{pathError}</p>
                        )}
                        {pathResult?.found && pathResult.node_paths.length > 0 && (
                            <p className="text-[10px] text-dc-text-muted leading-snug">
                                {pathResult.node_paths.length} hops
                                {pathResult.node_paths.length <= 8
                                    ? `: ${pathResult.node_paths.map(p => data.nodes.find(n => n.path === p)?.title ?? "").filter(Boolean).join(" -> ")}`
                                    : ""}
                            </p>
                        )}
                    </div>
                </div>
            )}
                </div>
                <button
                    type="button"
                    className="pointer-events-auto ml-1 flex h-28 w-9 shrink-0 flex-col items-center justify-center gap-1 self-center rounded-r-xl border border-dc-border bg-dc-bg-secondary/90 text-dc-text-muted shadow-lg backdrop-blur-md transition-colors hover:border-dc-accent/50 hover:text-dc-accent"
                    title={
                        graphPanelsCollapsed
                            ? "Show graph tools (filters, path, clusters). Shortcut: Ctrl+Shift+G"
                            : "Focus graph: hide side tools for full canvas. Shortcut: Ctrl+Shift+G"
                    }
                    aria-expanded={!graphPanelsCollapsed}
                    aria-label={graphPanelsCollapsed ? "Expand graph tools panel" : "Collapse graph tools panel"}
                    onClick={toggleGraphPanelsCollapsed}
                >
                    {graphPanelsCollapsed ? <ChevronRight size={18} /> : <ChevronLeft size={18} />}
                    <span className="text-[7px] font-bold uppercase leading-tight tracking-wide [writing-mode:vertical-rl]">
                        Tools
                    </span>
                </button>
            </div>

            {hoverPreview && hoverPreview.title !== undefined && (
                <div
                    className="fixed z-[100] max-w-sm max-h-[min(70vh,420px)] overflow-hidden rounded-2xl border border-dc-border/40 bg-gradient-to-br from-dc-bg-secondary/75 via-dc-bg-secondary/55 to-dc-bg/40 backdrop-blur-2xl shadow-[0_12px_48px_rgba(0,0,0,0.35)] ring-1 ring-dc-accent/15 p-4 pointer-events-none text-left"
                    style={{
                        left: hoverPreview.x + 14,
                        top: hoverPreview.y + 14,
                    }}
                >
                    <div className="absolute inset-0 rounded-2xl bg-[radial-gradient(ellipse_at_top_right,rgba(14,165,233,0.12),transparent_55%)] pointer-events-none" />
                    <div className="relative">
                    <p className="text-xs font-semibold text-dc-text mb-1 tracking-tight">
                        {hoverPreview.title || hoverPreview.path.split(/[/\\]/).pop() || hoverPreview.path}
                    </p>
                    {hoverPreview.excerpt ? (
                        <p className="text-[10px] text-dc-text-muted leading-relaxed line-clamp-6 whitespace-pre-wrap border-l-2 border-dc-accent/25 pl-2.5">
                            {hoverPreview.excerpt}
                        </p>
                    ) : (
                        <p className="text-[10px] text-dc-text-muted/70 italic">Loading preview...</p>
                    )}
                    {hoverPreview.last_modified && (
                        <p className="text-[9px] text-dc-text-muted/60 mt-2 font-mono">
                            {new Date(hoverPreview.last_modified).toLocaleString()}
                        </p>
                    )}
                    </div>
                </div>
            )}

            {/* Timeline UI (vision: temporal view) */}
            <div
                className={cn(
                    "pointer-events-none absolute bottom-6 inset-x-0 mx-auto flex flex-col items-center gap-2",
                    graphPanelsCollapsed
                        ? "w-[min(92vw,42rem)] min-w-[280px]"
                        : "w-1/2 min-w-[280px]"
                )}
            >
                <div className="pointer-events-auto flex h-12 w-full items-center gap-4 rounded-2xl border border-sky-400/20 bg-gradient-to-r from-dc-bg-secondary/70 via-dc-bg-secondary/50 to-dc-bg/40 p-3 shadow-[0_8px_40px_rgba(0,0,0,0.35)] ring-1 ring-white/10 backdrop-blur-2xl">
                    <div className="flex min-w-[108px] flex-col">
                        <span className="text-[8px] font-bold uppercase tracking-[0.2em] text-sky-300/90">
                            Temporal view
                        </span>
                        <span className="font-mono text-[10px] text-dc-text-muted">
                            {new Date(timeRange.min + (timeRange.max - timeRange.min) * (timeFilter / 100)).toLocaleDateString()}
                        </span>
                    </div>

                    <Button
                        type="button"
                        variant="secondary"
                        size="icon"
                        className="h-9 w-9 shrink-0 rounded-xl border border-white/15 bg-white/5"
                        title="Play timeline from past to present"
                        disabled={playingTimeline}
                        onClick={() => {
                            setTimeFilter(0);
                            setPlayingTimeline(true);
                        }}
                    >
                        <Play size={14} className={playingTimeline ? "opacity-40" : ""} />
                    </Button>

                    <input
                        type="range"
                        min="0"
                        max="100"
                        value={timeFilter}
                        onChange={(e) => setTimeFilter(Number.parseInt(e.target.value))}
                        className="h-2 flex-1 cursor-pointer appearance-none rounded-full bg-white/15 accent-sky-400 transition-all [&::-webkit-slider-thumb]:h-3.5 [&::-webkit-slider-thumb]:w-3.5 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:border-2 [&::-webkit-slider-thumb]:border-sky-200 [&::-webkit-slider-thumb]:bg-sky-400 [&::-webkit-slider-thumb]:shadow-[0_0_14px_rgba(56,189,248,0.85)]"
                    />

                    <div className="flex items-center gap-2 rounded-lg border border-white/10 bg-black/25 px-3 py-1">
                        <Activity size={12} className={timeFilter < 100 ? "animate-pulse text-sky-400" : "text-dc-text-muted/40"} />
                        <span className="text-[10px] font-bold text-dc-text-muted">{timeFilter}%</span>
                    </div>
                </div>
            </div>

            {/* Controls */}
            <div className="absolute bottom-6 right-6 flex flex-col gap-2">
                <Button
                    variant="secondary"
                    size="icon"
                    className="h-10 w-10 rounded-xl shadow-lg border border-dc-border bg-dc-bg-secondary/80 backdrop-blur-md"
                    onClick={copyGraphDebugInfo}
                    title="Copy graph debug info (JSON) to clipboard"
                >
                    <ClipboardList size={16} />
                </Button>
                <Button variant="secondary" size="icon" className="h-10 w-10 rounded-xl shadow-lg border border-dc-border bg-dc-bg-secondary/80 backdrop-blur-md"
                    onClick={fetchData} title="Refresh Graph">
                    <RefreshCw size={16} />
                </Button>
            </div>
        </div>
    );
}
