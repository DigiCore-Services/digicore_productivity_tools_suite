/**
 * Vault-relative POSIX path for KMS Git/history APIs, from absolute note path + vault root.
 */
export function kmsVaultRelativePath(vaultPath: string | null, absolutePath: string): string {
    if (!vaultPath?.trim()) {
        return absolutePath.replace(/\\/g, "/");
    }
    const v = vaultPath.replace(/[/\\]+$/, "").replace(/\\/g, "/");
    const a = absolutePath.replace(/\\/g, "/");
    const prefix = `${v}/`;
    if (a.length >= prefix.length && a.slice(0, prefix.length).toLowerCase() === prefix.toLowerCase()) {
        return a.slice(prefix.length);
    }
    return absolutePath.replace(/\\/g, "/");
}
