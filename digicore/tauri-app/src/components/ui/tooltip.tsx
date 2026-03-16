import * as React from "react"
import { cn } from "@/lib/utils"

interface TooltipProps {
    children: React.ReactNode
    content: React.ReactNode
    className?: string
}

export function Tooltip({ children, content, className }: TooltipProps) {
    const [show, setShow] = React.useState(false)

    return (
        <div
            className="relative flex items-center"
            onMouseEnter={() => setShow(true)}
            onMouseLeave={() => setShow(false)}
        >
            {children}
            {show && (
                <div className={cn(
                    "absolute bottom-full left-1/2 -translate-x-1/2 mb-2 z-[300] px-2 py-1 text-xs font-medium text-white bg-black/80 backdrop-blur-sm rounded shadow-lg whitespace-nowrap animate-in fade-in zoom-in-95 duration-150",
                    className
                )}>
                    {content}
                    <div className="absolute top-full left-1/2 -translate-x-1/2 border-x-4 border-x-transparent border-t-4 border-t-black/80" />
                </div>
            )}
        </div>
    )
}
