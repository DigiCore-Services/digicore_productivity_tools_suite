import React, { useEffect, useRef, useState, useMemo, useCallback } from "react";
import ForceGraph3D, { ForceGraphMethods } from "react-force-graph-3d";
import * as THREE from "three";
import SpriteText from "three-spritetext";
import { applyKmsGraphSpriteTextResolution } from "../../lib/kmsGraphSpriteText";
import { getTaurpc } from "../../lib/taurpc";
import { KmsNodeDto, KmsGraphPathDto, KmsNoteGraphPreviewDto, KmsGraphDto } from "../../bindings";
import {
    linkKeysFromGraphLinks,
    linkOnPathSet,
    pathEdgeSetFromDto,
    pathNodeSetFromDto,
    visiblePathEdgeCount,
} from "../../lib/kmsGraphHelpers";
import { formatIpcOrRaw } from "../../lib/ipcError";
import { copyKmsGraphDebugToClipboard } from "../../lib/kmsGraphDebugClipboard";
import { kmsGraphLog } from "../../lib/kmsGraphLog";
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
    readShowSemanticKnnEdges,
    writeShowSemanticKnnEdges,
    readShowSemanticEdges,
    writeShowSemanticEdges,
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
import { Loader2, RefreshCw, Box, Move, Bug, Play, Route, Activity, ChevronLeft, ChevronRight, ClipboardList } from "lucide-react";
import { Button } from "../ui/button";
import { cn } from "../../lib/utils";
import { useToast } from "../ui/use-toast";
import {
    KmsGraphConstellationBackdrop,
    KMS_CONSTELLATION_ISLAND_COLORS,
} from "./KmsGraphConstellationBackdrop";
import { useKmsForceGraphBloom } from "../../lib/useKmsForceGraphBloom";
import { useKmsGraphVisualPrefs } from "../../lib/useKmsGraphVisualPrefs";

interface KmsLocalGraph3DProps {
    path: string;
    depth?: number;
    onSelectNote: (path: string) => void;
    /** When false, Ctrl+Shift+G does not toggle this dock (e.g. another tab is visible). */
    toolsShortcutActive?: boolean;
}

interface GraphNode extends Omit<KmsNodeDto, "id"> {
    id: string; // path used as unique graph ID
    dbId: number; // database i32 id
    name: string; // title
    val: number; // radius equivalent
    color: string;
    num_links: number;
    isClusterLabel?: boolean;
    pulseHighlight?: boolean;
}

interface GraphLink {
    source: string;
    target: string;
    isSemantic?: boolean;
    isSemanticKnn?: boolean;
}

interface DebugInfoState {
    nodeCount: number;
    edgeCount: number;
    webgl: string;
    targetPath?: string;
    error?: string;
    timestamp?: string;
}

export default function KmsLocalGraph3D({
    path,
    depth = 2,
    onSelectNote,
    toolsShortcutActive = true,
}: KmsLocalGraph3DProps) {
    const lastGraphDtoRef = useRef<KmsGraphDto | null>(null);
    const fgRef = useRef<ForceGraphMethods>();
    const [data, setData] = useState<{ nodes: GraphNode[]; links: GraphLink[] } | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [rotationEnabled, setRotationEnabled] = useState(false);
    const [lastError, setLastError] = useState<string | null>(null);
    const [showDebug, setShowDebug] = useState(false);
    const [dimensions, setDimensions] = useState({ width: 0, height: 0 });
    const [timeFilter, setTimeFilter] = useState(100);
    const [playingTimeline, setPlayingTimeline] = useState(false);
    const [pathFrom, setPathFrom] = useState("");
    const [pathTo, setPathTo] = useState("");
    const [pathResult, setPathResult] = useState<KmsGraphPathDto | null>(null);
    const [pathError, setPathError] = useState<string | null>(null);
    const [hoverNode, setHoverNode] = useState<string | null>(null);
    const [rpcPreview, setRpcPreview] = useState<KmsNoteGraphPreviewDto | null>(null);
    const previewTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

    const [colorMode, setColorMode] = useState<GraphColorMode>(() => readGraphColorMode());
    const [pulseEnabled, setPulseEnabled] = useState(() => readPulseEnabled());
    const [pulseTopPercent, setPulseTopPercent] = useState(() => readPulseTopPercent());
    const [legendFilterText, setLegendFilterText] = useState(() => readLegendFilterQuery());
    const [hiddenFolderKeys, setHiddenFolderKeys] = useState(() => readHiddenFolderKeys());
    const [hiddenNodeTypes, setHiddenNodeTypes] = useState(() => readHiddenNodeTypes());
    const [focusedIslandIdx, setFocusedIslandIdx] = useState<number | null>(null);
    const [islandIsolateHide, setIslandIsolateHide] = useState(false);
    const [graphPanelsCollapsed, setGraphPanelsCollapsed] = useState(() => readGraphPanelsCollapsed());
    const [showWikiEdges, setShowWikiEdges] = useState(() => readShowWikiEdges());
    const [showSemanticKnnEdges, setShowSemanticKnnEdges] = useState(() =>
        readShowSemanticKnnEdges()
    );
    const [showSemanticEdges, setShowSemanticEdges] = useState(() => readShowSemanticEdges());
    const [graphWarnings, setGraphWarnings] = useState<string[]>([]);
    const parsedGraphWarnings = useMemo(() => normalizeKmsGraphWarnings(graphWarnings), [graphWarnings]);

    const graphVisualPrefs = useKmsGraphVisualPrefs();
    useKmsForceGraphBloom(
        fgRef,
        Boolean(
            graphVisualPrefs.bloomEnabled &&
                data &&
                data.nodes.some(n => !n.isClusterLabel)
        ),
        {
            strength: graphVisualPrefs.bloomStrength,
            radius: graphVisualPrefs.bloomRadius,
            threshold: graphVisualPrefs.bloomThreshold,
        },
        [data?.nodes.length, path, depth]
    );

    const { toast } = useToast();

    const setColorModePersist = useCallback((m: GraphColorMode) => {
        setColorMode(m);
        writeGraphColorMode(m);
    }, []);

    const toggleGraphPanelsCollapsed = useCallback(() => {
        setGraphPanelsCollapsed(prev => {
            const next = !prev;
            writeGraphPanelsCollapsed(next);
            return next;
        });
    }, []);

    useEffect(() => {
        if (!toolsShortcutActive) return;
        const onKey = (e: KeyboardEvent) => {
            if (e.ctrlKey && e.shiftKey && (e.key === "g" || e.key === "G")) {
                e.preventDefault();
                toggleGraphPanelsCollapsed();
            }
        };
        window.addEventListener("keydown", onKey);
        return () => window.removeEventListener("keydown", onKey);
    }, [toolsShortcutActive, toggleGraphPanelsCollapsed]);

    useEffect(() => {
        const onLocalToggle = () => toggleGraphPanelsCollapsed();
        window.addEventListener("kms-local-graph-toggle-tools-dock", onLocalToggle);
        return () => window.removeEventListener("kms-local-graph-toggle-tools-dock", onLocalToggle);
    }, [toggleGraphPanelsCollapsed]);

    const typeColor = useCallback((t: string) => {
        switch (t) {
            case "skill":
                return "#f59e0b";
            case "image":
                return "#ec4899";
            case "asset":
                return "#8b5cf6";
            default:
                return "#0ea5e9";
        }
    }, []);

    const checkWebGL = () => {
        try {
            const canvas = document.createElement('canvas');
            const gl = canvas.getContext('webgl') || canvas.getContext('experimental-webgl');
            if (!gl) return "WebGL Not Supported";
            const debugInfo = (gl as WebGLRenderingContext).getExtension('WEBGL_debug_renderer_info');
            return debugInfo ? (gl as WebGLRenderingContext).getParameter(debugInfo.UNMASKED_RENDERER_WEBGL) : "Generic WebGL";
        } catch (e) {
            return "WebGL Error: " + e;
        }
    };

    const [debugInfo, setDebugInfo] = useState<DebugInfoState>({ nodeCount: 0, edgeCount: 0, webgl: checkWebGL() });
    const normalizedTarget = useMemo(() => path.replace(/\\/g, '/').toLowerCase(), [path]);


    const fetchData = useCallback(async () => {
        setLoading(true);
        setLastError(null);
        try {
            const graphData: KmsGraphDto = await getTaurpc().kms_get_local_graph(path, depth);
            lastGraphDtoRef.current = graphData;
            setGraphWarnings(graphData.warnings ?? []);
            kmsGraphLog.debug(
                `[Spatial] Received ${graphData.nodes.length} nodes and ${graphData.edges.length} edges for ${path} request_id=${graphData.request_id ?? ""}`
            );

            if (graphData.nodes.length === 0) {
                setData({
                    nodes: [{
                        id: path, dbId: -1, path: path, title: path.split(/[\\/]/).pop()?.replace('.md', '') || 'Current Note',
                        name: path.split(/[\\/]/).pop()?.replace('.md', '') || 'Current Note',
                        node_type: 'note', last_modified: new Date().toISOString(), folder_path: '',
                        cluster_id: null, link_centrality: 0, num_links: 0, val: 3, color: "#ffffff"
                    }],
                    links: []
                });
                return;
            }

            const degreeByPath = new Map<string, number>();
            graphData.edges.forEach((e: { source: string; target: string; kind?: string }) => {
                degreeByPath.set(e.source, (degreeByPath.get(e.source) ?? 0) + 1);
                degreeByPath.set(e.target, (degreeByPath.get(e.target) ?? 0) + 1);
            });
            const nodes: GraphNode[] = graphData.nodes.map((n: KmsNodeDto) => {
                const normalizedNodePath = n.path.replace(/\\/g, '/').toLowerCase();
                const isCentral = normalizedNodePath === normalizedTarget;
                const num_links = degreeByPath.get(n.path) ?? 0;
                const pr = n.link_centrality ?? 0;
                const baseVal = 0.4 + 2.2 * pr + 0.35 * Math.log10(num_links + 1);
                const val = isCentral ? Math.max(3, baseVal) : Math.min(4, Math.max(0.35, baseVal));
                return {
                    ...n, id: n.path, dbId: n.id, name: n.title, num_links,
                    val, color: isCentral ? "#ffffff" : "#0ea5e9"
                };
            });

            const structuralLinks: GraphLink[] = graphData.edges.map(e => ({
                source: e.source,
                target: e.target,
                isSemanticKnn: (e.kind ?? "wiki") === "semantic_knn",
            }));

            const clusterGroups = new Map<number, string[]>();
            nodes.forEach(n => {
                if (n.cluster_id !== null && n.cluster_id !== undefined) {
                    const list = clusterGroups.get(n.cluster_id) || [];
                    list.push(n.id);
                    clusterGroups.set(n.cluster_id, list);
                }
            });
            const semanticLinks: GraphLink[] = [];
            clusterGroups.forEach(nodeIds => {
                if (nodeIds.length > 1) {
                    const rootId = nodeIds[0];
                    for (let i = 1; i < nodeIds.length; i++) {
                        semanticLinks.push({
                            source: rootId,
                            target: nodeIds[i],
                            isSemantic: true,
                        });
                    }
                }
            });

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
                color: "rgba(0, 255, 255, 0.8)",
            }));

            setData({
                nodes: [...nodes, ...labelNodes],
                links: [...structuralLinks, ...semanticLinks],
            });
            setDebugInfo(prev => ({
                ...prev,
                nodeCount: nodes.length + labelNodes.length,
                edgeCount: structuralLinks.length + semanticLinks.length,
                targetPath: normalizedTarget,
                timestamp: new Date().toLocaleTimeString(),
            }));
            setError(null);

            // Triple-Stage Centering for maximum reliability
            const recenter = () => {
                if (fgRef.current) {
                    kmsGraphLog.debug("[Spatial] Stage 1: Standard Centering");
                    if (fgRef.current) {
                        fgRef.current.d3Force('center', (d3: any) => d3.forceCenter(0, 0, 0));
                        fgRef.current.d3Force('charge')?.strength(-600);
                        fgRef.current.zoomToFit(1000, 80);
                    }

                    // Stage 2: Pan camera RIGHT to shift nodes LEFT
                    setTimeout(() => {
                        if (fgRef.current) {
                            const controls = fgRef.current.controls() as any;
                            if (controls) {
                                controls.target.set(180, 0, 0); // Pan target to the right
                                controls.update();
                            }
                        }
                    }, 1200);

                    // Stage 3: Final fit after physics settle
                    setTimeout(() => {
                        fgRef.current?.zoomToFit(800, 120);
                    }, 1800);
                }
            };
            recenter();
        } catch (err: any) {
            kmsGraphLog.error("Failed to fetch local 3D graph:", err);
            const errMsg = err?.message || String(err);
            setLastError(errMsg);
            setError("Failed to load local resonance");
        } finally {
            setLoading(false);
        }
    }, [path, depth, normalizedTarget]);

    const copyGraphDebugInfo = useCallback(() => {
        void copyKmsGraphDebugToClipboard(
            {
                graphView: "local3d",
                localCenterPath: path,
                localDepth: depth,
                data: lastGraphDtoRef.current,
                error: error ?? lastError,
                paging: null,
                indexedNoteCount: 0,
                vaultDiag: null,
                pathFrom,
                pathTo,
                pathResult,
                pathError,
                hoverPreviewPath: rpcPreview?.path ?? null,
            },
            { toast }
        );
    }, [
        path,
        depth,
        error,
        lastError,
        pathFrom,
        pathTo,
        pathResult,
        pathError,
        rpcPreview?.path,
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

    const focusLocalNoteId = useMemo(() => {
        if (!data) return null;
        const hit = data.nodes.find(
            n =>
                !n.isClusterLabel &&
                n.id.replace(/\\/g, "/").toLowerCase() === normalizedTarget
        );
        return hit?.id ?? null;
    }, [data, normalizedTarget]);

    const recoloredData = useMemo(() => {
        if (!data) return null;
        const lmRange = graphLastModifiedRange(data.nodes.filter(n => !n.isClusterLabel));
        return {
            nodes: data.nodes.map(n => {
                if (n.isClusterLabel) return n;
                const isCentral =
                    n.id.replace(/\\/g, "/").toLowerCase() === normalizedTarget;
                const ms = lastModifiedMs(n.last_modified);
                const r01 = recency01(ms, lmRange.min, lmRange.max);
                const pulseHighlight = shouldPulseRecent(r01, pulseTopPercent, pulseEnabled);
                if (isCentral) return { ...n, color: "#ffffff", pulseHighlight: false };
                const color =
                    colorMode === "folder"
                        ? colorForFolderKey(n.folder_path ?? "", folderColorMap)
                        : typeColor(n.node_type);
                return { ...n, color, pulseHighlight };
            }),
            links: data.links,
        };
    }, [data, colorMode, folderColorMap, normalizedTarget, typeColor, pulseEnabled, pulseTopPercent]);

    const timeRange = useMemo(() => {
        if (!data) return { min: 0, max: Date.now() };
        const timestamps = data.nodes
            .filter(n => !n.isClusterLabel)
            .map(n => new Date(n.last_modified).getTime())
            .filter(t => !isNaN(t));
        if (timestamps.length === 0) return { min: 0, max: Date.now() };
        return { min: Math.min(...timestamps), max: Math.max(...timestamps) };
    }, [data]);

    const filteredData = useMemo(() => {
        if (!recoloredData) return { nodes: [] as GraphNode[], links: [] as GraphLink[] };
        if (timeFilter === 100) return recoloredData;

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
        return { nodes: filteredNodes, links: filteredLinks };
    }, [recoloredData, timeFilter, timeRange]);

    const edgeFilteredData = useMemo(() => {
        const keep = (l: GraphLink) => {
            if (l.isSemantic) return showSemanticEdges;
            if (l.isSemanticKnn) return showSemanticKnnEdges;
            return showWikiEdges;
        };
        return {
            nodes: filteredData.nodes,
            links: filteredData.links.filter(keep),
        };
    }, [filteredData, showWikiEdges, showSemanticKnnEdges, showSemanticEdges]);

    const vizData = useMemo(() => {
        const q = legendFilterText.trim();
        if (!q) return edgeFilteredData;
        const nodes = edgeFilteredData.nodes.filter(n =>
            nodeMatchesGraphFilter(q, {
                title: n.title,
                path: n.path,
                folder_path: n.folder_path,
            })
        );
        const ids = new Set(nodes.map(n => n.id));
        const links = edgeFilteredData.links.filter(l => {
            const s = typeof l.source === "string" ? l.source : (l.source as { id: string }).id;
            const t = typeof l.target === "string" ? l.target : (l.target as { id: string }).id;
            return ids.has(s) && ids.has(t);
        });
        return { nodes, links };
    }, [edgeFilteredData, legendFilterText]);

    const legendFilteredData = useMemo(
        () =>
            applyLegendVisibilityFilter({
                nodes: vizData.nodes.map(n => ({
                    id: n.id,
                    folder_path: n.folder_path ?? "",
                    node_type: n.node_type,
                    isClusterLabel: n.isClusterLabel,
                })),
                links: vizData.links,
                colorMode,
                hiddenFolderKeys,
                hiddenNodeTypes,
            }),
        [vizData, colorMode, hiddenFolderKeys, hiddenNodeTypes]
    );

    const legendVisibleIdSet = useMemo(
        () => new Set(legendFilteredData.nodes.map(n => n.id)),
        [legendFilteredData.nodes]
    );

    const legendFilteredNodes = useMemo(
        () => vizData.nodes.filter(n => legendVisibleIdSet.has(n.id)),
        [vizData.nodes, legendVisibleIdSet]
    );

    const legendFilteredLinks = legendFilteredData.links;

    const islandComponents = useMemo(() => {
        const noteNodes = legendFilteredNodes.filter(n => !n.isClusterLabel).map(n => n.id);
        const noteIdSet = new Set(noteNodes);
        const noteLinks = legendFilteredLinks.filter(l => {
            const s = typeof l.source === "string" ? l.source : (l.source as { id: string }).id;
            const t = typeof l.target === "string" ? l.target : (l.target as { id: string }).id;
            return (
                noteIdSet.has(s) &&
                noteIdSet.has(t) &&
                !l.isSemantic &&
                !l.isSemanticKnn
            );
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
        if (!hoverNode || !forceGraphData.links.length) return new Set<string>();
        const res = new Set<string>();
        forceGraphData.links.forEach(l => {
            const s = typeof l.source === "string" ? l.source : (l.source as { id: string }).id;
            const t = typeof l.target === "string" ? l.target : (l.target as { id: string }).id;
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

    const localLinkKeys = useMemo(
        () => linkKeysFromGraphLinks(data?.links ?? []),
        [data]
    );

    const pathEdgeLocalVisibility = useMemo(
        () => visiblePathEdgeCount(pathResult?.edges, localLinkKeys),
        [pathResult, localLinkKeys]
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
        if (!hoverNode) {
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
    }, [hoverNode]);

    useEffect(() => {
        fgRef.current?.refresh();
    }, [pathResult, colorMode, recoloredData, legendFilterText, forceGraphData]);

    useEffect(() => {
        fgRef.current?.refresh();
    }, [focusedIslandIdx, islandIsolateHide]);

    useEffect(() => {
        setPathResult(null);
        setPathError(null);
        setPathFrom("");
        setPathTo("");
    }, [path]);

    useEffect(() => {
        fetchData();
    }, [fetchData]);

    useEffect(() => {
        const onWikiPrReady = () => {
            void fetchData();
        };
        window.addEventListener("kms-wiki-pagerank-ready", onWikiPrReady);
        return () => window.removeEventListener("kms-wiki-pagerank-ready", onWikiPrReady);
    }, [fetchData]);

    // Visibility Awareness: Recenter when the tab or sidebar changes
    useEffect(() => {
        const container = document.getElementById('kms-spatial-container');
        if (container) {
            const rect = container.getBoundingClientRect();
            setDimensions({ width: rect.width || 400, height: rect.height || 500 });
        }

        let timeout: any;
        const observer = new ResizeObserver((entries) => {
            if (entries[0]) {
                const { width, height } = entries[0].contentRect;
                if (width > 0 && height > 0) {
                    setDimensions({ width, height });

                    if (fgRef.current) {
                        clearTimeout(timeout);
                        timeout = setTimeout(() => {
                            kmsGraphLog.debug("[Spatial] Resize detected - Fitting View");
                            fgRef.current?.zoomToFit(500, 150);
                        }, 300);
                    }
                }
            }
        });
        if (container) observer.observe(container);
        return () => observer.disconnect();
    }, []);

    // Lighting
    useEffect(() => {
        if (!fgRef.current) return;
        const scene = fgRef.current.scene();
        scene.children = scene.children.filter((c: any) => !(c instanceof THREE.AmbientLight || c instanceof THREE.DirectionalLight || c instanceof THREE.PointLight));
        scene.add(new THREE.AmbientLight(0xffffff, 1.2));
        const dirLight = new THREE.DirectionalLight(0xffffff, 2);
        dirLight.position.set(100, 100, 100);
        scene.add(dirLight);
    }, [forceGraphData]);

    useEffect(() => {
        if (!fgRef.current || !recoloredData) return;

        fgRef.current.d3Force("z", (alpha: number) => {
            recoloredData.nodes.forEach((node: GraphNode & { vz?: number; z?: number }) => {
                if (node.cluster_id !== undefined && node.cluster_id !== null && !node.isClusterLabel) {
                    const targetZ = (node.cluster_id - 5) * 150;
                    node.vz = (node.vz || 0) + (targetZ - (node.z || 0)) * 0.1 * alpha;
                }
            });
        });

        fgRef.current
            .d3Force("link")
            ?.distance((link: GraphLink) =>
                link.isSemantic ? 90 : link.isSemanticKnn ? 105 : 120
            );
        fgRef.current.d3Force("charge")?.strength(-150);

        const interval = window.setInterval(() => {
            if (!fgRef.current || !recoloredData) return;

            const groups = new Map<number, { x: number; y: number; z: number; count: number }>();
            recoloredData.nodes.forEach((n: GraphNode & { x?: number; y?: number; z?: number }) => {
                if (
                    !n.isClusterLabel &&
                    n.cluster_id !== null &&
                    n.cluster_id !== undefined
                ) {
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

        return () => window.clearInterval(interval);
    }, [recoloredData]);

    // Rotation
    useEffect(() => {
        if (!fgRef.current || !rotationEnabled) return;
        let angle = 0;
        const interval = setInterval(() => {
            if (fgRef.current) {
                const distance = 180;
                fgRef.current.cameraPosition({
                    x: distance * Math.sin(angle),
                    z: distance * Math.cos(angle)
                });
                angle += 0.002;
            }
        }, 30);
        return () => clearInterval(interval);
    }, [rotationEnabled, forceGraphData]);

    const isCentralId = useCallback(
        (id: string) => id.replace(/\\/g, "/").toLowerCase() === normalizedTarget,
        [normalizedTarget]
    );

    const nodeThreeObject = useCallback(
        (node: any) => {
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
            const isCenter = isCentralId(node.id);
            const isHovered = node.id === hoverNode;
            const isNeighbor = neighbors.has(node.id);
            const isDimmed = hoverNode && !isHovered && !isNeighbor;
            const isOnPath = pathNodeSet.size > 0 && pathNodeSet.has(node.id);
            const baseSize = node.val * 4;
            const pathGreen = "#10b981";
            const shape = kmsNode3DShape(node.node_type);
            let geometry: THREE.BufferGeometry;
            switch (shape) {
                case "cone":
                    geometry = new THREE.ConeGeometry(baseSize * 0.95, baseSize * 2.2, 12);
                    break;
                case "box":
                    geometry = new THREE.BoxGeometry(
                        baseSize * 1.45,
                        baseSize * 1.45,
                        baseSize * 1.45
                    );
                    break;
                case "octahedron":
                    geometry = new THREE.OctahedronGeometry(baseSize * 1.12);
                    break;
                default:
                    geometry = new THREE.SphereGeometry(baseSize);
            }

            const islandMul = node.kmsIslandDimmed ? 0.12 : 1;
            const baseCol = new THREE.Color(isOnPath ? pathGreen : node.color || "#0ea5e9");
            const haloMat = new THREE.MeshBasicMaterial({
                color: baseCol,
                transparent: true,
                opacity: (isDimmed ? 0.06 : 0.16) * islandMul,
                depthWrite: false,
                blending: THREE.AdditiveBlending,
            });
            const halo = new THREE.Mesh(geometry.clone(), haloMat);
            if (shape === "cone") {
                halo.rotation.x = Math.PI;
            }
            halo.scale.multiplyScalar(1.45);
            group.add(halo);

            const material = new THREE.MeshBasicMaterial({
                color: isOnPath ? pathGreen : node.color || "#0ea5e9",
                transparent: true,
                opacity: (isDimmed ? 0.25 : 1.0) * islandMul,
            });
            const body = new THREE.Mesh(geometry, material);
            if (shape === "cone") {
                body.rotation.x = Math.PI;
            }
            group.add(body);

            if (
                node.pulseHighlight &&
                !isOnPath &&
                !isCenter &&
                pulseEnabled
            ) {
                const pr = new THREE.Mesh(
                    new THREE.SphereGeometry(baseSize * 2.1),
                    new THREE.MeshBasicMaterial({
                        color: "#2dd4bf",
                        transparent: true,
                        opacity: 0.15,
                        wireframe: true,
                    })
                );
                pr.userData = {
                    kmsPulseRing: true,
                    phase: Math.random() * Math.PI * 2,
                };
                group.add(pr);
            }

            const sprite = new SpriteText(node.name || "Note");
            sprite.color = isDimmed ? "rgba(255,255,255,0.35)" : "white";
            sprite.textHeight = isCenter || isHovered ? 6 : 4;
            sprite.fontWeight = "bold";
            sprite.backgroundColor = isDimmed ? "transparent" : "rgba(0,0,0,0.6)";
            sprite.padding = 3;
            sprite.borderRadius = 6;
            sprite.position.y = baseSize + 10;
            applyKmsGraphSpriteTextResolution(sprite, {
                maxDprScale: graphVisualPrefs.spriteLabelMaxDprScale,
                minResScale: graphVisualPrefs.spriteLabelMinResScale,
            });
            group.add(sprite);

            if (isOnPath || isCenter || isHovered) {
                const ring = new THREE.Mesh(
                    new THREE.SphereGeometry(baseSize * 2),
                    new THREE.MeshBasicMaterial({
                        color: isOnPath ? pathGreen : node.color || "#0ea5e9",
                        transparent: true,
                        opacity: isHovered ? 0.12 : 0.08,
                        wireframe: true,
                    })
                );
                group.add(ring);
            }

            return group;
        },
        [
            graphVisualPrefs.spriteLabelMaxDprScale,
            graphVisualPrefs.spriteLabelMinResScale,
            hoverNode,
            neighbors,
            pathNodeSet,
            isCentralId,
            pulseEnabled,
        ]
    );

    if (loading) {
        return (
            <div className="h-full w-full flex flex-col items-center justify-center bg-black/40 rounded-xl backdrop-blur-xl">
                <Loader2 className="h-8 w-8 animate-spin text-dc-accent mb-4" />
                <span className="text-[10px] uppercase tracking-[0.2em] text-white/40">Calibrating Spatial Matrix...</span>
            </div>
        );
    }

    if (error) {
        return (
            <div className="h-full w-full flex flex-col items-center justify-center bg-dc-bg/50 rounded-xl border border-white/5">
                <RefreshCw className="h-5 w-5 text-red-400 mb-3 opacity-50" />
                <p className="text-[10px] text-red-400/80 mb-4">{error}</p>
                <Button variant="ghost" size="sm" onClick={fetchData} className="h-8 text-[10px] px-6 border border-white/10 hover:bg-white/5">Retry</Button>
            </div>
        );
    }

    return (
        <div
            id="kms-spatial-container"
            className="group relative h-full min-h-[300px] w-full overflow-hidden bg-[#020308]"
        >
            <KmsGraphConstellationBackdrop
                hex={{
                    cellRadius: graphVisualPrefs.hexCellRadius,
                    layerOpacity: graphVisualPrefs.hexLayerOpacity,
                    strokeWidth: graphVisualPrefs.hexStrokeWidth,
                    strokeOpacity: graphVisualPrefs.hexStrokeOpacity,
                }}
            />
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
                    </div>
                </div>
            )}
            {data && (
                <div
                    className={cn(
                        "absolute left-0 top-0 z-20 flex h-full max-h-full items-stretch pl-3 pt-3 pb-24 transition-transform duration-300 ease-out pointer-events-none will-change-transform",
                        graphPanelsCollapsed ? "-translate-x-[calc(100%-2.75rem)]" : "translate-x-0"
                    )}
                    style={{ width: "min(100%, 332px)" }}
                >
                    <div className="pointer-events-auto flex min-h-0 w-[min(100%,300px)] max-w-[min(100%,300px)] shrink-0 flex-col gap-2">
                        <div className="relative min-h-0 max-h-[min(440px,calc(100%-3rem))] w-full overflow-x-hidden overflow-y-auto rounded-3xl border border-white/15 bg-gradient-to-br from-black/55 via-black/40 to-black/25 p-3 shadow-[0_16px_48px_rgba(0,0,0,0.5)] ring-1 ring-sky-400/20 backdrop-blur-2xl">
                    <div className="pointer-events-none absolute inset-0 rounded-3xl bg-[radial-gradient(ellipse_at_top_left,rgba(56,189,248,0.12),transparent_55%)]" />
                    <div className="relative">
                    <div className="mb-2.5">
                        <span className="mb-1 block text-[9px] font-bold uppercase tracking-[0.15em] text-sky-200/90">
                            Knowledge constellation
                        </span>
                        <span className="mb-2 block text-[8px] text-white/40">Local spatial graph</span>
                        <span className="mb-1 block text-[8px] uppercase tracking-wider text-white/40">
                            Node colors
                        </span>
                        <div className="flex rounded-md border border-white/10 p-0.5 bg-black/50">
                            <button
                                type="button"
                                className={`flex-1 rounded px-2 py-1 text-[9px] font-medium ${
                                    colorMode === "type"
                                        ? "bg-dc-accent text-white"
                                        : "text-white/50 hover:text-white/85"
                                }`}
                                onClick={() => setColorModePersist("type")}
                            >
                                By type
                            </button>
                            <button
                                type="button"
                                className={`flex-1 rounded px-2 py-1 text-[9px] font-medium ${
                                    colorMode === "folder"
                                        ? "bg-dc-accent text-white"
                                        : "text-white/50 hover:text-white/85"
                                }`}
                                onClick={() => setColorModePersist("folder")}
                            >
                                By folder
                            </button>
                        </div>
                    </div>
                    <div className="mb-2 space-y-1">
                        <span className="text-[8px] uppercase tracking-wider text-white/40 block">
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
                            className="w-full rounded-md border border-white/10 bg-black/50 px-2 py-1.5 text-[10px] text-white placeholder:text-white/35"
                            aria-label="Filter graph nodes"
                        />
                        {legendFilterText.trim() ? (
                            <p className="text-[8px] text-white/45">
                                Showing {vizData.nodes.length} of {filteredData.nodes.length} (after timeline)
                            </p>
                        ) : null}
                    </div>
                    <div className="mb-2 space-y-2">
                        <span className="text-[8px] uppercase tracking-wider text-white/40 block">
                            Recent edits
                        </span>
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
                    {colorMode === "type" && (
                        <div className="mb-2 max-h-24 overflow-y-auto flex flex-col gap-1 pr-0.5">
                            {LEGEND_TYPE_ROWS.map(row => (
                                <label
                                    key={row.type}
                                    className="flex items-center gap-2 cursor-pointer"
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
                                    <span className="text-[8px] text-white/55">{row.label}</span>
                                </label>
                            ))}
                        </div>
                    )}
                    {colorMode === "folder" && folderLegendRows.length > 0 && (
                        <div className="mb-2 max-h-24 overflow-y-auto flex flex-col gap-1 pr-0.5">
                            {folderLegendRows.map(row => (
                                <label
                                    key={row.key}
                                    className="flex items-center gap-2 cursor-pointer min-w-0"
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
                                    <span className="text-[8px] text-white/55 truncate" title={row.key}>
                                        {row.label}
                                    </span>
                                </label>
                            ))}
                        </div>
                    )}
                    <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        className="mb-2 h-6 text-[8px] px-2 text-white/50 border border-white/10 w-full"
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
                    <div className="mb-2 space-y-1.5 rounded-xl border border-white/10 bg-black/30 px-2 py-2">
                        <span className="block text-[8px] font-semibold uppercase tracking-[0.12em] text-sky-200/75">
                            Semantic clusters
                        </span>
                        <p className="text-[8px] leading-snug text-white/50">
                            {islandComponents.length} group
                            {islandComponents.length !== 1 ? "s" : ""}
                            {islandComponents.length > 0
                                ? ` (${islandComponents.map(c => c.length).join(" / ")})`
                                : ""}
                            {legendFilterText.trim() ? " - search may split groups." : ""}
                        </p>
                        {islandComponents.length > 0 && (
                            <div className="flex max-h-24 flex-col gap-1 overflow-y-auto pr-0.5">
                                {islandComponents.map((comp, idx) => {
                                    const hue =
                                        KMS_CONSTELLATION_ISLAND_COLORS[
                                            idx % KMS_CONSTELLATION_ISLAND_COLORS.length
                                        ];
                                    const pct = Math.round(
                                        (100 * comp.length) / maxIslandBarDenominator
                                    );
                                    return (
                                        <button
                                            key={idx}
                                            type="button"
                                            className={`flex w-full min-w-0 items-center gap-1.5 rounded-md border px-1.5 py-1 text-left ${
                                                focusedIslandIdx === idx
                                                    ? "border-sky-400/45 bg-sky-500/15 text-white"
                                                    : "border-white/10 text-white/55 hover:bg-white/5"
                                            }`}
                                            onClick={() =>
                                                setFocusedIslandIdx(
                                                    focusedIslandIdx === idx ? null : idx
                                                )
                                            }
                                        >
                                            <span
                                                className="h-1.5 w-1.5 shrink-0 rounded-full shadow-[0_0_5px_currentColor]"
                                                style={{ backgroundColor: hue, color: hue }}
                                            />
                                            <span className="min-w-0 flex-1 truncate text-[8px]">
                                                C{idx + 1}
                                            </span>
                                            <span className="flex h-1 w-10 shrink-0 overflow-hidden rounded-full bg-white/10">
                                                <span
                                                    className="h-full rounded-full"
                                                    style={{
                                                        width: `${pct}%`,
                                                        backgroundColor: hue,
                                                    }}
                                                />
                                            </span>
                                            <span className="w-5 shrink-0 text-right text-[8px] tabular-nums text-white/45">
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
                            <span className="text-[8px] text-white/50">Hide others</span>
                        </label>
                        <div className="flex flex-wrap gap-1">
                            <Button
                                type="button"
                                variant="secondary"
                                size="sm"
                                className="h-6 text-[8px] px-2 bg-white/10 border-white/10"
                                disabled={legendFilteredNodes.length === 0}
                                onClick={() => {
                                    const p = focusLocalNoteId ?? hoverNode ?? null;
                                    if (!p) return;
                                    const idx = componentIndexContaining(islandComponents, p);
                                    if (idx >= 0) setFocusedIslandIdx(idx);
                                }}
                            >
                                Focus (center / hover)
                            </Button>
                            <Button
                                type="button"
                                variant="ghost"
                                size="sm"
                                className="h-6 text-[8px] px-2 text-white/60"
                                disabled={focusedIslandIdx === null}
                                onClick={() => frameFocusedIsland()}
                            >
                                Frame
                            </Button>
                            {focusedIslandIdx !== null && (
                                <Button
                                    type="button"
                                    variant="ghost"
                                    size="sm"
                                    className="h-6 text-[8px] px-2 text-white/50"
                                    onClick={() => setFocusedIslandIdx(null)}
                                >
                                    Clear
                                </Button>
                            )}
                        </div>
                    </div>
                    <div className="flex items-center gap-2 mb-2 text-white">
                        <Route size={12} className="text-emerald-400 shrink-0" />
                        <span className="text-[9px] font-bold uppercase tracking-wider">Shortest path</span>
                    </div>
                    <div className="flex flex-col gap-1.5">
                        <select
                            value={pathFrom}
                            onChange={e => setPathFrom(e.target.value)}
                            className="text-[10px] rounded-md border border-white/10 bg-black/60 px-2 py-1 text-white max-w-full"
                            aria-label="From note"
                        >
                            <option value="">From...</option>
                            {data.nodes.map(n => (
                                <option key={n.id} value={n.id}>
                                    {n.title || n.path}
                                </option>
                            ))}
                        </select>
                        <select
                            value={pathTo}
                            onChange={e => setPathTo(e.target.value)}
                            className="text-[10px] rounded-md border border-white/10 bg-black/60 px-2 py-1 text-white max-w-full"
                            aria-label="To note"
                        >
                            <option value="">To...</option>
                            {data.nodes.map(n => (
                                <option key={`to-${n.id}`} value={n.id}>
                                    {n.title || n.path}
                                </option>
                            ))}
                        </select>
                        <div className="flex gap-1.5">
                            <Button
                                type="button"
                                variant="secondary"
                                size="sm"
                                className="flex-1 text-[10px] h-7 bg-white/10 border-white/10 text-white hover:bg-white/15"
                                onClick={findPath}
                            >
                                Find
                            </Button>
                            <Button
                                type="button"
                                variant="ghost"
                                size="sm"
                                className="text-[10px] h-7 px-2 text-white/70"
                                onClick={clearPath}
                            >
                                Clear
                            </Button>
                        </div>
                        {pathError && (
                            <p className="text-[9px] text-amber-400/90 leading-snug">{pathError}</p>
                        )}
                        {pathResult?.found && pathResult.node_paths.length > 0 && (
                            <p className="text-[9px] text-white/50 leading-snug">
                                {pathResult.node_paths.length} hops
                            </p>
                        )}
                        {pathResult?.found &&
                            pathEdgeLocalVisibility.total > 0 &&
                            pathEdgeLocalVisibility.visible < pathEdgeLocalVisibility.total && (
                                <p className="text-[9px] text-amber-400/85 leading-snug">
                                    Path continues beyond this neighborhood; only overlapping links are
                                    highlighted here.
                                </p>
                            )}
                    </div>
                    </div>
                        </div>
                    </div>
                <button
                    type="button"
                    className="pointer-events-auto ml-1 flex h-24 w-8 shrink-0 flex-col items-center justify-center gap-0.5 self-center rounded-r-xl border border-white/15 bg-black/60 text-white/70 shadow-lg backdrop-blur-md transition-colors hover:border-sky-400/40 hover:text-sky-300"
                    title={
                        graphPanelsCollapsed
                            ? "Show spatial tools. Shortcut: Ctrl+Shift+G (when Spatial tab is active)"
                            : "Hide tools for more canvas. Shortcut: Ctrl+Shift+G"
                    }
                    aria-expanded={!graphPanelsCollapsed}
                    aria-label={graphPanelsCollapsed ? "Expand spatial graph tools" : "Collapse spatial graph tools"}
                    onClick={toggleGraphPanelsCollapsed}
                >
                    {graphPanelsCollapsed ? <ChevronRight size={16} /> : <ChevronLeft size={16} />}
                    <span className="text-[6px] font-bold uppercase leading-tight tracking-wide [writing-mode:vertical-rl]">
                        Tools
                    </span>
                </button>
                </div>
            )}

            {hoverNode && !String(hoverNode).startsWith("cluster-label-") && (
                <div className="absolute top-3 right-3 z-20 max-h-[min(65vh,380px)] max-w-xs overflow-hidden rounded-2xl border border-white/15 bg-gradient-to-br from-black/60 via-black/45 to-black/30 backdrop-blur-2xl shadow-[0_12px_48px_rgba(0,0,0,0.42)] ring-1 ring-dc-accent/20 p-4 pointer-events-none">
                    <div className="absolute inset-0 rounded-2xl bg-[radial-gradient(ellipse_at_top_right,rgba(14,165,233,0.12),transparent_55%)] pointer-events-none" />
                    <div className="relative">
                        <p className="text-[9px] font-bold uppercase tracking-widest text-dc-accent/95 mb-1">
                            Note preview
                        </p>
                        <h3 className="text-xs font-bold text-white mb-1 tracking-tight">
                            {rpcPreview?.title ||
                                forceGraphData.nodes.find(n => n.id === hoverNode)?.title ||
                                data?.nodes.find(n => n.id === hoverNode)?.title ||
                                hoverNode.split(/[/\\]/).pop()}
                        </h3>
                        <p className="text-[10px] text-white/70 leading-relaxed line-clamp-5 whitespace-pre-wrap border-l-2 border-dc-accent/30 pl-2.5">
                            {rpcPreview?.excerpt || "Loading preview..."}
                        </p>
                    </div>
                </div>
            )}

            <div className="pointer-events-none absolute bottom-16 inset-x-4 z-20 flex flex-col items-center gap-1">
                <div className="pointer-events-auto flex w-full max-w-md items-center gap-2 rounded-2xl border border-sky-400/25 bg-gradient-to-r from-black/60 via-black/45 to-black/30 px-3 py-2 shadow-[0_8px_32px_rgba(0,0,0,0.4)] ring-1 ring-white/10 backdrop-blur-2xl">
                    <div className="flex min-w-[80px] shrink-0 flex-col">
                        <span className="text-[7px] font-bold uppercase tracking-[0.18em] text-sky-300/90">
                            Temporal view
                        </span>
                        <span className="font-mono text-[9px] text-white/50">
                            {new Date(
                                timeRange.min + (timeRange.max - timeRange.min) * (timeFilter / 100)
                            ).toLocaleDateString()}
                        </span>
                    </div>
                    <Button
                        type="button"
                        variant="secondary"
                        size="icon"
                        className="h-8 w-8 shrink-0 rounded-lg border border-white/15 bg-white/5 text-white"
                        title="Play timeline"
                        disabled={playingTimeline}
                        onClick={() => {
                            setTimeFilter(0);
                            setPlayingTimeline(true);
                        }}
                    >
                        <Play size={12} className={playingTimeline ? "opacity-40" : ""} />
                    </Button>
                    <input
                        type="range"
                        min={0}
                        max={100}
                        value={timeFilter}
                        onChange={e => setTimeFilter(Number.parseInt(e.target.value, 10))}
                        className="h-2 min-w-0 flex-1 cursor-pointer appearance-none rounded-full bg-white/15 accent-sky-400 [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:border-2 [&::-webkit-slider-thumb]:border-sky-200 [&::-webkit-slider-thumb]:bg-sky-400 [&::-webkit-slider-thumb]:shadow-[0_0_12px_rgba(56,189,248,0.8)]"
                    />
                    <div className="flex shrink-0 items-center gap-1 rounded border border-white/10 bg-black/30 px-2 py-0.5">
                        <Activity size={10} className={timeFilter < 100 ? "text-sky-400" : "text-white/25"} />
                        <span className="font-mono text-[9px] text-white/60">{timeFilter}%</span>
                    </div>
                </div>
            </div>

            {dimensions.width > 0 && dimensions.height > 0 && (
                <div className="absolute inset-0 z-[1] min-h-0">
                <ForceGraph3D
                    ref={fgRef}
                    width={dimensions.width}
                    height={dimensions.height}
                    graphData={data ? forceGraphData : { nodes: [], links: [] }}
                    nodeThreeObject={nodeThreeObject}
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
                    linkColor={(link: any) => {
                        if (pathEdgeSet.size > 0 && linkOnPathSet(link, pathEdgeSet)) {
                            return "rgba(16, 185, 129, 0.95)";
                        }
                        if (link.isSemantic) {
                            return "rgba(168, 85, 247, 0.5)";
                        }
                        if (link.isSemanticKnn) {
                            return "rgba(251, 191, 36, 0.72)";
                        }
                        const isRelated =
                            hoverNode &&
                            ((typeof link.source === "string"
                                ? link.source
                                : link.source?.id) === hoverNode ||
                                (typeof link.target === "string"
                                    ? link.target
                                    : link.target?.id) === hoverNode);
                        if (hoverNode && !isRelated) return "rgba(255,255,255,0.04)";
                        return "rgba(255,255,255,0.25)";
                    }}
                    linkWidth={(link: any) => {
                        if (pathEdgeSet.size > 0 && linkOnPathSet(link, pathEdgeSet)) return 4;
                        const isRelated =
                            hoverNode &&
                            ((typeof link.source === "string"
                                ? link.source
                                : link.source?.id) === hoverNode ||
                                (typeof link.target === "string"
                                    ? link.target
                                    : link.target?.id) === hoverNode);
                        return isRelated ? 2.5 : 1.5;
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
                                hoverNode && (sidStr === hoverNode || tidStr === hoverNode);
                            if (hoverNode && !isRelated) {
                                return dim ? 0.03 : 0.06;
                            }
                            return dim ? 0.06 : 0.65;
                        }) as unknown as number
                    }
                    backgroundColor="rgba(0,0,0,0)"
                    showNavInfo={false}
                    onNodeClick={(node: any) => {
                        if (node?.isClusterLabel) return;
                        onSelectNote(node.id);
                    }}
                    onNodeHover={(node: any) =>
                        setHoverNode(node && !node.isClusterLabel ? node.id : null)
                    }
                    enableNodeDrag={true}
                    onBackgroundClick={() => setRotationEnabled(!rotationEnabled)}
                    cooldownTicks={150}
                    d3AlphaDecay={0.03}
                    d3VelocityDecay={0.5}
                />
                </div>
            )}

            <div className="absolute inset-x-4 bottom-4 flex justify-between items-center pointer-events-none opacity-0 group-hover:opacity-100 transition-all duration-500">
                <div className="flex flex-col gap-2">
                    <div
                        className="flex items-center gap-2 px-3 py-1.5 rounded-full bg-black/80 border border-white/10 shadow-2xl backdrop-blur-md pointer-events-auto cursor-pointer hover:border-dc-accent/50"
                        onClick={() => setShowDebug(!showDebug)}
                    >
                        <Box size={10} className={cn("text-dc-accent", showDebug ? "animate-pulse" : "")} />
                        <span className="text-[9px] text-white/60 font-medium uppercase tracking-widest whitespace-nowrap">
                            {path.split(/[\\/]/).pop()?.toUpperCase()}
                        </span>
                    </div>
                    {showDebug && (
                        <div className="p-4 rounded-xl bg-black/95 border border-dc-accent/50 text-[10px] font-mono text-dc-accent whitespace-pre pointer-events-auto backdrop-blur-2xl w-[320px] shadow-2xl">
                            <div className="font-bold border-b border-dc-accent/30 mb-2 pb-1 uppercase tracking-widest text-white/90">Diagnostics Console</div>
                            <div className="flex justify-between"><span>Nodes:</span> <span className="text-white">{data?.nodes.length || 0}</span></div>
                            <div className="flex justify-between"><span>Edges:</span> <span className="text-white">{data?.links.length || 0}</span></div>
                            {lastError && <div className="mt-1 text-red-400 text-[8px] break-all border-l-2 border-red-500 pl-1">{lastError}</div>}
                            <div className="mt-2 text-dc-text-muted border-t border-white/5 pt-1 uppercase text-[8px]">GPU Details</div>
                            <div className="overflow-x-auto">{String(debugInfo.webgl || 'Unknown')}</div>
                            <div className="mt-2 text-dc-text-muted border-t border-white/5 pt-1 uppercase text-[8px]">File Context</div>
                            <div className="truncate">{String(debugInfo.targetPath || 'None')}</div>
                            <div className="mt-2 text-[8px] opacity-40 italic">{debugInfo.timestamp || 'No Sync'}</div>
                        </div>
                    )}
                </div>

                <div className="flex gap-2 pointer-events-auto">
                    <Button
                        variant="ghost"
                        size="icon"
                        className="h-8 w-8 rounded-full border border-white/10 text-white/40 hover:text-white"
                        title="Copy graph debug info (JSON)"
                        onClick={copyGraphDebugInfo}
                    >
                        <ClipboardList size={12} />
                    </Button>
                    <Button
                        variant="ghost"
                        size="icon"
                        className="h-8 w-8 rounded-full border border-white/10 text-white/40 hover:text-white"
                        onClick={() => {
                            kmsGraphLog.debug("[Spatial] NUCLEAR TEST: Injecting mock node");
                            setData({
                                nodes: [{ id: "TEST_NODE", dbId: -99, path: "TEST", title: "RENDER_TEST", name: "RENDER_TEST", node_type: "test", last_modified: "", folder_path: "", cluster_id: null, link_centrality: 0, num_links: 0, val: 10, color: "#ff0000" }],
                                links: []
                            });
                            setDebugInfo((prev: any) => ({ ...prev, timestamp: "MOCK_MODE: " + new Date().toLocaleTimeString() }));
                            setTimeout(() => fgRef.current?.zoomToFit(500, 40), 100);
                        }}
                    >
                        <Bug size={12} />
                    </Button>
                    <Button
                        variant="ghost"
                        size="icon"
                        className="h-8 w-8 rounded-full bg-black/80 border border-white/10 text-white/40 hover:text-white"
                        onClick={() => { if (fgRef.current) { fgRef.current.zoomToFit(1000, 40); fetchData(); } }}
                    >
                        <RefreshCw size={12} />
                    </Button>
                    <Button
                        variant="ghost"
                        size="icon"
                        className={cn("h-8 w-8 rounded-full border transition-all", rotationEnabled ? "bg-dc-accent/20 border-dc-accent/50 text-dc-accent" : "bg-black/80 border-white/10 text-white/40")}
                        onClick={() => setRotationEnabled(!rotationEnabled)}
                    >
                        <Move size={12} />
                    </Button>
                </div>
            </div>
        </div>
    );
}
