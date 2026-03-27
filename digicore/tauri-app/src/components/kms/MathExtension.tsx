import { Node, ReactNodeViewRenderer, NodeViewWrapper, mergeAttributes, InputRule } from '@tiptap/react'
import katex from 'katex'
import 'katex/dist/katex.min.css'
import React, { useMemo } from 'react'

const MathRenderer = ({ node }: { node: any }) => {
  const content = node.textContent
  const isDisplay = node.attrs.display

  const html = useMemo(() => {
    try {
      if (!content.trim()) return ''
      return katex.renderToString(content, {
        displayMode: isDisplay,
        throwOnError: false,
        trust: true,
      })
    } catch (e) {
      return `<span class="text-red-400 font-mono text-[10px]">Math Error: ${e}</span>`
    }
  }, [content, isDisplay])

  return (
    <NodeViewWrapper
      as={isDisplay ? 'div' : 'span'}
      className={`math-node relative group transition-all ${isDisplay ? 'my-6 flex justify-center py-4 bg-dc-bg-secondary/10 rounded-xl border border-dc-border/30 shadow-inner' : 'mx-0.5 px-0.5 rounded bg-dc-accent/5 font-mono text-dc-accent border border-dc-accent/10'}`}
    >
      <div
        dangerouslySetInnerHTML={{ __html: html }}
        className={isDisplay ? 'text-lg' : 'inline-block align-middle'}
      />
      {isDisplay && (
        <div className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity">
          <span className="px-2 py-1 bg-dc-accent/20 text-dc-accent text-[8px] font-bold uppercase rounded border border-dc-accent/30 backdrop-blur-md">
            LaTeX Math
          </span>
        </div>
      )}
    </NodeViewWrapper>
  )
}

export const MathExtension = Node.create({
  name: 'math',

  group: 'inline',

  inline: true,

  content: 'text*',

  addAttributes() {
    return {
      display: {
        default: false,
      },
    }
  },

  parseHTML() {
    return [
      {
        tag: 'span[data-type="math"]',
        getAttrs: (element: HTMLElement) => ({
          display: element.getAttribute('data-display') === 'true',
        }),
      },
      {
        tag: 'div[data-type="math"]',
        getAttrs: (element: HTMLElement) => ({
          display: true,
        }),
      },
      // Support for Markdown-style $ and $$ in HTML if passed as text
    ]
  },

  renderHTML({ node, HTMLAttributes }) {
    return [
      node.attrs.display ? 'div' : 'span',
      mergeAttributes(HTMLAttributes, {
        'data-type': 'math',
        'data-display': String(node.attrs.display),
      }),
      0,
    ]
  },

  addNodeView() {
    return ReactNodeViewRenderer(MathRenderer)
  },

  addInputRules() {
    return [
      // Block math: $$ ... $$
      new InputRule({
        find: /\$\$([\s\S]+?)\$\$$/,
        handler: ({ state, range, match }) => {
          const { tr } = state
          if (match[1]) {
            tr.replaceWith(range.from, range.to, this.type.create({ display: true }))
          }
        },
      }),
      // Inline math: $ ... $
      new InputRule({
        find: /\$([\s\S]+?)\$/,
        handler: ({ state, range, match }) => {
          const { tr } = state
          if (match[1]) {
            tr.replaceWith(range.from, range.to, this.type.create({ display: false }))
          }
        }
      })
    ]
  },
})
