import type { KmsFileSystemItemDto } from "../bindings";
import { noteDtoMatchesTagTokens, parseTagFilterTokens } from "./kmsTagFilter";

function norm(s: string): string {
    return s.trim().toLowerCase();
}

function matchesPathQuery(query: string, name: string, relPath: string): boolean {
    const q = norm(query);
    if (!q) return true;
    const n = name.toLowerCase();
    const r = relPath.toLowerCase();
    return n.includes(q) || r.includes(q);
}

function matchesTagQuery(tagQuery: string, item: KmsFileSystemItemDto): boolean {
    const tokens = parseTagFilterTokens(tagQuery);
    if (tokens.length === 0) return true;
    return noteDtoMatchesTagTokens(item.note ?? undefined, tokens);
}

/**
 * Client-side vault tree filter: keep branches that match the query on name or rel_path.
 * If a directory name matches, its full subtree is kept (unfiltered) so users can browse under it
 * (unless a tag filter is active; then children are filtered by tags).
 * Optional `tagQuery` filters file rows using indexed note tags (`item.note.tags`).
 */
export function filterVaultStructure(
    root: KmsFileSystemItemDto | null,
    query: string,
    tagQuery = ""
): KmsFileSystemItemDto | null {
    if (!root) return null;
    const q = query.trim();
    const tagActive = norm(tagQuery).length > 0;
    if (!q && !tagActive) return root;

    function walk(item: KmsFileSystemItemDto): KmsFileSystemItemDto | null {
        if (item.item_type === "directory") {
            const dirPathMatch = matchesPathQuery(q, item.name, item.rel_path);
            if (dirPathMatch && !tagActive) {
                return { ...item, children: item.children };
            }
            const rawKids = item.children ?? [];
            const kids = rawKids.map(walk).filter((x): x is KmsFileSystemItemDto => x != null);
            if (kids.length > 0) {
                return { ...item, children: kids };
            }
            return null;
        }
        if (matchesPathQuery(q, item.name, item.rel_path) && matchesTagQuery(tagQuery, item)) {
            return item;
        }
        return null;
    }

    const rawChildren = root.children ?? [];
    const children = rawChildren.map(walk).filter((x): x is KmsFileSystemItemDto => x != null);
    if (children.length === 0) return null;
    return { ...root, children };
}
