import { Node, ReactNodeViewRenderer, NodeViewWrapper, NodeViewContent } from '@tiptap/react'
import React from 'react'

export const FrontmatterExtension = Node.create({
  name: 'frontmatter',

  group: 'block',

  content: 'text*', // Use text* to encourage preserving the block as raw text

  marks: '', // No formatting marks inside the frontmatter

  code: true, // Mark it as code-like to Tiptap

  defining: true,

  parseHTML() {
    return [
      {
        tag: 'pre[data-type="frontmatter"]',
        preserveWhitespace: 'full',
      },
      {
        tag: 'div[data-type="frontmatter"]',
        preserveWhitespace: 'full',
      },
    ]
  },

  renderHTML({ HTMLAttributes }) {
    return ['pre', { 'data-type': 'frontmatter', ...HTMLAttributes }, 0]
  },

  addNodeView() {
    return ReactNodeViewRenderer(() => {
      return (
        <NodeViewWrapper className="frontmatter-node my-4">
          <div className="font-mono text-[13px] leading-relaxed text-dc-text-muted bg-dc-bg-secondary/20 p-4 border-y border-dc-border/30">
            <div className="opacity-40 mb-1 select-none">---</div>
            <div className="whitespace-pre focus:outline-none selection:bg-dc-accent/30 font-mono text-[13px] leading-relaxed">
              <NodeViewContent as="div" />
            </div>
            <div className="opacity-40 mt-1 select-none">---</div>
          </div>
        </NodeViewWrapper>
      )
    })
  },
})
