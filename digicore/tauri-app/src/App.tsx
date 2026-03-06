import React, { useCallback, useEffect, useState, lazy, Suspense } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { getTaurpc } from "./lib/taurpc";
import { emit, listen } from "@tauri-apps/api/event";
import { onOpenUrl } from "@tauri-apps/plugin-deep-link";
import { confirm as confirmDialog } from "@tauri-apps/plugin-dialog";
import {
  Library,
  Settings,
  ClipboardList,
  Code,
  Droplets,
  BarChart3,
  FileText,
} from "lucide-react";
import type { AppState, PendingVariableInput, Snippet } from "./types";
import { normalizeAppState, normalizePendingInput } from "./lib/normalizeState";
import { LibraryTab } from "./components/LibraryTab";
const ConfigTab = lazy(() =>
  import("./components/ConfigTab").then((m) => ({ default: m.ConfigTab }))
);
const ClipboardTab = lazy(() =>
  import("./components/ClipboardTab").then((m) => ({ default: m.ClipboardTab }))
);
const ScriptTab = lazy(() =>
  import("./components/ScriptTab").then((m) => ({ default: m.ScriptTab }))
);
const AppearanceTab = lazy(() =>
  import("./components/AppearanceTab").then((m) => ({ default: m.AppearanceTab }))
);
const AnalyticsTab = lazy(() =>
  import("./components/AnalyticsTab").then((m) => ({ default: m.AnalyticsTab }))
);
const LogTab = lazy(() =>
  import("./components/LogTab").then((m) => ({ default: m.LogTab }))
);
const SnippetEditor = lazy(() =>
  import("./components/modals/SnippetEditor").then((m) => ({
    default: m.SnippetEditor,
  }))
);
import {
  DeleteConfirm,
  ViewFull,
  ClipClearConfirm,
  ClipEntryDeleteConfirm,
  VariableInput,
} from "./components/modals";
import { CommandPalette } from "./components/CommandPalette";
import {
  initNotificationActionListener,
  notify,
  notifyDiscoverySuggestion,
} from "./lib/notifications";
import { syncLibraryToSqlite } from "./lib/sqliteSync";

const DEFAULT_COLUMNS = [
  "Profile",
  "Category",
  "Trigger",
  "Content Preview",
  "AppLock",
  "Options",
  "Last Modified",
];

function App() {
  const [activeTab, setActiveTab] = useState(0);
  const [appState, setAppState] = useState<AppState | null>(null);
  const [columnOrder, setColumnOrder] = useState<string[]>(DEFAULT_COLUMNS);
  const [sortColumn, setSortColumn] = useState<string | null>("Trigger");
  const [sortAsc, setSortAsc] = useState(true);

  const [snippetEditorVisible, setSnippetEditorVisible] = useState(false);
  const [snippetEditorMode, setSnippetEditorMode] = useState<"add" | "edit">(
    "add"
  );
  const [snippetEditorCategory, setSnippetEditorCategory] = useState("");
  const [snippetEditorIdx, setSnippetEditorIdx] = useState(-1);
  const [snippetEditorPrefill, setSnippetEditorPrefill] = useState<{
    content: string;
    trigger: string;
  } | undefined>();

  const [deleteConfirmVisible, setDeleteConfirmVisible] = useState(false);
  const [deleteConfirmCat, setDeleteConfirmCat] = useState("");
  const [deleteConfirmIdx, setDeleteConfirmIdx] = useState(-1);

  const [viewFullVisible, setViewFullVisible] = useState(false);
  const [viewFullContent, setViewFullContent] = useState("");
  const [viewFullEditMeta, setViewFullEditMeta] = useState<{
    category: string;
    snippetIdx: number;
    source?: "library" | "ghost-pinned";
  } | null>(null);
  const [viewFullClipboardMeta, setViewFullClipboardMeta] = useState<{
    index: number;
    canPromote: boolean;
    trigger?: string;
  } | null>(null);

  const [clipClearConfirmVisible, setClipClearConfirmVisible] = useState(false);
  const [clipEntryDeleteConfirmVisible, setClipEntryDeleteConfirmVisible] = useState(false);
  const [clipEntryDeleteConfirmIdx, setClipEntryDeleteConfirmIdx] = useState(-1);

  const [variableInputVisible, setVariableInputVisible] = useState(false);
  const [variableInputData, setVariableInputData] =
    useState<PendingVariableInput | null>(null);

  const [commandPaletteVisible, setCommandPaletteVisible] = useState(false);

  const [clipboardRefreshTrigger, setClipboardRefreshTrigger] = useState(0);

  const [discoveryBanner, setDiscoveryBanner] = useState<{
    phrase: string;
    count: number;
  } | null>(null);
  const normalizeContentForMatch = useCallback((value: string) => {
    return (value || "").replace(/\r\n/g, "\n").trim();
  }, []);
  const snippetExists = useCallback(
    (content: string) => {
      const normalizedContent = normalizeContentForMatch(content);
      if (!normalizedContent) return false;
      const library = appState?.library ?? {};
      return Object.values(library).some((snippets) =>
        snippets.some(
          (snippet) =>
            normalizeContentForMatch(snippet.content || "") === normalizedContent
        )
      );
    },
    [appState?.library, normalizeContentForMatch]
  );
  const findSnippetByContent = useCallback(
    (content: string): { category: string; snippetIdx: number; pinned: boolean } | null => {
      const normalizedContent = normalizeContentForMatch(content);
      if (!normalizedContent) return null;
      const library = appState?.library ?? {};
      for (const [category, snippets] of Object.entries(library)) {
        for (let i = 0; i < snippets.length; i += 1) {
          const snippet = snippets[i];
          if (normalizeContentForMatch(snippet.content || "") === normalizedContent) {
            return {
              category,
              snippetIdx: i,
              pinned: (snippet.pinned || "false").toLowerCase() === "true",
            };
          }
        }
      }
      return null;
    },
    [appState?.library, normalizeContentForMatch]
  );

  const loadAppState = useCallback(async () => {
    try {
      const state = await getTaurpc().get_app_state();
      setAppState(normalizeAppState(state));
      return normalizeAppState(state);
    } catch (e) {
      setLibraryStatus("Error: " + String(e));
      setLibraryStatusError(true);
      return null;
    }
  }, []);

  const [libraryStatus, setLibraryStatus] = useState("Ready");
  const [libraryStatusError, setLibraryStatusError] = useState(false);
  const setLibraryStatusFn = useCallback(
    (text: string, isError = false) => {
      setLibraryStatus(text);
      setLibraryStatusError(isError);
    },
    []
  );

  const openSnippetEditor = useCallback(
    (
      mode: "add" | "edit",
      category: string,
      snippetIdx: number,
      prefill?: { content: string; trigger: string }
    ) => {
      setSnippetEditorMode(mode);
      setSnippetEditorCategory(category);
      setSnippetEditorIdx(snippetIdx);
      setSnippetEditorPrefill(prefill);
      setSnippetEditorVisible(true);
    },
    []
  );

  const restoreGhostFollowerAlwaysOnTop = useCallback(() => {
    getTaurpc().ghost_follower_restore_always_on_top().catch(() => {});
  }, []);

  const closeSnippetEditor = useCallback(() => {
    setSnippetEditorVisible(false);
    restoreGhostFollowerAlwaysOnTop();
  }, [restoreGhostFollowerAlwaysOnTop]);

  const handleSnippetSave = useCallback(
    async (snippet: Snippet) => {
      if (!snippet.trigger.trim()) {
        setLibraryStatusFn("Trigger is required", true);
        return;
      }
      try {
        if (snippetEditorMode === "add") {
          await getTaurpc().add_snippet(
            snippet.category || "General",
            snippet
          );
        } else {
          await getTaurpc().update_snippet(
            snippetEditorCategory,
            snippetEditorIdx,
            snippet
          );
        }
        await getTaurpc().save_library();
        await getTaurpc().save_settings().catch(() => {});
        closeSnippetEditor();
        const state = await loadAppState();
        if (state) {
          setAppState(state);
          if (state.library && Object.keys(state.library).length > 0) {
            await syncLibraryToSqlite(state.library);
          }
        }
        setLibraryStatusFn(
          snippetEditorMode === "add" ? "Snippet added and saved" : "Snippet updated and saved"
        );
      } catch (e) {
        setLibraryStatusFn("Error: " + String(e), true);
      }
    },
    [
      snippetEditorMode,
      snippetEditorCategory,
      snippetEditorIdx,
      closeSnippetEditor,
      loadAppState,
      setLibraryStatusFn,
    ]
  );

  const openDeleteConfirm = useCallback((cat: string, idx: number) => {
    setDeleteConfirmCat(cat);
    setDeleteConfirmIdx(idx);
    setDeleteConfirmVisible(true);
  }, []);

  const openClipEntryDeleteConfirm = useCallback((idx: number) => {
    setActiveTab(2);
    setClipEntryDeleteConfirmIdx(idx);
    setClipEntryDeleteConfirmVisible(true);
  }, []);

  const handleDeleteConfirm = useCallback(async () => {
    try {
      await getTaurpc().delete_snippet(deleteConfirmCat, deleteConfirmIdx);
      await getTaurpc().save_library();
      await getTaurpc().save_settings().catch(() => {});
      setDeleteConfirmVisible(false);
      const state = await loadAppState();
      if (state) {
        setAppState(state);
        const lib = state.library ?? {};
        if (Object.keys(lib).length > 0) {
          await syncLibraryToSqlite(lib as Record<string, Snippet[]>);
        }
      }
      setLibraryStatusFn("Snippet deleted and saved");
    } catch (e) {
      setLibraryStatusFn("Delete failed: " + String(e), true);
    }
  }, [deleteConfirmCat, deleteConfirmIdx, loadAppState, setLibraryStatusFn, restoreGhostFollowerAlwaysOnTop]);

  const openViewFull = useCallback(
    (
      content: string,
      meta?:
        | { category: string; snippetIdx: number; source?: "library" | "ghost-pinned" }
        | { index: number; canPromote: boolean; trigger?: string }
        | null
    ) => {
      // TODO(ui-view-full-actions): Keep source-aware action routing centralized here.
      setViewFullContent(content);
      if (meta && "category" in meta) {
        setViewFullEditMeta(meta);
        setViewFullClipboardMeta(null);
      } else if (meta && "index" in meta) {
        setViewFullClipboardMeta(meta);
        setViewFullEditMeta(null);
      } else {
        setViewFullEditMeta(null);
        setViewFullClipboardMeta(null);
      }
      setViewFullVisible(true);
    },
    []
  );

  const handleViewFullCopy = useCallback(async () => {
    try {
      await getTaurpc().copy_to_clipboard(viewFullContent);
      setLibraryStatusFn("Copied to clipboard!");
    } catch (e) {
      setLibraryStatusFn("Error: " + String(e), true);
    }
  }, [viewFullContent, setLibraryStatusFn]);

  const handleViewFullPromote = useCallback(() => {
    if (!viewFullClipboardMeta?.canPromote) return;
    const categories = appState?.categories || ["General"];
    const trigger =
      viewFullClipboardMeta.trigger ||
      (viewFullContent || "").slice(0, 20).replace(/\s/g, "").trim() ||
      "clip";
    setViewFullVisible(false);
    setViewFullEditMeta(null);
    setViewFullClipboardMeta(null);
    openSnippetEditor("add", categories[0] || "General", -1, {
      content: viewFullContent,
      trigger,
    });
  }, [viewFullClipboardMeta, appState?.categories, viewFullContent, openSnippetEditor]);

  const handleViewFullDelete = useCallback(() => {
    if (viewFullClipboardMeta == null) return;
    const idx = viewFullClipboardMeta.index;
    setViewFullVisible(false);
    setViewFullEditMeta(null);
    setViewFullClipboardMeta(null);
    openClipEntryDeleteConfirm(idx);
  }, [viewFullClipboardMeta, openClipEntryDeleteConfirm]);

  const handleViewFullDeleteSnippet = useCallback(() => {
    if (!viewFullEditMeta) return;
    const { category, snippetIdx } = viewFullEditMeta;
    setViewFullVisible(false);
    setViewFullEditMeta(null);
    setViewFullClipboardMeta(null);
    openDeleteConfirm(category, snippetIdx);
  }, [viewFullEditMeta, openDeleteConfirm]);

  const handleViewFullPinPromoted = useCallback(async () => {
    const match = findSnippetByContent(viewFullContent);
    if (!match) {
      setLibraryStatusFn("No promoted snippet found to pin.", true);
      return;
    }
    if (match.pinned) {
      setLibraryStatusFn("Snippet already pinned.");
      return;
    }
    try {
      await getTaurpc().ghost_follower_toggle_pin(match.category, match.snippetIdx);
      setLibraryStatusFn("Snippet pinned.");
      await loadAppState();
    } catch (e) {
      setLibraryStatusFn("Pin failed: " + String(e), true);
    }
  }, [findSnippetByContent, viewFullContent, setLibraryStatusFn, loadAppState]);

  const handleViewFullUnpinSnippet = useCallback(async () => {
    if (!viewFullEditMeta) return;
    const { category, snippetIdx } = viewFullEditMeta;
    const snippet = appState?.library?.[category]?.[snippetIdx];
    const isPinned = (snippet?.pinned || "false").toLowerCase() === "true";
    if (!isPinned) {
      setLibraryStatusFn("Snippet is already unpinned.");
      return;
    }
    const confirmed = await confirmDialog(
      "Are you sure you want to unpin this snippet?",
      { title: "Confirm Unpin", kind: "warning" }
    );
    if (!confirmed) {
      setLibraryStatusFn("Unpin cancelled.");
      return;
    }
    try {
      await getTaurpc().ghost_follower_toggle_pin(category, snippetIdx);
      setLibraryStatusFn("Snippet unpinned.");
      await loadAppState();
    } catch (e) {
      setLibraryStatusFn("Unpin failed: " + String(e), true);
    }
  }, [viewFullEditMeta, appState?.library, setLibraryStatusFn, loadAppState]);

  const handleClipEntryDeleteConfirm = useCallback(async () => {
    try {
      await getTaurpc().delete_clip_entry(clipEntryDeleteConfirmIdx);
      setClipEntryDeleteConfirmVisible(false);
      setClipEntryDeleteConfirmIdx(-1);
      restoreGhostFollowerAlwaysOnTop();
      setClipboardRefreshTrigger((n) => n + 1);
      setLibraryStatusFn("Entry deleted.");
    } catch (e) {
      setLibraryStatusFn("Delete failed: " + String(e), true);
    }
  }, [clipEntryDeleteConfirmIdx, setLibraryStatusFn, restoreGhostFollowerAlwaysOnTop]);

  const handleClipClearConfirm = useCallback(async () => {
    try {
      await getTaurpc().clear_clipboard_history();
      setClipClearConfirmVisible(false);
      setClipboardRefreshTrigger((n) => n + 1);
      setLibraryStatusFn("Clipboard history cleared.");
    } catch (e) {
      setLibraryStatusFn("Error: " + String(e), true);
    }
  }, [setLibraryStatusFn]);

  const handleSaveUiPrefs = useCallback(
    async (lastTab: number, cols: string[]) => {
      try {
        await getTaurpc().save_ui_prefs(lastTab, cols);
      } catch {
        /* ignore */
      }
    },
    []
  );

  const handleColumnDrag = useCallback(
    (_from: string, _to: string) => {
      handleSaveUiPrefs(activeTab, columnOrder);
    },
    [activeTab, columnOrder, handleSaveUiPrefs]
  );

  const handleVariableInputOk = useCallback(async (values: Record<string, string>) => {
    try {
      await getTaurpc().submit_variable_input(values);
      setVariableInputVisible(false);
      setVariableInputData(null);
    } catch (e) {
      console.error("Variable input submit:", e);
    }
  }, []);

  const handleVariableInputCancel = useCallback(async () => {
    try {
      await getTaurpc().cancel_variable_input();
    } catch {
      /* ignore */
    }
    setVariableInputVisible(false);
    setVariableInputData(null);
  }, []);

  const showVariableInputModal = useCallback(async () => {
    try {
      const data = await getTaurpc().get_pending_variable_input();
      if (data) {
        setVariableInputData(normalizePendingInput(data));
        setVariableInputVisible(true);
      }
    } catch {
      /* ignore */
    }
  }, []);

  useEffect(() => {
    const pref =
      (typeof localStorage !== "undefined" &&
        localStorage.getItem("digicore-theme")) ||
      "light";
    const resolved =
      pref === "system"
        ? (window.matchMedia("(prefers-color-scheme: dark)").matches
            ? "dark"
            : "light")
        : pref;
    document.documentElement.dataset.theme = resolved;
    emit("digicore-theme-changed", { theme: resolved }).catch(() => {});
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen("show-variable-input", () => {
      showVariableInputModal();
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [showVariableInputModal]);

  const bringMainToForeground = useCallback(async () => {
    try {
      const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
      const win = await WebviewWindow.getByLabel("main");
      if (win) {
        await win.show();
        await win.setFocus();
        await win.unminimize();
      }
    } catch {
      /* ignore */
    }
  }, []);

  useEffect(() => {
    const unlistens: (() => void)[] = [];
    listen<
      | string
      | {
          content?: string;
          source?: "pinned" | "clipboard";
          category?: string;
          snippetIdx?: number;
          index?: number;
          trigger?: string;
        }
    >("ghost-follower-view-full", async (e) => {
      await bringMainToForeground();
      const payload = e.payload;
      if (typeof payload === "string") {
        openViewFull(payload ?? "");
        return;
      }
      if (!payload) {
        openViewFull("");
        return;
      }
      if (
        payload.source === "pinned" &&
        payload.category != null &&
        payload.snippetIdx != null
      ) {
        openViewFull(payload.content ?? "", {
          category: payload.category,
          snippetIdx: payload.snippetIdx,
          source: "ghost-pinned",
        });
        return;
      }
      if (payload.source === "clipboard" && payload.index != null) {
        const content = payload.content ?? "";
        openViewFull(content, {
          index: payload.index,
          canPromote: !snippetExists(content),
          trigger: payload.trigger,
        });
        return;
      }
      openViewFull(payload.content ?? "");
    }).then((fn) => unlistens.push(fn));
    listen<{ category: string; snippetIdx: number }>(
      "ghost-follower-edit",
      async (e) => {
        await bringMainToForeground();
        const { category, snippetIdx } = e.payload ?? {};
        if (category != null && snippetIdx != null) {
          openSnippetEditor("edit", category, snippetIdx);
        }
      }
    ).then((fn) => unlistens.push(fn));
    listen<{ content: string; trigger: string }>(
      "ghost-follower-promote",
      async (e) => {
        await bringMainToForeground();
        const { content, trigger } = e.payload ?? {};
        if (content != null && trigger != null) {
          const cats = appState?.categories ?? ["General"];
          openSnippetEditor("add", cats[0] ?? "General", -1, {
            content,
            trigger,
          });
        }
      }
    ).then((fn) => unlistens.push(fn));
    listen<{ category: string; snippetIdx: number }>(
      "ghost-follower-delete-snippet",
      async (e) => {
        await bringMainToForeground();
        const { category, snippetIdx } = e.payload ?? {};
        if (category != null && snippetIdx != null) {
          setActiveTab(0);
          openDeleteConfirm(category, snippetIdx);
        }
      }
    ).then((fn) => unlistens.push(fn));
    listen<{ index: number }>("ghost-follower-delete-clip", async (e) => {
      await bringMainToForeground();
      const { index } = e.payload ?? {};
      if (typeof index === "number" && index >= 0) {
        openClipEntryDeleteConfirm(index);
      }
    }).then((fn) => unlistens.push(fn));
    listen<{ text?: string }>("ghost-follower-status", (e) => {
      const text = e.payload?.text;
      if (text) setLibraryStatusFn(text);
    }).then((fn) => unlistens.push(fn));
    listen<{ category: string; snippetIdx: number; content?: string }>(
      "quick-search-view-full",
      async (e) => {
        await bringMainToForeground();
        const { category, snippetIdx, content } = e.payload ?? {};
        if (category != null && snippetIdx != null) {
          openViewFull(content ?? "", {
            category,
            snippetIdx,
            source: "library",
          });
        }
      }
    ).then((fn) => unlistens.push(fn));
    listen<{ category: string; snippetIdx: number }>(
      "quick-search-edit-snippet",
      async (e) => {
        await bringMainToForeground();
        const { category, snippetIdx } = e.payload ?? {};
        if (category != null && snippetIdx != null) {
          openSnippetEditor("edit", category, snippetIdx);
        }
      }
    ).then((fn) => unlistens.push(fn));
    listen("quick-search-library-refresh", async () => {
      await loadAppState();
    }).then((fn) => unlistens.push(fn));
    return () => unlistens.forEach((u) => u());
  }, [
    bringMainToForeground,
    openViewFull,
    openSnippetEditor,
    openDeleteConfirm,
    openClipEntryDeleteConfirm,
    appState?.categories,
    snippetExists,
    setLibraryStatusFn,
    loadAppState,
  ]);

  useEffect(() => {
    let cleanup: (() => void) | undefined;
    initNotificationActionListener().then((fn) => {
      cleanup = fn;
    });
    return () => cleanup?.();
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen("notification-view-library", async () => {
      setActiveTab(0);
      try {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        const win = getCurrentWindow();
        await win.setFocus();
        await win.unminimize();
      } catch {
        /* ignore */
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen("show-command-palette", async () => {
      if (!appState) await loadAppState();
      setCommandPaletteVisible(true);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [appState, loadAppState]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen("window-closed-to-tray", () => {
      notify("DigiCore Text Expander", "Running in background. Right-click tray icon to open.");
    }).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen("discovery-suggestion", (e) => {
      const [phrase, count] = (e.payload as [string, number]) ?? ["", 0];
      if (phrase) {
        setDiscoveryBanner({ phrase, count });
        notifyDiscoverySuggestion(phrase, count);
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen("discovery-action-snooze", async () => {
      setDiscoveryBanner(null);
      try {
        await getTaurpc().ghost_suggestor_snooze();
      } catch {
        /* ignore */
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen("discovery-action-ignore", async () => {
      setDiscoveryBanner(null);
      try {
        const state = await getTaurpc().get_ghost_suggestor_state();
        const phrase =
          state?.suggestions?.[0]?.trigger ?? state?.suggestions?.[state?.selected_index ?? 0]?.trigger ?? "";
        if (phrase) await getTaurpc().ghost_suggestor_ignore(phrase);
      } catch {
        /* ignore */
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen("discovery-action-promote", async () => {
      setDiscoveryBanner(null);
      try {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        const win = getCurrentWindow();
        await win.show();
        await win.setFocus();
        await win.unminimize();
        const result = await getTaurpc().ghost_suggestor_create_snippet();
        if (result) {
          const [, content] = result;
          setActiveTab(0);
          const cats = appState?.categories ?? ["General"];
          openSnippetEditor("add", cats[0] || "General", -1, {
            trigger: "",
            content,
          });
        }
      } catch {
        /* ignore */
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, [appState?.categories, openSnippetEditor]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen("tray-add-snippet", () => {
      setActiveTab(0);
      const cats = appState?.categories ?? ["General"];
      openSnippetEditor("add", cats[0] || "General", -1);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [appState?.categories, openSnippetEditor]);

  const handleCliArgs = useCallback(
    (args: string[]) => {
      if (args.some((a) => a === "--open-settings" || a === "-s")) {
        setActiveTab(1);
      } else if (args.some((a) => a === "--add-snippet" || a === "-a")) {
        setActiveTab(0);
        const cats = appState?.categories ?? ["General"];
        openSnippetEditor("add", cats[0] || "General", -1);
      }
    },
    [appState?.categories, openSnippetEditor]
  );

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen("initial-cli-args", (e) => {
      handleCliArgs((e.payload as string[]) ?? []);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [handleCliArgs]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen("secondary-instance-args", (e) => {
      handleCliArgs((e.payload as string[]) ?? []);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [handleCliArgs]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    onOpenUrl((urls) => {
      for (const url of urls) {
        try {
          const u = new URL(url);
          if (u.pathname === "/open/settings" || u.pathname === "/settings") {
            setActiveTab(1);
          } else if (u.pathname === "/open/snippet" && u.searchParams.has("trigger")) {
            const trigger = u.searchParams.get("trigger") ?? "";
            setActiveTab(0);
            const cats = appState?.categories ?? ["General"];
            openSnippetEditor("add", cats[0] || "General", -1, { trigger, content: "" });
          } else if (u.pathname === "/add-snippet") {
            setActiveTab(0);
            const cats = appState?.categories ?? ["General"];
            openSnippetEditor("add", cats[0] || "General", -1);
          }
        } catch {
          /* ignore invalid URLs */
        }
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [appState?.categories, openSnippetEditor]);

  useEffect(() => {
    (async () => {
      try {
        const prefs = await getTaurpc().get_ui_prefs();
        const cols = Array.isArray(prefs.column_order) && prefs.column_order.length
          ? prefs.column_order.filter((c) => DEFAULT_COLUMNS.includes(c))
          : [...DEFAULT_COLUMNS];
        for (const c of DEFAULT_COLUMNS) {
          if (!cols.includes(c)) cols.push(c);
        }
        setColumnOrder(cols);
        const lastTab = Math.min(0 | (prefs.last_tab ?? 0), 6);
        setActiveTab(lastTab);
      } catch {
        /* ignore */
      }
      const state = await loadAppState();
      const lib = state?.library ?? {};
      if (Object.keys(lib).length > 0) {
        syncLibraryToSqlite(lib as Record<string, Snippet[]>).catch(() => {});
      }
    })();
  }, [loadAppState]);

  // Ghost windows are created from Rust (lib.rs setup) for correct URL resolution.

  const tabs = [
    { id: "library", label: "Text Expansion Library", icon: Library },
    { id: "config", label: "Configurations and Settings", icon: Settings },
    { id: "clipboard", label: "Clipboard History", icon: ClipboardList },
    { id: "script", label: "Scripting Engine Library", icon: Code },
    { id: "appearance", label: "Appearance", icon: Droplets },
    { id: "analytics", label: "Statistics", icon: BarChart3 },
    { id: "log", label: "Log", icon: FileText },
  ];

  const handleTabClick = useCallback(
    async (idx: number) => {
      setActiveTab(idx);
      handleSaveUiPrefs(idx, columnOrder);
      if (idx === 1) {
        const state = await loadAppState();
        if (state) setAppState(state);
      }
    },
    [columnOrder, handleSaveUiPrefs, loadAppState]
  );

  const getInitialSnippet = (): Snippet | null => {
    if (snippetEditorMode !== "edit" || !appState?.library) return null;
    const snippets = appState.library[snippetEditorCategory];
    if (!snippets || snippetEditorIdx < 0) return null;
    return snippets[snippetEditorIdx] ?? null;
  };

  const deleteConfirmMessage =
    appState?.library?.[deleteConfirmCat]?.[deleteConfirmIdx]
      ? `Delete snippet "${(
          appState.library[deleteConfirmCat][deleteConfirmIdx].trigger || ""
        ).slice(0, 30)}${(appState.library[deleteConfirmCat][deleteConfirmIdx].trigger || "").length > 30 ? "..." : ""}"?`
      : "Are you sure you want to delete this snippet?";

  return (
    <div className="flex min-h-screen flex-col bg-[var(--dc-bg)] text-[var(--dc-text)] font-sans">
      <div className="sticky top-0 z-20 bg-[var(--dc-bg)] border-b border-[var(--dc-border)] p-5 pt-4 pb-4">
      <nav
        className="flex gap-1 flex-wrap"
        role="tablist"
        aria-label="Main navigation"
      >
        {tabs.map((tab, idx) => {
          const Icon = tab.icon;
          return (
            <motion.button
              key={tab.id}
              type="button"
              role="tab"
              aria-selected={activeTab === idx}
              aria-controls={`panel-${tab.id}`}
              id={`tab-${tab.id}`}
              tabIndex={activeTab === idx ? 0 : -1}
              onClick={() => handleTabClick(idx)}
              className={`inline-flex items-center gap-2 px-4 py-2.5 rounded-lg text-sm font-medium transition-colors ${
                activeTab === idx
                  ? "bg-[var(--dc-accent)] text-white shadow-sm"
                  : "bg-[var(--dc-bg-alt)] text-[var(--dc-text)] border border-[var(--dc-border)] hover:bg-[var(--dc-bg-tertiary)]"
              }`}
              whileHover={{ scale: 1.02 }}
              whileTap={{ scale: 0.98 }}
            >
              <Icon className="w-4 h-4" aria-hidden />
              {tab.label}
            </motion.button>
          );
        })}
      </nav>
      </div>
      {discoveryBanner && (
        <div className="mx-5 mt-2 mb-0 p-3 rounded-lg bg-[var(--dc-accent)]/10 border border-[var(--dc-accent)]/30 flex items-center justify-between gap-4">
          <span className="text-sm text-[var(--dc-text)] truncate flex-1">
            <strong>Discovery (typed {discoveryBanner.count}x):</strong>{" "}
            &quot;{discoveryBanner.phrase.length > 50
              ? discoveryBanner.phrase.slice(0, 47) + "..."
              : discoveryBanner.phrase}
            &quot;
          </span>
          <div className="flex gap-2 shrink-0">
            <button
              type="button"
              onClick={async () => {
                setDiscoveryBanner(null);
                try {
                  await getTaurpc().ghost_suggestor_snooze();
                } catch {
                  /* ignore */
                }
              }}
              className="px-3 py-1.5 text-xs font-medium rounded-md bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] hover:bg-[var(--dc-bg-tertiary)]"
            >
              Snooze
            </button>
            <button
              type="button"
              onClick={async () => {
                setDiscoveryBanner(null);
                try {
                  const result = await getTaurpc().ghost_suggestor_create_snippet();
                  if (result) {
                    const [, content] = result;
                    setActiveTab(0);
                    const cats = appState?.categories ?? ["General"];
                    openSnippetEditor("add", cats[0] || "General", -1, {
                      trigger: "",
                      content,
                    });
                  }
                } catch {
                  /* ignore */
                }
              }}
              className="px-3 py-1.5 text-xs font-medium rounded-md bg-[var(--dc-accent)] text-white hover:opacity-90"
            >
              Promote to Snippet
            </button>
            <button
              type="button"
              onClick={async () => {
                const phrase = discoveryBanner.phrase;
                setDiscoveryBanner(null);
                try {
                  await getTaurpc().ghost_suggestor_ignore(phrase);
                } catch {
                  /* ignore */
                }
              }}
              className="px-3 py-1.5 text-xs font-medium rounded-md bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] hover:bg-[var(--dc-bg-tertiary)]"
            >
              Ignore
            </button>
          </div>
        </div>
      )}
      <div className="flex-1 overflow-auto p-5 pt-4">
      <AnimatePresence mode="wait">
        {activeTab === 0 && (
        <motion.div
          key="library"
          id="panel-library"
          role="tabpanel"
          aria-labelledby="tab-library"
          initial={{ opacity: 0, x: -10 }}
          animate={{ opacity: 1, x: 0 }}
          exit={{ opacity: 0, x: 10 }}
          transition={{ duration: 0.2 }}
        >
        <LibraryTab
          appState={appState}
          onAppStateChange={setAppState}
          setStatus={setLibraryStatusFn}
          libraryStatus={libraryStatus}
          libraryStatusError={libraryStatusError}
          onOpenViewFull={openViewFull}
          onOpenSnippetEditor={openSnippetEditor}
          onOpenDeleteConfirm={openDeleteConfirm}
          columnOrder={columnOrder}
          sortColumn={sortColumn}
          sortAsc={sortAsc}
          onColumnOrderChange={setColumnOrder}
          onSortChange={(col, asc) => {
            setSortColumn(col);
            setSortAsc(asc);
          }}
          onColumnDrag={handleColumnDrag}
        />
        </motion.div>
      )}
      {activeTab === 1 && (
        <motion.div
          key="config"
          id="panel-config"
          role="tabpanel"
          aria-labelledby="tab-config"
          initial={{ opacity: 0, x: -10 }}
          animate={{ opacity: 1, x: 0 }}
          exit={{ opacity: 0, x: 10 }}
          transition={{ duration: 0.2 }}
        >
          <Suspense fallback={<div className="py-8 text-[var(--dc-text-muted)]">Loading...</div>}>
            <ConfigTab appState={appState} onConfigLoaded={(state) => setAppState(state)} />
          </Suspense>
        </motion.div>
      )}
      {activeTab === 2 && (
        <motion.div
          key="clipboard"
          id="panel-clipboard"
          role="tabpanel"
          aria-labelledby="tab-clipboard"
          initial={{ opacity: 0, x: -10 }}
          animate={{ opacity: 1, x: 0 }}
          exit={{ opacity: 0, x: 10 }}
          transition={{ duration: 0.2 }}
        >
          <Suspense fallback={<div className="py-8 text-[var(--dc-text-muted)]">Loading...</div>}>
          <ClipboardTab
            appState={appState}
            refreshTrigger={clipboardRefreshTrigger}
            onOpenViewFull={openViewFull}
            onOpenSnippetEditor={openSnippetEditor}
            onOpenClipClearConfirm={() => setClipClearConfirmVisible(true)}
            onOpenClipEntryDeleteConfirm={openClipEntryDeleteConfirm}
          />
          </Suspense>
        </motion.div>
      )}
      {activeTab === 3 && (
        <motion.div
          key="script"
          id="panel-script"
          role="tabpanel"
          aria-labelledby="tab-script"
          initial={{ opacity: 0, x: -10 }}
          animate={{ opacity: 1, x: 0 }}
          exit={{ opacity: 0, x: 10 }}
          transition={{ duration: 0.2 }}
        >
          <Suspense fallback={<div className="py-8 text-[var(--dc-text-muted)]">Loading...</div>}>
            <ScriptTab appState={appState} />
          </Suspense>
        </motion.div>
      )}
      {activeTab === 4 && (
        <motion.div
          key="appearance"
          id="panel-appearance"
          role="tabpanel"
          aria-labelledby="tab-appearance"
          initial={{ opacity: 0, x: -10 }}
          animate={{ opacity: 1, x: 0 }}
          exit={{ opacity: 0, x: 10 }}
          transition={{ duration: 0.2 }}
        >
          <Suspense fallback={<div className="py-8 text-[var(--dc-text-muted)]">Loading...</div>}>
            <AppearanceTab />
          </Suspense>
        </motion.div>
      )}
      {activeTab === 5 && (
        <motion.div
          key="analytics"
          initial={{ opacity: 0, x: -10 }}
          animate={{ opacity: 1, x: 0 }}
          exit={{ opacity: 0, x: 10 }}
          transition={{ duration: 0.2 }}
        >
          <Suspense fallback={<div className="py-8 text-[var(--dc-text-muted)]">Loading...</div>}>
            <AnalyticsTab />
          </Suspense>
        </motion.div>
      )}
      {activeTab === 6 && (
        <motion.div
          key="log"
          id="panel-log"
          role="tabpanel"
          aria-labelledby="tab-log"
          initial={{ opacity: 0, x: -10 }}
          animate={{ opacity: 1, x: 0 }}
          exit={{ opacity: 0, x: 10 }}
          transition={{ duration: 0.2 }}
        >
          <Suspense fallback={<div className="py-8 text-[var(--dc-text-muted)]">Loading...</div>}>
            <LogTab />
          </Suspense>
        </motion.div>
      )}
      </AnimatePresence>

      {snippetEditorVisible && (
        <Suspense fallback={null}>
          <SnippetEditor
            visible={snippetEditorVisible}
            mode={snippetEditorMode}
            category={snippetEditorCategory}
            snippetIdx={snippetEditorIdx}
            initialSnippet={getInitialSnippet()}
            prefill={snippetEditorPrefill}
            onSave={handleSnippetSave}
            onCancel={closeSnippetEditor}
          />
        </Suspense>
      )}

      <DeleteConfirm
        visible={deleteConfirmVisible}
        message={deleteConfirmMessage}
        onConfirm={handleDeleteConfirm}
        onCancel={() => {
          setDeleteConfirmVisible(false);
          restoreGhostFollowerAlwaysOnTop();
        }}
      />

      <ViewFull
        visible={viewFullVisible}
        content={viewFullContent}
        onClose={() => {
          setViewFullVisible(false);
          setViewFullEditMeta(null);
          setViewFullClipboardMeta(null);
          restoreGhostFollowerAlwaysOnTop();
        }}
        onEdit={
          viewFullEditMeta
            ? (cat, idx) => {
                openSnippetEditor("edit", cat, idx);
                setViewFullVisible(false);
                setViewFullEditMeta(null);
              }
            : undefined
        }
        editMeta={viewFullEditMeta}
        onPromote={viewFullClipboardMeta ? handleViewFullPromote : undefined}
        onPin={
          viewFullClipboardMeta && !viewFullClipboardMeta.canPromote
            ? handleViewFullPinPromoted
            : undefined
        }
        onUnpin={
          viewFullEditMeta?.source === "ghost-pinned"
            ? handleViewFullUnpinSnippet
            : undefined
        }
        onCopy={handleViewFullCopy}
        onDelete={
          viewFullClipboardMeta
            ? handleViewFullDelete
            : viewFullEditMeta
            ? handleViewFullDeleteSnippet
            : undefined
        }
        canPin={(() => {
          const match = findSnippetByContent(viewFullContent);
          return !match?.pinned;
        })()}
        canPromote={viewFullClipboardMeta?.canPromote ?? true}
      />

      <ClipClearConfirm
        visible={clipClearConfirmVisible}
        onConfirm={handleClipClearConfirm}
        onCancel={() => setClipClearConfirmVisible(false)}
      />

      <ClipEntryDeleteConfirm
        visible={clipEntryDeleteConfirmVisible}
        message="Are you sure you want to delete this clipboard entry?"
        onConfirm={handleClipEntryDeleteConfirm}
        onCancel={() => {
          setClipEntryDeleteConfirmVisible(false);
          setClipEntryDeleteConfirmIdx(-1);
          restoreGhostFollowerAlwaysOnTop();
        }}
      />

      <VariableInput
        visible={variableInputVisible}
        data={variableInputData}
        onOk={handleVariableInputOk}
        onCancel={handleVariableInputCancel}
      />

      <CommandPalette
        visible={commandPaletteVisible}
        appState={appState}
        onClose={() => setCommandPaletteVisible(false)}
        onOpenSnippetEditor={openSnippetEditor}
      />
      </div>
    </div>
  );
}

export default App;
