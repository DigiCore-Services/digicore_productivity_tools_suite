import { describe, expect, it } from "vitest";
import { resolveMarkdownLinkAgainstNotePath } from "./kmsMarkdownLinkResolve";

describe("resolveMarkdownLinkAgainstNotePath", () => {
    const note = "C:\\vault\\notes\\Note2.md";

    it("resolves ./Note3.md", () => {
        expect(resolveMarkdownLinkAgainstNotePath(note, "./Note3.md")).toBe(
            "C:\\vault\\notes\\Note3.md"
        );
    });

    it("resolves bare filename", () => {
        expect(resolveMarkdownLinkAgainstNotePath(note, "Note3.md")).toBe(
            "C:\\vault\\notes\\Note3.md"
        );
    });

    it("resolves ../sibling folder", () => {
        expect(resolveMarkdownLinkAgainstNotePath(note, "../README.md")).toBe("C:\\vault\\README.md");
    });

    it("returns null for http", () => {
        expect(resolveMarkdownLinkAgainstNotePath(note, "https://example.com")).toBeNull();
    });

    it("returns null for fragment", () => {
        expect(resolveMarkdownLinkAgainstNotePath(note, "#section")).toBeNull();
    });
});
