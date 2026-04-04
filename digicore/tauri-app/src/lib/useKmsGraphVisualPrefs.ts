import { useCallback, useEffect, useState } from "react";
import { getTaurpc } from "./taurpc";

/** Dispatched after saving Knowledge Graph settings so open graph views reload prefs without restart. */
export const KMS_GRAPH_VISUAL_PREFS_EVENT = "kms-graph-visual-prefs-changed";

export function notifyKmsGraphVisualPrefsChanged(): void {
    if (typeof window === "undefined") return;
    window.dispatchEvent(new CustomEvent(KMS_GRAPH_VISUAL_PREFS_EVENT));
}

export type KmsGraphVisualPrefs = {
    bloomEnabled: boolean;
    bloomStrength: number;
    bloomRadius: number;
    bloomThreshold: number;
    hexCellRadius: number;
    hexLayerOpacity: number;
    hexStrokeWidth: number;
    hexStrokeOpacity: number;
    /** Upper cap on DPR multiplier for three-spritetext canvas (3D labels). */
    spriteLabelMaxDprScale: number;
    /** Minimum texture scale (floor) for 3D labels; helps 1x displays when zooming. */
    spriteLabelMinResScale: number;
    /** 2D graph: WebWorker initial layout when node count >= threshold; 0 = main thread only. */
    webworkerLayoutThreshold: number;
    /** Upper bound on WebWorker force ticks (client also scales a minimum from node count). */
    webworkerLayoutMaxTicks: number;
    /** WebWorker simulation stops when alpha is below this. */
    webworkerLayoutAlphaMin: number;
};

const DEFAULTS: KmsGraphVisualPrefs = {
    bloomEnabled: true,
    bloomStrength: 0.48,
    bloomRadius: 0.4,
    bloomThreshold: 0.22,
    hexCellRadius: 2.35,
    hexLayerOpacity: 0.22,
    hexStrokeWidth: 0.11,
    hexStrokeOpacity: 0.38,
    spriteLabelMaxDprScale: 2.5,
    spriteLabelMinResScale: 1.25,
    webworkerLayoutThreshold: 800,
    webworkerLayoutMaxTicks: 450,
    webworkerLayoutAlphaMin: 0.02,
};

function clamp(n: number, lo: number, hi: number): number {
    return Math.min(hi, Math.max(lo, n));
}

/**
 * Loads bloom + hex backdrop settings from app state (Configurations > Knowledge Graph).
 */
export function useKmsGraphVisualPrefs(): KmsGraphVisualPrefs {
    const [prefs, setPrefs] = useState<KmsGraphVisualPrefs>(DEFAULTS);

    const load = useCallback(async () => {
        try {
            const s = await getTaurpc().get_app_state();
            setPrefs({
                bloomEnabled: s.kms_graph_bloom_enabled ?? DEFAULTS.bloomEnabled,
                bloomStrength: clamp(
                    Number(s.kms_graph_bloom_strength ?? DEFAULTS.bloomStrength),
                    0,
                    2.5
                ),
                bloomRadius: clamp(
                    Number(s.kms_graph_bloom_radius ?? DEFAULTS.bloomRadius),
                    0,
                    1.5
                ),
                bloomThreshold: clamp(
                    Number(s.kms_graph_bloom_threshold ?? DEFAULTS.bloomThreshold),
                    0,
                    1
                ),
                hexCellRadius: clamp(
                    Number(s.kms_graph_hex_cell_radius ?? DEFAULTS.hexCellRadius),
                    0.5,
                    8
                ),
                hexLayerOpacity: clamp(
                    Number(s.kms_graph_hex_layer_opacity ?? DEFAULTS.hexLayerOpacity),
                    0,
                    1
                ),
                hexStrokeWidth: clamp(
                    Number(s.kms_graph_hex_stroke_width ?? DEFAULTS.hexStrokeWidth),
                    0.02,
                    0.5
                ),
                hexStrokeOpacity: clamp(
                    Number(s.kms_graph_hex_stroke_opacity ?? DEFAULTS.hexStrokeOpacity),
                    0,
                    1
                ),
                spriteLabelMaxDprScale: clamp(
                    Number(s.kms_graph_sprite_label_max_dpr_scale ?? DEFAULTS.spriteLabelMaxDprScale),
                    1,
                    8
                ),
                spriteLabelMinResScale: clamp(
                    Number(s.kms_graph_sprite_label_min_res_scale ?? DEFAULTS.spriteLabelMinResScale),
                    1,
                    4
                ),
                webworkerLayoutThreshold: clamp(
                    Math.floor(
                        Number(s.kms_graph_webworker_layout_threshold ?? DEFAULTS.webworkerLayoutThreshold)
                    ),
                    0,
                    500_000
                ),
                webworkerLayoutMaxTicks: clamp(
                    Math.floor(
                        Number(s.kms_graph_webworker_layout_max_ticks ?? DEFAULTS.webworkerLayoutMaxTicks)
                    ),
                    20,
                    10_000
                ),
                webworkerLayoutAlphaMin: clamp(
                    Number(s.kms_graph_webworker_layout_alpha_min ?? DEFAULTS.webworkerLayoutAlphaMin),
                    0.0005,
                    0.5
                ),
            });
        } catch {
            /* keep previous */
        }
    }, []);

    useEffect(() => {
        load();
    }, [load]);

    useEffect(() => {
        const onEvt = () => {
            load();
        };
        window.addEventListener(KMS_GRAPH_VISUAL_PREFS_EVENT, onEvt);
        return () => window.removeEventListener(KMS_GRAPH_VISUAL_PREFS_EVENT, onEvt);
    }, [load]);

    return prefs;
}
