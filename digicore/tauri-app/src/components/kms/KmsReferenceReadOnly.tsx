import React, { useEffect, useRef } from "react";
import { useEditor, EditorContent } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { Markdown } from "tiptap-markdown";
import { Image } from "@tiptap/extension-image";
import { MermaidExtension } from "./MermaidExtension";
import { MathExtension } from "./MathExtension";
import { AdmonitionExtension } from "./AdmonitionExtension";
import { FrontmatterExtension } from "./FrontmatterExtension";
import { WikiLinkExtension } from "./WikiLinkExtension";
import { prepareMarkdownForTiptap } from "../../lib/kmsEditorPrepareMarkdown";
import { resolveMarkdownLinkAgainstNotePath } from "../../lib/kmsMarkdownLinkResolve";

export interface KmsReferenceReadOnlyProps {
    path: string;
    markdownContent: string;
    vaultPath?: string | null;
    currentTheme?: "light" | "dark";
    onSelectNote?: (path: string) => void;
    onOpenReferenceWikiTarget?: (target: string) => void;
}

export default function KmsReferenceReadOnly({
    path,
    markdownContent,
    vaultPath,
    currentTheme = "dark",
    onSelectNote,
    onOpenReferenceWikiTarget,
}: KmsReferenceReadOnlyProps) {
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
        content: "",
        editable: false,
        editorProps: {
            attributes: {
                class:
                    "prose dark:prose-invert prose-sm sm:prose-base lg:prose-lg focus:outline-none max-w-none min-h-[200px] p-6 bg-dc-bg",
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
    });

    useEffect(() => {
        if (!editor) return;
        const processed = prepareMarkdownForTiptap(markdownContent, vaultPath);
        setTimeout(() => {
            editor.commands.setContent(processed);
        }, 0);
    }, [path, markdownContent, vaultPath, editor]);

    return (
        <div
            className="reference-readonly-root flex-1 min-h-0 overflow-y-auto scrollbar-thin scrollbar-thumb-dc-border scrollbar-track-transparent bg-dc-bg"
            data-theme={currentTheme}
        >
            <div className="mx-auto max-w-4xl px-4 py-6 min-h-full">
                <EditorContent editor={editor} />
            </div>
        </div>
    );
}
