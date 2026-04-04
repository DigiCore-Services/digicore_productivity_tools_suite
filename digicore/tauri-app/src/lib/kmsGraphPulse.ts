/**
 * Recency scoring for "pulse" highlighting of recently modified notes on the graph.
 * Uses min/max last_modified across the current graph payload (same basis as the time slider).
 */

export function lastModifiedMs(iso: string | undefined | null): number | null {
    if (iso == null || String(iso).trim() === "") return null;
    const t = new Date(iso).getTime();
    return Number.isFinite(t) ? t : null;
}

/** 0 = oldest in range, 1 = newest in range. */
export function recency01(
    ms: number | null,
    rangeMin: number,
    rangeMax: number
): number {
    if (ms == null || rangeMax <= rangeMin) return 0;
    return Math.max(0, Math.min(1, (ms - rangeMin) / (rangeMax - rangeMin)));
}

export function graphLastModifiedRange(nodes: { last_modified?: string }[]): {
    min: number;
    max: number;
} {
    const times: number[] = [];
    for (const n of nodes) {
        const m = lastModifiedMs(n.last_modified);
        if (m != null) times.push(m);
    }
    if (times.length === 0) {
        const now = Date.now();
        return { min: now, max: now };
    }
    return { min: Math.min(...times), max: Math.max(...times) };
}

/** Top `topPercent` (1-100) most recent nodes in the range qualify for pulse when enabled. */
export function shouldPulseRecent(
    recency: number,
    topPercent: number,
    enabled: boolean
): boolean {
    if (!enabled || topPercent <= 0) return false;
    const p = Math.max(1, Math.min(100, topPercent));
    return recency >= 1 - p / 100;
}
