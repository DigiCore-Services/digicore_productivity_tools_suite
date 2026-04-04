type KmsReindexProgressBadgeProps = {
  providerId: string;
  providerIndex: number;
  providerTotal: number;
  elapsedMs: number;
  etaRemainingMs: number | null;
  indexedTotalSoFar: number;
  variant?: "toolbar" | "panel";
};

function formatDuration(ms: number | null): string {
  if (ms == null || !Number.isFinite(ms) || ms < 0) return "--";
  const totalSeconds = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return minutes > 0 ? `${minutes}m ${seconds}s` : `${seconds}s`;
}

export default function KmsReindexProgressBadge({
  providerId,
  providerIndex,
  providerTotal,
  elapsedMs,
  etaRemainingMs,
  indexedTotalSoFar,
  variant = "toolbar",
}: KmsReindexProgressBadgeProps) {
  const providerLabel = providerId
    ? providerId.charAt(0).toUpperCase() + providerId.slice(1)
    : "Unknown";

  if (variant === "panel") {
    return (
      <div className="rounded-md border border-dc-border/50 bg-dc-bg-secondary/40 px-3 py-2 text-[10px] text-dc-text-muted">
        <div className="font-bold uppercase tracking-wider text-dc-text">Reindex</div>
        <div>
          Provider {providerIndex}/{providerTotal}: {providerLabel}
        </div>
        <div>Indexed total: {indexedTotalSoFar.toLocaleString()}</div>
        <div>
          Elapsed: {formatDuration(elapsedMs)} | ETA: {formatDuration(etaRemainingMs)}
        </div>
      </div>
    );
  }

  return (
    <div className="h-[34px] rounded-xl bg-dc-bg-secondary/40 backdrop-blur-md border border-dc-border px-3 flex items-center gap-2 text-[10px]">
      <span className="font-bold uppercase tracking-wider text-dc-text">Reindex</span>
      <span className="text-dc-text-muted">
        {providerIndex}/{providerTotal} {providerLabel}
      </span>
      <span className="text-dc-text-muted">
        {formatDuration(elapsedMs)} / ETA {formatDuration(etaRemainingMs)}
      </span>
      <span className="text-dc-text-muted">{indexedTotalSoFar.toLocaleString()}</span>
    </div>
  );
}

