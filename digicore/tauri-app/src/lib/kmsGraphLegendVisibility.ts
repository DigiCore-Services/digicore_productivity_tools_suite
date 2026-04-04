import { normalizeFolderKey } from "./kmsGraphFolderPalette";

/** Type legend rows (checkbox visibility when color mode is "type"). */
export const LEGEND_TYPE_ROWS: { type: string; label: string }[] = [
    { type: "note", label: "Notes" },
    { type: "skill", label: "Skills" },
    { type: "image", label: "Media" },
    { type: "asset", label: "Assets" },
];

export type GraphLinkEnd = {
    source: string | { id: string };
    target: string | { id: string };
};

export function linkEndpointId(v: string | { id: string }): string {
    return typeof v === "string" ? v : v.id;
}

export type LegendVisibilityNode = {
    id: string;
    folder_path?: string | null;
    node_type?: string | null;
    isClusterLabel?: boolean;
};

/**
 * Drop nodes hidden by legend folder/type toggles; drop links with an endpoint not kept.
 * Cluster label nodes (3D) are always kept.
 */
export function applyLegendVisibilityFilter<
    T extends LegendVisibilityNode,
    L extends GraphLinkEnd,
>(params: {
    nodes: T[];
    links: L[];
    colorMode: "folder" | "type";
    hiddenFolderKeys: Set<string>;
    hiddenNodeTypes: Set<string>;
}): { nodes: T[]; links: L[] } {
    const { nodes, links, colorMode, hiddenFolderKeys, hiddenNodeTypes } = params;
    const keep = new Set<string>();

    for (const n of nodes) {
        if (n.isClusterLabel) {
            keep.add(n.id);
            continue;
        }
        if (colorMode === "folder") {
            const k = normalizeFolderKey(n.folder_path ?? "");
            if (hiddenFolderKeys.has(k)) {
                continue;
            }
        } else {
            const t = (n.node_type || "note").toLowerCase();
            if (hiddenNodeTypes.has(t)) {
                continue;
            }
        }
        keep.add(n.id);
    }

    const filteredNodes = nodes.filter(n => keep.has(n.id));
    const filteredLinks = links.filter(l => {
        const s = linkEndpointId(l.source);
        const t = linkEndpointId(l.target);
        return keep.has(s) && keep.has(t);
    });
    return { nodes: filteredNodes, links: filteredLinks };
}
