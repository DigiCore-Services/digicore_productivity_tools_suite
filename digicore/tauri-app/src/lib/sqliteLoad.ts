/**
 * Load snippets from SQLite with pagination for large libraries.
 * Use when library exceeds threshold (e.g. 5000 items) to avoid loading all into memory.
 */
import Database from "@tauri-apps/plugin-sql";

export interface SnippetRow {
  category: string;
  trigger: string;
  trigger_type: 'suffix' | 'regex';
  content: string;
  html_content: string | null;
  rtf_content: string | null;
  options: string;
  profile: string;
  app_lock: string;
  pinned: string;
  case_adaptive: boolean | number;
  case_sensitive: boolean | number;
  smart_suffix: boolean | number;
  is_sensitive: boolean | number;
  last_modified: string;
}

/**
 * Load a page of snippets from SQLite. Returns rows for display.
 * @param offset - Row offset (0-based)
 * @param limit - Max rows to return
 * @param search - Optional search filter (matches trigger, content, category)
 */
export async function loadSnippetsPage(
  offset: number,
  limit: number,
  search?: string
): Promise<{ rows: SnippetRow[]; total: number }> {
  try {
    const db = await Database.load("sqlite:digicore.db");

    const searchPattern = search?.trim() ? `%${search}%` : "";
    const searchClause = searchPattern
      ? `WHERE s.trigger LIKE $1 OR s.content LIKE $1 OR c.name LIKE $1`
      : "";
    const countSql = searchPattern
      ? `SELECT COUNT(*) as n FROM snippets s JOIN categories c ON s.category_id = c.id ${searchClause}`
      : `SELECT COUNT(*) as n FROM snippets`;
    const countParams = searchPattern ? [searchPattern] : [];

    const countResult = await db.select<{ n: number }[]>(
      countSql,
      countParams
    );
    const total = Number(countResult[0]?.n ?? 0);

    const selectWhere = searchPattern
      ? `WHERE s.trigger LIKE $1 OR s.content LIKE $1 OR c.name LIKE $1`
      : "";
    const selectParams = searchPattern
      ? [searchPattern, limit, offset]
      : [limit, offset];
    const limitOffset = searchPattern ? `LIMIT $2 OFFSET $3` : `LIMIT $1 OFFSET $2`;

    const sql = `
      SELECT c.name as category, s.trigger, s.trigger_type, s.content, s.html_content, s.rtf_content, s.options, s.profile, s.app_lock, s.pinned, s.case_adaptive, s.case_sensitive, s.smart_suffix, s.is_sensitive, s.last_modified
      FROM snippets s
      JOIN categories c ON s.category_id = c.id
      ${selectWhere}
      ORDER BY CASE WHEN s.pinned = 'true' THEN 0 ELSE 1 END, c.name, s.trigger
      ${limitOffset}
    `;

    const rows = (await db.select<SnippetRow[]>(sql, selectParams)) ?? [];
    return { rows, total };
  } catch (e) {
    console.warn("[sqliteLoad] Load failed:", e);
    return { rows: [], total: 0 };
  }
}
