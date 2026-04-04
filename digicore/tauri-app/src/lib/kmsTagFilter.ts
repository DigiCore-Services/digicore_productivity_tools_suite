import type { KmsNoteDto } from "../bindings";

/** Split user input on commas and whitespace into lowercase tokens. */
export function parseTagFilterTokens(input: string): string[] {
    return input
        .toLowerCase()
        .split(/[\s,]+/)
        .map((s) => s.trim())
        .filter(Boolean);
}

/**
 * True if any token matches any tag (substring, case-insensitive).
 * Empty tokens => true (no restriction).
 */
export function tagsMatchFilterTokens(tags: string[] | undefined | null, tokens: string[]): boolean {
    if (tokens.length === 0) return true;
    if (!tags?.length) return false;
    const lower = tags.map((t) => t.toLowerCase());
    return tokens.some((tok) => lower.some((tag) => tag.includes(tok)));
}

export function noteDtoMatchesTagTokens(note: KmsNoteDto | null | undefined, tokens: string[]): boolean {
    return tagsMatchFilterTokens(note?.tags, tokens);
}
