import React, { useEffect, useRef, useState, useMemo, useCallback } from "react";
import ForceGraph3D, { ForceGraphMethods } from "react-force-graph-3d";
import * as THREE from "three";
import SpriteText from "three-spritetext";
import { getTaurpc } from "../../lib/taurpc";
import { KmsGraphDto, KmsNodeDto } from "../../bindings";
import { Loader2, RefreshCw, Box, Layers, MousePointer2, Activity } from "lucide-react";
import { Button } from "../ui/button";

interface KmsGraph3DProps {
    onSelectNote: (path: string) => void;
    activeNotePath?: string | null;
    isVisible?: boolean;
    resetKey?: number;
}

interface GraphNode extends Omit<KmsNodeDto, "id"> {
    id: string; // path used as unique graph ID
    dbId: number; // database i32 id
    name: string; // title
    val: number; // radius equivalent
    color: string;
    isClusterLabel?: boolean;
    preview?: string | null;
}

interface GraphLink {
    source: string;
    target: string;
    isSemantic?: boolean;
    isAiBeam?: boolean;
    summary?: string;
}

export default function KmsGraph3D({ onSelectNote, activeNotePath, isVisible, resetKey }: KmsGraph3DProps) {
    const fgRef = useRef<ForceGraphMethods>();
    const [data, setData] = useState<{ nodes: GraphNode[]; links: GraphLink[] } | null>(null);
    const [clusterLabels, setClusterLabels] = useState<any[]>([]);
    const [centroids, setCentroids] = useState<any[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [timeFilter, setTimeFilter] = useState(100); // 0-100%
    const [hoverNode, setHoverNode] = useState<string | null>(null);
    const [hoverLink, setHoverLink] = useState<GraphLink | null>(null);

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

    const fetchData = useCallback(async () => {
        setLoading(true);
        try {
            const graphData = await getTaurpc().kms_get_graph();
            const nodes: GraphNode[] = graphData.nodes.map(n => ({
                ...n,
                id: n.path,
                dbId: n.id,
                name: n.title,
                val: 1, // Base value
                color: colorScale(n.node_type)
            }));

            const structuralLinks: GraphLink[] = graphData.edges.map(e => ({
                source: e.source,
                target: e.target
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
                val: 0,
                color: "rgba(0, 255, 255, 0.8)"
            }));

            const aiBeamsLinks: GraphLink[] = ((graphData as any).ai_beams || []).map((b: any) => ({
                source: b.source_path,
                target: b.target_path,
                isAiBeam: true,
                summary: b.summary
            }));

            setData({ nodes: [...nodes, ...labelNodes], links: [...structuralLinks, ...semanticLinks, ...aiBeamsLinks] });
            setError(null);
        } catch (err) {
            console.error("Failed to fetch 3D graph data:", err);
            setError("Failed to load 3D graph data");
        } finally {
            setLoading(false);
        }
    }, [colorScale]);

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
        if (!data) return { nodes: [], links: [] };
        if (timeFilter === 100) return data;

        const threshold = timeRange.min + (timeRange.max - timeRange.min) * (timeFilter / 100);

        const filteredNodes = data.nodes.filter(n => {
            if (n.isClusterLabel) return true; // Always show cluster labels for context
            const t = new Date(n.last_modified).getTime();
            return isNaN(t) || t <= threshold;
        });

        const nodeIds = new Set(filteredNodes.map(n => n.id));
        const filteredLinks = data.links.filter(l => {
            const s = typeof l.source === 'string' ? l.source : (l.source as any).id;
            const t = typeof l.target === 'string' ? l.target : (l.target as any).id;
            return nodeIds.has(s) && nodeIds.has(t);
        });

        return { nodes: filteredNodes, links: filteredLinks };
    }, [data, timeFilter, timeRange]);

    const neighbors = useMemo(() => {
        if (!hoverNode || !filteredData) return new Set<string>();
        const res = new Set<string>();
        filteredData.links.forEach(l => {
            const s = typeof l.source === 'string' ? l.source : (l.source as any).id;
            const t = typeof l.target === 'string' ? l.target : (l.target as any).id;
            if (s === hoverNode) res.add(t);
            if (t === hoverNode) res.add(s);
        });
        return res;
    }, [hoverNode, filteredData]);

    useEffect(() => {
        fetchData();
    }, [fetchData]);

    // Apply Semantic Z-Forces and Centroid Logic
    useEffect(() => {
        if (!fgRef.current || !data) return;

        // Add semantic Z-force attraction
        fgRef.current.d3Force('z', (alpha: number) => {
            data.nodes.forEach((node: any) => {
                if (node.cluster_id !== undefined && node.cluster_id !== null) {
                    const targetZ = (node.cluster_id - 5) * 150;
                    node.vz = (node.vz || 0) + (targetZ - node.z) * 0.1 * alpha;
                }
            });
        });

        fgRef.current.d3Force('link')?.distance(120);
        fgRef.current.d3Force('charge')?.strength(-150);

        const interval = setInterval(() => {
            if (!fgRef.current || !data) return;

            const groups = new Map<number, { x: number, y: number, z: number, count: number }>();
            data.nodes.forEach((n: any) => {
                if (!n.isClusterLabel && n.cluster_id !== null && n.cluster_id !== undefined) {
                    const current = groups.get(n.cluster_id) || { x: 0, y: 0, z: 0, count: 0 };
                    groups.set(n.cluster_id, {
                        x: current.x + (n.x || 0),
                        y: current.y + (n.y || 0),
                        z: current.z + (n.z || 0),
                        count: current.count + 1
                    });
                }
            });

            data.nodes.forEach((n: any) => {
                if (n.isClusterLabel && n.cluster_id !== undefined) {
                    const center = groups.get(n.cluster_id);
                    if (center && center.count > 0) {
                        n.x = center.x / center.count;
                        n.y = center.y / center.count + 60;
                        n.z = center.z / center.count;
                    }
                }
            });
        }, 100);

        return () => clearInterval(interval);
    }, [data]);

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

    const nodeThreeObject = useCallback((node: any) => {
        if (node.isClusterLabel) {
            const sprite = new SpriteText(node.name);
            sprite.color = "rgba(0, 255, 255, 1.0)";
            sprite.textHeight = 15;
            sprite.fontWeight = "bold";
            sprite.backgroundColor = "rgba(0,0,0,0.8)";
            sprite.padding = 6;
            sprite.borderRadius = 10;
            sprite.borderWidth = 2;
            sprite.borderColor = "rgba(0, 255, 255, 0.4)";
            return sprite;
        }

        const group = new THREE.Group();
        const isHovered = node.id === hoverNode;
        const isNeighbor = neighbors.has(node.id);
        const isDimmed = hoverNode && !isHovered && !isNeighbor;
        const isActive = node.id === activeNotePath;

        const linkCount = node.num_links || 1;
        const scaleFactor = Math.min(2.5, 1 + Math.log10(linkCount + 1));
        const baseRadius = scaleFactor * 5;

        const geometry = new THREE.SphereGeometry(baseRadius);
        const material = new THREE.MeshStandardMaterial({
            color: node.color,
            transparent: true,
            opacity: isDimmed ? 0.2 : 0.9,
            emissive: node.color,
            emissiveIntensity: isHovered ? 5.0 : isNeighbor ? 2.5 : isDimmed ? 0.1 : 0.8,
            roughness: 0.1,
            metalness: 0.9
        });
        const sphere = new THREE.Mesh(geometry, material);
        group.add(sphere);

        const sprite = new SpriteText(node.title);
        sprite.color = isDimmed ? "rgba(255,255,255,0.2)" : "white";
        sprite.textHeight = (isHovered || isNeighbor) ? 7 : 5;
        sprite.fontWeight = (isHovered || isNeighbor) ? "900" : "bold";
        sprite.padding = 2;
        sprite.backgroundColor = isDimmed ? "transparent" : "rgba(0,0,0,0.3)";
        sprite.borderRadius = 6;
        sprite.position.y = baseRadius + 10;
        group.add(sprite);

        if (isActive || isHovered) {
            // Pulse/Glow Ring
            const ringGeom = new THREE.SphereGeometry(baseRadius * 2.5);
            const ringMat = new THREE.MeshBasicMaterial({
                color: node.color,
                transparent: true,
                opacity: isHovered ? 0.15 : 0.05,
                wireframe: true
            });
            const ring = new THREE.Mesh(ringGeom, ringMat);
            group.add(ring);

            // Add a point light to the focused node
            const light = new THREE.PointLight(node.color, isHovered ? 3 : 2, baseRadius * 10);
            group.add(light);
        }

        return group;
    }, [activeNotePath, hoverNode, neighbors, timeFilter, timeRange]);

    if (loading) {
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

    return (
        <div className="flex-1 relative bg-[#050505] overflow-hidden">
            <ForceGraph3D
                ref={fgRef}
                graphData={filteredData}
                nodeLabel="title"
                nodeAutoColorBy="node_type"
                nodeThreeObject={nodeThreeObject}
                nodeThreeObjectExtend={false}
                linkColor={(link: any) => {
                    const isRelated = hoverNode && (
                        (typeof link.source === 'string' ? link.source === hoverNode : (link.source as any).id === hoverNode) ||
                        (typeof link.target === 'string' ? link.target === hoverNode : (link.target as any).id === hoverNode)
                    );
                    if (hoverNode && !isRelated) return "rgba(255,255,255,0.02)";
                    if (link.isAiBeam) return "rgba(168, 85, 247, 0.8)"; // Purple/Violet
                    return link.isSemantic ? "rgba(0, 255, 255, 0.2)" : "rgba(255,255,255,0.6)";
                }}
                linkWidth={(link: any) => {
                    const isRelated = hoverNode && (
                        (typeof link.source === 'string' ? link.source === hoverNode : (link.source as any).id === hoverNode) ||
                        (typeof link.target === 'string' ? link.target === hoverNode : (link.target as any).id === hoverNode)
                    );
                    if (link.isAiBeam) return isRelated ? 8 : 4;
                    return isRelated ? 4 : (link.isSemantic ? 1 : 2);
                }}
                linkOpacity={0.6}
                linkDirectionalParticles={(link: any) => link.isAiBeam ? 6 : (link.isSemantic ? 0 : 2)}
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
                backgroundColor="#050505"
                showNavInfo={false}
                controlType="orbit"
            />

            <div className="absolute top-6 left-6 p-4 rounded-2xl bg-black/40 backdrop-blur-xl border border-white/10 shadow-2xl pointer-events-none select-none min-w-[200px]">
                <div className="flex items-center gap-3 mb-4">
                    <div className="p-1.5 rounded-lg bg-dc-accent/20 border border-dc-accent/30">
                        <Box size={14} className="text-dc-accent" />
                    </div>
                    <div>
                        <h2 className="text-[10px] font-bold uppercase tracking-[0.2em] text-white/80">Hyper-Graph v3.0</h2>
                        <p className="text-[8px] text-white/40 italic">Semantic Spatial Mode</p>
                    </div>
                </div>
                <div className="space-y-2 mb-4">
                    <div className="flex items-center justify-between">
                        <span className="text-[9px] text-white/40 uppercase tracking-wider">Nodes</span>
                        <span className="text-[9px] font-mono text-dc-accent">{data?.nodes.length}</span>
                    </div>
                </div>
                <div className="grid grid-cols-2 gap-2 mt-4 pt-4 border-t border-white/5">
                    <div className="flex items-center gap-1.5">
                        <div className="w-1.5 h-1.5 rounded-full bg-dc-accent" />
                        <span className="text-[8px] text-white/60">Notes</span>
                    </div>
                    <div className="flex items-center gap-1.5">
                        <div className="w-1.5 h-1.5 rounded-full bg-amber-500" />
                        <span className="text-[8px] text-white/60">Skills</span>
                    </div>
                </div>
            </div>

            {hoverNode && !hoverNode.startsWith('cluster-label-') && (
                <div className="absolute top-6 left-[280px] p-4 rounded-2xl bg-dc-accent/10 backdrop-blur-2xl border border-dc-accent/30 shadow-[0_0_30px_rgba(14,165,233,0.2)] pointer-events-none select-none max-w-[350px] animate-in fade-in slide-in-from-left-4 duration-300">
                    <div className="flex flex-col gap-2">
                        <div className="flex items-center gap-2">
                            <div className="w-2 h-2 rounded-full bg-dc-accent animate-pulse" />
                            <span className="text-[10px] font-bold uppercase tracking-widest text-dc-accent">Knowledge Hologram</span>
                        </div>
                        <h3 className="text-sm font-bold text-white mb-1">
                            {filteredData.nodes.find(n => n.id === hoverNode)?.title}
                        </h3>
                        <p className="text-[11px] text-white/60 leading-relaxed line-clamp-4 italic border-l-2 border-white/10 pl-3">
                            {filteredData.nodes.find(n => n.id === hoverNode)?.preview || "No preview available for this node structure."}
                        </p>
                    </div>
                </div>
            )}
            {hoverLink && hoverLink.isAiBeam && (
                <div className="absolute top-24 left-[280px] p-4 rounded-2xl bg-purple-500/10 backdrop-blur-2xl border border-purple-500/30 shadow-[0_0_30px_rgba(168,85,247,0.3)] pointer-events-none select-none max-w-[350px] animate-in fade-in zoom-in duration-300">
                    <div className="flex flex-col gap-2">
                        <div className="flex items-center gap-2">
                            <Activity size={12} className="text-purple-400 animate-pulse" />
                            <span className="text-[10px] font-bold uppercase tracking-widest text-purple-400">AI Summary Beam</span>
                        </div>
                        <p className="text-[11px] text-white/80 leading-relaxed italic border-l-2 border-purple-500/30 pl-3">
                            {hoverLink.summary}
                        </p>
                    </div>
                </div>
            )}

            {/* Timeline UI */}
            <div className="absolute bottom-6 inset-x-0 mx-auto w-1/2 flex flex-col gap-2 items-center pointer-events-none">
                <div className="w-full h-12 p-3 flex items-center gap-4 bg-black/40 backdrop-blur-xl border border-white/10 rounded-2xl shadow-2xl pointer-events-auto">
                    <div className="flex flex-col min-w-[100px]">
                        <span className="text-[8px] uppercase tracking-widest text-dc-accent font-bold">Timeline Basis</span>
                        <span className="text-[10px] text-white/60 font-mono">
                            {new Date(timeRange.min + (timeRange.max - timeRange.min) * (timeFilter / 100)).toLocaleDateString()}
                        </span>
                    </div>

                    <input
                        type="range"
                        min="0"
                        max="100"
                        value={timeFilter}
                        onChange={(e) => setTimeFilter(Number.parseInt(e.target.value))}
                        className="flex-1 h-1.5 bg-white/10 rounded-lg appearance-none cursor-pointer accent-dc-accent hover:accent-dc-accent/80 transition-all"
                    />

                    <div className="flex items-center gap-2 px-3 py-1 bg-white/5 rounded-lg border border-white/5">
                        <Activity size={12} className={timeFilter < 100 ? "text-dc-accent animate-pulse" : "text-white/20"} />
                        <span className="text-[10px] font-bold text-white/60">{timeFilter}%</span>
                    </div>
                </div>
                <span className="text-[9px] uppercase tracking-[0.3em] font-bold text-white/20 select-none">Temporal Evolution Mode</span>
            </div>

            <div className="absolute bottom-6 left-6 flex items-center gap-2 p-2 rounded-xl bg-black/40 backdrop-blur-md border border-white/10">
                <div className="flex items-center gap-2 px-3 py-1.5 border-r border-white/10">
                    <Layers size={14} className="text-white/40" />
                    <span className="text-[10px] text-white/60 font-medium">Auto-Clustering</span>
                    <div className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse" />
                </div>
            </div>

            <div className="absolute top-6 right-6">
                <Button variant="ghost" size="icon" className="h-10 w-10 rounded-xl bg-black/40 backdrop-blur-md border border-white/10 text-white/60 hover:text-white hover:bg-white/5" onClick={fetchData}>
                    <RefreshCw size={16} />
                </Button>
            </div>
        </div>
    );
}
