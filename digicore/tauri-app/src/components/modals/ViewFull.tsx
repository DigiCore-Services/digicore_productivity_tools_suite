import { motion, AnimatePresence } from "framer-motion";
import { Button } from "@/components/ui/button";
import { ArrowUpToLine, Copy, Pencil, Pin, PinOff, Trash2 } from "lucide-react";

interface ViewFullProps {
  visible: boolean;
  content: string;
  onClose: () => void;
  /** When from Library tab: edit metadata to show Edit button. */
  onEdit?: (category: string, snippetIdx: number) => void;
  editMeta?: { category: string; snippetIdx: number } | null;
  onPromote?: () => void;
  onCopy?: () => void;
  onDelete?: () => void;
  onPin?: () => void;
  onUnpin?: () => void;
  canPin?: boolean;
  canPromote?: boolean;
}

export function ViewFull({
  visible,
  content,
  onClose,
  onEdit,
  editMeta,
  onPromote,
  onCopy,
  onDelete,
  onPin,
  onUnpin,
  canPin = true,
  canPromote = true,
}: ViewFullProps) {
  const handleEdit = () => {
    if (onEdit && editMeta) {
      onEdit(editMeta.category, editMeta.snippetIdx);
      onClose();
    }
  };

  return (
    <AnimatePresence>
      {visible && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className="fixed inset-0 bg-black/50 z-[1000] flex items-center justify-center p-4"
          onClick={onClose}
        >
          <motion.div
            initial={{ scale: 0.95 }}
            animate={{ scale: 1 }}
            exit={{ scale: 0.95 }}
            className="bg-[var(--dc-bg-elevated)] text-[var(--dc-text)] p-6 rounded-xl max-w-[90%] w-full max-h-[90vh] overflow-y-auto border border-[var(--dc-border)] shadow-xl"
            onClick={(e) => e.stopPropagation()}
          >
            <h3 className="text-lg font-semibold mb-4">View Full Content</h3>
            <pre className="bg-[var(--dc-bg-tertiary)] p-4 overflow-x-auto text-sm rounded-lg max-h-[60vh] overflow-y-auto whitespace-pre-wrap">
              {content}
            </pre>
            <div className="mt-4 flex gap-2">
              <Button variant="secondary" size="sm" onClick={onClose}>
                Close
              </Button>
              {onPin && (
                <Button
                  size="sm"
                  onClick={onPin}
                  disabled={!canPin}
                  variant={canPin ? "default" : "secondary"}
                  title={canPin ? "Pin snippet" : "Already pinned"}
                >
                  <Pin className="w-3 h-3 mr-1" aria-hidden />
                  {canPin ? "Pin" : "Pinned"}
                </Button>
              )}
              {onUnpin && (
                <Button
                  size="sm"
                  onClick={onUnpin}
                  variant="secondary"
                  title="Unpin snippet"
                >
                  <PinOff className="w-3 h-3 mr-1" aria-hidden />
                  Unpin
                </Button>
              )}
              {onPromote && (
                <Button
                  size="sm"
                  onClick={onPromote}
                  disabled={!canPromote}
                  variant={canPromote ? "default" : "secondary"}
                  title={canPromote ? "Promote to snippet" : "Already promoted"}
                >
                  <ArrowUpToLine className="w-3 h-3 mr-1" aria-hidden />
                  {canPromote ? "Promote" : "Promoted"}
                </Button>
              )}
              {onEdit && editMeta && (
                <Button size="sm" onClick={handleEdit} title="Edit snippet">
                  <Pencil className="w-3 h-3 mr-1" aria-hidden />
                  Edit
                </Button>
              )}
              {onCopy && (
                <Button size="sm" variant="secondary" onClick={onCopy} title="Copy full content">
                  <Copy className="w-3 h-3 mr-1" aria-hidden />
                  Copy
                </Button>
              )}
              {onDelete && (
                <Button
                  size="sm"
                  variant="secondary"
                  onClick={onDelete}
                  className="text-[var(--dc-error)]"
                  title="Delete with confirmation"
                >
                  <Trash2 className="w-3 h-3 mr-1" aria-hidden />
                  Delete
                </Button>
              )}
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
