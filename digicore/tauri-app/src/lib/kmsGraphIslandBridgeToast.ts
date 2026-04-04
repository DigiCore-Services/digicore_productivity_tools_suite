import { useCallback, useEffect, useRef } from "react";

type ToastFn = (opts: { title?: string; description?: string }) => void;

/**
 * When legend visibility or edge toggles expand the graph (show folder/type, enable edges),
 * island count may drop as components merge. Show a short toast once when that happens (C4).
 */
export function useIslandBridgeMergeToast(islandCount: number, toast: ToastFn) {
    const prevCountRef = useRef<number | null>(null);
    const pendingCheckRef = useRef(false);

    const markVisibilityExpanded = useCallback(() => {
        pendingCheckRef.current = true;
    }, []);

    useEffect(() => {
        const prev = prevCountRef.current;
        const n = islandCount;
        if (
            pendingCheckRef.current &&
            prev !== null &&
            n < prev &&
            prev > 0
        ) {
            const merged = prev - n;
            toast({
                title: "Graph connected",
                description: `Islands merged: ${prev} group${prev === 1 ? "" : "s"} -> ${n} (${merged} fewer).`,
            });
        }
        pendingCheckRef.current = false;
        prevCountRef.current = n;
    }, [islandCount, toast]);

    return markVisibilityExpanded;
}
