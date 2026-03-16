use digicore_core::domain::ports::TextExtractionPort;
use digicore_core::domain::{ExtractionSource, ExtractionMimeType, ExtractionResult, TableBlock, SemanticEntity};
use windows::Graphics::Imaging::BitmapDecoder;
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};
use std::fs;


/// Root configuration structure for runtime tuning.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RuntimeConfig {
    pub extraction: ExtractionConfig,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(default)]
pub struct ExtractionConfig {
    pub layout_heuristics: LayoutHeuristicsConfig,
    pub tables: TablesConfig,
    pub adaptive_overrides: AdaptiveOverridesConfig,
    pub refinement: RefinementConfig,
    pub classifier: ClassifierConfig,
    pub columns: ColumnsConfig,
    pub headers: HeadersConfig,
    pub scoring: ScoringConfig,
}

/// Configuration for OCR layout heuristics to allow automated tuning.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct LayoutHeuristicsConfig {
    pub row_overlap_tolerance: f32, // fraction of height
    pub cluster_threshold_factor: f32,
    pub zone_proximity: f32,
    pub cross_zone_gap_factor: f32,
    pub same_zone_gap_factor: f32,
    pub significant_gap_gate: f32,
    pub char_width_factor: f32,
    pub bridged_threshold: f32,
    pub word_spacing_factor: f32,
    pub row_lookback: usize,
    pub table_break_threshold: f32,
    pub paragraph_break_threshold: f32,
    pub max_space_clamp: usize,
}
impl Default for LayoutHeuristicsConfig {
    fn default() -> Self {
        Self {
            row_overlap_tolerance: 0.6,
            cluster_threshold_factor: 0.45,
            zone_proximity: 15.0,
            cross_zone_gap_factor: 0.25,
            same_zone_gap_factor: 0.8,
            significant_gap_gate: 0.8,
            char_width_factor: 0.45,
            bridged_threshold: 0.4,
            word_spacing_factor: 0.2,
            row_lookback: 5,
            table_break_threshold: 3.0,
            paragraph_break_threshold: 3.0,
            max_space_clamp: 6,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct TablesConfig {
    pub footer_triggers: Vec<String>,
    pub min_contiguous_rows: usize,
    pub min_avg_segments: f32,
    pub column_jitter_tolerance: f32,
    pub merge_y_gap_max: f32,
    pub merge_y_gap_min: f32,
}
impl Default for TablesConfig {
    fn default() -> Self {
        Self {
            footer_triggers: vec!["total".to_string(), "sum".to_string(), "subtotal".to_string(), "balance".to_string()],
            min_contiguous_rows: 4,
            min_avg_segments: 3.1,
            column_jitter_tolerance: 20.0,
            merge_y_gap_max: 100.0,
            merge_y_gap_min: 40.0,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ColumnsConfig {
    pub min_contiguous_rows: usize,
    pub gutter_gap_factor: f32,
    pub gutter_void_tolerance: f32,
    pub edge_margin_tolerance: f32,
}
impl Default for ColumnsConfig {
    fn default() -> Self {
        Self {
            min_contiguous_rows: 3,
            gutter_gap_factor: 5.0,
            gutter_void_tolerance: 0.7,
            edge_margin_tolerance: 30.0,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct HeadersConfig {
    pub max_width_ratio: f32,
    pub centered_tolerance: f32,
    pub h1_size_multiplier: f32,
    pub h2_size_multiplier: f32,
    pub h3_size_multiplier: f32,
}
impl Default for HeadersConfig {
    fn default() -> Self {
        Self {
            max_width_ratio: 0.75,
            centered_tolerance: 0.12,
            h1_size_multiplier: 1.6,
            h2_size_multiplier: 1.3,
            h3_size_multiplier: 1.2,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ScoringConfig {
    pub jitter_penalty_weight: f32,
    pub size_penalty_weight: f32,
    pub low_confidence_threshold: f32,
}
impl Default for ScoringConfig {
    fn default() -> Self {
        Self {
            jitter_penalty_weight: 0.4,
            size_penalty_weight: 0.1,
            low_confidence_threshold: 0.6,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AdaptiveOverridesConfig {
    pub plaintext_cluster_factor: f32,
    pub plaintext_gap_gate: f32,
    pub table_cluster_factor: f32,
    pub table_gap_gate: f32,
    pub column_cluster_factor: f32,
    pub column_gap_gate: f32,
    pub plaintext_cross_factor: f32,
    pub table_cross_factor: f32,
    pub column_cross_factor: f32,
}
impl Default for AdaptiveOverridesConfig {
    fn default() -> Self {
        Self {
            plaintext_cluster_factor: 1.1,
            plaintext_gap_gate: 0.5,
            table_cluster_factor: 0.45,
            table_gap_gate: 1.2,
            column_cluster_factor: 0.45,
            column_gap_gate: 0.8,
            plaintext_cross_factor: 1.0,
            table_cross_factor: 0.25,
            column_cross_factor: 0.8,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct RefinementConfig {
    pub entropy_threshold: f32,
    pub cluster_threshold_modifier: f32,
    pub cross_zone_gap_modifier: f32,
}
impl Default for RefinementConfig {
    fn default() -> Self {
        Self {
            entropy_threshold: 50.0,
            cluster_threshold_modifier: 0.8,
            cross_zone_gap_modifier: 1.2,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ClassifierConfig {
    pub gutter_weight: f32,
    pub density_weight: f32,
    pub multicolumn_density_max: f32,
    pub table_density_min: f32,
    pub table_entropy_min: f32,
}
impl Default for ClassifierConfig {
    fn default() -> Self {
        Self {
            gutter_weight: 15.0,
            density_weight: 10.0,
            multicolumn_density_max: 0.4,
            table_density_min: 1.0,
            table_entropy_min: 40.0,
        }
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            extraction: ExtractionConfig::default()
        }
    }
}

impl RuntimeConfig {
    pub fn load_from_json_adapter(storage: &dyn crate::ports::StoragePort) -> Option<Self> {
        use crate::ports::storage_keys;
        
        let mut def = Self::default();
        let ext = &mut def.extraction;
        
        ext.layout_heuristics = LayoutHeuristicsConfig {
            row_overlap_tolerance: storage.get(storage_keys::EXTRACTION_ROW_OVERLAP_TOLERANCE).and_then(|s| s.parse().ok()).unwrap_or(0.6),
            cluster_threshold_factor: storage.get(storage_keys::EXTRACTION_CLUSTER_THRESHOLD_FACTOR).and_then(|s| s.parse().ok()).unwrap_or(0.45),
            zone_proximity: storage.get(storage_keys::EXTRACTION_ZONE_PROXIMITY).and_then(|s| s.parse().ok()).unwrap_or(15.0),
            cross_zone_gap_factor: storage.get(storage_keys::EXTRACTION_CROSS_ZONE_GAP_FACTOR).and_then(|s| s.parse().ok()).unwrap_or(0.25),
            same_zone_gap_factor: storage.get(storage_keys::EXTRACTION_SAME_ZONE_GAP_FACTOR).and_then(|s| s.parse().ok()).unwrap_or(0.8),
            significant_gap_gate: storage.get(storage_keys::EXTRACTION_SIGNIFICANT_GAP_GATE).and_then(|s| s.parse().ok()).unwrap_or(0.8),
            char_width_factor: storage.get(storage_keys::EXTRACTION_CHAR_WIDTH_FACTOR).and_then(|s| s.parse().ok()).unwrap_or(0.45),
            bridged_threshold: storage.get(storage_keys::EXTRACTION_BRIDGED_THRESHOLD).and_then(|s| s.parse().ok()).unwrap_or(0.4),
            word_spacing_factor: storage.get(storage_keys::EXTRACTION_WORD_SPACING_FACTOR).and_then(|s| s.parse().ok()).unwrap_or(0.2),
            row_lookback: storage.get(storage_keys::EXTRACTION_LAYOUT_ROW_LOOKBACK).and_then(|s| s.parse().ok()).unwrap_or(5),
            table_break_threshold: storage.get(storage_keys::EXTRACTION_LAYOUT_TABLE_BREAK_THRESHOLD).and_then(|s| s.parse().ok()).unwrap_or(3.0),
            paragraph_break_threshold: storage.get(storage_keys::EXTRACTION_LAYOUT_PARAGRAPH_BREAK_THRESHOLD).and_then(|s| s.parse().ok()).unwrap_or(3.0),
            max_space_clamp: storage.get(storage_keys::EXTRACTION_LAYOUT_MAX_SPACE_CLAMP).and_then(|s| s.parse().ok()).unwrap_or(6),
        };
        ext.tables = TablesConfig {
            footer_triggers: storage.get(storage_keys::EXTRACTION_FOOTER_TRIGGERS)
                .map(|s| s.split(',').map(|t| t.trim().to_string()).collect())
                .unwrap_or_else(|| vec!["total".to_string(), "sum".to_string(), "subtotal".to_string(), "balance".to_string()]),
            min_contiguous_rows: storage.get(storage_keys::EXTRACTION_TABLE_MIN_CONTIGUOUS_ROWS).and_then(|s| s.parse().ok()).unwrap_or(4),
            min_avg_segments: storage.get(storage_keys::EXTRACTION_TABLE_MIN_AVG_SEGMENTS).and_then(|s| s.parse().ok()).unwrap_or(3.1),
            column_jitter_tolerance: storage.get(storage_keys::EXTRACTION_TABLES_COLUMN_JITTER_TOLERANCE).and_then(|s| s.parse().ok()).unwrap_or(20.0),
            merge_y_gap_max: storage.get(storage_keys::EXTRACTION_TABLES_MERGE_Y_GAP_MAX).and_then(|s| s.parse().ok()).unwrap_or(100.0),
            merge_y_gap_min: storage.get(storage_keys::EXTRACTION_TABLES_MERGE_Y_GAP_MIN).and_then(|s| s.parse().ok()).unwrap_or(40.0),
        };

        if let Some(s) = storage.get(storage_keys::EXTRACTION_ADAPTIVE_PLAINTEXT_CLUSTER_FACTOR) { if let Ok(v) = s.parse() { ext.adaptive_overrides.plaintext_cluster_factor = v; } }
        if let Some(s) = storage.get(storage_keys::EXTRACTION_ADAPTIVE_PLAINTEXT_GAP_GATE) { if let Ok(v) = s.parse() { ext.adaptive_overrides.plaintext_gap_gate = v; } }
        if let Some(s) = storage.get(storage_keys::EXTRACTION_ADAPTIVE_TABLE_CLUSTER_FACTOR) { if let Ok(v) = s.parse() { ext.adaptive_overrides.table_cluster_factor = v; } }
        if let Some(s) = storage.get(storage_keys::EXTRACTION_ADAPTIVE_TABLE_GAP_GATE) { if let Ok(v) = s.parse() { ext.adaptive_overrides.table_gap_gate = v; } }
        if let Some(s) = storage.get(storage_keys::EXTRACTION_ADAPTIVE_COLUMN_CLUSTER_FACTOR) { if let Ok(v) = s.parse() { ext.adaptive_overrides.column_cluster_factor = v; } }
        if let Some(s) = storage.get(storage_keys::EXTRACTION_ADAPTIVE_COLUMN_GAP_GATE) { if let Ok(v) = s.parse() { ext.adaptive_overrides.column_gap_gate = v; } }
        if let Some(s) = storage.get(storage_keys::EXTRACTION_ADAPTIVE_PLAINTEXT_CROSS_FACTOR) { if let Ok(v) = s.parse() { ext.adaptive_overrides.plaintext_cross_factor = v; } }
        if let Some(s) = storage.get(storage_keys::EXTRACTION_ADAPTIVE_TABLE_CROSS_FACTOR) { if let Ok(v) = s.parse() { ext.adaptive_overrides.table_cross_factor = v; } }
        if let Some(s) = storage.get(storage_keys::EXTRACTION_ADAPTIVE_COLUMN_CROSS_FACTOR) { if let Ok(v) = s.parse() { ext.adaptive_overrides.column_cross_factor = v; } }

        if let Some(s) = storage.get(storage_keys::EXTRACTION_REFINEMENT_ENTROPY_THRESHOLD) { if let Ok(v) = s.parse() { ext.refinement.entropy_threshold = v; } }
        if let Some(s) = storage.get(storage_keys::EXTRACTION_REFINEMENT_CLUSTER_THRESHOLD_MODIFIER) { if let Ok(v) = s.parse() { ext.refinement.cluster_threshold_modifier = v; } }
        if let Some(s) = storage.get(storage_keys::EXTRACTION_REFINEMENT_CROSS_ZONE_GAP_MODIFIER) { if let Ok(v) = s.parse() { ext.refinement.cross_zone_gap_modifier = v; } }

        if let Some(s) = storage.get(storage_keys::EXTRACTION_CLASSIFIER_GUTTER_WEIGHT) { if let Ok(v) = s.parse() { ext.classifier.gutter_weight = v; } }
        if let Some(s) = storage.get(storage_keys::EXTRACTION_CLASSIFIER_DENSITY_WEIGHT) { if let Ok(v) = s.parse() { ext.classifier.density_weight = v; } }
        if let Some(s) = storage.get(storage_keys::EXTRACTION_CLASSIFIER_MULTICOLUMN_DENSITY_MAX) { if let Ok(v) = s.parse() { ext.classifier.multicolumn_density_max = v; } }
        if let Some(s) = storage.get(storage_keys::EXTRACTION_CLASSIFIER_TABLE_DENSITY_MIN) { if let Ok(v) = s.parse() { ext.classifier.table_density_min = v; } }
        if let Some(s) = storage.get(storage_keys::EXTRACTION_CLASSIFIER_TABLE_ENTROPY_MIN) { if let Ok(v) = s.parse() { ext.classifier.table_entropy_min = v; } }

        if let Some(s) = storage.get(storage_keys::EXTRACTION_COLUMNS_MIN_CONTIGUOUS_ROWS) { if let Ok(v) = s.parse() { ext.columns.min_contiguous_rows = v; } }
        if let Some(s) = storage.get(storage_keys::EXTRACTION_COLUMNS_GUTTER_GAP_FACTOR) { if let Ok(v) = s.parse() { ext.columns.gutter_gap_factor = v; } }
        if let Some(s) = storage.get(storage_keys::EXTRACTION_COLUMNS_GUTTER_VOID_TOLERANCE) { if let Ok(v) = s.parse() { ext.columns.gutter_void_tolerance = v; } }
        if let Some(s) = storage.get(storage_keys::EXTRACTION_COLUMNS_EDGE_MARGIN_TOLERANCE) { if let Ok(v) = s.parse() { ext.columns.edge_margin_tolerance = v; } }

        if let Some(s) = storage.get(storage_keys::EXTRACTION_HEADERS_MAX_WIDTH_RATIO) { if let Ok(v) = s.parse() { ext.headers.max_width_ratio = v; } }
        if let Some(s) = storage.get(storage_keys::EXTRACTION_HEADERS_CENTERED_TOLERANCE) { if let Ok(v) = s.parse() { ext.headers.centered_tolerance = v; } }
        if let Some(s) = storage.get(storage_keys::EXTRACTION_HEADERS_H1_SIZE_MULTIPLIER) { if let Ok(v) = s.parse() { ext.headers.h1_size_multiplier = v; } }
        if let Some(s) = storage.get(storage_keys::EXTRACTION_HEADERS_H2_SIZE_MULTIPLIER) { if let Ok(v) = s.parse() { ext.headers.h2_size_multiplier = v; } }
        if let Some(s) = storage.get(storage_keys::EXTRACTION_HEADERS_H3_SIZE_MULTIPLIER) { if let Ok(v) = s.parse() { ext.headers.h3_size_multiplier = v; } }

        if let Some(s) = storage.get(storage_keys::EXTRACTION_SCORING_JITTER_PENALTY_WEIGHT) { if let Ok(v) = s.parse() { ext.scoring.jitter_penalty_weight = v; } }
        if let Some(s) = storage.get(storage_keys::EXTRACTION_SCORING_SIZE_PENALTY_WEIGHT) { if let Ok(v) = s.parse() { ext.scoring.size_penalty_weight = v; } }
        if let Some(s) = storage.get(storage_keys::EXTRACTION_SCORING_LOW_CONFIDENCE_THRESHOLD) { if let Ok(v) = s.parse() { ext.scoring.low_confidence_threshold = v; } }

        Some(def)
    }
}

/// Document classification for adaptive tuning.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum DocClass {
    Plaintext,
    FormsTables,
    MultiColumn,
}

impl DocClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            DocClass::Plaintext => "plaintext",
            DocClass::FormsTables => "forms_tables",
            DocClass::MultiColumn => "multi_column",
        }
    }
}

#[derive(Clone, Debug)]
struct WordInfo {
    text: String,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

#[derive(Clone, Debug)]
struct ColumnZone {
    min_x: f32,
    max_x: f32,
    char_index: usize,
}

#[derive(Clone, Debug)]
struct RowSegment {
    text: String,
    zone_idx: usize,
    start_x: f32,
    flags: Vec<String>,
}

#[derive(Clone, Debug)]
struct ProcessedRow {
    is_table_candidate: bool,
    segments: Vec<RowSegment>,
    y_top: f32,
    h: f32,
    row_x: f32,
}

struct ReconstructionResult {
    text: String,
    tables: Vec<TableBlock>,
    entities: Vec<SemanticEntity>,
    diagnostics: Vec<serde_json::Value>,
    doc_class: DocClass,
    entropy: f32,
    details: serde_json::Value,
}

/// Windows-native OCR adapter using Windows.Media.Ocr.
pub struct WindowsNativeOcrAdapter {
    pub config: Option<RuntimeConfig>,
}

impl WindowsNativeOcrAdapter {
    pub fn new(config: Option<RuntimeConfig>) -> Self {
        Self { config }
    }
}

impl Default for WindowsNativeOcrAdapter {
    fn default() -> Self {
        Self {
            config: None,
        }
    }
}

#[async_trait::async_trait]
impl TextExtractionPort for WindowsNativeOcrAdapter {
    fn can_handle(&self, mime_type: ExtractionMimeType) -> bool {
        match mime_type {
            ExtractionMimeType::Png | ExtractionMimeType::Jpeg => true,
            _ => false,
        }
    }

    async fn extract(&self, source: ExtractionSource, mime_type: ExtractionMimeType) -> std::result::Result<ExtractionResult, String> {
        let start_time = chrono::Utc::now();
        if !self.can_handle(mime_type) {
            return Err("WindowsNativeOcrAdapter cannot handle this MIME type".to_string());
        }

        let bytes = match source {
            ExtractionSource::Buffer(ref bytes) => bytes.clone(),
            ExtractionSource::File(ref path) => {
                fs::read(path).map_err(|e| format!("Failed to read file: {}", e))?
            }
        };

        if bytes.is_empty() {
            return Err("Empty image buffer".to_string());
        }

        // 1. Create a random access stream from the bytes
        let stream = InMemoryRandomAccessStream::new().map_err(|e: windows::core::Error| e.to_string())?;
        let writer = DataWriter::CreateDataWriter(&stream).map_err(|e| e.to_string())?;
        writer.WriteBytes(&bytes).map_err(|e| e.to_string())?;
        
        // Store and Flush are IAsyncOperations
        writer.StoreAsync().map_err(|e| e.to_string())?.get().map_err(|e| e.to_string())?;
        writer.FlushAsync().map_err(|e| e.to_string())?.get().map_err(|e| e.to_string())?;
        stream.Seek(0).map_err(|e| e.to_string())?;

        // 2. Load the decoder from the stream
        let decoder = BitmapDecoder::CreateAsync(&stream).map_err(|e| e.to_string())?.get().map_err(|e| e.to_string())?;
        let bitmap = decoder.GetSoftwareBitmapAsync().map_err(|e| e.to_string())?.get().map_err(|e| e.to_string())?;

        // 3. Perform OCR
        let engine = OcrEngine::TryCreateFromUserProfileLanguages().map_err(|e| e.to_string())?;
        let result = engine.RecognizeAsync(&bitmap).map_err(|e| e.to_string())?.get().map_err(|e| e.to_string())?;

        let ocr_lines = result.Lines().map_err(|e| e.to_string())?;

        let mut all_words = Vec::new();
        let mut total_h = 0.0;
        for line in ocr_lines {
            if let Ok(words) = line.Words() {
                for word in words {
                    if let (Ok(text), Ok(rect)) = (word.Text(), word.BoundingRect()) {
                        all_words.push(WordInfo {
                            text: text.to_string(),
                            x: rect.X,
                            y: rect.Y,
                            w: rect.Width,
                            h: rect.Height,
                        });
                        total_h += rect.Height;
                    }
                }
            }
        }
        let _avg_h = if all_words.is_empty() { 20.0 } else { total_h / all_words.len() as f32 };
        let img_width = match bitmap.PixelWidth() { Ok(w) => w as f32, Err(_) => 1000.0 };

        if all_words.is_empty() {
            return Ok(ExtractionResult {
                text: result.Text().map_err(|e| e.to_string())?.to_string(),
                confidence: 1.0,
                metadata: serde_json::json!({ "adapter": "WindowsNativeOcr", "fallback": true }),
                tables: None,
                entities: None,
                diagnostics: None,
            });
        }

        let img_height = match bitmap.PixelHeight() { Ok(h) => h as f32, Err(_) => 1000.0 };

        // --- PHASE 47: Recursive Refinement Loop ---
        use crate::adapters::storage::json_file_storage::JsonFileStorageAdapter;
        use crate::ports::StoragePort;
        let initial_config = self.config.clone().unwrap_or_else(|| {
            RuntimeConfig::load_from_json_adapter(&JsonFileStorageAdapter::load()).unwrap_or_default()
        });
        let first_pass = self.perform_reconstruction_pass(&all_words, &initial_config, img_width, img_height);
        
        let mut final_result = first_pass;
        let mut refinement_executed = false;

        // Trigger Refinement for High Entropy layouts
        if final_result.entropy > initial_config.extraction.refinement.entropy_threshold {
            let mut refined_config = initial_config;
            refined_config.extraction.layout_heuristics.cluster_threshold_factor *= refined_config.extraction.refinement.cluster_threshold_modifier; 
            refined_config.extraction.layout_heuristics.cross_zone_gap_factor *= refined_config.extraction.refinement.cross_zone_gap_modifier; 
            
            let second_pass = self.perform_reconstruction_pass(&all_words, &refined_config, img_width, img_height);
            
            if second_pass.entropy < final_result.entropy {
                final_result = second_pass;
                refinement_executed = true;
            }
        }

        let metadata = serde_json::json!({
            "adapter": "WindowsNativeOcr",
            "semantic_pass": true,
            "universal_capture": true,
            "adaptive_profile": final_result.doc_class.as_str(),
            "refinement_executed": refinement_executed,
            "performance_metrics": {
                "extraction_ms": chrono::Utc::now().signed_duration_since(start_time).num_milliseconds(),
                "entropy": final_result.entropy,
                "complexity_class": if final_result.entropy < 20.0 { "low" } else if final_result.entropy < 50.0 { "medium" } else { "high" },
                "details": final_result.details
            }
        });

        Ok(ExtractionResult {
            text: final_result.text,
            confidence: 1.0, 
            metadata,
            tables: if final_result.tables.is_empty() { None } else { Some(final_result.tables) },
            entities: if final_result.entities.is_empty() { None } else { Some(final_result.entities) },
            diagnostics: if final_result.diagnostics.is_empty() { None } else { Some(serde_json::Value::Array(final_result.diagnostics)) },
        })
    }
}

impl WindowsNativeOcrAdapter {
    fn perform_reconstruction_pass(&self, all_words: &[WordInfo], config: &RuntimeConfig, img_width: f32, _img_height: f32) -> ReconstructionResult {
        let mut words: Vec<&WordInfo> = all_words.iter().collect();
        // 1b. Adaptive Classification Phase (Pass 1)
        // Group words into Rows based on Y-coordinate overlap
        // Sort by Y first
        words.sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap());

        let mut rows: Vec<Vec<&WordInfo>> = Vec::new();
        for word in words {
            let mut found_row = false;
            let word_mid_y = word.y + (word.h / 2.0);
            
            for row in rows.iter_mut().rev().take(config.extraction.layout_heuristics.row_lookback) { 
                if let Some(first_in_row) = row.first() {
                    let row_h = first_in_row.h;
                    let tolerance = row_h * config.extraction.layout_heuristics.row_overlap_tolerance; 
                    if (word_mid_y - (first_in_row.y + row_h / 2.0)).abs() < tolerance {
                        row.push(word);
                        found_row = true;
                        break;
                    }
                }
            }
            
            if !found_row {
                rows.push(vec![word]);
            }
        }

        // Estimate Density and Potential Gutters
        let first_avg_h = all_words.iter().map(|w| w.h).sum::<f32>() / (all_words.len() as f32).max(1.0);
        let mut x_starts: Vec<f32> = rows.iter().flat_map(|r| r.iter().take(1).map(|w| w.x)).collect();
        x_starts.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let mut potential_gutters = 0;
        if x_starts.len() > 1 {
            for i in 0..x_starts.len() - 1 {
                if x_starts[i+1] - x_starts[i] > first_avg_h * config.extraction.columns.gutter_gap_factor {
                    potential_gutters += 1;
                }
            }
        }

        let multi_word_rows = rows.iter().filter(|r| r.len() > 1).count();
        let density = if rows.is_empty() { 0.0 } else { multi_word_rows as f32 / rows.len() as f32 };
        
        // Initial Entropy Check 
        let raw_entropy = (potential_gutters as f32 * config.extraction.classifier.gutter_weight) + (density * config.extraction.classifier.density_weight);
        
        let doc_class = if potential_gutters >= 1 && density < config.extraction.classifier.multicolumn_density_max {
            DocClass::MultiColumn
        } else if density > config.extraction.classifier.table_density_min || raw_entropy > config.extraction.classifier.table_entropy_min {
            DocClass::FormsTables
        } else {
            DocClass::Plaintext
        };

        // Apply Adaptive Overrides
        let mut active_heuristics = config.extraction.layout_heuristics.clone();
        let active_tables = config.extraction.tables.clone();

        match doc_class {
            DocClass::Plaintext => {
                active_heuristics.significant_gap_gate = config.extraction.adaptive_overrides.plaintext_gap_gate;
                active_heuristics.cluster_threshold_factor = config.extraction.adaptive_overrides.plaintext_cluster_factor;
                active_heuristics.cross_zone_gap_factor = config.extraction.adaptive_overrides.plaintext_cross_factor;
            }
            DocClass::FormsTables => {
                active_heuristics.significant_gap_gate = config.extraction.adaptive_overrides.table_gap_gate;
                active_heuristics.cluster_threshold_factor = config.extraction.adaptive_overrides.table_cluster_factor;
                active_heuristics.cross_zone_gap_factor = config.extraction.adaptive_overrides.table_cross_factor;
            }
            DocClass::MultiColumn => {
                active_heuristics.significant_gap_gate = config.extraction.adaptive_overrides.column_gap_gate;
                active_heuristics.cluster_threshold_factor = config.extraction.adaptive_overrides.column_cluster_factor;
                active_heuristics.cross_zone_gap_factor = config.extraction.adaptive_overrides.column_cross_factor;
            }
        }

        // Re-sort rows by their starting Y coordinate (since we searched backwards)
        rows.sort_by(|a, b| {
            let ay = a.iter().map(|w| w.y).fold(f32::MAX, f32::min);
            let by = b.iter().map(|w| w.y).fold(f32::MAX, f32::min);
            ay.partial_cmp(&by).unwrap()
        });

        // 3. Global Margin Analysis & Column Discovery
        let global_margin_x = rows.iter()
            .filter(|r| r.len() > 0)
            .map(|r| r[0].x)
            .fold(f32::MAX, f32::min);

        let mut x_starts_all: Vec<f32> = rows.iter()
            .flat_map(|r| r.iter().take(1).map(|w| w.x))
            .collect();
        for row in &rows {
            if row.len() > 1 {
                let row_h = row[0].h;
                let mut last_x_end = row[0].x + row[0].w;
                for word in row.iter().skip(1) {
                    if word.x - last_x_end > row_h * config.extraction.layout_heuristics.bridged_threshold { 
                        x_starts_all.push(word.x);
                    }
                    last_x_end = word.x + word.w;
                }
            }
        }
        x_starts_all.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let mut potential_zones: Vec<ColumnZone> = Vec::new();
        if !x_starts_all.is_empty() {
            let cluster_threshold = first_avg_h * active_heuristics.cluster_threshold_factor;
            let mut current_min = x_starts_all[0];
            let mut current_max = x_starts_all[0];
            for &x in &x_starts_all[1..] {
                if x - current_max < cluster_threshold {
                    current_max = x;
                } else {
                    potential_zones.push(ColumnZone { min_x: current_min, max_x: current_max, char_index: 0 });
                    current_min = x;
                    current_max = x;
                }
            }
            potential_zones.push(ColumnZone { min_x: current_min, max_x: current_max, char_index: 0 });
        }

        // 4. Strong Consensus via Vertical Contiguity
        let mut column_zones: Vec<ColumnZone> = Vec::new();
        if doc_class == DocClass::Plaintext {
            column_zones.push(ColumnZone { min_x: 0.0, max_x: img_width, char_index: 0 });
        } else {
            for zone in potential_zones {
                let mut max_contiguous = 0;
                let mut current_contiguous = 0;
                for row in &rows {
                    if row.iter().any(|w| w.x >= zone.min_x - active_heuristics.zone_proximity && w.x <= zone.max_x + active_heuristics.zone_proximity) {
                        current_contiguous += 1;
                        if current_contiguous > max_contiguous { max_contiguous = current_contiguous; }
                    } else {
                        current_contiguous = 0;
                    }
                }
                if max_contiguous >= config.extraction.columns.min_contiguous_rows || zone.min_x < global_margin_x + config.extraction.columns.edge_margin_tolerance {
                    column_zones.push(zone);
                }
            }
        }

        let char_width = first_avg_h * config.extraction.layout_heuristics.char_width_factor;
        for zone in &mut column_zones {
            zone.char_index = ((zone.min_x - global_margin_x).max(0.0) / char_width) as usize;
        }

        // 4b. Layout Gutter Discovery
        let mut layout_gutters = Vec::new();
        if column_zones.len() > 1 {
            for i in 0..column_zones.len() - 1 {
                let left = &column_zones[i];
                let right = &column_zones[i+1];
                let gap = right.min_x - left.max_x;
                
                // If a gap is > 5x average height, it's a potential gutter
                if gap > first_avg_h * config.extraction.columns.gutter_gap_factor {
                    // Verify the void spans at least 70% of rows
                    let mut void_count = 0;
                    for row in &rows {
                        if !row.iter().any(|w| w.x > left.max_x + active_heuristics.zone_proximity && w.x < right.min_x - active_heuristics.zone_proximity) {
                            void_count += 1;
                        }
                    }
                    if void_count as f32 / rows.len() as f32 > config.extraction.columns.gutter_void_tolerance {
                        layout_gutters.push(left.max_x + gap / 2.0);
                    }
                }
            }
        }

        // 5. Row Segmentation (Adaptive Gap Gate)
        let mut processed_rows = Vec::with_capacity(rows.len());
        for row_words in rows {
            let mut segments: Vec<RowSegment> = Vec::new();
            let mut sorted_row = row_words;
            sorted_row.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
            if sorted_row.is_empty() { continue; }

            let mut expected_chars = 0;
            for w in &sorted_row { expected_chars += w.text.len() + 1; }

            let row_top = sorted_row.iter().map(|w| w.y).fold(f32::MAX, f32::min);
            let row_h = sorted_row.iter().map(|w| w.h).fold(0.0, f32::max);
            let row_start_x = sorted_row[0].x;
            let mut current_segment_text = String::with_capacity(expected_chars);
            let mut current_zone_idx = column_zones.iter().position(|z| row_start_x >= z.min_x - active_heuristics.zone_proximity && row_start_x <= z.max_x + active_heuristics.zone_proximity).unwrap_or(0);
            let mut last_x_end = -1.0;
            let mut segment_start_x = row_start_x;
            for word in &sorted_row {
                let zone_idx_opt = column_zones.iter().position(|z| word.x >= z.min_x - active_heuristics.zone_proximity && word.x <= z.max_x + active_heuristics.zone_proximity);
                if let Some(zone_idx) = zone_idx_opt {
                    let gap = word.x - last_x_end;
                    let is_cross_zone = zone_idx != current_zone_idx;
                    let split_threshold = if is_cross_zone { row_h * active_heuristics.cross_zone_gap_factor } else { row_h * active_heuristics.same_zone_gap_factor };
                    if !current_segment_text.is_empty() && is_cross_zone && gap > split_threshold {
                        segments.push(RowSegment { text: current_segment_text.trim().to_string(), zone_idx: current_zone_idx, start_x: segment_start_x, flags: Vec::new() });
                        current_segment_text = String::with_capacity(expected_chars);
                        current_zone_idx = zone_idx;
                        segment_start_x = word.x;
                    }
                }
                if last_x_end >= 0.0 && word.x - last_x_end > word.h * config.extraction.layout_heuristics.word_spacing_factor { current_segment_text.push(' '); }
                current_segment_text.push_str(&word.text);
                last_x_end = word.x + word.w;
            }
            if !current_segment_text.is_empty() {
                let mut row_flags = Vec::new();
                if segments.is_empty() && sorted_row.len() > 1 {
                    let total_w: f32 = sorted_row.iter().map(|w| w.w).sum();
                    if total_w < (last_x_end - row_start_x) * config.extraction.layout_heuristics.bridged_threshold { row_flags.push("bridged".to_string()); }
                }
                segments.push(RowSegment { text: current_segment_text.trim().to_string(), zone_idx: current_zone_idx, start_x: segment_start_x, flags: row_flags });
            }
            processed_rows.push(ProcessedRow { is_table_candidate: segments.len() >= 2, segments, y_top: row_top, h: row_h, row_x: row_start_x });
        }

        // 5b. Structural Confidence Scoring
        let mut word_confidences = Vec::new();
        for row in &processed_rows {
            for seg in &row.segments {
                let zone = &column_zones[seg.zone_idx];
                let x_jitter = (seg.start_x - zone.min_x).abs();
                let jitter_penalty = (x_jitter / first_avg_h).min(1.0) * config.extraction.scoring.jitter_penalty_weight;
                let text_len = seg.text.len() as f32;
                let expected_w = text_len * char_width;
                let size_dev = (expected_w - (seg.text.len() as f32 * char_width)).abs() / expected_w;
                let size_penalty = size_dev.min(1.0) * config.extraction.scoring.size_penalty_weight;
                let confidence = 1.0 - jitter_penalty - size_penalty;
                word_confidences.push(serde_json::json!({
                    "text": seg.text, "x": seg.start_x, "y": row.y_top, "w": seg.text.len() as f32 * char_width, "h": row.h,
                    "confidence": (confidence * 100.0).clamp(0.0, 100.0) as u32,
                    "type": if confidence < config.extraction.scoring.low_confidence_threshold { "low_confidence" } else { "normal" }
                }));
            }
        }

        // 6. Block-Based Consensus
        let mut row_types = vec![0; processed_rows.len()]; 
        for i in 0..processed_rows.len() {
            let row = &processed_rows[i];
            let row_width = row.segments.iter().map(|s| s.text.len() as f32 * char_width).sum::<f32>();
            let is_centered = (row.row_x + row_width / 2.0 - img_width / 2.0).abs() < (img_width * config.extraction.headers.centered_tolerance);
            let is_short = row_width < img_width * config.extraction.headers.max_width_ratio;
            if (row.h > first_avg_h * config.extraction.headers.h1_size_multiplier || (row.h > first_avg_h * config.extraction.headers.h2_size_multiplier && is_centered)) && is_short {
                row_types[i] = 3; continue;
            } else if row.h > first_avg_h * config.extraction.headers.h3_size_multiplier && is_centered && is_short {
                row_types[i] = 4; continue;
            }
            if row.is_table_candidate {
                let mut contiguous = 1;
                let mut j = i; while j > 0 && (processed_rows[j-1].is_table_candidate || row_types[j-1] == 2) { j -= 1; contiguous += 1; }
                let mut k = i; while k + 1 < processed_rows.len() && (processed_rows[k+1].is_table_candidate || row_types[k+1] == 2) { k += 1; contiguous += 1; }
                let total_segments: usize = (j..=k).map(|idx| processed_rows[idx].segments.len()).sum();
                let avg_segments = total_segments as f32 / (k - j + 1) as f32;
                if contiguous >= config.extraction.tables.min_contiguous_rows && avg_segments > config.extraction.tables.min_avg_segments {
                    row_types[i] = 1;
                    if i > 0 && row_types[i-1] == 0 {
                         let prev_row = &processed_rows[i-1];
                         if prev_row.segments.len() >= row.segments.len() { row_types[i-1] = 2; }
                    }
                }
            }
        }
        
        let mut diagnostics = Vec::new();
        let mut strong_pipe_zones = Vec::new();
        for (z_idx, _) in column_zones.iter().enumerate() {
            let mut pipe_like_count = 0;
            for (r_idx, row) in processed_rows.iter().enumerate() {
                if row_types[r_idx] == 0 { continue; }
                if let Some(seg) = row.segments.iter().find(|s| s.zone_idx == z_idx) {
                    if seg.text == "|" || seg.text == "l" || seg.text == "1" || seg.text == "I" || seg.text == "!" { pipe_like_count += 1; }
                }
            }
            if pipe_like_count >= 3 { strong_pipe_zones.push(z_idx); }
        }

        let mut final_text = String::new();
        let mut layout_blocks = Vec::new();
        if layout_gutters.is_empty() { layout_blocks.push((0.0, f32::MAX)); } 
        else {
            let mut last_x = 0.0;
            for &gx in &layout_gutters { layout_blocks.push((last_x, gx)); last_x = gx; }
            layout_blocks.push((last_x, f32::MAX));
        }

        let mut all_extracted_tables: Vec<TableBlock> = Vec::new();
        for (b_idx, &(b_min, b_max)) in layout_blocks.iter().enumerate() {
            if b_idx > 0 { final_text.push_str("\n\n"); }
            let mut last_y_end = -1.0;
            let mut in_active_table = false;
            let mut current_table_rows: Vec<Vec<String>> = Vec::new();
            let mut current_table_y_top = 0.0;
            let mut current_table_column_centers = Vec::new();
            let mut _current_table_idx = 0;

            for i in 0..processed_rows.len() {
                let full_row = &processed_rows[i];
                let row_segments: Vec<_> = full_row.segments.iter()
                    .filter(|s| { let z = &column_zones[s.zone_idx]; z.min_x >= b_min && z.max_x <= b_max }).collect();
                if row_segments.is_empty() { continue; }
                let row_type = row_types[i];
                if last_y_end >= 0.0 {
                    let gap = full_row.y_top - last_y_end;
                    let break_threshold = if in_active_table { config.extraction.layout_heuristics.table_break_threshold } else { active_heuristics.significant_gap_gate };
                    if gap > full_row.h * break_threshold { 
                        final_text.push_str("\n\n");
                        if in_active_table {
                            if !current_table_rows.is_empty() { 
                                let mut headers = Vec::new();
                                let body;
                                let mut footers = Vec::new();
                                if !current_table_rows.is_empty() {
                                    headers.push(current_table_rows.remove(0));
                                }
                                if !current_table_rows.is_empty() {
                                    let last_row = current_table_rows.last().unwrap();
                                    let is_footer = last_row.iter().any(|c| {
                                        let l = c.to_lowercase();
                                        l.contains("total") || l.contains("sum") || l.contains("subtotal") || l.contains("balance")
                                    });
                                    if is_footer {
                                        footers.push(current_table_rows.pop().unwrap());
                                    }
                                }
                                body = current_table_rows;

                                all_extracted_tables.push(TableBlock {
                                    y_top: current_table_y_top,
                                    y_bottom: last_y_end,
                                    column_centers: current_table_column_centers.clone(),
                                    headers,
                                    body,
                                    footers,
                                });
                                current_table_rows = Vec::new(); 

                            }
                            in_active_table = false;
                        }
                    } else if gap > config.extraction.layout_heuristics.paragraph_break_threshold { final_text.push('\n'); }
                }

                for seg in &full_row.segments {
                    if seg.flags.contains(&"bridged".to_string()) {
                        diagnostics.push(serde_json::json!({ "text": seg.text, "x": seg.start_x, "y": full_row.y_top, "w": seg.text.len() as f32 * char_width, "h": full_row.h, "type": "bridged" }));
                    }
                }

                if row_type == 1 || row_type == 2 {
                    let mut b_start = i; while b_start > 0 && (row_types[b_start-1] == 1 || row_types[b_start-1] == 2) { b_start -= 1; }
                    let mut b_end = i; while b_end + 1 < processed_rows.len() && (row_types[b_end+1] == 1 || row_types[b_end+1] == 2) { b_end += 1; }

                    let active_zones: Vec<usize> = (0..column_zones.len())
                        .filter(|&z_idx| (b_start..=b_end).any(|r_idx| processed_rows[r_idx].segments.iter().any(|s| s.zone_idx == z_idx))).collect();

                    if !in_active_table {
                        in_active_table = true;
                        current_table_y_top = full_row.y_top;
                        current_table_column_centers = active_zones.iter().map(|&z_idx| {
                            let z = &column_zones[z_idx];
                            (z.min_x + z.max_x) / 2.0
                        }).collect();
                        _current_table_idx = all_extracted_tables.len();
                        final_text.push_str(&format!("{{{{TABLE_BLOCK_{}}}}}", _current_table_idx));
                    }

                    let mut current_row_cells = Vec::new();
                    for &z_idx in &active_zones {
                        let mut cell_text = full_row.segments.iter().find(|s| s.zone_idx == z_idx).map(|s| s.text.clone()).unwrap_or_default();
                        if strong_pipe_zones.contains(&z_idx) && cell_text.len() == 1 {
                            let ch = cell_text.chars().next().unwrap();
                            if ch == 'l' || ch == 'I' || ch == '1' || ch == '!' {
                                if let Some(seg) = full_row.segments.iter().find(|s| s.zone_idx == z_idx) {
                                    diagnostics.push(serde_json::json!({ "text": cell_text.clone(), "x": seg.start_x, "y": full_row.y_top, "w": seg.text.len() as f32 * char_width, "h": full_row.h, "type": "coerced" }));
                                }
                                cell_text = "|".to_string();
                            }
                        }
                        current_row_cells.push(cell_text);
                    }
                    current_table_rows.push(current_row_cells);
                } else {
                    if in_active_table {
                        if !current_table_rows.is_empty() { 
                            let mut headers = Vec::new();
                            let body;
                            let mut footers = Vec::new();
                            if !current_table_rows.is_empty() {
                                headers.push(current_table_rows.remove(0));
                            }
                            if !current_table_rows.is_empty() {
                                let last_row = current_table_rows.last().unwrap();
                                let is_footer = last_row.iter().any(|c| {
                                    let l = c.to_lowercase();
                                    active_tables.footer_triggers.iter().any(|t| l.contains(t))
                                });
                                if is_footer {
                                    footers.push(current_table_rows.pop().unwrap());
                                }
                            }
                            body = current_table_rows;

                            all_extracted_tables.push(TableBlock {
                                y_top: current_table_y_top,
                                y_bottom: last_y_end,
                                column_centers: current_table_column_centers.clone(),
                                headers,
                                body,
                                footers,
                            });
                            current_table_rows = Vec::new(); 
                        }
                        in_active_table = false;
                    }
                    let block_margin = row_segments.iter().map(|s| s.start_x).fold(f32::MAX, f32::min);
                    let indent_chars = if row_type >= 3 { 0 } else { ((block_margin - b_min.max(global_margin_x)).max(0.0) / char_width) as usize };
                    for _ in 0..indent_chars { final_text.push(' '); }
                    let prefix = match row_type { 3 => "# ", 4 => "## ", _ => "" };
                    final_text.push_str(prefix);
                    let mut row_content = String::new();
                    for (idx, seg) in row_segments.iter().enumerate() {
                        if idx > 0 { 
                            let last_seg = &row_segments[idx-1];
                            let gap_px = seg.start_x - (last_seg.start_x + last_seg.text.len() as f32 * char_width);
                            let gap_chars = (gap_px.max(0.0) / char_width) as usize;
                            for _ in 0..gap_chars.clamp(1, config.extraction.layout_heuristics.max_space_clamp) { row_content.push(' '); }
                        }
                        row_content.push_str(&seg.text);
                    }
                    let mut cleaned = row_content;
                    if cleaned.starts_with('•') || cleaned.starts_with('o') || cleaned.starts_with('*') {
                        let inner: String = cleaned.chars().skip(1).collect(); cleaned = format!("- {}", inner.trim());
                    } else if (cleaned.starts_with('0') || cleaned.starts_with('O')) && cleaned.len() > 5 && cleaned.chars().nth(1).unwrap().is_alphabetic() {
                        let inner: String = cleaned.chars().skip(1).collect(); cleaned = inner.trim().to_string();
                    }
                    final_text.push_str(&cleaned);
                }
                last_y_end = full_row.y_top + full_row.h;
            }
            if in_active_table && !current_table_rows.is_empty() {
                let mut headers = Vec::new();
                let body;
                let mut footers = Vec::new();
                if !current_table_rows.is_empty() {
                    headers.push(current_table_rows.remove(0));
                }
                if !current_table_rows.is_empty() {
                    let last_row = current_table_rows.last().unwrap();
                    let is_footer = last_row.iter().any(|c| {
                        let l = c.to_lowercase();
                        active_tables.footer_triggers.iter().any(|t| l.contains(t))
                    });
                    if is_footer {
                        footers.push(current_table_rows.pop().unwrap());
                    }
                }
                body = current_table_rows;

                all_extracted_tables.push(TableBlock {
                    y_top: current_table_y_top,
                    y_bottom: last_y_end,
                    column_centers: current_table_column_centers.clone(),
                    headers,
                    body,
                    footers,
                });
            }
        }

        let original_tables = all_extracted_tables.clone();
        let all_extracted_tables = self.merge_tables(all_extracted_tables, config);

        // Replace placeholders with merged Markdown
        let mut final_repaired_text = final_text;
        for (_i, table) in all_extracted_tables.iter().enumerate() {
            let mut md_table = String::new();
            
            let mut all_rows = Vec::new();
            all_rows.extend(table.headers.clone());
            all_rows.extend(table.body.clone());
            all_rows.extend(table.footers.clone());

            if !all_rows.is_empty() {
                md_table.push_str("| ");
                md_table.push_str(&all_rows[0].join(" | "));
                md_table.push_str(" |\n|");
                for _ in 0..all_rows[0].len() { md_table.push_str("---|"); }
                md_table.push('\n');
                for row in all_rows.iter().skip(1) {
                    md_table.push_str("| "); md_table.push_str(&row.join(" | ")); md_table.push_str(" |\n");
                }
            }
            
            // Map merged table back to its first original placeholder
            if let Some(first_orig_idx) = original_tables.iter().position(|t| t.y_top == table.y_top) {
                let placeholder = format!("{{{{TABLE_BLOCK_{}}}}}", first_orig_idx);
                final_repaired_text = final_repaired_text.replace(&placeholder, &md_table);
            }
        }
        
        // Cleanup orphaned placeholders (for tables that were merged away)
        for i in 0..original_tables.len() {
            let placeholder = format!("{{{{TABLE_BLOCK_{}}}}}", i);
            final_repaired_text = final_repaired_text.replace(&placeholder, "");
        }

        let bridge_count = diagnostics.iter().filter(|d| d["type"] == "bridged").count();
        let coercion_count = diagnostics.iter().filter(|d| d["type"] == "coerced").count();
        let table_count = all_extracted_tables.len();
        let block_count = layout_blocks.len();
        let entropy = (bridge_count as f32 * 1.5) + (coercion_count as f32 * 1.0) + (block_count as f32 * 5.0) + (table_count as f32 * 10.0);

        let entities = self.detect_entities(&final_repaired_text);

        ReconstructionResult {
            text: final_repaired_text,
            tables: all_extracted_tables,
            entities,
            diagnostics,
            doc_class,
            entropy,
            details: serde_json::json!({ "bridges": bridge_count, "coercions": coercion_count, "tables": table_count, "blocks": block_count }),
        }
    }

    fn detect_entities(&self, text: &str) -> Vec<SemanticEntity> {
        let mut entities = Vec::new();
        
        let email_re = regex::Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap();
        let currency_re = regex::Regex::new(r"[$£€¥]\s?\d+(?:[.,]\d+)?").unwrap();
        let date_re = regex::Regex::new(r"\b(?:\d{4}[-/]\d{1,2}[-/]\d{1,2}|\d{1,2}[-/]\d{1,2}[-/]\d{2,4})\b").unwrap();

        for m in email_re.find_iter(text) {
            entities.push(SemanticEntity { entity_type: "Email".to_string(), value: m.as_str().to_string(), start_offset: m.start(), end_offset: m.end() });
        }
        for m in currency_re.find_iter(text) {
            entities.push(SemanticEntity { entity_type: "Currency".to_string(), value: m.as_str().to_string(), start_offset: m.start(), end_offset: m.end() });
        }
        for m in date_re.find_iter(text) {
            entities.push(SemanticEntity { entity_type: "Date".to_string(), value: m.as_str().to_string(), start_offset: m.start(), end_offset: m.end() });
        }

        entities.sort_by_key(|e| e.start_offset);
        entities
    }

    fn merge_tables(&self, tables: Vec<TableBlock>, config: &RuntimeConfig) -> Vec<TableBlock> {
        if tables.len() < 2 { return tables; }
        
        let mut merged = Vec::new();
        if tables.is_empty() { return merged; }
        let mut current = tables[0].clone();
        
        for next in tables.into_iter().skip(1) {
            let y_gap = next.y_top - current.y_bottom;
            
            // Check if column counts match
            let column_match = if current.column_centers.len() == next.column_centers.len() {
                let mut all_match = true;
                for (c1, c2) in current.column_centers.iter().zip(next.column_centers.iter()) {
                    if (*c1 - *c2).abs() > config.extraction.tables.column_jitter_tolerance { // e.g. 20px jitter tolerance
                        all_match = false;
                        break;
                    }
                }
                all_match
            } else {
                false
            };
            
            // Merge if vertically close OR column signature is identical
            if (y_gap < config.extraction.tables.merge_y_gap_max && column_match) || (y_gap < config.extraction.tables.merge_y_gap_min && current.column_centers.len() == next.column_centers.len()) {
                current.body.extend(current.footers);
                current.footers = Vec::new();
                current.body.extend(next.headers);
                current.body.extend(next.body);
                current.footers.extend(next.footers);
                current.y_bottom = next.y_bottom;
            } else {
                merged.push(current);
                current = next;
            }
        }
        merged.push(current);
        merged
    }
}

impl WindowsNativeOcrAdapter {
    /// Performs a post-processing pass to correct common OCR misinterpretations 
    /// (e.g. '|' read as 'l', '1', or 'I' in tables).
    fn _refine_extracted_text(&self, text: String, _rows: &[ProcessedRow], _zones: &[ColumnZone]) -> String {
        let mut lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
        let mut table_block_ranges = Vec::new();
        
        // Identify continuous table blocks
        let mut start = None;
        for (i, line) in lines.iter().enumerate() {
            if line.contains("|---|") || (line.starts_with("| ") && line.ends_with(" |")) {
                if start.is_none() { start = Some(i); }
            } else if start.is_some() {
                table_block_ranges.push(start.unwrap()..i);
                start = None;
            }
        }
        if let Some(s) = start { table_block_ranges.push(s..lines.len()); }

        for range in table_block_ranges {
            // Find "strong" column indices (where multiple rows have a pipe)
            let mut pipe_indices = Vec::new();
            for i in range.clone() {
                for (idx, ch) in lines[i].char_indices() {
                    if ch == '|' { pipe_indices.push(idx); }
                }
            }
            pipe_indices.sort();
            pipe_indices.dedup();

            // Coerce floating characters that align with pipes
            for i in range {
                let mut new_line = Vec::new();
                let chars: Vec<char> = lines[i].chars().collect();
                for (idx, &ch) in chars.iter().enumerate() {
                    let mut coerced = ch;
                    // If we see a character that looks like a pipe and it's NOT a pipe
                    if (ch == 'l' || ch == 'I' || ch == '1' || ch == '!') && !lines[i].contains("---") {
                        // Check if this position (or ±1) aligns with a known pipe in this block
                        // Use a rough estimate of character index mapping 
                        if pipe_indices.iter().any(|&p_idx| (p_idx as i32 - idx as i32).abs() <= 1) {
                             coerced = '|';
                        }
                    }
                    new_line.push(coerced);
                }
                lines[i] = new_line.into_iter().collect();
            }
        }

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_handle() {
        let adapter = WindowsNativeOcrAdapter::default();
        assert!(adapter.can_handle(ExtractionMimeType::Png));
        assert!(adapter.can_handle(ExtractionMimeType::Jpeg));
        assert!(!adapter.can_handle(ExtractionMimeType::Text));
        assert!(!adapter.can_handle(ExtractionMimeType::Pdf));
    }
}
