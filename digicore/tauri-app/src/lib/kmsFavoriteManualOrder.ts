import type { KmsNoteDto } from "../bindings";

const LEGACY_STORAGE_KEY = "kms-favorite-path-order-v1";

export function readLegacyFavoritePathOrderFromLocalStorage(): string[] {
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

export function clearLegacyFavoritePathOrderFromLocalStorage(): void {
    try {
        localStorage.removeItem(LEGACY_STORAGE_KEY);
    } catch {
        /* ignore */
    }
}

/**
 * Favorites first in `order` (still starred), then remaining favorites by title/path.
 */
export function sortFavoriteNotes(notes: KmsNoteDto[], order: string[]): KmsNoteDto[] {
    const favs = notes.filter((n) => n.is_favorite);
    const seen = new Set<string>();
    const ordered: KmsNoteDto[] = [];
    for (const p of order) {
        const n = favs.find((f) => f.path === p);
        if (n) {
            ordered.push(n);
            seen.add(p);
        }
    }
    const rest = favs
        .filter((n) => !seen.has(n.path))
        .sort((a, b) =>
            (a.title || a.path).localeCompare(b.title || b.path, undefined, { sensitivity: "base" })
        );
    return [...ordered, ...rest];
}

/** Drop order entries that are not currently favorite (by path set). */
export function pruneFavoriteOrder(order: string[], favoritePaths: Set<string>): string[] {
    return order.filter((p) => favoritePaths.has(p));
}
