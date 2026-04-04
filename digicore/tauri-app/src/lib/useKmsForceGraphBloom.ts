import { useEffect, type MutableRefObject, type DependencyList } from "react";
import * as THREE from "three";
import { UnrealBloomPass } from "three/examples/jsm/postprocessing/UnrealBloomPass.js";
import type { ForceGraphMethods } from "react-force-graph-3d";

export type KmsBloomParams = {
    strength: number;
    radius: number;
    threshold: number;
};

/**
 * Adds UnrealBloomPass to react-force-graph-3d's composer (screen-space glow).
 * Retries briefly so the ref is populated after first paint.
 */
export function useKmsForceGraphBloom(
    fgRef: MutableRefObject<ForceGraphMethods | undefined>,
    enabled: boolean,
    bloom: KmsBloomParams,
    deps: DependencyList
): void {
    useEffect(() => {
        if (!enabled) return;
        let bloomPass: UnrealBloomPass | null = null;
        let cancelled = false;
        let tryId = 0;

        const attach = () => {
            const fg = fgRef.current;
            if (!fg || cancelled) return false;
            try {
                const w = typeof window !== "undefined" ? window.innerWidth : 1024;
                const h = typeof window !== "undefined" ? window.innerHeight : 768;
                bloomPass = new UnrealBloomPass(
                    new THREE.Vector2(w, h),
                    bloom.strength,
                    bloom.radius,
                    bloom.threshold
                );
                fg.postProcessingComposer().addPass(bloomPass);
                return true;
            } catch {
                return false;
            }
        };

        const tryAttach = () => {
            if (attach()) return;
            if (tryId >= 12 || cancelled) return;
            tryId += 1;
            window.setTimeout(tryAttach, 80);
        };

        window.setTimeout(tryAttach, 50);

        return () => {
            cancelled = true;
            if (!bloomPass) return;
            const fg = fgRef.current;
            try {
                fg?.postProcessingComposer().removePass(bloomPass);
            } catch {
                /* composer disposed */
            }
            bloomPass.dispose();
        };
        // eslint-disable-next-line react-hooks/exhaustive-deps -- caller supplies graph-related deps
    }, [enabled, fgRef, bloom.strength, bloom.radius, bloom.threshold, ...deps]);
}
