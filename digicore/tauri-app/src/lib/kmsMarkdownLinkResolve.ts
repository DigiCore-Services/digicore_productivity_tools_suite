/** Turn file:/// URLs from the DOM into a Windows path when possible. */
function fileUrlToWindowsPath(href: string): string | null {
    const raw = href.trim();
    if (!raw.toLowerCase().startsWith("file:")) return null;
    try {
        const u = new URL(raw);
        let p = u.pathname;
        if (p.startsWith("/") && /^\/[a-zA-Z]:/.test(p)) {
            p = p.slice(1);
        }
        return decodeURIComponent(p.replace(/\//g, "\\"));
    } catch {
        return null;
    }
}

/**
 * Resolve a markdown link href relative to the current note's directory (Windows-friendly).
 * Returns null for http(s), mailto, fragment-only, or empty href.
 */
export function resolveMarkdownLinkAgainstNotePath(
    currentNotePath: string,
    href: string
): string | null {
    const raw = href.trim().split(/[?#]/)[0];
    if (!raw) return null;
    if (/^https?:\/\//i.test(raw) || raw.startsWith("mailto:")) return null;
    if (raw.startsWith("#")) return null;

    const fromFile = fileUrlToWindowsPath(raw);
    if (fromFile && /^[a-zA-Z]:\\/.test(fromFile)) {
        return fromFile;
    }

    if (/^[a-zA-Z]:[\\/]/.test(raw)) {
        return raw.replace(/\//g, "\\");
    }

    const lastSlash = Math.max(currentNotePath.lastIndexOf("\\"), currentNotePath.lastIndexOf("/"));
    const dir = lastSlash >= 0 ? currentNotePath.slice(0, lastSlash) : currentNotePath;

    const rel = raw.replace(/\//g, "\\");
    const relSegments = rel.split(/\\/).filter((s) => s.length > 0);
    const dirSegments = dir.split(/\\/).filter((s) => s.length > 0);

    for (const seg of relSegments) {
        if (seg === "..") {
            if (dirSegments.length > 0) dirSegments.pop();
        } else if (seg !== ".") {
            dirSegments.push(seg);
        }
    }
    return dirSegments.join("\\");
}
