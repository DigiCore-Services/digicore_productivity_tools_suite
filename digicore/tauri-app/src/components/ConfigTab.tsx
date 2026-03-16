import { useEffect, useState } from "react";
import { emit } from "@tauri-apps/api/event";
import { getTaurpc } from "../lib/taurpc";
import {
  AppearanceTransparencyRuleDto,
  ConfigUpdateDto,
  ExpansionStatsDto,
  UiPrefsDto,
  IndexingStatusDto,
  KmsIndexStatusRow
} from "../bindings";
import { resolveTheme, applyThemeToDocument } from "@/lib/theme";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { open, save } from "@tauri-apps/plugin-dialog";
import { ShieldAlert, ShieldCheck, ShieldX } from "lucide-react";
import { normalizeAppState } from "@/lib/normalizeState";
import type { AppState } from "../types";
import { lazy, Suspense } from "react";
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
  { id: "core", label: "Core" },
  { id: "script_runtime", label: "Script Runtime" },
  { id: "corpus", label: "Corpus Generation" },
  { id: "extraction", label: "Extraction Engine" },
  { id: "appearance", label: "Appearance" },
  { id: "statistics", label: "Statistics" },
  { id: "log", label: "Log" },
  { id: "semantic_search", label: "Semantic Search" },
] as const;

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
      setClipMaxDepth(appState.clip_history_max_depth ?? 20);
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
    groupName: string
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
      <h2 className="text-xl font-semibold mb-4">
        Configurations and Settings
      </h2>
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
            Hover preview
          </label>
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
          <input
            type="text"
            value={copyBlacklistProcesses}
            onChange={(e) => setCopyBlacklistProcesses(e.target.value)}
            placeholder="KeePassXC.exe, 1Password.exe"
            className={inputCls}
          />
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
    </div>
  );
}
