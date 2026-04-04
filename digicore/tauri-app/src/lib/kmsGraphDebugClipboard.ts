import type { KmsDiagnosticsDto, KmsGraphDto, KmsGraphPathDto } from "../bindings";
import { getTaurpc } from "./taurpc";
import { formatIpcOrRaw, tryParseIpcError } from "./ipcError";

export type GraphPagingDebug =
    | null
    | { kind: "full" }
    | { kind: "paged"; offset: number; limit: number };

export type KmsGraphDebugClipboardInput = {
    graphView: "2d" | "3d" | "local3d";
    localCenterPath?: string | null;
    localDepth?: number;
    data: KmsGraphDto | null;
    error: string | null;
    paging: GraphPagingDebug;
    indexedNoteCount: number;
    vaultDiag: KmsDiagnosticsDto | null;
    pathFrom: string;
    pathTo: string;
    pathResult: KmsGraphPathDto | null;
    pathError: string | null;
    hoverPreviewPath?: string | null;
    extra?: Record<string, unknown>;
};

export function buildKmsGraphDebugPayload(input: KmsGraphDebugClipboardInput): Record<string, unknown> {
    const d = input.data;
    return {
        schema_version: "kms_graph_client_debug_v1",
        captured_at_iso: new Date().toISOString(),
        graph_view: input.graphView,
        local_center_path: input.localCenterPath ?? undefined,
        local_depth: input.localDepth,
        paging: input.paging,
        indexed_note_count: input.indexedNoteCount,
        vault_diag: input.vaultDiag,
        graph: d
            ? {
                  request_id: d.request_id,
                  build_time_ms: d.build_time_ms ?? null,
                  node_count: d.nodes.length,
                  edge_count: d.edges.length,
                  beam_count: d.ai_beams?.length ?? 0,
                  warnings: d.warnings,
                  pagination: d.pagination ?? null,
              }
            : null,
        graph_load_error: input.error,
        graph_load_error_ipc: input.error ? tryParseIpcError(input.error) : null,
        path_tool: {
            from: input.pathFrom,
            to: input.pathTo,
            result: input.pathResult
                ? {
                      request_id: input.pathResult.request_id,
                      found: input.pathResult.found,
                      hop_count: input.pathResult.node_paths?.length ?? 0,
                      message: input.pathResult.message ?? null,
                  }
                : null,
            error: input.pathError,
            error_ipc: input.pathError ? tryParseIpcError(input.pathError) : null,
        },
        hover_preview_path: input.hoverPreviewPath ?? undefined,
        client_notes: "Attach when reporting KMS graph or path issues.",
        ...input.extra,
    };
}

export async function copyKmsGraphDebugToClipboard(
    input: KmsGraphDebugClipboardInput,
    toast?: { toast: (o: { title: string; description?: string; variant?: "default" | "destructive" }) => void }
): Promise<void> {
    const text = JSON.stringify(buildKmsGraphDebugPayload(input), null, 2);
    try {
        await getTaurpc().copy_to_clipboard(text);
        toast?.toast({
            title: "Debug info copied",
            description: "KMS graph debug JSON is on the clipboard.",
        });
    } catch (e) {
        toast?.toast({
            title: "Copy failed",
            description: formatIpcOrRaw(e),
            variant: "destructive",
        });
    }
}

