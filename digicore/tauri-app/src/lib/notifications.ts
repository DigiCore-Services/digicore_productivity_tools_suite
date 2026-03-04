/**
 * Rich notifications with actionable buttons.
 * Registers "View Library" action for library load/save toasts.
 */
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
  registerActionTypes,
  onAction,
  createChannel,
  Importance,
  Visibility,
} from "@tauri-apps/plugin-notification";
import { emit } from "@tauri-apps/api/event";

const CHANNEL_LIBRARY = "digicore-library";
const ACTION_TYPE_LIBRARY = "digicore-library-actions";

let actionTypesRegistered = false;

async function ensurePermission(): Promise<boolean> {
  let granted = await isPermissionGranted();
  if (!granted) {
    const perm = await requestPermission();
    granted = perm === "granted";
  }
  return granted;
}

async function ensureActionTypes(): Promise<void> {
  if (actionTypesRegistered) return;
  try {
    await createChannel({
      id: CHANNEL_LIBRARY,
      name: "DigiCore Library",
      importance: Importance.Default,
      visibility: Visibility.Private,
    });
    await registerActionTypes([
      {
        id: ACTION_TYPE_LIBRARY,
        actions: [
          {
            id: "view-library",
            title: "View Library",
            foreground: true,
          },
        ],
      },
    ]);
    actionTypesRegistered = true;
  } catch {
    /* ignore */
  }
}

/**
 * Send a notification with optional "View Library" action.
 * When action is clicked, emits "notification-view-library" for App to handle.
 */
export async function notify(
  title: string,
  body: string,
  options?: { withViewLibrary?: boolean }
): Promise<void> {
  const granted = await ensurePermission();
  if (!granted) return;
  try {
    if (options?.withViewLibrary) {
      await ensureActionTypes();
      sendNotification({
        title,
        body,
        channelId: CHANNEL_LIBRARY,
        actionTypeId: ACTION_TYPE_LIBRARY,
      });
    } else {
      sendNotification({ title, body });
    }
  } catch {
    /* ignore */
  }
}

/** Payload from onAction when user clicks a notification action. */
interface ActionPayload {
  action?: { id?: string };
}

/**
 * Register listener for notification action clicks.
 * Call once at app startup. Returns cleanup function.
 */
export async function initNotificationActionListener(): Promise<() => void> {
  const listener = await onAction((notification) => {
    const payload = notification as ActionPayload;
    if (payload.action?.id === "view-library") {
      emit("notification-view-library", {});
    }
  });
  return () => {
    void listener.unregister();
  };
}
