import React, { useEffect, useRef, useState, useMemo, useCallback } from "react";
import ForceGraph3D, { ForceGraphMethods } from "react-force-graph-3d";
import * as THREE from "three";
import SpriteText from "three-spritetext";
import { applyKmsGraphSpriteTextResolution } from "../../lib/kmsGraphSpriteText";
import { getTaurpc } from "../../lib/taurpc";
import {
    KmsNodeDto,
    KmsGraphPathDto,
    KmsNoteGraphPreviewDto,
    KmsGraphDto,
    KmsGraphPaginationDto,
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
import {
    linkOnPathSet,
    pathEdgeSetFromDto,
    pathNodeSetFromDto,
} from "../../lib/kmsGraphHelpers";
import { formatIpcOrRaw } from "../../lib/ipcError";
import { copyKmsGraphDebugToClipboard } from "../../lib/kmsGraphDebugClipboard";
import { normalizeKmsGraphWarnings } from "../../lib/kmsGraphWarnings";
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
    readShowSemanticEdges,
    writeShowSemanticEdges,
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
import { applyLegendVisibilityFilter, LEGEND_TYPE_ROWS } from "../../lib/kmsGraphLegendVisibility";
import {
    computeWeaklyConnectedComponents,
    componentIndexContaining,
} from "../../lib/kmsGraphIslands";
import { useIslandBridgeMergeToast } from "../../lib/kmsGraphIslandBridgeToast";
import {
    graphLastModifiedRange,
    lastModifiedMs,
    recency01,
    shouldPulseRecent,
} from "../../lib/kmsGraphPulse";
import { kmsNode3DShape } from "../../lib/kmsGraphNodeIcons";
import { nodeMatchesGraphFilter } from "../../lib/kmsGraphGraphFilter";
import { Loader2, RefreshCw, Box, Layers, Activity, Play, Route, ChevronLeft, ChevronRight, ClipboardList } from "lucide-react";
import { Button } from "../ui/button";
import { cn } from "../../lib/utils";
import { useToast } from "../ui/use-toast";
import {
    KmsGraphConstellationBackdrop,
    KMS_CONSTELLATION_ISLAND_COLORS,
} from "./KmsGraphConstellationBackdrop";
import { useKmsForceGraphBloom } from "../../lib/useKmsForceGraphBloom";
import { useKmsGraphVisualPrefs } from "../../lib/useKmsGraphVisualPrefs";
import { kmsGraphLog } from "../../lib/kmsGraphLog";

const PATH_SORT_HELP =
    "Notes are ordered by vault-relative path (lexicographic). Pagination slices that list; it is not ranked by importance.";

type GraphPaging =
    | null
    | { kind: "full" }
    | { kind: "paged"; offset: number; limit: number };

interface KmsGraph3DProps {
    onSelectNote: (path: string) => void;
    activeNotePath?: string | null;
    isVisible?: boolean;
    resetKey?: number;
    indexedNoteCount?: number;
    indexedNotes?: KmsNoteDto[];
    /** When token changes, zoom to this note path (if present in the current graph payload). */
    graphNavigateRequest?: { token: number; path: string } | null;
}

interface GraphNode extends Omit<KmsNodeDto, "id"> {
    id: string; // path used as unique graph ID
    dbId: number; // database i32 id
    name: string; // title
    val: number; // radius equivalent
    color: string;
    /** Undirected wiki degree in this graph payload (structural edges only). */
    num_links: number;
    isClusterLabel?: boolean;
    preview?: string | null;
}

interface GraphLink {
    source: string;
    target: string;
    isSemantic?: boolean;
    isSemanticKnn?: boolean;
    isAiBeam?: boolean;
    summary?: string;
}

export default function KmsGraph3D({
    onSelectNote,
    activeNotePath,
    isVisible,
    resetKey,
    indexedNoteCount = 0,
    indexedNotes,
    graphNavigateRequest = null,
}: KmsGraph3DProps) {
    const lastGraphDtoRef = useRef<KmsGraphDto | null>(null);
    const lastGraphNavTokenRef = useRef(0);
    const fgRef = useRef<ForceGraphMethods>();
    const [paging, setPaging] = useState<GraphPaging>(null);
    const [pagination, setPagination] = useState<KmsGraphPaginationDto | null>(null);
    const [data, setData] = useState<{ nodes: GraphNode[]; links: GraphLink[] } | null>(null);
    const [clusterLabels, setClusterLabels] = useState<any[]>([]);
    const [centroids, setCentroids] = useState<any[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [timeFilter, setTimeFilter] = useState(100); // 0-100%
    const [hoverNode, setHoverNode] = useState<string | null>(null);
    const [hoverLink, setHoverLink] = useState<GraphLink | null>(null);
    const [graphWarnings, setGraphWarnings] = useState<string[]>([]);
    const parsedGraphWarnings = useMemo(() => normalizeKmsGraphWarnings(graphWarnings), [graphWarnings]);
    const [lastBuildMs, setLastBuildMs] = useState<number | null>(null);
    const [vaultDiag, setVaultDiag] = useState<KmsDiagnosticsDto | null>(null);
    const [playingTimeline, setPlayingTimeline] = useState(false);
    const [pathFrom, setPathFrom] = useState("");
    const [pathTo, setPathTo] = useState("");
    const [pathResult, setPathResult] = useState<KmsGraphPathDto | null>(null);
    const [pathError, setPathError] = useState<string | null>(null);
    const [rpcPreview, setRpcPreview] = useState<KmsNoteGraphPreviewDto | null>(null);
    const previewTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

    const [colorMode, setColorMode] = useState<GraphColorMode>(() => readGraphColorMode());
    const [showWikiEdges, setShowWikiEdges] = useState(() => readShowWikiEdges());
    const [showAiBeams, setShowAiBeams] = useState(() => readShowAiBeamEdges());
    const [showSemanticKnnEdges, setShowSemanticKnnEdges] = useState(() =>
        readShowSemanticKnnEdges()
    );
    const [showSemanticEdges, setShowSemanticEdges] = useState(() => readShowSemanticEdges());
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

    const graphVisualPrefs = useKmsGraphVisualPrefs();
    useKmsForceGraphBloom(
        fgRef,
        Boolean(
            graphVisualPrefs.bloomEnabled && data && data.nodes.some(n => !n.isClusterLabel)
        ),
        {
            strength: graphVisualPrefs.bloomStrength,
            radius: graphVisualPrefs.bloomRadius,
            threshold: graphVisualPrefs.bloomThreshold,
        },
        [data?.nodes.length, paging]
    );

    const { toast } = useToast();

    const setColorModePersist = useCallback((m: GraphColorMode) => {
        setColorMode(m);
        writeGraphColorMode(m);
    }, []);

    const colorScale = useMemo(() => {
        return (type: string) => {
            switch (type) {
                case "skill": return "#f59e0b"; // Amber 500
                case "image": return "#ec4899"; // Pink 500
                case "asset": return "#8b5cf6"; // Violet 500
                default: return "#0ea5e9"; // Sky 500 (Note)
            }
        };
    }, []);

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
            lastGraphDtoRef.current = graphData;
            setPagination(graphData.pagination ?? null);
            setGraphWarnings(graphData.warnings ?? []);
            setLastBuildMs(graphData.build_time_ms ?? null);
            const degreeByPath = new Map<string, number>();
            graphData.edges.forEach((e) => {
                degreeByPath.set(e.source, (degreeByPath.get(e.source) ?? 0) + 1);
                degreeByPath.set(e.target, (degreeByPath.get(e.target) ?? 0) + 1);
            });
            const nodes: GraphNode[] = graphData.nodes.map((n) => {
                const num_links = degreeByPath.get(n.path) ?? 0;
                const pr = n.link_centrality ?? 0;
                const val = 0.4 + 2.2 * pr + 0.35 * Math.log10(num_links + 1);
                return {
                    ...n,
                    id: n.path,
                    dbId: n.id,
                    name: n.title,
                    num_links,
                    val: Math.min(4, Math.max(0.35, val)),
                    color: colorScale(n.node_type),
                };
            });

            const structuralLinks: GraphLink[] = graphData.edges.map(e => ({
                source: e.source,
                target: e.target,
                isSemanticKnn: (e.kind ?? "wiki") === "semantic_knn",
            }));

            // Generate Semantic Links (virtual)
            const semanticLinks: GraphLink[] = [];
            const clusterGroups = new Map<number, string[]>();
            nodes.forEach(n => {
                if (n.cluster_id !== null && n.cluster_id !== undefined) {
                    const list = clusterGroups.get(n.cluster_id) || [];
                    list.push(n.id);
                    clusterGroups.set(n.cluster_id, list);
                }
            });

            clusterGroups.forEach((nodeIds) => {
                if (nodeIds.length > 1) {
                    const rootId = nodeIds[0];
                    for (let i = 1; i < nodeIds.length; i++) {
                        semanticLinks.push({ source: rootId, target: nodeIds[i], isSemantic: true });
                    }
                }
            });

            // Generate Virtual Cluster Label Nodes
            const labelNodes: GraphNode[] = (graphData.cluster_labels || []).map(l => ({
                id: `cluster-label-${l.cluster_id}`,
                name: l.label,
                cluster_id: l.cluster_id,
                isClusterLabel: true,
                node_type: "cluster-label",
                path: `cluster-label-${l.cluster_id}`,
                dbId: -1,
                title: l.label,
                last_modified: "",
                folder_path: "",
                link_centrality: 0,
                num_links: 0,
                val: 0,
                color: "rgba(0, 255, 255, 0.8)"
            }));

            const aiBeamsLinks: GraphLink[] = (graphData.ai_beams ?? []).map((b) => ({
                source: b.source_path,
                target: b.target_path,
                isAiBeam: true,
                summary: b.summary,
            }));

            setData({ nodes: [...nodes, ...labelNodes], links: [...structuralLinks, ...semanticLinks, ...aiBeamsLinks] });
            setError(null);
        } catch (err) {
            kmsGraphLog.error("Failed to fetch 3D graph data:", err);
            setError(formatIpcOrRaw(err));
        } finally {
            setLoading(false);
        }
    }, [colorScale, paging]);

    const copyGraphDebugInfo = useCallback(() => {
        void copyKmsGraphDebugToClipboard(
            {
                graphView: "3d",
                data: lastGraphDtoRef.current,
                error,
                paging,
                indexedNoteCount,
                vaultDiag,
                pathFrom,
                pathTo,
                pathResult,
                pathError,
                hoverPreviewPath: rpcPreview?.path ?? null,
                extra: {
                    last_build_time_ms_client: lastBuildMs,
                    graph_warnings_ui: graphWarnings,
                },
            },
            { toast }
        );
    }, [
        error,
        paging,
        indexedNoteCount,
        vaultDiag,
        pathFrom,
        pathTo,
        pathResult,
        pathError,
        rpcPreview?.path,
        lastBuildMs,
        graphWarnings,
        toast,
    ]);

    const folderColorMap = useMemo(() => {
        if (!data) return new Map<string, string>();
        return buildFolderColorMap(
            data.nodes.filter(n => !n.isClusterLabel).map(n => n.folder_path ?? "")
        );
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

    const recoloredData = useMemo(() => {
        if (!data) return null;
        const notesOnly = data.nodes.filter(n => !n.isClusterLabel);
        const lmRange = graphLastModifiedRange(notesOnly);
        return {
            nodes: data.nodes.map(n => {
                if (n.isClusterLabel) return n;
                const color =
                    colorMode === "folder"
                        ? colorForFolderKey(n.folder_path ?? "", folderColorMap)
                        : colorScale(n.node_type);
                const ms = lastModifiedMs(n.last_modified);
                const r01 = recency01(ms, lmRange.min, lmRange.max);
                const pulseHighlight = shouldPulseRecent(r01, pulseTopPercent, pulseEnabled);
                return { ...n, color, pulseHighlight };
            }),
            links: data.links,
        };
    }, [data, colorMode, folderColorMap, colorScale, pulseEnabled, pulseTopPercent]);

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

    const timeRange = useMemo(() => {
        if (!data) return { min: 0, max: Date.now() };
        const timestamps = data.nodes
            .filter(n => !n.isClusterLabel && n.last_modified)
            .map(n => new Date(n.last_modified).getTime())
            .filter(t => !isNaN(t));
        if (timestamps.length === 0) return { min: 0, max: Date.now() };
        return { min: Math.min(...timestamps), max: Math.max(...timestamps) };
    }, [data]);

    const filteredData = useMemo(() => {
        if (!recoloredData) return { nodes: [], links: [] };

        const filterLinksByKind = (links: GraphLink[]) =>
            links.filter(l => {
                if (l.isAiBeam) return showAiBeams;
                if (l.isSemantic) return showSemanticEdges;
                if (l.isSemanticKnn) return showSemanticKnnEdges;
                return showWikiEdges;
            });

        if (timeFilter === 100) {
            return {
                nodes: recoloredData.nodes,
                links: filterLinksByKind(recoloredData.links as GraphLink[]),
            };
        }

        const threshold = timeRange.min + (timeRange.max - timeRange.min) * (timeFilter / 100);

        const filteredNodes = recoloredData.nodes.filter(n => {
            if (n.isClusterLabel) return true;
            const t = new Date(n.last_modified).getTime();
            return isNaN(t) || t <= threshold;
        });

        const nodeIds = new Set(filteredNodes.map(n => n.id));
        const filteredLinks = recoloredData.links.filter(l => {
            const s = typeof l.source === "string" ? l.source : (l.source as { id: string }).id;
            const t = typeof l.target === "string" ? l.target : (l.target as { id: string }).id;
            return nodeIds.has(s) && nodeIds.has(t);
        });

        return {
            nodes: filteredNodes,
            links: filterLinksByKind(filteredLinks as GraphLink[]),
        };
    }, [
        recoloredData,
        timeFilter,
        timeRange,
        showWikiEdges,
        showAiBeams,
        showSemanticKnnEdges,
        showSemanticEdges,
    ]);

    const tagPathToTags = useMemo(() => {
        const m = new Map<string, string[]>();
        for (const n of indexedNotes ?? []) {
            if (n.path && n.tags?.length) m.set(n.path, n.tags);
        }
        return m;
    }, [indexedNotes]);

    const textVizData = useMemo(() => {
        const q = legendFilterText.trim();
        if (!q) return filteredData;
        const nodes = filteredData.nodes.filter(n => {
            if (n.isClusterLabel) return true;
            return nodeMatchesGraphFilter(q, {
                title: n.title,
                path: n.path,
                folder_path: n.folder_path,
            });
        });
        const ids = new Set(nodes.map(n => n.id));
        const links = filteredData.links.filter(l => {
            const s = typeof l.source === "string" ? l.source : (l.source as { id: string }).id;
            const t = typeof l.target === "string" ? l.target : (l.target as { id: string }).id;
            return ids.has(s) && ids.has(t);
        });
        return { nodes, links };
    }, [filteredData, legendFilterText]);

    const vizData = useMemo(() => {
        const tokens = parseTagFilterTokens(legendTagFilterText);
        if (tokens.length === 0) return textVizData;
        const nodes = textVizData.nodes.filter(n => {
            if (n.isClusterLabel) return true;
            return tagsMatchFilterTokens(tagPathToTags.get(n.id), tokens);
        });
        const ids = new Set(nodes.map(n => n.id));
        const links = textVizData.links.filter(l => {
            const s = typeof l.source === "string" ? l.source : (l.source as { id: string }).id;
            const t = typeof l.target === "string" ? l.target : (l.target as { id: string }).id;
            return ids.has(s) && ids.has(t);
        });
        return { nodes, links };
    }, [textVizData, legendTagFilterText, tagPathToTags]);

    const legendFilteredData = useMemo(
        () =>
            applyLegendVisibilityFilter({
                nodes: vizData.nodes,
                links: vizData.links,
                colorMode,
                hiddenFolderKeys,
                hiddenNodeTypes,
            }),
        [vizData, colorMode, hiddenFolderKeys, hiddenNodeTypes]
    );

    const legendFilteredNodes = legendFilteredData.nodes;
    const legendFilteredLinks = legendFilteredData.links;

    const islandComponents = useMemo(() => {
        const noteNodes = legendFilteredNodes
            .filter(n => !n.isClusterLabel)
            .map(n => n.id);
        const noteIdSet = new Set(noteNodes);
        const noteLinks = legendFilteredLinks.filter(l => {
            const s = typeof l.source === "string" ? l.source : (l.source as { id: string }).id;
            const t = typeof l.target === "string" ? l.target : (l.target as { id: string }).id;
            return noteIdSet.has(s) && noteIdSet.has(t);
        });
        return computeWeaklyConnectedComponents(noteNodes, noteLinks);
    }, [legendFilteredNodes, legendFilteredLinks]);

    const maxIslandBarDenominator = useMemo(
        () => Math.max(1, ...islandComponents.map(c => c.length)),
        [islandComponents]
    );

    const markIslandBridgeExpand = useIslandBridgeMergeToast(islandComponents.length, toast);

    useEffect(() => {
        if (focusedIslandIdx !== null && focusedIslandIdx >= islandComponents.length) {
            setFocusedIslandIdx(null);
        }
    }, [focusedIslandIdx, islandComponents.length]);

    /**
     * Embedding cluster_ids on notes in the focused weakly-connected island.
     * Used to dim non-matching labels (dim mode) and to include only relevant topic labels when isolating.
     */
    const focusedIslandClusterIds = useMemo(() => {
        if (focusedIslandIdx === null) return null;
        const comp = islandComponents[focusedIslandIdx];
        if (!comp?.length) return null;
        const idSet = new Set(comp);
        const cids = new Set<number>();
        for (const n of legendFilteredNodes) {
            if (n.isClusterLabel || !idSet.has(n.id)) continue;
            if (n.cluster_id !== null && n.cluster_id !== undefined) {
                cids.add(n.cluster_id);
            }
        }
        return cids;
    }, [focusedIslandIdx, islandComponents, legendFilteredNodes]);

    const layoutNodes3d = useMemo(() => {
        if (
            islandIsolateHide &&
            focusedIslandIdx !== null &&
            islandComponents[focusedIslandIdx]
        ) {
            const s = new Set(islandComponents[focusedIslandIdx]);
            const embedIds = focusedIslandClusterIds;
            return legendFilteredNodes.filter(n => {
                if (s.has(n.id)) return true;
                if (!n.isClusterLabel) return false;
                if (!embedIds || embedIds.size === 0) return false;
                const cid = n.cluster_id;
                return cid !== null && cid !== undefined && embedIds.has(cid);
            });
        }
        return legendFilteredNodes;
    }, [
        islandIsolateHide,
        focusedIslandIdx,
        islandComponents,
        legendFilteredNodes,
        focusedIslandClusterIds,
    ]);

    const islandFocusMemberSet = useMemo(() => {
        if (focusedIslandIdx === null || islandIsolateHide) return null;
        const c = islandComponents[focusedIslandIdx];
        return c ? new Set(c) : null;
    }, [focusedIslandIdx, islandIsolateHide, islandComponents]);

    /** Drop cross-wiki-island semantic edges from the sim when dimming an island (avoids ghost lines + extra repulsion). */
    const layoutLinks3d = useMemo(() => {
        const ids = new Set(layoutNodes3d.map(n => String(n.id)));
        return legendFilteredLinks.filter(l => {
            const s = typeof l.source === "string" ? l.source : (l.source as { id: string }).id;
            const t = typeof l.target === "string" ? l.target : (l.target as { id: string }).id;
            const sid = String(s);
            const tid = String(t);
            if (!ids.has(sid) || !ids.has(tid)) return false;
            if (
                l.isSemantic &&
                islandFocusMemberSet &&
                (!islandFocusMemberSet.has(sid) || !islandFocusMemberSet.has(tid))
            ) {
                return false;
            }
            return true;
        });
    }, [layoutNodes3d, legendFilteredLinks, islandFocusMemberSet]);

    const forceGraphData = useMemo(
        () => ({
            nodes: layoutNodes3d.map(n => {
                let kmsIslandDimmed = false;
                if (islandFocusMemberSet) {
                    if (n.isClusterLabel) {
                        if (!focusedIslandClusterIds || focusedIslandClusterIds.size === 0) {
                            kmsIslandDimmed = true;
                        } else {
                            const cid = n.cluster_id;
                            kmsIslandDimmed =
                                cid === null ||
                                cid === undefined ||
                                !focusedIslandClusterIds.has(cid);
                        }
                    } else {
                        kmsIslandDimmed = !islandFocusMemberSet.has(n.id);
                    }
                }
                return { ...n, kmsIslandDimmed };
            }),
            links: layoutLinks3d,
        }),
        [layoutNodes3d, layoutLinks3d, islandFocusMemberSet, focusedIslandClusterIds]
    );

    const neighbors = useMemo(() => {
        if (!hoverNode) return new Set<string>();
        const res = new Set<string>();
        forceGraphData.links.forEach(l => {
            const s = typeof l.source === "string" ? l.source : (l.source as any).id;
            const t = typeof l.target === "string" ? l.target : (l.target as any).id;
            if (s === hoverNode) res.add(t);
            if (t === hoverNode) res.add(s);
        });
        return res;
    }, [hoverNode, forceGraphData.links]);

    const frameFocusedIsland = useCallback(() => {
        if (focusedIslandIdx === null) return;
        const comp = islandComponents[focusedIslandIdx];
        if (!comp?.length) return;
        const ids = new Set(comp);
        const labelIds = focusedIslandClusterIds;
        const islandNodeFilter = (n: { id?: string | number }) => {
            const id = n?.id != null ? String(n.id) : "";
            return Boolean(id && ids.has(id));
        };

        const apply = () => {
            const fg = fgRef.current as unknown as ForceGraphMethods & {
                graphData?: () => { nodes: any[] };
            };
            if (!fg) return;

            const liveNodes =
                typeof fg.graphData === "function" ? (fg.graphData()?.nodes ?? []) : [];
            const pts: THREE.Vector3[] = [];
            for (const node of liveNodes) {
                if (
                    !Number.isFinite(node.x) ||
                    !Number.isFinite(node.y) ||
                    !Number.isFinite(node.z)
                ) {
                    continue;
                }
                if (node.isClusterLabel) {
                    if (
                        labelIds &&
                        labelIds.size > 0 &&
                        node.cluster_id !== null &&
                        node.cluster_id !== undefined &&
                        labelIds.has(node.cluster_id)
                    ) {
                        pts.push(new THREE.Vector3(node.x, node.y, node.z));
                    }
                    continue;
                }
                if (ids.has(String(node.id))) {
                    pts.push(new THREE.Vector3(node.x, node.y, node.z));
                }
            }

            const minExtent = 40;
            let cx: number;
            let cy: number;
            let cz: number;
            let maxDim: number;

            if (pts.length > 0) {
                const box = new THREE.Box3().setFromPoints(pts);
                box.expandByScalar(32);
                const center = box.getCenter(new THREE.Vector3());
                const size = box.getSize(new THREE.Vector3());
                cx = center.x;
                cy = center.y;
                cz = center.z;
                maxDim = Math.max(size.x, size.y, size.z, minExtent);
            } else {
                const bbox = fg.getGraphBbox(islandNodeFilter);
                if (!bbox) {
                    fg.zoomToFit(600, 55, islandNodeFilter);
                    return;
                }
                cx = (bbox.x[0] + bbox.x[1]) / 2;
                cy = (bbox.y[0] + bbox.y[1]) / 2;
                cz = (bbox.z[0] + bbox.z[1]) / 2;
                const sx = bbox.x[1] - bbox.x[0];
                const sy = bbox.y[1] - bbox.y[0];
                const sz = bbox.z[1] - bbox.z[0];
                maxDim = Math.max(sx, sy, sz, minExtent);
            }

            const lookAt = { x: cx, y: cy, z: cz };
            const cam = fg.camera() as THREE.PerspectiveCamera;
            const vFovRad = (cam.fov * Math.PI) / 180;
            const fitDist = (maxDim / 2) / Math.tan(vFovRad / 2);
            const distance = Math.max(fitDist * 1.3, 90);
            const lookV = new THREE.Vector3(cx, cy, cz);
            let dir = cam.position.clone().sub(lookV);
            if (dir.lengthSq() < 1e-8) {
                dir.set(0.35, 0.5, 1);
            }
            dir.normalize();
            const newPos = lookV.clone().add(dir.multiplyScalar(distance));
            fg.cameraPosition({ x: newPos.x, y: newPos.y, z: newPos.z }, lookAt, 600);
        };

        requestAnimationFrame(() => {
            requestAnimationFrame(apply);
        });
    }, [focusedIslandIdx, islandComponents, focusedIslandClusterIds]);

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

    useEffect(() => {
        if (isPagedView || !hoverNode || hoverNode.startsWith("cluster-label-")) {
            setRpcPreview(null);
            if (previewTimerRef.current) clearTimeout(previewTimerRef.current);
            return;
        }
        if (previewTimerRef.current) clearTimeout(previewTimerRef.current);
        previewTimerRef.current = setTimeout(async () => {
            try {
                const p = await getTaurpc().kms_get_note_graph_preview(hoverNode, 320);
                setRpcPreview(p);
            } catch {
                setRpcPreview(null);
            }
        }, 220);
        return () => {
            if (previewTimerRef.current) clearTimeout(previewTimerRef.current);
        };
    }, [hoverNode, isPagedView]);

    // Apply Semantic Z-Forces and Centroid Logic (mutates same node refs as ForceGraph graphData)
    useEffect(() => {
        if (!fgRef.current || !recoloredData) return;

        fgRef.current.d3Force("z", (alpha: number) => {
            recoloredData.nodes.forEach((node: GraphNode & { vz?: number; z?: number }) => {
                if (node.cluster_id !== undefined && node.cluster_id !== null) {
                    const targetZ = (node.cluster_id - 5) * 150;
                    node.vz = (node.vz || 0) + (targetZ - (node.z || 0)) * 0.1 * alpha;
                }
            });
        });

        fgRef.current.d3Force("link")?.distance(120);
        fgRef.current.d3Force("charge")?.strength(-150);

        const interval = setInterval(() => {
            if (!fgRef.current || !recoloredData) return;

            const groups = new Map<number, { x: number; y: number; z: number; count: number }>();
            recoloredData.nodes.forEach((n: GraphNode & { x?: number; y?: number; z?: number }) => {
                if (!n.isClusterLabel && n.cluster_id !== null && n.cluster_id !== undefined) {
                    const current = groups.get(n.cluster_id) || { x: 0, y: 0, z: 0, count: 0 };
                    groups.set(n.cluster_id, {
                        x: current.x + (n.x || 0),
                        y: current.y + (n.y || 0),
                        z: current.z + (n.z || 0),
                        count: current.count + 1,
                    });
                }
            });

            recoloredData.nodes.forEach((n: GraphNode & { x?: number; y?: number; z?: number }) => {
                if (
                    n.isClusterLabel &&
                    n.cluster_id !== undefined &&
                    n.cluster_id !== null
                ) {
                    const cid = n.cluster_id;
                    const center = groups.get(cid);
                    if (center && center.count > 0) {
                        n.x = center.x / center.count;
                        n.y = center.y / center.count + 92;
                        n.z = center.z / center.count;
                    }
                }
            });
        }, 100);

        return () => clearInterval(interval);
    }, [recoloredData]);

    useEffect(() => {
        if (isVisible && fgRef.current) {
            setTimeout(() => {
                fgRef.current?.refresh();
            }, 50);
        }
    }, [isVisible]);

    useEffect(() => {
        if (fgRef.current && resetKey !== undefined) {
            fgRef.current.zoomToFit(800);
        }
    }, [resetKey]);

    useEffect(() => {
        if (!graphNavigateRequest || !isVisible) return;
        if (graphNavigateRequest.token === lastGraphNavTokenRef.current) return;
        if (loading || !data) return;

        const path = graphNavigateRequest.path;
        lastGraphNavTokenRef.current = graphNavigateRequest.token;

        const exists = data.nodes.some(n => !n.isClusterLabel && n.id === path);
        if (!exists) {
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

        const idx = componentIndexContaining(islandComponents, path);
        if (idx >= 0) setFocusedIslandIdx(idx);

        requestAnimationFrame(() => {
            requestAnimationFrame(() => {
                const fg = fgRef.current;
                if (!fg) return;
                const filt = (n: { id?: string | number }) => String(n.id ?? "") === path;
                fg.zoomToFit(700, 100, filt);
            });
        });
    }, [graphNavigateRequest, isVisible, loading, data, islandComponents, toast]);

    useEffect(() => {
        fgRef.current?.refresh();
    }, [pathResult]);

    useEffect(() => {
        fgRef.current?.refresh();
    }, [colorMode, showWikiEdges, showAiBeams, showSemanticKnnEdges, showSemanticEdges, forceGraphData]);

    useEffect(() => {
        fgRef.current?.refresh();
    }, [focusedIslandIdx, islandIsolateHide]);

    const nodeThreeObject = useCallback((node: any) => {
        if (node.isClusterLabel) {
            const dim = Boolean(node.kmsIslandDimmed);
            const sprite = new SpriteText(node.name || node.title || "Topic");
            sprite.color = dim ? "rgba(224, 242, 254, 0.14)" : "rgba(224, 242, 254, 1)";
            sprite.textHeight = 20;
            sprite.fontWeight = "900";
            sprite.backgroundColor = dim ? "rgba(2, 12, 28, 0.38)" : "rgba(2, 12, 28, 0.82)";
            sprite.padding = 10;
            sprite.borderRadius = 12;
            sprite.borderWidth = 2;
            sprite.borderColor = dim
                ? "rgba(56, 189, 248, 0.12)"
                : "rgba(56, 189, 248, 0.55)";
            applyKmsGraphSpriteTextResolution(sprite, {
                maxDprScale: graphVisualPrefs.spriteLabelMaxDprScale,
                minResScale: graphVisualPrefs.spriteLabelMinResScale,
            });
            return sprite;
        }

        const group = new THREE.Group();
        const isHovered = node.id === hoverNode;
        const isNeighbor = neighbors.has(node.id);
        const isDimmed = hoverNode && !isHovered && !isNeighbor;
        const isActive = node.id === activeNotePath;
        const isOnPath = pathNodeSet.size > 0 && pathNodeSet.has(node.id);

        const pr =
            typeof node.link_centrality === "number" ? node.link_centrality : 0;
        const linkCount = node.num_links > 0 ? node.num_links : 1;
        const scaleFactor = Math.min(
            2.85,
            0.75 + 1.35 * pr + 0.4 * Math.log10(linkCount + 1)
        );
        const baseRadius = scaleFactor * 5;

        const pathGreen = "#10b981";
        const shape = kmsNode3DShape(node.node_type);
        let geometry: THREE.BufferGeometry;
        switch (shape) {
            case "cone":
                geometry = new THREE.ConeGeometry(baseRadius * 0.95, baseRadius * 2.2, 12);
                break;
            case "box":
                geometry = new THREE.BoxGeometry(
                    baseRadius * 1.45,
                    baseRadius * 1.45,
                    baseRadius * 1.45
                );
                break;
            case "octahedron":
                geometry = new THREE.OctahedronGeometry(baseRadius * 1.12);
                break;
            default:
                geometry = new THREE.SphereGeometry(baseRadius);
        }
        const islandMul = node.kmsIslandDimmed ? 0.11 : 1;
        const haloColor = new THREE.Color(isOnPath ? pathGreen : node.color);
        const haloMat = new THREE.MeshBasicMaterial({
            color: haloColor,
            transparent: true,
            opacity: (isDimmed ? 0.05 : 0.16) * islandMul,
            depthWrite: false,
            blending: THREE.AdditiveBlending,
        });
        const halo = new THREE.Mesh(geometry.clone(), haloMat);
        if (shape === "cone") {
            halo.rotation.x = Math.PI;
        }
        halo.scale.multiplyScalar(1.48);
        group.add(halo);

        const material = new THREE.MeshStandardMaterial({
            color: isOnPath ? pathGreen : node.color,
            transparent: true,
            opacity: (isDimmed ? 0.2 : 0.9) * islandMul,
            emissive: isOnPath ? pathGreen : node.color,
            emissiveIntensity:
                (isHovered
                    ? 5.0
                    : isOnPath
                      ? 3.2
                      : isNeighbor
                        ? 2.5
                        : isDimmed
                          ? 0.1
                          : 0.95) * (node.kmsIslandDimmed ? 0.15 : 1),
            roughness: 0.1,
            metalness: 0.9
        });
        const mesh = new THREE.Mesh(geometry, material);
        if (shape === "cone") {
            mesh.rotation.x = Math.PI;
        }
        group.add(mesh);

        if (node.pulseHighlight && !isOnPath && !node.isClusterLabel) {
            const ringGeom = new THREE.SphereGeometry(baseRadius * 2.15);
            const ringMat = new THREE.MeshBasicMaterial({
                color: "#2dd4bf",
                transparent: true,
                opacity: 0.12,
                wireframe: true,
            });
            const pulseRing = new THREE.Mesh(ringGeom, ringMat);
            pulseRing.userData = {
                kmsPulseRing: true,
                phase: Math.random() * Math.PI * 2,
            };
            group.add(pulseRing);
        }

        const sprite = new SpriteText(node.title);
        sprite.color =
            isDimmed || node.kmsIslandDimmed
                ? "rgba(255,255,255,0.12)"
                : "white";
        sprite.textHeight = (isHovered || isNeighbor) ? 7 : 5;
        sprite.fontWeight = (isHovered || isNeighbor) ? "900" : "bold";
        sprite.padding = 2;
        sprite.backgroundColor = isDimmed ? "transparent" : "rgba(0,0,0,0.3)";
        sprite.borderRadius = 6;
        sprite.position.y = baseRadius + 10;
        applyKmsGraphSpriteTextResolution(sprite, {
            maxDprScale: graphVisualPrefs.spriteLabelMaxDprScale,
            minResScale: graphVisualPrefs.spriteLabelMinResScale,
        });
        group.add(sprite);

        if (isActive || isHovered || isOnPath) {
            // Pulse/Glow Ring
            const ringGeom = new THREE.SphereGeometry(baseRadius * 2.5);
            const ringColor = isOnPath ? pathGreen : node.color;
            const ringMat = new THREE.MeshBasicMaterial({
                color: ringColor,
                transparent: true,
                opacity: isHovered ? 0.15 : isOnPath ? 0.12 : 0.05,
                wireframe: true
            });
            const ring = new THREE.Mesh(ringGeom, ringMat);
            group.add(ring);

            // Add a point light to the focused node
            const light = new THREE.PointLight(
                ringColor,
                isHovered ? 3 : isOnPath ? 2.5 : 2,
                baseRadius * 10
            );
            group.add(light);
        }

        return group;
    }, [
        activeNotePath,
        graphVisualPrefs.spriteLabelMaxDprScale,
        graphVisualPrefs.spriteLabelMinResScale,
        hoverNode,
        neighbors,
        pathNodeSet,
        timeFilter,
        timeRange,
    ]);

    if (!paging || loading) {
        return (
            <div className="flex-1 flex flex-col items-center justify-center bg-[#050505]">
                <Loader2 className="h-8 w-8 animate-spin text-dc-accent mb-4" />
                <span className="text-sm font-medium text-white/50">Initializing 3D Space...</span>
            </div>
        );
    }

    if (error) {
        return (
            <div className="flex-1 flex flex-col items-center justify-center bg-[#050505]">
                <div className="bg-red-500/10 p-6 rounded-2xl border border-red-500/20 text-center">
                    <p className="text-sm text-red-500 font-medium mb-4">{error}</p>
                    <Button variant="secondary" size="sm" onClick={fetchData} className="gap-2">
                        <RefreshCw size={14} /> Retry
                    </Button>
                </div>
            </div>
        );
    }

    const pagedEmpty =
        isPagedView && data && data.nodes.length === 0 && pagination;

    return (
        <div className="flex-1 relative overflow-hidden bg-[#020308]">
            {pagedEmpty && paging?.kind === "paged" && (
                <div className="absolute inset-0 z-40 flex items-center justify-center bg-black/80 backdrop-blur-sm pointer-events-auto px-4">
                    <div className="max-w-md rounded-2xl border border-white/20 bg-black/90 p-6 text-center shadow-xl">
                        <p className="text-sm font-semibold text-white mb-2">No notes on this page</p>
                        <p className="text-xs text-white/60 mb-4">
                            The offset may be past the end of the vault, or there are no indexed notes matching the
                            current graph build. Try the first page or load the full graph.
                        </p>
                        <div className="flex flex-wrap justify-center gap-2">
                            <Button
                                type="button"
                                variant="secondary"
                                size="sm"
                                className="bg-white/10 border-white/20"
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

            {isPagedView && pagination && (
                <div className="absolute top-16 left-1/2 -translate-x-1/2 z-30 flex flex-wrap items-center gap-2 rounded-xl border border-white/20 bg-black/80 backdrop-blur-md px-3 py-2 pointer-events-auto shadow-lg max-w-[95vw]">
                    <span
                        className="text-[10px] text-white/60 font-mono cursor-help"
                        title={PATH_SORT_HELP}
                    >
                        Page:{" "}
                        {pagination.returned_nodes === 0
                            ? "0"
                            : `${pagination.offset + 1}-${pagination.offset + pagination.returned_nodes}`}{" "}
                        of {pagination.total_nodes} (path-sorted)
                    </span>
                    {paging?.kind === "paged" && (
                        <label className="flex items-center gap-1 text-[10px] text-white/60">
                            <span className="sr-only">Page size</span>
                            <select
                                className="rounded border border-white/20 bg-black/50 px-1.5 py-0.5 text-[10px] font-mono text-white"
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
                        className="h-7 text-[10px] bg-white/10 border-white/20"
                        disabled={pagination.offset === 0}
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
                        className="h-7 text-[10px] bg-white/10 border-white/20"
                        disabled={!pagination.has_more}
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
                        className="h-7 text-[10px]"
                        onClick={() => {
                            writeGraphSession("full", 0, DEFAULT_PAGE_LIMIT);
                            setPaging({ kind: "full" });
                        }}
                    >
                        Full graph
                    </Button>
                </div>
            )}

            {!isPagedView && indexedNoteCount > 0 && (
                <div className="absolute top-16 right-6 z-20">
                    <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        className="h-7 text-[10px] text-white/50"
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
                <div className="absolute bottom-48 right-6 z-20 max-w-xs rounded-xl border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-[10px] text-amber-100 pointer-events-none">
                    Pathfinding and RPC hover preview are disabled in paged view. Switch to Full graph for those features.
                </div>
            )}

            {parsedGraphWarnings.length > 0 && (
                <div className="absolute top-4 right-4 z-20 max-w-lg pointer-events-auto">
                    <div className="rounded-xl border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-xs text-amber-100 shadow-lg backdrop-blur-md">
                        <p className="font-semibold text-amber-200 mb-1">Graph notices</p>
                        <ul className="list-disc pl-4 space-y-1 text-amber-100/90">
                            {parsedGraphWarnings.map((w, i) => (
                                <li key={`${w.code ?? "raw"}-${i}`}>
                                    {w.message}
                                    {w.code && <span className="ml-1 align-middle text-[10px] text-amber-200/70">[{w.code}]</span>}
                                </li>
                            ))}
                        </ul>
                        {isPagedView && (
                            <p className="mt-2 border-t border-amber-500/25 pt-2 text-[10px] leading-snug text-amber-200/80">
                                Paged view: clustering and beams apply only to this page; not comparable across pages. Cap/skip messages come from Knowledge Graph settings (global and per-vault overrides).
                            </p>
                        )}
                    </div>
                </div>
            )}
            <KmsGraphConstellationBackdrop
                hex={{
                    cellRadius: graphVisualPrefs.hexCellRadius,
                    layerOpacity: graphVisualPrefs.hexLayerOpacity,
                    strokeWidth: graphVisualPrefs.hexStrokeWidth,
                    strokeOpacity: graphVisualPrefs.hexStrokeOpacity,
                }}
            />
            <div className="absolute inset-0 z-[1] min-h-0">
            <ForceGraph3D
                ref={fgRef}
                graphData={forceGraphData}
                nodeLabel="title"
                nodeThreeObject={nodeThreeObject}
                nodeThreeObjectExtend={false}
                linkColor={(link: any) => {
                    if (pathEdgeSet.size > 0 && linkOnPathSet(link, pathEdgeSet)) {
                        return "rgba(16, 185, 129, 0.95)";
                    }
                    const isRelated = hoverNode && (
                        (typeof link.source === 'string' ? link.source === hoverNode : (link.source as any).id === hoverNode) ||
                        (typeof link.target === 'string' ? link.target === hoverNode : (link.target as any).id === hoverNode)
                    );
                    if (hoverNode && !isRelated) return "rgba(255,255,255,0.02)";
                    if (link.isAiBeam) return "rgba(168, 85, 247, 0.8)"; // Purple/Violet
                    if (link.isSemantic) return "rgba(34, 211, 238, 0.35)";
                    if (link.isSemanticKnn) return "rgba(251, 191, 36, 0.78)";
                    return "rgba(125, 211, 252, 0.72)";
                }}
                linkWidth={(link: any) => {
                    if (pathEdgeSet.size > 0 && linkOnPathSet(link, pathEdgeSet)) {
                        return 6;
                    }
                    const isRelated = hoverNode && (
                        (typeof link.source === 'string' ? link.source === hoverNode : (link.source as any).id === hoverNode) ||
                        (typeof link.target === 'string' ? link.target === hoverNode : (link.target as any).id === hoverNode)
                    );
                    if (link.isAiBeam) return isRelated ? 8 : 4;
                    return isRelated ? 4 : link.isSemantic ? 1 : link.isSemanticKnn ? 1.6 : 2;
                }}
                linkOpacity={
                    ((link: any) => {
                        if (pathEdgeSet.size > 0 && linkOnPathSet(link, pathEdgeSet)) {
                            return 0.95;
                        }
                        const sid =
                            typeof link.source === "string" ? link.source : link.source.id;
                        const tid =
                            typeof link.target === "string" ? link.target : link.target.id;
                        const sidStr = String(sid);
                        const tidStr = String(tid);
                        const ns = forceGraphData.nodes.find(n => String(n.id) === sidStr);
                        const nt = forceGraphData.nodes.find(n => String(n.id) === tidStr);
                        const dim = Boolean(ns?.kmsIslandDimmed || nt?.kmsIslandDimmed);
                        const isRelated =
                            hoverNode && (sid === hoverNode || tid === hoverNode);
                        if (hoverNode && !isRelated) {
                            return dim ? 0.02 : 0.04;
                        }
                        const base = link.isAiBeam
                            ? 0.72
                            : link.isSemantic
                              ? 0.22
                              : link.isSemanticKnn
                                ? 0.48
                                : 0.55;
                        return dim ? base * 0.09 : base;
                    }) as unknown as number
                }
                onEngineTick={() => {
                    const t = performance.now() / 1000;
                    fgRef.current?.scene().traverse((obj: THREE.Object3D) => {
                        const mesh = obj as THREE.Mesh & {
                            userData: { kmsPulseRing?: boolean; phase?: number };
                        };
                        if (mesh.userData?.kmsPulseRing && mesh.material) {
                            const m = mesh.material as THREE.MeshBasicMaterial;
                            const ph = mesh.userData.phase ?? 0;
                            m.opacity = 0.08 + 0.12 * Math.sin(t * 2.4 + ph);
                        }
                    });
                }}
                linkDirectionalParticles={(link: any) =>
                    link.isAiBeam ? 6 : link.isSemantic ? 0 : link.isSemanticKnn ? 1 : 2
                }
                linkDirectionalParticleSpeed={(link: any) => link.isAiBeam ? 0.02 : 0.005}
                linkDirectionalParticleWidth={(link: any) => link.isAiBeam ? 4 : 2}
                onNodeClick={(node: any) => {
                    if (fgRef.current) {
                        // Cinematic Fly-Through
                        const distance = 40;
                        const distRatio = 1 + distance / Math.hypot(node.x, node.y, node.z);
                        fgRef.current.cameraPosition(
                            { x: node.x * distRatio, y: node.y * distRatio, z: node.z * distRatio },
                            node,
                            1200
                        );

                        // Delay navigation to allow for the fly-through effect
                        setTimeout(() => {
                            onSelectNote(node.id);
                        }, 1000);
                    } else {
                        onSelectNote(node.id);
                    }
                }}
                onNodeHover={(node: any) => setHoverNode(node ? node.id : null)}
                onLinkHover={(link: any) => setHoverLink(link)}
                backgroundColor="rgba(0,0,0,0)"
                showNavInfo={false}
                controlType="orbit"
            />
            </div>

            <div
                className={cn(
                    "absolute left-0 top-0 z-30 flex h-full max-h-full items-stretch pl-6 pt-6 pb-36 transition-transform duration-300 ease-out pointer-events-none will-change-transform",
                    graphPanelsCollapsed ? "-translate-x-[calc(100%-2.75rem)]" : "translate-x-0"
                )}
                style={{ width: "min(100vw - 1.5rem, 332px)" }}
            >
                <div className="pointer-events-auto flex min-h-0 w-[min(100vw-3rem,280px)] max-w-[min(100vw-3rem,280px)] shrink-0 flex-col gap-3">
                    <div className="min-h-0 flex max-h-[calc(100vh-10rem)] flex-col overflow-y-auto rounded-2xl border border-white/10 bg-black/45 p-4 shadow-[0_12px_48px_rgba(0,0,0,0.45)] backdrop-blur-xl ring-1 ring-white/5 select-none">
                <div className="flex items-center gap-3 mb-3">
                    <div className="p-1.5 rounded-lg bg-dc-accent/20 border border-dc-accent/30">
                        <Box size={14} className="text-dc-accent" />
                    </div>
                    <div>
                        <h2 className="text-[10px] font-bold uppercase tracking-[0.2em] text-white/80">Hyper-Graph v3.0</h2>
                        <p className="text-[8px] text-white/40 italic">Semantic Spatial Mode</p>
                    </div>
                </div>
                <div className="space-y-2 mb-3">
                    <div className="flex items-center justify-between">
                        <span className="text-[9px] text-white/40 uppercase tracking-wider">Visible</span>
                        <span className="text-[9px] font-mono text-dc-accent">
                            {forceGraphData.nodes.length}
                            {data && data.nodes.length !== forceGraphData.nodes.length
                                ? ` / ${data.nodes.length}`
                                : ""}
                        </span>
                    </div>
                    {lastBuildMs != null && lastBuildMs > 0 && (
                        <div
                            className="flex items-center justify-between"
                            title="Server-side graph build time for the last fetch"
                        >
                            <span className="text-[9px] text-white/40 uppercase tracking-wider">Build</span>
                            <span className="text-[9px] font-mono text-white/70">{lastBuildMs} ms</span>
                        </div>
                    )}
                    {vaultDiag != null && (
                        <p
                            className="text-[8px] text-white/45 leading-snug"
                            title="Vault-wide counts from kms_get_diagnostics"
                        >
                            Vault: {vaultDiag.note_count} notes, {vaultDiag.vector_count} vectors
                            {vaultDiag.error_log_count > 0 ? `, ${vaultDiag.error_log_count} log alerts` : ""}
                        </p>
                    )}
                </div>

                <div className="mb-3 space-y-1">
                    <span className="text-[9px] uppercase tracking-wider text-white/40">Node colors</span>
                    <div className="flex rounded-lg border border-white/10 p-0.5 bg-black/40">
                        <button
                            type="button"
                            className={`flex-1 rounded-md px-2 py-1 text-[10px] font-medium ${
                                colorMode === "type" ? "bg-dc-accent text-white" : "text-white/50 hover:text-white/80"
                            }`}
                            onClick={() => setColorModePersist("type")}
                        >
                            By type
                        </button>
                        <button
                            type="button"
                            className={`flex-1 rounded-md px-2 py-1 text-[10px] font-medium ${
                                colorMode === "folder" ? "bg-dc-accent text-white" : "text-white/50 hover:text-white/80"
                            }`}
                            onClick={() => setColorModePersist("folder")}
                        >
                            By folder
                        </button>
                    </div>
                </div>

                <div className="mb-3 space-y-1">
                    <span className="text-[9px] uppercase tracking-wider text-white/40">Search / filter</span>
                    <input
                        type="search"
                        value={legendFilterText}
                        onChange={e => {
                            const v = e.target.value;
                            setLegendFilterText(v);
                            writeLegendFilterQuery(v);
                        }}
                        placeholder="Title, path, folder..."
                        className="w-full rounded-md border border-white/10 bg-black/50 px-2 py-1.5 text-[10px] text-white placeholder:text-white/35"
                        aria-label="Filter graph nodes"
                    />
                    {(indexedNotes?.length ?? 0) > 0 ? (
                        <input
                            type="search"
                            value={legendTagFilterText}
                            onChange={e => setLegendTagFilterText(e.target.value)}
                            placeholder="Tags (comma or space)..."
                            className="mt-1.5 w-full rounded-md border border-white/10 bg-black/50 px-2 py-1.5 text-[10px] text-white placeholder:text-white/35"
                            aria-label="Filter graph nodes by indexed YAML tags"
                            title="Uses tags from the vault note index (frontmatter). Any token can match any tag."
                        />
                    ) : null}
                </div>

                <div className="mb-3 space-y-2">
                    <span className="text-[9px] uppercase tracking-wider text-white/40">Recent edits</span>
                    <label className="flex items-center gap-2 cursor-pointer">
                        <input
                            type="checkbox"
                            className="rounded border-white/20"
                            checked={pulseEnabled}
                            onChange={e => {
                                const v = e.target.checked;
                                setPulseEnabled(v);
                                writePulseEnabled(v);
                            }}
                        />
                        <span className="text-[8px] text-white/60">Pulse newest in view</span>
                    </label>
                    {pulseEnabled && (
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
                    )}
                </div>

                <p className="text-[8px] text-white/35 mb-2 leading-snug">
                    3D shapes: note sphere, skill cone, image octahedron, asset cube
                </p>

                <label className="flex items-center gap-2 mb-2 cursor-pointer">
                    <input
                        type="checkbox"
                        className="rounded border-white/20"
                        checked={legendShowTypes}
                        onChange={e => {
                            const v = e.target.checked;
                            setLegendShowTypes(v);
                            writeLegendPanelTypes(v);
                        }}
                    />
                    <span className="text-[9px] text-white/50">Show type legend</span>
                </label>
                {legendShowTypes && colorMode === "type" && (
                    <div className="flex flex-col gap-1.5 mb-3 pl-0.5 max-h-36 overflow-y-auto pr-1">
                        {LEGEND_TYPE_ROWS.map(row => (
                            <label
                                key={row.type}
                                className="flex items-center gap-2 cursor-pointer min-w-0"
                            >
                                <input
                                    type="checkbox"
                                    className="rounded border-white/20 shrink-0"
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
                                <span className="text-[8px] text-white/60 truncate">{row.label}</span>
                            </label>
                        ))}
                    </div>
                )}

                <label className="flex items-center gap-2 mb-2 cursor-pointer">
                    <input
                        type="checkbox"
                        className="rounded border-white/20"
                        checked={legendShowFolders}
                        onChange={e => {
                            const v = e.target.checked;
                            setLegendShowFolders(v);
                            writeLegendPanelFolders(v);
                        }}
                    />
                    <span className="text-[9px] text-white/50">Show folder palette</span>
                </label>
                {legendShowFolders && colorMode === "folder" && folderLegendRows.length > 0 && (
                    <div className="max-h-28 overflow-y-auto flex flex-col gap-1 mb-2 pl-0.5 pr-1">
                        {folderLegendRows.map(row => (
                            <label
                                key={row.key}
                                className="flex items-center gap-2 min-w-0 cursor-pointer"
                            >
                                <input
                                    type="checkbox"
                                    className="rounded border-white/20 shrink-0"
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
                                    className="w-1.5 h-1.5 rounded-full shrink-0"
                                    style={{ backgroundColor: row.color }}
                                />
                                <span className="text-[8px] text-white/60 truncate" title={row.key}>
                                    {row.label}
                                </span>
                            </label>
                        ))}
                    </div>
                )}

                <div className="mb-3">
                    <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        className="h-7 text-[9px] px-2 text-white/60 border border-white/15 w-full"
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

                <div className="mb-3 space-y-2 rounded-2xl border border-white/10 bg-black/25 px-2.5 py-2.5 shadow-inner backdrop-blur-sm">
                    <span className="block text-[9px] font-semibold uppercase tracking-[0.14em] text-sky-200/75">
                        Semantic clusters
                    </span>
                    <p className="text-[8px] leading-snug text-white/50">
                        {islandComponents.length} connected group
                        {islandComponents.length !== 1 ? "s" : ""}
                        {islandComponents.length > 0
                            ? ` (${islandComponents.map(c => c.length).join(" / ")})`
                            : ""}
                        {legendFilterText.trim() ? " - search may span multiple groups." : ""}
                        {isPagedView ? " Page-local counts." : ""}
                    </p>
                    {islandComponents.length > 0 && (
                        <div className="flex max-h-28 flex-col gap-1.5 overflow-y-auto pr-0.5">
                            {islandComponents.map((comp, idx) => {
                                const hue =
                                    KMS_CONSTELLATION_ISLAND_COLORS[
                                        idx % KMS_CONSTELLATION_ISLAND_COLORS.length
                                    ];
                                const pct = Math.round((100 * comp.length) / maxIslandBarDenominator);
                                return (
                                    <button
                                        key={idx}
                                        type="button"
                                        className={`flex w-full min-w-0 items-center gap-2 rounded-lg border px-2 py-1.5 text-left transition-colors ${
                                            focusedIslandIdx === idx
                                                ? "border-sky-400/45 bg-sky-500/20 text-white"
                                                : "border-white/10 text-white/60 hover:bg-white/5"
                                        }`}
                                        onClick={() =>
                                            setFocusedIslandIdx(focusedIslandIdx === idx ? null : idx)
                                        }
                                    >
                                        <span
                                            className="h-2 w-2 shrink-0 rounded-full shadow-[0_0_6px_currentColor]"
                                            style={{ backgroundColor: hue, color: hue }}
                                        />
                                        <span className="min-w-0 flex-1 truncate text-[8px] font-medium">
                                            Cluster {idx + 1}
                                        </span>
                                        <span className="flex h-1.5 w-12 shrink-0 overflow-hidden rounded-full bg-white/10">
                                            <span
                                                className="h-full rounded-full"
                                                style={{
                                                    width: `${pct}%`,
                                                    backgroundColor: hue,
                                                    boxShadow: `0 0 8px ${hue}88`,
                                                }}
                                            />
                                        </span>
                                        <span className="w-6 shrink-0 text-right text-[8px] tabular-nums text-white/45">
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
                            className="rounded border-white/20"
                            checked={islandIsolateHide}
                            onChange={e => setIslandIsolateHide(e.target.checked)}
                        />
                        <span className="text-[8px] text-white/55">Hide other islands (else dim)</span>
                    </label>
                    <div className="flex flex-wrap gap-1.5">
                        <Button
                            type="button"
                            variant="secondary"
                            size="sm"
                            className="h-7 text-[8px] px-2 bg-white/10 border-white/15 text-white"
                            disabled={
                                legendFilteredNodes.filter(n => !n.isClusterLabel).length === 0
                            }
                            onClick={() => {
                                const p = activeNotePath ?? hoverNode ?? null;
                                if (!p || p.startsWith("cluster-label-")) return;
                                const idx = componentIndexContaining(islandComponents, p);
                                if (idx >= 0) setFocusedIslandIdx(idx);
                            }}
                        >
                            Focus island (active / hover)
                        </Button>
                        <Button
                            type="button"
                            variant="ghost"
                            size="sm"
                            className="h-7 text-[8px] px-2 text-white/65"
                            disabled={focusedIslandIdx === null || isPagedView}
                            onClick={() => frameFocusedIsland()}
                        >
                            Frame in 3D
                        </Button>
                        {focusedIslandIdx !== null && (
                            <Button
                                type="button"
                                variant="ghost"
                                size="sm"
                                className="h-7 text-[8px] px-2 text-white/55"
                                onClick={() => setFocusedIslandIdx(null)}
                            >
                                Clear
                            </Button>
                        )}
                    </div>
                </div>

                <label className="flex items-center gap-2 mb-2 cursor-pointer">
                    <input
                        type="checkbox"
                        className="rounded border-white/20"
                        checked={legendShowEdgeToggles}
                        onChange={e => {
                            const v = e.target.checked;
                            setLegendShowEdgeToggles(v);
                            writeLegendPanelEdgeToggles(v);
                        }}
                    />
                    <span className="text-[9px] text-white/50">Show edge toggles</span>
                </label>
                {legendShowEdgeToggles && (
                    <div className="flex flex-col gap-2 mb-3 pl-0.5">
                        <label className="flex items-center gap-2 cursor-pointer">
                            <input
                                type="checkbox"
                                className="rounded border-white/20"
                                checked={showWikiEdges}
                                onChange={e => {
                                    const v = e.target.checked;
                                    if (v) markIslandBridgeExpand();
                                    setShowWikiEdges(v);
                                    writeShowWikiEdges(v);
                                }}
                            />
                            <span className="text-[8px] text-white/60">Wiki links</span>
                        </label>
                        <label className="flex items-center gap-2 cursor-pointer">
                            <input
                                type="checkbox"
                                className="rounded border-white/20"
                                checked={showSemanticKnnEdges}
                                onChange={e => {
                                    const v = e.target.checked;
                                    if (v) markIslandBridgeExpand();
                                    setShowSemanticKnnEdges(v);
                                    writeShowSemanticKnnEdges(v);
                                }}
                            />
                            <span className="text-[8px] text-white/60">Semantic kNN</span>
                        </label>
                        <label className="flex items-center gap-2 cursor-pointer">
                            <input
                                type="checkbox"
                                className="rounded border-white/20"
                                checked={showSemanticEdges}
                                onChange={e => {
                                    const v = e.target.checked;
                                    if (v) markIslandBridgeExpand();
                                    setShowSemanticEdges(v);
                                    writeShowSemanticEdges(v);
                                }}
                            />
                            <span className="text-[8px] text-white/60">Cluster links</span>
                        </label>
                        <label className="flex items-center gap-2 cursor-pointer">
                            <input
                                type="checkbox"
                                className="rounded border-white/20"
                                checked={showAiBeams}
                                onChange={e => {
                                    const v = e.target.checked;
                                    if (v) markIslandBridgeExpand();
                                    setShowAiBeams(v);
                                    writeShowAiBeamEdges(v);
                                }}
                            />
                            <span className="text-[8px] text-white/60">AI beams</span>
                        </label>
                    </div>
                )}
                    </div>

            {data && !isPagedView && (
                <div className="max-w-full shrink-0 rounded-2xl border border-white/10 bg-black/70 backdrop-blur-xl shadow-2xl p-3">
                    <div className="flex items-center gap-2 mb-2 text-white">
                        <Route size={14} className="text-emerald-400 shrink-0" />
                        <span className="text-[10px] font-bold uppercase tracking-wider">Shortest path</span>
                    </div>
                    <div className="flex flex-col gap-2">
                        <select
                            value={pathFrom}
                            onChange={e => setPathFrom(e.target.value)}
                            className="text-[11px] rounded-lg border border-white/10 bg-black/50 px-2 py-1.5 text-white max-w-full"
                            aria-label="From note"
                        >
                            <option value="">From note...</option>
                            {data.nodes
                                .filter(n => !n.isClusterLabel)
                                .map(n => (
                                    <option key={n.id} value={n.id}>
                                        {n.title || n.path}
                                    </option>
                                ))}
                        </select>
                        <select
                            value={pathTo}
                            onChange={e => setPathTo(e.target.value)}
                            className="text-[11px] rounded-lg border border-white/10 bg-black/50 px-2 py-1.5 text-white max-w-full"
                            aria-label="To note"
                        >
                            <option value="">To note...</option>
                            {data.nodes
                                .filter(n => !n.isClusterLabel)
                                .map(n => (
                                    <option key={`to-${n.id}`} value={n.id}>
                                        {n.title || n.path}
                                    </option>
                                ))}
                        </select>
                        <div className="flex gap-2">
                            <Button
                                type="button"
                                variant="secondary"
                                size="sm"
                                className="flex-1 text-[11px] h-8 bg-white/10 border-white/10 text-white hover:bg-white/15"
                                onClick={findPath}
                            >
                                Find path
                            </Button>
                            <Button
                                type="button"
                                variant="ghost"
                                size="sm"
                                className="text-[11px] h-8 px-2 text-white/80"
                                onClick={clearPath}
                            >
                                Clear
                            </Button>
                        </div>
                        {pathError && (
                            <p className="text-[10px] text-amber-400/90 leading-snug">{pathError}</p>
                        )}
                        {pathResult?.found && pathResult.node_paths.length > 0 && (
                            <p className="text-[10px] text-white/50 leading-snug">
                                {pathResult.node_paths.length} hops
                                {pathResult.node_paths.length <= 8
                                    ? `: ${pathResult.node_paths
                                          .map(
                                              p =>
                                                  data.nodes.find(n => n.id === p)?.title ?? ""
                                          )
                                          .filter(Boolean)
                                          .join(" -> ")}`
                                    : ""}
                            </p>
                        )}
                    </div>
                </div>
            )}
                </div>
                <button
                    type="button"
                    className="pointer-events-auto ml-1 flex h-28 w-9 shrink-0 flex-col items-center justify-center gap-1 self-center rounded-r-xl border border-white/15 bg-black/55 text-white/70 shadow-[0_8px_32px_rgba(0,0,0,0.4)] backdrop-blur-md transition-colors hover:border-dc-accent/40 hover:text-dc-accent"
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

            {hoverNode && !hoverNode.startsWith('cluster-label-') && (
                <div
                    className={cn(
                        "absolute top-6 z-[45] max-h-[min(70vh,420px)] max-w-[min(100vw-18rem,380px)] overflow-hidden rounded-2xl border border-white/15 bg-gradient-to-br from-black/55 via-black/40 to-black/25 backdrop-blur-2xl shadow-[0_12px_48px_rgba(0,0,0,0.45)] ring-1 ring-dc-accent/20 p-4 pointer-events-none select-none animate-in fade-in slide-in-from-left-4 duration-300",
                        graphPanelsCollapsed ? "left-4 sm:left-6" : "left-[280px]"
                    )}
                >
                    <div className="absolute inset-0 rounded-2xl bg-[radial-gradient(ellipse_at_top_right,rgba(14,165,233,0.14),transparent_55%)] pointer-events-none" />
                    <div className="relative flex flex-col gap-2">
                        <div className="flex items-center gap-2">
                            <div className="w-2 h-2 rounded-full bg-dc-accent animate-pulse shadow-[0_0_10px_rgba(14,165,233,0.6)]" />
                            <span className="text-[10px] font-bold uppercase tracking-widest text-dc-accent/95">
                                Knowledge Hologram
                            </span>
                        </div>
                        <h3 className="text-sm font-bold text-white mb-0.5 tracking-tight">
                            {rpcPreview?.title ||
                                forceGraphData.nodes.find(n => n.id === hoverNode)?.title ||
                                data?.nodes.find(n => n.id === hoverNode)?.title}
                        </h3>
                        <p className="text-[11px] text-white/70 leading-relaxed line-clamp-6 whitespace-pre-wrap border-l-2 border-dc-accent/35 pl-3">
                            {rpcPreview?.excerpt ||
                                vizData.nodes.find(n => n.id === hoverNode)?.preview ||
                                data?.nodes.find(n => n.id === hoverNode)?.preview ||
                                "Loading preview or no excerpt available."}
                        </p>
                        {(() => {
                            const lm =
                                rpcPreview?.last_modified ||
                                forceGraphData.nodes.find(n => n.id === hoverNode)?.last_modified ||
                                data?.nodes.find(n => n.id === hoverNode)?.last_modified;
                            if (!lm) return null;
                            return (
                                <p className="text-[9px] text-white/45 font-mono">
                                    {new Date(lm).toLocaleString()}
                                </p>
                            );
                        })()}
                    </div>
                </div>
            )}
            {hoverLink && hoverLink.isAiBeam && (
                <div
                    className={cn(
                        "absolute top-24 z-[45] max-h-[min(50vh,320px)] max-w-[min(100vw-18rem,380px)] overflow-hidden rounded-2xl border border-purple-400/25 bg-gradient-to-br from-purple-950/50 via-black/45 to-black/25 backdrop-blur-2xl shadow-[0_12px_48px_rgba(88,28,135,0.35)] ring-1 ring-purple-400/25 p-4 pointer-events-none select-none animate-in fade-in zoom-in duration-300",
                        graphPanelsCollapsed ? "left-4 sm:left-6" : "left-[280px]"
                    )}
                >
                    <div className="absolute inset-0 rounded-2xl bg-[radial-gradient(ellipse_at_top_right,rgba(168,85,247,0.12),transparent_55%)] pointer-events-none" />
                    <div className="relative flex flex-col gap-2">
                        <div className="flex items-center gap-2">
                            <Activity size={12} className="text-purple-300 animate-pulse" />
                            <span className="text-[10px] font-bold uppercase tracking-widest text-purple-300/95">
                                AI Summary Beam
                            </span>
                        </div>
                        <p className="text-[11px] text-white/85 leading-relaxed italic border-l-2 border-purple-400/40 pl-3">
                            {hoverLink.summary}
                        </p>
                    </div>
                </div>
            )}

            {/* Temporal view (vision dock) */}
            <div
                className={cn(
                    "pointer-events-none absolute bottom-6 inset-x-0 mx-auto flex flex-col items-center gap-2",
                    graphPanelsCollapsed
                        ? "w-[min(92vw,42rem)] min-w-[280px]"
                        : "w-1/2 min-w-[280px]"
                )}
            >
                <div className="pointer-events-auto flex h-12 w-full items-center gap-4 rounded-2xl border border-sky-400/25 bg-gradient-to-r from-black/60 via-black/45 to-black/30 p-3 shadow-[0_8px_40px_rgba(0,0,0,0.45)] ring-1 ring-white/10 backdrop-blur-2xl">
                    <div className="flex min-w-[108px] flex-col">
                        <span className="text-[8px] font-bold uppercase tracking-[0.2em] text-sky-300/90">
                            Temporal view
                        </span>
                        <span className="font-mono text-[10px] text-white/55">
                            {new Date(timeRange.min + (timeRange.max - timeRange.min) * (timeFilter / 100)).toLocaleDateString()}
                        </span>
                    </div>

                    <Button
                        type="button"
                        variant="secondary"
                        size="icon"
                        className="h-9 w-9 shrink-0 rounded-xl border border-white/15 bg-white/5 text-white hover:bg-white/10"
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

                    <div className="flex items-center gap-2 rounded-lg border border-white/10 bg-black/30 px-3 py-1">
                        <Activity size={12} className={timeFilter < 100 ? "animate-pulse text-sky-400" : "text-white/20"} />
                        <span className="text-[10px] font-bold text-white/60">{timeFilter}%</span>
                    </div>
                </div>
                <span className="pointer-events-none select-none text-[9px] font-bold uppercase tracking-[0.25em] text-white/25">
                    Constellation timeline
                </span>
            </div>

            <div
                className={cn(
                    "absolute bottom-6 left-6 flex items-center gap-2 p-2 rounded-xl bg-black/40 backdrop-blur-md border border-white/10 transition-opacity duration-300",
                    graphPanelsCollapsed ? "pointer-events-none opacity-0" : "opacity-100"
                )}
            >
                <div className="flex items-center gap-2 px-3 py-1.5 border-r border-white/10">
                    <Layers size={14} className="text-white/40" />
                    <span className="text-[10px] text-white/60 font-medium">Auto-Clustering</span>
                    <div className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse" />
                </div>
            </div>

            <div className="absolute top-6 right-6 flex flex-col gap-2">
                <Button
                    variant="ghost"
                    size="icon"
                    className="h-10 w-10 rounded-xl bg-black/40 backdrop-blur-md border border-white/10 text-white/60 hover:text-white hover:bg-white/5"
                    onClick={copyGraphDebugInfo}
                    title="Copy graph debug info (JSON) to clipboard"
                >
                    <ClipboardList size={16} />
                </Button>
                <Button variant="ghost" size="icon" className="h-10 w-10 rounded-xl bg-black/40 backdrop-blur-md border border-white/10 text-white/60 hover:text-white hover:bg-white/5" onClick={fetchData}>
                    <RefreshCw size={16} />
                </Button>
            </div>
        </div>
    );
}
