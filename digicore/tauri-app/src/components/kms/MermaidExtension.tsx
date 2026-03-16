import { NodeViewContent, NodeViewWrapper, ReactNodeViewRenderer, NodeViewProps } from '@tiptap/react'
import CodeBlock from '@tiptap/extension-code-block'
import mermaid from 'mermaid'
import React, { useEffect, useState, useRef } from 'react'

// Initialize Mermaid with a dark theme match for DigiCore
mermaid.initialize({
  startOnLoad: false,
  theme: 'dark',
  securityLevel: 'loose',
  fontFamily: 'Inter, system-ui, sans-serif',
  themeVariables: {
    primaryColor: '#0078D4',
    background: 'transparent',
    mainBkg: 'transparent',
    nodeBorder: '#444',
  }
})

const MermaidRenderer = ({ content }: { content: string }) => {
  const [svg, setSvg] = useState<string>('')
  const [error, setError] = useState<string | null>(null)
  const isInitial = useRef(true)

  useEffect(() => {
    const renderDiagram = async () => {
      if (!content.trim()) {
        setSvg('')
        return
      }

      try {
        const id = `mermaid-${Math.random().toString(36).substring(2, 11)}`
        const { svg: renderedSvg } = await mermaid.render(id, content.trim())
        setSvg(renderedSvg)
        setError(null)
      } catch (e: any) {
        console.error('Mermaid render error:', e)
        setError(typeof e === 'string' ? e : e.message || 'Failed to render diagram')
      }
    }

    // Small delay on initial render to ensure container is ready
    if (isInitial.current) {
      isInitial.current = false
      setTimeout(renderDiagram, 100)
    } else {
      renderDiagram()
    }
  }, [content])

  if (error) {
    return (
      <div className="p-4 bg-red-950/30 border border-red-500/50 rounded-xl text-red-400 text-xs font-mono shadow-lg my-4">
        <div className="flex items-center gap-2 mb-2 font-bold uppercase tracking-wider">
          <span className="w-2 h-2 bg-red-500 rounded-full animate-pulse" />
          Mermaid Render Error
        </div>
        <pre className="whitespace-pre-wrap opacity-80">{error}</pre>
      </div>
    )
  }

  if (svg) {
    return (
      <div
        className="flex justify-center bg-dc-bg-secondary/10 p-6 rounded-2xl border border-dc-border/30 shadow-inner overflow-x-auto transition-all hover:border-dc-accent/30 my-6"
        dangerouslySetInnerHTML={{ __html: svg }}
      />
    )
  }

  return (
    <div className="p-8 bg-dc-bg-secondary/20 border border-dc-border border-dashed rounded-xl text-center opacity-50 italic text-xs my-6">
      Loading Mermaid diagram...
    </div>
  )
}

const MermaidCodeBlockView = (props: NodeViewProps) => {
  const isMermaid = props.node.attrs.language === 'mermaid'

  return (
    <NodeViewWrapper className="mermaid-wrapper relative group">
      {isMermaid ? (
        <>
          <div className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity z-10">
            <span className="px-2 py-1 bg-dc-accent/20 text-dc-accent text-[8px] font-bold uppercase rounded border border-dc-accent/30 backdrop-blur-md">
              Mermaid Diagram
            </span>
          </div>
          <MermaidRenderer content={props.node.textContent} />
          {/* Hidden content for Tiptap to keep track of the node */}
          <pre className="hidden">
            <NodeViewContent as="div" />
          </pre>
        </>
      ) : (
        <pre className="bg-dc-bg-secondary/50 p-4 rounded-xl border border-dc-border/50 font-mono text-sm leading-relaxed">
          <NodeViewContent as="div" />
        </pre>
      )}
    </NodeViewWrapper>
  )
}

export const MermaidExtension = CodeBlock.extend({
  addNodeView() {
    return ReactNodeViewRenderer(MermaidCodeBlockView)
  },
})
