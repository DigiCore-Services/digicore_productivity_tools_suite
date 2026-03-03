import { useEffect, useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { Snippet } from "../../types";

interface SnippetEditorProps {
  visible: boolean;
  mode: "add" | "edit";
  category: string;
  snippetIdx: number;
  initialSnippet?: Snippet | null;
  prefill?: { content: string; trigger: string };
  onSave: (snippet: Snippet) => void;
  onCancel: () => void;
}

export function SnippetEditor({
  visible,
  mode,
  category,
  snippetIdx,
  initialSnippet,
  prefill,
  onSave,
  onCancel,
}: SnippetEditorProps) {
  const [trigger, setTrigger] = useState("");
  const [profile, setProfile] = useState("Default");
  const [options, setOptions] = useState("*:");
  const [snippetCategory, setSnippetCategory] = useState("General");
  const [content, setContent] = useState("");
  const [appLock, setAppLock] = useState("");
  const [pinned, setPinned] = useState(false);

  useEffect(() => {
    if (visible) {
      if (mode === "edit" && initialSnippet) {
        setTrigger(initialSnippet.trigger || "");
        setProfile(initialSnippet.profile || "Default");
        setOptions(initialSnippet.options || "*:");
        setSnippetCategory(initialSnippet.category || category);
        setContent(initialSnippet.content || "");
        setAppLock(initialSnippet.app_lock || "");
        setPinned((initialSnippet.pinned || "").toLowerCase() === "true");
      } else if (mode === "add" && prefill) {
        setTrigger(prefill.trigger || "");
        setContent(prefill.content || "");
        setProfile("Default");
        setOptions("*:");
        setSnippetCategory(category || "General");
        setAppLock("");
        setPinned(false);
      } else {
        setTrigger("");
        setProfile("Default");
        setOptions("*:");
        setSnippetCategory(category || "General");
        setContent("");
        setAppLock("");
        setPinned(false);
      }
    }
  }, [visible, mode, category, initialSnippet, prefill]);

  const handleSave = () => {
    const snippet: Snippet = {
      trigger: trigger.trim(),
      content,
      options: options.trim() || "*:",
      category: snippetCategory.trim(),
      profile: profile.trim() || "Default",
      app_lock: appLock.trim(),
      pinned: pinned ? "true" : "false",
      last_modified: "",
    };
    onSave(snippet);
  };

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
            initial={{ scale: 0.95, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            exit={{ scale: 0.95, opacity: 0 }}
            transition={{ type: "spring", duration: 0.3 }}
            className="bg-[var(--dc-bg-elevated)] text-[var(--dc-text)] p-6 rounded-xl max-w-[600px] w-full max-h-[90vh] overflow-y-auto border border-[var(--dc-border)] shadow-xl"
            onClick={(e) => e.stopPropagation()}
          >
            <h3 className="text-lg font-semibold mb-4">
              {mode === "add" ? "Add Snippet" : "Edit Snippet"}
            </h3>
            <div className="space-y-4">
              <div>
                <Label className="mb-1 block">Trigger</Label>
                <Input
                  value={trigger}
                  onChange={(e) => setTrigger(e.target.value)}
                  placeholder="e.g. /sig"
                />
              </div>
              <div>
                <Label className="mb-1 block">Profile</Label>
                <Input
                  value={profile}
                  onChange={(e) => setProfile(e.target.value)}
                  placeholder="Default"
                />
              </div>
              <div>
                <Label className="mb-1 block">Options</Label>
                <Input
                  value={options}
                  onChange={(e) => setOptions(e.target.value)}
                  placeholder="*:"
                />
              </div>
              <div>
                <Label className="mb-1 block">Category</Label>
                <Input
                  value={snippetCategory}
                  onChange={(e) => setSnippetCategory(e.target.value)}
                  placeholder="General"
                />
              </div>
              <div>
                <Label className="mb-1 block">Content</Label>
                <textarea
                  value={content}
                  onChange={(e) => setContent(e.target.value)}
                  placeholder="Snippet content..."
                  className="flex min-h-[120px] w-full rounded-md border border-[var(--dc-border)] bg-[var(--dc-bg)] px-3 py-2 text-sm"
                />
              </div>
              <div>
                <Label className="mb-1 block">AppLock</Label>
                <Input
                  value={appLock}
                  onChange={(e) => setAppLock(e.target.value)}
                  placeholder="comma-separated exe names"
                />
              </div>
              <label className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={pinned}
                  onChange={(e) => setPinned(e.target.checked)}
                  className="rounded"
                />
                <span>Pinned</span>
              </label>
            </div>
            <div className="mt-6 flex gap-2">
              <Button onClick={handleSave}>Save</Button>
              <Button variant="secondary" onClick={onCancel}>
                Cancel
              </Button>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
