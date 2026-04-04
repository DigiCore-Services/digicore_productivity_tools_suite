/** Legacy localStorage key; used once to migrate into SQLite (`kms_ui_state`). */
const LEGACY_STORAGE_KEY = "kms-recent-note-paths-v1";
export const KMS_RECENT_NOTES_MAX = 25;

/** Read legacy browser-only list for migration after DB-backed sidebar state ships. */
export function readLegacyRecentNotePathsFromLocalStorage(): string[] {
    try {
        const raw = localStorage.getItem(LEGACY_STORAGE_KEY);
        if (!raw) return [];
        const parsed = JSON.parse(raw) as unknown;
        if (!Array.isArray(parsed)) return [];
        return parsed.filter((x): x is string => typeof x === "string" && x.length > 0);
    } catch {
        return [];
    }
}

export function clearLegacyRecentNotePathsFromLocalStorage(): void {
    try {
        localStorage.removeItem(LEGACY_STORAGE_KEY);
    } catch {
        /* ignore */
    }
}

/** Normalize for Map lookup (Windows path casing / slash style). */
export function normalizeKmsNotePathForLookup(p: string): string {
    return p.replace(/\\/g, "/").toLowerCase();
}

/** Most recently opened first; dedupes; caps length. */
export function recordRecentNotePath(paths: string[], openedPath: string): string[] {
    const openedKey = normalizeKmsNotePathForLookup(openedPath);
    const next = [openedPath, ...paths.filter((p) => normalizeKmsNotePathForLookup(p) !== openedKey)];
    return next.slice(0, KMS_RECENT_NOTES_MAX);
}
