import React, { useState, useEffect } from "react";
import { useEditor, EditorContent } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { Markdown } from "tiptap-markdown";
import CodeMirror from "@uiw/react-codemirror";
import { markdown } from "@codemirror/lang-markdown";
import { MermaidExtension } from "./MermaidExtension";
import { Button } from "../ui/button";
import { Eye, Code, Save, Trash2, Sparkles, ChevronRight, ChevronLeft, FileText, ExternalLink, Link2 } from "lucide-react";
import { getTaurpc } from "../../lib/taurpc";
import { KmsNoteDto, SearchResultDto, KmsLinksDto } from "../../bindings";
import { Tooltip } from "../ui/tooltip";
import { cn } from "../../lib/utils";

interface KmsEditorProps {
    path: string;
    initialContent: string;
    onSave: (content: string) => Promise<void>;
    onDelete?: () => void;
    onRename?: (newName: string) => Promise<void>;
    onSelectNote?: (path: string) => void;
}

export default function KmsEditor({ path, initialContent, onSave, onDelete, onRename, onSelectNote }: KmsEditorProps) {
    const [mode, setMode] = useState<"wysiwyg" | "source">("wysiwyg");
    const [content, setContent] = useState(initialContent);
    const [isDirty, setIsDirty] = useState(false);
    const [saving, setSaving] = useState(false);
    const [isRenaming, setIsRenaming] = useState(false);
    const [tempTitle, setTempTitle] = useState("");
    const [similarNotes, setSimilarNotes] = useState<SearchResultDto[]>([]);
    const [links, setLinks] = useState<KmsLinksDto | null>(null);
    const [loadingSimilar, setLoadingSimilar] = useState(false);
    const [loadingLinks, setLoadingLinks] = useState(false);
    const [showSidebar, setShowSidebar] = useState(false);
    const [sidebarTab, setSidebarTab] = useState<"similar" | "backlinks">("backlinks");

    const filename = path.split(/[\\/]/).pop() || "";

    const editor = useEditor({
        extensions: [
            StarterKit.configure({
                codeBlock: false,
            }),
            MermaidExtension,
            Markdown.configure({
                html: false,
                tightLists: true,
            }) as any,
        ],
        content: initialContent,
        editorProps: {
            attributes: {
                class: "prose dark:prose-invert prose-sm sm:prose-base lg:prose-lg focus:outline-none max-w-none min-h-[500px] p-8 bg-dc-bg",
            },
        },
        onUpdate: ({ editor }) => {
            const md = (editor.storage as any).markdown.getMarkdown();
            setContent(md);
            setIsDirty(true);
        },
    });

    useEffect(() => {
        if (editor && initialContent !== content) {
            editor.commands.setContent(initialContent);
            setContent(initialContent);
            setIsDirty(false);
        }
    }, [path, initialContent, editor]);

    useEffect(() => {
        const fetchLinks = async () => {
            setLoadingLinks(true);
            try {
                const res = await getTaurpc().kms_get_note_links(path);
                setLinks(res);
            } catch (error) {
                console.error("Failed to fetch links:", error);
            } finally {
                setLoadingLinks(false);
            }
        };
        fetchLinks();
    }, [path]);

    useEffect(() => {
        const fetchSimilar = async () => {
            if (!content || content.length < 30) {
                setSimilarNotes([]);
                return;
            }
            setLoadingSimilar(true);
            try {
                // Perform a text-based semantic search
                const results = await getTaurpc().kms_search_semantic(content, "text", 5, "Semantic");
                // Filter out the current note itself
                setSimilarNotes(results.filter((r: SearchResultDto) => r.entity_id !== path));
            } catch (error) {
                console.error("Failed to fetch similar notes:", error);
            } finally {
                setLoadingSimilar(false);
            }
        };

        const timer = setTimeout(fetchSimilar, 1500); // 1.5s debounce for performance
        return () => clearTimeout(timer);
    }, [content, path]);

    const handleSave = async () => {
        if (!isDirty || saving) return;
        setSaving(true);
        try {
            await onSave(content);
            setIsDirty(false);
        } finally {
            setSaving(false);
        }
    };

    const handleRenameSubmit = async () => {
        if (!tempTitle || tempTitle === filename || !onRename) {
            setIsRenaming(false);
            return;
        }
        try {
            await onRename(tempTitle);
            setIsRenaming(false);
        } catch (error) {
            console.error("Rename failed", error);
            // Optionally show toast here if we have access to it, 
            // but KmsApp can handle it via the promise
        }
    };

    const toggleMode = () => {
        if (mode === "wysiwyg") {
            setMode("source");
        } else {
            // When switching back to WYSIWYG, content from CodeMirror is already in 'content' state
            editor?.commands.setContent(content);
            setMode("wysiwyg");
        }
    };

    return (
        <div className="flex flex-col h-full overflow-hidden bg-dc-bg">
            {/* Editor Toolbar */}
            <header className="flex items-center justify-between px-6 py-3 border-b border-dc-border bg-dc-bg-secondary/30 backdrop-blur-sm">
                <div className="flex items-center gap-4">
                    <div className="flex items-center gap-2 bg-dc-bg-hover p-1 rounded-lg border border-dc-border">
                        <Button
                            variant={mode === "wysiwyg" ? "secondary" : "ghost"}
                            size="sm"
                            className="h-8 gap-2 px-3 text-xs"
                            onClick={() => mode !== "wysiwyg" && toggleMode()}
                        >
                            <Eye size={14} />
                            Visual
                        </Button>
                        <Button
                            variant={mode === "source" ? "secondary" : "ghost"}
                            size="sm"
                            className="h-8 gap-2 px-3 text-xs"
                            onClick={() => mode !== "source" && toggleMode()}
                        >
                            <Code size={14} />
                            Source
                        </Button>
                    </div>

                    <div className="h-6 w-[1px] border-l border-dc-border mx-1" />

                    {isRenaming ? (
                        <input
                            autoFocus
                            className="bg-dc-bg-hover border border-dc-accent rounded px-2 py-0.5 text-xs text-dc-text outline-none w-48 font-medium"
                            value={tempTitle}
                            onChange={(e) => setTempTitle(e.target.value)}
                            onBlur={handleRenameSubmit}
                            onKeyDown={(e) => {
                                if (e.key === "Enter") handleRenameSubmit();
                                if (e.key === "Escape") setIsRenaming(false);
                            }}
                        />
                    ) : (
                        <div
                            className="group flex items-center gap-2 cursor-pointer"
                            onClick={() => {
                                setTempTitle(filename);
                                setIsRenaming(true);
                            }}
                        >
                            <span className="text-sm font-medium text-dc-text truncate max-w-[300px] hover:text-dc-accent transition-colors">
                                {filename}
                            </span>
                        </div>
                    )}
                </div>

                <div className="flex items-center gap-3">
                    <div className="flex items-center gap-1 border-r border-dc-border pr-3 mr-1">
                        <Tooltip content="Delete Note">
                            <Button
                                variant="ghost"
                                size="sm"
                                className="h-8 w-8 p-0 text-dc-text-muted hover:text-red-400 hover:bg-red-400/10"
                                onClick={onDelete}
                            >
                                <Trash2 size={16} />
                            </Button>
                        </Tooltip>
                    </div>

                    <Button
                        variant={isDirty ? "default" : "secondary"}
                        size="sm"
                        className={cn(
                            "h-8 gap-2 px-4 text-xs transition-all shadow-sm",
                            isDirty ? "bg-dc-accent text-white" : "opacity-80"
                        )}
                        onClick={handleSave}
                        disabled={!isDirty || saving}
                    >
                        <Save size={14} className={saving ? "animate-pulse" : ""} />
                        {saving ? "Saving..." : isDirty ? "Save Changes" : "Saved"}
                    </Button>
                </div>
            </header>

            {/* Editor Area */}
            <div className="flex-1 overflow-y-auto scrollbar-thin scrollbar-thumb-dc-border scrollbar-track-transparent bg-dc-bg">
                {mode === "wysiwyg" ? (
                    <div className="mx-auto max-w-4xl px-4 py-8 min-h-full">
                        <EditorContent editor={editor} />
                    </div>
                ) : (
                    <div className="h-full font-mono text-sm">
                        <CodeMirror
                            value={content}
                            height="100%"
                            theme="dark" // TODO: We should match DigiCore theme
                            extensions={[markdown()]}
                            onChange={(value) => {
                                setContent(value);
                                setIsDirty(true);
                            }}
                            className="h-full border-0"
                            basicSetup={{
                                lineNumbers: true,
                                highlightActiveLine: true,
                                foldGutter: true,
                            }}
                        />
                    </div>
                )}
            </div>

            {/* Sidebar overlay */}
            {showSidebar && (
                <div className="absolute top-[61px] right-0 bottom-[33px] w-80 bg-dc-bg-secondary/95 backdrop-blur-xl border-l border-dc-border z-40 flex flex-col shadow-2xl animate-in slide-in-from-right duration-300">
                    <div className="p-0 border-b border-dc-border flex items-center justify-between bg-dc-accent/5">
                        <div className="flex flex-1">
                            <button
                                className={cn(
                                    "flex-1 py-3 px-4 text-[10px] font-bold uppercase tracking-wider transition-all flex items-center justify-center gap-2",
                                    sidebarTab === "backlinks" ? "bg-dc-accent/10 text-dc-accent border-b-2 border-dc-accent" : "text-dc-text-muted hover:text-dc-text"
                                )}
                                onClick={() => setSidebarTab("backlinks")}
                            >
                                <Link2 size={14} />
                                Backlinks ({links?.incoming.length || 0})
                            </button>
                            <button
                                className={cn(
                                    "flex-1 py-3 px-4 text-[10px] font-bold uppercase tracking-wider transition-all flex items-center justify-center gap-2",
                                    sidebarTab === "similar" ? "bg-dc-accent/10 text-dc-accent border-b-2 border-dc-accent" : "text-dc-text-muted hover:text-dc-text"
                                )}
                                onClick={() => setSidebarTab("similar")}
                            >
                                <Sparkles size={14} />
                                Similar
                            </button>
                        </div>
                        <Button
                            variant="ghost"
                            size="sm"
                            className="h-10 w-10 p-0 hover:bg-dc-bg-hover rounded-none border-l border-dc-border"
                            onClick={() => setShowSidebar(false)}
                        >
                            <ChevronRight size={14} />
                        </Button>
                    </div>

                    <div className="flex-1 overflow-y-auto p-3 custom-scrollbar">
                        {sidebarTab === "similar" ? (
                            <div className="space-y-3">
                                {loadingSimilar && similarNotes.length === 0 ? (
                                    <div className="flex flex-col items-center justify-center py-10 gap-3 opacity-50">
                                        <div className="animate-spin rounded-full h-5 w-5 border-b-2 border-dc-accent" />
                                        <span className="text-[10px]">Analyzing context...</span>
                                    </div>
                                ) : similarNotes.length === 0 ? (
                                    <div className="text-center py-10 px-4">
                                        <p className="text-[10px] text-dc-text-muted italic">Keep writing to discover related notes and snippets.</p>
                                    </div>
                                ) : (
                                    similarNotes.map((result, idx) => (
                                        <ResultItem key={`${result.entity_id}-${idx}`} result={result} onSelect={onSelectNote} />
                                    ))
                                )}
                            </div>
                        ) : (
                            <div className="space-y-3">
                                {loadingLinks ? (
                                    <div className="flex flex-col items-center justify-center py-10 gap-3 opacity-50">
                                        <div className="animate-spin rounded-full h-5 w-5 border-b-2 border-dc-accent" />
                                        <span className="text-[10px]">Mapping connections...</span>
                                    </div>
                                ) : !links || links.incoming.length === 0 ? (
                                    <div className="text-center py-10 px-4">
                                        <p className="text-[10px] text-dc-text-muted italic">No backlinks found for this note yet.</p>
                                    </div>
                                ) : (
                                    links.incoming.map((note) => (
                                        <div
                                            key={note.path}
                                            className="group p-3 rounded-xl border border-dc-border bg-dc-bg/50 hover:border-dc-accent/50 hover:bg-dc-accent/5 transition-all cursor-pointer shadow-sm"
                                            onClick={() => onSelectNote?.(note.path)}
                                        >
                                            <div className="flex items-center gap-2 mb-2">
                                                <div className="p-1 bg-dc-accent/10 rounded text-dc-accent">
                                                    <Link2 size={10} />
                                                </div>
                                                <span className="text-[10px] font-semibold text-dc-accent uppercase tracking-tighter">
                                                    REFERENCE
                                                </span>
                                            </div>
                                            <h4 className="text-xs font-medium text-dc-text group-hover:text-dc-accent transition-colors truncate">
                                                {note.title}
                                            </h4>
                                            {note.preview && (
                                                <p className="text-[10px] text-dc-text-muted line-clamp-2 mt-1 opacity-70">
                                                    {note.preview}
                                                </p>
                                            )}
                                        </div>
                                    ))
                                )}
                            </div>
                        )}
                    </div>
                </div>
            )}

            {/* Toggle Switch Overlay for Sidebar */}
            {!showSidebar && (
                <div className="absolute top-1/2 -right-1 transform -translate-y-1/2 z-50">
                    <Button
                        size="sm"
                        className="h-10 w-6 p-0 rounded-l-xl rounded-r-none bg-dc-accent text-white shadow-xl hover:w-8 transition-all"
                        onClick={() => setShowSidebar(true)}
                    >
                        <ChevronLeft size={16} />
                    </Button>
                </div>
            )}

            {/* Bottom Status bar */}
            <footer className="px-6 py-2 border-t border-dc-border bg-dc-bg-secondary/40 flex justify-between items-center text-[10px] text-dc-text-muted">
                <div className="flex gap-4">
                    <span>{content.split(/\s+/).filter(Boolean).length} words</span>
                    <span>{content.length} characters</span>
                </div>
                <div className="flex items-center gap-2">
                    <div className={cn(
                        "w-1.5 h-1.5 rounded-full",
                        isDirty ? "bg-amber-400 animate-pulse" : "bg-emerald-400"
                    )} />
                    <span className="opacity-70">{isDirty ? "Draft - Unsaved changes" : "All changes saved locally"}</span>
                </div>
            </footer>
        </div>
    );
}
function ResultItem({ result, onSelect }: { result: SearchResultDto, onSelect?: (path: string) => void }) {
    return (
        <div
            className="group p-3 rounded-xl border border-dc-border bg-dc-bg/50 hover:border-dc-accent/50 hover:bg-dc-accent/5 transition-all cursor-pointer shadow-sm"
            onClick={() => onSelect?.(result.entity_id)}
        >
            <div className="flex items-start justify-between mb-2">
                <div className="flex items-center gap-1.5">
                    <div className="p-1 bg-dc-accent/10 rounded text-dc-accent">
                        <FileText size={10} />
                    </div>
                    <span className="text-[10px] font-semibold text-dc-accent uppercase tracking-tighter">
                        {result.entity_type}
                    </span>
                </div>
                <div className="text-[9px] text-dc-text-muted font-mono bg-dc-bg-hover px-1.5 py-0.5 rounded border border-dc-border">
                    {Math.round((1 - result.distance) * 100)}% Match
                </div>
            </div>
            <h4 className="text-xs font-medium text-dc-text mb-1 group-hover:text-dc-accent transition-colors truncate">
                {result.entity_id.split(/[\\/]/).pop()?.replace(".md", "")}
            </h4>
            {result.metadata && (
                <p className="text-[10px] text-dc-text-muted line-clamp-2 leading-relaxed opacity-70">
                    {result.metadata}
                </p>
            )}
            <div className="mt-2 flex justify-end opacity-0 group-hover:opacity-100 transition-opacity">
                <ExternalLink size={10} className="text-dc-accent" />
            </div>
        </div>
    );
}
