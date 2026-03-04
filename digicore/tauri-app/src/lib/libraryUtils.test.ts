import { describe, it, expect } from "vitest";
import {
  formatLastModified,
  getCellValue,
  COLUMN_KEYS,
  type SnippetLike,
} from "./libraryUtils";

describe("formatLastModified", () => {
  it("returns empty string for empty input", () => {
    expect(formatLastModified("")).toBe("");
  });

  it("formats YYYYMMDDHHmmss to YYYY-MM-DD HH:mm:ss", () => {
    expect(formatLastModified("20260303143022")).toBe("2026-03-03 14:30:22");
  });

  it("returns original value when shorter than 14 chars", () => {
    expect(formatLastModified("2026")).toBe("2026");
  });
});

describe("getCellValue", () => {
  it("returns empty for unknown column", () => {
    expect(getCellValue({ trigger: "sig" }, "Unknown")).toBe("");
  });

  it("returns trigger value for Trigger column", () => {
    expect(getCellValue({ trigger: "sig" } as SnippetLike, "Trigger")).toBe(
      "sig"
    );
  });

  it("truncates content with ellipsis when > 60 chars", () => {
    const long = "a".repeat(70);
    const result = getCellValue({ content: long } as SnippetLike, "Content Preview");
    expect(result).toHaveLength(63);
    expect(result.endsWith("...")).toBe(true);
  });

  it("formats last_modified for Last Modified column", () => {
    expect(
      getCellValue(
        { last_modified: "20260303143022" } as SnippetLike,
        "Last Modified"
      )
    ).toBe("2026-03-03 14:30:22");
  });

  it("returns profile for Profile column", () => {
    expect(getCellValue({ profile: "Work" } as SnippetLike, "Profile")).toBe(
      "Work"
    );
  });
});

describe("COLUMN_KEYS", () => {
  it("maps all expected columns", () => {
    expect(Object.keys(COLUMN_KEYS)).toContain("Trigger");
    expect(Object.keys(COLUMN_KEYS)).toContain("Content Preview");
    expect(COLUMN_KEYS["Last Modified"]).toBe("last_modified");
  });
});
