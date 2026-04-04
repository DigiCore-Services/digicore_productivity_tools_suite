/**
 * Debug logging for KMS graph UI. Phase 0: gate noisy logs behind dev or localStorage.
 * Set localStorage "kms_graph_debug_log" = "1" to enable debug lines in production builds.
 */
const LS_KEY = "kms_graph_debug_log";

export function kmsGraphDebugEnabled(): boolean {
  try {
    if (import.meta.env?.DEV) return true;
  } catch {
    /* ignore */
  }
  try {
    return typeof localStorage !== "undefined" && localStorage.getItem(LS_KEY) === "1";
  } catch {
    return false;
  }
}

export const kmsGraphLog = {
  debug(...args: unknown[]) {
    if (kmsGraphDebugEnabled()) {
      console.log("[KMS][Graph]", ...args);
    }
  },
  warn(...args: unknown[]) {
    if (kmsGraphDebugEnabled()) {
      console.warn("[KMS][Graph]", ...args);
    }
  },
  /** Always emit (fetch failures and user-visible issues). */
  error(...args: unknown[]) {
    console.error("[KMS][Graph]", ...args);
  },
};
