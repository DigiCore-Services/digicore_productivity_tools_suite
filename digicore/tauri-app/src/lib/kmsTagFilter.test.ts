import { describe, expect, it } from "vitest";
import { noteDtoMatchesTagTokens, parseTagFilterTokens, tagsMatchFilterTokens } from "./kmsTagFilter";

describe("parseTagFilterTokens", () => {
    it("splits on comma and whitespace", () => {
        expect(parseTagFilterTokens("a, b  c")).toEqual(["a", "b", "c"]);
    });
});

describe("tagsMatchFilterTokens", () => {
    it("matches substring on any tag", () => {
        expect(tagsMatchFilterTokens(["Alpha", "Beta"], ["alp"])).toBe(true);
        expect(tagsMatchFilterTokens(["Alpha", "Beta"], ["gamma"])).toBe(false);
    });
});

describe("noteDtoMatchesTagTokens", () => {
    it("uses note.tags", () => {
        expect(
            noteDtoMatchesTagTokens(
                {
                    id: 1,
                    path: "p",
                    title: "t",
                    preview: null,
                    last_modified: null,
                    is_favorite: false,
                    sync_status: "",
                    node_type: "note",
                    folder_path: "",
                    embedding_model_id: null,
                    tags: ["x"],
                },
                ["x"]
            )
        ).toBe(true);
    });
});
