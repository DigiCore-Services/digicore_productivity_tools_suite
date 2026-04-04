import { describe, it, expect } from "vitest";
import { kmsVaultRelativePath } from "./kmsVaultRelPath";

describe("kmsVaultRelativePath", () => {
    it("strips vault prefix with forward slashes", () => {
        expect(kmsVaultRelativePath("C:/Vault", "C:/Vault/notes/a.md")).toBe("notes/a.md");
    });

    it("normalizes mixed separators", () => {
        expect(kmsVaultRelativePath("C:\\My Vault", "C:\\My Vault\\x\\y.md")).toBe("x/y.md");
    });

    it("returns normalized absolute path when not under vault", () => {
        expect(kmsVaultRelativePath("C:/Other", "C:/Vault/note.md")).toBe("C:/Vault/note.md");
    });
});
