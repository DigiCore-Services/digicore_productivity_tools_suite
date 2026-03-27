import React, { useEffect, useRef, useState, useMemo, useCallback } from "react";
import ForceGraph3D, { ForceGraphMethods } from "react-force-graph-3d";
import * as THREE from "three";
import SpriteText from "three-spritetext";
import { getTaurpc } from "../../lib/taurpc";
import { KmsGraphDto, KmsNodeDto } from "../../bindings";
import { Loader2, RefreshCw, Box, Move, Crosshair, Bug } from "lucide-react";
import { Button } from "../ui/button";

interface KmsLocalGraph3DProps {
    path: string;
    depth?: number;
    onSelectNote: (path: string) => void;
}

interface GraphNode extends Omit<KmsNodeDto, "id"> {
    id: string; // path used as unique graph ID
    dbId: number; // database i32 id
    name: string; // title
    val: number; // radius equivalent
    color: string;
}

interface GraphLink {
    source: string;
    target: string;
}

interface DebugInfoState {
    nodeCount: number;
    edgeCount: number;
    webgl: string;
    targetPath?: string;
    error?: string;
    timestamp?: string;
}

const cn = (...classes: any[]) => classes.filter(Boolean).join(' ');

export default function KmsLocalGraph3D({ path, depth = 2, onSelectNote }: KmsLocalGraph3DProps) {
    const fgRef = useRef<ForceGraphMethods>();
    const [data, setData] = useState<{ nodes: GraphNode[]; links: GraphLink[] } | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [rotationEnabled, setRotationEnabled] = useState(false);
    const [lastError, setLastError] = useState<string | null>(null);
    const [showDebug, setShowDebug] = useState(false);
    const [dimensions, setDimensions] = useState({ width: 0, height: 0 });

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
            const graphData = await (getTaurpc() as any).kms_get_local_graph(path, depth);
            console.log(`[KMS][Spatial] Received ${graphData.nodes.length} nodes and ${graphData.edges.length} edges for ${path}`);

            if (graphData.nodes.length === 0) {
                setData({
                    nodes: [{
                        id: path, dbId: -1, path: path, title: path.split(/[\\/]/).pop()?.replace('.md', '') || 'Current Note',
                        name: path.split(/[\\/]/).pop()?.replace('.md', '') || 'Current Note',
                        node_type: 'note', last_modified: new Date().toISOString(), folder_path: '',
                        cluster_id: null, val: 3, color: "#ffffff"
                    }],
                    links: []
                });
                return;
            }

            const nodes: GraphNode[] = graphData.nodes.map((n: KmsNodeDto) => {
                const normalizedNodePath = n.path.replace(/\\/g, '/').toLowerCase();
                const isCentral = normalizedNodePath === normalizedTarget;
                return {
                    ...n, id: n.path, dbId: n.id, name: n.title,
                    val: isCentral ? 3 : 1.5, color: isCentral ? "#ffffff" : "#0ea5e9"
                };
            });

            const links: GraphLink[] = graphData.edges.map((e: any) => ({
                source: e.source, target: e.target
            }));

            setData({ nodes, links });
            setDebugInfo(prev => ({
                ...prev, nodeCount: nodes.length, edgeCount: links.length,
                targetPath: normalizedTarget, timestamp: new Date().toLocaleTimeString(),
                bounds: { nodes: nodes.length }
            }));
            setError(null);

            // Triple-Stage Centering for maximum reliability
            const recenter = () => {
                if (fgRef.current) {
                    console.log("[KMS][Spatial] Stage 1: Standard Centering");
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
            console.error("Failed to fetch local 3D graph:", err);
            const errMsg = err?.message || String(err);
            setLastError(errMsg);
            setError("Failed to load local resonance");
        } finally {
            setLoading(false);
        }
    }, [path, depth, normalizedTarget]);

    useEffect(() => {
        fetchData();
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
                            console.log("[KMS][Spatial] Resize detected - Fitting View");
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
    }, [data]);

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
    }, [rotationEnabled, data]);

    const nodeThreeObject = useCallback((node: any) => {
        const group = new THREE.Group();
        const isCenter = node.id === path;
        const baseSize = node.val * 4;

        const material = new THREE.MeshBasicMaterial({
            color: node.color || "#0ea5e9", transparent: true, opacity: 1.0
        });
        group.add(new THREE.Mesh(new THREE.SphereGeometry(baseSize), material));

        const sprite = new SpriteText(node.name || "Note");
        sprite.color = "white";
        sprite.textHeight = isCenter ? 6 : 4;
        sprite.fontWeight = "bold";
        sprite.backgroundColor = "rgba(0,0,0,0.6)";
        sprite.padding = 3;
        sprite.borderRadius = 6;
        sprite.position.y = baseSize + 10;
        group.add(sprite);

        return group;
    }, [path]);

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
        <div id="kms-spatial-container" className="h-full w-full relative group bg-black overflow-hidden min-h-[300px]">
            {dimensions.width > 0 && dimensions.height > 0 && (
                <ForceGraph3D
                    ref={fgRef}
                    width={dimensions.width}
                    height={dimensions.height}
                    graphData={data || { nodes: [], links: [] }}
                    nodeThreeObject={nodeThreeObject}
                    linkColor={() => "rgba(255,255,255,0.2)"}
                    linkWidth={1.5}
                    linkOpacity={0.6}
                    backgroundColor="rgba(0,0,0,0)"
                    showNavInfo={false}
                    onNodeClick={(node: any) => onSelectNote(node.id)}
                    enableNodeDrag={true}
                    onBackgroundClick={() => setRotationEnabled(!rotationEnabled)}
                    cooldownTicks={150}
                    d3AlphaDecay={0.03}
                    d3VelocityDecay={0.5}
                />
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
                        onClick={() => {
                            console.log("[KMS][Spatial] NUCLEAR TEST: Injecting mock node");
                            setData({
                                nodes: [{ id: "TEST_NODE", dbId: -99, path: "TEST", title: "RENDER_TEST", name: "RENDER_TEST", node_type: "test", last_modified: "", folder_path: "", cluster_id: null, val: 10, color: "#ff0000" }],
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
