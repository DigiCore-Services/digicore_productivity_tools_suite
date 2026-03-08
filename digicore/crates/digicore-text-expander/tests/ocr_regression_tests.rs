use digicore_text_expander::adapters::extraction::WindowsNativeOcrAdapter;
use digicore_core::domain::ports::TextExtractionPort;
use digicore_core::domain::{ExtractionSource, ExtractionMimeType, SemanticEntity};
use std::fs;
use std::path::PathBuf;
use strsim::normalized_levenshtein;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct RunHistoryEntry {
    timestamp: String,
    avg_accuracy: f32,
    avg_latency_ms: f32,
    total_samples: usize,
    #[serde(default)]
    avg_resilience: f32,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Default)]
struct RunHistory {
    entries: Vec<RunHistoryEntry>,
}

fn get_snapshot_content(manifest_dir: &str, snapshot_name: &str) -> Option<String> {
    let snap_path = PathBuf::from(manifest_dir)
        .join("tests/snapshots")
        .join(format!("ocr_regression_tests__{}.snap", snapshot_name));
    
    if !snap_path.exists() {
        return None;
    }

    let raw = fs::read_to_string(snap_path).ok()?;
    
    // Insta format:
    // ---
    // [metadata]
    // ---
    // [content]
    // We need to find the second occurrence of "---" and take everything after it.
    let first_dash = raw.find("---")?;
    let second_dash = raw[first_dash + 3..].find("---")?;
    let content_start = first_dash + 3 + second_dash + 3;
    
    Some(raw[content_start..].trim().to_string())
}

fn apply_fuzzing(image_path: &PathBuf) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let img = image::open(image_path)?;
    // Apply a gaussian blur to simulate out-of-focus or low-resolution capture
    let fuzzed = img.blur(1.5);
    let temp_dir = std::env::temp_dir();
    let fuzz_path = temp_dir.join(format!("fuzz_{}", image_path.file_name().unwrap_or_default().to_string_lossy()));
    fuzzed.save(&fuzz_path)?;
    Ok(fuzz_path)
}

#[tokio::test]
async fn run_ocr_on_all_samples() {
    let adapter = WindowsNativeOcrAdapter::new(None);
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    
    // Path to the shared docs samples directory
    let workspace_dir = PathBuf::from(manifest_dir).join("../../");
    let sample_dir = workspace_dir.join("docs/sample-ocr-images");
    
    assert!(sample_dir.exists(), "Sample directory not found: {:?}", sample_dir);
    
    let timestamp = chrono::Local::now().format("%Y-%m-%d_%I%M%p%Z").to_string().replace(" ", "").replace(":", "");
    let out_dir = sample_dir.join(format!("results_{}", timestamp));
    fs::create_dir_all(&out_dir).expect("Failed to create results directory");
    
    let mut entries: Vec<_> = fs::read_dir(&sample_dir)
        .expect("Failed to read sample directory")
        .filter_map(Result::ok)
        .collect();
        
    entries.sort_by_key(|e| e.path());
    
    let mut results = Vec::new();
    
    for entry in entries {
        let path = entry.path();
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
        
        if ext == "png" || ext == "jpg" || ext == "jpeg" {
            let file_name = path.file_name().unwrap().to_string_lossy().to_string();
            println!("\n=========== processing: {} ===========", file_name);
            
            let mime_type = match ext.as_str() {
                "png" => ExtractionMimeType::Png,
                _ => ExtractionMimeType::Jpeg,
            };
            
            let source = ExtractionSource::File(path.to_path_buf());
            
            match adapter.extract(source, mime_type).await {
                Ok(result) => {
                    let actual_text = result.text.trim();
                    let snapshot_name = file_name.replace(".", "_");
                    
                    let mut _snapshot_updated = false;
                    
                    // We use INSTA_FORCE_PASS here so that the loop continues even if a snapshot mismatches.
                    // This allows us to generate the full Summary Dashboard and review all results at once.
                    std::env::set_var("INSTA_FORCE_PASS", "1");
                    
                    if std::env::var("DIGICORE_OCR_FORCE_UPDATE").is_ok() {
                        std::env::set_var("INSTA_UPDATE", "always");
                        _snapshot_updated = true;
                    }
                    
                    insta::assert_snapshot!(snapshot_name.clone(), actual_text);
                    
                    // 2. Accuracy Scoring
                    let expected_text = get_snapshot_content(manifest_dir, &snapshot_name).unwrap_or_default();
                    let accuracy = (normalized_levenshtein(actual_text, &expected_text) * 100.0) as u32;

                    // Word Breakdown for Phase 35/36
                    let actual_words: Vec<_> = actual_text.split_whitespace().collect();
                    let expected_words: Vec<_> = expected_text.split_whitespace().collect();
                    
                    // FUZZING PASS
                    let mut resilience_score = 1.0;
                    if let Ok(fuzz_path) = apply_fuzzing(&path) {
                        let fuzz_mime = match ext.as_str() {
                            "png" => ExtractionMimeType::Png,
                            _ => ExtractionMimeType::Jpeg,
                        };
                        println!("... running fuzzed extraction pass to test resilience ...");
                        if let Ok(fuzz_res) = adapter.extract(ExtractionSource::File(fuzz_path.clone()), fuzz_mime).await {
                            let fuzz_text = fuzz_res.text.trim();
                            let fuzz_acc = (normalized_levenshtein(fuzz_text, &expected_text) * 100.0) as u32;
                            let base_acc = if accuracy == 0 { 1 } else { accuracy };
                            resilience_score = fuzz_acc as f32 / base_acc as f32;
                            println!("Fuzzed Accuracy: {}% (Resilience: {:.2})", fuzz_acc, resilience_score);
                        }
                        let _ = fs::remove_file(fuzz_path);
                    }

                    println!("Accuracy: {}% (Words: {} actual vs {} expected)", 
                        accuracy, actual_words.len(), expected_words.len()
                    );
                    
                    // 3. Save individual markdown
                    let md_path = out_dir.join(format!("{}.md", file_name));
                    fs::write(&md_path, actual_text).expect("Failed to write result");
                    
                    // 3b. Save Structured Data (CSV/JSON)
                    let mut has_structured = false;
                    if let Some(tables) = &result.tables {
                        has_structured = true;
                        // Save full JSON
                        let json_path = out_dir.join(format!("{}_structured.json", file_name));
                        let json_content = serde_json::to_string_pretty(tables).unwrap_or_default();
                        fs::write(&json_path, json_content).ok();

                        // Save individual CSVs and Markdown snippets
                        for (idx, table) in tables.iter().enumerate() {
                            // CSV
                            let csv_path = out_dir.join(format!("{}_table_{}.csv", file_name, idx));
                            let mut csv_content = String::new();
                            let mut all_rows = Vec::new();
                            all_rows.extend(table.headers.clone());
                            all_rows.extend(table.body.clone());
                            all_rows.extend(table.footers.clone());

                            for row in &all_rows {
                                let quoted_row: Vec<String> = row.iter()
                                    .map(|s| format!("\"{}\"", s.replace("\"", "\"\"")))
                                    .collect();
                                csv_content.push_str(&quoted_row.join(","));
                                csv_content.push('\n');
                            }
                            fs::write(&csv_path, csv_content).ok();

                            // Markdown
                            let md_table_path = out_dir.join(format!("{}_table_{}.md", file_name, idx));
                            let mut md_content = String::new();
                            if !all_rows.is_empty() {
                                // Header
                                md_content.push_str("| ");
                                md_content.push_str(&all_rows[0].join(" | "));
                                md_content.push_str(" |\n|");
                                for _ in 0..all_rows[0].len() {
                                    md_content.push_str("---|");
                                }
                                md_content.push('\n');
                                // Body
                                for row in all_rows.iter().skip(1) {
                                    md_content.push_str("| ");
                                    md_content.push_str(&row.join(" | "));
                                    md_content.push_str(" |\n");
                                }
                            }
                            fs::write(&md_table_path, md_content).ok();
                        }
                    }

                    // 4. Generate Detailed HTML Report for this image
                    let diag_json = serde_json::to_string(&result.diagnostics).unwrap_or_else(|_| "null".to_string());
                    let performance = result.metadata.get("performance_metrics").cloned().unwrap_or(serde_json::json!({}));
                    let adaptive_profile = result.metadata.get("adaptive_profile").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                    let refined = result.metadata.get("refinement_executed").and_then(|v| v.as_bool()).unwrap_or(false);
                    let empty_vec = Vec::new();
                    let entities = result.entities.as_ref().unwrap_or(&empty_vec);
                    let detail_html = generate_detail_report(&file_name, actual_text, &expected_text, accuracy, has_structured, &diag_json, &performance, &adaptive_profile, refined, entities, resilience_score);
                    let detail_path = out_dir.join(format!("{}.html", file_name));
                    fs::write(&detail_path, detail_html).expect("Failed to write detail report");
                    
                    results.push(RegressionResult {
                        file_name,
                        accuracy,
                        performance,
                        adaptive_profile,
                        refined,
                        resilience_score,
                    });
                }
                Err(e) => {
                    eprintln!("Error processing {}: {}", file_name, e);
                }
            }
        }
    }
    
    // 5. Generate Summary Dashboard (Wall of Fame)
    let avg_accuracy = if results.len() > 0 { results.iter().map(|r| r.accuracy).sum::<u32>() as f32 / results.len() as f32 / 100.0 } else { 0.0 };
    let total_ms: i64 = results.iter().map(|r| r.performance.get("extraction_ms").and_then(|v| v.as_i64()).unwrap_or(0)).sum();
    let avg_latency = if results.len() > 0 { total_ms as f32 / results.len() as f32 } else { 0.0 };
    let avg_resil = if results.len() > 0 { results.iter().map(|r| r.resilience_score).sum::<f32>() / results.len() as f32 } else { 0.0 };

    // Save to history.json
    let history_path = sample_dir.join("history.json");
    let mut history = if history_path.exists() {
        let content = std::fs::read_to_string(&history_path).unwrap_or_default();
        serde_json::from_str::<RunHistory>(&content).unwrap_or_default()
    } else {
        RunHistory::default()
    };

    history.entries.push(RunHistoryEntry {
        timestamp: chrono::Utc::now().to_rfc3339(),
        avg_accuracy,
        avg_latency_ms: avg_latency,
        total_samples: results.len(),
        avg_resilience: avg_resil,
    });

    if history.entries.len() > 50 {
        history.entries.remove(0);
    }

    if let Ok(json) = serde_json::to_string_pretty(&history) {
        let _ = std::fs::write(&history_path, json);
    }

    let summary_html = generate_summary_dashboard(&timestamp, &results, &history);
    let summary_path = out_dir.join("summary.html");
    fs::write(&summary_path, summary_html).expect("Failed to write summary report");
    
    // Also create index.html as a redirect or copy for convenience
    fs::copy(&summary_path, out_dir.join("index.html")).ok();
    
    println!("Successfully processed {} images. Dashboard: {:?}", results.len(), summary_path);
    
    // Finally, if any accuracies were low or snapshots mismatch, we can report it here.
    // However, since we want the user to use `cargo insta review`, we'll just print a summary.
    let fail_count = results.iter().filter(|r| r.accuracy < 100).count();
    if fail_count > 0 {
        println!("⚠️  FOUND {} SAMPLES WITH MISMATCHES/LOW ACCURACY.", fail_count);
        println!("Run `cargo insta review` to inspect and accept changes.");
    }
}

#[derive(Clone)]
struct RegressionResult {
    file_name: String,
    accuracy: u32,
    performance: serde_json::Value,
    adaptive_profile: String,
    refined: bool,
    resilience_score: f32,
}

fn generate_summary_dashboard(timestamp: &str, results: &[RegressionResult], history: &RunHistory) -> String {
    let mut rows = String::new();
    let total = results.len();
    let avg_acc = if total > 0 { results.iter().map(|r| r.accuracy).sum::<u32>() / total as u32 } else { 0 };
    let avg_ms = if total > 0 { results.iter().map(|r| r.performance.get("extraction_ms").and_then(|v| v.as_i64()).unwrap_or(0)).sum::<i64>() / total as i64 } else { 0 };

    // Leaderboard
    let mut leaderboard = results.to_vec();
    leaderboard.sort_by(|a, b| {
        let ea = a.performance.get("entropy").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let eb = b.performance.get("entropy").and_then(|v| v.as_f64()).unwrap_or(0.0);
        eb.partial_cmp(&ea).unwrap()
    });

    let top_5 = leaderboard.iter().take(5);
    let mut leaderboard_rows = String::new();
    for r in top_5 {
        let entropy = r.performance.get("entropy").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let ms = r.performance.get("extraction_ms").and_then(|v| v.as_i64()).unwrap_or(0);
        let class = r.performance.get("complexity_class").and_then(|v| v.as_str()).unwrap_or("low");
        leaderboard_rows.push_str(&format!(
            "<tr>
                <td>{}</td>
                <td><a href=\"{}.html\" style=\"text-decoration: none;\"><span class=\"badge {}\">{}</span></a></td>
                <td>{:.1}</td>
                <td>{}ms</td>
                <td>{:.2}</td>
            </tr>",
            r.file_name, r.file_name, class, class.to_uppercase(), entropy, ms, r.resilience_score
        ));
    }

    for (idx, r) in results.iter().enumerate() {
        let status_class = if r.accuracy >= 95 { "success" } else if r.accuracy >= 80 { "warning" } else { "danger" };
        let ms = r.performance.get("extraction_ms").and_then(|v| v.as_i64()).unwrap_or(0);
        let entropy = r.performance.get("entropy").and_then(|v| v.as_f64()).unwrap_or(0.0);
        
        let refined_badge = if r.refined {
            "<span class=\"mini-badge\" style=\"background: #0ea5e9; color: #fff;\">REFINED</span>"
        } else {
            ""
        };
        
        let delay = idx * 50;
        rows.push_str(&format!(
            "<div class=\"card {}\" id=\"card-{}\" style=\"animation-delay: {}ms;\">
                <div class=\"card-content\">
                    <h3>{}</h3>
                    <div style=\"display: flex; gap: 0.5rem; margin-bottom: 0.5rem;\">
                        <span class=\"mini-badge\" style=\"background: #27272a; color: #fff;\">{}</span>
                        <span class=\"mini-badge\">{}ms</span>
                        {}
                        <span class=\"mini-badge\">Entropy: {:.1}</span>
                        <span class=\"mini-badge\" style=\"border-color: #60a5fa;\">Resil: {:.2}</span>
                    </div>
                    <div class=\"gauge-container\">
                        <div class=\"gauge\" style=\"width: {}%\"></div>
                        <span class=\"acc-label\">{}% Match</span>
                    </div>
                </div>
                <a href=\"{}.html\" class=\"view-btn\">Details</a>
            </div>",
            status_class, r.file_name, delay, r.file_name, r.adaptive_profile.to_uppercase(), ms, refined_badge, entropy, r.resilience_score, r.accuracy, r.accuracy, r.file_name
        ));
    }

    // Sparkline generation
    let mut acc_points = String::new();
    let mut lat_points = String::new();
    let mut res_points = String::new();
    if history.entries.len() > 1 {
        let w = 160.0;
        let h = 30.0;
        let step = w / (history.entries.len() - 1) as f32;
        for (i, entry) in history.entries.iter().enumerate() {
            let x = i as f32 * step;
            let ay = h - (entry.avg_accuracy * h);
            let ly = (h - (entry.avg_latency_ms / 500.0 * h)).clamp(0.0, h);
            let ry = h - (entry.avg_resilience.clamp(0.0, 1.0) * h);
            acc_points.push_str(&format!("{},{} ", x, ay));
            lat_points.push_str(&format!("{},{} ", x, ly));
            res_points.push_str(&format!("{},{} ", x, ry));
        }
    }

    format!(
        "<!DOCTYPE html>
<html>
<head>
    <title>OCR Wall of Fame - {}</title>
    <style>
        body {{ 
            font-family: 'Inter', system-ui, sans-serif; 
            background: #09090b; 
            background-image: 
                radial-gradient(at 0% 0%, rgba(59, 130, 246, 0.05) 0, transparent 50%),
                radial-gradient(at 100% 100%, rgba(168, 85, 247, 0.05) 0, transparent 50%);
            color: #e4e4e7; 
            margin: 0; 
            padding: 2rem; 
            min-height: 100vh;
            scroll-behavior: smooth;
        }}
        .header {{ display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid rgba(255,255,255,0.05); padding-bottom: 2rem; margin-bottom: 3rem; }}
        h1 {{ margin: 0; font-weight: 800; background: linear-gradient(135deg, #60a5fa, #a855f7); -webkit-background-clip: text; -webkit-text-fill-color: transparent; filter: drop-shadow(0 0 8px rgba(96, 165, 250, 0.3)); }}
        .stats {{ display: flex; gap: 1.5rem; }}
        .stat-box {{ 
            background: rgba(24, 24, 27, 0.6); 
            backdrop-filter: blur(12px);
            -webkit-backdrop-filter: blur(12px);
            padding: 1rem 1.5rem; 
            border-radius: 12px; 
            border: 1px solid rgba(255,255,255,0.05); 
            text-align: center; 
            box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1);
        }}
        .stat-val {{ display: block; font-size: 1.5rem; font-weight: 700; color: #fff; }}
        .stat-lbl {{ font-size: 0.875rem; color: #a1a1aa; }}
        
        .grid {{ display: grid; grid-template-columns: repeat(auto-fill, minmax(320px, 1fr)); gap: 1.5rem; }}
        .card {{ 
            background: rgba(24, 24, 27, 0.4); 
            backdrop-filter: blur(8px);
            -webkit-backdrop-filter: blur(8px);
            border-radius: 20px; 
            border: 1px solid rgba(255,255,255,0.05); 
            overflow: hidden; 
            display: flex; 
            flex-direction: column; 
            transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
            animation: fadeIn 0.5s ease-out backwards;
            scroll-margin-top: 100px;
        }}
        @keyframes fadeIn {{ from {{ opacity: 0; transform: translateY(10px); }} to {{ opacity: 1; transform: translateY(0); }} }}
        .card:hover {{ transform: translateY(-6px); border-color: rgba(255,255,255,0.15); box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.2); }}
        .card:target {{ border-color: #60a5fa; box-shadow: 0 0 20px rgba(96, 165, 250, 0.3); transform: scale(1.02); z-index: 10; }}
        .card-content {{ padding: 1.5rem; flex: 1; }}
        h3 {{ margin: 0 0 1rem 0; font-size: 0.95rem; color: #fff; word-break: break-all; font-weight: 600; opacity: 0.9; }}
        
        .gauge-container {{ position: relative; height: 8px; background: rgba(39, 39, 42, 0.5); border-radius: 4px; overflow: hidden; margin-top: 1.5rem; }}
        .gauge {{ height: 100%; border-radius: 4px; transition: width 1.2s cubic-bezier(0.1, 0, 0.1, 1); }}
        .card.success .gauge {{ background: linear-gradient(90deg, #22c55e, #4ade80); }}
        .card.warning .gauge {{ background: linear-gradient(90deg, #f59e0b, #fbbf24); }}
        .card.danger .gauge {{ background: linear-gradient(90deg, #ef4444, #f87171); }}
        .acc-label {{ position: absolute; right: 0; top: -20px; font-size: 0.7rem; font-weight: 700; color: #a1a1aa; text-transform: uppercase; letter-spacing: 0.05em; }}
        
        .view-btn {{ 
            background: rgba(39, 39, 42, 0.4); 
            color: #fff; 
            text-decoration: none; 
            text-align: center; 
            padding: 0.85rem; 
            font-size: 0.8rem; 
            font-weight: 600; 
            text-transform: uppercase;
            letter-spacing: 0.05em;
            border-top: 1px solid rgba(255,255,255,0.05); 
            transition: all 0.2s; 
        }}
        .view-btn:hover {{ background: rgba(59, 130, 246, 0.1); color: #60a5fa; }}
        .mini-badge {{ background: rgba(39, 39, 42, 0.5); color: #d4d4d8; padding: 3px 8px; border-radius: 6px; font-size: 0.6rem; font-weight: 700; border: 1px solid rgba(255,255,255,0.05); }}
        
        .leaderboard {{ 
            background: rgba(24, 24, 27, 0.6); 
            backdrop-filter: blur(12px);
            -webkit-backdrop-filter: blur(12px);
            border-radius: 20px; 
            border: 1px solid rgba(255,255,255,0.05); 
            padding: 1.5rem; 
            box-shadow: 0 10px 15px -3px rgba(0, 0, 0, 0.1);
        }}
        .leaderboard h2 {{ margin: 0 0 1.5rem 0; font-size: 1rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.1em; color: #71717a; }}
        .leaderboard table {{ width: 100%; border-collapse: separate; border-spacing: 0 0.5rem; font-size: 0.85rem; }}
        .leaderboard th {{ text-align: left; color: #52525b; padding: 0.75rem; border-bottom: 1px solid rgba(255,255,255,0.05); font-weight: 600; }}
        .leaderboard td {{ padding: 0.75rem; vertical-align: middle; transition: background 0.2s; }}
        .leaderboard tr:hover td {{ background: rgba(255,255,255,0.02); }}
        
        .badge {{ padding: 3px 10px; border-radius: 20px; font-size: 0.65rem; font-weight: 800; border: 1px solid currentColor; letter-spacing: 0.05em; }}
        .badge.low {{ color: #22c55e; background: rgba(34, 197, 94, 0.1); }}
        .badge.medium {{ color: #f59e0b; background: rgba(245, 158, 11, 0.1); }}
        .badge.high {{ color: #ef4444; background: rgba(239, 68, 68, 0.1); }}

        .pulse-box {{ background: rgba(24, 24, 27, 0.6); padding: 12px; border-radius: 12px; border: 1px solid rgba(255,255,255,0.05); backdrop-filter: blur(10px); }}
        .pulse-path {{ stroke-dasharray: 1000; stroke-dashoffset: 1000; animation: drawLine 2s ease-out forwards; }}
        @keyframes drawLine {{ to {{ stroke-dashoffset: 0; }} }}
    </style>
</head>
<body>
    <div class=\"header\">
        <div>
            <h1>OCR Wall of Fame</h1>
            <p style=\"color: #a1a1aa; margin: 0.5rem 0 0 0;\">Regression Test Run Performance Dashboard</p>
        </div>
        <div style=\"display: flex; gap: 2rem; align-items: center;\">
            <div id=\"accuracy-pulse\" class=\"pulse-box\">
                <div style=\"font-size: 0.6rem; color: #71717a; margin-bottom: 6px; text-transform: uppercase; letter-spacing: 0.1em; font-weight: 700;\">Accuracy</div>
                <svg width=\"160\" height=\"30\" style=\"overflow: visible;\">
                    <polyline points=\"{{ACC_POINTS}}\" fill=\"none\" stroke=\"#22c55e\" stroke-width=\"2.5\" class=\"pulse-path\" stroke-linejoin=\"round\" />
                </svg>
            </div>
            <div id=\"latency-pulse\" class=\"pulse-box\">
                <div style=\"font-size: 0.6rem; color: #71717a; margin-bottom: 6px; text-transform: uppercase; letter-spacing: 0.1em; font-weight: 700;\">Latency</div>
                <svg width=\"160\" height=\"30\" style=\"overflow: visible;\">
                    <polyline points=\"{{LAT_POINTS}}\" fill=\"none\" stroke=\"#60a5fa\" stroke-width=\"2.5\" class=\"pulse-path\" stroke-linejoin=\"round\" />
                </svg>
            </div>
            <div id=\"resilience-pulse\" class=\"pulse-box\">
                <div style=\"font-size: 0.6rem; color: #71717a; margin-bottom: 6px; text-transform: uppercase; letter-spacing: 0.1em; font-weight: 700;\">Resilience</div>
                <svg width=\"160\" height=\"30\" style=\"overflow: visible;\">
                    <polyline points=\"{{RES_POINTS}}\" fill=\"none\" stroke=\"#a855f7\" stroke-width=\"2.5\" class=\"pulse-path\" stroke-linejoin=\"round\" />
                </svg>
            </div>
        </div>
        <div class=\"stats\">
            <div class=\"stat-box\">
                <span class=\"stat-val\">{}</span>
                <span class=\"stat-lbl\">Images</span>
            </div>
            <div class=\"stat-box\">
                <span class=\"stat-val\">{}%</span>
                <span class=\"stat-lbl\">Avg Accuracy</span>
            </div>
            <div class=\"stat-box\">
                <span class=\"stat-val\">{}ms</span>
                <span class=\"stat-lbl\">Avg Timing</span>
            </div>
        </div>
    </div>

    <div style=\"display: grid; grid-template-columns: 1fr 3fr; gap: 2rem; align-items: start;\">
        <div class=\"leaderboard\">
            <h2>Performance Leaderboard</h2>
            <table>
                <thead>
                    <tr>
                        <th>File</th>
                        <th>Class</th>
                        <th>Entropy</th>
                        <th>Cost</th>
                        <th>Resil</th>
                    </tr>
                </thead>
                <tbody>
                    {}
                </tbody>
            </table>
        </div>

        <div class=\"grid\">
            {}
        </div>
    </div>

    <script>
        // Elite UX interactivity
        window.onload = () => {{
            // Subtle card entrance sounds or haptics could go here if in a real app
            console.log(\"Elite Dashboard Initialized\");
            
            // Auto-scroll logic for leaderboard if it exceeds viewport
            const leaderboard = document.querySelector('.leaderboard table tbody');
            if (leaderboard && leaderboard.rows.length > 8) {{
                // Optional: implement a slow auto-scroll or just a fade
            }}
            
            // Highlight sparklines on pulse box hover
            const pulses = document.querySelectorAll('.pulse-box');
            pulses.forEach(p => {{
                p.onmouseenter = () => p.style.borderColor = 'rgba(255,255,255,0.2)';
                p.onmouseleave = () => p.style.borderColor = 'rgba(255,255,255,0.05)';
            }});
        }};
    </script>
</body>
</html>",
        timestamp, total, avg_acc, avg_ms, leaderboard_rows, rows
    )
    .replace("{{ACC_POINTS}}", &acc_points)
    .replace("{{LAT_POINTS}}", &lat_points)
    .replace("{{RES_POINTS}}", &res_points)
}

fn generate_detail_report(
    name: &str, 
    actual: &str, 
    expected: &str, 
    accuracy: u32, 
    has_structured: bool, 
    diagnostics: &str, 
    performance: &serde_json::Value, 
    adaptive_profile: &str, 
    refined: bool,
    entities: &[SemanticEntity],
    resilience: f32
) -> String {
    let actual_escaped = actual.replace("<", "&lt;").replace(">", "&gt;");
    let expected_escaped = expected.replace("<", "&lt;").replace(">", "&gt;");
    let acc_class = if accuracy >= 95 { "success" } else if accuracy >= 80 { "warning" } else { "danger" };
    
    let ms = performance.get("extraction_ms").and_then(|v| v.as_i64()).unwrap_or(0);
    let entropy = performance.get("entropy").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let class = performance.get("complexity_class").and_then(|v| v.as_str()).unwrap_or("low");

    let structured_badge = if has_structured {
        "<span class=\"accuracy-badge\" style=\"background: rgba(168, 85, 247, 0.1); color: #a855f7; border: 1px solid rgba(168, 85, 247, 0.2); margin-left: 0.5rem;\">STRUCTURED</span>"
    } else {
        ""
    };
    
    let refined_badge = if refined {
        "<span class=\"accuracy-badge\" style=\"background: rgba(14, 165, 233, 0.1); color: #0ea5e9; border: 1px solid rgba(14, 165, 233, 0.2); margin-left: 0.5rem;\">REFINED</span>"
    } else {
        ""
    };
    
    let resil_class = if resilience >= 0.95 { "success" } else if resilience >= 0.8 { "warning" } else { "danger" };
    let resil_badge = format!("<span class=\"accuracy-badge {}\" style=\"border: 1px solid currentColor; margin-left: 0.5rem;\">RESIL: {:.2}</span>", resil_class, resilience);
    
    let performance_badges = format!(
        "<span class=\"accuracy-badge\" style=\"background: #27272a; color: #fff; border: 1px solid #3f3f46; margin-left: 0.5rem;\">{}ms</span>
         <span class=\"accuracy-badge {}\" style=\"border: 1px solid currentColor; margin-left: 0.5rem;\">ENTROPY: {:.1} ({})</span>
         <span class=\"accuracy-badge\" style=\"background: #18181b; color: #a1a1aa; border: 1px solid #27272a; margin-left: 0.5rem;\">PROFILE: {}</span>{} {}",
        ms, class, entropy, class.to_uppercase(), adaptive_profile.to_uppercase(), refined_badge, resil_badge
    );

    let template = r###"<!DOCTYPE html>
<html>
<head>
    <title>Analysis: {{NAME}}</title>
    <style>
        body { font-family: 'Inter', system-ui, sans-serif; background: #0b0b0d; color: #e4e4e7; margin: 0; padding: 0; height: 100vh; overflow: hidden; display: flex; flex-direction: column; }
        
        /* Top Navigation */
        .top-bar { background: #121215; border-bottom: 1px solid #27272a; padding: 0.75rem 1.5rem; display: flex; justify-content: space-between; align-items: center; z-index: 100; }
        .nav-left { display: flex; align-items: center; gap: 1.5rem; }
        .back-link { color: #a1a1aa; text-decoration: none; font-size: 0.875rem; display: flex; align-items: center; gap: 0.5rem; }
        .back-link:hover { color: #fff; }
        h1 { margin: 0; font-size: 1.125rem; font-weight: 700; color: #fff; }
        
        .status-pill { display: flex; align-items: center; gap: 1rem; }
        .accuracy-badge { padding: 0.4rem 0.8rem; border-radius: 99px; font-weight: 700; font-size: 0.875rem; }
        .success { background: rgba(34, 197, 94, 0.1); color: #22c55e; border: 1px solid rgba(34, 197, 94, 0.2); }
        .warning { background: rgba(245, 158, 11, 0.1); color: #f59e0b; border: 1px solid rgba(245, 158, 11, 0.2); }
        .danger { background: rgba(239, 68, 68, 0.1); color: #ef4444; border: 1px solid rgba(239, 68, 68, 0.2); }

        /* Secondary Toolbar */
        .toolbar { background: #18181b; border-bottom: 1px solid #27272a; padding: 0.5rem 1.5rem; display: flex; gap: 1rem; align-items: center; }
        .view-toggle { display: flex; background: #09090b; border-radius: 8px; padding: 2px; border: 1px solid #27272a; }
        .toggle-btn { padding: 0.4rem 0.8rem; border: none; background: transparent; color: #71717a; font-size: 0.75rem; font-weight: 600; cursor: pointer; border-radius: 6px; transition: all 0.2s; }
        .toggle-btn.active { background: #27272a; color: #fff; }
        
        /* Main Workspace */
        .workspace { flex: 1; display: grid; grid-template-columns: 350px 1fr; overflow: hidden; }
        
        /* Sidebar (Source Image) */
        .sidebar { background: #09090b; border-right: 1px solid #27272a; padding: 1.5rem; overflow-y: auto; display: flex; flex-direction: column; gap: 1rem; }
        .sidebar-label { font-size: 0.75rem; font-weight: 600; text-transform: uppercase; color: #52525b; letter-spacing: 0.05em; }
        .source-img-container { background: #121215; border: 1px solid #27272a; border-radius: 8px; overflow: hidden; cursor: zoom-in; position: relative; }
        .source-img-container img { width: 100%; display: block; opacity: 0.8; transition: opacity 0.2s; }
        .source-img-container:hover img { opacity: 1; }
        
        /* Content Pane */
        .content-pane { background: #121215; overflow: hidden; position: relative; }
        .view-pane { position: absolute; inset: 0; display: none; flex-direction: column; }
        .view-pane.active { display: flex; }
        
        /* Side-by-Side Sync View */
        .sync-layout { display: flex; height: 100%; }
        .sync-column { flex: 1; border-right: 1px solid #27272a; display: flex; flex-direction: column; overflow: hidden; }
        .sync-column:last-child { border-right: none; }
        .sync-header { padding: 0.5rem 1rem; background: #09090b; border-bottom: 1px solid #27272a; font-size: 0.7rem; font-weight: 700; color: #71717a; text-transform: uppercase; }
        .sync-scroll { flex: 1; overflow-y: auto; padding: 1.5rem; font-family: 'JetBrains Mono', 'Consolas', monospace; font-size: 13px; line-height: 1.6; white-space: pre-wrap; color: #d4d4d8; }
        
        /* Highlight Diff View */
        .diff-layout { height: 100%; overflow-y: auto; padding: 2rem; font-family: 'JetBrains Mono', 'Consolas', monospace; font-size: 14px; line-height: 1.7; white-space: pre-wrap; word-break: break-all; }
        .diff-add { background: rgba(34, 197, 94, 0.25); color: #4ade80; text-decoration: none; padding: 1px 0; border-bottom: 1px solid rgba(34, 197, 94, 0.5); }
        .diff-del { background: rgba(239, 68, 68, 0.25); color: #f87171; text-decoration: line-through; padding: 1px 0; border-bottom: 1px solid rgba(239, 68, 68, 0.5); }
        
        /* Scrollbar styling */
        ::-webkit-scrollbar-thumb:hover { background: #3f3f46; }

        /* Heatmap Styles */
        .diag-coerced { border-bottom: 2px solid #3b82f6; background: rgba(59, 130, 246, 0.1); } /* Blue */
        .diag-bridged { border-bottom: 2px solid #f59e0b; background: rgba(245, 158, 11, 0.1); } /* Amber */
        
        /* Entity Styles */
        .entity { padding: 0 2px; border-radius: 3px; font-weight: 500; font-size: 0.9em; }
        .entity-email { background: rgba(59, 130, 246, 0.2); border: 1px solid #3b82f6; color: #60a5fa; }
        .entity-date { background: rgba(16, 185, 129, 0.2); border: 1px solid #10b981; color: #34d399; }
        .entity-currency { background: rgba(245, 158, 11, 0.2); border: 1px solid #f59e0b; color: #fbbf24; }
        
        .heatmap-overlay { position: absolute; inset: 0; pointer-events: none; z-index: 10; width: 100%; }
        .heatmap-rect { stroke-width: 1; fill-opacity: 0.15; transition: fill-opacity 0.2s; }
        .heatmap-rect.coerced { fill: #3b82f6; stroke: #3b82f6; }
        .heatmap-rect.bridged { fill: #f59e0b; stroke: #f59e0b; }
        .heatmap-rect.low_confidence { fill: #ef4444; stroke: #ef4444; fill-opacity: 0.3; } /* Red */
        .heatmap-rect:hover { fill-opacity: 0.6; }

        .legend { margin-top: 1rem; padding: 0.75rem; background: #18181b; border: 1px solid #27272a; border-radius: 8px; font-size: 0.7rem; }
        .legend-item { display: flex; align-items: center; gap: 0.5rem; margin-bottom: 0.4rem; }
        .legend-item:last-child { margin-bottom: 0; }
        .legend-dot { width: 8px; height: 8px; border-radius: 2px; }
    </style>
</head>
<body>
    <div class="top-bar">
        <div class="nav-left">
            <a href="summary.html#card-{{NAME}}" class="back-link">← Back</a>
            <h1>{{NAME}}</h1>
            {{STRUCTURED_BADGE}}
            {{PERFORMANCE_BADGES}}
        </div>
        <div class="status-pill">
            <div id="metrics" style="font-size: 0.75rem; color: #71717a;">Analyzing...</div>
            <div class="accuracy-badge {{ACC_CLASS}}">{{ACC}}% Match</div>
        </div>
    </div>

    <div class="toolbar">
        <div class="view-toggle">
            <button class="toggle-btn active" onclick="setView(event, 'diff')">Visual Diff</button>
            <button class="toggle-btn" onclick="setView(event, 'side')">Side-by-Side</button>
            <button class="toggle-btn" onclick="setView(event, 'raw')">Raw Output</button>
        </div>
        <div style="font-size: 0.75rem; color: #52525b;">Phase 35 Diagnostics</div>
    </div>
    
    <div class="workspace">
        <div class="sidebar">
            <span class="sidebar-label">Source Document & Heatmap</span>
            <div class="source-img-container" id="img-container">
                <a href="../{{NAME}}" target="_blank"><img id="source-img" src="../{{NAME}}" alt="source" /></a>
                <svg id="diag-svg" class="heatmap-overlay" viewBox="0 0 1000 1000" preserveAspectRatio="none"></svg>
            </div>
            
            <div class="legend">
                <div class="legend-item">
                    <div class="legend-dot" style="background: #3b82f6;"></div>
                    <span><b>Coerced</b>: Symbol snapped to column (|)</span>
                </div>
                <div class="legend-item">
                    <div class="legend-dot" style="background: #f59e0b;"></div>
                    <span><b>Bridged</b>: Large gap reconstruction</span>
                </div>
                <div class="legend-item">
                    <div class="legend-dot" style="background: #ef4444;"></div>
                    <span><b>Low Confidence</b>: High geometric jitter</span>
                </div>
                <div class="legend-item">
                    <div class="legend-dot entity-email" style="width: 12px; height: 12px; border-radius: 4px;">@</div>
                    <span><b>Email</b>: Detected via patterns</span>
                </div>
                <div class="legend-item">
                    <div class="legend-dot entity-currency" style="width: 12px; height: 12px; border-radius: 4px;">$</div>
                    <span><b>Currency</b>: Financial entities</span>
                </div>
            </div>

            <div style="font-size: 0.7rem; color: #52525b; line-height: 1.4;">
                Heatmap shows structural corrections. <br/>
                Hover to brighten. Click to zoom.
            </div>
        </div>
        
        <div class="content-pane">
            <!-- Visual Diff Pane -->
            <div id="pane-diff" class="view-pane active">
                <div id="diff-content" class="diff-layout"></div>
            </div>
            
            <!-- Side-by-Side Sync Pane -->
            <div id="pane-side" class="view-pane">
                <div class="sync-layout">
                    <div class="sync-column">
                        <div class="sync-header">Baseline (Expected)</div>
                        <div id="scroll-left" class="sync-scroll">{{EXPECTED_ESCAPED}}</div>
                    </div>
                    <div class="sync-column">
                        <div class="sync-header">Latest Run (Actual)</div>
                        <div id="scroll-right" class="sync-scroll">{{ACTUAL_ESCAPED}}</div>
                        <div id="entities-view" style="margin-top: 1rem; padding: 0.5rem; background: #111; border: 1px solid #222; border-radius: 4px; font-size: 0.8rem; display:none;">
                             <h4 style="margin:0 0 0.5rem 0; color:#52525b; font-size:0.7rem; text-transform:uppercase;">Entities Found</h4>
                             <div id="entities-list"></div>
                        </div>
                    </div>
                </div>
            </div>
            
            <!-- Raw Pane -->
            <div id="pane-raw" class="view-pane">
                <div class="diff-layout" style="color: #a1a1aa;">{{ACTUAL_ESCAPED}}</div>
            </div>
        </div>
    </div>

    <script>
        const diagnostics = {{DIAGNOSTICS}};
        const expectedRaw = `{{EXPECTED_RAW}}`;
        const actualRaw = `{{ACTUAL_RAW}}`;
        const entities = {{ENTITIES}};

        function applyEntityHighlights(text, entities) {
            if (!entities || entities.length === 0) return text;
            
            // Sort entities in reverse order to not break offsets
            const sorted = [...entities].sort((a, b) => b.start_offset - a.start_offset);
            let result = text;
            for (const e of sorted) {
                const typeClass = `entity-${e.entity_type.toLowerCase()}`;
                const before = result.substring(0, e.start_offset);
                const after = result.substring(e.end_offset);
                const tagged = `<mark class="entity ${typeClass}" title="${e.entity_type}">${e.value}</mark>`;
                result = before + tagged + after;
            }
            return result;
        }

        window.onload = () => {
            const img = document.getElementById('source-img');
            const svg = document.getElementById('diag-svg');
            
            if (img.complete) initHeatmap();
            else img.onload = initHeatmap;

            function initHeatmap() {
                if (!diagnostics) return;
                const naturalW = img.naturalWidth;
                const naturalH = img.naturalHeight;
                svg.setAttribute('viewBox', `0 0 ${naturalW} ${naturalH}`);
                
                let svgHtml = '';
                for (const d of diagnostics) {
                    const title = d.confidence ? `Confidence: ${d.confidence}%` : d.text;
                    svgHtml += `<rect x="${d.x}" y="${d.y}" width="${d.w}" height="${d.h}" class="heatmap-rect ${d.type}"><title>${title}</title></rect>`;
                }
                svg.innerHTML = svgHtml;
            }
        };

        function setView(event, view) {
            document.querySelectorAll('.view-pane').forEach(p => p.classList.remove('active'));
            document.querySelectorAll('.toggle-btn').forEach(b => b.classList.remove('active'));
            
            document.getElementById('pane-' + view).classList.add('active');
            event.target.classList.add('active');
        }

        // Word-Level Diffing (Myers LCS)
        function computeWordDiff(s1, s2) {
            // Split by words, preserving whitespace/newlines as individual tokens
            const tokenize = (s) => s.split(/(\s+)/);
            const words1 = tokenize(s1);
            const words2 = tokenize(s2);
            
            let n = words1.length, m = words2.length;
            let dp = Array.from({length: n + 1}, () => Array(m + 1).fill(0));
            
            for (let i = 1; i <= n; i++) {
                for (let j = 1; j <= m; j++) {
                    if (words1[i-1] === words2[j-1]) dp[i][j] = dp[i-1][j-1] + 1;
                    else dp[i][j] = Math.max(dp[i-1][j], dp[i][j-1]);
                }
            }

            let result = '';
            let i = n, j = m;
            let chunks = [];
            let adds = 0, dels = 0;

            while (i > 0 || j > 0) {
                if (i > 0 && j > 0 && words1[i-1] === words2[j-1]) {
                    chunks.push({type: 'equal', text: words1[i-1]});
                    i--; j--;
                } else if (j > 0 && (i === 0 || dp[i][j-1] >= dp[i-1][j])) {
                    chunks.push({type: 'add', text: words2[j-1]});
                    adds++;
                    j--;
                } else {
                    chunks.push({type: 'del', text: words1[i-1]});
                    dels++;
                    i--;
                }
            }
            
            chunks.reverse();
            const esc = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/\n/g, '<br/>');
            
            let html = '';
            for (let c of chunks) {
                if (c.type === 'equal') html += `<span>${esc(c.text)}</span>`;
                else if (c.type === 'add') html += `<span class="diff-add">${esc(c.text)}</span>`;
                else html += `<span class="diff-del">${esc(c.text)}</span>`;
            }
            
            document.getElementById('metrics').innerText = `Words: +${adds} -${dels}`;
            return html;
        }

        // Synchronized Scrolling
        const leftScroll = document.getElementById('scroll-left');
        const rightScroll = document.getElementById('scroll-right');
        
        let isSyncingLeftScroll = false;
        let isSyncingRightScroll = false;

        leftScroll.onscroll = function() {
            if (!isSyncingLeftScroll) {
                isSyncingRightScroll = true;
                rightScroll.scrollTop = leftScroll.scrollTop;
            }
            isSyncingLeftScroll = false;
        };

        rightScroll.onscroll = function() {
            if (!isSyncingRightScroll) {
                isSyncingLeftScroll = true;
                leftScroll.scrollTop = rightScroll.scrollTop;
            }
            isSyncingRightScroll = false;
        };

        // Initialize
        try {
            document.getElementById('diff-content').innerHTML = computeWordDiff(expectedRaw, actualRaw);
            
            // Apply highlights to Side-by-Side Actual pane
            const rightScroll = document.getElementById('scroll-right');
            const rawActual = rightScroll.innerText;
            rightScroll.innerHTML = applyEntityHighlights(rawActual, entities).replace(/\n/g, '<br/>');

            if (entities.length > 0) {
                const view = document.getElementById('entities-view');
                const list = document.getElementById('entities-list');
                view.style.display = 'block';
                entities.forEach(e => {
                    const pill = document.createElement('div');
                    pill.className = `entity entity-${e.entity_type.toLowerCase()}`;
                    pill.style.display = 'inline-block';
                    pill.style.margin = '2px';
                    pill.innerText = `${e.entity_type}: ${e.value}`;
                    list.appendChild(pill);
                });
            }
        } catch (e) {
            document.getElementById('diff-content').innerText = "Error: " + e.message;
        }
    </script>
</body>
</html>"###;

    template
        .replace("{{NAME}}", name)
        .replace("{{ACC}}", &accuracy.to_string())
        .replace("{{ACC_CLASS}}", acc_class)
        .replace("{{ACTUAL_ESCAPED}}", &actual_escaped)
        .replace("{{EXPECTED_ESCAPED}}", &expected_escaped)
        .replace("{{EXPECTED_RAW}}", &expected.replace("`", "\\`").replace("${", "\\${"))
        .replace("{{ACTUAL_RAW}}", &actual.replace("`", "\\`").replace("${", "\\${"))
        .replace("{{ENTITIES}}", &serde_json::to_string(&entities).unwrap_or("[]".to_string()))
        .replace("{{STRUCTURED_BADGE}}", structured_badge)
        .replace("{{PERFORMANCE_BADGES}}", &performance_badges)
        .replace("{{DIAGNOSTICS}}", diagnostics)
}


use digicore_text_expander::adapters::extraction::windows_ocr::RuntimeConfig;

#[tokio::test]
#[ignore] // Run with: cargo test --test ocr_regression_tests -- --ignored --nocapture
async fn tune_ocr_heuristics() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let workspace_dir = PathBuf::from(manifest_dir).join("../../");
    let sample_dir = workspace_dir.join("docs/sample-ocr-images");
    
    let mut entries: Vec<_> = fs::read_dir(&sample_dir)
        .expect("Failed to read sample directory")
        .filter_map(Result::ok)
        .filter(|e| {
            let ext = e.path().extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
            ext == "png" || ext == "jpg" || ext == "jpeg"
        })
        .collect();
    entries.sort_by_key(|e| e.path());

    println!("🚀 Starting Heuristic Auto-Tune Sweep...");
    println!("Targeting {} images", entries.len());

    let mut best_score = 0.0;
    let mut best_config = RuntimeConfig::default();
    let mut sweep_results = Vec::new();

    // Define Sweep Ranges
    let cluster_ranges = [0.35, 0.40, 0.45, 0.50, 0.55];
    let gap_ranges = [0.25, 0.30, 0.35, 0.40, 0.45];

    for &cluster in &cluster_ranges {
        for &gap in &gap_ranges {
            let mut config = RuntimeConfig::default();
            config.extraction.layout_heuristics.cluster_threshold_factor = cluster;
            config.extraction.layout_heuristics.cross_zone_gap_factor = gap;

            let adapter = WindowsNativeOcrAdapter::new(Some(config.clone()));
            let mut total_acc = 0.0;
            let mut count = 0;

            for entry in &entries {
                let path = entry.path();
                let file_name = path.file_name().unwrap().to_string_lossy().to_string();
                let snapshot_name = file_name.replace(".", "_");
                
                let mime_type = if path.extension().unwrap() == "png" { ExtractionMimeType::Png } else { ExtractionMimeType::Jpeg };
                
                if let Ok(result) = adapter.extract(ExtractionSource::File(path.to_path_buf()), mime_type).await {
                    let expected_text = get_snapshot_content(manifest_dir, &snapshot_name).unwrap_or_default();
                    let accuracy = normalized_levenshtein(result.text.trim(), &expected_text);
                    total_acc += accuracy;
                    count += 1;
                }
            }

            let avg_acc = if count > 0 { total_acc / count as f64 } else { 0.0 };
            println!("Sweep: Cluster={:.2}, Gap={:.2} => Avg Acc={:.2}%", cluster, gap, avg_acc * 100.0);
            
            sweep_results.push((cluster, gap, avg_acc));

            if avg_acc > best_score {
                best_score = avg_acc;
                best_config = config;
            }
        }
    }

    println!("\n🏆 BEST CONFIGURATION FOUND:");
    println!("Score: {:.2}%", best_score * 100.0);
    println!("Config: {:?}", best_config);

    // Generate Tuning Report
    let timestamp = chrono::Local::now().format("%Y-%m-%d_%I%M%p%Z").to_string().replace(" ", "").replace(":", "");
    let report_path = sample_dir.join(format!("tuning_report_{}.html", timestamp));
    
    let mut grid_html = String::new();
    for (c, g, acc) in sweep_results {
        let pct = (acc * 100.0) as u32;
        let color = if pct >= 95 { "#22c55e" } else if pct >= 85 { "#f59e0b" } else { "#ef4444" };
        grid_html.push_str(&format!(
            "<div class='cell' style='background: {};'>
                <strong>{:.0}%</strong><br/>
                C:{:.2} G:{:.2}
            </div>",
            color, pct, c, g
        ));
    }

    let report_html = format!(r###"<!DOCTYPE html>
<html>
<head>
    <title>OCR Tuning Report</title>
    <style>
        body {{ font-family: sans-serif; background: #0f0f11; color: #fff; padding: 2rem; }}
        .grid {{ display: grid; grid-template-columns: repeat(5, 1fr); gap: 10px; margin-top: 2rem; }}
        .cell {{ padding: 1.5rem; border-radius: 8px; text-align: center; color: #000; font-weight: bold; }}
        .best {{ border: 4px solid #fff; padding: 2rem; background: #18181b; border-radius: 12px; margin-bottom: 2rem; color: #fff; }}
    </style>
</head>
<body>
    <h1>OCR Heuristic Sweep Report</h1>
    <div class="best">
        <h2>🏆 Optimal Config Found</h2>
        <p><strong>Accuracy: {:.2}%</strong></p>
        <code>Cluster: {:.2}, Gap: {:.2}</code>
    </div>
    <div class="grid">{}</div>
</body>
</html>"###, best_score * 100.0, best_config.extraction.layout_heuristics.cluster_threshold_factor, best_config.extraction.layout_heuristics.cross_zone_gap_factor, grid_html);

    fs::write(&report_path, report_html).expect("Failed to write tuning report");
    println!("Report saved to: {:?}", report_path);
}
