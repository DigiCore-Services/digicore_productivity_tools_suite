import { describe, expect, it } from "vitest";
import type { KmsNoteDto, SearchResultDto } from "../bindings";
import { filterSearchResults, defaultKmsSearchClientFilters, parseInputDateToDay } from "./kmsSearchResultFilter";

function noteDto(path: string, lastModified: string | null, tags: string[] = []): KmsNoteDto {
    return {
        id: 1,
        path,
        title: "t",
        preview: null,
        last_modified: lastModified,
        is_favorite: false,
        sync_status: "ok",
        node_type: "note",
        folder_path: "",
        embedding_model_id: null,
        tags,
    };
}

function result(entity_type: string, entity_id: string, modality = "text"): SearchResultDto {
    return {
        entity_type,
        entity_id,
        distance: 0.5,
        modality,
        metadata: null,
        snippet: null,
        kms_query_embedding_ms: null,
        kms_effective_embedding_model_id: null,
    };
}

describe("filterSearchResults", () => {
    it("filters by entity type toggles", () => {
        const rows = [
            result("note", "C:/v/a.md"),
            result("snippet", "s1"),
            result("clipboard", "9", "text"),
        ];
        const f = defaultKmsSearchClientFilters();
        f.includeSnippets = false;
        expect(filterSearchResults(rows, f, new Map()).map((r) => r.entity_type)).toEqual(["note", "clipboard"]);
    });

    it("filters by path prefix", () => {
        const rows = [result("note", "C:/v/notes/x.md"), result("note", "C:/v/other/y.md")];
        const f = defaultKmsSearchClientFilters();
        f.pathPrefix = "notes";
        const m = new Map<string, KmsNoteDto>();
        expect(filterSearchResults(rows, f, m).length).toBe(1);
    });

    it("noteScope skills_only", () => {
        const rows = [result("note", "C:/v/notes/a.md"), result("note", "C:/v/skills/k/SKILL.md")];
        const f = defaultKmsSearchClientFilters();
        f.noteScope = "skills_only";
        expect(filterSearchResults(rows, f, new Map()).length).toBe(1);
    });

    it("date range uses note map", () => {
        const path = "C:/v/d.md";
        const rows = [result("note", path)];
        const f = defaultKmsSearchClientFilters();
        f.dateFromDay = parseInputDateToDay("2026-01-10")!;
        f.dateToDay = parseInputDateToDay("2026-01-20")!;
        const m = new Map([[path, noteDto(path, "2026-01-15T10:00:00Z")]]);
        expect(filterSearchResults(rows, f, m).length).toBe(1);
    });

    it("excludes note without mtime when date filter set", () => {
        const path = "C:/v/d.md";
        const rows = [result("note", path)];
        const f = defaultKmsSearchClientFilters();
        f.dateFromDay = parseInputDateToDay("2026-01-01")!;
        const m = new Map([[path, noteDto(path, null)]]);
        expect(filterSearchResults(rows, f, m).length).toBe(0);
    });

    it("filters notes by indexed tags", () => {
        const a = "C:/v/a.md";
        const b = "C:/v/b.md";
        const rows = [result("note", a), result("note", b)];
        const f = defaultKmsSearchClientFilters();
        f.tagsFilter = "alpha";
        const m = new Map<string, KmsNoteDto>([
            [a, noteDto(a, null, ["alpha", "beta"])],
            [b, noteDto(b, null, ["gamma"])],
        ]);
        expect(filterSearchResults(rows, f, m).map((r) => r.entity_id)).toEqual([a]);
    });
});
