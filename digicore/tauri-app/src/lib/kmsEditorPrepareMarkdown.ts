import { convertFileSrc } from "@tauri-apps/api/core";

function wrapExceptCodeBlocks(
    text: string,
    regex: RegExp,
    replacement: string | ((substring: string, ...args: unknown[]) => string)
): string {
    if (!text) return text;
    const parts = text.split(/(```[\s\S]*?```|~~~[\s\S]*?~~~)/g);
    return parts
        .map((part) => {
            if (part.startsWith("```") || part.startsWith("~~~")) return part;
            return part.replace(regex, replacement as (substring: string, ...args: unknown[]) => string);
        })
        .join("");
}

function wrapWikiLinks(md: string): string {
    return wrapExceptCodeBlocks(md, /\[\[([^\]]+)\]\]/g, (_match, target) => {
        return `<span data-type="wiki-link" data-target="${target}">[[${target}]]</span>`;
    });
}

function wrapImages(md: string, vaultPath: string | null | undefined): string {
    if (!md) return md;
    return wrapExceptCodeBlocks(md, /!\[([^\]]*)\]\(([^)]+)\)/g, (match, ...args: unknown[]) => {
        const alt = String(args[0] ?? "");
        const src = String(args[1] ?? "");
        if (src.startsWith("http") || src.startsWith("data:")) {
            return match;
        }

        if (vaultPath) {
            let fullPath = src;
            if (!src.includes(":") && !src.startsWith("/") && !src.startsWith("\\")) {
                fullPath = vaultPath + (vaultPath.endsWith("/") || vaultPath.endsWith("\\") ? "" : "/") + src;
            } else if (src.startsWith("/") || src.startsWith("\\")) {
                fullPath = vaultPath + src;
            }

            try {
                const assetUrl = convertFileSrc(fullPath);
                return `<img src="${assetUrl}" alt="${alt}" data-path="${src}" data-type="vault-image" />`;
            } catch (e) {
                console.error("Failed to convert image path:", e);
                return match;
            }
        }
        return match;
    });
}

function wrapFrontmatter(md: string): string {
    if (!md) return md;
    const trimmed = md.trim();
    if (trimmed.startsWith("---")) {
        const parts = trimmed.split("---");
        if (parts.length >= 3) {
            const frontmatter = parts[1].trim();
            const remainder = parts.slice(2).join("---").trim();
            return `<pre data-type="frontmatter">${frontmatter}</pre>\n\n${remainder}`;
        }
    }
    return md;
}

function wrapMath(md: string): string {
    let processed = wrapExceptCodeBlocks(md, /\$\$([\s\S]+?)\$\$/g, '<div data-type="math" data-display="true">$1</div>');
    processed = wrapExceptCodeBlocks(processed, /\$([^$]+?)\$/g, '<span data-type="math" data-display="false">$1</span>');
    return processed;
}

function wrapAdmonitions(md: string): string {
    return wrapExceptCodeBlocks(md, /:::(\w+)\n?([\s\S]+?)\n?:::/g, '<div data-type="admonition" data-admonition-type="$1">$2</div>');
}

/**
 * Pre-process markdown before loading into Tiptap (main editor and read-only reference pane).
 */
export function prepareMarkdownForTiptap(md: string, vaultPath: string | null | undefined): string {
    let processed = wrapFrontmatter(md);
    processed = wrapMath(processed);
    processed = wrapAdmonitions(processed);
    processed = wrapWikiLinks(processed);
    processed = wrapImages(processed, vaultPath);
    return processed;
}
