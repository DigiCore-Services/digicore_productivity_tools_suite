import React, { useId, useMemo } from "react";

export type KmsGraphHexBackdropParams = {
    cellRadius: number;
    layerOpacity: number;
    strokeWidth: number;
    strokeOpacity: number;
};

function flatTopHexPath(cx: number, cy: number, r: number): string {
    const segs: string[] = [];
    for (let i = 0; i < 6; i++) {
        const deg = -90 + i * 60;
        const rad = (deg * Math.PI) / 180;
        const x = cx + r * Math.cos(rad);
        const y = cy + r * Math.sin(rad);
        segs.push(`${i === 0 ? "M" : "L"}${x.toFixed(3)},${y.toFixed(3)}`);
    }
    return `${segs.join("")}Z`;
}

/**
 * Deep-space background (starfield + soft nebula + full hex mesh) aligned with
 * digicore/docs/kms_graph_v3_vision_1774540191272.png -- "Knowledge constellation" look.
 */
export function KmsGraphConstellationBackdrop({
    className = "",
    hex,
}: {
    className?: string;
    hex?: KmsGraphHexBackdropParams;
}) {
    const rawId = useId().replace(/:/g, "");
    const hexPatternId = `kms-hex-${rawId}`;

    const stars = useMemo(
        () =>
            Array.from({ length: 160 }, (_, i) => {
                const cx = (((i * 2654435761) >>> 0) % 10000) / 100;
                const cy = (((i * 2246822519) >>> 0) % 10000) / 100;
                const r = 0.05 + (((i * 13) % 9) / 200);
                const op = 0.12 + (((i * 7) % 55) / 100);
                return { cx, cy, r, op };
            }),
        []
    );

    const hexPattern = useMemo(() => {
        const r = hex?.cellRadius ?? 2.35;
        const dx = Math.sqrt(3) * r;
        const row = 1.5 * r;
        const w = dx;
        const h = 2 * row;
        return {
            d1: flatTopHexPath(w / 2, r, r),
            d2: flatTopHexPath(w, r + row, r),
            w,
            h,
        };
    }, [hex?.cellRadius]);

    return (
        <div
            className={`pointer-events-none absolute inset-0 z-0 overflow-hidden ${className}`}
            aria-hidden
        >
            <div className="absolute inset-0 bg-gradient-to-b from-[#0b1020] via-[#060a14] to-[#020308]" />
            <div className="absolute inset-0 bg-[radial-gradient(ellipse_85%_55%_at_50%_42%,rgba(14,165,233,0.09),transparent_62%)]" />
            <div className="absolute inset-0 bg-[radial-gradient(ellipse_50%_40%_at_80%_75%,rgba(139,92,246,0.06),transparent_55%)]" />
            <svg
                className="absolute inset-0 h-full w-full text-slate-200"
                preserveAspectRatio="none"
                viewBox="0 0 100 100"
            >
                {stars.map((s, i) => (
                    <circle key={i} cx={s.cx} cy={s.cy} r={s.r} fill="currentColor" opacity={s.op} />
                ))}
            </svg>
            <svg
                className="absolute inset-0 h-full w-full"
                style={{ opacity: hex?.layerOpacity ?? 0.22 }}
                preserveAspectRatio="none"
                viewBox="0 0 100 100"
            >
                <defs>
                    <pattern
                        id={hexPatternId}
                        width={hexPattern.w}
                        height={hexPattern.h}
                        patternUnits="userSpaceOnUse"
                    >
                        <path
                            d={hexPattern.d1}
                            fill="none"
                            stroke={`rgba(148, 210, 255, ${hex?.strokeOpacity ?? 0.38})`}
                            strokeWidth={hex?.strokeWidth ?? 0.11}
                        />
                        <path
                            d={hexPattern.d2}
                            fill="none"
                            stroke={`rgba(148, 210, 255, ${(hex?.strokeOpacity ?? 0.38) * 0.78})`}
                            strokeWidth={hex?.strokeWidth ?? 0.11}
                        />
                    </pattern>
                </defs>
                <rect width="100%" height="100%" fill={`url(#${hexPatternId})`} />
            </svg>
            <div className="absolute inset-0 bg-[radial-gradient(ellipse_at_center,transparent_0%,rgba(2,3,8,0.55)_100%)]" />
        </div>
    );
}

/** Accent colors for semantic cluster / island rows (vision mockup palette). */
export const KMS_CONSTELLATION_ISLAND_COLORS = [
    "#a855f7",
    "#22c55e",
    "#f59e0b",
    "#38bdf8",
    "#f472b6",
    "#818cf8",
    "#2dd4bf",
] as const;
