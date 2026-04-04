const LS_VIEW = "kms_graph_session_view_mode";
const LS_OFFSET = "kms_graph_session_paged_offset";
const LS_LIMIT = "kms_graph_session_paged_limit";

export const DEFAULT_PAGE_LIMIT = 500;

/** Shipped defaults; keep in sync with AppState::default (digicore-text-expander) and docs 7.2.2. */
export const KMS_GRAPH_DEFAULT_AUTO_PAGING_ENABLED = true;
export const KMS_GRAPH_DEFAULT_AUTO_PAGING_NOTE_THRESHOLD = 2000;
export const KMS_GRAPH_DEFAULT_WARN_NOTE_THRESHOLD = 1500;

/** Presets for the graph pagination control (PRD Epic A3). */
export const PAGE_SIZE_PRESETS = [200, 500, 1000, 2000] as const;

export function clampPageLimit(n: number): number {
    if (!Number.isFinite(n) || n < 1) return DEFAULT_PAGE_LIMIT;
    return Math.min(50_000, Math.floor(n));
}

/** Presets plus current session limit if it is not a preset (for HTML select value matching). */
export function pageSizeSelectOptions(currentLimit: number): number[] {
    const cur = clampPageLimit(currentLimit);
    const set = new Set<number>([...PAGE_SIZE_PRESETS, cur]);
    return Array.from(set).sort((a, b) => a - b);
}

export type GraphSessionView = "full" | "paged";

export function readGraphSession(): {
    viewMode: GraphSessionView | null;
    offset: number;
    limit: number;
} {
    if (typeof localStorage === "undefined") {
        return { viewMode: null, offset: 0, limit: DEFAULT_PAGE_LIMIT };
    }
    const raw = localStorage.getItem(LS_VIEW);
    const viewMode =
        raw === "full" || raw === "paged" ? (raw as GraphSessionView) : null;
    const offset = Math.max(0, parseInt(localStorage.getItem(LS_OFFSET) || "0", 10) || 0);
    const limit = Math.max(
        1,
        parseInt(localStorage.getItem(LS_LIMIT) || String(DEFAULT_PAGE_LIMIT), 10) ||
            DEFAULT_PAGE_LIMIT
    );
    return { viewMode, offset, limit };
}

export function writeGraphSession(
    viewMode: GraphSessionView,
    offset: number,
    limit: number
): void {
    if (typeof localStorage === "undefined") return;
    localStorage.setItem(LS_VIEW, viewMode);
    localStorage.setItem(LS_OFFSET, String(offset));
    localStorage.setItem(LS_LIMIT, String(limit));
}

export function shouldUsePagedGraph(
    autoEnabled: boolean,
    threshold: number,
    noteCount: number,
    sessionView: GraphSessionView | null
): boolean {
    if (sessionView === "full") return false;
    if (sessionView === "paged") return true;
    return autoEnabled && noteCount >= threshold;
}
