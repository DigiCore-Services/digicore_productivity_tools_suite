import React, { useState, useEffect } from "react";
import { useEditor, EditorContent } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { Markdown } from "tiptap-markdown";
import CodeMirror from "@uiw/react-codemirror";
import { markdown } from "@codemirror/lang-markdown";
import { Image } from "@tiptap/extension-image";
import { convertFileSrc } from "@tauri-apps/api/core";
import { MermaidExtension } from "./MermaidExtension";
import { MathExtension } from "./MathExtension";
import { AdmonitionExtension } from "./AdmonitionExtension";
import { FrontmatterExtension } from "./FrontmatterExtension";
import { Button } from "../ui/button";
import { Eye, Code, Save, Trash2, Sparkles, ChevronRight, ChevronLeft, FileText, ExternalLink, Link2, Sun, Moon, Maximize2, Minimize2, History } from "lucide-react";
import { WikiLinkExtension } from "./WikiLinkExtension";
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
    onOpenSkillEditor?: () => void;
    isZenMode?: boolean;
    onToggleZenMode?: () => void;
    onToggleTheme?: () => void;
    onToggleHistory?: () => void;
    isHistoryOpen?: boolean;
    currentTheme?: "light" | "dark";
    vaultPath?: string | null;
}

export default function KmsEditor({ path, initialContent, onSave, onDelete, onRename, onSelectNote, onOpenSkillEditor, isZenMode, onToggleZenMode, onToggleHistory, isHistoryOpen, onToggleTheme, currentTheme, vaultPath }: KmsEditorProps) {
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
            FrontmatterExtension,
            MermaidExtension,
            MathExtension,
            AdmonitionExtension,
            WikiLinkExtension.configure({
                onLinkClick: (target) => {
                    if (onSelectNote) {
                        onSelectNote(target);
                    }
                },
            }),
            Image.configure({
                allowBase64: true,
                HTMLAttributes: {
                    class: "rounded-xl border border-dc-border shadow-lg max-w-full h-auto my-4 mx-auto block",
                },
            }).configure({
                HTMLAttributes: {
                    "data-type": "vault-image",
                },
            }),
            Markdown.configure({
                html: true,
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
            // Post-process: convert the wrapped nodes back to standard Markdown format
            let finalMd = unwrapFrontmatter(md);
            finalMd = unwrapMath(finalMd);
            finalMd = unwrapAdmonitions(finalMd);
            finalMd = unwrapWikiLinks(finalMd);
            finalMd = unwrapImages(finalMd);

            setContent(finalMd);
            setIsDirty(true);
        },
    });

    useEffect(() => {
        if (editor && initialContent !== content) {
            // Pre-process: detect specialized blocks and wrap them for Tiptap extensions
            let processed = wrapFrontmatter(initialContent);
            processed = wrapMath(processed);
            processed = wrapAdmonitions(processed);
            processed = wrapWikiLinks(processed);
            processed = wrapImages(processed, vaultPath);

            editor.commands.setContent(processed);
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
            <header className={cn(
                "flex items-center justify-between px-6 py-3 border-b border-dc-border bg-dc-bg-secondary/30 backdrop-blur-sm transition-all",
                isZenMode && "opacity-20 hover:opacity-100 border-transparent bg-transparent"
            )}>
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

                <div className="flex items-center gap-4">

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

                    {filename.toLowerCase() === "skill.md" && onOpenSkillEditor && (
                        <div className="flex items-center gap-1 border-r border-dc-border pr-3 mr-1">
                            <Tooltip content="Manage Skill Metadata">
                                <Button
                                    variant="ghost"
                                    size="sm"
                                    className="h-8 gap-2 px-3 text-xs text-dc-accent hover:bg-dc-accent/10 border border-dc-accent/20"
                                    onClick={onOpenSkillEditor}
                                >
                                    <Sparkles size={14} />
                                    Edit Skill
                                </Button>
                            </Tooltip>
                        </div>
                    )}

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
                        {saving ? "Saving..." : isDirty ? "Save Changes" : isZenMode ? "" : "Saved"}
                    </Button>

                    <div className="flex items-center gap-2 border-l border-dc-border pl-4 ml-2">

                        {onToggleTheme && !isZenMode && (
                            <Button
                                variant="ghost"
                                size="sm"
                                className="h-8 w-8 p-0 text-dc-text-muted hover:bg-dc-bg-hover"
                                onClick={onToggleTheme}
                                title={`Switch to ${currentTheme === "dark" ? "light" : "dark"} mode`}
                            >
                                {currentTheme === "dark" ? (
                                    <Sun size={16} className="text-amber-400" />
                                ) : (
                                    <Moon size={16} className="text-slate-400" />
                                )}
                            </Button>
                        )}
                        {onToggleZenMode && (
                            <Tooltip content={isZenMode ? "Exit Zen Mode (Alt+Z)" : "Enter Zen Mode (Alt+Z)"}>
                                <Button
                                    variant="ghost"
                                    size="sm"
                                    className={cn(
                                        "h-8 gap-2 px-3 transition-all border border-transparent self-center",
                                        isZenMode ? "bg-dc-accent text-white hover:bg-dc-accent/90 border-dc-accent" : "text-dc-text-muted hover:bg-dc-accent/10 hover:text-dc-accent hover:border-dc-accent/30"
                                    )}
                                    onClick={onToggleZenMode}
                                >
                                    {isZenMode ? <Minimize2 size={16} /> : <Maximize2 size={16} />}
                                    <span className="text-[10px] font-bold uppercase tracking-widest px-1">
                                        {isZenMode ? "Exit Zen" : "Zen Mode"}
                                    </span>
                                </Button>

                            </Tooltip>
                        )}
                        {onToggleHistory && !isZenMode && (
                            <Tooltip content={isHistoryOpen ? "Hide History" : "View Versions"}>
                                <Button
                                    variant="ghost"
                                    size="sm"
                                    className={cn(
                                        "h-8 w-8 p-0 transition-all border border-transparent",
                                        isHistoryOpen ? "bg-dc-accent/20 text-dc-accent border-dc-accent/40" : "text-dc-text-muted hover:bg-dc-bg-hover hover:text-dc-accent"
                                    )}
                                    onClick={onToggleHistory}
                                >
                                    <History size={16} />
                                </Button>
                            </Tooltip>
                        )}

                    </div>
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
            {!isZenMode && (
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
            )}
        </div>
    );
}

// --- Helper Functions for Wiki-link Handling ---

function wrapWikiLinks(md: string): string {
    if (!md) return md;
    // Look for [[link]] but NOT inside existing HTML tags or code blocks
    // Simple version first: replace all [[...]] that are not already in a data-target
    return md.replace(/\[\[([^\]]+)\]\]/g, (match, target) => {
        // Basic check to see if we are already inside a span we created
        // (Though wrap is usually called on fresh markdown)
        return `<span data-type="wiki-link" data-target="${target}">[[${target}]]</span>`;
    });
}

function unwrapWikiLinks(md: string): string {
    if (!md) return md;
    return md.replace(/<span\s+[^>]*data-type="wiki-link"[^>]*data-target="([^"]+)"[^>]*>\[\[.*?\]\]<\/span>/g, '[[$1]]');
}

// --- Helper Functions for Image Handling ---

function wrapImages(md: string, vaultPath: string | null | undefined): string {
    if (!md) return md;
    // Look for ![alt](src)
    return md.replace(/!\[([^\]]*)\]\(([^)]+)\)/g, (match, alt, src) => {
        if (src.startsWith('http') || src.startsWith('data:')) {
            return match;
        }

        // It's a local path. If we have a vaultPath, try to resolve it.
        if (vaultPath) {
            // Normalize path (handle relative to vault root)
            let fullPath = src;
            if (!src.includes(':') && !src.startsWith('/') && !src.startsWith('\\')) {
                // Assume relative to vault root for now, or we could resolve relative to the current note
                // For simplicity, let's assume all vault images are relative to vault root
                fullPath = vaultPath + (vaultPath.endsWith('/') || vaultPath.endsWith('\\') ? '' : '/') + src;
            } else if (src.startsWith('/') || src.startsWith('\\')) {
                fullPath = vaultPath + src;
            }

            try {
                const assetUrl = convertFileSrc(fullPath);
                return `<img src="${assetUrl}" alt="${alt}" data-path="${src}" data-type="vault-image" />`;
            } catch (e) {
                console.error("Failed to convert image path:", e);
                return match;
            }
        }
        return match;
    });
}

function unwrapImages(md: string): string {
    if (!md) return md;
    // Look for our specific img tag with data-path
    return md.replace(/<img\s+[^>]*data-type="vault-image"[^>]*data-path="([^"]+)"[^>]*alt="([^"]*)"[^>]*\/?>/g, '![$2]($1)');
}

// --- Helper Functions for Frontmatter Handling ---

function wrapFrontmatter(md: string): string {
    if (!md) return md;
    const trimmed = md.trim();
    if (trimmed.startsWith('---')) {
        const parts = trimmed.split('---');
        if (parts.length >= 3) {
            const frontmatter = parts[1].trim();
            const remainder = parts.slice(2).join('---').trim();
            // Use PRE tag to preserve whitespace during Tiptap parsing
            return `<pre data-type="frontmatter">${frontmatter}</pre>\n\n${remainder}`;
        }
    }
    return md;
}

function unwrapFrontmatter(md: string): string {
    if (!md) return md;
    const regex = /<pre data-type="frontmatter">([\s\S]*?)<\/pre>/;
    const match = md.match(regex);
    if (match) {
        const fm = match[1].trim();
        const body = md.replace(regex, '').trim();
        return `---\n${fm}\n---\n\n${body}`;
    }

    // Fallback: if it was converted to Horizontal Rules by standard markdown rendering
    // This is a bit more complex to reverse accurately without a dedicated extension parser
    return md;
}

function wrapMath(md: string): string {
    if (!md) return md;
    // Replace block math $$ ... $$
    let processed = md.replace(/\$\$([\s\S]+?)\$\$/g, '<div data-type="math" data-display="true">$1</div>');
    // Replace inline math $ ... $ (avoiding $ in other contexts is tricky but this is a common approach)
    processed = processed.replace(/\$([^$]+?)\$/g, '<span data-type="math" data-display="false">$1</span>');
    return processed;
}

function unwrapMath(md: string): string {
    if (!md) return md;
    // Reverse display math
    let processed = md.replace(/<div data-type="math" data-display="true">([\s\S]+?)<\/div>/g, '$$\n$1\n$$');
    // Reverse inline math
    processed = processed.replace(/<span data-type="math" data-display="false">([\s\S]+?)<\/span>/g, '$$$1$$');
    return processed;
}

function wrapAdmonitions(md: string): string {
    if (!md) return md;
    // Support ::: type ... ::: blocks
    return md.replace(/:::(\w+)\n?([\s\S]+?)\n?:::/g, '<div data-type="admonition" data-admonition-type="$1">$2</div>');
}

function unwrapAdmonitions(md: string): string {
    if (!md) return md;
    return md.replace(/<div data-type="admonition" data-admonition-type="(\w+)">([\s\S]+?)<\/div>/g, ':::$1\n$2\n:::');
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
