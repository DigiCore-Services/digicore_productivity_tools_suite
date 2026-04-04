import { describe, expect, it } from "vitest";
import { filterVaultStructure } from "./kmsVaultTreeFilter";
import type { KmsFileSystemItemDto, KmsNoteDto } from "../bindings";

function minimalNote(path: string, tags: string[]): KmsNoteDto {
    return {
        id: 1,
        path,
        title: "t",
        preview: null,
        last_modified: null,
        is_favorite: false,
        sync_status: "ok",
        node_type: "note",
        folder_path: "",
        embedding_model_id: null,
        tags,
    };
}

function fileTagged(name: string, rel: string, tags: string[]): KmsFileSystemItemDto {
    const path = `/v/${rel}`;
    return {
        name,
        path,
        rel_path: rel,
        item_type: "file",
        children: null,
        note: minimalNote(path, tags),
    };
}

function file(name: string, rel: string): KmsFileSystemItemDto {
    return {
        name,
        path: `/v/${rel}`,
        rel_path: rel,
        item_type: "file",
        children: null,
        note: null,
    };
}

function dir(name: string, rel: string, children: KmsFileSystemItemDto[]): KmsFileSystemItemDto {
    return {
        name,
        path: `/v/${rel}`,
        rel_path: rel,
        item_type: "directory",
        children,
        note: null,
    };
}

describe("filterVaultStructure", () => {
    it("returns root unchanged for blank query", () => {
        const root = dir("vault", "", [file("a.md", "notes/a.md")]);
        expect(filterVaultStructure(root, "")).toEqual(root);
        expect(filterVaultStructure(root, "   ")).toEqual(root);
    });

    it("keeps only matching files and prunes empty dirs", () => {
        const root = dir("vault", "", [
            dir("notes", "notes", [file("foo.md", "notes/foo.md"), file("bar.md", "notes/bar.md")]),
        ]);
        const out = filterVaultStructure(root, "bar");
        expect(out?.children?.length).toBe(1);
        expect(out?.children?.[0].item_type).toBe("directory");
        expect(out?.children?.[0].children?.length).toBe(1);
        expect(out?.children?.[0].children?.[0].name).toBe("bar.md");
    });

    it("matches rel_path segments", () => {
        const root = dir("vault", "", [file("x.md", "deep/nested/x.md")]);
        const out = filterVaultStructure(root, "nested");
        expect(out?.children?.length).toBe(1);
    });

    it("returns null when nothing matches", () => {
        const root = dir("vault", "", [file("a.md", "a.md")]);
        expect(filterVaultStructure(root, "zzz")).toBeNull();
    });

    it("keeps full subtree when directory name matches", () => {
        const root = dir("vault", "", [
            dir("meetings", "meetings", [file("a.md", "meetings/a.md"), file("b.md", "meetings/b.md")]),
        ]);
        const out = filterVaultStructure(root, "meet");
        expect(out?.children?.[0].children?.length).toBe(2);
    });

    it("filters files by indexed tags on note rows", () => {
        const root = dir("vault", "", [
            fileTagged("a.md", "notes/a.md", ["work"]),
            fileTagged("b.md", "notes/b.md", ["home"]),
        ]);
        const out = filterVaultStructure(root, "", "work");
        expect(out?.children?.length).toBe(1);
        expect(out?.children?.[0].name).toBe("a.md");
    });
});
