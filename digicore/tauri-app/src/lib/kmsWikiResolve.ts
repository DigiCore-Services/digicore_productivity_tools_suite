import type { KmsNoteDto } from "../bindings";
import { normalizeKmsNotePathForLookup } from "./kmsRecentNotes";

/**
 * Resolve a [[wiki link]] target or path string to a note in the vault list.
 */
export function resolveNoteFromWikiTarget(notes: KmsNoteDto[], target: string): KmsNoteDto | null {
    const t = target.trim();
    if (!t) return null;

    let hit = notes.find((n) => n.path === t);
    if (hit) return hit;

    const normTarget = normalizeKmsNotePathForLookup(t);
    hit = notes.find((n) => normalizeKmsNotePathForLookup(n.path) === normTarget);
    if (hit) return hit;

    hit = notes.find((n) => normalizeKmsNotePathForLookup(n.path).endsWith(normTarget));
    if (hit) return hit;

    const titleKey = normTarget.replace(/\.md$/i, "");
    hit = notes.find((n) => {
        const title = (n.title || "").trim().toLowerCase();
        const base = n.path
            .split(/[\\/]/)
            .pop()
            ?.replace(/\.md$/i, "")
            .toLowerCase() ?? "";
        return title === titleKey || base === titleKey;
    });
    return hit ?? null;
}
