import React, { useState, useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { getTaurpc } from "./lib/taurpc";
import { resolveTheme } from "./lib/theme";
import { Toaster } from "./components/ui/toaster";
import { useToast } from "./components/ui/use-toast";
import { Book, FolderOpen, Search, Settings, Plus, Star, FileText, Sun, Moon, AlertCircle, RefreshCw, Check, Terminal, Activity } from "lucide-react";
import { Button } from "./components/ui/button";
import KmsEditor from "./components/kms/KmsEditor";
import KmsLogViewer from "./components/kms/KmsLogViewer";
import VaultSettingsModal from "./components/modals/VaultSettingsModal";
import { ViewFull } from "./components/modals/ViewFull";
import { ImageViewerModal } from "./components/modals/ImageViewerModal";
import FileExplorer from "./components/kms/FileExplorer";
import { KmsNoteDto, KmsFileSystemItemDto, KmsLogDto } from "./bindings";
import { ClipEntry } from "./types";

export default function KmsApp() {
    const { toast } = useToast();
    const [vaultPath, setVaultPath] = useState<string | null>(null);
    const [initializing, setInitializing] = useState(true);
    const [notes, setNotes] = useState<KmsNoteDto[]>([]);
    const [activeNote, setActiveNote] = useState<KmsNoteDto | null>(null);
    const [activeContent, setActiveContent] = useState<string>("");
    const [theme, setTheme] = useState<"light" | "dark">("light");
    const [themeOverride, setThemeOverride] = useState<"light" | "dark" | null>(null);
    const [view, setView] = useState<"explorer" | "search" | "favorites" | "logs">("explorer");
    const [searchQuery, setSearchQuery] = useState("");
    const [searchResults, setSearchResults] = useState<any[]>([]); // SearchResultDto
    const [searchLoading, setSearchLoading] = useState(false);
    const [searchMode, setSearchMode] = useState<"Hybrid" | "Semantic" | "Keyword">("Hybrid");
    const [isSettingsOpen, setIsSettingsOpen] = useState(false);
    const [syncStatus, setSyncStatus] = useState<string>("Idle");
    const [viewFullVisible, setViewFullVisible] = useState(false);
    const [viewFullContent, setViewFullContent] = useState("");
    const [viewFullEditMeta, setViewFullEditMeta] = useState<{ category: string; snippetIdx: number } | null>(null);
    const [viewFullClipboardMeta, setViewFullClipboardMeta] = useState<{ id: number; canPromote: boolean; trigger?: string } | null>(null);
    const [imageViewerVisible, setImageViewerVisible] = useState(false);
    const [imageViewerCurrent, setImageViewerCurrent] = useState<ClipEntry | null>(null);
    const [imageViewerContext, setImageViewerContext] = useState<ClipEntry[]>([]);
    const searchAbortController = useRef<AbortController | null>(null);
    const [vaultStructure, setVaultStructure] = useState<KmsFileSystemItemDto | null>(null);
    const [sidebarWidth, setSidebarWidth] = useState(() => {
        const saved = localStorage.getItem("kms-sidebar-width");
        return saved ? Number.parseInt(saved) : 280;
    });
    const [isResizing, setIsResizing] = useState(false);

    useEffect(() => {
        const init = async () => {
            try {
                const path = await getTaurpc().kms_initialize();
                setVaultPath(path);
                refreshNotes();
                refreshStructure();

                // Initialize theme from global settings
                const globalThemePref = localStorage.getItem("digicore-theme") || "light";
                setTheme(resolveTheme(globalThemePref));
            } catch (error) {
                toast({
                    title: "KMS Initialization Error",
                    description: String(error),
                    variant: "destructive",
                });
            } finally {
                setInitializing(false);
            }
        };
        init();

        // Listen for global theme changes
        const unlistenPromise = listen("digicore-theme-changed", (event: any) => {
            const { theme: newResolvedTheme } = event.payload;
            if (newResolvedTheme) {
                setTheme(newResolvedTheme);
            }
        });

        // Listen for sync events
        const unlistenSyncStatus = listen("kms-sync-status", (event: any) => {
            setSyncStatus(event.payload as string);
        });
        const unlistenSyncComplete = listen("kms-sync-complete", () => {
            refreshNotes();
            refreshStructure();
        });

        return () => {
            unlistenPromise.then(f => f());
            unlistenSyncStatus.then(f => f());
            unlistenSyncComplete.then(f => f());
        };
    }, [toast]);

    const refreshNotes = async () => {
        try {
            const list = await getTaurpc().kms_list_notes();
            setNotes(list);
        } catch (error) {
            console.error("Failed to list notes:", error);
        }
    };

    useEffect(() => {
        if (!isResizing) return;

        const handleMouseMove = (e: MouseEvent) => {
            const newWidth = Math.max(200, Math.min(600, e.clientX));
            setSidebarWidth(newWidth);
        };

        const handleMouseUp = () => {
            setIsResizing(false);
            localStorage.setItem("kms-sidebar-width", sidebarWidth.toString());
        };

        window.addEventListener("mousemove", handleMouseMove);
        window.addEventListener("mouseup", handleMouseUp);
        return () => {
            window.removeEventListener("mousemove", handleMouseMove);
            window.removeEventListener("mouseup", handleMouseUp);
        };
    }, [isResizing, sidebarWidth]);

    const refreshStructure = async () => {
        try {
            const structure = await getTaurpc().kms_get_vault_structure();
            setVaultStructure(structure);
        } catch (error) {
            console.error("Failed to get vault structure:", error);
        }
    };

    const handleSelectNote = async (note: KmsNoteDto) => {
        try {
            const content = await getTaurpc().kms_load_note(note.path);
            setActiveContent(content);
            setActiveNote(note);
        } catch (error) {
            toast({
                title: "Error Loading Note",
                description: String(error),
                variant: "destructive",
            });
        }
    };

    const handleCreateNote = async (parentPath?: string) => {
        if (!vaultPath) return;

        const targetDir = parentPath || `${vaultPath}\\notes`;
        const title = `Untitled Note ${notes.length + 1}`;
        const fileName = `${title}.md`;
        const path = `${targetDir}\\${fileName}`;
        const initialContent = `# ${title}\n\nStart writing here...`;

        try {
            await getTaurpc().kms_save_note(path, initialContent);
            await refreshStructure();
            await refreshNotes();
            // Find the newly created note in the list
            const newList = await getTaurpc().kms_list_notes();
            const newNote = newList.find(n => n.path === path);
            if (newNote) {
                handleSelectNote(newNote);
            }
        } catch (error) {
            toast({
                title: "Error Creating Note",
                description: String(error),
                variant: "destructive",
            });
        }
    };

    const handleCreateFolder = async (parentPath?: string) => {
        if (!vaultPath) return;
        const targetParent = parentPath || vaultPath;
        const name = window.prompt("Enter notebook name:");
        if (!name) return;

        const path = `${targetParent}\\${name}`;
        try {
            await (getTaurpc() as any).kms_create_folder(path);
            refreshStructure();
            toast({
                title: "Folder Created",
                description: `Created notebook "${name}"`,
            });
        } catch (error) {
            toast({
                title: "Error Creating Folder",
                description: String(error),
                variant: "destructive",
            });
        }
    };

    const handleSaveNote = async (content: string) => {
        if (!activeNote) return;
        try {
            await getTaurpc().kms_save_note(activeNote.path, content);
            setActiveContent(content);
            refreshNotes();
            refreshStructure();
            toast({
                title: "Note Saved",
                description: "Your changes have been persisted locally.",
            });
        } catch (error) {
            toast({
                title: "Save Failed",
                description: String(error),
                variant: "destructive",
            });
        }
    };

    const handleDeleteNote = async (path?: string) => {
        const targetPath = path || activeNote?.path;
        if (!targetPath) return;

        const confirmed = await window.confirm(`Are you sure you want to delete this note?`);
        if (!confirmed) return;

        try {
            await getTaurpc().kms_delete_note(targetPath);
            if (activeNote?.path === targetPath) {
                setActiveNote(null);
                setActiveContent("");
            }
            refreshNotes();
            refreshStructure();
            toast({
                title: "Note Deleted",
                description: "The note and its local file have been removed.",
            });
        } catch (error) {
            toast({
                title: "Delete Failed",
                description: String(error),
                variant: "destructive",
            });
        }
    };

    const handleRenameNote = async (newName: string, oldPath?: string) => {
        const targetPath = oldPath || activeNote?.path;
        if (!targetPath) return;

        try {
            const newPath = await getTaurpc().kms_rename_note(targetPath, newName);
            if (activeNote?.path === targetPath) {
                const updatedNote = { ...activeNote, path: newPath, title: newName.replace(/\.md$/i, "") };
                setActiveNote(updatedNote);
            }
            await refreshNotes();
            await refreshStructure();
            toast({
                title: "Note Renamed",
                description: `Successfully renamed to ${newName}`,
            });
            return newPath;
        } catch (error) {
            toast({
                title: "Rename Failed",
                description: String(error),
                variant: "destructive",
            });
            throw error;
        }
    };

    const handleSearch = async () => {
        if (!searchQuery.trim()) {
            setSearchResults([]);
            return;
        }

        if (searchAbortController.current) {
            searchAbortController.current.abort();
        }
        const abortController = new AbortController();
        searchAbortController.current = abortController;

        setSearchLoading(true);
        try {
            const results = await getTaurpc().kms_search_semantic(searchQuery, "text", 15, searchMode);
            if (abortController.signal.aborted) return;
            setSearchResults(results);
        } catch (error) {
            if (abortController.signal.aborted) return;
            toast({
                title: "Search Failed",
                description: String(error),
                variant: "destructive",
            });
        } finally {
            if (!abortController.signal.aborted) {
                setSearchLoading(false);
            }
        }
    };

    const cancelSearch = () => {
        if (searchAbortController.current) {
            searchAbortController.current.abort();
            searchAbortController.current = null;
        }
        setSearchLoading(false);
    };

    const handleRenameFolder = async (oldPath: string) => {
        const currentName = oldPath.split(/[/\\]/).pop() || "";
        const newName = window.prompt("Rename notebook/folder:", currentName);
        if (!newName || newName === currentName) return;

        if (!window.confirm(`Are you sure you want to rename '${currentName}' to '${newName}'? This will update all notes within this folder.`)) {
            return;
        }

        try {
            await (getTaurpc() as any).kms_rename_folder(oldPath, newName);
            toast({ title: "Folder Renamed", description: `Renamed to ${newName}` });
            refreshStructure();
            refreshNotes();
        } catch (err: any) {
            toast({ title: "Rename Failed", description: String(err), variant: "destructive" });
        }
    };

    const handleDeleteFolder = async (path: string) => {
        const name = path.split(/[/\\]/).pop() || "folder";
        if (!window.confirm(`CRITICAL: Are you sure you want to delete '${name}' and ALL its contents? This cannot be undone.`)) {
            return;
        }

        try {
            await (getTaurpc() as any).kms_delete_folder(path);
            toast({ title: "Folder Deleted", description: name });
            refreshStructure();
            refreshNotes();
            if (activeNote && activeNote.path.startsWith(path)) {
                setActiveNote(null);
            }
        } catch (err: any) {
            toast({ title: "Delete Failed", description: String(err), variant: "destructive" });
        }
    };

    const handleMoveItem = async (path: string, newParentPath: string) => {
        const itemName = path.split(/[/\\]/).pop() || "item";
        const folderName = newParentPath.split(/[/\\]/).pop() || "root";

        if (!window.confirm(`Move '${itemName}' to '${folderName}'?`)) {
            return;
        }

        try {
            await (getTaurpc() as any).kms_move_item(path, newParentPath);
            toast({ title: "Item Moved", description: `Moved ${itemName} to ${folderName}` });
            refreshStructure();
            refreshNotes();
        } catch (err: any) {
            toast({ title: "Move Failed", description: String(err), variant: "destructive" });
        }
    };

    const handleNavigateToResult = async (result: any) => {
        if (result.entity_type === "note") {
            const note = notes.find(n => n.path === result.entity_id);
            if (note) handleSelectNote(note);
        } else if (result.entity_type === "snippet") {
            try {
                // Try from snippet field first (newly populated), then metadata
                const content = result.snippet || (() => {
                    const meta = JSON.parse(result.metadata || "{}");
                    return meta.content || result.metadata || "";
                })();

                const meta = JSON.parse(result.metadata || "{}");
                setViewFullContent(content);
                setViewFullEditMeta({
                    category: meta.category || "General",
                    snippetIdx: typeof meta.snippetIdx === "number" ? meta.snippetIdx : -1
                });
                setViewFullVisible(true);
            } catch (e) {
                // Fallback for older or malformed metadata
                const content = result.snippet || result.metadata || "";
                setViewFullContent(content);
                setViewFullEditMeta(null);
                setViewFullVisible(true);
            }
        } else if (result.entity_type === "clipboard") {
            try {
                setSearchLoading(true);
                const id = Number.parseInt(result.entity_id);
                const entry = await getTaurpc().get_clip_entry_by_id(id);

                if (result.modality === "image" && entry) {
                    setImageViewerCurrent(entry);
                    setImageViewerContext([entry]);
                    setImageViewerVisible(true);
                    return;
                }

                // Fallback to text modal
                const content = entry?.content || result.snippet || (() => {
                    const meta = JSON.parse(result.metadata || "{}");
                    return meta.content || result.metadata || "";
                })();

                const meta = JSON.parse(result.metadata || "{}");
                setViewFullContent(content);
                setViewFullClipboardMeta({
                    id: entry?.id || (typeof meta.id === "number" ? meta.id : Number.parseInt(result.entity_id)),
                    canPromote: entry ? (entry.entry_type !== "image" && entry.entry_type !== "extracted_text") : (meta.entry_type !== "image"),
                    trigger: (content || "").slice(0, 20).replace(/\s/g, "").trim() || "clip"
                });
                setViewFullEditMeta(null);
                setViewFullVisible(true);
            } catch (err) {
                console.error("Failed to handle clipboard navigation:", err);
                toast({ title: "Error", description: "Failed to open clipboard entry", variant: "destructive" });
            } finally {
                setSearchLoading(false);
            }
        }
    };

    const handleRepairDatabase = async () => {
        if (!window.confirm("This will surgically reset the AI search index. Your actual notes and snippets will NOT be deleted. Proceed?")) {
            return;
        }
        try {
            await (getTaurpc() as any).kms_repair_database();
            toast({
                title: "KMS Index Reset",
                description: "AI tables cleared. Please RESTART the app now to finish the repair.",
            });
        } catch (error) {
            toast({
                title: "Repair Failed",
                description: String(error),
                variant: "destructive",
            });
        }
    };

    const currentTheme = themeOverride || theme;

    const toggleTheme = () => {
        setThemeOverride(currentTheme === "dark" ? "light" : "dark");
    };

    if (initializing) {
        return (
            <div className="flex items-center justify-center h-screen bg-dc-bg text-dc-text">
                <div className="flex flex-col items-center gap-4">
                    <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-dc-accent" />
                    <p className="text-sm font-medium animate-pulse">Initializing Knowledge Suite...</p>
                </div>
            </div>
        );
    }

    return (
        <div className="flex h-screen bg-dc-bg text-dc-text font-sans overflow-hidden select-none" data-theme={currentTheme}>
            {/* Sidebar */}
            <aside
                className="border-r border-dc-border flex flex-col bg-dc-bg-secondary/30 backdrop-blur-md relative"
                style={{ width: sidebarWidth }}
            >
                <div className="p-4 border-b border-dc-border flex items-center gap-2">
                    <div className="p-1.5 bg-dc-accent rounded-lg text-white">
                        < Book size={18} />
                    </div>
                    <span className="font-semibold tracking-tight">DigiCore KMS</span>
                </div>
                <div className="flex items-center gap-4 px-4 py-2">
                    <div className="flex items-center gap-1.5 px-3 py-1 bg-dc-bg-secondary/50 rounded-full border border-dc-border">
                        <div className={`w-1.5 h-1.5 rounded-full ${syncStatus === "Idle" ? "bg-dc-green" : syncStatus.toLowerCase().includes("error") || syncStatus.toLowerCase().includes("failed") ? "bg-dc-red" : "bg-dc-amber animate-pulse"}`} />
                        <span className="text-[10px] text-dc-text-muted uppercase tracking-wider font-bold">
                            Sync: <span className={syncStatus !== "Idle" ? "text-dc-text" : ""}>{syncStatus}</span>
                        </span>
                    </div>
                </div>

                <div className="flex-1 overflow-y-auto p-4 space-y-6">
                    <div>
                        <div className="text-[10px] font-bold text-dc-text-muted uppercase tracking-wider mb-2 px-2">Navigation</div>
                        <nav className="space-y-1">
                            <Button
                                variant={view === "explorer" ? "secondary" : "ghost"}
                                size="sm"
                                className={`w-full justify-start gap-2 h-9 px-2 ${view === "explorer" ? "bg-dc-bg-hover text-dc-accent font-medium" : "text-dc-text-muted hover:bg-dc-bg-hover"}`}
                                onClick={() => setView("explorer")}
                            >
                                <FolderOpen size={16} className={view === "explorer" ? "text-dc-accent" : "text-dc-text-muted"} />
                                <span className="text-sm">Explorer</span>
                            </Button>
                            <Button
                                variant={view === "search" ? "secondary" : "ghost"}
                                size="sm"
                                className={`w-full justify-start gap-2 h-9 px-2 ${view === "search" ? "bg-dc-bg-hover text-dc-accent font-medium" : "text-dc-text-muted hover:bg-dc-bg-hover"}`}
                                onClick={() => setView("search")}
                            >
                                <Search size={16} className={view === "search" ? "text-dc-accent" : "text-dc-text-muted"} />
                                <span className="text-sm">Semantic Search</span>
                            </Button>
                            <Button
                                variant={view === "favorites" ? "secondary" : "ghost"}
                                size="sm"
                                className={`w-full justify-start gap-2 h-9 px-2 ${view === "favorites" ? "bg-dc-bg-hover text-dc-accent font-medium" : "text-dc-text-muted hover:bg-dc-bg-hover"}`}
                                onClick={() => setView("favorites")}
                            >
                                <Star size={16} className={view === "favorites" ? "text-dc-accent" : "text-dc-text-muted"} />
                                <span className="text-sm">Favorites</span>
                            </Button>
                            <Button
                                variant={view === "logs" ? "secondary" : "ghost"}
                                size="sm"
                                className={`w-full justify-start gap-2 h-9 px-2 ${view === "logs" ? "bg-dc-bg-hover text-dc-accent font-medium" : "text-dc-text-muted hover:bg-dc-bg-hover"}`}
                                onClick={() => setView("logs")}
                            >
                                <Activity size={16} className={view === "logs" ? "text-dc-accent" : "text-dc-text-muted"} />
                                <span className="text-sm">Operational Logs</span>
                            </Button>
                        </nav>
                    </div>

                    <div className="flex-1 overflow-y-auto pt-0">
                        {view === "explorer" ? (
                            <div className="p-4 space-y-4">
                                <div>
                                    <div className="text-[10px] font-bold text-dc-text-muted uppercase tracking-wider mb-2 px-2 flex justify-between items-center">
                                        Vault Explorer
                                        <div className="flex items-center gap-2">
                                            <div title="New Notebook/Folder">
                                                <FolderOpen
                                                    size={14}
                                                    className="cursor-pointer hover:text-dc-accent transition-colors"
                                                    onClick={() => handleCreateFolder()}
                                                />
                                            </div>
                                            <div title="New Note">
                                                <Plus
                                                    size={14}
                                                    className="cursor-pointer hover:text-dc-accent transition-colors"
                                                    onClick={() => handleCreateNote()}
                                                />
                                            </div>
                                            <div title="Force Reindex Vault">
                                                <RefreshCw
                                                    size={14}
                                                    className={`cursor-pointer transition-colors ${syncStatus !== "Idle" ? "text-dc-amber animate-spin" : "hover:text-dc-accent"}`}
                                                    onClick={async () => {
                                                        try {
                                                            await getTaurpc().kms_reindex_all();
                                                            refreshNotes();
                                                            refreshStructure();
                                                            toast({ title: "Reindex Triggered", description: "Indexing vault contents." });
                                                        } catch (err: any) {
                                                            toast({ title: "Reindex Failed", description: String(err), variant: "destructive" });
                                                        }
                                                    }}
                                                />
                                            </div>
                                        </div>
                                    </div>
                                    <div className="flex-1 overflow-y-auto min-h-0">
                                        <FileExplorer
                                            structure={vaultStructure}
                                            activeNote={activeNote}
                                            onSelectNote={handleSelectNote}
                                            onCreateNote={handleCreateNote}
                                            onCreateFolder={handleCreateFolder}
                                            onRenameNote={async (oldPath: string, newName: string) => {
                                                if (window.confirm(`Rename note to '${newName}'?`)) {
                                                    await handleRenameNote(newName, oldPath);
                                                }
                                            }}
                                            onDeleteNote={async (path: string) => {
                                                if (window.confirm("Are you sure you want to delete this note?")) {
                                                    await handleDeleteNote(path);
                                                }
                                            }}
                                            onRenameFolder={handleRenameFolder}
                                            onDeleteFolder={handleDeleteFolder}
                                            onMoveItem={handleMoveItem}
                                        />
                                    </div>
                                </div>
                            </div>
                        ) : view === "search" ? (
                            <div className="p-4 space-y-4">
                                <div className="text-[10px] font-bold text-dc-text-muted uppercase tracking-wider mb-2 px-2">Knowledge Search</div>
                                <div className="relative group px-1">
                                    <Search size={14} className="absolute left-4 top-1/2 -translate-y-1/2 text-dc-text-muted group-focus-within:text-dc-accent transition-colors" />
                                    <input
                                        autoFocus
                                        placeholder="Recall anything..."
                                        className="w-full bg-dc-bg-secondary text-dc-text border border-dc-border rounded-xl py-2 pl-10 pr-4 text-xs focus:outline-none focus:border-dc-accent/50 focus:bg-dc-bg-hover/50 transition-all font-medium placeholder:text-dc-text-muted/50"
                                        value={searchQuery}
                                        onChange={(e) => setSearchQuery(e.target.value)}
                                        onKeyDown={(e) => e.key === "Enter" && handleSearch()}
                                    />
                                    <div className="absolute right-3 top-1/2 -translate-y-1/2 flex gap-1">
                                        {searchQuery && (
                                            <div className="text-[8px] bg-dc-bg font-mono border border-dc-border rounded px-1 text-dc-text-muted opacity-50 shadow-sm">ENTER</div>
                                        )}
                                    </div>
                                </div>

                                {/* Search Mode Selector */}
                                <div className="flex bg-dc-bg-secondary/50 rounded-lg p-0.5 border border-dc-border mx-1">
                                    {(["Hybrid", "Semantic", "Keyword"] as const).map((mode) => (
                                        <button
                                            key={mode}
                                            onClick={() => setSearchMode(mode)}
                                            className={`flex-1 py-1 text-[10px] font-bold uppercase tracking-tight rounded-md transition-all ${searchMode === mode
                                                ? "bg-dc-accent text-white shadow-sm"
                                                : "text-dc-text-muted hover:text-dc-text hover:bg-dc-bg-hover"
                                                }`}
                                        >
                                            {mode}
                                        </button>
                                    ))}
                                </div>

                                <div className="space-y-1 pb-10">
                                    {searchLoading ? (
                                        <div className="flex flex-col items-center justify-center py-12 gap-3 opacity-50">
                                            <div className="animate-spin rounded-full h-5 w-5 border-b-2 border-dc-accent" />
                                            <span className="text-[10px] uppercase font-bold tracking-[0.2em] text-dc-accent">Thinking...</span>
                                            <Button variant="ghost" size="sm" onClick={cancelSearch} className="mt-2 text-xs border border-dc-border hover:bg-dc-bg-hover">
                                                Cancel Search
                                            </Button>
                                        </div>
                                    ) : searchResults.length === 0 && searchQuery ? (
                                        <div className="text-center py-12 px-6">
                                            <div className="w-10 h-10 bg-dc-bg-hover rounded-full flex items-center justify-center mx-auto mb-3 opacity-50">
                                                <Search size={16} />
                                            </div>
                                            <p className="text-[10px] text-dc-text-muted italic leading-relaxed">We couldn't find any direct or semantic matches for your query.</p>
                                        </div>
                                    ) : (
                                        searchResults.map((result, idx) => (
                                            <Button
                                                key={`${result.entity_id}-${idx}`}
                                                variant="ghost"
                                                size="sm"
                                                className="w-full justify-start gap-2 py-3 px-3 h-auto hover:bg-dc-bg-hover group border border-transparent hover:border-dc-accent/20 rounded-xl transition-all mb-1"
                                                onClick={() => handleNavigateToResult(result)}
                                            >
                                                <div className="flex flex-col items-start text-left w-full gap-1">
                                                    <div className="flex items-center justify-between w-full">
                                                        <div className="flex items-center gap-1.5 overflow-hidden flex-1 mr-2">
                                                            <span className="text-[9px] font-bold text-dc-accent uppercase tracking-tighter opacity-80 whitespace-nowrap">
                                                                {result.entity_type === "clipboard" ? `CLIPBOARD (${result.modality})` : result.entity_type}
                                                            </span>
                                                            {result.modality === "image" && <div className="p-0.5 bg-dc-accent/10 rounded text-dc-accent"><Book size={10} strokeWidth={3} /></div>}
                                                            {result.modality === "text" && result.entity_type === "clipboard" && <div className="p-0.5 bg-dc-accent/5 rounded text-dc-accent/60"><FileText size={10} strokeWidth={3} /></div>}
                                                        </div>
                                                        <span className="text-[8px] opacity-40 font-mono italic shrink-0">{Math.round((1 - result.distance) * 100)}% Match</span>
                                                    </div>

                                                    <span className="text-sm font-medium truncate group-hover:text-dc-accent transition-colors">
                                                        {result.entity_type === "note"
                                                            ? result.entity_id.split(/[\\/]/).pop()?.replace(".md", "")
                                                            : result.entity_type === "snippet"
                                                                ? `Snippet: ${result.entity_id}`
                                                                : (() => {
                                                                    if (result.entity_type === "clipboard") {
                                                                        try {
                                                                            const meta = JSON.parse(result.metadata || "{}");
                                                                            return meta.process_name || `Clipboard ${result.entity_id}`;
                                                                        } catch {
                                                                            return `Clipboard ${result.entity_id}`;
                                                                        }
                                                                    }
                                                                    return result.entity_id;
                                                                })()
                                                        }
                                                    </span>

                                                    {(result.snippet || result.metadata) && (
                                                        <span className="text-[10px] text-dc-text-muted mt-1 leading-normal line-clamp-3 opacity-70 group-hover:opacity-100 transition-opacity">
                                                            {result.snippet || (() => {
                                                                if (result.entity_type === "snippet") {
                                                                    try {
                                                                        const meta = JSON.parse(result.metadata || "{}");
                                                                        return meta.content || result.metadata;
                                                                    } catch {
                                                                        return result.metadata;
                                                                    }
                                                                }
                                                                if (result.entity_type === "clipboard" && result.metadata) {
                                                                    try {
                                                                        const meta = JSON.parse(result.metadata);
                                                                        return (meta.content || "").substring(0, 150);
                                                                    } catch {
                                                                        return result.metadata.substring(0, 150);
                                                                    }
                                                                }
                                                                return result.metadata;
                                                            })()}
                                                        </span>
                                                    )}
                                                </div>
                                            </Button>
                                        ))
                                    )}
                                </div>
                            </div>
                        ) : view === "logs" ? (
                            <KmsLogViewer />
                        ) : (
                            <div className="p-4 flex flex-col items-center justify-center py-20 opacity-30 text-center">
                                <Star size={32} className="mb-4" />
                                <span className="text-xs">Favorites placeholder</span>
                            </div>
                        )}
                    </div>
                </div >

                <div className="p-4 border-t border-dc-border bg-dc-bg-secondary/50 space-y-2">
                    <div className="flex flex-col gap-1">
                        <Button
                            variant="ghost"
                            size="sm"
                            className="w-full justify-start gap-2 h-9 px-2 text-dc-text-muted hover:text-dc-text"
                            onClick={() => setIsSettingsOpen(true)}
                        >
                            <Settings size={16} className="text-dc-text-muted" />
                            <span className="text-sm">Vault Settings</span>
                        </Button>
                        <Button
                            variant="ghost"
                            size="sm"
                            className="w-full justify-start gap-2 h-7 px-2 text-[10px] text-dc-accent hover:text-dc-accent hover:bg-dc-accent/10"
                            onClick={handleRepairDatabase}
                        >
                            <AlertCircle size={12} />
                            <span>Repair KMS Index</span>
                        </Button>
                    </div>
                    <div className="px-2 pb-1 flex items-center justify-between text-[10px] text-dc-text-muted opacity-50">
                        <span className="truncate flex-1" title={vaultPath || ""}>{vaultPath}</span>
                        {syncStatus !== "Idle" && (
                            <div className="flex items-center gap-1.5 text-dc-accent animate-pulse">
                                <RefreshCw size={10} className="animate-spin" />
                                <span>{syncStatus}</span>
                            </div>
                        )}
                    </div>
                </div>

                {/* Resize Handle */}
                <div
                    className={`absolute top-0 right-0 w-1 h-full cursor-col-resize transition-colors z-50 ${isResizing ? "bg-dc-accent" : "hover:bg-dc-accent/30"}`}
                    onMouseDown={(e) => {
                        e.preventDefault();
                        setIsResizing(true);
                    }}
                />
            </aside >

            {/* Main Content Area */}
            < main className="flex-1 flex flex-col bg-dc-bg relative" >
                {/* Theme Toggle Overlay */}
                < div className="absolute top-4 right-4 z-50" >
                    <Button
                        variant="ghost"
                        size="sm"
                        className="h-9 w-9 p-0 bg-dc-bg-secondary/50 backdrop-blur-md border border-dc-border shadow-sm hover:bg-dc-bg-hover"
                        onClick={toggleTheme}
                        title={`Switch to ${currentTheme === "dark" ? "light" : "dark"} mode`}
                    >
                        {currentTheme === "dark" ? (
                            <Sun size={18} className="text-amber-400" />
                        ) : (
                            <Moon size={18} className="text-slate-700" />
                        )}
                    </Button>
                </div >

                {
                    activeNote ? (
                        <KmsEditor
                            path={activeNote.path}
                            initialContent={activeContent}
                            onSave={handleSaveNote}
                            onDelete={handleDeleteNote}
                            onRename={(newName) => handleRenameNote(newName).then(() => { })
                            }
                            onSelectNote={(path) => {
                                const note = notes.find(n => n.path === path);
                                if (note) handleSelectNote(note);
                            }}
                        />
                    ) : (
                        <div className="flex-1 flex items-center justify-center p-8">
                            <div className="max-w-md w-full text-center space-y-4">
                                <div className="mx-auto w-16 h-16 bg-dc-accent/10 rounded-2xl flex items-center justify-center text-dc-accent mb-6">
                                    <Book size={32} />
                                </div>
                                <h2 className="text-2xl font-bold tracking-tight text-dc-text">Select a note to get started</h2>
                                <p className="text-dc-text-muted text-sm leading-relaxed">
                                    Every note you create is a local Markdown file stored securely in your vault.
                                    Use the sidebar to explore your knowledge graph.
                                </p>
                                <div className="pt-6 flex justify-center gap-3">
                                    <Button
                                        size="sm"
                                        className="bg-dc-accent hover:bg-dc-accent/90 text-white gap-2 px-6"
                                        onClick={() => handleCreateNote()}
                                    >
                                        <Plus size={16} />
                                        Create New Note
                                    </Button>
                                </div>
                            </div>
                        </div>
                    )}

                {/* Visual Accent */}
                {
                    !activeNote && (
                        <>
                            <div className="absolute top-0 right-0 w-64 h-64 bg-dc-accent/5 blur-[120px] pointer-events-none rounded-full" />
                            <div className="absolute bottom-0 left-0 w-96 h-96 bg-dc-accent/5 blur-[160px] pointer-events-none rounded-full" />
                        </>
                    )
                }
            </main >

            <Toaster />
            <VaultSettingsModal
                isOpen={isSettingsOpen}
                onClose={() => setIsSettingsOpen(false)}
                currentPath={vaultPath}
                onPathUpdated={(newPath) => {
                    setVaultPath(newPath);
                    refreshNotes();
                }}
            />
            <ViewFull
                visible={viewFullVisible}
                content={viewFullContent}
                onClose={() => setViewFullVisible(false)}
                onEdit={(cat, idx) => {
                    getTaurpc().ghost_follower_request_edit(cat, idx as any);
                    setViewFullVisible(false);
                }}
                editMeta={viewFullEditMeta}
                onPromote={viewFullClipboardMeta ? async () => {
                    if (viewFullClipboardMeta.canPromote) {
                        const trigger = viewFullClipboardMeta.trigger || "clip";
                        await getTaurpc().ghost_follower_request_promote(viewFullContent, trigger);
                        setViewFullVisible(false);
                    }
                } : undefined}
                onCopy={async () => {
                    try {
                        await getTaurpc().copy_to_clipboard(viewFullContent);
                        toast({ title: "Snippet Copied", description: "Content copied to clipboard." });
                    } catch (err) {
                        toast({ title: "Copy Failed", description: String(err), variant: "destructive" });
                    }
                }}
                onDelete={viewFullEditMeta ? async () => {
                    if (window.confirm("Are you sure you want to delete this snippet?")) {
                        try {
                            await getTaurpc().delete_snippet(viewFullEditMeta.category, viewFullEditMeta.snippetIdx as any);
                            setViewFullVisible(false);
                            toast({ title: "Snippet Deleted", description: "The snippet has been removed." });
                        } catch (err) {
                            toast({ title: "Delete Failed", description: String(err), variant: "destructive" });
                        }
                    }
                } : viewFullClipboardMeta ? async () => {
                    if (window.confirm("Are you sure you want to delete this clipboard entry?")) {
                        try {
                            await getTaurpc().delete_clip_entry_by_id(viewFullClipboardMeta.id);
                            setViewFullVisible(false);
                            toast({ title: "Entry Deleted", description: "Clipboard entry removed." });
                        } catch (err) {
                            toast({ title: "Delete Failed", description: String(err), variant: "destructive" });
                        }
                    }
                } : undefined}
            />
            <ImageViewerModal
                isOpen={imageViewerVisible}
                onClose={() => setImageViewerVisible(false)}
                currentImage={imageViewerCurrent}
                allImages={imageViewerContext}
                onNavigate={(img) => setImageViewerCurrent(img)}
                onDeleteSuccess={() => {
                    // Update search results if needed
                }}
            />
        </div >
    );
}
