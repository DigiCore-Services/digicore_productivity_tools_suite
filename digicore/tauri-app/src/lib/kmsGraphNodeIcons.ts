/**
 * SVG path data and 3D geometry hints for KMS graph nodes by `node_type`.
 */

/** Lucide-style simplified paths for 2D SVG rendering. */
export const KMS_NODE_ICON_PATHS: Record<string, string> = {
    note: "M9 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V9l-6-6z",
    skill: "M13 2L3 14h9l-1 8 10-12h-9l1-8z",
    image:
        "M4 5a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V5zm2 0v14h16V5H6zm2 4 4 3 4-3 4 3-8 4-4-4-4 4z",
    asset: "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8l-6-6zm-1 2 5 5h-5V4z",
};

export function kmsNodeIconPath(nodeType: string): string {
    const t = (nodeType || "note").toLowerCase();
    return KMS_NODE_ICON_PATHS[t] ?? KMS_NODE_ICON_PATHS.note;
}

/** Distinct 3D primitive per type (react-three / THREE). */
export type KmsNode3DShape = "sphere" | "cone" | "box" | "octahedron";

export function kmsNode3DShape(nodeType: string): KmsNode3DShape {
    switch ((nodeType || "note").toLowerCase()) {
        case "skill":
            return "cone";
        case "image":
            return "octahedron";
        case "asset":
            return "box";
        default:
            return "sphere";
    }
}
