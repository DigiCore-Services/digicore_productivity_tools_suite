/**
 * Pure helpers for KMS graph path highlighting and link identity (2D/3D).
 */

/** Stable key for an undirected edge between two vault paths. */
export function undirectedEdgeKey(a: string, b: string): string {
    return [a, b].sort().join("|");
}

/** Keys for all edges returned by `kms_get_graph_shortest_path` when `found` is true. */
export function pathEdgeSetFromDto(
    found: boolean,
    edges: Array<{ source: string; target: string }> | null | undefined
): Set<string> {
    const set = new Set<string>();
    if (!found || !edges?.length) return set;
    for (const e of edges) {
        set.add(undirectedEdgeKey(e.source, e.target));
    }
    return set;
}

/** Absolute node paths on a found shortest path. */
export function pathNodeSetFromDto(
    found: boolean,
    nodePaths: string[] | null | undefined
): Set<string> {
    if (!found || !nodePaths?.length) return new Set();
    return new Set(nodePaths);
}

/**
 * Whether a force-graph link (source/target may be string id or resolved node) lies on the path set.
 */
export function linkOnPathSet(
    link: { source: unknown; target: unknown },
    pathEdgeSet: Set<string>
): boolean {
    const s =
        typeof link.source === "string"
            ? link.source
            : String((link.source as { id?: string }).id ?? "");
    const t =
        typeof link.target === "string"
            ? link.target
            : String((link.target as { id?: string }).id ?? "");
    if (!s || !t) return false;
    return pathEdgeSet.has(undirectedEdgeKey(s, t));
}

/** Undirected keys for force-graph / local subgraph links (string or resolved node id). */
export function linkKeysFromGraphLinks(
    links: ReadonlyArray<{ source: unknown; target: unknown }>
): Set<string> {
    const set = new Set<string>();
    for (const l of links) {
        const s =
            typeof l.source === "string"
                ? l.source
                : String((l.source as { id?: string }).id ?? "");
        const t =
            typeof l.target === "string"
                ? l.target
                : String((l.target as { id?: string }).id ?? "");
        if (s && t) set.add(undirectedEdgeKey(s, t));
    }
    return set;
}

/** Counts how many global shortest-path edges appear in a local neighborhood link set. */
export function visiblePathEdgeCount(
    pathEdges: ReadonlyArray<{ source: string; target: string }> | null | undefined,
    localLinkKeys: Set<string>
): { total: number; visible: number } {
    if (!pathEdges?.length) return { total: 0, visible: 0 };
    let visible = 0;
    for (const e of pathEdges) {
        if (localLinkKeys.has(undirectedEdgeKey(e.source, e.target))) visible++;
    }
    return { total: pathEdges.length, visible };
}
