import { describe, expect, it } from "vitest";
import { normalizeKmsGraphWarnings, parseKmsGraphWarning } from "./kmsGraphWarnings";

describe("kmsGraphWarnings", () => {
    it("parses coded warnings and keeps legacy warnings", () => {
        expect(parseKmsGraphWarning("KMS_WARN_SEMANTIC_KNN_PAIR_BUDGET::Semantic kNN stopped.")).toEqual({
            code: "KMS_WARN_SEMANTIC_KNN_PAIR_BUDGET",
            message: "Semantic kNN stopped.",
            raw: "KMS_WARN_SEMANTIC_KNN_PAIR_BUDGET::Semantic kNN stopped.",
        });
        expect(parseKmsGraphWarning("Legacy warning")).toEqual({
            code: null,
            message: "Legacy warning",
            raw: "Legacy warning",
        });
    });

    it("normalizes and de-duplicates warnings by code+message", () => {
        const out = normalizeKmsGraphWarnings([
            "KMS_WARN_LEIDEN_WIKI_ONLY::Leiden used wiki links only.",
            "KMS_WARN_LEIDEN_WIKI_ONLY::Leiden used wiki links only.",
            "Uncoded warning",
            "Uncoded warning",
        ]);
        expect(out).toHaveLength(2);
        expect(out[0].code).toBe("KMS_WARN_LEIDEN_WIKI_ONLY");
        expect(out[1].code).toBeNull();
    });
});

