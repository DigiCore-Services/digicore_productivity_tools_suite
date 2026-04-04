import { linkEndpointId, type GraphLinkEnd } from "./kmsGraphLegendVisibility";

/**
 * Weakly connected components on an undirected view of `links`.
 * Only `nodeIds` present in the graph are included; each returned component is a list of ids.
 * Components are sorted by descending size (largest first).
 */
export function computeWeaklyConnectedComponents(
    nodeIds: string[],
    links: GraphLinkEnd[]
): string[][] {
    const idSet = new Set(nodeIds);
    const parent = new Map<string, string>();

    const find = (x: string): string => {
        let p = parent.get(x);
        if (p === undefined) {
            parent.set(x, x);
            return x;
        }
        if (p !== x) {
            const r = find(p);
            parent.set(x, r);
            return r;
        }
        return x;
    };

    const union = (a: string, b: string) => {
        const ra = find(a);
        const rb = find(b);
        if (ra !== rb) {
            parent.set(ra, rb);
        }
    };

    for (const id of nodeIds) {
        if (!parent.has(id)) {
            parent.set(id, id);
        }
    }

    for (const l of links) {
        const s = linkEndpointId(l.source);
        const t = linkEndpointId(l.target);
        if (!idSet.has(s) || !idSet.has(t)) {
            continue;
        }
        union(s, t);
    }

    const groups = new Map<string, string[]>();
    for (const id of nodeIds) {
        const r = find(id);
        const arr = groups.get(r) ?? [];
        arr.push(id);
        groups.set(r, arr);
    }

    return Array.from(groups.values()).sort((a, b) => b.length - a.length);
}

/** Find index of the weak component containing `nodeId`, or -1. */
export function componentIndexContaining(
    components: string[][],
    nodeId: string
): number {
    for (let i = 0; i < components.length; i++) {
        if (components[i].includes(nodeId)) {
            return i;
        }
    }
    return -1;
}
