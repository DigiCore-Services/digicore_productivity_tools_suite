/**
 * Shared theme resolution and broadcast for main window, Ghost Follower, Ghost Suggestor.
 * Configuration Core sets Dark/Light/System; this syncs to all windows.
 */

export type ResolvedTheme = "dark" | "light";

export function resolveTheme(pref: string): ResolvedTheme {
  if (pref === "system") {
    return window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "dark"
      : "light";
  }
  return pref === "dark" ? "dark" : "light";
}

export function applyThemeToDocument(resolved: ResolvedTheme): void {
  document.documentElement.dataset.theme = resolved;
}
