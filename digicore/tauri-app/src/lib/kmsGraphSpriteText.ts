/**
 * three-spritetext draws labels to a 2D canvas; default fontSize (90) is often too small
 * relative to display DPI, so zooming the 3D camera magnifies a blurry texture.
 * World size stays driven by `textHeight`; raising `fontSize` only adds texels.
 */
const BASE_SPRITE_FONT_PX = 90;

export type KmsGraphSpriteLabelScales = {
    maxDprScale: number;
    minResScale: number;
};

const FALLBACK_SCALES: KmsGraphSpriteLabelScales = {
    maxDprScale: 2.5,
    minResScale: 1.25,
};

export function kmsGraphSpriteCanvasFontSize(opts?: Partial<KmsGraphSpriteLabelScales>): number {
    const maxDpr = clampScale(opts?.maxDprScale ?? FALLBACK_SCALES.maxDprScale, 1, 8);
    const minRes = clampScale(opts?.minResScale ?? FALLBACK_SCALES.minResScale, 1, 4);
    if (typeof window === "undefined") {
        return Math.round(BASE_SPRITE_FONT_PX * minRes);
    }
    const dpr = window.devicePixelRatio ?? 1;
    const scale = Math.min(maxDpr, Math.max(minRes, dpr));
    return Math.round(BASE_SPRITE_FONT_PX * scale);
}

function clampScale(n: number, lo: number, hi: number): number {
    if (!Number.isFinite(n)) return lo;
    return Math.min(hi, Math.max(lo, n));
}

/** Call after other SpriteText props so the final rasterize uses the higher resolution. */
export function applyKmsGraphSpriteTextResolution(
    sprite: { fontSize: number },
    opts?: Partial<KmsGraphSpriteLabelScales>
): void {
    sprite.fontSize = kmsGraphSpriteCanvasFontSize(opts);
}
