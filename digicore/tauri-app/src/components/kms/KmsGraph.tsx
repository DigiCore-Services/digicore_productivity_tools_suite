import React, { useEffect, useRef, useState, useMemo, useCallback } from "react";
import * as d3 from "d3";
import { getTaurpc } from "../../lib/taurpc";
import { KmsGraphDto, KmsNodeDto } from "../../bindings";
import { Loader2, ZoomIn, ZoomOut, RefreshCw, Maximize2, Activity } from "lucide-react";
import { Button } from "../ui/button";
import { cn } from "../../lib/utils";

interface KmsGraphProps {
    onSelectNote: (path: string) => void;
    activeNotePath?: string | null;
    isVisible?: boolean;
    resetKey?: number;
}

interface GraphNode extends d3.SimulationNodeDatum {
    id: string; // path
    title: string;
    numLinks: number;
    nodeType: string;
    folderPath: string;
    lastModified: string;
    clusterId?: number | null;
}

interface GraphLink extends d3.SimulationLinkDatum<GraphNode> {
    source: string | GraphNode;
    target: string | GraphNode;
    isAiBeam?: boolean;
    summary?: string;
}

export default function KmsGraph({ onSelectNote, activeNotePath, isVisible, resetKey }: KmsGraphProps) {
    const svgRef = useRef<SVGSVGElement>(null);
    const containerRef = useRef<HTMLDivElement>(null);
    const [data, setData] = useState<KmsGraphDto | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [dimensions, setDimensions] = useState({ width: 0, height: 0 });
    const [timeFilter, setTimeFilter] = useState(100); // 0-100%
    const zoomBehaviorRef = useRef<d3.ZoomBehavior<SVGSVGElement, unknown> | null>(null);

    const handleResize = useCallback(() => {
        if (containerRef.current) {
            setDimensions({
                width: containerRef.current.clientWidth,
                height: containerRef.current.clientHeight
            });
        }
    }, []);

    useEffect(() => {
        handleResize(); // Initial size
        window.addEventListener("resize", handleResize);
        return () => window.removeEventListener("resize", handleResize);
    }, [handleResize]);

    useEffect(() => {
        if (isVisible) {
            handleResize();
        }
    }, [isVisible, handleResize]);

    const fetchData = async () => {
        setLoading(true);
        try {
            const graphData = await getTaurpc().kms_get_graph();
            setData(graphData);
            setError(null);
        } catch (err) {
            console.error("Failed to fetch graph data:", err);
            setError("Failed to load graph data");
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        fetchData();
    }, []);

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

        const structural = data.edges.filter(e => nodeIds.has(e.source) && nodeIds.has(e.target));
        const aiBeams = (data as any).ai_beams?.filter((b: any) => nodeIds.has(b.source_path) && nodeIds.has(b.target_path))
            .map((b: any) => ({
                source: b.source_path,
                target: b.target_path,
                isAiBeam: true,
                summary: b.summary
            })) || [];

        return [...structural, ...aiBeams];
    }, [data, filteredNodes]);

    useEffect(() => {
        if (!data || !svgRef.current || !containerRef.current) return;

        const { width, height } = dimensions;
        if (width === 0 || height === 0) return;

        const svg = d3.select(svgRef.current);
        svg.selectAll("*").remove(); // Clear previous rendering

        const g = svg.append("g");

        // Add pulse animation style
        svg.append("defs").append("style").text(`
            @keyframes pulse-ring {
                0% { transform: scale(0.8); opacity: 0.8; stroke-width: 2px; }
                50% { transform: scale(1.5); opacity: 0; stroke-width: 1px; }
                100% { transform: scale(0.8); opacity: 0; stroke-width: 0px; }
            }
            .pulse-circle {
                animation: pulse-ring 2s cubic-bezier(0.24, 0, 0.38, 1) infinite;
                transform-origin: center;
            }
        `);

        // Zoom helper
        const zoom = d3.zoom<SVGSVGElement, unknown>()
            .scaleExtent([0.1, 4])
            .on("zoom", (event) => {
                g.attr("transform", event.transform);
            });

        zoomBehaviorRef.current = zoom;
        svg.call(zoom as any);

        // Prepare data for D3
        const nodes: GraphNode[] = filteredNodes.map(n => ({
            id: n.path,
            title: n.title,
            nodeType: n.node_type || "note",
            folderPath: n.folder_path || "",
            lastModified: n.last_modified || "",
            numLinks: data.edges.filter(e => e.source === n.path || e.target === n.path).length,
            clusterId: n.cluster_id,
        }));

        const links: GraphLink[] = filteredLinks
            .map(e => ({
                source: e.source,
                target: e.target,
                isAiBeam: e.isAiBeam,
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

        // Clustering logic: Calculate centers for semantic clusters
        const clusterCenters = new Map<number, { x: number, y: number }>();
        const numClusters = 10;
        for (let i = 0; i < numClusters; i++) {
            const angle = (i / numClusters) * 2 * Math.PI;
            const radius = Math.min(width, height) * 0.28;
            clusterCenters.set(i, {
                x: width / 2 + Math.cos(angle) * (radius * 1.2),
                y: height / 2 + Math.sin(angle) * (radius * 1.2)
            });
        }

        // Color scale for node types
        const colorScale = d3.scaleOrdinal<string>()
            .domain(["note", "skill", "image", "asset"])
            .range(["var(--dc-accent)", "#f59e0b", "#ec4899", "#8b5cf6"]);

        const simulation = d3.forceSimulation<GraphNode>(nodes)
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
            .force("collision", d3.forceCollide<GraphNode>().radius(d => Math.sqrt(d.numLinks || 1) * 5 + 35));

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
            .attr("stroke", d => d.isAiBeam ? "#a855f7" : "var(--dc-border)")
            .attr("stroke-width", d => d.isAiBeam ? 3 : 1)
            .attr("stroke-opacity", d => d.isAiBeam ? 0.8 : 0.4)
            .attr("stroke-dasharray", d => d.isAiBeam ? "5,5" : "none")
            .attr("marker-end", d => d.isAiBeam ? "url(#arrowhead-ai)" : "url(#arrowhead)");

        link.filter(d => !!d.isAiBeam)
            .append("title")
            .text(d => `AI Summary: ${d.summary}`);

        // Nodes
        // Node icons (simplified paths)
        const iconPaths: Record<string, string> = {
            note: "M9 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V9l-6-6z",
            skill: "M13 2L3 14h9l-1 8 10-12h-9l1-8z",
            image: "M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h7 M16 5l5 5 M21 5l-5 5", // Simplified image/asset
            asset: "M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h7",
        };

        const node = g.append("g")
            .attr("class", "nodes")
            .selectAll(".node")
            .data(nodes)
            .enter().append("g")
            .attr("class", "node")
            .call(d3.drag<SVGGElement, GraphNode>()
                .on("start", dragstarted)
                .on("drag", dragged)
                .on("end", dragended))
            .on("click", (event, d) => {
                if (event.defaultPrevented) return;
                onSelectNote(d.id);
            });

        // Pulsing ring for active node
        node.filter(d => d.id === activeNotePath)
            .append("circle")
            .attr("r", d => Math.sqrt(d.numLinks || 1) * 3 + 12)
            .attr("fill", "none")
            .attr("stroke", d => colorScale(d.nodeType))
            .attr("class", "pulse-circle")
            .style("pointer-events", "none");

        // Node circles
        node.append("circle")
            .attr("r", d => Math.sqrt(d.numLinks || 1) * 3 + 8)
            .attr("fill", d => colorScale(d.nodeType))
            .attr("fill-opacity", d => d.id === activeNotePath ? 0.4 : 0.15)
            .attr("stroke", d => colorScale(d.nodeType))
            .attr("stroke-width", d => d.id === activeNotePath ? 4 : 2)
            .style("cursor", "pointer")
            .style("transition", "all 0.2s ease")
            .on("mouseover", function (event, d) {
                d3.select(this).attr("stroke-width", 4).attr("fill-opacity", 0.3);
            })
            .on("mouseout", function (event, d) {
                d3.select(this).attr("stroke-width", 2).attr("fill-opacity", 0.15);
            });

        // Icon path
        node.append("path")
            .attr("d", d => iconPaths[d.nodeType] || iconPaths.note)
            .attr("transform", "scale(0.6) translate(-12, -12)")
            .attr("fill", d => colorScale(d.nodeType))
            .attr("fill-opacity", 0.9)
            .style("pointer-events", "none");

        // Inner glows for types
        node.append("circle")
            .attr("r", d => Math.sqrt(d.numLinks || 1) * 3 + 2)
            .attr("fill", d => colorScale(d.nodeType))
            .style("filter", "blur(4px)")
            .style("opacity", 0.4)
            .style("pointer-events", "none");

        // Tooltip (simple SVG title for now)
        node.append("title")
            .text(d => `${d.title}\nType: ${d.nodeType}\nFolder: ${d.folderPath || 'Root'}\nModified: ${d.lastModified}`);

        // Node labels
        node.append("text")
            .attr("dy", d => Math.sqrt(d.numLinks || 1) * 3 + 20)
            .attr("text-anchor", "middle")
            .text(d => d.title)
            .attr("fill", "var(--dc-text)")
            .style("font-size", "10px")
            .style("pointer-events", "none")
            .style("text-shadow", "0 1px 2px rgba(0,0,0,0.5)");

        simulation.on("tick", () => {
            link
                .attr("x1", d => (d.source as GraphNode).x!)
                .attr("y1", d => (d.source as GraphNode).y!)
                .attr("x2", d => (d.target as GraphNode).x!)
                .attr("y2", d => (d.target as GraphNode).y!);

            node.attr("transform", d => `translate(${d.x},${d.y})`);
        });

        function dragstarted(event: any, d: any) {
            if (!event.active) simulation.alphaTarget(0.3).restart();
            d.fx = d.x;
            d.fy = d.y;
        }

        function dragged(event: any, d: any) {
            d.fx = event.x;
            d.fy = event.y;
        }

        function dragended(event: any, d: any) {
            if (!event.active) simulation.alphaTarget(0);
            d.fx = null;
            d.fy = null;
        }

        return () => {
            simulation.stop();
        };
    }, [data, filteredNodes, filteredLinks, onSelectNote, dimensions]);

    useEffect(() => {
        if (svgRef.current && zoomBehaviorRef.current && resetKey !== undefined) {
            d3.select(svgRef.current)
                .transition()
                .duration(750)
                .call(zoomBehaviorRef.current.transform as any, d3.zoomIdentity);
        }
    }, [resetKey]);

    if (loading) {
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

    return (
        <div ref={containerRef} className="flex-1 relative overflow-hidden bg-dc-bg select-none">
            <svg ref={svgRef} className="w-full h-full" />

            {/* Legend/Info Badge */}
            <div className="absolute top-6 left-6 p-4 rounded-2xl bg-dc-bg-secondary/40 backdrop-blur-xl border border-dc-border shadow-2xl pointer-events-none">
                <div className="flex items-center gap-3 mb-4">
                    <div className="w-3 h-3 rounded-full bg-dc-accent shadow-[0_0_8px_rgba(var(--dc-accent-rgb),0.5)]" />
                    <span className="text-xs font-bold uppercase tracking-wider text-dc-text">Knowledge Map v2.1</span>
                </div>

                <div className="flex flex-col gap-2">
                    <div className="flex items-center gap-2">
                        <div className="w-2 h-2 rounded-full bg-dc-accent" />
                        <span className="text-[10px] text-dc-text-muted">Notes</span>
                    </div>
                    <div className="flex items-center gap-2">
                        <div className="w-2 h-2 rounded-full bg-[#f59e0b]" />
                        <span className="text-[10px] text-dc-text-muted">Skills</span>
                    </div>
                    <div className="flex items-center gap-2">
                        <div className="w-2 h-2 rounded-full bg-[#ec4899]" />
                        <span className="text-[10px] text-dc-text-muted">Media</span>
                    </div>
                    <div className="flex items-center gap-2">
                        <div className="w-2 h-2 rounded-full bg-[#a855f7]" />
                        <span className="text-[10px] text-dc-text-muted">AI Beams</span>
                    </div>
                </div>

                <div className="mt-4 pt-4 border-t border-dc-border/50 text-[10px] text-dc-text-muted">
                    {filteredNodes.length} Nodes • {filteredLinks.length} Connections
                </div>
            </div>

            {/* Timeline UI */}
            <div className="absolute bottom-6 inset-x-0 mx-auto w-1/2 flex flex-col gap-2 items-center pointer-events-none">
                <div className="w-full h-12 p-3 flex items-center gap-4 bg-dc-bg-secondary/60 backdrop-blur-xl border border-dc-border rounded-2xl shadow-2xl pointer-events-auto">
                    <div className="flex flex-col min-w-[100px]">
                        <span className="text-[8px] uppercase tracking-widest text-dc-accent font-bold">Timeline Basis</span>
                        <span className="text-[10px] text-dc-text-muted font-mono">
                            {new Date(timeRange.min + (timeRange.max - timeRange.min) * (timeFilter / 100)).toLocaleDateString()}
                        </span>
                    </div>

                    <input
                        type="range"
                        min="0"
                        max="100"
                        value={timeFilter}
                        onChange={(e) => setTimeFilter(Number.parseInt(e.target.value))}
                        className="flex-1 h-1.5 bg-dc-border rounded-lg appearance-none cursor-pointer accent-dc-accent hover:accent-dc-accent/80 transition-all"
                    />

                    <div className="flex items-center gap-2 px-3 py-1 bg-dc-bg-secondary rounded-lg border border-dc-border">
                        <Activity size={12} className={timeFilter < 100 ? "text-dc-accent animate-pulse" : "text-dc-text-muted/40"} />
                        <span className="text-[10px] font-bold text-dc-text-muted">{timeFilter}%</span>
                    </div>
                </div>
            </div>

            {/* Controls */}
            <div className="absolute bottom-6 right-6 flex flex-col gap-2">
                <Button variant="secondary" size="icon" className="h-10 w-10 rounded-xl shadow-lg border border-dc-border bg-dc-bg-secondary/80 backdrop-blur-md"
                    onClick={fetchData} title="Refresh Graph">
                    <RefreshCw size={16} />
                </Button>
            </div>
        </div>
    );
}
