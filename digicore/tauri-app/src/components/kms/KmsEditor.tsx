import React, { useState, useEffect, useCallback, useRef } from "react";
import { useEditor, EditorContent } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { Markdown } from "tiptap-markdown";
import CodeMirror from "@uiw/react-codemirror";
import { markdown } from "@codemirror/lang-markdown";
import { Image } from "@tiptap/extension-image";
import { MermaidExtension } from "./MermaidExtension";
import { MathExtension } from "./MathExtension";
import { AdmonitionExtension } from "./AdmonitionExtension";
import { FrontmatterExtension } from "./FrontmatterExtension";
import { Button } from "../ui/button";
import {
    Eye, Code, Save, Trash2, Sparkles, ChevronRight, ChevronLeft,
    FileText, ExternalLink, Link2, Sun, Moon, Maximize2, Minimize2,
    History, List, Globe, Network, Star, BookOpen, GripVertical, X,
} from "lucide-react";
import KmsLocalGraph3D from "./KmsLocalGraph3D";
import { WikiLinkExtension } from "./WikiLinkExtension";
import { getTaurpc } from "../../lib/taurpc";
import { KmsNoteDto, SearchResultDto, KmsLinksDto, SnippetLogicTestResultDto } from "../../bindings";
import { Tooltip } from "../ui/tooltip";
import { cn } from "../../lib/utils";
import KmsSmartTemplateModal from "./KmsSmartTemplateModal";
import KmsReferenceReadOnly from "./KmsReferenceReadOnly";
import { prepareMarkdownForTiptap } from "../../lib/kmsEditorPrepareMarkdown";
import { resolveMarkdownLinkAgainstNotePath } from "../../lib/kmsMarkdownLinkResolve";

interface KmsEditorProps {
    path: string;
    initialContent: string;
    onSave: (content: string) => Promise<void>;
    onDelete?: () => void;
    onRename?: (newName: string) => Promise<void>;
    onSelectNote?: (path: string) => void;
    onOpenSkillEditor?: () => void;
    /** When set with `onFavoriteChange`, shows a star toggle in the toolbar. */
    isFavorite?: boolean;
    onFavoriteChange?: (next: boolean) => Promise<void>;
    isZenMode?: boolean;
    onToggleZenMode?: () => void;
    onToggleTheme?: () => void;
    onToggleHistory?: () => void;
    isHistoryOpen?: boolean;
    currentTheme?: "light" | "dark";
    vaultPath?: string | null;
    /** Exposes latest markdown (editor state) for history diff / other tools. */
    workingCopyMarkdownRef?: React.MutableRefObject<(() => string) | null>;
    /** Open full KMS knowledge graph focused on this note (parent switches view). */
    onOpenGlobalGraph?: () => void;
    referenceNote?: { path: string; title: string } | null;
    referenceContent?: string | null;
    onClearReference?: () => void;
    onOpenReferenceWikiTarget?: (wikiTarget: string) => void;
    /** When `id` changes, inserts markdown at the cursor (WYSIWYG) or appends in source mode. */
    pendingMarkdownInsert?: { id: number; text: string } | null;
    onPendingMarkdownInsertConsumed?: () => void;
}

const REF_PANE_WIDTH_LS = "kms-reference-pane-width-v1";

function readReferencePaneWidth(): number {
    try {
        const raw = localStorage.getItem(REF_PANE_WIDTH_LS);
        if (!raw) return 340;
        const n = Number.parseInt(raw, 10);
        if (Number.isFinite(n) && n >= 240 && n <= 720) return n;
    } catch {
        /* ignore */
    }
    return 340;
}

export default function KmsEditor({
    path,
    initialContent,
    onSave,
    onDelete,
    onRename,
    onSelectNote,
    onOpenSkillEditor,
    isFavorite,
    onFavoriteChange,
    isZenMode,
    onToggleZenMode,
    onToggleHistory,
    isHistoryOpen,
    onToggleTheme,
    currentTheme,
    vaultPath,
    workingCopyMarkdownRef,
    onOpenGlobalGraph,
    referenceNote,
    referenceContent,
    onClearReference,
    onOpenReferenceWikiTarget,
    pendingMarkdownInsert,
    onPendingMarkdownInsertConsumed,
}: KmsEditorProps) {
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
    const [sidebarTab, setSidebarTab] = useState<"similar" | "backlinks" | "toc" | "spatial">("backlinks");
    const [smartTemplateVisible, setSmartTemplateVisible] = useState(false);
    const [smartTemplateData, setSmartTemplateData] = useState<SnippetLogicTestResultDto | null>(null);
    const [evaluatingTemplate, setEvaluatingTemplate] = useState(false);
    const [activeHeading, setActiveHeading] = useState<number>(-1);
    const [sidebarWidth, setSidebarWidth] = useState(380);
    const [isResizing, setIsResizing] = useState(false);
    const [favoriteBusy, setFavoriteBusy] = useState(false);
    const [referencePaneWidth, setReferencePaneWidth] = useState(readReferencePaneWidth);
    const [isResizingReference, setIsResizingReference] = useState(false);

    useEffect(() => {
        if (!workingCopyMarkdownRef) return;
        workingCopyMarkdownRef.current = () => content;
        return () => {
            workingCopyMarkdownRef.current = null;
        };
    }, [content, workingCopyMarkdownRef]);

    const startResizing = useCallback((e: React.MouseEvent) => {
        e.preventDefault();
        setIsResizing(true);
    }, []);

    const stopResizing = useCallback(() => {
        setIsResizing(false);
    }, []);

    const resize = useCallback((e: MouseEvent) => {
        if (isResizing) {
            const newWidth = window.innerWidth - e.clientX;
            if (newWidth > 260 && newWidth < 900) {
                setSidebarWidth(newWidth);
            }
        }
    }, [isResizing]);

    useEffect(() => {
        if (isResizing) {
            window.addEventListener("mousemove", resize);
            window.addEventListener("mouseup", stopResizing);
        }
        return () => {
            window.removeEventListener("mousemove", resize);
            window.removeEventListener("mouseup", stopResizing);
        };
    }, [isResizing, resize, stopResizing]);

    const stopResizingReference = useCallback(() => {
        setIsResizingReference(false);
    }, []);

    const resizeReference = useCallback((e: MouseEvent) => {
        if (!isResizingReference) return;
        const root = document.querySelector("[data-kms-editor-split-root]");
        const rect = root?.getBoundingClientRect();
        if (!rect) return;
        const newRefWidth = rect.right - e.clientX;
        if (newRefWidth >= 240 && newRefWidth <= 720) {
            setReferencePaneWidth(newRefWidth);
            try {
                localStorage.setItem(REF_PANE_WIDTH_LS, String(newRefWidth));
            } catch {
                /* ignore */
            }
        }
    }, [isResizingReference]);

    useEffect(() => {
        if (isResizingReference) {
            window.addEventListener("mousemove", resizeReference);
            window.addEventListener("mouseup", stopResizingReference);
        }
        return () => {
            window.removeEventListener("mousemove", resizeReference);
            window.removeEventListener("mouseup", stopResizingReference);
        };
    }, [isResizingReference, resizeReference, stopResizingReference]);

    const startResizingReference = useCallback((e: React.MouseEvent) => {
        e.preventDefault();
        setIsResizingReference(true);
    }, []);

    const filename = path.split(/[\\/]/).pop() || "";

    const onSelectNoteRef = useRef(onSelectNote);
    const onOpenReferenceWikiTargetRef = useRef(onOpenReferenceWikiTarget);
    const pathRef = useRef(path);
    useEffect(() => {
        onSelectNoteRef.current = onSelectNote;
        onOpenReferenceWikiTargetRef.current = onOpenReferenceWikiTarget;
        pathRef.current = path;
    }, [onSelectNote, onOpenReferenceWikiTarget, path]);

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
                onLinkClick: (target, e) => {
                    if (e.ctrlKey || e.metaKey) {
                        onOpenReferenceWikiTargetRef.current?.(target);
                    } else {
                        onSelectNoteRef.current?.(target);
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
            handleDOMEvents: {
                click: (_view, event) => {
                    const e = event as MouseEvent;
                    const t = e.target;
                    if (!(t instanceof Element)) return false;
                    const a = t.closest("a");
                    if (!a) return false;
                    const hrefRaw = a.getAttribute("href");
                    if (!hrefRaw) return false;

                    const resolved = resolveMarkdownLinkAgainstNotePath(pathRef.current, hrefRaw);
                    if (!resolved || !/\.md$/i.test(resolved)) return false;

                    e.preventDefault();
                    e.stopPropagation();

                    if (e.ctrlKey || e.metaKey) {
                        onOpenReferenceWikiTargetRef.current?.(resolved);
                    } else {
                        onSelectNoteRef.current?.(resolved);
                    }
                    return true;
                },
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
            const processed = prepareMarkdownForTiptap(initialContent, vaultPath);

            // Defer setContent to avoid flushSync warning during React lifecycle
            setTimeout(() => {
                if (editor) {
                    editor.commands.setContent(processed);
                    setContent(initialContent);
                    setIsDirty(false);
                }
            }, 0);
        }
    }, [path, initialContent, editor]);

    useEffect(() => {
        const p = pendingMarkdownInsert;
        if (!p || !editor) return;
        const { text } = p;
        if (mode === "wysiwyg") {
            editor.chain().focus().insertContent(text).run();
        } else {
            setContent((prev) => `${prev}${prev && !prev.endsWith("\n") ? "\n" : ""}${text}\n`);
            setIsDirty(true);
        }
        onPendingMarkdownInsertConsumed?.();
    }, [pendingMarkdownInsert, editor, mode, onPendingMarkdownInsertConsumed]);

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

    const headings = React.useMemo(() => {
        const lines = content.split('\n');
        const h: { level: number; text: string; index: number }[] = [];
        let headingCount = 0;
        for (const line of lines) {
            const match = line.match(/^(#{1,6})\s+(.*)$/);
            if (match) {
                h.push({
                    level: match[1].length,
                    text: match[2].trim().replace(/\[\[.*?\]\]/g, (m) => m.replace(/\[\[|\]\]/g, '')),
                    index: headingCount++
                });
            }
        }
        return h;
    }, [content]);

    const scrollToHeading = (index: number) => {
        if (mode === "wysiwyg") {
            const allHeadings = document.querySelectorAll(
                '.ProseMirror h1, .ProseMirror h2, .ProseMirror h3, .ProseMirror h4, .ProseMirror h5, .ProseMirror h6'
            );
            if (allHeadings[index]) {
                allHeadings[index].scrollIntoView({ behavior: 'smooth', block: 'start' });
                setActiveHeading(index);
            }
        } else {
            const lines = document.querySelectorAll('.cm-line');
            let count = 0;
            for (let i = 0; i < lines.length; i++) {
                const text = lines[i].textContent || "";
                const cleanText = text.replace(/[\u200B-\u200D\uFEFF]/g, ""); // strip zero-width chars if any
                if (/^#{1,6}\s+/.test(cleanText)) {
                    if (count === index) {
                        lines[i].scrollIntoView({ behavior: 'smooth', block: 'start' });
                        setActiveHeading(index);
                        break;
                    }
                    count++;
                }
            }
        }
    };

    // Scroll Spy Logic
    useEffect(() => {
        const editorArea = document.querySelector('.editor-scroll-area');
        if (!editorArea) return;

        const handleScroll = () => {
            if (sidebarTab !== "toc" || !showSidebar) return;

            const headingElements = mode === "wysiwyg"
                ? document.querySelectorAll('.ProseMirror h1, .ProseMirror h2, .ProseMirror h3, .ProseMirror h4, .ProseMirror h5, .ProseMirror h6')
                : document.querySelectorAll('.cm-line');

            let current = -1;
            let count = 0;

            if (mode === "wysiwyg") {
                headingElements.forEach((h, i) => {
                    const rect = h.getBoundingClientRect();
                    // If heading is near the top of the viewport area
                    if (rect.top < 200) {
                        current = i;
                    }
                });
            } else {
                for (let i = 0; i < headingElements.length; i++) {
                    const text = headingElements[i].textContent || "";
                    if (/^#{1,6}\s+/.test(text.replace(/[\u200B-\u200D\uFEFF]/g, ""))) {
                        const rect = headingElements[i].getBoundingClientRect();
                        if (rect.top < 200) {
                            current = count;
                        }
                        count++;
                    }
                }
            }
            setActiveHeading(current);
        };

        editorArea.addEventListener('scroll', handleScroll);
        return () => editorArea.removeEventListener('scroll', handleScroll);
    }, [mode, sidebarTab, showSidebar, content]);

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

    const handleEvaluateSmartTemplates = async (userValues: Record<string, string> | null = null) => {
        if (!editor && mode === "wysiwyg") return;
        if (evaluatingTemplate) return;

        let selectedText = "";
        let selectionRange = { from: 0, to: 0 };

        if (mode === "wysiwyg" && editor) {
            const { from, to } = editor.state.selection;
            if (from === to) {
                // No selection, use entire content
                selectedText = content;
                selectionRange = { from: 0, to: content.length };
            } else {
                selectedText = editor.state.doc.textBetween(from, to, " ");
                selectionRange = { from, to };
            }
        } else {
            // In source mode, just use entire content for now
            selectedText = content;
            selectionRange = { from: 0, to: content.length };
        }

        if (!selectedText.trim()) return;

        setEvaluatingTemplate(true);
        try {
            const res = await getTaurpc().kms_evaluate_placeholders(selectedText, userValues);

            if (res.requires_input && !userValues) {
                setSmartTemplateData(res);
                setSmartTemplateVisible(true);
            } else {
                setSmartTemplateVisible(false);
                setSmartTemplateData(null);

                // Replace content
                if (mode === "wysiwyg" && editor) {
                    const { from, to } = selectionRange;
                    if (from === 0 && to === content.length) {
                        editor.commands.setContent(res.result);
                    } else {
                        editor.commands.insertContentAt({ from, to }, res.result);
                    }
                } else {
                    setContent(res.result);
                    setIsDirty(true);
                }
            }
        } catch (error) {
            console.error("Failed to evaluate smart template:", error);
        } finally {
            setEvaluatingTemplate(false);
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
            editor?.commands.setContent(prepareMarkdownForTiptap(content, vaultPath));
            setMode("wysiwyg");
        }
    };

    const showReference = Boolean(
        referenceNote && referenceContent != null && !isZenMode
    );

    return (
        <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden bg-dc-bg">
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

                    <div className="flex items-center gap-1">
                        <Tooltip content="Evaluate Smart Templates ({js:...}, {date}, etc)">
                            <Button
                                variant="ghost"
                                size="sm"
                                className={cn(
                                    "h-8 w-8 p-0 text-dc-accent hover:bg-dc-accent/10 transition-all",
                                    evaluatingTemplate && "animate-pulse"
                                )}
                                onClick={() => handleEvaluateSmartTemplates()}
                                disabled={evaluatingTemplate}
                            >
                                <Sparkles size={16} />
                            </Button>
                        </Tooltip>
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
                        {onFavoriteChange && (
                            <Tooltip content={isFavorite ? "Remove from favorites" : "Add to favorites"}>
                                <Button
                                    variant="ghost"
                                    size="sm"
                                    className={cn(
                                        "h-8 w-8 p-0",
                                        isFavorite
                                            ? "text-dc-accent hover:bg-dc-accent/10"
                                            : "text-dc-text-muted hover:text-dc-accent hover:bg-dc-accent/10"
                                    )}
                                    disabled={favoriteBusy}
                                    onClick={async () => {
                                        setFavoriteBusy(true);
                                        try {
                                            await onFavoriteChange(!isFavorite);
                                        } finally {
                                            setFavoriteBusy(false);
                                        }
                                    }}
                                >
                                    <Star size={16} className={isFavorite ? "fill-dc-accent" : ""} />
                                </Button>
                            </Tooltip>
                        )}
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
                        {!isZenMode && (
                            <Tooltip content="Open Contextual Sub-Graph">
                                <Button
                                    variant="ghost"
                                    size="sm"
                                    className={cn(
                                        "h-8 w-8 p-0 transition-all border border-transparent",
                                        sidebarTab === "spatial" && showSidebar ? "bg-dc-accent/20 text-dc-accent border-dc-accent/40" : "text-dc-text-muted hover:bg-dc-bg-hover hover:text-dc-accent"
                                    )}
                                    onClick={() => {
                                        setSidebarTab("spatial");
                                        setShowSidebar(true);
                                    }}
                                >
                                    <Globe size={16} />
                                </Button>
                            </Tooltip>
                        )}

                    </div>
                </div>
            </header>

            <div
                className="flex min-h-0 min-w-0 flex-1 flex-row"
                data-kms-editor-split-root
            >
                <div className="relative flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
            {/* Editor Area */}
            <div className="flex-1 overflow-y-auto scrollbar-thin scrollbar-thumb-dc-border scrollbar-track-transparent bg-dc-bg editor-scroll-area">
                {mode === "wysiwyg" ? (
                    <div className="mx-auto max-w-4xl px-4 py-8 min-h-full">
                        <EditorContent editor={editor} />
                    </div>
                ) : (
                    <div className="h-full font-mono text-sm">
                        <CodeMirror
                            value={content}
                            height="100%"
                            theme={currentTheme === "dark" ? "dark" : "light"}
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
                <div
                    style={{ width: `${sidebarWidth}px` }}
                    className={cn(
                        "absolute top-[61px] right-0 bottom-[33px] bg-dc-bg-secondary/95 backdrop-blur-xl border-l border-dc-border z-40 flex flex-col shadow-2xl animate-in slide-in-from-right duration-300",
                        isResizing && "transition-none select-none"
                    )}
                >
                    {/* Resize Handle */}
                    <div
                        className="absolute left-[-2px] top-0 bottom-0 w-1 cursor-col-resize hover:bg-dc-accent/50 z-50 transition-colors group/handle"
                        onMouseDown={startResizing}
                    >
                        <div className="absolute inset-y-0 left-[-4px] right-[-4px]" /> {/* Invisible hitbox */}
                    </div>

                    <div className="p-0 border-b border-dc-border flex items-center justify-between bg-dc-accent/5">
                        <div className="flex flex-1 overflow-hidden">
                            <button
                                className={cn(
                                    "flex-1 py-3 px-1 text-[9px] font-bold uppercase tracking-tight transition-all flex items-center justify-center gap-1 min-w-0",
                                    sidebarTab === "backlinks" ? "bg-dc-accent/10 text-dc-accent border-b-2 border-dc-accent" : "text-dc-text-muted hover:text-dc-text"
                                )}
                                onClick={() => setSidebarTab("backlinks")}
                                title="Backlinks"
                            >
                                <Link2 size={12} className="flex-shrink-0" />
                                <span className="truncate">Links ({links?.incoming.length || 0})</span>
                            </button>
                            <button
                                className={cn(
                                    "flex-1 py-3 px-1 text-[9px] font-bold uppercase tracking-tight transition-all flex items-center justify-center gap-1 min-w-0",
                                    sidebarTab === "similar" ? "bg-dc-accent/10 text-dc-accent border-b-2 border-dc-accent" : "text-dc-text-muted hover:text-dc-text"
                                )}
                                onClick={() => setSidebarTab("similar")}
                                title="Similar Notes"
                            >
                                <Sparkles size={12} className="flex-shrink-0" />
                                <span className="truncate">Similar</span>
                            </button>
                            <button
                                className={cn(
                                    "flex-1 py-3 px-1 text-[9px] font-bold uppercase tracking-tight transition-all flex items-center justify-center gap-1 min-w-0",
                                    sidebarTab === "toc" ? "bg-dc-accent/10 text-dc-accent border-b-2 border-dc-accent" : "text-dc-text-muted hover:text-dc-text"
                                )}
                                onClick={() => setSidebarTab("toc")}
                                title="Table of Contents"
                            >
                                <List size={12} className="flex-shrink-0" />
                                <span className="truncate">TOC</span>
                            </button>
                            <button
                                className={cn(
                                    "flex-1 py-3 px-1 text-[9px] font-bold uppercase tracking-tight transition-all flex items-center justify-center gap-1 min-w-0",
                                    sidebarTab === "spatial" ? "bg-dc-accent/10 text-dc-accent border-b-2 border-dc-accent" : "text-dc-text-muted hover:text-dc-text"
                                )}
                                onClick={() => setSidebarTab("spatial")}
                                title="Local Resonance"
                            >
                                <Globe size={12} className="flex-shrink-0" />
                                <span className="truncate">Spatial</span>
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

                    <div className="flex-1 relative overflow-hidden">
                        {/* Persistent Tab Containers */}
                        <div className={cn("absolute inset-0 overflow-y-auto p-3 space-y-3 custom-scrollbar", sidebarTab !== "similar" && "hidden")}>
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

                        <div className={cn("absolute inset-0 overflow-y-auto p-3 space-y-1 custom-scrollbar", sidebarTab !== "toc" && "hidden")}>
                            {headings.length === 0 ? (
                                <div className="text-center py-10 px-4">
                                    <p className="text-[10px] text-dc-text-muted italic">No headings found in this note.</p>
                                </div>
                            ) : (
                                headings.map((h, i) => (
                                    <div
                                        key={i}
                                        className={cn(
                                            "relative py-1.5 px-3 rounded-lg cursor-pointer transition-all flex items-start gap-3 group",
                                            activeHeading === i ? "bg-dc-accent/10 text-dc-accent font-medium shadow-sm" : "hover:bg-dc-bg-hover text-dc-text-muted hover:text-dc-text"
                                        )}
                                        style={{ marginLeft: `${(h.level - 1) * 12}px` }}
                                        onClick={() => scrollToHeading(i)}
                                    >
                                        {/* Indentation Line */}
                                        {h.level > 1 && (
                                            <div className="absolute left-[-10px] top-0 bottom-0 w-[1px] bg-dc-border group-hover:bg-dc-accent/30 transition-colors" />
                                        )}

                                        <div className={cn(
                                            "mt-1.5 transition-all duration-300",
                                            activeHeading === i ? "opacity-100 scale-125" : "opacity-20 group-hover:opacity-60"
                                        )}>
                                            <div className={cn(
                                                "w-1.5 h-1.5 rounded-full",
                                                activeHeading === i ? "bg-dc-accent shadow-[0_0_8px_rgba(var(--dc-accent-rgb),0.6)]" : "bg-dc-text-muted"
                                            )} />
                                        </div>

                                        <span className={cn(
                                            "text-xs leading-relaxed",
                                            h.level === 1 ? "uppercase tracking-tight font-bold" : ""
                                        )}>
                                            {h.text}
                                        </span>
                                    </div>
                                ))
                            )}
                        </div>

                        <div className={cn("absolute inset-0 overflow-y-auto p-3 h-full flex flex-col gap-4 overflow-hidden custom-scrollbar", sidebarTab !== "spatial" && "hidden")}>
                            <div className="h-[500px] min-h-[500px] w-full rounded-2xl overflow-hidden border border-white/5 bg-black/20 shadow-inner relative">
                                <KmsLocalGraph3D
                                    path={path}
                                    toolsShortcutActive={sidebarTab === "spatial"}
                                    onSelectNote={(p) => {
                                        if (onSelectNote) onSelectNote(p);
                                    }}
                                />
                            </div>
                            <div className="p-4 rounded-2xl bg-dc-accent/5 border border-dc-accent/10 mb-4">
                                <div className="flex items-center gap-2 mb-2">
                                    <div className="w-1.5 h-1.5 rounded-full bg-dc-accent animate-pulse" />
                                    <span className="text-[10px] font-bold uppercase tracking-widest text-dc-accent">Resonance Dynamics</span>
                                </div>
                                <p className="text-[10px] text-dc-text-muted leading-relaxed italic opacity-80">
                                    Sub-graph depth 2 centered on focus. Click background to toggle spin.
                                </p>
                            </div>
                        </div>

                        <div className={cn("absolute inset-0 overflow-y-auto p-3 space-y-3 custom-scrollbar", sidebarTab !== "backlinks" && "hidden")}>
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

                </div>

                {showReference && referenceNote && referenceContent != null && (
                    <>
                        <div
                            role="separator"
                            aria-orientation="vertical"
                            aria-label="Resize reference pane"
                            title="Drag to resize reference pane"
                            onMouseDown={startResizingReference}
                            className={cn(
                                "relative flex w-2 shrink-0 cursor-col-resize items-center justify-center border-l border-dc-border bg-dc-bg-secondary/20",
                                isResizingReference && "bg-dc-accent/15"
                            )}
                        >
                            <GripVertical className="pointer-events-none h-8 w-4 text-dc-text-muted opacity-70" />
                        </div>
                        <div
                            className="flex min-h-0 min-w-0 shrink-0 flex-col border-l border-dc-border bg-dc-bg/50"
                            style={{ width: referencePaneWidth }}
                        >
                            <div className="flex items-center justify-between gap-2 border-b border-dc-border bg-dc-bg-secondary/40 px-3 py-2">
                                <div className="flex min-w-0 items-center gap-2">
                                    <BookOpen className="h-4 w-4 shrink-0 text-dc-accent" />
                                    <span className="text-[10px] font-bold uppercase tracking-wider text-dc-text-muted">
                                        Reference
                                    </span>
                                    <span
                                        className="truncate text-xs font-medium text-dc-text"
                                        title={referenceNote.path}
                                    >
                                        {referenceNote.title ||
                                            referenceNote.path.split(/[\\/]/).pop()}
                                    </span>
                                </div>
                                <Button
                                    variant="ghost"
                                    size="sm"
                                    className="h-8 w-8 p-0"
                                    onClick={onClearReference}
                                    title="Close reference pane"
                                >
                                    <X size={16} />
                                </Button>
                            </div>
                            <KmsReferenceReadOnly
                                path={referenceNote.path}
                                markdownContent={referenceContent}
                                vaultPath={vaultPath}
                                currentTheme={currentTheme}
                                onSelectNote={onSelectNote}
                                onOpenReferenceWikiTarget={onOpenReferenceWikiTarget}
                            />
                        </div>
                    </>
                )}
            </div>

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

            <KmsSmartTemplateModal
                visible={smartTemplateVisible}
                data={smartTemplateData}
                onOk={(values: Record<string, string>) => handleEvaluateSmartTemplates(values)}
                onCancel={() => setSmartTemplateVisible(false)}
            />
        </div>
    );
}

// --- Helper Functions for Wiki-link Handling ---

function unwrapWikiLinks(md: string): string {
    if (!md) return md;
    return md.replace(/<span\s+[^>]*data-type="wiki-link"[^>]*data-target="([^"]+)"[^>]*>\[\[.*?\]\]<\/span>/g, '[[$1]]');
}

// --- Helper Functions for Image Handling ---

function unwrapImages(md: string): string {
    if (!md) return md;
    // Look for our specific img tag with data-path
    return md.replace(/<img\s+[^>]*data-type="vault-image"[^>]*data-path="([^"]+)"[^>]*alt="([^"]*)"[^>]*\/?>/g, '![$2]($1)');
}

// --- Helper Functions for Frontmatter Handling ---

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

function unwrapMath(md: string): string {
    if (!md) return md;
    // Reverse display math
    let processed = md.replace(/<div data-type="math" data-display="true">([\s\S]+?)<\/div>/g, '$$\n$1\n$$');
    // Reverse inline math
    processed = processed.replace(/<span data-type="math" data-display="false">([\s\S]+?)<\/span>/g, '$$$1$$');
    return processed;
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
