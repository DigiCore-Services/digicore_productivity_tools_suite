/**
 * Shared 2D KMS graph node sizing (must match KmsGraph forceCollide and radii).
 */

export function graphNodeSizeWeight(numLinks: number, linkCentrality: number): number {
    const deg = Math.sqrt(Math.max(1, numLinks));
    const pr = Math.max(0, Math.min(1, linkCentrality));
    return deg * (0.65 + 0.55 * pr);
}

export function graphNodeCollisionRadius(numLinks: number, linkCentrality: number): number {
    return Math.sqrt(graphNodeSizeWeight(numLinks, linkCentrality)) * 5 + 35;
}

