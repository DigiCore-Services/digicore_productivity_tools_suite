import { Node, mergeAttributes, InputRule } from "@tiptap/core";
import { ReactNodeViewRenderer, NodeViewProps } from "@tiptap/react";
import React from "react";
import { Link2 } from "lucide-react";

export interface WikiLinkOptions {
    HTMLAttributes: Record<string, any>;
    onLinkClick?: (path: string) => void;
}

declare module "@tiptap/core" {
    interface Commands<ReturnType> {
        wikiLink: {
            setWikiLink: (attributes: { target: string }) => ReturnType;
        };
    }
}

export const WikiLinkExtension = Node.create<WikiLinkOptions>({
    name: "wikiLink",
    inline: true,
    group: "inline",
    atom: true,

    addOptions() {
        return {
            HTMLAttributes: {
                class: "wiki-link inline-flex items-center gap-1 text-dc-accent hover:underline cursor-pointer font-medium py-[1px] px-[2px] rounded bg-dc-accent/5 transition-all hover:bg-dc-accent/10 decoration-dc-accent/30",
            },
            onLinkClick: undefined,
        };
    },

    addAttributes() {
        return {
            target: {
                default: null,
            },
        };
    },

    parseHTML() {
        return [
            {
                tag: 'span[data-type="wiki-link"]',
                getAttrs: (element) => ({
                    target: element.getAttribute("data-target"),
                }),
            },
        ];
    },

    renderHTML({ HTMLAttributes, node }) {
        return [
            "span",
            mergeAttributes(this.options.HTMLAttributes, HTMLAttributes, {
                "data-type": "wiki-link",
                "data-target": node.attrs.target,
            }),
            `[[${node.attrs.target}]]`,
        ];
    },

    addCommands() {
        return {
            setWikiLink:
                (attributes) =>
                    ({ commands }) => {
                        return commands.insertContent({
                            type: this.name,
                            attrs: attributes,
                        });
                    },
        };
    },

    addNodeView() {
        return ReactNodeViewRenderer((props: NodeViewProps) => {
            const { node } = props;
            const target = node.attrs.target;

            const handleClick = (e: React.MouseEvent) => {
                e.preventDefault();
                e.stopPropagation();
                if (this.options.onLinkClick) {
                    this.options.onLinkClick(target);
                }
            };

            return (
                <span
                    className={this.options.HTMLAttributes.class}
                    onClick={handleClick}
                    title={`Go to ${target}`}
                    data-type="wiki-link"
                    data-target={target}
                >
                    <Link2 size={12} className="opacity-70" />
                    <span>{target}</span>
                </span>
            );
        });
    },

    addInputRules() {
        return [
            new InputRule({
                find: /\[\[([^\]]+)\]\]$/,
                handler: ({ state, range, match }) => {
                    const { tr } = state;
                    const target = match[1];

                    if (target) {
                        tr.replaceWith(range.from, range.to, this.type.create({ target }));
                    }
                },
            }),
        ];
    },
});
