import type { GraphColorMode } from "./kmsGraphFolderPalette";

export type { GraphColorMode };

const LS_COLOR_MODE = "kms_graph_session_color_mode";
const LS_WIKI = "kms_graph_legend_show_wiki_edges";
const LS_AI = "kms_graph_legend_show_ai_edges";
const LS_KNN = "kms_graph_legend_show_semantic_knn_edges";
const LS_SEM = "kms_graph_legend_show_semantic_edges";
const LS_PANEL_TYPES = "kms_graph_legend_panel_show_types";
const LS_PANEL_FOLDERS = "kms_graph_legend_panel_show_folders";
const LS_PANEL_EDGES = "kms_graph_legend_panel_show_edge_toggles";
const LS_PULSE = "kms_graph_pulse_enabled";
const LS_PULSE_PCT = "kms_graph_pulse_top_percent";
const LS_FILTER_Q = "kms_graph_legend_filter_query";
const LS_HIDDEN_FOLDERS = "kms_graph_session_hidden_folder_keys_json";
const LS_HIDDEN_TYPES = "kms_graph_session_hidden_node_types_json";
/** When true, the left graph tools dock (legend, filters, shortest path) is slid off-canvas for a clearer view. */
const LS_PANELS_COLLAPSED = "kms_graph_ui_panels_collapsed";

function readBool(key: string, defaultVal: boolean): boolean {
    if (typeof localStorage === "undefined") return defaultVal;
    const v = localStorage.getItem(key);
    if (v === null) return defaultVal;
    return v === "1" || v === "true";
}

function writeBool(key: string, v: boolean): void {
    if (typeof localStorage === "undefined") return;
    localStorage.setItem(key, v ? "1" : "0");
}

export function readGraphColorMode(): GraphColorMode {
    if (typeof localStorage === "undefined") return "type";
    const v = localStorage.getItem(LS_COLOR_MODE);
    return v === "folder" ? "folder" : "type";
}

export function writeGraphColorMode(m: GraphColorMode): void {
    if (typeof localStorage === "undefined") return;
    localStorage.setItem(LS_COLOR_MODE, m);
}

export function readShowWikiEdges(): boolean {
    return readBool(LS_WIKI, true);
}

export function writeShowWikiEdges(v: boolean): void {
    writeBool(LS_WIKI, v);
}

export function readShowAiBeamEdges(): boolean {
    return readBool(LS_AI, true);
}

export function writeShowAiBeamEdges(v: boolean): void {
    writeBool(LS_AI, v);
}

export function readShowSemanticKnnEdges(): boolean {
    return readBool(LS_KNN, true);
}

export function writeShowSemanticKnnEdges(v: boolean): void {
    writeBool(LS_KNN, v);
}

export function readShowSemanticEdges(): boolean {
    return readBool(LS_SEM, true);
}

export function writeShowSemanticEdges(v: boolean): void {
    writeBool(LS_SEM, v);
}

export function readLegendPanelTypes(): boolean {
    return readBool(LS_PANEL_TYPES, true);
}

export function writeLegendPanelTypes(v: boolean): void {
    writeBool(LS_PANEL_TYPES, v);
}

export function readLegendPanelFolders(): boolean {
    return readBool(LS_PANEL_FOLDERS, true);
}

export function writeLegendPanelFolders(v: boolean): void {
    writeBool(LS_PANEL_FOLDERS, v);
}

export function readLegendPanelEdgeToggles(): boolean {
    return readBool(LS_PANEL_EDGES, true);
}

export function writeLegendPanelEdgeToggles(v: boolean): void {
    writeBool(LS_PANEL_EDGES, v);
}

export function readPulseEnabled(): boolean {
    return readBool(LS_PULSE, true);
}

export function writePulseEnabled(v: boolean): void {
    writeBool(LS_PULSE, v);
}

/** Top percent of the graph date range treated as "recent" (5-50, default 20). */
export function readPulseTopPercent(): number {
    if (typeof localStorage === "undefined") return 20;
    const raw = localStorage.getItem(LS_PULSE_PCT);
    const n = raw != null ? parseInt(raw, 10) : 20;
    if (!Number.isFinite(n)) return 20;
    return Math.max(5, Math.min(50, n));
}

export function writePulseTopPercent(n: number): void {
    if (typeof localStorage === "undefined") return;
    localStorage.setItem(LS_PULSE_PCT, String(Math.max(5, Math.min(50, Math.floor(n)))));
}

export function readLegendFilterQuery(): string {
    if (typeof localStorage === "undefined") return "";
    return localStorage.getItem(LS_FILTER_Q) ?? "";
}

export function writeLegendFilterQuery(q: string): void {
    if (typeof localStorage === "undefined") return;
    localStorage.setItem(LS_FILTER_Q, q);
}

function readStringSetJson(key: string): Set<string> {
    if (typeof localStorage === "undefined") return new Set();
    const raw = localStorage.getItem(key);
    if (raw == null || raw === "") return new Set();
    try {
        const arr = JSON.parse(raw) as unknown;
        if (!Array.isArray(arr)) return new Set();
        return new Set(arr.filter((x): x is string => typeof x === "string"));
    } catch {
        return new Set();
    }
}

function writeStringSetJson(key: string, s: Set<string>): void {
    if (typeof localStorage === "undefined") return;
    localStorage.setItem(key, JSON.stringify(Array.from(s).sort((a, b) => a.localeCompare(b))));
}

/** Folder keys (normalized) hidden via legend when color mode is folder. */
export function readHiddenFolderKeys(): Set<string> {
    return readStringSetJson(LS_HIDDEN_FOLDERS);
}

export function writeHiddenFolderKeys(s: Set<string>): void {
    writeStringSetJson(LS_HIDDEN_FOLDERS, s);
}

/** Lowercase node_type values hidden via legend when color mode is type. */
export function readHiddenNodeTypes(): Set<string> {
    return readStringSetJson(LS_HIDDEN_TYPES);
}

export function writeHiddenNodeTypes(s: Set<string>): void {
    writeStringSetJson(LS_HIDDEN_TYPES, s);
}

/** Clear legend visibility overrides (folders + types). */
export function resetLegendVisibilityFilters(): void {
    writeHiddenFolderKeys(new Set());
    writeHiddenNodeTypes(new Set());
}

export function readGraphPanelsCollapsed(): boolean {
    return readBool(LS_PANELS_COLLAPSED, false);
}

export function writeGraphPanelsCollapsed(v: boolean): void {
    writeBool(LS_PANELS_COLLAPSED, v);
}

