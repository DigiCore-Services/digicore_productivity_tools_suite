import { getTaurpc } from "./taurpc";

export async function fetchKmsRecentPathsFromDb(): Promise<string[]> {
    try {
        return await getTaurpc().kms_get_recent_note_paths();
    } catch {
        return [];
    }
}

export async function persistKmsRecentPaths(paths: string[]): Promise<void> {
    try {
        await getTaurpc().kms_set_recent_note_paths(paths);
    } catch (e) {
        console.warn("[KMS] Failed to persist recent note paths", e);
    }
}

export async function fetchKmsFavoritePathOrderFromDb(): Promise<string[]> {
    try {
        return await getTaurpc().kms_get_favorite_path_order();
    } catch {
        return [];
    }
}

export async function persistKmsFavoritePathOrder(paths: string[]): Promise<void> {
    try {
        await getTaurpc().kms_set_favorite_path_order(paths);
    } catch (e) {
        console.warn("[KMS] Failed to persist favorite path order", e);
    }
}
