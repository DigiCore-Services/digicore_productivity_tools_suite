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
const CHANNEL_DISCOVERY = "digicore-discovery";
const ACTION_TYPE_DISCOVERY = "digicore-discovery-actions";

let actionTypesRegistered = false;
let discoveryActionTypesRegistered = false;

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

async function ensureDiscoveryActionTypes(): Promise<void> {
  if (discoveryActionTypesRegistered) return;
  try {
    await createChannel({
      id: CHANNEL_DISCOVERY,
      name: "DigiCore Discovery",
      importance: Importance.Default,
      visibility: Visibility.Private,
    });
    await registerActionTypes([
      {
        id: ACTION_TYPE_DISCOVERY,
        actions: [
          { id: "discovery-snooze", title: "Snooze", foreground: false },
          { id: "discovery-promote", title: "Promote to Snippet", foreground: true },
          { id: "discovery-ignore", title: "Ignore", foreground: false },
        ],
      },
    ]);
    discoveryActionTypesRegistered = true;
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

/**
 * Send Discovery suggestion notification (typed phrase repeated, lower-right toast).
 * Shows Snooze, Promote to Snippet, Ignore buttons.
 */
export async function notifyDiscoverySuggestion(
  phrase: string,
  count: number
): Promise<void> {
  const granted = await ensurePermission();
  if (!granted) return;
  try {
    await ensureDiscoveryActionTypes();
    const body =
      phrase.length > 60 ? `${phrase.slice(0, 57)}...` : phrase;
    sendNotification({
      title: `Discovery (typed ${count}x)`,
      body: `"${body}"`,
      channelId: CHANNEL_DISCOVERY,
      actionTypeId: ACTION_TYPE_DISCOVERY,
    });
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
    const actionId = payload.action?.id;
    if (actionId === "view-library") {
      emit("notification-view-library", {});
    } else if (actionId === "discovery-snooze") {
      emit("discovery-action-snooze", {});
    } else if (actionId === "discovery-promote") {
      emit("discovery-action-promote", {});
    } else if (actionId === "discovery-ignore") {
      emit("discovery-action-ignore", {});
    }
  });
  return () => {
    void listener.unregister();
  };
}
