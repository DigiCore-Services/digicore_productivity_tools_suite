import type { KmsNoteDto, SearchResultDto } from "../bindings";
import { noteDtoMatchesTagTokens, parseTagFilterTokens } from "./kmsTagFilter";

export type KmsSearchNoteScope = "all" | "standard_only" | "skills_only";

export type KmsSearchClientFilters = {
    includeNotes: boolean;
    includeSnippets: boolean;
    includeClipboard: boolean;
    /** Notes under vault skills paths only, or exclude them, or no restriction. */
    noteScope: KmsSearchNoteScope;
    /** When false, drops image modality and common image file extensions on paths. */
    includeImages: boolean;
    /** Case-insensitive substring match on normalized entity_id (path). Empty = no filter. */
    pathPrefix: string;
    /**
     * Indexed note tags (from YAML frontmatter). Comma or whitespace separated tokens;
     * a note passes if any token matches any tag (substring, case-insensitive). Empty = no filter.
     */
    tagsFilter: string;
    /** Inclusive YYYY-MM-DD; null = no bound. Uses indexed note last_modified for entity_type note only. */
    dateFromDay: number | null;
    dateToDay: number | null;
};

export function defaultKmsSearchClientFilters(): KmsSearchClientFilters {
    return {
        includeNotes: true,
        includeSnippets: true,
        includeClipboard: true,
        noteScope: "all",
        includeImages: true,
        pathPrefix: "",
        tagsFilter: "",
        dateFromDay: null,
        dateToDay: null,
    };
}

export function normalizePathForMatch(p: string): string {
    return p.replace(/\\/g, "/").toLowerCase();
}

function isSkillNotePath(entityId: string): boolean {
    return normalizePathForMatch(entityId).includes("/skills/");
}

function looksLikeImagePathOrModality(result: SearchResultDto): boolean {
    if ((result.modality || "").toLowerCase() === "image") return true;
    return /\.(png|jpe?g|gif|webp|bmp|svg)$/i.test(result.entity_id);
}

function noteModifiedDay(note: KmsNoteDto | undefined): number | null {
    if (!note?.last_modified) return null;
    const t = Date.parse(note.last_modified);
    if (Number.isNaN(t)) return null;
    return Math.floor(t / 86400000);
}

function parseIsoDateToDay(iso: string): number | null {
    const t = iso.trim();
    if (!t) return null;
    const d = new Date(`${t}T12:00:00`);
    if (Number.isNaN(d.getTime())) return null;
    return Math.floor(d.getTime() / 86400000);
}

/** Parse `<input type="date" />` value to day index, or null if empty/invalid. */
export function parseInputDateToDay(value: string): number | null {
    return parseIsoDateToDay(value);
}

export function filterSearchResults(
    results: SearchResultDto[],
    filters: KmsSearchClientFilters,
    noteByPath: Map<string, KmsNoteDto>
): SearchResultDto[] {
    const pnorm = normalizePathForMatch(filters.pathPrefix.trim());
    const tagTokens = parseTagFilterTokens(filters.tagsFilter ?? "");

    return results.filter((r) => {
        const t = r.entity_type;
        if (t === "note") {
            if (!filters.includeNotes) return false;
            if (!filters.includeImages && looksLikeImagePathOrModality(r)) return false;
            const skill = isSkillNotePath(r.entity_id);
            if (filters.noteScope === "standard_only" && skill) return false;
            if (filters.noteScope === "skills_only" && !skill) return false;
        } else if (t === "snippet") {
            if (!filters.includeSnippets) return false;
        } else if (t === "clipboard") {
            if (!filters.includeClipboard) return false;
            if (!filters.includeImages && (r.modality || "").toLowerCase() === "image") return false;
        } else {
            if (!filters.includeNotes) return false;
        }

        if (pnorm) {
            const id = normalizePathForMatch(r.entity_id);
            if (!id.includes(pnorm)) return false;
        }

        if (filters.dateFromDay != null || filters.dateToDay != null) {
            if (t !== "note") return true;
            const day = noteModifiedDay(noteByPath.get(r.entity_id));
            if (day == null) return false;
            if (filters.dateFromDay != null && day < filters.dateFromDay) return false;
            if (filters.dateToDay != null && day > filters.dateToDay) return false;
        }

        if (tagTokens.length > 0 && t === "note") {
            if (!noteDtoMatchesTagTokens(noteByPath.get(r.entity_id), tagTokens)) return false;
        }

        return true;
    });
}

export function searchEmbeddingDiagFromResults(
    results: SearchResultDto[]
): { ms: number; modelId: string } | null {
    for (const r of results) {
        if (r.kms_query_embedding_ms != null && r.kms_effective_embedding_model_id) {
            return {
                ms: r.kms_query_embedding_ms,
                modelId: r.kms_effective_embedding_model_id,
            };
        }
    }
    return null;
}
