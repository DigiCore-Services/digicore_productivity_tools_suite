import { describe, expect, it, beforeEach } from "vitest";
import {
  estimateWeightedEtaMs,
  expectedProviderDurationMs,
  loadProviderDurationHistory,
  recordProviderDuration,
  saveProviderDurationHistory,
  vaultSizeTierFromNoteCount,
} from "./kmsReindexEta";

describe("kmsReindexEta", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("uses defaults when no history exists", () => {
    const history = loadProviderDurationHistory();
    expect(expectedProviderDurationMs("notes", history, "small")).toBe(6_500);
    expect(expectedProviderDurationMs("notes", history, "medium")).toBe(10_000);
    expect(expectedProviderDurationMs("unknown", history, "large")).toBe(7_000);
  });

  it("records provider durations and updates moving average", () => {
    recordProviderDuration("notes", 12_000, "large");
    recordProviderDuration("notes", 8_000, "large");
    const history = loadProviderDurationHistory();
    expect(history["large:notes"].samples).toBe(2);
    expect(history["large:notes"].lastMs).toBe(8_000);
    expect(history["large:notes"].avgMs).toBe(10_000);
  });

  it("estimates weighted eta using current provider and remaining slots", () => {
    saveProviderDurationHistory({
      notes: { avgMs: 12_000, samples: 4, lastMs: 11_500 },
    });
    const eta = estimateWeightedEtaMs(
      {
        providerId: "notes",
        phase: "progress",
        providerIndex: 1,
        providerTotal: 3,
        elapsedMs: 5_000,
      },
      5_000,
      loadProviderDurationHistory(),
      "large"
    );
    expect(eta).not.toBeNull();
    expect(eta!).toBeGreaterThan(0);
  });

  it("returns zero when final provider end event arrives", () => {
    const eta = estimateWeightedEtaMs(
      {
        providerId: "clipboard",
        phase: "end",
        providerIndex: 3,
        providerTotal: 3,
        elapsedMs: 20_000,
      },
      2_000,
      loadProviderDurationHistory(),
      "small"
    );
    expect(eta).toBe(0);
  });

  it("maps vault note counts to stable tiers", () => {
    expect(vaultSizeTierFromNoteCount(100)).toBe("small");
    expect(vaultSizeTierFromNoteCount(1200)).toBe("medium");
    expect(vaultSizeTierFromNoteCount(8000)).toBe("large");
    expect(vaultSizeTierFromNoteCount(20000)).toBe("xlarge");
  });
});

