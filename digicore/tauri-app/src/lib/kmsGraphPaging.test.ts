import { beforeEach, describe, expect, it } from "vitest";
import {
    clampPageLimit,
    DEFAULT_PAGE_LIMIT,
    pageSizeSelectOptions,
    readGraphSession,
    shouldUsePagedGraph,
    writeGraphSession,
} from "./kmsGraphPaging";

describe("kmsGraphPaging", () => {
    beforeEach(() => {
        localStorage.clear();
    });

    describe("clampPageLimit", () => {
        it("falls back to default for invalid limits", () => {
            expect(clampPageLimit(Number.NaN)).toBe(DEFAULT_PAGE_LIMIT);
            expect(clampPageLimit(0)).toBe(DEFAULT_PAGE_LIMIT);
            expect(clampPageLimit(-10)).toBe(DEFAULT_PAGE_LIMIT);
        });

        it("caps very large values and floors decimals", () => {
            expect(clampPageLimit(123.9)).toBe(123);
            expect(clampPageLimit(500_001)).toBe(50_000);
        });
    });

    describe("pageSizeSelectOptions", () => {
        it("includes presets and current non-preset limit", () => {
            const options = pageSizeSelectOptions(750);
            expect(options).toContain(200);
            expect(options).toContain(500);
            expect(options).toContain(1000);
            expect(options).toContain(2000);
            expect(options).toContain(750);
        });
    });

    describe("read/writeGraphSession", () => {
        it("returns defaults when storage is empty", () => {
            expect(readGraphSession()).toEqual({
                viewMode: null,
                offset: 0,
                limit: DEFAULT_PAGE_LIMIT,
            });
        });

        it("round-trips stored session values", () => {
            writeGraphSession("paged", 300, 1000);
            expect(readGraphSession()).toEqual({
                viewMode: "paged",
                offset: 300,
                limit: 1000,
            });
        });

        it("defensively clamps malformed storage values", () => {
            localStorage.setItem("kms_graph_session_view_mode", "bad");
            localStorage.setItem("kms_graph_session_paged_offset", "-42");
            localStorage.setItem("kms_graph_session_paged_limit", "0");
            expect(readGraphSession()).toEqual({
                viewMode: null,
                offset: 0,
                limit: DEFAULT_PAGE_LIMIT,
            });
        });
    });

    describe("shouldUsePagedGraph", () => {
        it("respects explicit session preference", () => {
            expect(shouldUsePagedGraph(true, 100, 1000, "full")).toBe(false);
            expect(shouldUsePagedGraph(false, 100, 10, "paged")).toBe(true);
        });

        it("uses auto mode when session preference is unset", () => {
            expect(shouldUsePagedGraph(true, 500, 1000, null)).toBe(true);
            expect(shouldUsePagedGraph(true, 500, 100, null)).toBe(false);
            expect(shouldUsePagedGraph(false, 500, 1000, null)).toBe(false);
        });
    });
});

