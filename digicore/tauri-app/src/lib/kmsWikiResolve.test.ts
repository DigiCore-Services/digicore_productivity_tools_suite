import { describe, expect, it } from "vitest";
import type { KmsNoteDto } from "../bindings";
import { resolveNoteFromWikiTarget } from "./kmsWikiResolve";

describe("resolveNoteFromWikiTarget", () => {
    const base: Omit<KmsNoteDto, "path" | "title"> = {
        id: 1,
        preview: null,
        last_modified: null,
        is_favorite: false,
        sync_status: "",
        node_type: "note",
        folder_path: "",
        embedding_model_id: null,
        tags: [],
    };
    const notes: KmsNoteDto[] = [
        { ...base, id: 1, path: "C:\\vault\\notes\\Alpha.md", title: "Alpha" },
        { ...base, id: 2, path: "C:\\vault\\notes\\sub\\Beta.md", title: "Beta" },
    ];

    it("matches exact path", () => {
        expect(resolveNoteFromWikiTarget(notes, "C:\\vault\\notes\\Alpha.md")?.title).toBe("Alpha");
    });

    it("matches title / filename without extension", () => {
        expect(resolveNoteFromWikiTarget(notes, "Beta")?.path).toContain("Beta.md");
        expect(resolveNoteFromWikiTarget(notes, "Alpha")?.title).toBe("Alpha");
    });

    it("returns null when nothing matches", () => {
        expect(resolveNoteFromWikiTarget(notes, "NoSuchNote")).toBeNull();
    });
});
