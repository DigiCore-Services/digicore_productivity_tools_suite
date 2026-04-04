/**
 * Client-side substring filter for graph legend / island search (title, path, folder).
 */

export function nodeMatchesGraphFilter(
    query: string,
    fields: { title?: string; path?: string; folder_path?: string }
): boolean {
    const q = query.trim().toLowerCase();
    if (q.length === 0) return true;
    const title = (fields.title ?? "").toLowerCase();
    const path = (fields.path ?? "").toLowerCase();
    const folder = (fields.folder_path ?? "").toLowerCase();
    return title.includes(q) || path.includes(q) || folder.includes(q);
}
