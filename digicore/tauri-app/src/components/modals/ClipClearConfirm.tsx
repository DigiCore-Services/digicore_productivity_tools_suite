import { motion, AnimatePresence } from "framer-motion";
import { Button } from "../ui/button";

interface ClipClearConfirmProps {
  visible: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

export function ClipClearConfirm({
  visible,
  onConfirm,
  onCancel,
}: ClipClearConfirmProps) {
  return (
    <AnimatePresence>
      {visible && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className="fixed inset-0 bg-black/50 z-[1000] flex items-center justify-center p-4"
          onClick={onCancel}
        >
          <motion.div
            initial={{ scale: 0.95 }}
            animate={{ scale: 1 }}
            exit={{ scale: 0.95 }}
            className="bg-[var(--dc-bg-elevated)] text-[var(--dc-text)] p-6 rounded-xl max-w-[600px] w-full border border-[var(--dc-border)] shadow-xl"
            onClick={(e) => e.stopPropagation()}
          >
            <h3 className="text-lg font-semibold mb-4">Clear All History</h3>
            <p className="mb-6 text-[var(--dc-text-muted)]">
              Are you sure you want to clear all clipboard history?
            </p>
            <div className="flex gap-2">
              <Button variant="destructive" size="sm" onClick={onConfirm}>
                Clear All
              </Button>
              <Button variant="secondary" size="sm" onClick={onCancel}>
                Cancel
              </Button>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
