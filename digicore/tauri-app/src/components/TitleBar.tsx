import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-react";

export function TitleBar() {
  const appWindow = getCurrentWindow();

  return (
    <div
      data-tauri-drag-region
      className="flex h-8 shrink-0 items-center justify-between border-b border-[var(--dc-border)] bg-[var(--dc-bg-alt)] px-2 select-none"
    >
      <span
        data-tauri-drag-region
        className="text-sm font-medium text-[var(--dc-text)]"
      >
        DigiCore Text Expander
      </span>
      <div className="flex items-center" data-tauri-drag-region={false}>
        <button
          type="button"
          onClick={() => appWindow.minimize()}
          className="flex h-8 w-10 items-center justify-center text-[var(--dc-text)] hover:bg-[var(--dc-bg-tertiary)]"
          title="Minimize"
          aria-label="Minimize window"
        >
          <Minus className="h-4 w-4" aria-hidden />
        </button>
        <button
          type="button"
          onClick={() => appWindow.toggleMaximize()}
          className="flex h-8 w-10 items-center justify-center text-[var(--dc-text)] hover:bg-[var(--dc-bg-tertiary)]"
          title="Maximize"
          aria-label="Maximize window"
        >
          <Square className="h-3.5 w-3.5" aria-hidden />
        </button>
        <button
          type="button"
          onClick={() => appWindow.close()}
          className="flex h-8 w-10 items-center justify-center text-[var(--dc-text)] hover:bg-red-500 hover:text-white"
          title="Close"
          aria-label="Close window"
        >
          <X className="h-4 w-4" aria-hidden />
        </button>
      </div>
    </div>
  );
}
