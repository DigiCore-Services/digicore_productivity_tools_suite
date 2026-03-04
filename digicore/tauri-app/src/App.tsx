import React, { useCallback, useEffect, useState, lazy, Suspense } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { invoke } from "@tauri-apps/api/core";
import { emitTo, listen } from "@tauri-apps/api/event";
import { onOpenUrl } from "@tauri-apps/plugin-deep-link";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  Library,
  Settings,
  ClipboardList,
  Code,
  BarChart3,
  FileText,
} from "lucide-react";
import type { AppState, PendingVariableInput, Snippet } from "./types";
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
  VariableInput,
} from "./components/modals";
import { CommandPalette } from "./components/CommandPalette";
import { initNotificationActionListener } from "./lib/notifications";
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
  } | null>(null);

  const [clipClearConfirmVisible, setClipClearConfirmVisible] = useState(false);

  const [variableInputVisible, setVariableInputVisible] = useState(false);
  const [variableInputData, setVariableInputData] =
    useState<PendingVariableInput | null>(null);

  const [commandPaletteVisible, setCommandPaletteVisible] = useState(false);

  const [clipboardRefreshTrigger, setClipboardRefreshTrigger] = useState(0);

  const loadAppState = useCallback(async () => {
    try {
      const state = (await invoke("get_app_state")) as AppState;
      setAppState(state);
      return state;
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

  const closeSnippetEditor = useCallback(() => {
    setSnippetEditorVisible(false);
  }, []);

  const handleSnippetSave = useCallback(
    async (snippet: Snippet) => {
      if (!snippet.trigger.trim()) {
        setLibraryStatusFn("Trigger is required", true);
        return;
      }
      try {
        if (snippetEditorMode === "add") {
          await invoke("add_snippet", {
            category: snippet.category || "General",
            snippet,
          });
        } else {
          await invoke("update_snippet", {
            category: snippetEditorCategory,
            snippetIdx: snippetEditorIdx,
            snippet,
          });
        }
        await invoke("save_library");
        await invoke("save_settings").catch(() => {});
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

  const handleDeleteConfirm = useCallback(async () => {
    try {
      await invoke("delete_snippet", {
        category: deleteConfirmCat,
        snippetIdx: deleteConfirmIdx,
      });
      await invoke("save_library");
      await invoke("save_settings").catch(() => {});
      setDeleteConfirmVisible(false);
      const state = await loadAppState();
      if (state) {
        setAppState(state);
        if (state.library && Object.keys(state.library).length > 0) {
          await syncLibraryToSqlite(state.library);
        }
      }
      setLibraryStatusFn("Snippet deleted and saved");
    } catch (e) {
      setLibraryStatusFn("Delete failed: " + String(e), true);
    }
  }, [deleteConfirmCat, deleteConfirmIdx, loadAppState, setLibraryStatusFn]);

  const openViewFull = useCallback(
    (
      content: string,
      editMeta?: { category: string; snippetIdx: number } | null
    ) => {
      setViewFullContent(content);
      setViewFullEditMeta(editMeta ?? null);
      setViewFullVisible(true);
    },
    []
  );

  const handleClipClearConfirm = useCallback(async () => {
    try {
      await invoke("clear_clipboard_history");
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
        await invoke("save_ui_prefs", { last_tab: lastTab, column_order: cols });
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
      await invoke("submit_variable_input", { values });
      setVariableInputVisible(false);
      setVariableInputData(null);
    } catch (e) {
      console.error("Variable input submit:", e);
    }
  }, []);

  const handleVariableInputCancel = useCallback(async () => {
    try {
      await invoke("cancel_variable_input");
    } catch {
      /* ignore */
    }
    setVariableInputVisible(false);
    setVariableInputData(null);
  }, []);

  const showVariableInputModal = useCallback(async () => {
    try {
      const data = (await invoke("get_pending_variable_input")) as
        | PendingVariableInput
        | null;
      if (data) {
        setVariableInputData(data);
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

  useEffect(() => {
    const unlistens: (() => void)[] = [];
    listen<string>("ghost-follower-view-full", (e) => {
      openViewFull(e.payload ?? "");
    }).then((fn) => unlistens.push(fn));
    listen<{ category: string; snippetIdx: number }>(
      "ghost-follower-edit",
      (e) => {
        const { category, snippetIdx } = e.payload ?? {};
        if (category != null && snippetIdx != null) {
          openSnippetEditor("edit", category, snippetIdx);
        }
      }
    ).then((fn) => unlistens.push(fn));
    listen<{ content: string; trigger: string }>(
      "ghost-follower-promote",
      (e) => {
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
    return () => unlistens.forEach((u) => u());
  }, [openViewFull, openSnippetEditor, appState?.categories]);

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
        const prefs = (await invoke("get_ui_prefs")) as {
          last_tab?: number;
          column_order?: string[];
        };
        const cols = Array.isArray(prefs.column_order) && prefs.column_order.length
          ? prefs.column_order.filter((c) => DEFAULT_COLUMNS.includes(c))
          : [...DEFAULT_COLUMNS];
        for (const c of DEFAULT_COLUMNS) {
          if (!cols.includes(c)) cols.push(c);
        }
        setColumnOrder(cols);
        const lastTab = Math.min(0 | (prefs.last_tab ?? 0), 5);
        setActiveTab(lastTab);
      } catch {
        /* ignore */
      }
      const state = await loadAppState();
      if (state?.library && Object.keys(state.library).length > 0) {
        syncLibraryToSqlite(state.library).catch(() => {});
      }
    })();
  }, [loadAppState]);

  useEffect(() => {
    try {
      new WebviewWindow("ghost-suggestor", {
        url: "ghost-suggestor.html",
        title: "Ghost Suggestor",
        width: 320,
        height: 260,
        decorations: false,
        transparent: true,
        alwaysOnTop: true,
        visible: false,
      });
      const followerWin = new WebviewWindow("ghost-follower", {
        url: "ghost-follower.html",
        title: "Ghost Follower",
        width: 280,
        height: 420,
        decorations: false,
        transparent: true,
        alwaysOnTop: true,
        visible: false,
      });
      followerWin.once("tauri://created", () => {
        setTimeout(() => emitTo("ghost-follower", "ghost-follower-update"), 800);
      });
    } catch {
      /* ghost windows may fail in dev without Tauri */
    }
  }, []);

  const tabs = [
    { id: "library", label: "Text Expansion Library", icon: Library },
    { id: "config", label: "Configurations and Settings", icon: Settings },
    { id: "clipboard", label: "Clipboard History", icon: ClipboardList },
    { id: "script", label: "Scripting Engine Library", icon: Code },
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
            <ConfigTab appState={appState} onConfigLoaded={() => {}} />
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
      {activeTab === 5 && (
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
        onCancel={() => setDeleteConfirmVisible(false)}
      />

      <ViewFull
        visible={viewFullVisible}
        content={viewFullContent}
        onClose={() => {
          setViewFullVisible(false);
          setViewFullEditMeta(null);
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
      />

      <ClipClearConfirm
        visible={clipClearConfirmVisible}
        onConfirm={handleClipClearConfirm}
        onCancel={() => setClipClearConfirmVisible(false)}
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
