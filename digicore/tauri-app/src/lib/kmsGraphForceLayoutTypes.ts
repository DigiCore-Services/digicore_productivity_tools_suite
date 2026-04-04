export type KmsGraphForceLayoutWorkerPayload = {
    width: number;
    height: number;
    linkDistance: number;
    chargeStrength: number;
    xyStrength: number;
    alphaMin: number;
    maxTicks: number;
    nodes: Array<{
        id: string;
        folderPath: string;
        clusterId: number | null;
        collisionRadius: number;
    }>;
    links: Array<{ source: string; target: string }>;
};

export type KmsGraphForceLayoutWorkerMessageIn =
    | { type: "layout"; id: number; payload: KmsGraphForceLayoutWorkerPayload }
    | { type: "cancel"; id: number };

export type KmsGraphForceLayoutWorkerMessageOut =
    | { type: "layout-done"; id: number; positions: Array<{ id: string; x: number; y: number }> }
    | { type: "layout-error"; id: number; message: string };
