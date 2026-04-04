/// <reference lib="webworker" />

import {
    forceCenter,
    forceCollide,
    forceLink,
    forceManyBody,
    forceSimulation,
    forceX,
    forceY,
} from "d3-force";
import type {
    KmsGraphForceLayoutWorkerMessageIn,
    KmsGraphForceLayoutWorkerMessageOut,
    KmsGraphForceLayoutWorkerPayload,
} from "./kmsGraphForceLayoutTypes";

type SimNode = {
    id: string;
    x?: number;
    y?: number;
    folderPath: string;
    clusterId: number | null;
    collisionRadius: number;
};

function computeLayout(p: KmsGraphForceLayoutWorkerPayload) {
    const { width, height, nodes: nodeIn, links: linkIn } = p;

    const folders = Array.from(new Set(nodeIn.map(n => n.folderPath)));
    const folderCenters = new Map<string, { x: number; y: number }>();
    folders.forEach((f, i) => {
        const angle = (i / (folders.length || 1)) * 2 * Math.PI;
        const radius = Math.min(width, height) * 0.3;
        folderCenters.set(f, {
            x: width / 2 + Math.cos(angle) * radius,
            y: height / 2 + Math.sin(angle) * radius,
        });
    });

    let maxClusterId = -1;
    for (const n of nodeIn) {
        if (n.clusterId !== null && n.clusterId !== undefined) {
            maxClusterId = Math.max(maxClusterId, n.clusterId);
        }
    }
    const numClusters = Math.max(1, maxClusterId + 1);
    const clusterCenters = new Map<number, { x: number; y: number }>();
    for (let i = 0; i < numClusters; i++) {
        const angle = (i / numClusters) * 2 * Math.PI;
        const radius = Math.min(width, height) * 0.28;
        clusterCenters.set(i, {
            x: width / 2 + Math.cos(angle) * (radius * 1.2),
            y: height / 2 + Math.sin(angle) * (radius * 1.2),
        });
    }

    const nodes: SimNode[] = nodeIn.map(n => ({
        id: n.id,
        folderPath: n.folderPath,
        clusterId: n.clusterId,
        collisionRadius: n.collisionRadius,
    }));

    const links = linkIn.map(l => ({ source: l.source, target: l.target }));

    const simulation = forceSimulation(nodes as SimNode[])
        .force(
            "link",
            forceLink<SimNode, { source: string; target: string }>(links)
                .id(d => d.id)
                .distance(p.linkDistance)
        )
        .force("charge", forceManyBody().strength(p.chargeStrength))
        .force("center", forceCenter(width / 2, height / 2))
        .force(
            "x",
            forceX<SimNode>(d => {
                if (d.clusterId !== undefined && d.clusterId !== null) {
                    return clusterCenters.get(d.clusterId)?.x ?? width / 2;
                }
                return folderCenters.get(d.folderPath)?.x ?? width / 2;
            }).strength(p.xyStrength)
        )
        .force(
            "y",
            forceY<SimNode>(d => {
                if (d.clusterId !== undefined && d.clusterId !== null) {
                    return clusterCenters.get(d.clusterId)?.y ?? height / 2;
                }
                return folderCenters.get(d.folderPath)?.y ?? height / 2;
            }).strength(p.xyStrength)
        )
        .force(
            "collision",
            forceCollide<SimNode>().radius(d => d.collisionRadius)
        );

    simulation.stop();
    simulation.alpha(1);
    let ticks = 0;
    while (simulation.alpha() > p.alphaMin && ticks < p.maxTicks) {
        simulation.tick();
        ticks += 1;
    }

    return nodes.map(n => ({
        id: n.id,
        x: n.x ?? width / 2,
        y: n.y ?? height / 2,
    }));
}

declare const self: DedicatedWorkerGlobalScope;

self.onmessage = (ev: MessageEvent<KmsGraphForceLayoutWorkerMessageIn>) => {
    const msg = ev.data;
    if (msg.type !== "layout") return;
    try {
        const positions = computeLayout(msg.payload);
        const out: KmsGraphForceLayoutWorkerMessageOut = {
            type: "layout-done",
            id: msg.id,
            positions,
        };
        self.postMessage(out);
    } catch (e) {
        const out: KmsGraphForceLayoutWorkerMessageOut = {
            type: "layout-error",
            id: msg.id,
            message: e instanceof Error ? e.message : String(e),
        };
        self.postMessage(out);
    }
};
