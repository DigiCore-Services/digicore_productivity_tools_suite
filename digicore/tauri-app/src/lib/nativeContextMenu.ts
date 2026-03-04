/**
 * Native OS context menu via Tauri Menu API.
 * Replaces Radix ContextMenu for system dark/light mode and accessibility.
 */
import { LogicalPosition } from "@tauri-apps/api/dpi";
import { Menu, MenuItem } from "@tauri-apps/api/menu";
import { getCurrentWindow } from "@tauri-apps/api/window";

export interface NativeContextMenuAction {
  id: string;
  text: string;
  onClick: () => void;
}

export async function showNativeContextMenu(
  x: number,
  y: number,
  actions: NativeContextMenuAction[]
): Promise<void> {
  const items = await Promise.all(
    actions.map((a) =>
      MenuItem.new({
        id: a.id,
        text: a.text,
        action: a.onClick,
      })
    )
  );
  const menu = await Menu.new({ items });
  const win = getCurrentWindow();
  const pos = new LogicalPosition(x, y);
  await menu.popup(pos, win);
}
