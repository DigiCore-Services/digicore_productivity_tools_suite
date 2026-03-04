import { motion, AnimatePresence } from "framer-motion";
import { Button } from "@/components/ui/button";
import { Pencil } from "lucide-react";

interface ViewFullProps {
  visible: boolean;
  content: string;
  onClose: () => void;
  /** When from Library tab: edit metadata to show Edit button. */
  onEdit?: (category: string, snippetIdx: number) => void;
  editMeta?: { category: string; snippetIdx: number } | null;
}

export function ViewFull({
  visible,
  content,
  onClose,
  onEdit,
  editMeta,
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
              {onEdit && editMeta && (
                <Button size="sm" onClick={handleEdit}>
                  <Pencil className="w-3 h-3 mr-1" aria-hidden />
                  Edit
                </Button>
              )}
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
