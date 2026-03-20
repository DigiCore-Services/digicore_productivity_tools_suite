/**
 * Library tab utilities: formatting and cell value extraction.
 * Extracted for reuse and unit testing.
 */

export const COLUMN_KEYS: Record<string, string> = {
  Profile: "profile",
  Category: "category",
  Trigger: "trigger",
  "Content Preview": "content",
  AppLock: "app_lock",
  Options: "options",
  "Last Modified": "last_modified",
};

/**
 * Format last_modified from YYYYMMDDHHmmss to YYYY-MM-DD HH:mm:ss.
 */
export function formatLastModified(val: string): string {
  if (!val) return "";
  if (val.length >= 14) {
    const y = val.slice(0, 4),
      m = val.slice(4, 6),
      d = val.slice(6, 8);
    const h = val.slice(8, 10),
      min = val.slice(10, 12),
      sec = val.slice(12, 14);
    return `${y}-${m}-${d} ${h}:${min}:${sec}`;
  }
  return val;
}

export interface SnippetLike {
  content?: string;
  last_modified?: string;
  trigger?: string;
  profile?: string;
  category?: string;
  app_lock?: string;
  options?: string;
  htmlContent?: string | null;
  rtfContent?: string | null;
}

/**
 * Get raw value from snippet, supporting both snake_case (SQLite) and camelCase (backend DTO).
 * Exported for use in sorting and other non-display logic.
 */
export function getRawField(
  rec: Record<string, string | undefined>,
  key: string
): string {
  const snake = rec[key];
  if (snake !== undefined && snake !== "") return String(snake);
  const camel = key === "app_lock" ? "appLock" : key === "last_modified" ? "lastModified" : key;
  const val = rec[camel];
  return val !== undefined ? String(val) : "";
}

/**
 * Get display value for a table cell by column name.
 */
export function getCellValue(s: SnippetLike, col: string): string {
  const key = COLUMN_KEYS[col];
  if (!key) return "";
  const rec = s as Record<string, string | undefined>;
  if (key === "content") {
    const content = (rec.content || "").replace(/\n/g, " ").slice(0, 60);
    return content + (rec.content?.length && rec.content.length > 60 ? "..." : "");
  }
  if (key === "last_modified") return formatLastModified(getRawField(rec, "last_modified") || "");
  return getRawField(rec, key);
}
