import { describe, expect, it } from "vitest";
import { pruneFavoriteOrder, sortFavoriteNotes } from "./kmsFavoriteManualOrder";
import type { KmsNoteDto } from "../bindings";

function note(path: string, title: string, fav: boolean): KmsNoteDto {
    return {
        id: 1,
        path,
        title,
        preview: null,
        last_modified: null,
        is_favorite: fav,
        sync_status: "ok",
        node_type: "note",
        folder_path: "",
        embedding_model_id: null,
        tags: [],
    };
}

describe("sortFavoriteNotes", () => {
    it("orders by saved path list then title for the rest", () => {
        const notes = [
            note("/a", "A", true),
            note("/b", "B", true),
            note("/c", "C", true),
            note("/d", "D", false),
        ];
        const out = sortFavoriteNotes(notes, ["/c", "/a"]);
        expect(out.map((n) => n.path)).toEqual(["/c", "/a", "/b"]);
    });

    it("ignores order entries that are not favorite", () => {
        const notes = [note("/a", "A", true)];
        expect(sortFavoriteNotes(notes, ["/missing", "/a"]).map((n) => n.path)).toEqual(["/a"]);
    });
});

describe("pruneFavoriteOrder", () => {
    it("drops paths not in favorite set", () => {
        expect(pruneFavoriteOrder(["/a", "/b"], new Set(["/a"]))).toEqual(["/a"]);
    });
});
