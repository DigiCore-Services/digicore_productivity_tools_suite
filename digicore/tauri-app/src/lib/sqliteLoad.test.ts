import { describe, it, expect, vi } from "vitest";
import { loadSnippetsPage } from "./sqliteLoad";

const { mockSelect } = vi.hoisted(() => ({
  mockSelect: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-sql", () => ({
  default: {
    load: vi.fn().mockResolvedValue({
      select: mockSelect,
    }),
  },
}));

describe("loadSnippetsPage", () => {
  it("returns empty when no data", async () => {
    mockSelect
      .mockResolvedValueOnce([{ n: 0 }])
      .mockResolvedValueOnce([]);
    const result = await loadSnippetsPage(0, 50);
    expect(result.rows).toHaveLength(0);
    expect(result.total).toBe(0);
  });

  it("returns rows and total from SQLite", async () => {
    mockSelect
      .mockResolvedValueOnce([{ n: 2 }])
      .mockResolvedValueOnce([
        { category: "General", trigger: "sig", content: "Best regards" },
        { category: "General", trigger: "ty", content: "Thank you" },
      ]);
    const result = await loadSnippetsPage(0, 50);
    expect(result.rows).toHaveLength(2);
    expect(result.total).toBe(2);
    expect(result.rows[0].trigger).toBe("sig");
  });

  it("does not throw when Database.load fails", async () => {
    const { default: Database } = await import("@tauri-apps/plugin-sql");
    (Database.load as ReturnType<typeof vi.fn>).mockRejectedValueOnce(
      new Error("DB error")
    );
    const result = await loadSnippetsPage(0, 50);
    expect(result.rows).toHaveLength(0);
    expect(result.total).toBe(0);
  });
});
