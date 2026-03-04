import { describe, it, expect, vi, beforeEach } from "vitest";
import { syncLibraryToSqlite, type SnippetForSync } from "./sqliteSync";

const { mockExecute } = vi.hoisted(() => ({
  mockExecute: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@tauri-apps/plugin-sql", () => ({
  default: {
    load: vi.fn().mockResolvedValue({
      execute: mockExecute,
    }),
  },
}));

describe("syncLibraryToSqlite", () => {
  beforeEach(() => {
    mockExecute.mockClear();
  });

  it("clears snippets and categories before insert", async () => {
    await syncLibraryToSqlite({});
    expect(mockExecute).toHaveBeenCalledWith("DELETE FROM snippets");
    expect(mockExecute).toHaveBeenCalledWith("DELETE FROM categories");
  });

  it("inserts categories and snippets for non-empty library", async () => {
    const library: Record<string, SnippetForSync[]> = {
      General: [
        {
          trigger: "sig",
          content: "Best regards",
          profile: "Default",
          options: "",
          app_lock: "",
          pinned: "false",
          last_modified: "",
        },
      ],
    };
    await syncLibraryToSqlite(library);
    expect(mockExecute).toHaveBeenCalledWith(
      "INSERT INTO categories (id, name) VALUES ($1, $2)",
      [1, "General"]
    );
    expect(mockExecute).toHaveBeenCalledWith(
      expect.stringContaining("INSERT INTO snippets"),
      expect.arrayContaining([1, "sig", "Best regards"])
    );
  });

  it("skips empty categories", async () => {
    await syncLibraryToSqlite({
      Empty: [],
      General: [{ trigger: "x", content: "y" }],
    });
    const insertCalls = mockExecute.mock.calls.filter((c: unknown[]) =>
      (c[0] as string).startsWith("INSERT INTO categories")
    );
    expect(insertCalls).toHaveLength(1);
    expect(insertCalls[0][1]).toEqual([1, "General"]);
  });

  it("does not throw when Database.load fails", async () => {
    const Database = (await import("@tauri-apps/plugin-sql")).default;
    (Database.load as ReturnType<typeof vi.fn>).mockRejectedValueOnce(
      new Error("DB error")
    );
    await expect(
      syncLibraryToSqlite({ General: [{ trigger: "x", content: "y" }] })
    ).resolves.not.toThrow();
  });
});
