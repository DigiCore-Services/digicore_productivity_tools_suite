/**
 * Sync library from JSON/appState to SQLite.
 * Keeps digicore.db in sync for future partial loading and search.
 */
import Database from "@tauri-apps/plugin-sql";

export interface SnippetForSync {
  trigger: string;
  content: string;
  options?: string;
  category?: string;
  profile?: string;
  app_lock?: string;
  pinned?: string;
  last_modified?: string;
}

/**
 * Sync library to SQLite. Clears and repopulates categories + snippets.
 * Call after load_library or when appState.library changes.
 */
export async function syncLibraryToSqlite(
  library: Record<string, SnippetForSync[]>
): Promise<void> {
  try {
    const db = await Database.load("sqlite:digicore.db");

    await db.execute("DELETE FROM snippets");
    await db.execute("DELETE FROM categories");

    let categoryId = 1;

    for (const [catName, snippets] of Object.entries(library)) {
      if (!catName || !snippets?.length) continue;

      await db.execute(
        "INSERT INTO categories (id, name) VALUES ($1, $2)",
        [categoryId, catName]
      );

      for (const s of snippets) {
        await db.execute(
          `INSERT INTO snippets (category_id, trigger, content, options, profile, app_lock, pinned, last_modified)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)`,
          [
            categoryId,
            s.trigger ?? "",
            s.content ?? "",
            s.options ?? "",
            s.profile ?? "Default",
            s.app_lock ?? "",
            s.pinned ?? "false",
            s.last_modified ?? "",
          ]
        );
      }
      categoryId++;
    }
  } catch (e) {
    console.warn("[sqliteSync] Sync failed:", e);
  }
}
