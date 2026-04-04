import { useEffect, useRef, useState } from "react";
import { emit, listen } from "@tauri-apps/api/event";
import { getTaurpc } from "../lib/taurpc";
import {
  AppearanceTransparencyRuleDto,
  ConfigUpdateDto,
  ExpansionStatsDto,
  UiPrefsDto,
  IndexingStatusDto,
  KmsIndexStatusRow,
  KmsEmbeddingPolicyDiagnosticsDto,
} from "../bindings";
import { resolveTheme, applyThemeToDocument } from "@/lib/theme";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { open, save } from "@tauri-apps/plugin-dialog";
import { ShieldAlert, ShieldCheck, ShieldX, Minimize2, X } from "lucide-react";
import { normalizeAppState } from "@/lib/normalizeState";
import { notifyKmsGraphVisualPrefsChanged } from "@/lib/useKmsGraphVisualPrefs";
import {
  KMS_GRAPH_DEFAULT_AUTO_PAGING_ENABLED,
  KMS_GRAPH_DEFAULT_AUTO_PAGING_NOTE_THRESHOLD,
  KMS_GRAPH_DEFAULT_WARN_NOTE_THRESHOLD,
} from "@/lib/kmsGraphPaging";
import {
  estimateWeightedEtaMs,
  loadProviderDurationHistory,
  recordProviderDuration,
  vaultSizeTierFromNoteCount,
} from "@/lib/kmsReindexEta";
import KmsReindexProgressBadge from "@/components/kms/KmsReindexProgressBadge";
import type { AppState } from "../types";
import { lazy, Suspense } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "./ui/dialog";

const KMS_EMBED_HEALTH_STORAGE_KEY = "digicore-kms-embedding-health-report-v1";

type KmsEmbedMigrateProgressPayload = {
  generation: number;
  phase: string;
  done: number;
  total: number;
  current_path?: string | null;
  failed: number;
  elapsed_ms: number;
  detail?: string | null;
  failures?: { path: string; message: string }[];
  failures_truncated?: boolean;
};

type KmsEmbedHealthReportStored = {
  v: 1;
  savedAt: string;
  generation: number;
  phase: string;
  totalNotes: number;
  succeeded: number;
  failed: number;
  elapsedMs: number;
  detail: string | null;
  failures: { path: string; message: string }[];
  failuresTruncated?: boolean;
};

function loadKmsEmbedHealthReport(): KmsEmbedHealthReportStored | null {
  try {
    const raw = localStorage.getItem(KMS_EMBED_HEALTH_STORAGE_KEY);
    if (!raw) return null;
    const p = JSON.parse(raw) as KmsEmbedHealthReportStored;
    if (p?.v !== 1) return null;
    return p;
  } catch {
    return null;
  }
}

function saveKmsEmbedHealthReport(p: KmsEmbedHealthReportStored) {
  try {
    localStorage.setItem(KMS_EMBED_HEALTH_STORAGE_KEY, JSON.stringify(p));
  } catch {
    /* quota */
  }
}

function storedReportToProgressView(s: KmsEmbedHealthReportStored): KmsEmbedMigrateProgressPayload {
  return {
    generation: s.generation,
    phase: s.phase,
    done: s.succeeded,
    total: s.totalNotes,
    failed: s.failed,
    elapsed_ms: s.elapsedMs,
    detail: s.detail,
    current_path: null,
    failures: s.failures,
    failures_truncated: s.failuresTruncated,
  };
}

function isKmsEmbedMigrationRunning(
  progress: KmsEmbedMigrateProgressPayload | null
): boolean {
  if (!progress) return false;
  return !["complete", "cancelled", "nothing_to_do", "error"].includes(progress.phase);
}

const AppearanceTab = lazy(() => import("./AppearanceTab").then((m) => ({ default: m.AppearanceTab })));
const AnalyticsTab = lazy(() => import("./AnalyticsTab").then((m) => ({ default: m.AnalyticsTab })));
const LogTab = lazy(() => import("./LogTab").then((m) => ({ default: m.LogTab })));

type TauriAutostart = {
  isEnabled: () => Promise<boolean>;
  enable: () => Promise<void>;
  disable: () => Promise<void>;
};

function getAutostart(): TauriAutostart | undefined {
  return (window as unknown as { __TAURI__?: { autostart?: TauriAutostart } })
    .__TAURI__?.autostart;
}

interface ConfigTabProps {
  appState: AppState | null;
  onConfigLoaded: (state: AppState) => void;
}

type SettingsBundlePreview = {
  path: string;
  schema_version: string;
  available_groups: string[];
  warnings: string[];
  valid: boolean;
};

type CopyToClipboardConfig = {
  enabled: boolean;
  image_capture_enabled: boolean;
  min_log_length: number;
  mask_cc: boolean;
  mask_ssn: boolean;
  mask_email: boolean;
  blacklist_processes: string;
  max_history_entries: number;
  json_output_enabled: boolean;
  json_output_dir: string;
  image_storage_dir: string;
  ocr_enabled: boolean;
};

type PreviewBadge = {
  label: "Ready" | "Review Warnings" | "Blocked";
  className: string;
  icon: "check" | "alert" | "blocked";
};

function getPreviewBadge(preview: SettingsBundlePreview): PreviewBadge {
  if (!preview.valid) {
    return { label: "Blocked", className: "text-red-500", icon: "blocked" };
  }
  if (preview.warnings.length > 0) {
    return {
      label: "Review Warnings",
      className: "text-amber-500",
      icon: "alert",
    };
  }
  return { label: "Ready", className: "text-emerald-500", icon: "check" };
}

const SETTINGS_GROUP_OPTIONS = [
  { id: "templates", label: "Templates" },
  { id: "sync", label: "Sync" },
  { id: "discovery", label: "Discovery" },
  { id: "ghost_suggestor", label: "Ghost Suggestor" },
  { id: "ghost_follower", label: "Ghost Follower" },
  { id: "clipboard_history", label: "Clipboard History" },
  { id: "copy_to_clipboard", label: "Copy-to-Clipboard" },
  { id: "text_expansion", label: "Text Expansion" },
  { id: "core", label: "Core" },
  { id: "script_runtime", label: "Script Runtime" },
  { id: "corpus", label: "Corpus Generation" },
  { id: "extraction", label: "Extraction Engine" },
  { id: "appearance", label: "Appearance" },
  { id: "statistics", label: "Statistics" },
  { id: "log", label: "Log" },
  { id: "semantic_search", label: "Semantic Search" },
  { id: "kms_graph", label: "Knowledge Graph" },
  { id: "kms_search_embeddings", label: "KMS Search and embeddings" },
] as const;

/** Matches `embedding_service::DEFAULT_KMS_TEXT_EMBEDDING_MODEL_ID` when the configured id is empty. */
const DEFAULT_KMS_TEXT_EMBEDDING_MODEL_ID = "BGESmallENV15";

const CONFIG_NAV_TABS = [
  ...SETTINGS_GROUP_OPTIONS,
  { id: "import_export", label: "Import/Export" },
  { id: "updates", label: "Updates" },
];

export function ConfigTab({ appState, onConfigLoaded }: ConfigTabProps) {
  const [status, setStatus] = useState("");
  const [statusError, setStatusError] = useState(false);
  const [templateDate, setTemplateDate] = useState("%Y-%m-%d");
  const [templateTime, setTemplateTime] = useState("%H:%M");
  const [syncUrl, setSyncUrl] = useState("");
  const [discoveryEnabled, setDiscoveryEnabled] = useState(false);
  const [discoveryThreshold, setDiscoveryThreshold] = useState(3);
  const [discoveryLookback, setDiscoveryLookback] = useState(30);
  const [discoveryMinLen, setDiscoveryMinLen] = useState(5);
  const [discoveryMaxLen, setDiscoveryMaxLen] = useState(30);
  const [discoveryExcludedApps, setDiscoveryExcludedApps] = useState("");
  const [discoveryExcludedTitles, setDiscoveryExcludedTitles] = useState("");
  const [ghostSuggestorEnabled, setGhostSuggestorEnabled] = useState(false);
  const [ghostSuggestorDebounce, setGhostSuggestorDebounce] = useState(80);
  const [ghostSuggestorDisplay, setGhostSuggestorDisplay] = useState(10);
  const [ghostSuggestorSnooze, setGhostSuggestorSnooze] = useState(5);
  const [ghostSuggestorOffsetX, setGhostSuggestorOffsetX] = useState(0);
  const [ghostSuggestorOffsetY, setGhostSuggestorOffsetY] = useState(0);
  const [ghostFollowerEnabled, setGhostFollowerEnabled] = useState(false);
  const [ghostFollowerHover, setGhostFollowerHover] = useState(false);
  const [ghostFollowerCollapse, setGhostFollowerCollapse] = useState(5);
  const [ghostFollowerEdge, setGhostFollowerEdge] = useState<"right" | "left">(
    "right"
  );
  const [ghostFollowerMode, setGhostFollowerMode] = useState("EdgeAnchored");
  const [ghostFollowerExpandTrigger, setGhostFollowerExpandTrigger] = useState("Click");
  const [ghostFollowerMonitor, setGhostFollowerMonitor] = useState("0");
  const [ghostFollowerOpacity, setGhostFollowerOpacity] = useState(100);
  const [clipMaxDepth, setClipMaxDepth] = useState(20);
  const [copyEnabled, setCopyEnabled] = useState(true);
  const [copyImageEnabled, setCopyImageEnabled] = useState(true);
  const [copyMinLogLength, setCopyMinLogLength] = useState(1);
  const [copyMaskCc, setCopyMaskCc] = useState(false);
  const [copyMaskSsn, setCopyMaskSsn] = useState(false);
  const [copyMaskEmail, setCopyMaskEmail] = useState(false);
  const [copyBlacklistProcesses, setCopyBlacklistProcesses] = useState("");
  const [copyJsonOutputEnabled, setCopyJsonOutputEnabled] = useState(true);
  const [copyJsonOutputDir, setCopyJsonOutputDir] = useState("");
  const [copyImageStorageDir, setCopyImageStorageDir] = useState("");
  const [copyOcrEnabled, setCopyOcrEnabled] = useState(true);
  const [loadedImageStorageDir, setLoadedImageStorageDir] = useState("");
  const [expansionLogPath, setExpansionLogPath] = useState("");
  const [expansionPaused, setExpansionPaused] = useState(false);

  const [corpusEnabled, setCorpusEnabled] = useState(false);
  const [corpusOutputDir, setCorpusOutputDir] = useState("");
  const [corpusSnapshotDir, setCorpusSnapshotDir] = useState("");
  const [corpusShortcutModifiers, setCorpusShortcutModifiers] = useState(0);
  const [corpusShortcutKey, setCorpusShortcutKey] = useState(0);

  const [extractionRowOverlapTolerance, setExtractionRowOverlapTolerance] = useState(0.35);
  const [extractionClusterThresholdFactor, setExtractionClusterThresholdFactor] = useState(1.2);
  const [extractionZoneProximity, setExtractionZoneProximity] = useState(12.0);
  const [extractionCrossZoneGapFactor, setExtractionCrossZoneGapFactor] = useState(0.35);
  const [extractionSameZoneGapFactor, setExtractionSameZoneGapFactor] = useState(0.2);
  const [extractionSignificantGapGate, setExtractionSignificantGapGate] = useState(0.8);
  const [extractionCharWidthFactor, setExtractionCharWidthFactor] = useState(0.45);
  const [extractionBridgedThreshold, setExtractionBridgedThreshold] = useState(0.4);
  const [extractionWordSpacingFactor, setExtractionWordSpacingFactor] = useState(0.2);

  const [extractionFooterTriggers, setExtractionFooterTriggers] = useState("total,sum,subtotal");
  const [extractionTableMinContiguousRows, setExtractionTableMinContiguousRows] = useState(4);
  const [extractionTableMinAvgSegments, setExtractionTableMinAvgSegments] = useState(3.1);
  const [extractionLayoutRowLookback, setExtractionLayoutRowLookback] = useState(5);
  const [extractionLayoutTableBreakThreshold, setExtractionLayoutTableBreakThreshold] = useState(3.0);
  const [extractionLayoutParagraphBreakThreshold, setExtractionLayoutParagraphBreakThreshold] = useState(3.0);
  const [extractionLayoutMaxSpaceClamp, setExtractionLayoutMaxSpaceClamp] = useState(6);
  const [extractionTablesColumnJitterTolerance, setExtractionTablesColumnJitterTolerance] = useState(15.0);
  const [extractionTablesMergeYGapMax, setExtractionTablesMergeYGapMax] = useState(80.0);
  const [extractionTablesMergeYGapMin, setExtractionTablesMergeYGapMin] = useState(15.0);

  const [extractionAdaptivePlaintextClusterFactor, setExtractionAdaptivePlaintextClusterFactor] = useState(1.2);
  const [extractionAdaptivePlaintextGapGate, setExtractionAdaptivePlaintextGapGate] = useState(0.3);
  const [extractionAdaptiveTableClusterFactor, setExtractionAdaptiveTableClusterFactor] = useState(0.5);
  const [extractionAdaptiveTableGapGate, setExtractionAdaptiveTableGapGate] = useState(1.2);
  const [extractionAdaptiveColumnClusterFactor, setExtractionAdaptiveColumnClusterFactor] = useState(0.45);
  const [extractionAdaptiveColumnGapGate, setExtractionAdaptiveColumnGapGate] = useState(0.8);

  const [extractionRefinementEntropyThreshold, setExtractionRefinementEntropyThreshold] = useState(50.0);
  const [extractionRefinementClusterThresholdModifier, setExtractionRefinementClusterThresholdModifier] = useState(0.8);
  const [extractionRefinementCrossZoneGapModifier, setExtractionRefinementCrossZoneGapModifier] = useState(1.2);

  const [extractionAdaptivePlaintextCrossFactor, setExtractionAdaptivePlaintextCrossFactor] = useState(1.0);
  const [extractionAdaptiveTableCrossFactor, setExtractionAdaptiveTableCrossFactor] = useState(0.25);
  const [extractionAdaptiveColumnCrossFactor, setExtractionAdaptiveColumnCrossFactor] = useState(0.8);

  const [extractionClassifierGutterWeight, setExtractionClassifierGutterWeight] = useState(15.0);
  const [extractionClassifierDensityWeight, setExtractionClassifierDensityWeight] = useState(10.0);
  const [extractionClassifierMulticolumnDensityMax, setExtractionClassifierMulticolumnDensityMax] = useState(0.4);
  const [extractionClassifierTableDensityMin, setExtractionClassifierTableDensityMin] = useState(1.0);
  const [extractionClassifierTableEntropyMin, setExtractionClassifierTableEntropyMin] = useState(40.0);

  const [extractionColumnsMinContiguousRows, setExtractionColumnsMinContiguousRows] = useState(3);
  const [extractionColumnsGutterGapFactor, setExtractionColumnsGutterGapFactor] = useState(5.0);
  const [extractionColumnsGutterVoidTolerance, setExtractionColumnsGutterVoidTolerance] = useState(0.7);
  const [extractionColumnsEdgeMarginTolerance, setExtractionColumnsEdgeMarginTolerance] = useState(30.0);

  const [extractionHeadersMaxWidthRatio, setExtractionHeadersMaxWidthRatio] = useState(0.75);
  const [extractionHeadersCenteredTolerance, setExtractionHeadersCenteredTolerance] = useState(0.12);
  const [extractionHeadersH1SizeMultiplier, setExtractionHeadersH1SizeMultiplier] = useState(1.6);
  const [extractionHeadersH2SizeMultiplier, setExtractionHeadersH2SizeMultiplier] = useState(1.3);
  const [extractionHeadersH3SizeMultiplier, setExtractionHeadersH3SizeMultiplier] = useState(1.2);

  const [extractionScoringJitterPenaltyWeight, setExtractionScoringJitterPenaltyWeight] = useState(0.4);
  const [extractionScoringSizePenaltyWeight, setExtractionScoringSizePenaltyWeight] = useState(0.1);
  const [extractionScoringLowConfidenceThreshold, setExtractionScoringLowConfidenceThreshold] = useState(0.6);

  const [kmsGraphKMeansMaxK, setKmsGraphKMeansMaxK] = useState(10);
  const [kmsGraphKMeansIterations, setKmsGraphKMeansIterations] = useState(15);
  const [kmsGraphAiBeamMaxNodes, setKmsGraphAiBeamMaxNodes] = useState(400);
  const [kmsGraphAiBeamSimilarityThreshold, setKmsGraphAiBeamSimilarityThreshold] = useState(0.9);
  const [kmsGraphAiBeamMaxEdges, setKmsGraphAiBeamMaxEdges] = useState(20);
  const [kmsGraphEnableAiBeams, setKmsGraphEnableAiBeams] = useState(true);
  const [kmsGraphEnableSemanticClustering, setKmsGraphEnableSemanticClustering] = useState(true);
  const [kmsGraphEnableLeidenCommunities, setKmsGraphEnableLeidenCommunities] = useState(true);
  const [kmsGraphSemanticMaxNotes, setKmsGraphSemanticMaxNotes] = useState(2500);
  const [kmsGraphWarnNoteThreshold, setKmsGraphWarnNoteThreshold] = useState(
    KMS_GRAPH_DEFAULT_WARN_NOTE_THRESHOLD
  );
  const [kmsGraphBeamMaxPairChecks, setKmsGraphBeamMaxPairChecks] = useState(200000);
  const [kmsGraphEnableSemanticKnnEdges, setKmsGraphEnableSemanticKnnEdges] = useState(true);
  const [kmsGraphSemanticKnnPerNote, setKmsGraphSemanticKnnPerNote] = useState(5);
  const [kmsGraphSemanticKnnMinSimilarity, setKmsGraphSemanticKnnMinSimilarity] = useState(0.82);
  const [kmsGraphSemanticKnnMaxEdges, setKmsGraphSemanticKnnMaxEdges] = useState(8000);
  const [kmsGraphSemanticKnnMaxPairChecks, setKmsGraphSemanticKnnMaxPairChecks] = useState(400000);
  const [kmsGraphPagerankIterations, setKmsGraphPagerankIterations] = useState(48);
  const [kmsGraphPagerankLocalIterations, setKmsGraphPagerankLocalIterations] = useState(32);
  const [kmsGraphPagerankDamping, setKmsGraphPagerankDamping] = useState(0.85);
  const [kmsGraphPagerankScope, setKmsGraphPagerankScope] = useState("auto");
  const [kmsGraphBackgroundWikiPagerankEnabled, setKmsGraphBackgroundWikiPagerankEnabled] =
    useState(true);
  const [kmsGraphSpriteLabelMaxDprScale, setKmsGraphSpriteLabelMaxDprScale] = useState(2.5);
  const [kmsGraphSpriteLabelMinResScale, setKmsGraphSpriteLabelMinResScale] = useState(1.25);
  const [kmsGraphWebworkerLayoutThreshold, setKmsGraphWebworkerLayoutThreshold] = useState(800);
  const [kmsGraphWebworkerLayoutMaxTicks, setKmsGraphWebworkerLayoutMaxTicks] = useState(450);
  const [kmsGraphWebworkerLayoutAlphaMin, setKmsGraphWebworkerLayoutAlphaMin] = useState(0.02);
  const [kmsGraphAutoPagingEnabled, setKmsGraphAutoPagingEnabled] = useState(
    KMS_GRAPH_DEFAULT_AUTO_PAGING_ENABLED
  );
  const [kmsGraphAutoPagingNoteThreshold, setKmsGraphAutoPagingNoteThreshold] = useState(
    KMS_GRAPH_DEFAULT_AUTO_PAGING_NOTE_THRESHOLD
  );
  const [kmsGraphBloomEnabled, setKmsGraphBloomEnabled] = useState(true);
  const [kmsGraphBloomStrength, setKmsGraphBloomStrength] = useState(0.48);
  const [kmsGraphBloomRadius, setKmsGraphBloomRadius] = useState(0.4);
  const [kmsGraphBloomThreshold, setKmsGraphBloomThreshold] = useState(0.22);
  const [kmsGraphHexCellRadius, setKmsGraphHexCellRadius] = useState(2.35);
  const [kmsGraphHexLayerOpacity, setKmsGraphHexLayerOpacity] = useState(0.22);
  const [kmsGraphHexStrokeWidth, setKmsGraphHexStrokeWidth] = useState(0.11);
  const [kmsGraphHexStrokeOpacity, setKmsGraphHexStrokeOpacity] = useState(0.38);

  const [kmsGraphTemporalWindowEnabled, setKmsGraphTemporalWindowEnabled] = useState(false);
  const [kmsGraphTemporalDefaultDays, setKmsGraphTemporalDefaultDays] = useState(0);
  const [kmsGraphTemporalIncludeNoMtime, setKmsGraphTemporalIncludeNoMtime] = useState(true);
  const [kmsGraphTemporalEdgeRecencyEnabled, setKmsGraphTemporalEdgeRecencyEnabled] =
    useState(false);
  const [kmsGraphTemporalEdgeRecencyStrength, setKmsGraphTemporalEdgeRecencyStrength] =
    useState(1.0);
  const [kmsGraphTemporalEdgeRecencyHalfLifeDays, setKmsGraphTemporalEdgeRecencyHalfLifeDays] =
    useState(30.0);
  const [kmsSearchMinSimilarity, setKmsSearchMinSimilarity] = useState(0.0);
  const [kmsSearchIncludeEmbeddingDiagnostics, setKmsSearchIncludeEmbeddingDiagnostics] =
    useState(true);
  const [kmsSearchDefaultMode, setKmsSearchDefaultMode] = useState<
    "Hybrid" | "Semantic" | "Keyword"
  >("Hybrid");
  const [kmsSearchDefaultLimit, setKmsSearchDefaultLimit] = useState(20);
  const [kmsEmbeddingModelId, setKmsEmbeddingModelId] = useState("");
  const [kmsEmbeddingBatchNotes, setKmsEmbeddingBatchNotes] = useState(8);
  const [kmsEmbeddingChunkEnabled, setKmsEmbeddingChunkEnabled] = useState(false);
  const [kmsEmbeddingChunkMaxChars, setKmsEmbeddingChunkMaxChars] = useState(2048);
  const [kmsEmbeddingChunkOverlapChars, setKmsEmbeddingChunkOverlapChars] = useState(128);
  const [kmsEmbedMigrateLog, setKmsEmbedMigrateLog] = useState<string | null>(null);
  const [kmsEmbedModalOpen, setKmsEmbedModalOpen] = useState(false);
  const [kmsEmbedArchivedReportOpen, setKmsEmbedArchivedReportOpen] = useState(false);
  const [kmsEmbedHealthStored, setKmsEmbedHealthStored] = useState<KmsEmbedHealthReportStored | null>(
    () => (typeof window === "undefined" ? null : loadKmsEmbedHealthReport())
  );
  const [kmsEmbedProgress, setKmsEmbedProgress] = useState<KmsEmbedMigrateProgressPayload | null>(null);
  const [kmsEmbedPolicyDiag, setKmsEmbedPolicyDiag] =
    useState<KmsEmbeddingPolicyDiagnosticsDto | null>(null);
  const [kmsEmbedDiagnosticLogPath, setKmsEmbedDiagnosticLogPath] = useState<string | null>(null);

  const embedReportForModal: KmsEmbedMigrateProgressPayload | null =
    kmsEmbedProgress ??
    (kmsEmbedArchivedReportOpen && kmsEmbedHealthStored
      ? storedReportToProgressView(kmsEmbedHealthStored)
      : null);
  const kmsEmbedRunning = isKmsEmbedMigrationRunning(kmsEmbedProgress);
  const kmsEmbedStripReport: KmsEmbedMigrateProgressPayload | null =
    kmsEmbedProgress ??
    (kmsEmbedHealthStored ? storedReportToProgressView(kmsEmbedHealthStored) : null);
  const showKmsEmbedPersistentStrip =
    kmsEmbedStripReport != null || Boolean(kmsEmbedMigrateLog);

  const [indexingStatus, setIndexingStatus] = useState<IndexingStatusDto[]>([]);
  const [isIndexing, setIsIndexing] = useState<string | null>(null);
  const [expandedFailures, setExpandedFailures] = useState<string | null>(null);
  const [failureDetails, setFailureDetails] = useState<KmsIndexStatusRow[]>([]);

  const [autostart, setAutostart] = useState(false);
  const [theme, setTheme] = useState("light");
  const [updateChecking, setUpdateChecking] = useState(false);
  const [updateStatus, setUpdateStatus] = useState("");
  const [settingsTransferMode, setSettingsTransferMode] = useState<
    "export" | "import"
  >("export");
  const [settingsTransferScope, setSettingsTransferScope] = useState<
    "all" | "selected"
  >("all");
  const [selectedSettingsGroups, setSelectedSettingsGroups] = useState<string[]>(
    SETTINGS_GROUP_OPTIONS.map((g) => g.id)
  );
  const [importPreview, setImportPreview] = useState<SettingsBundlePreview | null>(
    null
  );
  const [importWarningsAcknowledged, setImportWarningsAcknowledged] =
    useState(false);
  const [activeGroup, setActiveGroup] = useState<string>(
    () => localStorage.getItem("digicore-config-subtab") || "templates"
  );
  const [lastSavedGroup, setLastSavedGroup] = useState<string | null>(null);
  const [reindexProgress, setReindexProgress] = useState<{
    providerId: string;
    phase: "start" | "progress" | "end";
    elapsedMs: number;
    etaRemainingMs: number | null;
    providerIndex: number;
    providerTotal: number;
    indexedTotalSoFar: number;
  } | null>(null);
  const providerStartElapsedRef = useRef<Record<string, number>>({});
  const indexedNoteCountRef = useRef(0);

  const showFeedback = (groupName: string) => {
    setLastSavedGroup(groupName);
    setTimeout(() => {
      setLastSavedGroup((prev) => (prev === groupName ? null : prev));
    }, 3000);
  };

  useEffect(() => {
    localStorage.setItem("digicore-config-subtab", activeGroup);
  }, [activeGroup]);

  useEffect(() => {
    const noteStatus = indexingStatus.find((s) => s.category === "notes");
    indexedNoteCountRef.current = noteStatus?.total_count ?? noteStatus?.indexed_count ?? 0;
  }, [indexingStatus]);

  useEffect(() => {
    let offProgress: (() => void) | undefined;
    let offComplete: (() => void) | undefined;
    void listen("kms-reindex-provider-progress", (ev) => {
      const p = (ev.payload ?? {}) as {
        provider_id?: string;
        phase?: string;
        elapsed_ms?: number;
        eta_remaining_ms?: number | null;
        provider_index?: number;
        provider_total?: number;
        indexed_total_so_far?: number;
      };
      const phase =
        p.phase === "start" || p.phase === "progress" || p.phase === "end"
          ? p.phase
          : "progress";
      const providerId = p.provider_id ?? "unknown";
      const runElapsedMs = typeof p.elapsed_ms === "number" ? p.elapsed_ms : 0;
      if (phase === "start") {
        providerStartElapsedRef.current[providerId] = runElapsedMs;
      }
      const providerStartElapsed = providerStartElapsedRef.current[providerId] ?? runElapsedMs;
      const providerElapsedMs = Math.max(0, runElapsedMs - providerStartElapsed);
      let history = loadProviderDurationHistory();
      const tier = vaultSizeTierFromNoteCount(indexedNoteCountRef.current);
      if (phase === "end") {
        history = recordProviderDuration(providerId, providerElapsedMs, tier);
        delete providerStartElapsedRef.current[providerId];
      }
      const weightedEtaMs = estimateWeightedEtaMs(
        {
          providerId,
          phase,
          providerIndex: typeof p.provider_index === "number" ? p.provider_index : 0,
          providerTotal: typeof p.provider_total === "number" ? p.provider_total : 0,
          elapsedMs: runElapsedMs,
        },
        providerElapsedMs,
        history,
        tier
      );
      setReindexProgress({
        providerId,
        phase,
        elapsedMs: typeof p.elapsed_ms === "number" ? p.elapsed_ms : 0,
        etaRemainingMs: weightedEtaMs,
        providerIndex: typeof p.provider_index === "number" ? p.provider_index : 0,
        providerTotal: typeof p.provider_total === "number" ? p.provider_total : 0,
        indexedTotalSoFar:
          typeof p.indexed_total_so_far === "number" ? p.indexed_total_so_far : 0,
      });
    }).then((fn) => {
      offProgress = fn;
    });

    void listen("kms-reindex-complete", () => {
      providerStartElapsedRef.current = {};
      setReindexProgress(null);
    }).then((fn) => {
      offComplete = fn;
    });

    return () => {
      offProgress?.();
      offComplete?.();
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen<KmsEmbedMigrateProgressPayload>("kms-embedding-migrate-progress", (ev) => {
      const p = ev.payload;
      setKmsEmbedArchivedReportOpen(false);
      const failures = p.failures ?? [];
      const normalized: KmsEmbedMigrateProgressPayload = {
        ...p,
        failures,
        failures_truncated: p.failures_truncated ?? false,
      };
      setKmsEmbedProgress(normalized);
      setKmsEmbedMigrateLog(
        `[gen ${p.generation}] ${p.phase}: ${p.done}/${p.total} failed=${p.failed}` +
          (p.current_path ? ` ${p.current_path}` : "") +
          (p.detail ? ` — ${p.detail}` : "")
      );
      if (
        p.phase === "starting" ||
        p.phase === "batch" ||
        p.phase === "nothing_to_do" ||
        p.phase === "error"
      ) {
        setKmsEmbedModalOpen(true);
      }
      if (
        p.phase === "complete" ||
        p.phase === "cancelled" ||
        p.phase === "nothing_to_do" ||
        p.phase === "error"
      ) {
        const rep: KmsEmbedHealthReportStored = {
          v: 1,
          savedAt: new Date().toISOString(),
          generation: p.generation,
          phase: p.phase,
          totalNotes: p.total,
          succeeded: p.done,
          failed: p.failed,
          elapsedMs: p.elapsed_ms,
          detail: p.detail ?? null,
          failures,
          failuresTruncated: p.failures_truncated ?? false,
        };
        saveKmsEmbedHealthReport(rep);
        setKmsEmbedHealthStored(rep);
        void getTaurpc()
          .kms_get_embedding_policy_diagnostics()
          .then(setKmsEmbedPolicyDiag)
          .catch(() => {});
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    const s = loadKmsEmbedHealthReport();
    if (s) setKmsEmbedHealthStored(s);
  }, [activeGroup]);

  useEffect(() => {
    if (activeGroup !== "kms_search_embeddings") return;
    void getTaurpc()
      .kms_get_embedding_diagnostic_log_path()
      .then(setKmsEmbedDiagnosticLogPath)
      .catch(() => setKmsEmbedDiagnosticLogPath(null));
  }, [activeGroup]);

  useEffect(() => {
    let cancelled = false;
    const tick = () => {
      void getTaurpc()
        .kms_get_embedding_policy_diagnostics()
        .then((d) => {
          if (!cancelled) setKmsEmbedPolicyDiag(d);
        })
        .catch(() => {
          if (!cancelled) setKmsEmbedPolicyDiag(null);
        });
    };
    tick();
    const id = window.setInterval(tick, 5000);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, []);

  useEffect(() => {
    if (appState) {
      setTemplateDate(appState.template_date_format || "%Y-%m-%d");
      setTemplateTime(appState.template_time_format || "%H:%M");
      setSyncUrl(appState.sync_url || "");
      setDiscoveryEnabled(!!appState.discovery_enabled);
      setDiscoveryThreshold(appState.discovery_threshold ?? 3);
      setDiscoveryLookback(appState.discovery_lookback ?? 30);
      setDiscoveryMinLen(appState.discovery_min_len ?? 5);
      setDiscoveryMaxLen(appState.discovery_max_len ?? 30);
      setDiscoveryExcludedApps(appState.discovery_excluded_apps || "");
      setDiscoveryExcludedTitles(appState.discovery_excluded_window_titles || "");
      setGhostSuggestorEnabled(!!appState.ghost_suggestor_enabled);
      setGhostSuggestorDebounce(appState.ghost_suggestor_debounce_ms ?? 80);
      setGhostSuggestorDisplay(appState.ghost_suggestor_display_secs ?? 10);
      setGhostSuggestorSnooze(appState.ghost_suggestor_snooze_duration_mins ?? 5);
      setGhostSuggestorOffsetX(appState.ghost_suggestor_offset_x ?? 0);
      setGhostSuggestorOffsetY(appState.ghost_suggestor_offset_y ?? 0);
      setGhostFollowerEnabled(!!appState.ghost_follower_enabled);
      setGhostFollowerHover(!!appState.ghost_follower_hover_preview);
      setGhostFollowerCollapse(appState.ghost_follower_collapse_delay_secs ?? 5);
      setGhostFollowerEdge(appState.ghost_follower_edge_right ? "right" : "left");
      setGhostFollowerMonitor(
        String(appState.ghost_follower_monitor_anchor ?? 0)
      );
      setGhostFollowerOpacity(appState.ghost_follower_opacity ?? 100);
      setGhostFollowerMode(appState.ghost_follower_mode || "EdgeAnchored");
      setGhostFollowerExpandTrigger(appState.ghost_follower_expand_trigger || "Click");
      setClipMaxDepth(appState.clip_history_max_depth ?? 20);
      setExpansionLogPath(appState.expansion_log_path || "");
      setExpansionPaused(!!appState.expansion_paused);

      setCorpusEnabled(!!appState.corpus_enabled);
      setCorpusOutputDir(appState.corpus_output_dir || "");
      setCorpusSnapshotDir(appState.corpus_snapshot_dir || "");
      setCorpusShortcutModifiers(appState.corpus_shortcut_modifiers ?? 0);
      setCorpusShortcutKey(appState.corpus_shortcut_key ?? 0);

      setExtractionRowOverlapTolerance(appState.extraction_row_overlap_tolerance ?? 0.35);
      setExtractionClusterThresholdFactor(appState.extraction_cluster_threshold_factor ?? 1.2);
      setExtractionZoneProximity(appState.extraction_zone_proximity ?? 12.0);
      setExtractionCrossZoneGapFactor(appState.extraction_cross_zone_gap_factor ?? 0.35);
      setExtractionSameZoneGapFactor(appState.extraction_same_zone_gap_factor ?? 0.2);
      setExtractionSignificantGapGate(appState.extraction_significant_gap_gate ?? 0.8);
      setExtractionCharWidthFactor(appState.extraction_char_width_factor ?? 0.45);
      setExtractionBridgedThreshold(appState.extraction_bridged_threshold ?? 0.4);
      setExtractionWordSpacingFactor(appState.extraction_word_spacing_factor ?? 0.2);
      setExtractionFooterTriggers(appState.extraction_footer_triggers || "");
      setExtractionTableMinContiguousRows(appState.extraction_table_min_contiguous_rows ?? 4);
      setExtractionTableMinAvgSegments(appState.extraction_table_min_avg_segments ?? 3.1);
      setExtractionLayoutRowLookback(appState.extraction_layout_row_lookback ?? 5);
      setExtractionLayoutTableBreakThreshold(appState.extraction_layout_table_break_threshold ?? 3.0);
      setExtractionLayoutParagraphBreakThreshold(appState.extraction_layout_paragraph_break_threshold ?? 3.0);
      setExtractionLayoutMaxSpaceClamp(appState.extraction_layout_max_space_clamp ?? 6);
      setExtractionTablesColumnJitterTolerance(appState.extraction_tables_column_jitter_tolerance ?? 15.0);
      setExtractionTablesMergeYGapMax(appState.extraction_tables_merge_y_gap_max ?? 80.0);
      setExtractionTablesMergeYGapMin(appState.extraction_tables_merge_y_gap_min ?? 15.0);

      setExtractionAdaptivePlaintextClusterFactor(appState.extraction_adaptive_plaintext_cluster_factor ?? 1.2);
      setExtractionAdaptivePlaintextGapGate(appState.extraction_adaptive_plaintext_gap_gate ?? 0.3);
      setExtractionAdaptiveTableClusterFactor(appState.extraction_adaptive_table_cluster_factor ?? 0.5);
      setExtractionAdaptiveTableGapGate(appState.extraction_adaptive_table_gap_gate ?? 1.2);
      setExtractionAdaptiveColumnClusterFactor(appState.extraction_adaptive_column_cluster_factor ?? 0.45);
      setExtractionAdaptiveColumnGapGate(appState.extraction_adaptive_column_gap_gate ?? 0.8);

      setExtractionAdaptivePlaintextCrossFactor(appState.extraction_adaptive_plaintext_cross_factor ?? 1.0);
      setExtractionAdaptiveTableCrossFactor(appState.extraction_adaptive_table_cross_factor ?? 0.25);
      setExtractionAdaptiveColumnCrossFactor(appState.extraction_adaptive_column_cross_factor ?? 0.8);

      setExtractionRefinementEntropyThreshold(appState.extraction_refinement_entropy_threshold ?? 50.0);
      setExtractionRefinementClusterThresholdModifier(appState.extraction_refinement_cluster_threshold_modifier ?? 0.8);
      setExtractionRefinementCrossZoneGapModifier(appState.extraction_refinement_cross_zone_gap_modifier ?? 1.2);

      setExtractionClassifierGutterWeight(appState.extraction_classifier_gutter_weight ?? 15.0);
      setExtractionClassifierDensityWeight(appState.extraction_classifier_density_weight ?? 10.0);
      setExtractionClassifierMulticolumnDensityMax(appState.extraction_classifier_multicolumn_density_max ?? 0.4);
      setExtractionClassifierTableDensityMin(appState.extraction_classifier_table_density_min ?? 1.0);
      setExtractionClassifierTableEntropyMin(appState.extraction_classifier_table_entropy_min ?? 40.0);

      setExtractionColumnsMinContiguousRows(appState.extraction_columns_min_contiguous_rows ?? 3);
      setExtractionColumnsGutterGapFactor(appState.extraction_columns_gutter_gap_factor ?? 5.0);
      setExtractionColumnsGutterVoidTolerance(appState.extraction_columns_gutter_void_tolerance ?? 0.7);
      setExtractionColumnsEdgeMarginTolerance(appState.extraction_columns_edge_margin_tolerance ?? 30.0);

      setExtractionHeadersMaxWidthRatio(appState.extraction_headers_max_width_ratio ?? 0.75);
      setExtractionHeadersCenteredTolerance(appState.extraction_headers_centered_tolerance ?? 0.12);
      setExtractionHeadersH1SizeMultiplier(appState.extraction_headers_h1_size_multiplier ?? 1.6);
      setExtractionHeadersH2SizeMultiplier(appState.extraction_headers_h2_size_multiplier ?? 1.3);
      setExtractionHeadersH3SizeMultiplier(appState.extraction_headers_h3_size_multiplier ?? 1.2);

      setExtractionScoringJitterPenaltyWeight(appState.extraction_scoring_jitter_penalty_weight ?? 0.4);
      setExtractionScoringSizePenaltyWeight(appState.extraction_scoring_size_penalty_weight ?? 0.1);
      setExtractionScoringLowConfidenceThreshold(appState.extraction_scoring_low_confidence_threshold ?? 0.6);

      setKmsGraphKMeansMaxK(clampInt(appState.kms_graph_k_means_max_k ?? 10, 2, 200));
      setKmsGraphKMeansIterations(clampInt(appState.kms_graph_k_means_iterations ?? 15, 1, 500));
      setKmsGraphAiBeamMaxNodes(clampInt(appState.kms_graph_ai_beam_max_nodes ?? 400, 2, 50000));
      setKmsGraphAiBeamSimilarityThreshold(
        Math.min(1, Math.max(0, appState.kms_graph_ai_beam_similarity_threshold ?? 0.9))
      );
      setKmsGraphAiBeamMaxEdges(clampInt(appState.kms_graph_ai_beam_max_edges ?? 20, 0, 500));
      setKmsGraphEnableAiBeams(appState.kms_graph_enable_ai_beams ?? true);
      setKmsGraphEnableSemanticClustering(appState.kms_graph_enable_semantic_clustering ?? true);
      setKmsGraphEnableLeidenCommunities(appState.kms_graph_enable_leiden_communities ?? true);
      setKmsGraphSemanticMaxNotes(clampInt(appState.kms_graph_semantic_max_notes ?? 2500, 0, 1000000));
      setKmsGraphWarnNoteThreshold(
        clampInt(appState.kms_graph_warn_note_threshold ?? KMS_GRAPH_DEFAULT_WARN_NOTE_THRESHOLD, 0, 1000000)
      );
      setKmsGraphBeamMaxPairChecks(clampInt(appState.kms_graph_beam_max_pair_checks ?? 200000, 0, 50_000_000));
      setKmsGraphEnableSemanticKnnEdges(appState.kms_graph_enable_semantic_knn_edges ?? true);
      setKmsGraphSemanticKnnPerNote(
        clampInt(appState.kms_graph_semantic_knn_per_note ?? 5, 1, 30)
      );
      setKmsGraphSemanticKnnMinSimilarity(
        Math.min(0.999, Math.max(0.5, appState.kms_graph_semantic_knn_min_similarity ?? 0.82))
      );
      setKmsGraphSemanticKnnMaxEdges(
        clampInt(appState.kms_graph_semantic_knn_max_edges ?? 8000, 0, 500_000)
      );
      setKmsGraphSemanticKnnMaxPairChecks(
        clampInt(appState.kms_graph_semantic_knn_max_pair_checks ?? 400000, 0, 50_000_000)
      );
      setKmsGraphPagerankIterations(clampInt(appState.kms_graph_pagerank_iterations ?? 48, 4, 500));
      setKmsGraphPagerankLocalIterations(
        clampInt(appState.kms_graph_pagerank_local_iterations ?? 32, 4, 500)
      );
      setKmsGraphPagerankDamping(
        Math.min(0.99, Math.max(0.5, appState.kms_graph_pagerank_damping ?? 0.85))
      );
      setKmsGraphPagerankScope(appState.kms_graph_pagerank_scope?.trim() || "auto");
      setKmsGraphBackgroundWikiPagerankEnabled(
        appState.kms_graph_background_wiki_pagerank_enabled ?? true
      );
      setKmsGraphTemporalWindowEnabled(appState.kms_graph_temporal_window_enabled ?? false);
      setKmsGraphTemporalDefaultDays(
        clampInt(appState.kms_graph_temporal_default_days ?? 0, 0, 3650)
      );
      setKmsGraphTemporalIncludeNoMtime(
        appState.kms_graph_temporal_include_notes_without_mtime ?? true
      );
      setKmsGraphTemporalEdgeRecencyEnabled(
        appState.kms_graph_temporal_edge_recency_enabled ?? false
      );
      setKmsGraphTemporalEdgeRecencyStrength(
        Math.min(1, Math.max(0, appState.kms_graph_temporal_edge_recency_strength ?? 1.0))
      );
      setKmsGraphTemporalEdgeRecencyHalfLifeDays(
        Math.max(0.1, appState.kms_graph_temporal_edge_recency_half_life_days ?? 30.0)
      );
      setKmsSearchMinSimilarity(Math.min(1, Math.max(0, appState.kms_search_min_similarity ?? 0)));
      setKmsSearchIncludeEmbeddingDiagnostics(
        appState.kms_search_include_embedding_diagnostics ?? true
      );
      {
        const rawMode = (appState.kms_search_default_mode ?? "Hybrid").trim();
        setKmsSearchDefaultMode(
          rawMode === "Semantic" || rawMode === "Keyword" ? rawMode : "Hybrid"
        );
      }
      setKmsSearchDefaultLimit(
        Math.min(200, Math.max(1, appState.kms_search_default_limit ?? 20))
      );
      setKmsEmbeddingModelId((appState.kms_embedding_model_id ?? "").trim());
      setKmsEmbeddingBatchNotes(
        Math.min(500, Math.max(1, appState.kms_embedding_batch_notes_per_tick ?? 8))
      );
      setKmsEmbeddingChunkEnabled(appState.kms_embedding_chunk_enabled ?? false);
      setKmsEmbeddingChunkMaxChars(
        Math.min(8192, Math.max(256, appState.kms_embedding_chunk_max_chars ?? 2048))
      );
      setKmsEmbeddingChunkOverlapChars(
        Math.min(4096, Math.max(0, appState.kms_embedding_chunk_overlap_chars ?? 128))
      );
      setKmsGraphSpriteLabelMaxDprScale(
        Math.min(8, Math.max(1, appState.kms_graph_sprite_label_max_dpr_scale ?? 2.5))
      );
      setKmsGraphSpriteLabelMinResScale(
        Math.min(4, Math.max(1, appState.kms_graph_sprite_label_min_res_scale ?? 1.25))
      );
      setKmsGraphWebworkerLayoutThreshold(
        clampInt(appState.kms_graph_webworker_layout_threshold ?? 800, 0, 500_000)
      );
      setKmsGraphWebworkerLayoutMaxTicks(
        clampInt(appState.kms_graph_webworker_layout_max_ticks ?? 450, 20, 10_000)
      );
      setKmsGraphWebworkerLayoutAlphaMin(
        Math.min(0.5, Math.max(0.0005, appState.kms_graph_webworker_layout_alpha_min ?? 0.02))
      );
      setKmsGraphAutoPagingEnabled(
        appState.kms_graph_auto_paging_enabled ?? KMS_GRAPH_DEFAULT_AUTO_PAGING_ENABLED
      );
      setKmsGraphAutoPagingNoteThreshold(
        clampInt(
          appState.kms_graph_auto_paging_note_threshold ?? KMS_GRAPH_DEFAULT_AUTO_PAGING_NOTE_THRESHOLD,
          1,
          1_000_000
        )
      );
      setKmsGraphBloomEnabled(appState.kms_graph_bloom_enabled ?? true);
      setKmsGraphBloomStrength(
        Math.min(2.5, Math.max(0, appState.kms_graph_bloom_strength ?? 0.48))
      );
      setKmsGraphBloomRadius(
        Math.min(1.5, Math.max(0, appState.kms_graph_bloom_radius ?? 0.4))
      );
      setKmsGraphBloomThreshold(
        Math.min(1, Math.max(0, appState.kms_graph_bloom_threshold ?? 0.22))
      );
      setKmsGraphHexCellRadius(
        Math.min(8, Math.max(0.5, appState.kms_graph_hex_cell_radius ?? 2.35))
      );
      setKmsGraphHexLayerOpacity(
        Math.min(1, Math.max(0, appState.kms_graph_hex_layer_opacity ?? 0.22))
      );
      setKmsGraphHexStrokeWidth(
        Math.min(0.5, Math.max(0.02, appState.kms_graph_hex_stroke_width ?? 0.11))
      );
      setKmsGraphHexStrokeOpacity(
        Math.min(1, Math.max(0, appState.kms_graph_hex_stroke_opacity ?? 0.38))
      );

      const fetchIndexingStatus = async () => {
        try {
          const status = await getTaurpc().kms_get_indexing_status();
          setIndexingStatus(status);
        } catch (e) {
          console.error("Failed to fetch indexing status:", e);
        }
      };
      fetchIndexingStatus();

      setTheme(
        (typeof localStorage !== "undefined" &&
          localStorage.getItem("digicore-theme")) ||
        "light"
      );
      onConfigLoaded(appState);
    }
  }, [appState]);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const cfg: CopyToClipboardConfig =
          await getTaurpc().get_copy_to_clipboard_config();
        if (cancelled) return;
        setCopyEnabled(!!cfg.enabled);
        setCopyImageEnabled(!!cfg.image_capture_enabled);
        setCopyMinLogLength(cfg.min_log_length ?? 1);
        setCopyMaskCc(!!cfg.mask_cc);
        setCopyMaskSsn(!!cfg.mask_ssn);
        setCopyMaskEmail(!!cfg.mask_email);
        setCopyBlacklistProcesses(cfg.blacklist_processes || "");
        setCopyJsonOutputEnabled(cfg.json_output_enabled ?? true);
        setCopyJsonOutputDir(cfg.json_output_dir || "");
        setCopyImageStorageDir(cfg.image_storage_dir || "");
        setCopyOcrEnabled(!!cfg.ocr_enabled);
        setLoadedImageStorageDir(cfg.image_storage_dir || "");
        if (typeof cfg.max_history_entries === "number") {
          setClipMaxDepth(cfg.max_history_entries);
        }
      } catch {
        /* ignore load failure */
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const loadAutostart = async () => {
    try {
      const autostartPlugin = getAutostart();
      if (autostartPlugin) {
        const enabled = await autostartPlugin.isEnabled();
        setAutostart(enabled);
      }
    } catch {
      /* ignore */
    }
  };

  useEffect(() => {
    loadAutostart();
  }, []);

  useEffect(() => {
    let intervalId: number | undefined;

    const fetchStatus = async () => {
      try {
        const stats = await getTaurpc().kms_get_indexing_status();
        setIndexingStatus(stats);
      } catch (err) {
        console.error("Failed to fetch indexing status:", err);
      }
    };

    if (activeGroup === "semantic_search") {
      fetchStatus();
      intervalId = window.setInterval(fetchStatus, 5000);
    }

    return () => {
      if (intervalId) {
        clearInterval(intervalId);
      }
    };
  }, [activeGroup]);

  const parseIntOr = (raw: string, fallback: number): number => {
    const parsed = Number.parseInt(raw, 10);
    return Number.isFinite(parsed) ? parsed : fallback;
  };

  const clampInt = (value: number, min: number, max: number): number =>
    Math.min(max, Math.max(min, value));

  const monitorAnchorSafe = clampInt(
    parseIntOr(ghostFollowerMonitor, 0),
    0,
    2
  );

  const applyConfig = async (
    partial: Record<string, unknown>,
    groupName: string,
    options?: { onApplied?: () => void }
  ) => {
    try {
      await getTaurpc().update_config(
        partial as Parameters<ReturnType<typeof getTaurpc>["update_config"]>[0]
      );
      await getTaurpc().save_settings();
      const dto = await getTaurpc().get_app_state();
      onConfigLoaded(normalizeAppState(dto));
      setStatus("Settings saved.");
      setStatusError(false);
      showFeedback(groupName);
      options?.onApplied?.();
    } catch (e) {
      setStatus("Error: " + String(e));
      setStatusError(true);
    }
  };

  const applyCopyToClipboardConfig = async () => {
    const minLen = clampInt(copyMinLogLength, 1, 2000);
    const maxEntries = Math.max(0, Number.isFinite(clipMaxDepth) ? clipMaxDepth : 20);
    const imagePathChanged =
      copyImageStorageDir.trim() !== loadedImageStorageDir.trim();
    try {
      await getTaurpc().save_copy_to_clipboard_config({
        enabled: copyEnabled,
        image_capture_enabled: copyImageEnabled,
        min_log_length: minLen,
        mask_cc: copyMaskCc,
        mask_ssn: copyMaskSsn,
        mask_email: copyMaskEmail,
        blacklist_processes: copyBlacklistProcesses,
        max_history_entries: maxEntries,
        json_output_enabled: copyJsonOutputEnabled,
        json_output_dir: copyJsonOutputDir,
        image_storage_dir: copyImageStorageDir,
        ocr_enabled: copyOcrEnabled,
      });
      setStatus(
        imagePathChanged
          ? "Copy-to-Clipboard settings saved. Image assets migration completed."
          : "Copy-to-Clipboard settings saved."
      );
      setStatusError(false);
      setClipMaxDepth(maxEntries);
      setLoadedImageStorageDir(copyImageStorageDir);
      await applyConfig({ clip_history_max_depth: maxEntries }, "core");
      showFeedback("copy_to_clipboard");
    } catch (e) {
      setStatus("Copy-to-Clipboard save failed: " + String(e));
      setStatusError(true);
    }
  };

  const chooseDirectory = async (
    currentValue: string,
    setter: (value: string) => void
  ) => {
    const selected = await open({
      directory: true,
      multiple: false,
      defaultPath: currentValue || undefined,
      title: "Select folder",
    });
    if (typeof selected === "string" && selected.trim()) {
      setter(selected);
    }
  };

  const handleClearClipboardHistory = async () => {
    try {
      await getTaurpc().clear_clipboard_history();
      setStatus("Clipboard history cleared.");
      setStatusError(false);
    } catch (e) {
      setStatus("Clipboard history clear failed: " + String(e));
      setStatusError(true);
    }
  };

  const applyTheme = (pref: string) => {
    const resolved = resolveTheme(pref);
    applyThemeToDocument(resolved);
    emit("digicore-theme-changed", { theme: resolved }).catch(() => { });
  };

  useEffect(() => {
    applyTheme(theme);
    if (theme !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = () => applyTheme("system");
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, [theme]);

  const inputCls =
    "w-full max-w-[400px] p-1 bg-[var(--dc-bg)] text-[var(--dc-text)] border border-[var(--dc-border)] rounded";

  const sectionCls =
    "my-3 border border-[var(--dc-border)] rounded p-2";

  const getTargetSettingsGroups = (): string[] => {
    if (settingsTransferScope === "all") {
      return SETTINGS_GROUP_OPTIONS.map((g) => g.id);
    }
    return selectedSettingsGroups;
  };

  const toggleSettingsGroup = (groupId: string, checked: boolean) => {
    setSelectedSettingsGroups((prev) => {
      if (checked) {
        return prev.includes(groupId) ? prev : [...prev, groupId];
      }
      return prev.filter((g) => g !== groupId);
    });
  };

  const handleExportSettingsBundle = async () => {
    const groups = getTargetSettingsGroups();
    if (groups.length === 0) {
      setStatus("Validation: choose at least one settings group to export.");
      setStatusError(true);
      return;
    }
    const path = await save({
      title: "Export DigiCore Settings",
      defaultPath: "digicore_settings_bundle.json",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path) {
      setStatus("Export cancelled.");
      setStatusError(false);
      return;
    }
    try {
      const count = await getTaurpc().export_settings_bundle_to_file(
        path,
        groups,
        theme,
        autostart
      );
      setStatus(
        `Exported settings bundle with ${count} group${count === 1 ? "" : "s"
        } to ${String(path)} `
      );
      setStatusError(false);
    } catch (e) {
      setStatus("Export failed: " + String(e));
      setStatusError(true);
    }
  };

  const handleImportSettingsBundle = async () => {
    const pathSelection = await open({
      title: "Preview DigiCore Settings Bundle",
      multiple: false,
      directory: false,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    const path = Array.isArray(pathSelection) ? pathSelection[0] : pathSelection;
    if (!path) {
      setStatus("Import preview cancelled.");
      setStatusError(false);
      return;
    }
    try {
      const preview = await getTaurpc().preview_settings_bundle_from_file(path);
      setImportPreview(preview);
      setImportWarningsAcknowledged(false);
      setStatus(
        `Preview loaded: schema ${preview.schema_version}, groups ${preview.available_groups.length}, warnings ${preview.warnings.length}.`
      );
      setStatusError(!preview.valid);
    } catch (e) {
      setImportPreview(null);
      setImportWarningsAcknowledged(false);
      setStatus("Preview failed: " + String(e));
      setStatusError(true);
    }
  };

  const handleApplyImportSettingsBundle = async () => {
    const groups = getTargetSettingsGroups();
    if (groups.length === 0) {
      setStatus("Validation: choose at least one settings group to import.");
      setStatusError(true);
      return;
    }
    if (!importPreview?.path) {
      setStatus("Validation: preview a settings bundle file first.");
      setStatusError(true);
      return;
    }
    if (!importPreview.valid) {
      setStatus("Validation: cannot import from an invalid preview bundle.");
      setStatusError(true);
      return;
    }
    if (importPreview.warnings.length > 0 && !importWarningsAcknowledged) {
      setStatus("Validation: acknowledge preview warnings before import.");
      setStatusError(true);
      return;
    }
    try {
      const result = await getTaurpc().import_settings_bundle_from_file(
        importPreview.path,
        groups
      );
      if (result.theme) {
        setTheme(result.theme);
        localStorage.setItem("digicore-theme", result.theme);
        applyTheme(result.theme);
      }
      if (typeof result.autostart_enabled === "boolean") {
        try {
          const autostartPlugin = getAutostart();
          if (autostartPlugin) {
            if (result.autostart_enabled) await autostartPlugin.enable();
            else await autostartPlugin.disable();
            setAutostart(result.autostart_enabled);
          }
        } catch {
          /* ignore plugin apply failure */
        }
      }
      const dto = await getTaurpc().get_app_state();
      onConfigLoaded(normalizeAppState(dto));
      setStatus(
        `Import complete: applied ${result.applied_groups.length} group${result.applied_groups.length === 1 ? "" : "s"
        }, warnings ${result.warnings.length}.`
      );
      setStatusError(false);
      setImportPreview(null);
      setImportWarningsAcknowledged(false);
    } catch (e) {
      setStatus("Import failed: " + String(e));
      setStatusError(true);
    }
  };

  return (
    <div className="p-4 border border-[var(--dc-border)] rounded mt-2">
      <div className="mb-4 flex items-center justify-between gap-3">
        <h2 className="text-xl font-semibold">Configurations and Settings</h2>
        {reindexProgress && (
          <KmsReindexProgressBadge
            variant="toolbar"
            providerId={reindexProgress.providerId}
            providerIndex={reindexProgress.providerIndex}
            providerTotal={reindexProgress.providerTotal}
            elapsedMs={reindexProgress.elapsedMs}
            etaRemainingMs={reindexProgress.etaRemainingMs}
            indexedTotalSoFar={reindexProgress.indexedTotalSoFar}
          />
        )}
      </div>
      <p className={`text - sm mt - 2 ${statusError ? "text-[var(--dc-error)]" : "text-[var(--dc-text-muted)]"} `}>
        {status}
      </p>

      <div className="mb-6 mt-4 flex flex-wrap gap-2">
        {CONFIG_NAV_TABS.map((group) => (
          <button
            key={group.id}
            onClick={() => setActiveGroup(group.id)}
            className={`px - 3 py - 1.5 rounded border border - [var(--dc - border)]text - sm transition - colors ${activeGroup === group.id
              ? "bg-[var(--dc-accent)] text-white"
              : "bg-[var(--dc-bg)] text-[var(--dc-text)] hover:bg-[var(--dc-bg-alt)]"
              } `}
          >
            {group.label}
          </button>
        ))}
      </div>

      {activeGroup === "templates" && (
        <div className={sectionCls}>
          <h3 className="font-bold mb-2">Templates (F16-F20)</h3>
          <p className="text-sm text-[var(--dc-text-muted)] mt-1">
            Placeholders: {"{date}"}, {"{time}"}, {"{time:fmt}"}, {"{clipboard}"},{" "}
            {"{clip:1}"}-{"{clip:N}"}, {"{env:VAR}"}
          </p>
          <label className="block mt-2">Date format:</label>
          <input
            type="text"
            value={templateDate}
            onChange={(e) => setTemplateDate(e.target.value)}
            placeholder="%Y-%m-%d"
            className={inputCls}
          />
          <label className="block mt-2">Time format:</label>
          <input
            type="text"
            value={templateTime}
            onChange={(e) => setTemplateTime(e.target.value)}
            placeholder="%H:%M"
            className={inputCls}
          />
          <button
            onClick={() =>
              applyConfig(
                {
                  template_date_format: templateDate,
                  template_time_format: templateTime,
                },
                "templates"
              )
            }
            className="mt-2 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
          >
            Save Templates Settings
          </button>
          {lastSavedGroup === "templates" && (
            <span className="ml-2 text-emerald-500 font-medium animate-in fade-in duration-300">
              Saved!
            </span>
          )}
        </div>
      )}

      {activeGroup === "sync" && (
        <div className={sectionCls}>
          <h3 className="font-bold mb-2">Sync (WebDAV)</h3>
          <label className="block mt-2">WebDAV URL:</label>
          <input
            type="text"
            value={syncUrl}
            onChange={(e) => setSyncUrl(e.target.value)}
            placeholder="https://webdav.example.com/library.json"
            className={inputCls}
          />
          <button
            onClick={() => applyConfig({ sync_url: syncUrl }, "sync")}
            className="mt-2 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
          >
            Save Sync URL Settings
          </button>
          {lastSavedGroup === "sync" && (
            <span className="ml-2 text-emerald-500 font-medium animate-in fade-in duration-300">
              Saved!
            </span>
          )}
        </div>
      )}

      {activeGroup === "discovery" && (
        <div className={sectionCls}>
          <h3 className="font-bold mb-2">Discovery (F60-F69)</h3>
          <label className="block mt-2 flex items-center gap-2">
            <input
              type="checkbox"
              checked={discoveryEnabled}
              onChange={(e) => setDiscoveryEnabled(e.target.checked)}
            />
            Enable Discovery
          </label>
          <label className="block mt-2">Threshold (repeats):</label>
          <input
            type="number"
            value={discoveryThreshold}
            onChange={(e) =>
              setDiscoveryThreshold((prev) =>
                clampInt(parseIntOr(e.target.value, prev), 2, 10)
              )
            }
            min={2}
            max={10}
            className={inputCls}
          />
          <label className="block mt-2">Lookback (min):</label>
          <input
            type="number"
            value={discoveryLookback}
            onChange={(e) =>
              setDiscoveryLookback((prev) =>
                clampInt(parseIntOr(e.target.value, prev), 5, 240)
              )
            }
            min={5}
            max={240}
            className={inputCls}
          />
          <label className="block mt-2">Min phrase length:</label>
          <input
            type="number"
            value={discoveryMinLen}
            onChange={(e) =>
              setDiscoveryMinLen((prev) =>
                clampInt(parseIntOr(e.target.value, prev), 2, 20)
              )
            }
            min={2}
            max={20}
            className={inputCls}
          />
          <label className="block mt-2">Max phrase length:</label>
          <input
            type="number"
            value={discoveryMaxLen}
            onChange={(e) =>
              setDiscoveryMaxLen((prev) =>
                clampInt(parseIntOr(e.target.value, prev), 10, 100)
              )
            }
            min={10}
            max={100}
            className={inputCls}
          />
          <label className="block mt-2">Excluded apps (comma-separated):</label>
          <input
            type="text"
            value={discoveryExcludedApps}
            onChange={(e) => setDiscoveryExcludedApps(e.target.value)}
            className={inputCls}
          />
          <label className="block mt-2">Excluded window titles:</label>
          <input
            type="text"
            value={discoveryExcludedTitles}
            onChange={(e) => setDiscoveryExcludedTitles(e.target.value)}
            className={inputCls}
          />
          <button
            onClick={() =>
              applyConfig(
                {
                  discovery_enabled: discoveryEnabled,
                  discovery_threshold: discoveryThreshold,
                  discovery_lookback: discoveryLookback,
                  discovery_min_len: discoveryMinLen,
                  discovery_max_len: discoveryMaxLen,
                  discovery_excluded_apps: discoveryExcludedApps,
                  discovery_excluded_window_titles: discoveryExcludedTitles,
                },
                "discovery"
              )
            }
            className="mt-2 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
          >
            Save Discovery Settings
          </button>
          {lastSavedGroup === "discovery" && (
            <span className="ml-2 text-emerald-500 font-medium animate-in fade-in duration-300">
              Saved!
            </span>
          )}
        </div>
      )}

      {activeGroup === "ghost_suggestor" && (
        <div className={sectionCls}>
          <h3 className="font-bold mb-2">Ghost Suggestor (F43-F47)</h3>
          <label className="block mt-2 flex items-center gap-2">
            <input
              type="checkbox"
              checked={ghostSuggestorEnabled}
              onChange={(e) => setGhostSuggestorEnabled(e.target.checked)}
            />
            Enable Ghost Suggestor
          </label>
          <label className="block mt-2">Debounce (ms):</label>
          <input
            type="number"
            value={ghostSuggestorDebounce}
            onChange={(e) =>
              setGhostSuggestorDebounce((prev) =>
                clampInt(parseIntOr(e.target.value, prev), 20, 200)
              )
            }
            min={20}
            max={200}
            className={inputCls}
          />
          <label className="block mt-2">
            Display duration (sec, 0=no auto-hide):
          </label>
          <input
            type="number"
            value={ghostSuggestorDisplay}
            onChange={(e) =>
              setGhostSuggestorDisplay((prev) =>
                clampInt(parseIntOr(e.target.value, prev), 0, 120)
              )
            }
            min={0}
            max={120}
            className={inputCls}
          />
          <label className="block mt-2">Snooze duration (min, 1-120):</label>
          <input
            type="number"
            value={ghostSuggestorSnooze}
            onChange={(e) =>
              setGhostSuggestorSnooze((prev) =>
                clampInt(parseIntOr(e.target.value, prev), 1, 120)
              )
            }
            min={1}
            max={120}
            className={inputCls}
          />
          <label className="block mt-2">Offset X:</label>
          <input
            type="number"
            value={ghostSuggestorOffsetX}
            onChange={(e) =>
              setGhostSuggestorOffsetX((prev) =>
                parseIntOr(e.target.value, prev)
              )
            }
            className={inputCls}
          />
          <label className="block mt-2">Offset Y:</label>
          <input
            type="number"
            value={ghostSuggestorOffsetY}
            onChange={(e) =>
              setGhostSuggestorOffsetY((prev) =>
                parseIntOr(e.target.value, prev)
              )
            }
            className={inputCls}
          />
          <button
            onClick={() =>
              applyConfig(
                {
                  ghost_suggestor_enabled: ghostSuggestorEnabled,
                  ghost_suggestor_debounce_ms: ghostSuggestorDebounce,
                  ghost_suggestor_display_secs: ghostSuggestorDisplay,
                  ghost_suggestor_snooze_duration_mins: ghostSuggestorSnooze,
                  ghost_suggestor_offset_x: ghostSuggestorOffsetX,
                  ghost_suggestor_offset_y: ghostSuggestorOffsetY,
                },
                "ghost_suggestor"
              )
            }
            className="mt-2 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
          >
            Save Ghost Suggestor Settings
          </button>
          {lastSavedGroup === "ghost_suggestor" && (
            <span className="ml-2 text-emerald-500 font-medium animate-in fade-in duration-300">
              Saved!
            </span>
          )}
        </div>
      )}

      {activeGroup === "ghost_follower" && (
        <div className={sectionCls}>
          <h3 className="font-bold mb-2">Ghost Follower (F48-F59)</h3>
          <label className="block mt-2 flex items-center gap-2">
            <input
              type="checkbox"
              checked={ghostFollowerEnabled}
              onChange={(e) => setGhostFollowerEnabled(e.target.checked)}
            />
            Enable Ghost Follower
          </label>
          <label className="block mt-2 flex items-center gap-2">
            <input
              type="checkbox"
              checked={ghostFollowerHover}
              onChange={(e) => setGhostFollowerHover(e.target.checked)}
            />
            Hover preview content
          </label>
          <label className="block mt-2">Follower Mode:</label>
          <select
            value={ghostFollowerMode}
            onChange={(e) => setGhostFollowerMode(e.target.value)}
            className={inputCls}
          >
            <option value="EdgeAnchored">Edge Anchored</option>
            <option value="FloatingBubble">Floating Bubble</option>
          </select>

          <label className="block mt-2">Expansion Trigger:</label>
          <select
            value={ghostFollowerExpandTrigger}
            onChange={(e) => setGhostFollowerExpandTrigger(e.target.value)}
            className={inputCls}
          >
            <option value="Click">Click</option>
            <option value="Hover">Hover</option>
          </select>
          <label className="block mt-2">Collapse delay (s):</label>
          <input
            type="number"
            value={ghostFollowerCollapse}
            onChange={(e) =>
              setGhostFollowerCollapse((prev) =>
                clampInt(parseIntOr(e.target.value, prev), 0, 60)
              )
            }
            min={0}
            max={60}
            className={inputCls}
          />
          <p className="mt-1 text-xs text-[var(--dc-text-muted)]">
            NOTE: value "0" keeps open always, otherwise change duration to 1-n
            seconds before collapsing to pill.
          </p>
          <label className="block mt-2">Edge:</label>
          <select
            value={ghostFollowerEdge}
            onChange={(e) =>
              setGhostFollowerEdge(e.target.value as "right" | "left")
            }
            className={inputCls}
          >
            <option value="right">Right</option>
            <option value="left">Left</option>
          </select>
          <label className="block mt-2">Monitor:</label>
          <select
            value={ghostFollowerMonitor}
            onChange={(e) => setGhostFollowerMonitor(e.target.value)}
            className={inputCls}
          >
            <option value="0">Primary</option>
            <option value="1">Secondary</option>
            <option value="2">Current</option>
          </select>
          <label className="block mt-2">Transparency: {ghostFollowerOpacity}%</label>
          <input
            type="range"
            min={10}
            max={100}
            value={ghostFollowerOpacity}
            onChange={(e) => {
              const v = clampInt(parseIntOr(e.target.value, ghostFollowerOpacity), 10, 100);
              setGhostFollowerOpacity(v);
              getTaurpc().ghost_follower_set_opacity(v).catch(() => { });
            }}
            className="w-full mt-1"
          />
          <button
            onClick={() =>
              applyConfig(
                {
                  ghost_follower_enabled: ghostFollowerEnabled,
                  ghost_follower_hover_preview: ghostFollowerHover,
                  ghost_follower_collapse_delay_secs: ghostFollowerCollapse,
                  ghost_follower_edge_right: ghostFollowerEdge === "right",
                  ghost_follower_monitor_anchor: monitorAnchorSafe,
                  ghost_follower_opacity: ghostFollowerOpacity,
                },
                "ghost_follower"
              )
            }
            className="mt-2 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
          >
            Save Ghost Follower Settings
          </button>
          {lastSavedGroup === "ghost_follower" && (
            <span className="ml-2 text-emerald-500 font-medium animate-in fade-in duration-300">
              Saved!
            </span>
          )}
        </div>
      )}

      {activeGroup === "clipboard_history" && (
        <div className={sectionCls}>
          <h3 className="font-bold mb-2">Clipboard History (F38-F42)</h3>
          <p className="text-sm text-[var(--dc-text-muted)] mt-1">
            Clipboard entry depth is managed from the Copy-to-Clipboard section to keep
            one single source of truth.
          </p>
          <p className="text-sm mt-2">
            Current max depth: <strong>{clipMaxDepth}</strong>
          </p>
        </div>
      )}

      {activeGroup === "copy_to_clipboard" && (
        <div className={sectionCls}>
          <h3 className="font-bold mb-2">Copy-to-Clipboard</h3>
          <p className="text-sm text-[var(--dc-text-muted)] mt-1">
            Controls capture, filtering, masking, and JSON-only persistence behavior.
          </p>
          <label className="block mt-2 flex items-center gap-2">
            <input
              type="checkbox"
              checked={copyEnabled}
              onChange={(e) => setCopyEnabled(e.target.checked)}
            />
            Enable Copy-to-Clipboard (Text) Capture
          </label>
          <label className="block mt-2 flex items-center gap-2">
            <input
              type="checkbox"
              checked={copyImageEnabled}
              onChange={(e) => setCopyImageEnabled(e.target.checked)}
            />
            Enable Copy-to-Clipboard (Image) Capture
          </label>
          {copyImageEnabled && (
            <label className="block mt-2 ml-6 flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={copyOcrEnabled}
                onChange={(e) => setCopyOcrEnabled(e.target.checked)}
              />
              Enable Image OCR Text Capture
            </label>
          )}
          <label className="block mt-2 flex items-center gap-2">
            <input
              type="checkbox"
              checked={copyJsonOutputEnabled}
              onChange={(e) => setCopyJsonOutputEnabled(e.target.checked)}
            />
            Enable JSON Output
          </label>
          <label className="block mt-2">JSON Output Directory:</label>
          <div className="mt-1 flex gap-2">
            <input
              type="text"
              value={copyJsonOutputDir}
              onChange={(e) => setCopyJsonOutputDir(e.target.value)}
              placeholder="C:\\Users\\...\\DigiCore\\clipboard-json"
              className={`${inputCls} flex - 1`}
            />
            <button
              type="button"
              onClick={() => void chooseDirectory(copyJsonOutputDir, setCopyJsonOutputDir)}
              className="px-3 py-1.5 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
            >
              Browse
            </button>
          </div>
          <label className="block mt-2">Image Storage Directory:</label>
          <div className="mt-1 flex gap-2">
            <input
              type="text"
              value={copyImageStorageDir}
              onChange={(e) => setCopyImageStorageDir(e.target.value)}
              placeholder="C:\\Users\\...\\DigiCore\\clipboard-images"
              className={`${inputCls} flex - 1`}
            />
            <button
              type="button"
              onClick={() =>
                void chooseDirectory(copyImageStorageDir, setCopyImageStorageDir)
              }
              className="px-3 py-1.5 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
            >
              Browse
            </button>
          </div>
          <label className="block mt-2">Min logged content length (1-2000):</label>
          <input
            type="number"
            value={copyMinLogLength}
            onChange={(e) =>
              setCopyMinLogLength((prev) =>
                clampInt(parseIntOr(e.target.value, prev), 1, 2000)
              )
            }
            min={1}
            max={2000}
            className={inputCls}
          />
          <label className="block mt-2">Max history entries:</label>
          <input
            type="number"
            value={clipMaxDepth}
            onChange={(e) =>
              setClipMaxDepth((prev) =>
                Math.max(0, parseIntOr(e.target.value, prev))
              )
            }
            min={0}
            className={inputCls}
          />
          <p className="text-xs text-[var(--dc-text-muted)] mt-1">
            Note: 0 = Unlimited
          </p>
          <label className="block mt-2">Blacklist process names (comma-separated):</label>
          <div className="mt-1 flex gap-2">
            <input
              type="text"
              value={copyBlacklistProcesses}
              onChange={(e) => setCopyBlacklistProcesses(e.target.value)}
              placeholder="KeePassXC.exe, 1Password.exe"
              className={`${inputCls} flex-1`}
            />
            <button
              type="button"
              onClick={() => setCopyBlacklistProcesses("KeePass.exe, KeePassXC.exe, 1Password.exe, Bitwarden.exe, Dashlane.exe")}
              className="px-2 py-1 text-xs bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded hover:border-[var(--dc-accent)]"
              title="Load suggested password managers"
            >
              Defaults
            </button>
          </div>
          <div className="mt-2 flex flex-wrap gap-3">
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={copyMaskCc}
                onChange={(e) => setCopyMaskCc(e.target.checked)}
              />
              Mask credit cards
            </label>
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={copyMaskSsn}
                onChange={(e) => setCopyMaskSsn(e.target.checked)}
              />
              Mask SSN
            </label>
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={copyMaskEmail}
                onChange={(e) => setCopyMaskEmail(e.target.checked)}
              />
              Mask email
            </label>
          </div>
          <button
            onClick={applyCopyToClipboardConfig}
            className="mt-2 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
          >
            Save Copy-to-Clipboard Settings
          </button>
          {lastSavedGroup === "copy_to_clipboard" && (
            <span className="ml-2 text-emerald-500 font-medium animate-in fade-in duration-300">
              Saved!
            </span>
          )}
          <button
            onClick={handleClearClipboardHistory}
            className="mt-2 ml-2 px-3 py-1.5 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
          >
            Clear All
          </button>
        </div>
      )}

      {activeGroup === "appearance" && (
        <div className={sectionCls}>
          <Suspense fallback={<div className="py-8 text-[var(--dc-text-muted)]">Loading Appearance...</div>}>
            <AppearanceTab />
          </Suspense>
        </div>
      )}

      {activeGroup === "statistics" && (
        <div className={sectionCls}>
          <Suspense fallback={<div className="py-8 text-[var(--dc-text-muted)]">Loading Statistics...</div>}>
            <AnalyticsTab />
          </Suspense>
        </div>
      )}

      {activeGroup === "log" && (
        <div className={sectionCls}>
          <Suspense fallback={<div className="py-8 text-[var(--dc-text-muted)]">Loading Logs...</div>}>
            <LogTab />
          </Suspense>
        </div>
      )}

      {activeGroup === "text_expansion" && (
        <div className={sectionCls}>
          <h3 className="font-bold mb-2">Text Expansion</h3>
          <label className="block mt-2">Expansion Log Path (JSON):</label>
          <div className="mt-1 flex gap-2">
            <input
              type="text"
              value={expansionLogPath}
              onChange={(e) => setExpansionLogPath(e.target.value)}
              placeholder="Default: <AppData>/logs"
              className={`${inputCls} flex-1`}
            />
            <button
              type="button"
              onClick={() => void chooseDirectory(expansionLogPath, setExpansionLogPath)}
              className="px-3 py-1.5 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
            >
              Browse
            </button>
          </div>
          <p className="text-xs text-[var(--dc-text-muted)] mt-1">
            Specify a custom directory for text expansion diagnostic logs. Leave empty for default.
          </p>
          <button
            onClick={() => applyConfig({ expansion_log_path: expansionLogPath }, "text_expansion")}
            className="mt-3 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
          >
            Save Expansion Settings
          </button>
          {lastSavedGroup === "text_expansion" && (
            <span className="ml-2 text-emerald-500 font-medium animate-in fade-in duration-300">
              Saved!
            </span>
          )}
        </div>
      )}

      {activeGroup === "core" && (
        <div className={sectionCls}>
          <h3 className="font-bold mb-2">Core</h3>
          <label className="block mt-2 flex items-center gap-2">
            <input
              type="checkbox"
              checked={expansionPaused}
              onChange={(e) => setExpansionPaused(e.target.checked)}
            />
            Pause expansion (F7)
          </label>
          <label className="block mt-2 flex items-center gap-2">
            <input
              type="checkbox"
              checked={autostart}
              onChange={async (e) => {
                try {
                  const autostartPlugin = getAutostart();
                  if (autostartPlugin) {
                    if (e.target.checked) await autostartPlugin.enable();
                    else await autostartPlugin.disable();
                    setAutostart(e.target.checked);
                  }
                } catch {
                  /* ignore */
                }
              }}
            />
            Start with Windows
          </label>
          <label className="block mt-2">
            Theme:{" "}
            <select
              value={theme}
              onChange={(e) => {
                const v = e.target.value;
                setTheme(v);
                localStorage.setItem("digicore-theme", v);
                applyTheme(v);
              }}
              className={inputCls}
            >
              <option value="light">Light</option>
              <option value="dark">Dark</option>
              <option value="system">System</option>
            </select>
          </label>
          <button
            onClick={async () => {
              try {
                const autostartPlugin = getAutostart();
                if (autostartPlugin) {
                  if (autostart) await autostartPlugin.enable();
                  else await autostartPlugin.disable();
                }
              } catch {
                /* ignore */
              }
              await applyConfig(
                {
                  expansion_paused: expansionPaused,
                  template_date_format: templateDate,
                  template_time_format: templateTime,
                  sync_url: syncUrl,
                  discovery_enabled: discoveryEnabled,
                  discovery_threshold: discoveryThreshold,
                  discovery_lookback: discoveryLookback,
                  discovery_min_len: discoveryMinLen,
                  discovery_max_len: discoveryMaxLen,
                  discovery_excluded_apps: discoveryExcludedApps,
                  discovery_excluded_window_titles: discoveryExcludedTitles,
                  ghost_suggestor_enabled: ghostSuggestorEnabled,
                  ghost_suggestor_debounce_ms: ghostSuggestorDebounce,
                  ghost_suggestor_display_secs: ghostSuggestorDisplay,
                  ghost_suggestor_snooze_duration_mins: ghostSuggestorSnooze,
                  ghost_suggestor_offset_x: ghostSuggestorOffsetX,
                  ghost_suggestor_offset_y: ghostSuggestorOffsetY,
                  ghost_follower_enabled: ghostFollowerEnabled,
                  ghost_follower_hover_preview: ghostFollowerHover,
                  ghost_follower_collapse_delay_secs: ghostFollowerCollapse,
                  ghost_follower_edge_right: ghostFollowerEdge === "right",
                  ghost_follower_monitor_anchor: monitorAnchorSafe,
                  ghost_follower_opacity: ghostFollowerOpacity,
                  clip_history_max_depth: clipMaxDepth,

                  corpus_enabled: corpusEnabled,
                  corpus_output_dir: corpusOutputDir,
                  corpus_snapshot_dir: corpusSnapshotDir,
                  corpus_shortcut_modifiers: corpusShortcutModifiers,
                  corpus_shortcut_key: corpusShortcutKey,

                  extraction_row_overlap_tolerance: extractionRowOverlapTolerance,
                  extraction_cluster_threshold_factor: extractionClusterThresholdFactor,
                  extraction_zone_proximity: extractionZoneProximity,
                  extraction_cross_zone_gap_factor: extractionCrossZoneGapFactor,
                  extraction_same_zone_gap_factor: extractionSameZoneGapFactor,
                  extraction_significant_gap_gate: extractionSignificantGapGate,
                  extraction_char_width_factor: extractionCharWidthFactor,
                  extraction_bridged_threshold: extractionBridgedThreshold,
                  extraction_word_spacing_factor: extractionWordSpacingFactor,
                  extraction_footer_triggers: extractionFooterTriggers,
                  extraction_table_min_contiguous_rows: extractionTableMinContiguousRows,
                  extraction_table_min_avg_segments: extractionTableMinAvgSegments,

                  extraction_adaptive_plaintext_cluster_factor: extractionAdaptivePlaintextClusterFactor,
                  extraction_adaptive_plaintext_gap_gate: extractionAdaptivePlaintextGapGate,
                  extraction_adaptive_table_cluster_factor: extractionAdaptiveTableClusterFactor,
                  extraction_adaptive_table_gap_gate: extractionAdaptiveTableGapGate,
                  extraction_adaptive_column_cluster_factor: extractionAdaptiveColumnClusterFactor,
                  extraction_adaptive_column_gap_gate: extractionAdaptiveColumnGapGate,
                  extraction_adaptive_plaintext_cross_factor: extractionAdaptivePlaintextCrossFactor,
                  extraction_adaptive_table_cross_factor: extractionAdaptiveTableCrossFactor,
                  extraction_adaptive_column_cross_factor: extractionAdaptiveColumnCrossFactor,

                  extraction_refinement_entropy_threshold: extractionRefinementEntropyThreshold,
                  extraction_refinement_cluster_threshold_modifier: extractionRefinementClusterThresholdModifier,
                  extraction_refinement_cross_zone_gap_modifier: extractionRefinementCrossZoneGapModifier,

                  extraction_classifier_gutter_weight: extractionClassifierGutterWeight,
                  extraction_classifier_density_weight: extractionClassifierDensityWeight,
                  extraction_classifier_multicolumn_density_max: extractionClassifierMulticolumnDensityMax,
                  extraction_classifier_table_density_min: extractionClassifierTableDensityMin,
                  extraction_classifier_table_entropy_min: extractionClassifierTableEntropyMin,

                  extraction_columns_min_contiguous_rows: extractionColumnsMinContiguousRows,
                  extraction_columns_gutter_gap_factor: extractionColumnsGutterGapFactor,
                  extraction_columns_gutter_void_tolerance: extractionColumnsGutterVoidTolerance,
                  extraction_columns_edge_margin_tolerance: extractionColumnsEdgeMarginTolerance,

                  extraction_headers_max_width_ratio: extractionHeadersMaxWidthRatio,
                  extraction_headers_centered_tolerance: extractionHeadersCenteredTolerance,
                  extraction_headers_h1_size_multiplier: extractionHeadersH1SizeMultiplier,
                  extraction_headers_h2_size_multiplier: extractionHeadersH2SizeMultiplier,
                  extraction_headers_h3_size_multiplier: extractionHeadersH3SizeMultiplier,

                  extraction_scoring_jitter_penalty_weight: extractionScoringJitterPenaltyWeight,
                  extraction_scoring_size_penalty_weight: extractionScoringSizePenaltyWeight,
                  extraction_scoring_low_confidence_threshold: extractionScoringLowConfidenceThreshold,
                },
                "all"
              );
            }}
            className="mt-2 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
          >
            Save All Settings
          </button>
          {lastSavedGroup === "all" && (
            <span className="ml-2 text-emerald-500 font-medium animate-in fade-in duration-300">
              Saved!
            </span>
          )}
        </div>
      )}

      {activeGroup === "semantic_search" && (
        <div className={sectionCls}>
          <h3 className="font-bold mb-2">Semantic Search & Indexing</h3>
          <p className="text-sm text-[var(--dc-text-muted)] mb-4">
            Manage AI-powered search indexing for your knowledge base.
            Reindexing updates embeddings for better search matches.
          </p>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            {indexingStatus.map((status) => (
              <div key={status.category} className="p-3 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded-lg flex flex-col">
                <div className="flex items-center justify-between mb-2">
                  <h4 className="text-sm font-semibold capitalize">
                    {status.category === "notes" ? "Notes & Documents" :
                      status.category === "snippets" ? "Snippets Library" :
                        "Clipboard History"}
                  </h4>
                  {status.failed_count > 0 && (
                    <span className="text-[10px] bg-red-500/10 text-red-500 px-1.5 py-0.5 rounded border border-red-500/20 font-bold uppercase">
                      Action Required
                    </span>
                  )}
                </div>

                <p className="text-xs text-[var(--dc-text-muted)] mb-3">
                  {status.category === "notes" ? "Local markdown files in your vault." :
                    status.category === "snippets" ? "Text expansion triggers and contents." :
                      "Recent text and images for semantic recall."}
                </p>

                <div className="grid grid-cols-3 gap-2 mb-4 bg-[var(--dc-bg)] p-2 rounded border border-[var(--dc-border)]">
                  <div className="text-center">
                    <div className="text-[10px] uppercase text-[var(--dc-text-muted)]">Indexed</div>
                    <div className="text-xs font-mono font-bold text-emerald-500">{status.indexed_count}</div>
                  </div>
                  <div className="text-center border-x border-[var(--dc-border)]">
                    <div className="text-[10px] uppercase text-[var(--dc-text-muted)]">Failed</div>
                    <div className="text-xs font-mono font-bold text-red-500">{status.failed_count}</div>
                  </div>
                  <div className="text-center">
                    <div className="text-[10px] uppercase text-[var(--dc-text-muted)]">Total</div>
                    <div className="text-xs font-mono font-bold">{status.total_count}</div>
                  </div>
                </div>

                <div className="mt-auto flex flex-wrap gap-2">
                  <button
                    onClick={async () => {
                      setIsIndexing(status.category);
                      try {
                        await getTaurpc().kms_reindex_type(status.category);
                        const newStatus = await getTaurpc().kms_get_indexing_status();
                        setIndexingStatus(newStatus);
                      } finally {
                        setIsIndexing(null);
                      }
                    }}
                    disabled={isIndexing !== null}
                    className="px-2 py-1 text-xs bg-[var(--dc-accent)] text-white rounded disabled:opacity-50 flex-1 whitespace-nowrap"
                  >
                    {isIndexing === status.category ? "Indexing..." : `Full Reindex`}
                  </button>

                  <button
                    onClick={async () => {
                      setIsIndexing(`${status.category} _retry`);
                      try {
                        await getTaurpc().kms_retry_failed(status.category);
                        const newStatus = await getTaurpc().kms_get_indexing_status();
                        setIndexingStatus(newStatus);
                      } finally {
                        setIsIndexing(null);
                      }
                    }}
                    disabled={isIndexing !== null || status.failed_count === 0}
                    className="px-2 py-1 text-xs bg-[var(--dc-bg)] border border-[var(--dc-border)] hover:border-[var(--dc-accent)] rounded disabled:opacity-50 flex-1 whitespace-nowrap"
                  >
                    Retry Failed
                  </button>

                  {status.failed_count > 0 && (
                    <button
                      onClick={async () => {
                        if (expandedFailures === status.category) {
                          setExpandedFailures(null);
                        } else {
                          const details = await getTaurpc().kms_get_indexing_details(status.category);
                          setFailureDetails(details);
                          setExpandedFailures(status.category);
                        }
                      }}
                      className="text-[10px] text-[var(--dc-accent)] hover:underline w-full text-center mt-1"
                    >
                      {expandedFailures === status.category ? "Hide Details" : "View Failure Details"}
                    </button>
                  )}
                </div>

                {expandedFailures === status.category && (
                  <div className="mt-3 p-2 bg-[var(--dc-bg)] rounded border border-red-500/20 max-h-40 overflow-y-auto">
                    <div className="text-[10px] font-bold text-red-500 mb-2 uppercase">Recent Failures</div>
                    {failureDetails.length === 0 ? (
                      <div className="text-[10px] text-[var(--dc-text-muted)] italic">No detailed records found.</div>
                    ) : (
                      <div className="space-y-2">
                        {failureDetails.map((f, idx) => (
                          <div key={idx} className="border-b border-[var(--dc-border)] last:border-0 pb-1 mb-1">
                            <div className="flex items-center justify-between">
                              <span className="text-[10px] font-mono truncate max-w-[150px]" title={f.entity_id}>{f.entity_id}</span>
                              <button
                                onClick={async () => {
                                  await getTaurpc().kms_retry_item(status.category, f.entity_id);
                                  const details = await getTaurpc().kms_get_indexing_details(status.category);
                                  setFailureDetails(details);
                                  const newStatus = await getTaurpc().kms_get_indexing_status();
                                  setIndexingStatus(newStatus);
                                }}
                                className="text-[9px] bg-[var(--dc-accent)] text-white px-1 rounded"
                              >
                                Retry
                              </button>
                            </div>
                            <div className="text-[9px] text-red-400 italic mt-0.5 leading-tight">{f.error || "Unknown error"}</div>
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                )}
              </div>
            ))}

            <div className="p-3 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded-lg opacity-60">
              <div className="flex items-center gap-2 mb-2">
                <h4 className="text-sm font-semibold">Media & Attachments</h4>
                <span className="text-[10px] bg-[var(--dc-border)] px-1 rounded text-[var(--dc-text-muted)] uppercase">Future</span>
              </div>
              <p className="text-xs text-[var(--dc-text-muted)] mb-3">
                Videos, Audio, and PDFs (COMING SOON).
              </p>
              <button disabled className="px-2 py-1 text-xs bg-[var(--dc-bg)] border border-[var(--dc-border)] text-[var(--dc-text-muted)] rounded cursor-not-allowed">
                Planned
              </button>
            </div>
          </div>

          <div className="mt-6 pt-4 border-t border-[var(--dc-border)] flex items-center justify-between">
            <p className="text-xs text-[var(--dc-text-muted)]">
              Full reindex may take several minutes depending on data volume.
            </p>
            <div className="flex items-center gap-3">
              {lastSavedGroup === "semantic_search" && (
                <span className="text-emerald-500 text-sm font-medium animate-in fade-in">
                  Reindex triggered!
                </span>
              )}
              <button
                onClick={async () => {
                  setIsIndexing("all");
                  try {
                    await getTaurpc().kms_reindex_all();
                    showFeedback("semantic_search");
                  } finally {
                    setIsIndexing(null);
                  }
                }}
                disabled={isIndexing !== null}
                className="px-4 py-2 bg-[var(--dc-accent)] text-white rounded font-semibold shadow-sm hover:opacity-90 transition-opacity disabled:opacity-50 flex items-center gap-2"
              >
                {isIndexing === "all" ? "Indexing Everything..." : "Global Reindex All"}
              </button>
            </div>
          </div>
        </div>
      )}

      {activeGroup === "kms_graph" && (
        <div className={sectionCls}>
          <h3 className="font-bold mb-2">Knowledge Graph</h3>
          <p className="text-sm text-[var(--dc-text-muted)] mb-4">
            Tune semantic clustering (k-means on note embeddings) and AI insight beams shown in KMS graph views.
            Reload or refresh the graph to apply changes.
          </p>
          <p className="text-xs text-[var(--dc-text-muted)] mb-4 border border-[var(--dc-border)] rounded-lg p-3 bg-[var(--dc-bg)]/50">
            <span className="font-semibold text-[var(--dc-text)]">Scale:</span> Set{" "}
            <em>Max notes for semantics</em> to skip k-means and beams above that vault size (0 = never skip).{" "}
            <em>Large-vault warning</em> shows a notice in graph views when note count reaches this threshold (0 = off).{" "}
            <em>Beam pair budget</em> limits cosine comparisons for AI beams (0 = unlimited; lower for faster loads).{" "}
            In <em>paged</em> graph mode, semantics run on the current page only (cluster colors are not comparable across pages).{" "}
            AI beams use <strong>cosine similarity</strong> on stored embeddings (robust if vector lengths differ).
          </p>
          <div
            className="mb-6 p-4 border border-[var(--dc-border)] rounded-lg bg-[var(--dc-bg)]/30 space-y-3"
            title="Large vaults: load the graph in path-sorted pages instead of all nodes at once. Session view mode (full vs paged) is stored in the app when you switch in the graph view."
          >
            <h4
              className="text-xs font-bold text-[var(--dc-text)] uppercase tracking-wide"
              title="Applies to all vaults. Per-vault graph overrides can still tune build params in Vault Settings."
            >
              Paged graph (global)
            </h4>
            <label
              className="flex items-center gap-2 text-sm"
              title="When enabled, opening the graph uses paged mode if indexed note count is at or above the threshold. Turn off to always open in full graph unless you choose Paged view in the graph UI."
            >
              <input
                type="checkbox"
                className="rounded border-[var(--dc-border)]"
                checked={kmsGraphAutoPagingEnabled}
                onChange={(e) => setKmsGraphAutoPagingEnabled(e.target.checked)}
              />
              Auto-use paged graph when indexed note count reaches threshold (disable to always default to full graph)
            </label>
            <label
              className="flex flex-col gap-1 text-xs max-w-xs"
              title="Minimum number of indexed notes in the active vault before auto-paging applies. Below this, the graph opens in full mode (unless you switched to paged in-session)."
            >
              <span className="font-semibold text-[var(--dc-text)]">Auto-paging note threshold</span>
              <input
                type="number"
                min={1}
                max={1000000}
                className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)]"
                value={kmsGraphAutoPagingNoteThreshold}
                onChange={(e) =>
                  setKmsGraphAutoPagingNoteThreshold(
                    clampInt(parseIntOr(e.target.value, 1200), 1, 1_000_000)
                  )
                }
              />
            </label>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <label className="flex flex-col gap-1 text-xs">
              <span className="font-semibold text-[var(--dc-text)]">K-means max K (cap on sqrt heuristic)</span>
              <input
                type="number"
                min={2}
                max={200}
                className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)]"
                value={kmsGraphKMeansMaxK}
                onChange={(e) =>
                  setKmsGraphKMeansMaxK(clampInt(parseIntOr(e.target.value, 10), 2, 200))
                }
              />
            </label>
            <label className="flex flex-col gap-1 text-xs">
              <span className="font-semibold text-[var(--dc-text)]">K-means iterations</span>
              <input
                type="number"
                min={1}
                max={500}
                className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)]"
                value={kmsGraphKMeansIterations}
                onChange={(e) =>
                  setKmsGraphKMeansIterations(clampInt(parseIntOr(e.target.value, 10), 1, 500))
                }
              />
            </label>
            <label className="flex flex-col gap-1 text-xs">
              <span className="font-semibold text-[var(--dc-text)]">AI beam max nodes (pairwise scan cap)</span>
              <input
                type="number"
                min={2}
                max={50000}
                className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)]"
                value={kmsGraphAiBeamMaxNodes}
                onChange={(e) =>
                  setKmsGraphAiBeamMaxNodes(clampInt(parseIntOr(e.target.value, 10), 2, 50000))
                }
              />
            </label>
            <label className="flex flex-col gap-1 text-xs">
              <span className="font-semibold text-[var(--dc-text)]">AI beam similarity threshold (0-1)</span>
              <input
                type="number"
                min={0}
                max={1}
                step={0.01}
                className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)]"
                value={kmsGraphAiBeamSimilarityThreshold}
                onChange={(e) => {
                  const v = Number.parseFloat(e.target.value);
                  setKmsGraphAiBeamSimilarityThreshold(
                    Number.isFinite(v) ? Math.min(1, Math.max(0, v)) : 0.9
                  );
                }}
              />
            </label>
            <label className="flex flex-col gap-1 text-xs md:col-span-2">
              <span className="font-semibold text-[var(--dc-text)]">Max AI beam edges</span>
              <input
                type="number"
                min={0}
                max={500}
                className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)] max-w-xs"
                value={kmsGraphAiBeamMaxEdges}
                onChange={(e) =>
                  setKmsGraphAiBeamMaxEdges(clampInt(parseIntOr(e.target.value, 10), 0, 500))
                }
              />
            </label>
            <label className="flex flex-col gap-1 text-xs">
              <span className="font-semibold text-[var(--dc-text)]">Max notes for semantics (0 = no cap)</span>
              <input
                type="number"
                min={0}
                max={1000000}
                className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)]"
                value={kmsGraphSemanticMaxNotes}
                onChange={(e) =>
                  setKmsGraphSemanticMaxNotes(clampInt(parseIntOr(e.target.value, 10), 0, 1000000))
                }
              />
            </label>
            <label className="flex flex-col gap-1 text-xs">
              <span className="font-semibold text-[var(--dc-text)]">Large-vault warning at note count (0 = off)</span>
              <input
                type="number"
                min={0}
                max={1000000}
                className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)]"
                value={kmsGraphWarnNoteThreshold}
                onChange={(e) =>
                  setKmsGraphWarnNoteThreshold(clampInt(parseIntOr(e.target.value, 10), 0, 1000000))
                }
              />
            </label>
            <label className="flex flex-col gap-1 text-xs md:col-span-2">
              <span className="font-semibold text-[var(--dc-text)]">AI beam max pair checks (0 = unlimited)</span>
              <input
                type="number"
                min={0}
                max={50000000}
                className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)] max-w-xs"
                value={kmsGraphBeamMaxPairChecks}
                onChange={(e) =>
                  setKmsGraphBeamMaxPairChecks(clampInt(parseIntOr(e.target.value, 10), 0, 50_000_000))
                }
              />
            </label>
          </div>
          <div
            className="mb-6 p-4 border border-[var(--dc-border)] rounded-lg bg-[var(--dc-bg)]/30 space-y-3"
            title="Embedding cosine k-nearest-neighbor edges (semantic_knn) are drawn in the graph. PageRank still uses wiki links only."
          >
            <h4 className="text-xs font-bold text-[var(--dc-text)] uppercase tracking-wide">
              Semantic kNN edges
            </h4>
            <p className="text-[11px] text-[var(--dc-text-muted)] leading-relaxed">
              Adds undirected similarity edges from note embeddings. Tune per-note neighbors and a global edge cap for large vaults; 0 pair checks = unlimited inner scoring work (still capped by max edges).
            </p>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <label className="flex flex-col gap-1 text-xs">
                <span className="font-semibold text-[var(--dc-text)]">kNN neighbors per note (1-30)</span>
                <input
                  type="number"
                  min={1}
                  max={30}
                  className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)]"
                  value={kmsGraphSemanticKnnPerNote}
                  onChange={(e) =>
                    setKmsGraphSemanticKnnPerNote(clampInt(parseIntOr(e.target.value, 5), 1, 30))
                  }
                />
              </label>
              <label className="flex flex-col gap-1 text-xs">
                <span className="font-semibold text-[var(--dc-text)]">Min cosine similarity (0.5 - 0.999)</span>
                <input
                  type="number"
                  min={0.5}
                  max={0.999}
                  step={0.01}
                  className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)]"
                  value={kmsGraphSemanticKnnMinSimilarity}
                  onChange={(e) => {
                    const v = Number.parseFloat(e.target.value);
                    setKmsGraphSemanticKnnMinSimilarity(
                      Number.isFinite(v) ? Math.min(0.999, Math.max(0.5, v)) : 0.82
                    );
                  }}
                />
              </label>
              <label className="flex flex-col gap-1 text-xs md:col-span-2">
                <span className="font-semibold text-[var(--dc-text)]">Max kNN edges in one graph build (0-500k)</span>
                <input
                  type="number"
                  min={0}
                  max={500000}
                  className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)] max-w-xs"
                  value={kmsGraphSemanticKnnMaxEdges}
                  onChange={(e) =>
                    setKmsGraphSemanticKnnMaxEdges(clampInt(parseIntOr(e.target.value, 10), 0, 500_000))
                  }
                />
              </label>
              <label className="flex flex-col gap-1 text-xs md:col-span-2">
                <span className="font-semibold text-[var(--dc-text)]">kNN max pair checks (0 = unlimited)</span>
                <input
                  type="number"
                  min={0}
                  max={50000000}
                  className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)] max-w-xs"
                  value={kmsGraphSemanticKnnMaxPairChecks}
                  onChange={(e) =>
                    setKmsGraphSemanticKnnMaxPairChecks(
                      clampInt(parseIntOr(e.target.value, 10), 0, 50_000_000)
                    )
                  }
                />
              </label>
            </div>
          </div>
          <div
            className="mb-6 p-4 border border-[var(--dc-border)] rounded-lg bg-[var(--dc-bg)]/30 space-y-3"
            title="Undirected PageRank on wiki links for node link_centrality (0-1). Higher iterations converge closer but cost more CPU on large graphs."
          >
            <h4 className="text-xs font-bold text-[var(--dc-text)] uppercase tracking-wide">
              PageRank (link centrality)
            </h4>
            <p className="text-[11px] text-[var(--dc-text-muted)] leading-relaxed md:col-span-2">
              For very large vaults, use paged graph mode together with scope{" "}
              <strong className="font-medium text-[var(--dc-text)]">Auto</strong> or{" "}
              <strong className="font-medium text-[var(--dc-text)]">Page subgraph</strong> so centrality is computed on
              the visible page (recommended up to on the order of 12k indexed notes for interactive builds).{" "}
              <strong className="font-medium text-[var(--dc-text)]">Full vault</strong> runs PageRank on every note
              before pagination and is best reserved for smaller vaults or when you need global ranks on a slice.
            </p>
            <label className="flex flex-col gap-1 text-xs md:col-span-2">
              <span className="font-semibold text-[var(--dc-text)]">Global graph PageRank scope</span>
              <select
                className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)] max-w-md"
                value={kmsGraphPagerankScope}
                onChange={(e) => setKmsGraphPagerankScope(e.target.value)}
              >
                <option value="auto">Auto (paged request = page subgraph; full request = full vault)</option>
                <option value="full_vault">Full vault (then slice if paged)</option>
                <option value="page_subgraph">Page subgraph only when paged</option>
                <option value="off">Off (no PageRank)</option>
              </select>
            </label>
            <label className="flex items-start gap-2 text-xs md:col-span-2">
              <input
                type="checkbox"
                className="rounded border-[var(--dc-border)] mt-0.5 shrink-0"
                checked={kmsGraphBackgroundWikiPagerankEnabled}
                onChange={(e) => setKmsGraphBackgroundWikiPagerankEnabled(e.target.checked)}
              />
              <span>
                <span className="font-semibold text-[var(--dc-text)]">
                  Background materialized wiki PageRank after vault sync
                </span>
                <span className="block text-[var(--dc-text-muted)] mt-1 leading-relaxed">
                  When enabled, a non-blocking job refreshes stored vault-wide scores after bulk sync so the graph can reuse them without stalling the UI. Uncheck to disable this background work (in-request PageRank still follows the scope above).
                </span>
              </span>
            </label>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <label className="flex flex-col gap-1 text-xs">
                <span className="font-semibold text-[var(--dc-text)]">Global graph iterations (min 4)</span>
                <input
                  type="number"
                  min={4}
                  max={500}
                  className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)]"
                  value={kmsGraphPagerankIterations}
                  onChange={(e) =>
                    setKmsGraphPagerankIterations(clampInt(parseIntOr(e.target.value, 48), 4, 500))
                  }
                />
              </label>
              <label className="flex flex-col gap-1 text-xs">
                <span className="font-semibold text-[var(--dc-text)]">Local graph iterations (min 4)</span>
                <input
                  type="number"
                  min={4}
                  max={500}
                  className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)]"
                  value={kmsGraphPagerankLocalIterations}
                  onChange={(e) =>
                    setKmsGraphPagerankLocalIterations(clampInt(parseIntOr(e.target.value, 32), 4, 500))
                  }
                />
              </label>
              <label className="flex flex-col gap-1 text-xs md:col-span-2">
                <span className="font-semibold text-[var(--dc-text)]">Damping factor (0.5 - 0.99, typical 0.85)</span>
                <input
                  type="number"
                  min={0.5}
                  max={0.99}
                  step={0.01}
                  className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)] max-w-xs"
                  value={kmsGraphPagerankDamping}
                  onChange={(e) => {
                    const v = Number.parseFloat(e.target.value);
                    setKmsGraphPagerankDamping(
                      Number.isFinite(v) ? Math.min(0.99, Math.max(0.5, v)) : 0.85
                    );
                  }}
                />
              </label>
            </div>
          </div>
          <div
            className="mb-6 p-4 border border-[var(--dc-border)] rounded-lg bg-[var(--dc-bg)]/30 space-y-3"
            title="three-spritetext rasterizes 3D node labels to a canvas. Higher scales use more GPU memory but look sharper when zooming. Compare against a future CSS2DRenderer label path."
          >
            <h4 className="text-xs font-bold text-[var(--dc-text)] uppercase tracking-wide">
              3D label texture (sprite canvas)
            </h4>
            <p className="text-[11px] text-[var(--dc-text-muted)] leading-relaxed">
              Effective scale per frame is{" "}
              <code className="text-[10px] bg-[var(--dc-bg)] px-1 rounded">
                min(MAX_DPR_SCALE, max(MIN_RES_SCALE, devicePixelRatio))
              </code>
              . Base canvas font is 90px; world label size still follows node textHeight.
            </p>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <label className="flex flex-col gap-1 text-xs">
                <span className="font-semibold text-[var(--dc-text)]">MAX_DPR_SCALE (1 - 8, default 2.5)</span>
                <input
                  type="number"
                  min={1}
                  max={8}
                  step={0.05}
                  className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)]"
                  value={kmsGraphSpriteLabelMaxDprScale}
                  onChange={(e) => {
                    const v = Number.parseFloat(e.target.value);
                    setKmsGraphSpriteLabelMaxDprScale(
                      Number.isFinite(v) ? Math.min(8, Math.max(1, v)) : 2.5
                    );
                  }}
                />
              </label>
              <label className="flex flex-col gap-1 text-xs">
                <span className="font-semibold text-[var(--dc-text)]">MIN_RES_SCALE (1 - 4, default 1.25)</span>
                <input
                  type="number"
                  min={1}
                  max={4}
                  step={0.05}
                  className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)]"
                  value={kmsGraphSpriteLabelMinResScale}
                  onChange={(e) => {
                    const v = Number.parseFloat(e.target.value);
                    setKmsGraphSpriteLabelMinResScale(
                      Number.isFinite(v) ? Math.min(4, Math.max(1, v)) : 1.25
                    );
                  }}
                />
              </label>
            </div>
          </div>
          <div
            className="mb-6 p-4 border border-[var(--dc-border)] rounded-lg bg-[var(--dc-bg)]/30 space-y-3"
            title="Large 2D graphs run the initial d3-force layout off the UI thread so pan/zoom stays responsive."
          >
            <h4 className="text-xs font-bold text-[var(--dc-text)] uppercase tracking-wide">
              2D graph layout (WebWorker)
            </h4>
            <p className="text-[11px] text-[var(--dc-text-muted)] leading-relaxed">
              When the visible node count is at or above this threshold, the first force layout pass runs in a
              background worker. Use 0 to always compute on the main thread (legacy behavior). Max ticks is capped
              against a scaled minimum from node count on the graph. Lower alpha min runs longer before stopping.
            </p>
            <label className="flex flex-col gap-1 text-xs max-w-md">
              <span className="font-semibold text-[var(--dc-text)]">
                WebWorker layout threshold (0 - 500000 nodes, 0 = off)
              </span>
              <input
                type="number"
                min={0}
                max={500000}
                step={50}
                className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)]"
                value={kmsGraphWebworkerLayoutThreshold}
                onChange={(e) => {
                  const v = Number.parseInt(e.target.value, 10);
                  setKmsGraphWebworkerLayoutThreshold(
                    Number.isFinite(v) ? Math.min(500_000, Math.max(0, v)) : 800
                  );
                }}
              />
            </label>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-3 max-w-xl">
              <label className="flex flex-col gap-1 text-xs">
                <span className="font-semibold text-[var(--dc-text)]">Max ticks (20 - 10000)</span>
                <input
                  type="number"
                  min={20}
                  max={10000}
                  step={10}
                  className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)]"
                  value={kmsGraphWebworkerLayoutMaxTicks}
                  onChange={(e) => {
                    const v = Number.parseInt(e.target.value, 10);
                    setKmsGraphWebworkerLayoutMaxTicks(
                      Number.isFinite(v) ? Math.min(10_000, Math.max(20, v)) : 450
                    );
                  }}
                />
              </label>
              <label className="flex flex-col gap-1 text-xs">
                <span className="font-semibold text-[var(--dc-text)]">Alpha min (0.0005 - 0.5)</span>
                <input
                  type="number"
                  min={0.0005}
                  max={0.5}
                  step={0.005}
                  className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)]"
                  value={kmsGraphWebworkerLayoutAlphaMin}
                  onChange={(e) => {
                    const v = Number.parseFloat(e.target.value);
                    setKmsGraphWebworkerLayoutAlphaMin(
                      Number.isFinite(v) ? Math.min(0.5, Math.max(0.0005, v)) : 0.02
                    );
                  }}
                />
              </label>
            </div>
          </div>
          <div
            className="mb-6 p-4 border border-[var(--dc-border)] rounded-lg bg-[var(--dc-bg)]/30 space-y-3"
            title="Affects 2D and 3D graph constellation backdrop and 3D UnrealBloomPass post-processing."
          >
            <h4 className="text-xs font-bold text-[var(--dc-text)] uppercase tracking-wide">
              Bloom (3D) and hex backdrop
            </h4>
            <label className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                className="rounded border-[var(--dc-border)]"
                checked={kmsGraphBloomEnabled}
                onChange={(e) => setKmsGraphBloomEnabled(e.target.checked)}
              />
              Enable bloom on 3D graphs
            </label>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
              <label className="flex flex-col gap-1 text-xs">
                <span className="font-semibold text-[var(--dc-text)]">Bloom strength (0-2.5)</span>
                <input
                  type="number"
                  min={0}
                  max={2.5}
                  step={0.02}
                  className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)]"
                  value={kmsGraphBloomStrength}
                  onChange={(e) => {
                    const v = Number.parseFloat(e.target.value);
                    setKmsGraphBloomStrength(Number.isFinite(v) ? Math.min(2.5, Math.max(0, v)) : 0.48);
                  }}
                />
              </label>
              <label className="flex flex-col gap-1 text-xs">
                <span className="font-semibold text-[var(--dc-text)]">Bloom radius (0-1.5)</span>
                <input
                  type="number"
                  min={0}
                  max={1.5}
                  step={0.02}
                  className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)]"
                  value={kmsGraphBloomRadius}
                  onChange={(e) => {
                    const v = Number.parseFloat(e.target.value);
                    setKmsGraphBloomRadius(Number.isFinite(v) ? Math.min(1.5, Math.max(0, v)) : 0.4);
                  }}
                />
              </label>
              <label className="flex flex-col gap-1 text-xs md:col-span-2">
                <span className="font-semibold text-[var(--dc-text)]">Bloom threshold (0-1)</span>
                <input
                  type="number"
                  min={0}
                  max={1}
                  step={0.01}
                  className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)] max-w-xs"
                  value={kmsGraphBloomThreshold}
                  onChange={(e) => {
                    const v = Number.parseFloat(e.target.value);
                    setKmsGraphBloomThreshold(Number.isFinite(v) ? Math.min(1, Math.max(0, v)) : 0.22);
                  }}
                />
              </label>
            </div>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-3 pt-2 border-t border-[var(--dc-border)]/60">
              <label className="flex flex-col gap-1 text-xs">
                <span className="font-semibold text-[var(--dc-text)]">Hex cell radius (0.5-8)</span>
                <input
                  type="number"
                  min={0.5}
                  max={8}
                  step={0.05}
                  className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)]"
                  value={kmsGraphHexCellRadius}
                  onChange={(e) => {
                    const v = Number.parseFloat(e.target.value);
                    setKmsGraphHexCellRadius(Number.isFinite(v) ? Math.min(8, Math.max(0.5, v)) : 2.35);
                  }}
                />
              </label>
              <label className="flex flex-col gap-1 text-xs">
                <span className="font-semibold text-[var(--dc-text)]">Hex layer opacity (0-1)</span>
                <input
                  type="number"
                  min={0}
                  max={1}
                  step={0.02}
                  className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)]"
                  value={kmsGraphHexLayerOpacity}
                  onChange={(e) => {
                    const v = Number.parseFloat(e.target.value);
                    setKmsGraphHexLayerOpacity(Number.isFinite(v) ? Math.min(1, Math.max(0, v)) : 0.22);
                  }}
                />
              </label>
              <label className="flex flex-col gap-1 text-xs">
                <span className="font-semibold text-[var(--dc-text)]">Hex stroke width (0.02-0.5)</span>
                <input
                  type="number"
                  min={0.02}
                  max={0.5}
                  step={0.01}
                  className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)]"
                  value={kmsGraphHexStrokeWidth}
                  onChange={(e) => {
                    const v = Number.parseFloat(e.target.value);
                    setKmsGraphHexStrokeWidth(Number.isFinite(v) ? Math.min(0.5, Math.max(0.02, v)) : 0.11);
                  }}
                />
              </label>
              <label className="flex flex-col gap-1 text-xs">
                <span className="font-semibold text-[var(--dc-text)]">Hex stroke opacity (0-1)</span>
                <input
                  type="number"
                  min={0}
                  max={1}
                  step={0.02}
                  className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)]"
                  value={kmsGraphHexStrokeOpacity}
                  onChange={(e) => {
                    const v = Number.parseFloat(e.target.value);
                    setKmsGraphHexStrokeOpacity(Number.isFinite(v) ? Math.min(1, Math.max(0, v)) : 0.38);
                  }}
                />
              </label>
            </div>
          </div>
          <div className="mt-6 pt-4 border-t border-[var(--dc-border)] space-y-3">
            <h4 className="text-sm font-semibold text-[var(--dc-text)]">Temporal graph (server)</h4>
            <p className="text-xs text-[var(--dc-text-muted)]">
              Option A filters nodes by note <code className="text-[10px]">last_modified</code> after
              full-vault PageRank. Option B adds <code className="text-[10px]">edge_recency</code> on wiki
              edges for styling. RPC can pass one-off UTC bounds via{" "}
              <code className="text-[10px]">kms_get_graph</code>.
            </p>
            <label className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                className="rounded border-[var(--dc-border)]"
                checked={kmsGraphTemporalWindowEnabled}
                onChange={(e) => setKmsGraphTemporalWindowEnabled(e.target.checked)}
              />
              Time window filter (Option A) — use default days below when enabled
            </label>
            <label className="flex flex-col gap-1 text-xs max-w-xs">
              <span className="font-semibold">Default window length (days, 0 = off)</span>
              <input
                type="number"
                min={0}
                max={3650}
                className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)]"
                value={kmsGraphTemporalDefaultDays}
                onChange={(e) =>
                  setKmsGraphTemporalDefaultDays(clampInt(parseIntOr(e.target.value, 0), 0, 3650))
                }
              />
            </label>
            <label className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                className="rounded border-[var(--dc-border)]"
                checked={kmsGraphTemporalIncludeNoMtime}
                onChange={(e) => setKmsGraphTemporalIncludeNoMtime(e.target.checked)}
              />
              Include notes without parseable modified time when Option A is active
            </label>
            <label className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                className="rounded border-[var(--dc-border)]"
                checked={kmsGraphTemporalEdgeRecencyEnabled}
                onChange={(e) => setKmsGraphTemporalEdgeRecencyEnabled(e.target.checked)}
              />
              Edge recency channel (Option B)
            </label>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
              <label className="flex flex-col gap-1 text-xs">
                <span className="font-semibold">Recency strength (0-1)</span>
                <input
                  type="number"
                  min={0}
                  max={1}
                  step={0.05}
                  className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)]"
                  value={kmsGraphTemporalEdgeRecencyStrength}
                  onChange={(e) => {
                    const v = Number.parseFloat(e.target.value);
                    setKmsGraphTemporalEdgeRecencyStrength(
                      Number.isFinite(v) ? Math.min(1, Math.max(0, v)) : 1.0
                    );
                  }}
                />
              </label>
              <label className="flex flex-col gap-1 text-xs">
                <span className="font-semibold">Half-life (days)</span>
                <input
                  type="number"
                  min={0.1}
                  max={3650}
                  step={0.5}
                  className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)]"
                  value={kmsGraphTemporalEdgeRecencyHalfLifeDays}
                  onChange={(e) => {
                    const v = Number.parseFloat(e.target.value);
                    setKmsGraphTemporalEdgeRecencyHalfLifeDays(
                      Number.isFinite(v) ? Math.max(0.1, v) : 30.0
                    );
                  }}
                />
              </label>
            </div>
          </div>
          <div className="mt-4 flex flex-col gap-3">
            <label className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                className="rounded border-[var(--dc-border)]"
                checked={kmsGraphEnableSemanticClustering}
                onChange={(e) => setKmsGraphEnableSemanticClustering(e.target.checked)}
              />
              Enable semantic clustering (k-means and cluster labels)
            </label>
            <label className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                className="rounded border-[var(--dc-border)]"
                checked={kmsGraphEnableLeidenCommunities}
                onChange={(e) => setKmsGraphEnableLeidenCommunities(e.target.checked)}
              />
              Enable Leiden communities (experimental; default on for smoke testing until kNN+Leiden ships)
            </label>
            <label className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                className="rounded border-[var(--dc-border)]"
                checked={kmsGraphEnableAiBeams}
                onChange={(e) => setKmsGraphEnableAiBeams(e.target.checked)}
                disabled={!kmsGraphEnableSemanticClustering}
              />
              Enable AI insight beams (high similarity across clusters)
            </label>
            <label className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                className="rounded border-[var(--dc-border)]"
                checked={kmsGraphEnableSemanticKnnEdges}
                onChange={(e) => setKmsGraphEnableSemanticKnnEdges(e.target.checked)}
              />
              Enable semantic kNN edges (embedding similarity in the graph)
            </label>
          </div>
          <div className="mt-6 flex items-center gap-3">
            <button
              type="button"
              className="px-4 py-2 bg-[var(--dc-accent)] text-white rounded font-semibold text-sm shadow-sm hover:opacity-90"
              onClick={() =>
                applyConfig(
                  {
                    kms_graph_k_means_max_k: clampInt(kmsGraphKMeansMaxK, 2, 200),
                    kms_graph_k_means_iterations: clampInt(kmsGraphKMeansIterations, 1, 500),
                    kms_graph_ai_beam_max_nodes: clampInt(kmsGraphAiBeamMaxNodes, 2, 50000),
                    kms_graph_ai_beam_similarity_threshold: Math.min(
                      1,
                      Math.max(0, kmsGraphAiBeamSimilarityThreshold)
                    ),
                    kms_graph_ai_beam_max_edges: clampInt(kmsGraphAiBeamMaxEdges, 0, 500),
                    kms_graph_enable_ai_beams: kmsGraphEnableAiBeams,
                    kms_graph_enable_semantic_clustering: kmsGraphEnableSemanticClustering,
                    kms_graph_enable_leiden_communities: kmsGraphEnableLeidenCommunities,
                    kms_graph_semantic_max_notes: clampInt(kmsGraphSemanticMaxNotes, 0, 1000000),
                    kms_graph_warn_note_threshold: clampInt(kmsGraphWarnNoteThreshold, 0, 1000000),
                    kms_graph_beam_max_pair_checks: clampInt(kmsGraphBeamMaxPairChecks, 0, 50_000_000),
                    kms_graph_enable_semantic_knn_edges: kmsGraphEnableSemanticKnnEdges,
                    kms_graph_semantic_knn_per_note: clampInt(kmsGraphSemanticKnnPerNote, 1, 30),
                    kms_graph_semantic_knn_min_similarity: Math.min(
                      0.999,
                      Math.max(0.5, kmsGraphSemanticKnnMinSimilarity)
                    ),
                    kms_graph_semantic_knn_max_edges: clampInt(kmsGraphSemanticKnnMaxEdges, 0, 500_000),
                    kms_graph_semantic_knn_max_pair_checks: clampInt(
                      kmsGraphSemanticKnnMaxPairChecks,
                      0,
                      50_000_000
                    ),
                    kms_graph_pagerank_iterations: clampInt(kmsGraphPagerankIterations, 4, 500),
                    kms_graph_pagerank_local_iterations: clampInt(
                      kmsGraphPagerankLocalIterations,
                      4,
                      500
                    ),
                    kms_graph_pagerank_damping: Math.min(0.99, Math.max(0.5, kmsGraphPagerankDamping)),
                    kms_graph_pagerank_scope: kmsGraphPagerankScope.trim() || "auto",
                    kms_graph_background_wiki_pagerank_enabled: kmsGraphBackgroundWikiPagerankEnabled,
                    kms_graph_sprite_label_max_dpr_scale: Math.min(
                      8,
                      Math.max(1, kmsGraphSpriteLabelMaxDprScale)
                    ),
                    kms_graph_sprite_label_min_res_scale: Math.min(
                      4,
                      Math.max(1, kmsGraphSpriteLabelMinResScale)
                    ),
                    kms_graph_webworker_layout_threshold: clampInt(
                      kmsGraphWebworkerLayoutThreshold,
                      0,
                      500_000
                    ),
                    kms_graph_webworker_layout_max_ticks: clampInt(
                      kmsGraphWebworkerLayoutMaxTicks,
                      20,
                      10_000
                    ),
                    kms_graph_webworker_layout_alpha_min: Math.min(
                      0.5,
                      Math.max(0.0005, kmsGraphWebworkerLayoutAlphaMin)
                    ),
                    kms_graph_auto_paging_enabled: kmsGraphAutoPagingEnabled,
                    kms_graph_auto_paging_note_threshold: clampInt(
                      kmsGraphAutoPagingNoteThreshold,
                      1,
                      1_000_000
                    ),
                    kms_graph_bloom_enabled: kmsGraphBloomEnabled,
                    kms_graph_bloom_strength: Math.min(2.5, Math.max(0, kmsGraphBloomStrength)),
                    kms_graph_bloom_radius: Math.min(1.5, Math.max(0, kmsGraphBloomRadius)),
                    kms_graph_bloom_threshold: Math.min(1, Math.max(0, kmsGraphBloomThreshold)),
                    kms_graph_hex_cell_radius: Math.min(8, Math.max(0.5, kmsGraphHexCellRadius)),
                    kms_graph_hex_layer_opacity: Math.min(1, Math.max(0, kmsGraphHexLayerOpacity)),
                    kms_graph_hex_stroke_width: Math.min(0.5, Math.max(0.02, kmsGraphHexStrokeWidth)),
                    kms_graph_hex_stroke_opacity: Math.min(1, Math.max(0, kmsGraphHexStrokeOpacity)),
                  },
                  "kms_graph",
                  { onApplied: notifyKmsGraphVisualPrefsChanged }
                )
              }
            >
              Save Knowledge Graph Settings
            </button>
            {lastSavedGroup === "kms_graph" && (
              <span className="text-emerald-500 text-sm font-medium animate-in fade-in">Saved!</span>
            )}
          </div>
        </div>
      )}

      {activeGroup === "kms_search_embeddings" && (
        <div className={sectionCls}>
          <h3 className="font-bold mb-2">KMS Search and embeddings</h3>
          <p className="text-sm text-[var(--dc-text-muted)] mt-1 mb-4">
            Hybrid and semantic search use the same embedding model as indexed notes. Minimum similarity drops weak
            vector hits before rank fusion (0 = disabled). Note embeddings run through the O3 pipeline (normalize,
            generate, store). Each indexed note stores a policy fingerprint (normalized model id, chunk settings, vector
            dimension). Changing the model id or chunk policy starts a background re-embed (D6) for mismatched notes; leave
            model id empty to use the default ({DEFAULT_KMS_TEXT_EMBEDDING_MODEL_ID}).
          </p>
          <p className="text-xs text-[var(--dc-text-muted)] mb-4 border border-[var(--dc-border)] rounded p-2 bg-[var(--dc-bg-alt)]">
            <span className="font-semibold text-[var(--dc-text)]">Embedding diagnostic log file: </span>
            {kmsEmbedDiagnosticLogPath ? (
              <code className="text-[10px] break-all align-top block mt-1">{kmsEmbedDiagnosticLogPath}</code>
            ) : (
              <span className="text-[var(--dc-text-muted)]">(path unavailable)</span>
            )}
            <span className="block mt-1">
              WARN/ERROR from KMS embedding (D6 re-embed, fastembed, sqlite upsert) append here. Optional per-note DEBUG
              lines: set env <code className="text-[10px]">KMS_EMBED_LOG_FILE_DEBUG=1</code> before launch. Console:{" "}
              <code className="text-[10px]">RUST_LOG=info,kms_embed=debug</code> or{" "}
              <code className="text-[10px]">.\scripts\dev-tauri-kms-debug.ps1</code>.
            </span>
          </p>
          {kmsEmbedPolicyDiag != null && kmsEmbedPolicyDiag.expected_policy_signature !== "" && (
            <p className="text-xs text-[var(--dc-text-muted)] mb-4 border border-[var(--dc-border)] rounded p-2 bg-[var(--dc-bg-alt)]">
              Vault disk scan: {kmsEmbedPolicyDiag.vault_all_files_on_disk} file(s) total,{" "}
              {kmsEmbedPolicyDiag.vault_markdown_files_on_disk} markdown. SQLite index rows:{" "}
              {kmsEmbedPolicyDiag.total_notes_in_index} (indexed {kmsEmbedPolicyDiag.indexed_note_count}, pending{" "}
              {kmsEmbedPolicyDiag.pending_note_count}, sync failed {kmsEmbedPolicyDiag.failed_sync_note_count}). Embeddings
              matching current policy: {kmsEmbedPolicyDiag.embedding_aligned_note_count}. Stale / missing / wrong
              fingerprint:{" "}
              <span
                className={
                  kmsEmbedPolicyDiag.stale_embedding_note_count > 0
                    ? "text-amber-600 font-semibold"
                    : ""
                }
              >
                {kmsEmbedPolicyDiag.stale_embedding_note_count}
              </span>
              . Expected fingerprint:{" "}
              <code className="text-[10px] break-all align-top">
                {kmsEmbedPolicyDiag.expected_policy_signature || "(n/a)"}
              </code>
            </p>
          )}
          <label className="flex flex-col gap-1 text-xs max-w-md">
            <span className="font-semibold">Minimum cosine similarity for vector search (0-1)</span>
            <input
              type="number"
              min={0}
              max={1}
              step={0.01}
              className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)]"
              value={kmsSearchMinSimilarity}
              onChange={(e) => {
                const v = Number.parseFloat(e.target.value);
                setKmsSearchMinSimilarity(Number.isFinite(v) ? Math.min(1, Math.max(0, v)) : 0);
              }}
            />
          </label>
          <label className="mt-3 flex items-center gap-2 text-xs max-w-md">
            <input
              type="checkbox"
              checked={kmsSearchIncludeEmbeddingDiagnostics}
              onChange={(e) => setKmsSearchIncludeEmbeddingDiagnostics(e.target.checked)}
            />
            <span>Include embedding diagnostics in semantic search results (timing and model id per row)</span>
          </label>
          <label className="flex flex-col gap-1 text-xs max-w-md mt-3">
            <span className="font-semibold">Default KMS Explorer search mode</span>
            <select
              className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)] max-w-xs"
              value={kmsSearchDefaultMode}
              onChange={(e) => {
                const v = e.target.value;
                if (v === "Hybrid" || v === "Semantic" || v === "Keyword") {
                  setKmsSearchDefaultMode(v);
                }
              }}
            >
              <option value="Hybrid">Hybrid</option>
              <option value="Semantic">Semantic</option>
              <option value="Keyword">Keyword</option>
            </select>
          </label>
          <label className="flex flex-col gap-1 text-xs max-w-md mt-3">
            <span className="font-semibold">Default search result limit (1-200)</span>
            <input
              type="number"
              min={1}
              max={200}
              className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)] max-w-xs"
              value={kmsSearchDefaultLimit}
              onChange={(e) => {
                const v = Number.parseInt(e.target.value, 10);
                setKmsSearchDefaultLimit(Number.isFinite(v) ? Math.min(200, Math.max(1, v)) : 20);
              }}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs max-w-md mt-4">
            <span className="font-semibold">Note embedding model id</span>
            <input
              type="text"
              className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)]"
              value={kmsEmbeddingModelId}
              onChange={(e) => setKmsEmbeddingModelId(e.target.value)}
              placeholder={DEFAULT_KMS_TEXT_EMBEDDING_MODEL_ID}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs max-w-md mt-3">
            <span className="font-semibold">Re-embed batch size (notes per tick, 1-500)</span>
            <input
              type="number"
              min={1}
              max={500}
              className="border border-[var(--dc-border)] rounded px-2 py-1 bg-[var(--dc-bg)] text-[var(--dc-text)]"
              value={kmsEmbeddingBatchNotes}
              onChange={(e) => {
                const v = Number.parseInt(e.target.value, 10);
                setKmsEmbeddingBatchNotes(Number.isFinite(v) ? Math.min(500, Math.max(1, v)) : 8);
              }}
            />
          </label>
          <div className="mt-4 flex flex-wrap items-center gap-3">
            <button
              type="button"
              className="px-4 py-2 bg-[var(--dc-accent)] text-white rounded font-semibold text-sm shadow-sm hover:opacity-90"
              onClick={() =>
                applyConfig(
                  {
                    kms_search_min_similarity: Math.min(1, Math.max(0, kmsSearchMinSimilarity)),
                    kms_search_include_embedding_diagnostics: kmsSearchIncludeEmbeddingDiagnostics,
                    kms_search_default_mode: kmsSearchDefaultMode,
                    kms_search_default_limit: Math.min(
                      200,
                      Math.max(1, kmsSearchDefaultLimit)
                    ),
                    kms_embedding_model_id: kmsEmbeddingModelId.trim(),
                    kms_embedding_batch_notes_per_tick: Math.min(
                      500,
                      Math.max(1, kmsEmbeddingBatchNotes)
                    ),
                    kms_embedding_chunk_enabled: kmsEmbeddingChunkEnabled,
                    kms_embedding_chunk_max_chars: Math.min(
                      8192,
                      Math.max(256, kmsEmbeddingChunkMaxChars)
                    ),
                    kms_embedding_chunk_overlap_chars: Math.min(
                      4096,
                      Math.max(0, kmsEmbeddingChunkOverlapChars)
                    ),
                  },
                  "kms_search_embeddings"
                )
              }
            >
              Save search and embedding settings
            </button>
            <button
              type="button"
              className="px-4 py-2 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded font-semibold text-sm hover:bg-[var(--dc-border)]"
              data-testid="kms-reembed-vault-btn"
              onClick={async () => {
                setKmsEmbedArchivedReportOpen(false);
                setKmsEmbedModalOpen(true);
                try {
                  await getTaurpc().kms_request_note_embedding_migration();
                  setStatus("Background note re-embed queued.");
                  setStatusError(false);
                } catch (e) {
                  setStatus("Error: " + String(e));
                  setStatusError(true);
                }
              }}
            >
              Re-embed vault now
            </button>
            {kmsEmbedHealthStored ? (
              <button
                type="button"
                className="px-3 py-2 text-sm font-medium text-[var(--dc-accent)] underline-offset-2 hover:underline"
                onClick={() => {
                  setKmsEmbedArchivedReportOpen(true);
                  setKmsEmbedModalOpen(true);
                }}
              >
                View last embedding health report
              </button>
            ) : null}
            {lastSavedGroup === "kms_search_embeddings" && (
              <span className="text-emerald-500 text-sm font-medium animate-in fade-in">Saved!</span>
            )}
          </div>
          <p className="text-[10px] text-[var(--dc-text-muted)] mt-3 max-w-3xl">
            Full embedding health and live D6 batch progress stay in the <span className="font-semibold">persistent panel</span>{" "}
            at the bottom of this Config page (all subtabs). New notes and edits embed in the background via vault sync and
            the file watcher; that incremental work is not counted in the D6 batch totals below.
          </p>
        </div>
      )}

      {activeGroup === "script_runtime" && (
        <div className={sectionCls}>
          <h3 className="font-bold mb-2">Script Runtime</h3>
          <p className="text-sm text-[var(--dc-text-muted)] mt-1">
            Global script execution settings. For individual engine configurations, see the &apos;Scripting Engine Library&apos; tab.
          </p>
        </div>
      )}

      {activeGroup === "corpus" && (
        <div className={sectionCls}>
          <h3 className="font-bold mb-2">Corpus Generation</h3>
          <p className="text-sm text-[var(--dc-text-muted)] mt-1">
            Save samples automatically into dataset folders for heuristic tuning.
          </p>
          <label className="block mt-2 flex items-center gap-2">
            <input
              type="checkbox"
              checked={corpusEnabled}
              onChange={(e) => setCorpusEnabled(e.target.checked)}
            />
            Enable automatic corpus generation
          </label>
          <label className="block mt-2">Corpus output directory:</label>
          <div className="flex gap-2">
            <input
              type="text"
              value={corpusOutputDir}
              onChange={(e) => setCorpusOutputDir(e.target.value)}
              placeholder="C:\Users\...\corpus"
              className={inputCls}
            />
            <button
              type="button"
              onClick={() => chooseDirectory(corpusOutputDir, setCorpusOutputDir)}
              className="px-2 py-1 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded text-sm min-w-[32px] hover:bg-[var(--dc-border)]"
              title="Browse folder..."
            >
              ...
            </button>
          </div>
          <label className="block mt-2">Corpus snapshot directory:</label>
          <div className="flex gap-2">
            <input
              type="text"
              value={corpusSnapshotDir}
              onChange={(e) => setCorpusSnapshotDir(e.target.value)}
              placeholder="C:\Users\...\snapshots"
              className={inputCls}
            />
            <button
              type="button"
              onClick={() => chooseDirectory(corpusSnapshotDir, setCorpusSnapshotDir)}
              className="px-2 py-1 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded text-sm min-w-[32px] hover:bg-[var(--dc-border)]"
              title="Browse folder..."
            >
              ...
            </button>
          </div>

          <button
            onClick={() =>
              applyConfig(
                {
                  corpus_enabled: corpusEnabled,
                  corpus_output_dir: corpusOutputDir,
                  corpus_snapshot_dir: corpusSnapshotDir,
                  corpus_shortcut_modifiers: corpusShortcutModifiers,
                  corpus_shortcut_key: corpusShortcutKey,
                },
                "corpus"
              )
            }
            className="mt-3 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
          >
            Save Corpus Settings
          </button>
          {lastSavedGroup === "corpus" && (
            <span className="ml-2 text-emerald-500 font-medium animate-in fade-in duration-300">
              Saved!
            </span>
          )}
        </div>
      )}

      {activeGroup === "extraction" && (
        <div className={sectionCls}>
          <h3 className="font-bold mb-2">Extraction Engine (Advanced)</h3>
          <p className="text-sm text-[var(--dc-text-muted)] mt-1">
            Adjust heuristics for Layout-Aware OCR, Grid Alignment, and semantic tables.
          </p>

          <div className="space-y-4">
            <div>
              <h4 className="font-semibold mb-2 text-sm text-[var(--dc-accent)] border-b border-[var(--dc-border)] pb-1">General OCR Geometry</h4>
              <div className="space-y-2">
                <div className="flex flex-col">
                  <label className="text-xs text-[var(--dc-text-muted)]">Row overlap tolerance (0.0-1.0):</label>
                  <input type="number" step="0.05" min="0" max="1" value={extractionRowOverlapTolerance} onChange={(e) => setExtractionRowOverlapTolerance(parseFloat(e.target.value))} className={inputCls} />
                </div>
                <div className="flex flex-col">
                  <label className="text-xs text-[var(--dc-text-muted)]">Cluster threshold factor:</label>
                  <input type="number" step="0.05" value={extractionClusterThresholdFactor} onChange={(e) => setExtractionClusterThresholdFactor(parseFloat(e.target.value))} className={inputCls} />
                </div>
                <div className="flex flex-col">
                  <label className="text-xs text-[var(--dc-text-muted)]">Zone proximity:</label>
                  <input type="number" step="0.1" value={extractionZoneProximity} onChange={(e) => setExtractionZoneProximity(parseFloat(e.target.value))} className={inputCls} />
                </div>
                <div className="flex flex-col">
                  <label className="text-xs text-[var(--dc-text-muted)]">Cross-zone gap factor:</label>
                  <input type="number" step="0.05" value={extractionCrossZoneGapFactor} onChange={(e) => setExtractionCrossZoneGapFactor(parseFloat(e.target.value))} className={inputCls} />
                </div>
                <div className="flex flex-col">
                  <label className="text-xs text-[var(--dc-text-muted)]">Same-zone gap factor:</label>
                  <input type="number" step="0.05" value={extractionSameZoneGapFactor} onChange={(e) => setExtractionSameZoneGapFactor(parseFloat(e.target.value))} className={inputCls} />
                </div>
                <div className="flex flex-col">
                  <label className="text-xs text-[var(--dc-text-muted)]">Significant gap gate (e.g. 0.8):</label>
                  <input type="number" step="0.1" value={extractionSignificantGapGate} onChange={(e) => setExtractionSignificantGapGate(parseFloat(e.target.value))} className={inputCls} />
                </div>
              </div>
            </div>

            <div>
              <h4 className="font-semibold mb-2 text-sm text-[var(--dc-accent)] border-b border-[var(--dc-border)] pb-1">Layout Heuristics</h4>
              <div className="space-y-2">
                <div className="flex flex-col">
                  <label className="text-xs text-[var(--dc-text-muted)]">Row lookback (count):</label>
                  <input type="number" min="1" value={extractionLayoutRowLookback} onChange={(e) => setExtractionLayoutRowLookback(parseIntOr(e.target.value, 5))} className={inputCls} />
                </div>
                <div className="flex flex-col">
                  <label className="text-xs text-[var(--dc-text-muted)]">Table break threshold (h mult):</label>
                  <input type="number" step="0.5" value={extractionLayoutTableBreakThreshold} onChange={(e) => setExtractionLayoutTableBreakThreshold(parseFloat(e.target.value))} className={inputCls} />
                </div>
                <div className="flex flex-col">
                  <label className="text-xs text-[var(--dc-text-muted)]">Paragraph break threshold (h mult):</label>
                  <input type="number" step="0.5" value={extractionLayoutParagraphBreakThreshold} onChange={(e) => setExtractionLayoutParagraphBreakThreshold(parseFloat(e.target.value))} className={inputCls} />
                </div>
                <div className="flex flex-col">
                  <label className="text-xs text-[var(--dc-text-muted)]">Max space clamp (chars):</label>
                  <input type="number" min="1" value={extractionLayoutMaxSpaceClamp} onChange={(e) => setExtractionLayoutMaxSpaceClamp(parseIntOr(e.target.value, 6))} className={inputCls} />
                </div>
              </div>
            </div>
          </div>

          <div className="space-y-4">
            <div>
              <h4 className="font-semibold mb-2 text-sm text-[var(--dc-accent)] border-b border-[var(--dc-border)] pb-1">Table Reconstruction</h4>
              <div className="space-y-2">
                <div className="flex flex-col">
                  <label className="text-xs text-[var(--dc-text-muted)]">Min contiguous rows:</label>
                  <input type="number" min="1" value={extractionTableMinContiguousRows} onChange={(e) => setExtractionTableMinContiguousRows(parseIntOr(e.target.value, 3))} className={inputCls} />
                </div>
                <div className="flex flex-col">
                  <label className="text-xs text-[var(--dc-text-muted)]">Min average segments:</label>
                  <input type="number" step="0.1" value={extractionTableMinAvgSegments} onChange={(e) => setExtractionTableMinAvgSegments(parseFloat(e.target.value))} className={inputCls} />
                </div>
                <div className="flex flex-col">
                  <label className="text-xs text-[var(--dc-text-muted)]">Column jitter tolerance (px):</label>
                  <input type="number" step="1" value={extractionTablesColumnJitterTolerance} onChange={(e) => setExtractionTablesColumnJitterTolerance(parseFloat(e.target.value))} className={inputCls} />
                </div>
                <div className="flex flex-col">
                  <label className="text-xs text-[var(--dc-text-muted)]">Merge Y gap max (px):</label>
                  <input type="number" step="10" value={extractionTablesMergeYGapMax} onChange={(e) => setExtractionTablesMergeYGapMax(parseFloat(e.target.value))} className={inputCls} />
                </div>
                <div className="flex flex-col">
                  <label className="text-xs text-[var(--dc-text-muted)]">Merge Y gap min (px):</label>
                  <input type="number" step="5" value={extractionTablesMergeYGapMin} onChange={(e) => setExtractionTablesMergeYGapMin(parseFloat(e.target.value))} className={inputCls} />
                </div>
                <div className="flex flex-col">
                  <label className="text-xs text-[var(--dc-text-muted)]">Footer triggers (comma-separated):</label>
                  <input type="text" value={extractionFooterTriggers} onChange={(e) => setExtractionFooterTriggers(e.target.value)} placeholder="total,sum,subtotal" className={inputCls} />
                </div>
              </div>
            </div>

            <div>
              <h4 className="font-semibold mb-2 text-sm text-[var(--dc-accent)] border-b border-[var(--dc-border)] pb-1">Adaptive Character Scaling</h4>
              <div className="space-y-2">
                <div className="flex flex-col">
                  <label className="text-xs text-[var(--dc-text-muted)]">Char width factor:</label>
                  <input type="number" step="0.05" value={extractionCharWidthFactor} onChange={(e) => setExtractionCharWidthFactor(parseFloat(e.target.value))} className={inputCls} />
                </div>
                <div className="flex flex-col">
                  <label className="text-xs text-[var(--dc-text-muted)]">Bridged threshold:</label>
                  <input type="number" step="0.05" value={extractionBridgedThreshold} onChange={(e) => setExtractionBridgedThreshold(parseFloat(e.target.value))} className={inputCls} />
                </div>
                <div className="flex flex-col">
                  <label className="text-xs text-[var(--dc-text-muted)]">Word spacing factor:</label>
                  <input type="number" step="0.05" value={extractionWordSpacingFactor} onChange={(e) => setExtractionWordSpacingFactor(parseFloat(e.target.value))} className={inputCls} />
                </div>
              </div>
            </div>
          </div>

          <div className="space-y-4">
            <div>
              <h4 className="font-semibold mb-2 text-sm text-[var(--dc-accent)] border-b border-[var(--dc-border)] pb-1">Adaptive Refinement Tuning</h4>
              <div className="space-y-2">
                <div className="flex flex-col">
                  <label className="text-xs text-[var(--dc-text-muted)]">Plaintext Cluster Factor / Gap Gate:</label>
                  <div className="flex gap-2">
                    <input type="number" step="0.1" value={extractionAdaptivePlaintextClusterFactor} onChange={(e) => setExtractionAdaptivePlaintextClusterFactor(parseFloat(e.target.value))} className={`${inputCls} w-1/2`} title="Cluster Factor" />
                    <input type="number" step="0.1" value={extractionAdaptivePlaintextGapGate} onChange={(e) => setExtractionAdaptivePlaintextGapGate(parseFloat(e.target.value))} className={`${inputCls} w-1/2`} title="Gap Gate" />
                  </div>
                </div>
                <div className="flex flex-col">
                  <label className="text-xs text-[var(--dc-text-muted)]">Table Cluster Factor / Gap Gate:</label>
                  <div className="flex gap-2">
                    <input type="number" step="0.1" value={extractionAdaptiveTableClusterFactor} onChange={(e) => setExtractionAdaptiveTableClusterFactor(parseFloat(e.target.value))} className={`${inputCls} w-1/2`} title="Cluster Factor" />
                    <input type="number" step="0.1" value={extractionAdaptiveTableGapGate} onChange={(e) => setExtractionAdaptiveTableGapGate(parseFloat(e.target.value))} className={`${inputCls} w-1/2`} title="Gap Gate" />
                  </div>
                </div>
                <div className="flex flex-col">
                  <label className="text-xs text-[var(--dc-text-muted)]">Column Cluster Factor / Gap Gate:</label>
                  <div className="flex gap-2">
                    <input type="number" step="0.1" value={extractionAdaptiveColumnClusterFactor} onChange={(e) => setExtractionAdaptiveColumnClusterFactor(parseFloat(e.target.value))} className={`${inputCls} w-1/2`} title="Cluster Factor" />
                    <input type="number" step="0.1" value={extractionAdaptiveColumnGapGate} onChange={(e) => setExtractionAdaptiveColumnGapGate(parseFloat(e.target.value))} className={`${inputCls} w-1/2`} title="Gap Gate" />
                  </div>
                </div>
                <div className="flex flex-col pt-2 border-t border-[var(--dc-border)]">
                  <label className="text-xs text-[var(--dc-text-muted)]">Precision Cross-Factors (PT / Table / Col):</label>
                  <div className="flex gap-2">
                    <input type="number" step="0.1" value={extractionAdaptivePlaintextCrossFactor} onChange={(e) => setExtractionAdaptivePlaintextCrossFactor(parseFloat(e.target.value))} className={`${inputCls} w-1/3`} title="Plaintext Cross-Factor" />
                    <input type="number" step="0.1" value={extractionAdaptiveTableCrossFactor} onChange={(e) => setExtractionAdaptiveTableCrossFactor(parseFloat(e.target.value))} className={`${inputCls} w-1/3`} title="Table Cross-Factor" />
                    <input type="number" step="0.1" value={extractionAdaptiveColumnCrossFactor} onChange={(e) => setExtractionAdaptiveColumnCrossFactor(parseFloat(e.target.value))} className={`${inputCls} w-1/3`} title="Column Cross-Factor" />
                  </div>
                </div>
              </div>
            </div>

            <div>
              <h4 className="font-semibold mb-2 text-sm text-[var(--dc-accent)] border-b border-[var(--dc-border)] pb-1">Classifier & Refinement</h4>
              <div className="space-y-2">
                <div className="flex flex-col">
                  <label className="text-xs text-[var(--dc-text-muted)]">Refinement Entropy / Cluster Mod / Cross Mod:</label>
                  <div className="flex gap-2">
                    <input type="number" step="1" value={extractionRefinementEntropyThreshold} onChange={(e) => setExtractionRefinementEntropyThreshold(parseFloat(e.target.value))} className={`${inputCls} w-1/3`} title="Entropy Threshold" />
                    <input type="number" step="0.1" value={extractionRefinementClusterThresholdModifier} onChange={(e) => setExtractionRefinementClusterThresholdModifier(parseFloat(e.target.value))} className={`${inputCls} w-1/3`} title="Cluster Mod" />
                    <input type="number" step="0.1" value={extractionRefinementCrossZoneGapModifier} onChange={(e) => setExtractionRefinementCrossZoneGapModifier(parseFloat(e.target.value))} className={`${inputCls} w-1/3`} title="Cross Mod" />
                  </div>
                </div>
                <div className="flex flex-col">
                  <label className="text-xs text-[var(--dc-text-muted)]">Classifier Gutter / Density Weight:</label>
                  <div className="flex gap-2">
                    <input type="number" step="1" value={extractionClassifierGutterWeight} onChange={(e) => setExtractionClassifierGutterWeight(parseFloat(e.target.value))} className={`${inputCls} w-1/2`} title="Gutter Weight" />
                    <input type="number" step="1" value={extractionClassifierDensityWeight} onChange={(e) => setExtractionClassifierDensityWeight(parseFloat(e.target.value))} className={`${inputCls} w-1/2`} title="Density Weight" />
                  </div>
                </div>
                <div className="flex flex-col">
                  <label className="text-xs text-[var(--dc-text-muted)]">Classifier Mc-Max-D / T-Min-D / T-Min-E:</label>
                  <div className="flex gap-2">
                    <input type="number" step="0.1" value={extractionClassifierMulticolumnDensityMax} onChange={(e) => setExtractionClassifierMulticolumnDensityMax(parseFloat(e.target.value))} className={`${inputCls} w-1/3`} title="Mc Density Max" />
                    <input type="number" step="0.1" value={extractionClassifierTableDensityMin} onChange={(e) => setExtractionClassifierTableDensityMin(parseFloat(e.target.value))} className={`${inputCls} w-1/3`} title="Table Density Min" />
                    <input type="number" step="1" value={extractionClassifierTableEntropyMin} onChange={(e) => setExtractionClassifierTableEntropyMin(parseFloat(e.target.value))} className={`${inputCls} w-1/3`} title="Table Entropy Min" />
                  </div>
                </div>
              </div>
            </div>
          </div>

          <div className="space-y-4">
            <div>
              <h4 className="font-semibold mb-2 text-sm text-[var(--dc-accent)] border-b border-[var(--dc-border)] pb-1">Column & Gutter Tuning</h4>
              <div className="space-y-2">
                <div className="flex flex-col">
                  <label className="text-xs text-[var(--dc-text-muted)]">Min Rows / Gutter Gap Factor:</label>
                  <div className="flex gap-2">
                    <input type="number" min="1" value={extractionColumnsMinContiguousRows} onChange={(e) => setExtractionColumnsMinContiguousRows(parseIntOr(e.target.value, 3))} className={`${inputCls} w-1/2`} title="Min Rows" />
                    <input type="number" step="0.5" value={extractionColumnsGutterGapFactor} onChange={(e) => setExtractionColumnsGutterGapFactor(parseFloat(e.target.value))} className={`${inputCls} w-1/2`} title="Gutter Gap Factor" />
                  </div>
                </div>
                <div className="flex flex-col">
                  <label className="text-xs text-[var(--dc-text-muted)]">Gutter Void Tol / Edge Margin Tol:</label>
                  <div className="flex gap-2">
                    <input type="number" step="0.05" value={extractionColumnsGutterVoidTolerance} onChange={(e) => setExtractionColumnsGutterVoidTolerance(parseFloat(e.target.value))} className={`${inputCls} w-1/2`} title="Void Tolerance" />
                    <input type="number" step="1" value={extractionColumnsEdgeMarginTolerance} onChange={(e) => setExtractionColumnsEdgeMarginTolerance(parseFloat(e.target.value))} className={`${inputCls} w-1/2`} title="Edge Margin Tolerance" />
                  </div>
                </div>
              </div>
            </div>

            <div>
              <h4 className="font-semibold mb-2 text-sm text-[var(--dc-accent)] border-b border-[var(--dc-border)] pb-1">Header Detection & Scoring</h4>
              <div className="space-y-2">
                <div className="flex flex-col">
                  <label className="text-xs text-[var(--dc-text-muted)]">H-Max-Width % / H-Centered Tol %:</label>
                  <div className="flex gap-2">
                    <input type="number" step="0.05" value={extractionHeadersMaxWidthRatio} onChange={(e) => setExtractionHeadersMaxWidthRatio(parseFloat(e.target.value))} className={`${inputCls} w-1/2`} title="Max Width Ratio" />
                    <input type="number" step="0.01" value={extractionHeadersCenteredTolerance} onChange={(e) => setExtractionHeadersCenteredTolerance(parseFloat(e.target.value))} className={`${inputCls} w-1/2`} title="Centered Tolerance" />
                  </div>
                </div>
                <div className="flex flex-col">
                  <label className="text-xs text-[var(--dc-text-muted)]">Size Multipliers (H1 / H2 / H3):</label>
                  <div className="flex gap-2">
                    <input type="number" step="0.1" value={extractionHeadersH1SizeMultiplier} onChange={(e) => setExtractionHeadersH1SizeMultiplier(parseFloat(e.target.value))} className={`${inputCls} w-1/3`} title="H1 Multiplier" />
                    <input type="number" step="0.1" value={extractionHeadersH2SizeMultiplier} onChange={(e) => setExtractionHeadersH2SizeMultiplier(parseFloat(e.target.value))} className={`${inputCls} w-1/3`} title="H2 Multiplier" />
                    <input type="number" step="0.1" value={extractionHeadersH3SizeMultiplier} onChange={(e) => setExtractionHeadersH3SizeMultiplier(parseFloat(e.target.value))} className={`${inputCls} w-1/3`} title="H3 Multiplier" />
                  </div>
                </div>
                <div className="flex flex-col pt-2 border-t border-[var(--dc-border)]">
                  <label className="text-xs text-[var(--dc-text-muted)]">Scoring (Jitter W / Size W / Min-Conf):</label>
                  <div className="flex gap-2">
                    <input type="number" step="0.05" value={extractionScoringJitterPenaltyWeight} onChange={(e) => setExtractionScoringJitterPenaltyWeight(parseFloat(e.target.value))} className={`${inputCls} w-1/3`} title="Jitter Weight" />
                    <input type="number" step="0.01" value={extractionScoringSizePenaltyWeight} onChange={(e) => setExtractionScoringSizePenaltyWeight(parseFloat(e.target.value))} className={`${inputCls} w-1/3`} title="Size Weight" />
                    <input type="number" step="0.05" value={extractionScoringLowConfidenceThreshold} onChange={(e) => setExtractionScoringLowConfidenceThreshold(parseFloat(e.target.value))} className={`${inputCls} w-1/3`} title="Min Confidence" />
                  </div>
                </div>
              </div>
            </div>
          </div>

          <button
            onClick={() =>
              applyConfig(
                {
                  extraction_row_overlap_tolerance: extractionRowOverlapTolerance,
                  extraction_cluster_threshold_factor: extractionClusterThresholdFactor,
                  extraction_zone_proximity: extractionZoneProximity,
                  extraction_cross_zone_gap_factor: extractionCrossZoneGapFactor,
                  extraction_same_zone_gap_factor: extractionSameZoneGapFactor,
                  extraction_significant_gap_gate: extractionSignificantGapGate,
                  extraction_char_width_factor: extractionCharWidthFactor,
                  extraction_bridged_threshold: extractionBridgedThreshold,
                  extraction_word_spacing_factor: extractionWordSpacingFactor,
                  extraction_footer_triggers: extractionFooterTriggers,
                  extraction_table_min_contiguous_rows: extractionTableMinContiguousRows,
                  extraction_table_min_avg_segments: extractionTableMinAvgSegments,
                  extraction_layout_row_lookback: extractionLayoutRowLookback,
                  extraction_layout_table_break_threshold: extractionLayoutTableBreakThreshold,
                  extraction_layout_paragraph_break_threshold: extractionLayoutParagraphBreakThreshold,
                  extraction_layout_max_space_clamp: extractionLayoutMaxSpaceClamp,
                  extraction_tables_column_jitter_tolerance: extractionTablesColumnJitterTolerance,
                  extraction_tables_merge_y_gap_max: extractionTablesMergeYGapMax,
                  extraction_tables_merge_y_gap_min: extractionTablesMergeYGapMin,

                  extraction_adaptive_plaintext_cluster_factor: extractionAdaptivePlaintextClusterFactor,
                  extraction_adaptive_plaintext_gap_gate: extractionAdaptivePlaintextGapGate,
                  extraction_adaptive_table_cluster_factor: extractionAdaptiveTableClusterFactor,
                  extraction_adaptive_table_gap_gate: extractionAdaptiveTableGapGate,
                  extraction_adaptive_column_cluster_factor: extractionAdaptiveColumnClusterFactor,
                  extraction_adaptive_column_gap_gate: extractionAdaptiveColumnGapGate,
                  extraction_adaptive_plaintext_cross_factor: extractionAdaptivePlaintextCrossFactor,
                  extraction_adaptive_table_cross_factor: extractionAdaptiveTableCrossFactor,
                  extraction_adaptive_column_cross_factor: extractionAdaptiveColumnCrossFactor,

                  extraction_refinement_entropy_threshold: extractionRefinementEntropyThreshold,
                  extraction_refinement_cluster_threshold_modifier: extractionRefinementClusterThresholdModifier,
                  extraction_refinement_cross_zone_gap_modifier: extractionRefinementCrossZoneGapModifier,

                  extraction_classifier_gutter_weight: extractionClassifierGutterWeight,
                  extraction_classifier_density_weight: extractionClassifierDensityWeight,
                  extraction_classifier_multicolumn_density_max: extractionClassifierMulticolumnDensityMax,
                  extraction_classifier_table_density_min: extractionClassifierTableDensityMin,
                  extraction_classifier_table_entropy_min: extractionClassifierTableEntropyMin,

                  extraction_columns_min_contiguous_rows: extractionColumnsMinContiguousRows,
                  extraction_columns_gutter_gap_factor: extractionColumnsGutterGapFactor,
                  extraction_columns_gutter_void_tolerance: extractionColumnsGutterVoidTolerance,
                  extraction_columns_edge_margin_tolerance: extractionColumnsEdgeMarginTolerance,

                  extraction_headers_max_width_ratio: extractionHeadersMaxWidthRatio,
                  extraction_headers_centered_tolerance: extractionHeadersCenteredTolerance,
                  extraction_headers_h1_size_multiplier: extractionHeadersH1SizeMultiplier,
                  extraction_headers_h2_size_multiplier: extractionHeadersH2SizeMultiplier,
                  extraction_headers_h3_size_multiplier: extractionHeadersH3SizeMultiplier,

                  extraction_scoring_jitter_penalty_weight: extractionScoringJitterPenaltyWeight,
                  extraction_scoring_size_penalty_weight: extractionScoringSizePenaltyWeight,
                  extraction_scoring_low_confidence_threshold: extractionScoringLowConfidenceThreshold,
                },
                "extraction"
              )
            }
            className="mt-4 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
          >
            Save Extraction Settings
          </button>
          {lastSavedGroup === "extraction" && (
            <span className="ml-2 text-emerald-500 font-medium animate-in fade-in duration-300">
              Saved!
            </span>
          )}
        </div>
      )}

      {activeGroup === "import_export" && (
        <div className={sectionCls}>
          <h3 className="font-bold mb-2">Import/Export Settings</h3>
          <p className="text-sm text-[var(--dc-text-muted)] mt-1">
            Export or import all settings, or only selected categories for team
            sharing and backups.
          </p>

          <label className="block mt-2 font-medium">Mode:</label>
          <div className="mt-1 flex items-center gap-4">
            <label className="flex items-center gap-2">
              <input
                type="radio"
                name="settings-transfer-mode"
                checked={settingsTransferMode === "export"}
                onChange={() => {
                  setSettingsTransferMode("export");
                  setImportPreview(null);
                  setImportWarningsAcknowledged(false);
                }}
              />
              Export
            </label>
            <label className="flex items-center gap-2">
              <input
                type="radio"
                name="settings-transfer-mode"
                checked={settingsTransferMode === "import"}
                onChange={() => setSettingsTransferMode("import")}
              />
              Import
            </label>
          </div>

          <label className="block mt-3 font-medium">Scope:</label>
          <div className="mt-1 flex items-center gap-4">
            <label className="flex items-center gap-2">
              <input
                type="radio"
                name="settings-transfer-scope"
                checked={settingsTransferScope === "all"}
                onChange={() => setSettingsTransferScope("all")}
              />
              All Settings
            </label>
            <label className="flex items-center gap-2">
              <input
                type="radio"
                name="settings-transfer-scope"
                checked={settingsTransferScope === "selected"}
                onChange={() => setSettingsTransferScope("selected")}
              />
              Selected Groups
            </label>
          </div>

          <div className="mt-3 grid grid-cols-1 md:grid-cols-2 gap-2">
            {SETTINGS_GROUP_OPTIONS.map((group) => (
              <label
                key={group.id}
                className="flex items-center gap-2 text-sm"
              >
                <input
                  type="checkbox"
                  checked={selectedSettingsGroups.includes(group.id)}
                  disabled={settingsTransferScope === "all"}
                  onChange={(e) =>
                    toggleSettingsGroup(group.id, e.target.checked)
                  }
                />
                {group.label}
              </label>
            ))}
          </div>

          <div className="mt-3 flex gap-2">
            <button
              type="button"
              onClick={handleExportSettingsBundle}
              className="px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
              disabled={settingsTransferMode !== "export"}
            >
              Export Settings JSON
            </button>
            <button
              type="button"
              onClick={handleImportSettingsBundle}
              className="px-3 py-1.5 bg-[var(--dc-bg-alt)] border border-[var(--dc-border)] rounded"
              disabled={settingsTransferMode !== "import"}
            >
              Select Import File (Preview)
            </button>
            <button
              type="button"
              onClick={handleApplyImportSettingsBundle}
              className="px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded"
              disabled={
                settingsTransferMode !== "import" ||
                !importPreview ||
                !importPreview.valid ||
                (importPreview.warnings.length > 0 && !importWarningsAcknowledged)
              }
            >
              Apply Import from Preview
            </button>
          </div>

          {settingsTransferMode === "import" && importPreview && (
            <div className="mt-3 p-2 border border-[var(--dc-border)] rounded text-sm">
              {(() => {
                const badge = getPreviewBadge(importPreview);
                const icon =
                  badge.icon === "check" ? (
                    <ShieldCheck className="w-3.5 h-3.5" aria-hidden />
                  ) : badge.icon === "alert" ? (
                    <ShieldAlert className="w-3.5 h-3.5" aria-hidden />
                  ) : (
                    <ShieldX className="w-3.5 h-3.5" aria-hidden />
                  );
                return (
                  <p>
                    <strong>Preview Status:</strong>{" "}
                    <span
                      className={`inline - flex items - center gap - 1 ${badge.className} `}
                      title={`Preview status: ${badge.label} `}
                    >
                      {icon}
                      {badge.label}
                    </span>
                  </p>
                );
              })()}
              <p>
                <strong>Preview File:</strong> {importPreview.path}
              </p>
              <p>
                <strong>Schema:</strong> {importPreview.schema_version} (
                {importPreview.valid ? "valid" : "invalid"})
              </p>
              <p>
                <strong>Available Groups:</strong>{" "}
                {importPreview.available_groups.join(", ") || "(none)"}
              </p>
              <p>
                <strong>Target Groups:</strong> {getTargetSettingsGroups().join(", ")}
              </p>
              {importPreview.warnings.length > 0 && (
                <>
                  <p className="text-amber-500">
                    <strong>Warnings:</strong> {importPreview.warnings.join(" | ")}
                  </p>
                  <label className="mt-2 flex items-center gap-2">
                    <input
                      type="checkbox"
                      checked={importWarningsAcknowledged}
                      onChange={(e) =>
                        setImportWarningsAcknowledged(e.target.checked)
                      }
                    />
                    I reviewed and acknowledge the preview warnings before import.
                  </label>
                </>
              )}

              <p className="mt-2 text-xs text-[var(--dc-text-muted)]">
                <strong>Legend:</strong>{" "}
                <span className="inline-flex items-center gap-1 text-emerald-500 mr-2">
                  <ShieldCheck className="w-3.5 h-3.5" aria-hidden />
                  Ready
                </span>
                <span className="inline-flex items-center gap-1 text-amber-500 mr-2">
                  <ShieldAlert className="w-3.5 h-3.5" aria-hidden />
                  Review
                </span>
                <span className="inline-flex items-center gap-1 text-red-500">
                  <ShieldX className="w-3.5 h-3.5" aria-hidden />
                  Blocked
                </span>
              </p>
            </div>
          )}
        </div>
      )}

      {activeGroup === "updates" && (
        <div className={sectionCls}>
          <h3 className="font-bold mb-2">Updates</h3>
          <p className="text-sm text-[var(--dc-text-muted)] mt-1">
            Check for app updates. Configure update endpoints in tauri.conf.json to enable.
          </p>
          <button
            onClick={async () => {
              setUpdateChecking(true);
              setUpdateStatus("");
              try {
                const update = await check();
                if (update) {
                  setUpdateStatus(`Update available: ${update.version}. Downloading...`);
                  await update.downloadAndInstall();
                  setUpdateStatus("Update installed. Restarting...");
                  await relaunch();
                } else {
                  setUpdateStatus("You are on the latest version.");
                }
              } catch (e) {
                setUpdateStatus("Error: " + String(e));
              } finally {
                setUpdateChecking(false);
              }
            }}
            disabled={updateChecking}
            className="mt-2 px-3 py-1.5 bg-[var(--dc-accent)] text-white rounded disabled:opacity-50"
          >
            {updateChecking ? "Checking..." : "Check for Updates"}
          </button>
          {updateStatus && (
            <p className="text-sm mt-2 text-[var(--dc-text-muted)]">{updateStatus}</p>
          )}
        </div>
      )}

      {showKmsEmbedPersistentStrip ? (
        <div
          className="mt-8 border-t-2 border-[var(--dc-border)] pt-4 space-y-3"
          data-testid="kms-embed-persistent-strip"
        >
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div>
              <h3 className="text-sm font-bold text-[var(--dc-text)]">
                KMS embedding: vault re-embed (D6) and health report
              </h3>
              <p className="text-[11px] text-[var(--dc-text-muted)] max-w-3xl mt-1 leading-relaxed">
                These totals are for the last <strong>manual D6 batch re-embed</strong> job only. For live vault-wide
                embedding alignment (including notes processed by the watcher after that batch), use the{" "}
                <strong>Live vault note index and embeddings</strong> section below (refreshes automatically).
              </p>
            </div>
            <div className="flex flex-wrap gap-2 shrink-0">
              <button
                type="button"
                className="px-2.5 py-1 text-xs font-medium rounded border border-[var(--dc-border)] hover:bg-[var(--dc-bg-alt)]"
                onClick={() => setActiveGroup("kms_search_embeddings")}
              >
                KMS Search and embeddings settings
              </button>
              {kmsEmbedStripReport ? (
                <button
                  type="button"
                  className="px-2.5 py-1 text-xs font-medium rounded bg-[var(--dc-accent)] text-white hover:opacity-90"
                  onClick={() => {
                    setKmsEmbedArchivedReportOpen(!kmsEmbedProgress && Boolean(kmsEmbedHealthStored));
                    setKmsEmbedModalOpen(true);
                  }}
                >
                  Open detail modal
                </button>
              ) : null}
            </div>
          </div>

          {kmsEmbedRunning ? (
            <div className="flex flex-wrap items-center gap-2 text-xs">
              <span className="inline-flex items-center rounded-full bg-emerald-600/20 px-2.5 py-0.5 font-semibold text-emerald-800 dark:text-emerald-200">
                Batch running
              </span>
              {kmsEmbedStripReport ? (
                <span className="font-mono text-[11px] text-[var(--dc-text-muted)]">
                  {kmsEmbedStripReport.phase} {kmsEmbedStripReport.done}/{kmsEmbedStripReport.total} failed=
                  {kmsEmbedStripReport.failed}
                  {kmsEmbedStripReport.current_path
                    ? ` | ${kmsEmbedStripReport.current_path}`
                    : ""}
                </span>
              ) : null}
            </div>
          ) : kmsEmbedHealthStored ? (
            <p className="text-[11px] text-[var(--dc-text-muted)]">
              Last saved run: {new Date(kmsEmbedHealthStored.savedAt).toLocaleString()} (generation{" "}
              {kmsEmbedHealthStored.generation}, phase {kmsEmbedHealthStored.phase}). Stored in this browser (localStorage)
              so it survives switching subtabs.
            </p>
          ) : null}

          {kmsEmbedStripReport ? (
            <>
              <dl className="grid grid-cols-[minmax(0,1.4fr)_auto] gap-x-4 gap-y-1 text-xs max-w-lg">
                <dt className="text-[var(--dc-text-muted)]">Total notes (job scope)</dt>
                <dd className="text-right font-mono">{kmsEmbedStripReport.total}</dd>
                <dt className="text-[var(--dc-text-muted)]">Successfully embedded</dt>
                <dd className="text-right font-mono text-emerald-600 dark:text-emerald-400">
                  {kmsEmbedStripReport.done}
                </dd>
                <dt className="text-[var(--dc-text-muted)]">Failures</dt>
                <dd className="text-right font-mono text-red-600 dark:text-red-400">
                  {kmsEmbedStripReport.failed}
                </dd>
                {kmsEmbedRunning && kmsEmbedStripReport.total > 0 ? (
                  <>
                    <dt className="text-[var(--dc-text-muted)]">Remaining (approx.)</dt>
                    <dd className="text-right font-mono">
                      {Math.max(
                        0,
                        kmsEmbedStripReport.total -
                          kmsEmbedStripReport.done -
                          kmsEmbedStripReport.failed
                      )}
                    </dd>
                  </>
                ) : null}
                <dt className="text-[var(--dc-text-muted)]">Elapsed</dt>
                <dd className="text-right font-mono">
                  {(kmsEmbedStripReport.elapsed_ms / 1000).toFixed(1)}s
                </dd>
              </dl>
              {kmsEmbedStripReport.failures && kmsEmbedStripReport.failures.length > 0 ? (
                <div>
                  <div className="text-xs font-semibold mt-2 mb-1">Failed notes</div>
                  <ul className="max-h-36 overflow-y-auto text-[11px] font-mono space-y-2 border border-[var(--dc-border)] rounded p-2 bg-[var(--dc-bg-alt)]">
                    {kmsEmbedStripReport.failures.map((f, i) => (
                      <li key={`${i}-${f.path}`} className="break-all">
                        <span className="font-medium text-[var(--dc-text)]">{f.path}</span>
                        <span className="text-[var(--dc-text-muted)]"> — {f.message}</span>
                      </li>
                    ))}
                  </ul>
                </div>
              ) : null}
              {kmsEmbedStripReport.failures_truncated ? (
                <p className="text-[11px] text-amber-600 dark:text-amber-400">
                  Failure list truncated in the UI. See kms_embedding.log for full detail.
                </p>
              ) : null}
            </>
          ) : null}

          {kmsEmbedMigrateLog ? (
            <p
              className="text-[10px] font-mono text-[var(--dc-text-muted)] break-all border border-[var(--dc-border)] rounded p-2 bg-[var(--dc-bg)]"
              data-testid="kms-embed-migrate-log"
            >
              {kmsEmbedMigrateLog}
            </p>
          ) : null}
        </div>
      ) : null}

      <div
        className={`border-t-2 border-[var(--dc-border)] pt-4 space-y-3 ${showKmsEmbedPersistentStrip ? "mt-4" : "mt-8"}`}
        data-testid="kms-live-embed-dashboard"
      >
        <div>
          <h3 className="text-sm font-bold text-[var(--dc-text)]">Live vault note index and embeddings</h3>
          <p className="text-[11px] text-[var(--dc-text-muted)] max-w-4xl mt-1 leading-relaxed">
            Near real-time view (refreshes about every 5s while Config is open). Only{" "}
            <strong>.md</strong> and <strong>.markdown</strong> files are added to the KMS note index; images, PDFs, and
            other types count under &quot;all files on disk&quot; but not in SQLite. Compare disk counts below to Explorer if
            totals looked wrong before.
          </p>
        </div>
        {!kmsEmbedPolicyDiag ? (
          <p className="text-xs text-[var(--dc-text-muted)]">Loading live counts…</p>
        ) : kmsEmbedPolicyDiag.total_notes_in_index === 0 &&
          !kmsEmbedPolicyDiag.expected_policy_signature ? (
          <p className="text-xs text-[var(--dc-text-muted)]">
            No KMS vault path configured, or the index is empty. Set the vault under KMS settings.
          </p>
        ) : (
          <>
            <div className="text-[11px] font-semibold text-[var(--dc-text)] pt-1">Configured vault folder (disk scan)</div>
            <dl className="grid grid-cols-[minmax(0,1.5fr)_auto] gap-x-4 gap-y-1 text-xs max-w-xl">
              <dt className="text-[var(--dc-text-muted)]">All files on disk (any type)</dt>
              <dd className="text-right font-mono">{kmsEmbedPolicyDiag.vault_all_files_on_disk}</dd>
              <dt className="text-[var(--dc-text-muted)]">Markdown on disk (.md / .markdown)</dt>
              <dd className="text-right font-mono">{kmsEmbedPolicyDiag.vault_markdown_files_on_disk}</dd>
            </dl>
            {kmsEmbedPolicyDiag.vault_all_files_on_disk > kmsEmbedPolicyDiag.vault_markdown_files_on_disk ? (
              <p className="text-[11px] text-[var(--dc-text-muted)] max-w-3xl">
                {kmsEmbedPolicyDiag.vault_all_files_on_disk - kmsEmbedPolicyDiag.vault_markdown_files_on_disk} file(s) are
                not markdown and are excluded from the note index (this often explains Explorer showing more items than the
                dashboard).
              </p>
            ) : null}
            {kmsEmbedPolicyDiag.vault_markdown_files_on_disk > kmsEmbedPolicyDiag.total_notes_in_index ? (
              <p className="text-[11px] text-amber-700 dark:text-amber-300 max-w-3xl">
                {kmsEmbedPolicyDiag.vault_markdown_files_on_disk - kmsEmbedPolicyDiag.total_notes_in_index} markdown
                file(s) on disk are not in the SQLite index yet. Open KMS and let sync finish, or wait for the file
                watcher.
              </p>
            ) : null}
            {kmsEmbedPolicyDiag.total_notes_in_index > kmsEmbedPolicyDiag.vault_markdown_files_on_disk ? (
              <p className="text-[11px] text-[var(--dc-text-muted)] max-w-3xl">
                The index has more rows than markdown files on disk (e.g. deleted files not cleaned up yet, or a
                different vault path was used earlier). A full vault sync removes missing paths.
              </p>
            ) : null}
            <div className="text-[11px] font-semibold text-[var(--dc-text)] pt-2">SQLite note index</div>
            <dl className="grid grid-cols-[minmax(0,1.5fr)_auto] gap-x-4 gap-y-1 text-xs max-w-xl">
              <dt className="text-[var(--dc-text-muted)]">Total rows in index</dt>
              <dd className="text-right font-mono">{kmsEmbedPolicyDiag.total_notes_in_index}</dd>
              <dt className="text-[var(--dc-text-muted)]">Indexed (sync OK)</dt>
              <dd className="text-right font-mono">{kmsEmbedPolicyDiag.indexed_note_count}</dd>
              <dt className="text-[var(--dc-text-muted)]">Pending sync</dt>
              <dd className="text-right font-mono">{kmsEmbedPolicyDiag.pending_note_count}</dd>
              <dt className="text-[var(--dc-text-muted)]">Sync / read failed</dt>
              <dd className="text-right font-mono text-red-600 dark:text-red-400">
                {kmsEmbedPolicyDiag.failed_sync_note_count}
              </dd>
              {kmsEmbedPolicyDiag.other_sync_status_note_count > 0 ? (
                <>
                  <dt className="text-[var(--dc-text-muted)]">Other status</dt>
                  <dd className="text-right font-mono text-amber-600 dark:text-amber-400">
                    {kmsEmbedPolicyDiag.other_sync_status_note_count}
                  </dd>
                </>
              ) : null}
              <dt className="text-[var(--dc-text-muted)]">Embeddings aligned (current policy)</dt>
              <dd className="text-right font-mono text-emerald-600 dark:text-emerald-400">
                {kmsEmbedPolicyDiag.embedding_aligned_note_count}
              </dd>
              <dt className="text-[var(--dc-text-muted)]">Embeddings stale / missing / mismatch</dt>
              <dd className="text-right font-mono text-amber-600 dark:text-amber-400">
                {kmsEmbedPolicyDiag.stale_embedding_note_count}
              </dd>
            </dl>
            {kmsEmbedPolicyDiag.expected_policy_signature ? (
              <p className="text-[10px] text-[var(--dc-text-muted)] font-mono break-all">
                Current expected fingerprint: {kmsEmbedPolicyDiag.expected_policy_signature}
              </p>
            ) : null}
          </>
        )}
      </div>

      <Dialog
        open={kmsEmbedModalOpen}
        onOpenChange={(open) => {
          if (!open) {
            setKmsEmbedArchivedReportOpen(false);
          }
          setKmsEmbedModalOpen(open);
        }}
      >
        <DialogContent className="max-w-lg max-h-[85vh] overflow-y-auto" showClose={false}>
          <div className="absolute right-3 top-3 flex items-center gap-1">
            {kmsEmbedRunning ? (
              <button
                type="button"
                className="rounded-sm p-2 opacity-80 hover:opacity-100 focus:outline-none focus:ring-2 focus:ring-[var(--dc-accent)]"
                title="Minimize (close modal; progress stays in the panel below)"
                aria-label="Minimize embedding progress"
                onClick={() => setKmsEmbedModalOpen(false)}
              >
                <Minimize2 className="h-4 w-4" />
              </button>
            ) : null}
            <button
              type="button"
              className="rounded-sm p-2 opacity-80 hover:opacity-100 focus:outline-none focus:ring-2 focus:ring-[var(--dc-accent)]"
              title="Close"
              aria-label="Close"
              onClick={() => setKmsEmbedModalOpen(false)}
            >
              <X className="h-4 w-4" />
            </button>
          </div>
          <DialogHeader className="pr-14">
            <DialogTitle>Note embedding migration</DialogTitle>
            <DialogDescription>
              Background re-embed (D6). Cancel stops the current job; notes already updated keep new vectors. Minimize or
              close this dialog to use other settings; live progress and the saved health report remain in the persistent
              panel at the bottom of Config on every sub-tab.
            </DialogDescription>
          </DialogHeader>
          {embedReportForModal ? (
            <>
              <div className="space-y-2 text-sm text-[var(--dc-text)]">
                <div>
                  Phase: <span className="font-medium">{embedReportForModal.phase}</span>
                </div>
                <div>
                  Progress: {embedReportForModal.done} / {embedReportForModal.total}
                </div>
                <div>Failed: {embedReportForModal.failed}</div>
                {embedReportForModal.current_path ? (
                  <div className="font-mono text-[10px] break-all text-[var(--dc-text-muted)]">
                    Current: {embedReportForModal.current_path}
                  </div>
                ) : null}
                {kmsEmbedRunning &&
                embedReportForModal.done > 0 &&
                embedReportForModal.total > embedReportForModal.done ? (
                  <div className="text-[var(--dc-text-muted)] text-xs">
                    ETA about{" "}
                    {Math.max(
                      0,
                      Math.round(
                        (embedReportForModal.elapsed_ms /
                          1000 /
                          embedReportForModal.done) *
                          (embedReportForModal.total - embedReportForModal.done)
                      )
                    )}
                    s (linear estimate)
                  </div>
                ) : null}
                {embedReportForModal.detail ? (
                  <p className="text-xs text-[var(--dc-text-muted)]">{embedReportForModal.detail}</p>
                ) : null}
              </div>

              <div className="mt-4 rounded-md border border-[var(--dc-border)] bg-[var(--dc-bg)] p-3">
                <div className="text-sm font-semibold mb-2">Embedding health report</div>
                <dl className="grid grid-cols-[minmax(0,1fr)_auto] gap-x-3 gap-y-1 text-xs">
                  <dt className="text-[var(--dc-text-muted)]">Total notes (job scope)</dt>
                  <dd className="text-right font-mono">{embedReportForModal.total}</dd>
                  <dt className="text-[var(--dc-text-muted)]">Successfully embedded</dt>
                  <dd className="text-right font-mono text-emerald-600 dark:text-emerald-400">
                    {embedReportForModal.done}
                  </dd>
                  <dt className="text-[var(--dc-text-muted)]">Failures</dt>
                  <dd className="text-right font-mono text-red-600 dark:text-red-400">
                    {embedReportForModal.failed}
                  </dd>
                  {kmsEmbedRunning && embedReportForModal.total > 0 ? (
                    <>
                      <dt className="text-[var(--dc-text-muted)]">Remaining (approx.)</dt>
                      <dd className="text-right font-mono">
                        {Math.max(
                          0,
                          embedReportForModal.total -
                            embedReportForModal.done -
                            embedReportForModal.failed
                        )}
                      </dd>
                    </>
                  ) : null}
                  <dt className="text-[var(--dc-text-muted)]">Elapsed</dt>
                  <dd className="text-right font-mono">
                    {(embedReportForModal.elapsed_ms / 1000).toFixed(1)}s
                  </dd>
                </dl>
                {embedReportForModal.failures && embedReportForModal.failures.length > 0 ? (
                  <div className="mt-3">
                    <div className="text-xs font-semibold mb-1">Failed notes (action may be required)</div>
                    <ul className="max-h-40 overflow-y-auto text-[11px] font-mono space-y-2 border border-[var(--dc-border)] rounded p-2 bg-[var(--dc-bg-alt)]">
                      {embedReportForModal.failures.map((f, i) => (
                        <li key={`${i}-${f.path}`} className="break-all">
                          <div className="font-medium text-[var(--dc-text)]">{f.path}</div>
                          <div className="text-[var(--dc-text-muted)]">{f.message}</div>
                        </li>
                      ))}
                    </ul>
                  </div>
                ) : null}
                {embedReportForModal.failures_truncated ? (
                  <p className="mt-2 text-xs text-amber-600 dark:text-amber-400">
                    Not all failures are listed here ({embedReportForModal.failures?.length ?? 0} shown). Check the
                    diagnostic log file for full detail.
                  </p>
                ) : null}
                <p className="mt-2 text-[10px] text-[var(--dc-text-muted)]">
                  Summary is saved in the browser when a run finishes. Incremental embedding (watcher / sync) is separate
                  from these D6 batch totals.
                </p>
              </div>
            </>
          ) : (
            <p className="text-sm text-[var(--dc-text-muted)]">Waiting for progress from the app…</p>
          )}
          <DialogFooter className="flex flex-wrap gap-2 sm:justify-end">
            <button
              type="button"
              className="px-3 py-1.5 rounded border border-[var(--dc-border)] text-sm font-medium hover:bg-[var(--dc-border)] disabled:opacity-50 disabled:pointer-events-none"
              disabled={
                !kmsEmbedProgress ||
                ["complete", "cancelled", "nothing_to_do", "error"].includes(kmsEmbedProgress.phase)
              }
              onClick={() => {
                void getTaurpc().kms_cancel_note_embedding_migration();
              }}
            >
              Cancel migration
            </button>
            <button
              type="button"
              className="px-3 py-1.5 rounded bg-[var(--dc-accent)] text-white text-sm font-medium hover:opacity-90"
              onClick={() => setKmsEmbedModalOpen(false)}
            >
              Close
            </button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
