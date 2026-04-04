/** Color nodes by note type (skill/image) or by parent folder path. */
export type GraphColorMode = "type" | "folder";

/**
 * Deterministic folder -> color mapping for KMS graph views.
 * Sorted unique folder keys get palette slots (stable across renders for the same graph).
 */

export const FOLDER_PALETTE = [
    "#0ea5e9",
    "#f59e0b",
    "#10b981",
    "#ec4899",
    "#8b5cf6",
    "#14b8a6",
    "#f97316",
    "#22c55e",
    "#eab308",
    "#6366f1",
    "#d946ef",
    "#06b6d4",
    "#84cc16",
    "#ef4444",
    "#64748b",
] as const;

const ROOT_KEY = "(root)";

export function normalizeFolderKey(folderPath: string): string {
    const t = folderPath.trim();
    return t.length === 0 ? ROOT_KEY : t.replace(/\\/g, "/");
}

export function buildFolderColorMap(folderPaths: Iterable<string>): Map<string, string> {
    const unique = Array.from(
        new Set(Array.from(folderPaths, (p) => normalizeFolderKey(p ?? "")))
    ).sort((a, b) => a.localeCompare(b));
    const m = new Map<string, string>();
    unique.forEach((fp, i) => {
        m.set(fp, FOLDER_PALETTE[i % FOLDER_PALETTE.length]);
    });
    return m;
}

export function colorForFolderKey(folderPath: string, map: Map<string, string>): string {
    const k = normalizeFolderKey(folderPath ?? "");
    return map.get(k) ?? FOLDER_PALETTE[0];
}

/** Short label for legend rows (vault-relative folder path). */
export function folderLegendLabel(folderKey: string): string {
    if (folderKey === ROOT_KEY) return "Vault root";
    const parts = folderKey.split("/").filter(Boolean);
    const last = parts[parts.length - 1];
    return last || folderKey;
}
