import { describe, expect, it } from "vitest";
import { formatIpcOrRaw, KMS_IPC_CODES, parseIpcErrorFromUnknown, tryParseIpcError } from "./ipcError";

describe("ipcError parsing", () => {
    it("parses a valid structured IPC JSON error", () => {
        const raw = JSON.stringify({
            code: KMS_IPC_CODES.GRAPH_BUILD,
            message: "Graph build failed",
            details: "timeout",
        });
        expect(tryParseIpcError(raw)).toEqual({
            code: KMS_IPC_CODES.GRAPH_BUILD,
            message: "Graph build failed",
            details: "timeout",
        });
    });

    it("returns null for non-JSON and malformed payloads", () => {
        expect(tryParseIpcError("plain error")).toBeNull();
        expect(tryParseIpcError("{bad-json")).toBeNull();
        expect(tryParseIpcError(JSON.stringify({ message: "missing code" }))).toBeNull();
    });

    it("parses from unknown Error values", () => {
        const err = new Error(
            JSON.stringify({
                code: KMS_IPC_CODES.REPO_NOTE,
                message: "Note not found",
            })
        );
        expect(parseIpcErrorFromUnknown(err)?.code).toBe(KMS_IPC_CODES.REPO_NOTE);
    });
});

describe("formatIpcOrRaw", () => {
    it("includes code, details, and hint for known structured codes", () => {
        const text = formatIpcOrRaw(
            new Error(
                JSON.stringify({
                    code: KMS_IPC_CODES.VAULT_OVERRIDES_JSON,
                    message: "Invalid JSON",
                    details: "line 2",
                })
            )
        );

        expect(text).toContain("Invalid JSON");
        expect(text).toContain(KMS_IPC_CODES.VAULT_OVERRIDES_JSON);
        expect(text).toContain("line 2");
        expect(text).toContain("Fix invalid JSON");
    });

    it("falls back to raw text for unknown formats", () => {
        expect(formatIpcOrRaw("simple failure")).toBe("simple failure");
    });
});

