import { useToast, Toast } from "@/components/ui/use-toast"
import { motion, AnimatePresence } from "framer-motion"
import { X, CheckCircle, AlertCircle } from "lucide-react"

export function Toaster() {
    const { toasts, dismiss } = useToast()

    return (
        <div className="fixed bottom-4 right-4 z-[200] flex flex-col gap-2 w-full max-w-[400px]">
            <AnimatePresence>
                {toasts.map((toast: Toast) => (
                    <motion.div
                        key={toast.id}
                        initial={{ opacity: 0, y: 20, scale: 0.95 }}
                        animate={{ opacity: 1, y: 0, scale: 1 }}
                        exit={{ opacity: 0, scale: 0.95 }}
                        className={`
              relative flex w-full items-center justify-between space-x-4 overflow-hidden rounded-lg border p-4 pr-8 shadow-lg transition-all
              ${toast.variant === 'destructive'
                                ? 'bg-red-50 border-red-200 text-red-900'
                                : 'bg-[var(--dc-bg-elevated)] border-[var(--dc-border)] text-[var(--dc-text)]'}
            `}
                    >
                        <div className="flex items-start gap-3">
                            {toast.variant === 'destructive' ? (
                                <AlertCircle className="h-5 w-5 text-red-500 shrink-0 mt-0.5" />
                            ) : (
                                <CheckCircle className="h-5 w-5 text-emerald-500 shrink-0 mt-0.5" />
                            )}
                            <div className="grid gap-1">
                                {toast.title && <div className="text-sm font-semibold">{toast.title}</div>}
                                {toast.description && (
                                    <div className="text-sm opacity-90">{toast.description}</div>
                                )}
                            </div>
                        </div>
                        <button
                            onClick={() => dismiss(toast.id)}
                            className="absolute right-2 top-2 rounded-md p-1 text-[var(--dc-text-muted)] opacity-0 transition-opacity hover:text-[var(--dc-text)] group-hover:opacity-100 focus:opacity-100 focus:outline-none focus:ring-2"
                        >
                            <X className="h-4 w-4" />
                        </button>
                    </motion.div>
                ))}
            </AnimatePresence>
        </div>
    )
}
