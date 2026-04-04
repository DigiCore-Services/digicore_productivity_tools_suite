export type IpcErrorDto = { code: string; message: string; details?: string | null };

/** Stable codes returned by KMS graph-related IPC (Rust `ipc_error`). */
export const KMS_IPC_CODES = {
    GRAPH_BUILD: "KMS_GRAPH_BUILD",
    GRAPH_WORKER: "KMS_GRAPH_WORKER",
    GRAPH_DTO_EXPORT_JSON: "KMS_GRAPH_DTO_EXPORT_JSON",
    GRAPH_DTO_EXPORT_IO: "KMS_GRAPH_DTO_EXPORT_IO",
    STATE_LOCK: "KMS_STATE_LOCK",
    PATH_OUTSIDE_VAULT: "KMS_PATH_OUTSIDE_VAULT",
    REPO_NOTES: "KMS_REPO_NOTES",
    REPO_LINKS: "KMS_REPO_LINKS",
    REPO_NOTE: "KMS_REPO_NOTE",
    NOTE_NOT_INDEXED: "KMS_NOTE_NOT_INDEXED",
    VAULT_OVERRIDES_JSON: "KMS_VAULT_OVERRIDES_JSON",
    VAULT_OVERRIDES_SHAPE: "KMS_VAULT_OVERRIDES_SHAPE",
    VAULT_OVERRIDES_SERIALIZE: "KMS_VAULT_OVERRIDES_SERIALIZE",
} as const;

/** Short recovery hints keyed by `IpcErrorDto.code` (KMS graph / vault IPC). */
const KMS_IPC_HINTS: Partial<Record<string, string>> = {
    [KMS_IPC_CODES.GRAPH_BUILD]:
        "Confirm the vault is indexed, disk is reachable, and Knowledge Graph settings are valid. Retry after indexing completes.",
    [KMS_IPC_CODES.GRAPH_WORKER]:
        "The background graph task failed. Close other heavy work and try again; if it persists, restart the app.",
    [KMS_IPC_CODES.GRAPH_DTO_EXPORT_JSON]:
        "Could not serialize the graph for export. Retry; if it persists, export GraphML or diagnostics instead.",
    [KMS_IPC_CODES.GRAPH_DTO_EXPORT_IO]:
        "Could not write the export file. Check the path, permissions, and disk space.",
    [KMS_IPC_CODES.STATE_LOCK]:
        "Another operation is using KMS state. Wait a moment and retry.",
    [KMS_IPC_CODES.PATH_OUTSIDE_VAULT]:
        "Choose a path inside the active vault folder, or switch vaults to match the file you picked.",
    [KMS_IPC_CODES.REPO_NOTES]:
        "The note index could not be read. Check database permissions and try re-opening the vault.",
    [KMS_IPC_CODES.REPO_LINKS]:
        "Wiki links could not be loaded from the index. Try re-indexing the vault.",
    [KMS_IPC_CODES.REPO_NOTE]:
        "That note record could not be loaded. Re-index if the file was moved or renamed.",
    [KMS_IPC_CODES.NOTE_NOT_INDEXED]:
        "Open the note once or run indexing so this path exists in the KMS index.",
    [KMS_IPC_CODES.VAULT_OVERRIDES_JSON]:
        "Fix invalid JSON in per-vault graph overrides, or clear overrides to use global settings.",
    [KMS_IPC_CODES.VAULT_OVERRIDES_SHAPE]:
        "Per-vault overrides must be a JSON object of string keys to values. Remove unknown nesting or types.",
    [KMS_IPC_CODES.VAULT_OVERRIDES_SERIALIZE]:
        "Could not save vault overrides. Check disk space and app data permissions.",
};

/**
 * When the Rust side returns Err(ipc_error(...)), the rejection message is JSON
 * matching IpcErrorDto. Plain string errors return null.
 */
export function tryParseIpcError(message: string | undefined): IpcErrorDto | null {
    if (!message || message[0] !== "{") return null;
    try {
        const o = JSON.parse(message) as IpcErrorDto;
        if (typeof o?.code === "string" && typeof o?.message === "string") return o;
    } catch {
        /* ignore */
    }
    return null;
}

export function parseIpcErrorFromUnknown(err: unknown): IpcErrorDto | null {
    const raw = err instanceof Error ? err.message : String(err);
    return tryParseIpcError(raw);
}

export function formatIpcOrRaw(err: unknown): string {
    const raw = err instanceof Error ? err.message : String(err);
    const parsed = tryParseIpcError(raw);
    if (parsed) {
        let s = `${parsed.message} (${parsed.code})`;
        if (parsed.details) s += ` — ${parsed.details}`;
        const hint = KMS_IPC_HINTS[parsed.code];
        if (hint) s += `\n${hint}`;
        return s;
    }
    return raw;
}

