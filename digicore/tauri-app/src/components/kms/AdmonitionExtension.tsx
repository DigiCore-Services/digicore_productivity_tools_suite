import { Node, ReactNodeViewRenderer, NodeViewWrapper, NodeViewContent, mergeAttributes, InputRule } from '@tiptap/react'
import React from 'react'
import { Info, AlertTriangle, AlertCircle, CheckCircle2, Flame } from 'lucide-react'

const ADMONITION_TYPES = {
  note: { icon: Info, color: 'text-blue-400', bg: 'bg-blue-400/10', border: 'border-blue-400/30' },
  warning: { icon: AlertTriangle, color: 'text-amber-400', bg: 'bg-amber-400/10', border: 'border-amber-400/30' },
  important: { icon: AlertCircle, color: 'text-purple-400', bg: 'bg-purple-400/10', border: 'border-purple-400/30' },
  success: { icon: CheckCircle2, color: 'text-emerald-400', bg: 'bg-emerald-400/10', border: 'border-emerald-400/30' },
  danger: { icon: Flame, color: 'text-red-400', bg: 'bg-red-400/10', border: 'border-red-400/30' },
}

const AdmonitionView = ({ node }: { node: any }) => {
  const type = node.attrs.type as keyof typeof ADMONITION_TYPES
  const config = ADMONITION_TYPES[type] || ADMONITION_TYPES.note
  const Icon = config.icon

  return (
    <NodeViewWrapper className={`admonition-node my-6 ${config.bg} ${config.border} border rounded-2xl overflow-hidden shadow-sm transition-all hover:shadow-md`}>
      <div className={`flex items-center gap-2 px-4 py-2 border-b ${config.border} bg-dc-bg-secondary/20`}>
        <Icon size={16} className={config.color} />
        <span className={`text-[10px] font-bold uppercase tracking-wider ${config.color}`}>
          {type}
        </span>
      </div>
      <div className="p-4 prose dark:prose-invert prose-sm max-w-none">
        <NodeViewContent />
      </div>
    </NodeViewWrapper>
  )
}

export const AdmonitionExtension = Node.create({
  name: 'admonition',

  group: 'block',

  content: 'block+',

  defining: true,

  addAttributes() {
    return {
      type: {
        default: 'note',
      },
    }
  },

  parseHTML() {
    return [
      {
        tag: 'div[data-type="admonition"]',
        getAttrs: (element: HTMLElement) => ({
          type: element.getAttribute('data-admonition-type'),
        }),
      },
    ]
  },

  renderHTML({ node, HTMLAttributes }) {
    return [
      'div',
      mergeAttributes(HTMLAttributes, {
        'data-type': 'admonition',
        'data-admonition-type': node.attrs.type,
      }),
      0,
    ]
  },

  addNodeView() {
    return ReactNodeViewRenderer(AdmonitionView)
  },

  addInputRules() {
    return [
      // ::: type
      new InputRule({
        find: /^:::(\w+)\s$/,
        handler: ({ state, range, match }) => {
          const type = match[1].toLowerCase()
          if (ADMONITION_TYPES[type as keyof typeof ADMONITION_TYPES]) {
            const { tr } = state
            tr.replaceWith(range.from, range.to, this.type.create({ type }))
          }
        },
      }),
      // > [!TYPE]
      new InputRule({
        find: /^>\s\[!(\w+)\]\s$/,
        handler: ({ state, range, match }) => {
          const type = match[1].toLowerCase()
          if (ADMONITION_TYPES[type as keyof typeof ADMONITION_TYPES]) {
            const { tr } = state
            tr.replaceWith(range.from, range.to, this.type.create({ type }))
          }
        }
      })
    ]
  },
})
