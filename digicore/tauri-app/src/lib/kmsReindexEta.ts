const KMS_REINDEX_PROVIDER_HISTORY_KEY = "digicore-kms-reindex-provider-history-v1";

type ProviderHistoryItem = {
  avgMs: number;
  samples: number;
  lastMs: number;
};

type ProviderHistoryMap = Record<string, ProviderHistoryItem>;
export type VaultSizeTier = "small" | "medium" | "large" | "xlarge";

const DEFAULT_PROVIDER_MS: Record<string, number> = {
  notes: 10_000,
  snippets: 2_500,
  clipboard: 2_000,
};

const DEFAULT_FALLBACK_MS = 4_000;
const MAX_SAMPLES = 25;
const DEFAULT_TIER: VaultSizeTier = "medium";
const TIER_MULTIPLIER: Record<VaultSizeTier, number> = {
  small: 0.65,
  medium: 1,
  large: 1.75,
  xlarge: 2.5,
};

export type ReindexProviderProgressLike = {
  providerId: string;
  phase: "start" | "progress" | "end";
  providerIndex: number;
  providerTotal: number;
  elapsedMs: number;
};

function tieredProviderKey(providerId: string, tier: VaultSizeTier): string {
  return `${tier}:${providerId}`;
}

export function vaultSizeTierFromNoteCount(noteCount: number): VaultSizeTier {
  if (!Number.isFinite(noteCount) || noteCount <= 0) return DEFAULT_TIER;
  if (noteCount < 1000) return "small";
  if (noteCount < 5000) return "medium";
  if (noteCount < 15000) return "large";
  return "xlarge";
}

export function loadProviderDurationHistory(): ProviderHistoryMap {
  try {
    const raw = localStorage.getItem(KMS_REINDEX_PROVIDER_HISTORY_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as ProviderHistoryMap;
    if (!parsed || typeof parsed !== "object") return {};
    return parsed;
  } catch {
    return {};
  }
}

export function saveProviderDurationHistory(history: ProviderHistoryMap): void {
  try {
    localStorage.setItem(KMS_REINDEX_PROVIDER_HISTORY_KEY, JSON.stringify(history));
  } catch {
    /* best effort */
  }
}

export function expectedProviderDurationMs(
  providerId: string,
  history: ProviderHistoryMap,
  tier: VaultSizeTier = DEFAULT_TIER
): number {
  const tiered = history[tieredProviderKey(providerId, tier)];
  if (tiered && Number.isFinite(tiered.avgMs) && tiered.avgMs > 0) return Math.floor(tiered.avgMs);
  const legacy = history[providerId];
  if (legacy && Number.isFinite(legacy.avgMs) && legacy.avgMs > 0) return Math.floor(legacy.avgMs);
  const base = DEFAULT_PROVIDER_MS[providerId] ?? DEFAULT_FALLBACK_MS;
  return Math.round(base * TIER_MULTIPLIER[tier]);
}

export function recordProviderDuration(
  providerId: string,
  durationMs: number,
  tier: VaultSizeTier = DEFAULT_TIER
): ProviderHistoryMap {
  if (!Number.isFinite(durationMs) || durationMs <= 0) return loadProviderDurationHistory();
  const history = loadProviderDurationHistory();
  const key = tieredProviderKey(providerId, tier);
  const prev = history[key];
  if (!prev) {
    history[key] = { avgMs: durationMs, samples: 1, lastMs: durationMs };
  } else {
    const sampleCount = Math.min(MAX_SAMPLES, prev.samples + 1);
    const alpha = 1 / sampleCount;
    const avgMs = Math.round(prev.avgMs * (1 - alpha) + durationMs * alpha);
    history[key] = { avgMs, samples: sampleCount, lastMs: durationMs };
  }
  saveProviderDurationHistory(history);
  return history;
}

function averageExpectedDurationForTier(history: ProviderHistoryMap, tier: VaultSizeTier): number {
  const entries = Object.entries(history)
    .filter(([key, v]) => key.startsWith(`${tier}:`) && Number.isFinite(v.avgMs) && v.avgMs > 0)
    .map(([, v]) => v.avgMs);
  if (entries.length > 0) {
    return Math.round(entries.reduce((a, b) => a + b, 0) / entries.length);
  }
  const defaultAvg = Object.values(DEFAULT_PROVIDER_MS).reduce((a, b) => a + b, 0) / Object.keys(DEFAULT_PROVIDER_MS).length;
  return Math.round(defaultAvg * TIER_MULTIPLIER[tier]);
}

export function estimateWeightedEtaMs(
  progress: ReindexProviderProgressLike,
  currentProviderElapsedMs: number,
  history: ProviderHistoryMap,
  tier: VaultSizeTier = DEFAULT_TIER
): number | null {
  const total = Math.max(0, progress.providerTotal);
  const idx = Math.max(1, progress.providerIndex);
  if (total === 0 || idx > total) return null;
  if (progress.phase === "end" && idx >= total) return 0;

  const currentExpected = expectedProviderDurationMs(progress.providerId, history, tier);
  const remainingCurrent =
    progress.phase === "end"
      ? 0
      : Math.max(0, currentExpected - Math.max(0, currentProviderElapsedMs));

  const remainingProviders = Math.max(0, total - idx);
  if (remainingProviders === 0) return remainingCurrent;

  const avgFuture = averageExpectedDurationForTier(history, tier);
  const futureExpected = remainingProviders * avgFuture;
  return remainingCurrent + futureExpected;
}

