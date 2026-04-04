import type {
    KmsGraphForceLayoutWorkerMessageOut,
    KmsGraphForceLayoutWorkerPayload,
} from "./kmsGraphForceLayoutTypes";
import KmsGraphForceWorker from "./kmsGraphForceLayout.worker?worker";

const LAYOUT_TIMEOUT_MS = 120_000;

let seq = 0;

/**
 * Runs the same d3-force model as KmsGraph 2D in a WebWorker. Returns null on failure or timeout.
 */
export async function runKmsGraphForceLayoutWorker(
    payload: KmsGraphForceLayoutWorkerPayload
): Promise<Array<{ id: string; x: number; y: number }> | null> {
    return new Promise(resolve => {
        const w = new KmsGraphForceWorker();
        const id = ++seq;
        const timer = window.setTimeout(() => {
            w.terminate();
            resolve(null);
        }, LAYOUT_TIMEOUT_MS);

        w.onmessage = (ev: MessageEvent<KmsGraphForceLayoutWorkerMessageOut>) => {
            const m = ev.data;
            if (m.id !== id) return;
            window.clearTimeout(timer);
            w.terminate();
            if (m.type === "layout-done") {
                resolve(m.positions);
            } else {
                resolve(null);
            }
        };
        w.onerror = () => {
            window.clearTimeout(timer);
            w.terminate();
            resolve(null);
        };

        w.postMessage({ type: "layout", id, payload });
    });
}
