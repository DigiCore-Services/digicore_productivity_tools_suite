import { describe, expect, it } from "vitest";
import { KMS_RECENT_NOTES_MAX, recordRecentNotePath } from "./kmsRecentNotes";

describe("recordRecentNotePath", () => {
    it("moves opened path to front and dedupes", () => {
        expect(recordRecentNotePath(["/b", "/a"], "/a")).toEqual(["/a", "/b"]);
    });

    it("caps length", () => {
        const many = Array.from({ length: KMS_RECENT_NOTES_MAX + 5 }, (_, i) => `/n${i}`);
        const out = recordRecentNotePath(many, "/new");
        expect(out.length).toBe(KMS_RECENT_NOTES_MAX);
        expect(out[0]).toBe("/new");
    });
});
