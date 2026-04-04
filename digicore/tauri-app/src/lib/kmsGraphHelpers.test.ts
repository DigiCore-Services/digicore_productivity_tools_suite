import { describe, it, expect } from "vitest";
import {
    undirectedEdgeKey,
    pathEdgeSetFromDto,
    pathNodeSetFromDto,
    linkOnPathSet,
    linkKeysFromGraphLinks,
    visiblePathEdgeCount,
} from "./kmsGraphHelpers";

describe("undirectedEdgeKey", () => {
    it("orders endpoints so A-B equals B-A", () => {
        expect(undirectedEdgeKey("a", "b")).toBe(undirectedEdgeKey("b", "a"));
    });

    it("is stable for identical endpoints", () => {
        expect(undirectedEdgeKey("x", "x")).toBe("x|x");
    });
});

describe("pathEdgeSetFromDto", () => {
    it("returns empty set when not found or no edges", () => {
        expect(pathEdgeSetFromDto(false, [{ source: "a", target: "b" }])).toEqual(new Set());
        expect(pathEdgeSetFromDto(true, null)).toEqual(new Set());
        expect(pathEdgeSetFromDto(true, [])).toEqual(new Set());
    });

    it("adds undirected keys for each edge", () => {
        const set = pathEdgeSetFromDto(true, [
            { source: "n1", target: "n2" },
            { source: "n2", target: "n3" },
        ]);
        expect(set.has(undirectedEdgeKey("n1", "n2"))).toBe(true);
        expect(set.has(undirectedEdgeKey("n2", "n3"))).toBe(true);
        expect(set.size).toBe(2);
    });
});

describe("pathNodeSetFromDto", () => {
    it("returns empty when not found or empty paths", () => {
        expect(pathNodeSetFromDto(false, ["/a"])).toEqual(new Set());
        expect(pathNodeSetFromDto(true, [])).toEqual(new Set());
        expect(pathNodeSetFromDto(true, null)).toEqual(new Set());
    });

    it("collects unique paths", () => {
        const set = pathNodeSetFromDto(true, ["/a", "/b", "/a"]);
        expect(set.size).toBe(2);
        expect(set.has("/a")).toBe(true);
        expect(set.has("/b")).toBe(true);
    });
});

describe("linkOnPathSet", () => {
    const keys = pathEdgeSetFromDto(true, [{ source: "u", target: "v" }]);

    it("matches string endpoints", () => {
        expect(linkOnPathSet({ source: "u", target: "v" }, keys)).toBe(true);
        expect(linkOnPathSet({ source: "v", target: "u" }, keys)).toBe(true);
    });

    it("matches object endpoints with id", () => {
        expect(
            linkOnPathSet({ source: { id: "u" }, target: { id: "v" } }, keys)
        ).toBe(true);
    });

    it("returns false when edge not in set", () => {
        expect(linkOnPathSet({ source: "a", target: "z" }, keys)).toBe(false);
    });
});

describe("linkKeysFromGraphLinks", () => {
    it("collects keys from string endpoints", () => {
        const s = linkKeysFromGraphLinks([
            { source: "a", target: "b" },
            { source: "b", target: "c" },
        ]);
        expect(s.size).toBe(2);
        expect(s.has(undirectedEdgeKey("a", "b"))).toBe(true);
    });

    it("supports object endpoints", () => {
        const s = linkKeysFromGraphLinks([{ source: { id: "x" }, target: { id: "y" } }]);
        expect(s.has(undirectedEdgeKey("x", "y"))).toBe(true);
    });
});

describe("visiblePathEdgeCount", () => {
    it("counts overlap with local keys", () => {
        const local = new Set([undirectedEdgeKey("a", "b"), undirectedEdgeKey("b", "c")]);
        const r = visiblePathEdgeCount(
            [
                { source: "a", target: "b" },
                { source: "b", target: "c" },
                { source: "c", target: "z" },
            ],
            local
        );
        expect(r.total).toBe(3);
        expect(r.visible).toBe(2);
    });
});
