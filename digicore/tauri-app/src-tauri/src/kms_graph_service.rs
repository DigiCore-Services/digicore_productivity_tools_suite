//! Knowledge graph analytics: k-means clustering, semantic beams, cluster labels, local BFS,
//! and full-graph DTO assembly (hexagonal: repository + pure graph logic, IPC adapters map DTOs).
//!
//! ## Directed vs undirected semantics
//! - **`kms_links` rows** are **directed** (source -> target) wiki resolutions.
//! - **Local neighborhood BFS**, **undirected PageRank**, and **shortest_path_undirected_wiki** treat the
//!   wiki graph as **undirected** for traversal and centrality (both endpoints of a stored row are neighbors).
//! - **AI beams** are undirected pairs for visualization; similarity is computed on the two note embeddings.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

use chrono::{DateTime, Duration, NaiveDate, Utc};
use digicore_kms_ports::LoadNoteEmbeddingsPort;
use sha2::{Digest, Sha256};
use single_clustering::community_search::leiden::partition::{ModularityPartition, VertexPartition};
use single_clustering::community_search::leiden::{LeidenConfig, LeidenOptimizer};
use single_clustering::network::CSRNetwork;
use single_clustering::network::grouping::VectorGrouping;

use crate::kms_diagnostic_service::KmsDiagnosticService;
use crate::kms_graph_ports::{
    KmsRepositoryEmbeddingsAdapter, KmsRepositoryGraphAdapter, LoadNotesMinimalPort, LoadWikiLinksPort,
    WikiLinkAdjacencyCacheAdapter,
};
use crate::kms_repository::{self, KmsNoteMinimal};

/// Tunables for semantic clustering and AI beams (already clamped by the caller).
#[derive(Clone, Debug)]
pub struct KmsGraphSemanticParams {
    pub enable_ai_beams: bool,
    pub k_means_max_k: usize,
    pub k_means_iterations: usize,
    pub ai_beam_max_nodes: usize,
    pub ai_beam_similarity_threshold: f32,
    pub ai_beam_max_edges: usize,
    /// Caps inner-loop cosine comparisons (approximate / bounded beam search). 0 = unlimited.
    pub beam_max_pair_checks: usize,
}

#[derive(Clone, Debug)]
pub struct SemanticBeam {
    pub source_path: String,
    pub target_path: String,
    pub summary: String,
}

/// Optional UTC bounds from the client for one-shot graph views (RFC3339 or `YYYY-MM-DD`).
#[derive(Clone, Debug, Default)]
pub struct TemporalRpcOverride {
    pub time_from_utc: Option<String>,
    pub time_to_utc: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct SemanticLayerResult {
    pub cluster_map: HashMap<String, i32>,
    pub beams: Vec<SemanticBeam>,
    /// True when beam search stopped early due to the pair-check budget (not edge count).
    pub beam_pair_budget_exhausted: bool,
}

pub fn abs_path_from_vault(vault: &Path, relative_path: &str) -> String {
    vault.join(relative_path).to_string_lossy().to_string()
}

/// Normalize vault-relative paths for embedding / cluster key alignment (slashes, trim, no leading `/`).
pub fn norm_vault_rel_path(rel: &str) -> String {
    rel.replace('\\', "/").trim().trim_start_matches('/').to_string()
}

/// Resolved PageRank computation mode for one `build_full_graph` call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KmsPagerankScopeMode {
    /// Run PageRank on the full vault graph, then apply pagination (if any).
    FullVault,
    /// When paginated, PageRank runs only on the page subgraph after slicing.
    PageSubgraph,
    Off,
}

/// Map persisted scope string + whether this request is paginated to a concrete mode.
/// `auto`: paginated => page subgraph; unpaged => full vault (matches product default).
pub fn resolve_pagerank_scope(raw: &str, paged: bool) -> KmsPagerankScopeMode {
    let s = raw.trim().to_ascii_lowercase();
    match s.as_str() {
        "" | "auto" => {
            if paged {
                KmsPagerankScopeMode::PageSubgraph
            } else {
                KmsPagerankScopeMode::FullVault
            }
        }
        "full_vault" | "full" => KmsPagerankScopeMode::FullVault,
        "page_subgraph" | "page" => KmsPagerankScopeMode::PageSubgraph,
        "off" | "none" => KmsPagerankScopeMode::Off,
        _ => {
            if paged {
                KmsPagerankScopeMode::PageSubgraph
            } else {
                KmsPagerankScopeMode::FullVault
            }
        }
    }
}

/// Minimal node fields for cluster label generation (titles + cluster ids).
#[derive(Clone, Debug)]
pub struct NodeTitleCluster {
    pub cluster_id: Option<i32>,
    pub title: String,
}

fn dot_product(v1: &[f32], v2: &[f32]) -> f32 {
    v1.iter().zip(v2.iter()).map(|(a, b)| a * b).sum()
}

fn l2_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-12)
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    dot_product(a, b) / (l2_norm(a) * l2_norm(b))
}

/// Undirected kNN links from cosine similarity on note embeddings. Deduplicates by normalized path pair.
/// `max_pair_checks`: 0 = unlimited inner comparisons (careful on large vaults).
pub fn semantic_knn_edges_from_embeddings(
    embedding_data: &[(String, Vec<f32>)],
    k_per_node: usize,
    min_similarity: f32,
    max_total_edges: usize,
    max_pair_checks: usize,
    resolve_absolute: impl Fn(&str) -> String,
) -> (Vec<BuiltGraphEdge>, bool) {
    let m = embedding_data.len();
    if m < 2 || k_per_node == 0 || max_total_edges == 0 {
        return (Vec::new(), false);
    }
    let k_take = k_per_node.min(m.saturating_sub(1));
    let mut pair_checks: usize = 0;
    let mut budget_hit = false;
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut out: Vec<BuiltGraphEdge> = Vec::new();

    'nodes: for i in 0..m {
        if out.len() >= max_total_edges {
            break;
        }
        let mut scored: Vec<(usize, f32)> = Vec::new();
        let mut inner_budget = false;
        for j in 0..m {
            if i == j {
                continue;
            }
            if max_pair_checks > 0 {
                pair_checks += 1;
                if pair_checks > max_pair_checks {
                    budget_hit = true;
                    inner_budget = true;
                    break;
                }
            }
            let sim = cosine_similarity(&embedding_data[i].1, &embedding_data[j].1);
            if sim >= min_similarity {
                scored.push((j, sim));
            }
        }
        if inner_budget && scored.is_empty() {
            break 'nodes;
        }
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        for (j, _) in scored.into_iter().take(k_take) {
            if out.len() >= max_total_edges {
                break;
            }
            let a_abs = resolve_absolute(&embedding_data[i].0);
            let b_abs = resolve_absolute(&embedding_data[j].0);
            let na = norm_rel_path(&a_abs);
            let nb = norm_rel_path(&b_abs);
            let (low, high) = if na <= nb { (na, nb) } else { (nb, na) };
            if seen.insert((low.clone(), high.clone())) {
                let (src, tgt) = if a_abs <= b_abs {
                    (a_abs, b_abs)
                } else {
                    (b_abs, a_abs)
                };
                out.push(BuiltGraphEdge {
                    source: src,
                    target: tgt,
                    kind: "semantic_knn".to_string(),
                    edge_recency: None,
                });
            }
        }
    }
    (out, budget_hit)
}

fn wiki_edge_pairs(edges: &[BuiltGraphEdge]) -> Vec<(String, String)> {
    edges
        .iter()
        .filter(|e| e.kind.as_str() == "wiki")
        .map(|e| (e.source.clone(), e.target.clone()))
        .collect()
}

/// Stable fingerprint for the undirected wiki edge set (used to validate materialized PageRank).
fn wiki_link_graph_fingerprint(wiki_pairs: &[(String, String)]) -> String {
    let mut pairs: Vec<(&str, &str)> = wiki_pairs
        .iter()
        .map(|(a, b)| {
            if a.as_str() <= b.as_str() {
                (a.as_str(), b.as_str())
            } else {
                (b.as_str(), a.as_str())
            }
        })
        .collect();
    pairs.sort();
    let mut hasher = Sha256::new();
    for (a, b) in pairs {
        hasher.update(a.as_bytes());
        hasher.update([0u8]);
        hasher.update(b.as_bytes());
        hasher.update(b"\n");
    }
    format!("{:x}", hasher.finalize())
}

/// Recomputes **wiki-only** undirected PageRank for all indexed notes and persists scores plus the link fingerprint.
/// Intended after bulk note/link changes (vault sync) so the next full-graph build can reuse materialized scores.
/// No-op when there are no notes. Uses the same path order and edge resolution as [`build_full_graph_from_notes_and_links`].
pub fn materialize_wiki_pagerank_full_vault(
    vault: &Path,
    pagerank_iterations: usize,
    pagerank_damping: f32,
) -> Result<usize, String> {
    let notes = kms_repository::get_all_notes_minimal().map_err(|e| e.to_string())?;
    if notes.is_empty() {
        return Ok(0);
    }
    let links = kms_repository::get_all_links().map_err(|e| e.to_string())?;
    let edges: Vec<BuiltGraphEdge> = links
        .into_iter()
        .map(|(s, t)| BuiltGraphEdge {
            source: abs_path_from_vault(vault, &s),
            target: abs_path_from_vault(vault, &t),
            kind: "wiki".to_string(),
            edge_recency: None,
        })
        .collect();
    let wiki_pairs = wiki_edge_pairs(&edges);
    let cur_fp = wiki_link_graph_fingerprint(&wiki_pairs);
    let paths_full: Vec<String> = notes
        .iter()
        .map(|n| abs_path_from_vault(vault, &n.path))
        .collect();
    let pr_it = pagerank_iterations.max(4);
    let d = pagerank_damping.clamp(0.5, 0.99);
    let pr_scores = undirected_pagerank(&paths_full, &wiki_pairs, pr_it, d);
    let persist: Vec<(String, f32)> = notes
        .iter()
        .zip(pr_scores.iter().copied())
        .map(|(n, s)| (n.path.clone(), s))
        .collect();
    kms_repository::bulk_set_wiki_pagerank(&persist).map_err(|e| e.to_string())?;
    kms_repository::kms_graph_meta_upsert(kms_repository::KMS_GRAPH_META_WIKI_PR_FP, &cur_fp)
        .map_err(|e| e.to_string())?;
    log::info!(
        "[KMS][Graph] materialize_wiki_pagerank_full_vault: persisted {} scores (fingerprint ok for reuse)",
        persist.len()
    );
    Ok(persist.len())
}

/// For each embedding cluster, use the note title whose vector is closest to the cluster centroid (cosine).
/// Keys in `cluster_map` and `embedding_data` must be the same vault-relative paths.
pub fn cluster_medoid_titles_from_embeddings(
    cluster_map: &HashMap<String, i32>,
    embedding_data: &[(String, Vec<f32>)],
    path_to_title: &HashMap<String, String>,
) -> HashMap<i32, String> {
    let emb_by_path: HashMap<String, Vec<f32>> = embedding_data
        .iter()
        .map(|(p, v)| (p.clone(), v.clone()))
        .collect();
    let mut by_cluster: HashMap<i32, Vec<(String, Vec<f32>)>> = HashMap::new();
    for (path, cid) in cluster_map {
        if let Some(vec) = emb_by_path.get(path) {
            by_cluster
                .entry(*cid)
                .or_default()
                .push((path.clone(), vec.clone()));
        }
    }

    let mut out: HashMap<i32, String> = HashMap::new();
    for (cid, members) in by_cluster {
        if members.is_empty() {
            continue;
        }
        let dim = members[0].1.len();
        let mut centroid = vec![0.0f32; dim];
        for (_, v) in &members {
            for i in 0..dim {
                centroid[i] += v[i];
            }
        }
        let inv = 1.0f32 / members.len() as f32;
        for c in &mut centroid {
            *c *= inv;
        }
        let mut best_sim = -2.0f32;
        let mut best_path: Option<&str> = None;
        for (path, v) in &members {
            let s = cosine_similarity(v, &centroid);
            if s > best_sim {
                best_sim = s;
                best_path = Some(path.as_str());
            }
        }
        if let Some(p) = best_path {
            if let Some(t) = path_to_title.get(p) {
                let t = t.trim();
                if !t.is_empty() {
                    let short: String = t.chars().take(52).collect();
                    out.insert(cid, short);
                }
            }
        }
    }
    out
}

/// Prefer medoid (embedding-centroid) titles; fall back to keyword labels; then `Topic {id}`.
pub fn merge_medoid_and_keyword_cluster_labels(
    medoids: HashMap<i32, String>,
    keyword_pairs: Vec<(i32, String)>,
) -> Vec<(i32, String)> {
    let kw: HashMap<i32, String> = keyword_pairs.into_iter().collect();
    let mut ids: Vec<i32> = medoids.keys().chain(kw.keys()).copied().collect();
    ids.sort_unstable();
    ids.dedup();
    ids.into_iter()
        .map(|cid| {
            let label = medoids
                .get(&cid)
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .or_else(|| kw.get(&cid).cloned())
                .unwrap_or_else(|| format!("Topic {}", cid));
            (cid, label)
        })
        .collect()
}

/// K-means (Forgy init). Pure analytics; not a database concern.
pub fn calculate_kmeans_clusters(data: &[Vec<f32>], k: usize, max_iterations: usize) -> Vec<usize> {
    if data.is_empty() {
        return Vec::new();
    }
    if k <= 1 || data.len() <= k {
        return (0..data.len()).map(|i| if k > 0 { i % k } else { 0 }).collect();
    }

    let dim = data[0].len();
    let mut centroids: Vec<Vec<f32>> = data.iter().take(k).cloned().collect();
    let mut assignments = vec![0; data.len()];

    for _ in 0..max_iterations {
        let mut changed = false;

        for (i, point) in data.iter().enumerate() {
            let mut min_dist = f32::MAX;
            let mut best_cluster = 0;
            for (c_idx, centroid) in centroids.iter().enumerate() {
                let dist: f32 = point
                    .iter()
                    .zip(centroid.iter())
                    .map(|(a, b)| (a - b).powi(2))
                    .sum();
                if dist < min_dist {
                    min_dist = dist;
                    best_cluster = c_idx;
                }
            }
            if assignments[i] != best_cluster {
                assignments[i] = best_cluster;
                changed = true;
            }
        }

        if !changed {
            break;
        }

        let mut new_centroids = vec![vec![0.0; dim]; k];
        let mut counts = vec![0; k];
        for (i, cluster_idx) in assignments.iter().enumerate() {
            counts[*cluster_idx] += 1;
            for d in 0..dim {
                new_centroids[*cluster_idx][d] += data[i][d];
            }
        }

        for c_idx in 0..k {
            if counts[c_idx] > 0 {
                for d in 0..dim {
                    centroids[c_idx][d] = new_centroids[c_idx][d] / counts[c_idx] as f32;
                }
            }
        }
    }

    assignments
}

/// AI beams from an existing cluster assignment (e.g. k-means or Leiden).
pub fn semantic_beams_from_cluster_assignments(
    embedding_data: &[(String, Vec<f32>)],
    cluster_map: &HashMap<String, i32>,
    params: &KmsGraphSemanticParams,
    resolve_absolute: impl Fn(&str) -> String,
) -> (Vec<SemanticBeam>, bool) {
    if embedding_data.is_empty() || !params.enable_ai_beams {
        return (Vec::new(), false);
    }

    let paths_for_embeddings: Vec<&String> = embedding_data.iter().map(|(p, _)| p).collect();
    let vectors: Vec<&Vec<f32>> = embedding_data.iter().map(|(_, v)| v).collect();

    let mut ai_beams = Vec::new();
    let max_nodes = vectors.len().min(params.ai_beam_max_nodes);
    let pair_budget = params.beam_max_pair_checks;
    let mut pair_checks: usize = 0;
    let mut pair_budget_exhausted = false;
    'outer: for i in 0..max_nodes {
        for j in (i + 1)..max_nodes {
            if pair_budget > 0 {
                pair_checks += 1;
                if pair_checks > pair_budget {
                    pair_budget_exhausted = true;
                    break 'outer;
                }
            }

            let path_a = paths_for_embeddings[i];
            let path_b = paths_for_embeddings[j];

            let cid_a = cluster_map.get(path_a).cloned().unwrap_or(-1);
            let cid_b = cluster_map.get(path_b).cloned().unwrap_or(-1);

            if cid_a != cid_b && cid_a != -1 && cid_b != -1 {
                let sim = cosine_similarity(vectors[i], vectors[j]);
                if sim > params.ai_beam_similarity_threshold {
                    let pct = (sim.clamp(0.0, 1.0) * 100.0).round();
                    ai_beams.push(SemanticBeam {
                        source_path: resolve_absolute(path_a),
                        target_path: resolve_absolute(path_b),
                        summary: format!("Deep Semantic Connection ({:.0}% cosine match)", pct),
                    });
                }
            }

            if ai_beams.len() >= params.ai_beam_max_edges {
                break 'outer;
            }
        }
    }
    (ai_beams, pair_budget_exhausted)
}

/// Runs k-means on embeddings and optionally cross-cluster similarity beams.
pub fn semantic_clustering_and_beams(
    embedding_data: &[(String, Vec<f32>)],
    params: &KmsGraphSemanticParams,
    resolve_absolute: impl Fn(&str) -> String,
) -> SemanticLayerResult {
    let mut out = SemanticLayerResult::default();
    if embedding_data.is_empty() {
        return out;
    }

    let paths_for_embeddings: Vec<String> = embedding_data.iter().map(|(p, _)| p.clone()).collect();
    let vectors: Vec<Vec<f32>> = embedding_data.iter().map(|(_, v)| v.clone()).collect();

    let k_raw = (vectors.len() as f32).sqrt() as usize;
    let k_cap = params.k_means_max_k;
    let k = k_raw.max(2).min(k_cap);

    let clusters = calculate_kmeans_clusters(&vectors, k, params.k_means_iterations);
    for (path, cluster_id) in paths_for_embeddings.iter().zip(clusters.into_iter()) {
        out.cluster_map.insert(path.clone(), cluster_id as i32);
    }

    let (beams, exhausted) = semantic_beams_from_cluster_assignments(
        embedding_data,
        &out.cluster_map,
        params,
        resolve_absolute,
    );
    out.beams = beams;
    out.beam_pair_budget_exhausted = exhausted;
    out
}

/// K-means and medoid/keyword cluster labels on a note subset (e.g. local BFS neighborhood).
/// Does not emit AI beams. Paths in `embedding_rows` must match vault-relative `kms_notes.path`.
pub fn cluster_subgraph_from_embeddings(
    embedding_rows: Vec<(String, Vec<f32>)>,
    path_to_title: &HashMap<String, String>,
    build_params: &KmsGraphBuildParams,
    resolve_absolute: impl Fn(&str) -> String,
) -> (HashMap<String, i32>, Vec<(i32, String)>) {
    let empty_map = HashMap::new();
    let empty_labels = Vec::new();
    if !build_params.enable_semantic_clustering || embedding_rows.is_empty() {
        return (empty_map, empty_labels);
    }

    let semantic_params = KmsGraphSemanticParams {
        enable_ai_beams: false,
        k_means_max_k: build_params.k_means_max_k as usize,
        k_means_iterations: build_params.k_means_iterations,
        ai_beam_max_nodes: build_params.ai_beam_max_nodes,
        ai_beam_similarity_threshold: build_params.ai_beam_similarity_threshold,
        ai_beam_max_edges: build_params.ai_beam_max_edges,
        beam_max_pair_checks: build_params.beam_max_pair_checks,
    };

    let layer = semantic_clustering_and_beams(&embedding_rows, &semantic_params, resolve_absolute);
    let cluster_map = layer.cluster_map;
    if cluster_map.is_empty() {
        return (empty_map, empty_labels);
    }

    let cluster_labels =
        cluster_labels_from_map_and_embeddings(&cluster_map, &embedding_rows, path_to_title);
    (cluster_map, cluster_labels)
}

/// Medoid + keyword labels for a cluster assignment (local subgraph or post-Leiden refresh).
pub fn cluster_labels_from_map_and_embeddings(
    cluster_map: &HashMap<String, i32>,
    embedding_rows: &[(String, Vec<f32>)],
    path_to_title: &HashMap<String, String>,
) -> Vec<(i32, String)> {
    if cluster_map.is_empty() {
        return Vec::new();
    }
    let label_inputs: Vec<NodeTitleCluster> = path_to_title
        .iter()
        .filter_map(|(p, title)| {
            cluster_map.get(p).map(|cid| NodeTitleCluster {
                cluster_id: Some(*cid),
                title: title.clone(),
            })
        })
        .collect();
    let keyword_pairs = cluster_keyword_labels(&label_inputs);
    let medoids =
        cluster_medoid_titles_from_embeddings(cluster_map, embedding_rows, path_to_title);
    merge_medoid_and_keyword_cluster_labels(medoids, keyword_pairs)
}

const CLUSTER_STOP_WORDS: &[&str] = &[
    "the", "and", "a", "of", "to", "in", "is", "it", "for", "with", "on", "notes", "file", "new",
    "note",
];

/// Keyword-based labels per cluster id from note titles.
pub fn cluster_keyword_labels(nodes: &[NodeTitleCluster]) -> Vec<(i32, String)> {
    let mut clusters_to_nodes: HashMap<i32, Vec<&NodeTitleCluster>> = HashMap::new();
    for node in nodes {
        if let Some(cid) = node.cluster_id {
            clusters_to_nodes.entry(cid).or_default().push(node);
        }
    }

    let mut cluster_labels = Vec::new();
    for (cid, cluster_nodes) in clusters_to_nodes {
        let mut word_counts: HashMap<String, usize> = HashMap::new();
        for node in &cluster_nodes {
            let clean_title = node
                .title
                .to_lowercase()
                .replace(|c: char| !c.is_alphanumeric(), " ");
            for word in clean_title.split_whitespace() {
                if word.len() > 3 && !CLUSTER_STOP_WORDS.contains(&word) {
                    *word_counts.entry(word.to_string()).or_default() += 1;
                }
            }
        }

        let mut sorted_words: Vec<_> = word_counts.into_iter().collect();
        sorted_words.sort_by(|a, b| b.1.cmp(&a.1));

        let label = sorted_words
            .into_iter()
            .take(2)
            .map(|(w, _)| {
                let mut c = w.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" & ");

        if !label.is_empty() {
            cluster_labels.push((cid, label));
        }
    }
    cluster_labels
}

/// Undirected BFS neighborhood: vault-relative link pairs, each **undirected** edge once (first-seen orientation).
/// Reference implementation for tests; production builds use [`local_neighborhood_edges_incremental`].
#[allow(dead_code)]
pub fn local_neighborhood_edges(
    all_links: &[(String, String)],
    resolve_abs_display: impl Fn(&str) -> String,
    center_vault_relative_path: &str,
    depth: u32,
) -> (HashSet<String>, Vec<(String, String)>) {
    let target_abs = resolve_abs_display(center_vault_relative_path)
        .replace('\\', "/")
        .to_lowercase();

    let mut adj: HashMap<String, Vec<(String, String, String)>> = HashMap::new();
    for (s_raw, t_raw) in all_links {
        let s = resolve_abs_display(s_raw).replace('\\', "/").to_lowercase();
        let t = resolve_abs_display(t_raw).replace('\\', "/").to_lowercase();

        adj.entry(s.clone())
            .or_default()
            .push((t.clone(), s_raw.clone(), t_raw.clone()));
        adj.entry(t.clone())
            .or_default()
            .push((s, t_raw.clone(), s_raw.clone()));
    }

    let mut visited = HashSet::new();
    let mut local_links_raw = Vec::new();
    let mut seen_undirected_edges = HashSet::<(String, String)>::new();
    let mut queue = VecDeque::new();

    queue.push_back((target_abs.clone(), 0u32));
    visited.insert(target_abs.clone());

    while let Some((curr, d)) = queue.pop_front() {
        if d >= depth {
            continue;
        }
        if let Some(neighbors) = adj.get(&curr) {
            for (neighbor_abs, s_orig, t_orig) in neighbors {
                let edge_key = undirected_vault_link_key(s_orig, t_orig);
                if seen_undirected_edges.insert(edge_key) {
                    local_links_raw.push((s_orig.clone(), t_orig.clone()));
                }
                if !visited.contains(neighbor_abs) {
                    visited.insert(neighbor_abs.clone());
                    queue.push_back((neighbor_abs.clone(), d + 1));
                }
            }
        }
    }

    (visited, local_links_raw)
}

fn norm_abs_display(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

/// Stable key for one undirected wiki link; each physical edge appears once in neighborhood output.
fn undirected_vault_link_key(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

/// Incremental BFS: loads only edges touching the current frontier via `fetch` (SQL-scoped).
/// Returns visited absolute-path keys (`norm_abs_display`) and `(source_rel, target_rel)` pairs
/// in **original storage order** for the first time each undirected link is seen (same contract as [`local_neighborhood_edges`]).
pub fn local_neighborhood_edges_incremental<F, G>(
    center_canonical_rel: &str,
    depth: u32,
    resolve_abs: F,
    mut fetch: G,
) -> Result<(HashSet<String>, Vec<(String, String)>, Vec<String>), String>
where
    F: Fn(&str) -> String,
    G: FnMut(&[String]) -> Result<Vec<(String, String)>, String>,
{
    let center_rel = center_canonical_rel.replace('\\', "/").trim().to_string();
    if center_rel.is_empty() {
        return Ok((HashSet::new(), Vec::new(), Vec::new()));
    }
    let target_abs = norm_abs_display(&resolve_abs(&center_rel));

    let mut visited_abs: HashSet<String> = HashSet::new();
    visited_abs.insert(target_abs.clone());

    let mut rel_by_abs: HashMap<String, String> = HashMap::new();
    rel_by_abs.insert(target_abs.clone(), center_rel);

    let mut seen_undirected: HashSet<(String, String)> = HashSet::new();
    let mut local_links: Vec<(String, String)> = Vec::new();

    let mut queue: VecDeque<(String, u32)> = VecDeque::new();
    queue.push_back((target_abs, 0));

    while let Some((curr_abs, d)) = queue.pop_front() {
        if d >= depth {
            continue;
        }
        let Some(rel) = rel_by_abs.get(&curr_abs).cloned() else {
            continue;
        };
        let batch = fetch(&[rel])?;
        for (s_orig, t_orig) in batch {
            let edge_key = undirected_vault_link_key(&s_orig, &t_orig);
            if seen_undirected.insert(edge_key) {
                local_links.push((s_orig.clone(), t_orig.clone()));
            }
            let s_abs = norm_abs_display(&resolve_abs(&s_orig));
            let t_abs = norm_abs_display(&resolve_abs(&t_orig));

            let (neigh_abs, neigh_rel) = if s_abs == curr_abs {
                (t_abs, t_orig)
            } else if t_abs == curr_abs {
                (s_abs, s_orig)
            } else {
                continue;
            };

            if !visited_abs.contains(&neigh_abs) {
                visited_abs.insert(neigh_abs.clone());
                rel_by_abs.insert(neigh_abs.clone(), neigh_rel);
                queue.push_back((neigh_abs, d + 1));
            }
        }
    }

    let mut visited_rels: Vec<String> = rel_by_abs.values().cloned().collect();
    visited_rels.sort_unstable();
    visited_rels.dedup();

    Ok((visited_abs, local_links, visited_rels))
}

/// Notes whose vault-relative path resolves to a normalized absolute key in `visited_normalized`.
#[allow(dead_code)]
pub fn filter_notes_in_neighborhood(
    all_notes: Vec<KmsNoteMinimal>,
    visited_normalized: &HashSet<String>,
    vault_abs_normalized: impl Fn(&str) -> String,
) -> Vec<KmsNoteMinimal> {
    all_notes
        .into_iter()
        .filter(|n| visited_normalized.contains(&vault_abs_normalized(&n.path)))
        .collect()
}

pub fn node_type_and_folder(path: &str) -> (String, String) {
    let p = Path::new(path);
    let node_type = if path.contains("/skills/") {
        "skill"
    } else if path.to_lowercase().ends_with(".png") || path.to_lowercase().ends_with(".jpg") {
        "image"
    } else {
        "note"
    };
    let folder_path = p
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    (node_type.to_string(), folder_path)
}

// ---------------------------------------------------------------------------
// Full-graph assembly (repository + analytics; `api` maps to IPC DTOs)
// ---------------------------------------------------------------------------

/// Tunables from app state (mirrors previous `KmsGraphBuildSnapshot` in api).
#[derive(Clone, Debug)]
pub struct KmsGraphBuildParams {
    pub enable_semantic_clustering: bool,
    /// When true, cluster colors use Leiden communities on the undirected wiki + semantic_kNN graph (k-means skipped).
    pub enable_leiden_communities: bool,
    pub enable_ai_beams: bool,
    pub k_means_max_k: u32,
    pub k_means_iterations: usize,
    pub ai_beam_max_nodes: usize,
    pub ai_beam_similarity_threshold: f32,
    pub ai_beam_max_edges: usize,
    pub semantic_max_notes: u32,
    pub warn_note_threshold: u32,
    pub beam_max_pair_checks: usize,
    pub pagerank_iterations: u32,
    pub pagerank_local_iterations: u32,
    pub pagerank_damping: f32,
    /// `auto` | `full_vault` | `page_subgraph` | `off`
    pub pagerank_scope: String,
    /// When false, skip non-blocking materialized wiki PageRank after bulk vault sync (vault overrides may override global AppState).
    pub background_wiki_pagerank_enabled: bool,
    /// Embedding kNN edges in the graph (`semantic_knn`); PageRank still uses wiki links only.
    pub enable_semantic_knn_edges: bool,
    /// Top neighbors per note by cosine similarity (clamped when building).
    pub semantic_knn_per_note: u32,
    pub semantic_knn_min_similarity: f32,
    /// Hard cap on kNN edges added (0 = none).
    pub semantic_knn_max_edges: u32,
    /// Cap on cosine comparisons (0 = unlimited).
    pub semantic_knn_max_pair_checks: u32,
    /// Option A: when true and `temporal_default_days > 0`, or when RPC sends bounds, filter nodes by `last_modified`.
    pub temporal_window_enabled: bool,
    /// Default window length ending at UTC now when `temporal_window_enabled` (0 = no default window).
    pub temporal_default_days: u32,
    /// Include notes with missing or unparseable `last_modified` when Option A is active.
    pub temporal_include_notes_without_mtime: bool,
    /// Option B: attach `edge_recency` on wiki edges for client styling (PageRank unchanged; D3).
    pub temporal_edge_recency_enabled: bool,
    /// Scales Option B channel (0..1).
    pub temporal_edge_recency_strength: f32,
    /// Exponential decay half-life in days for Option B.
    pub temporal_edge_recency_half_life_days: f32,
}

const WARN_LARGE_VAULT: &str = "KMS_WARN_LARGE_VAULT";
const WARN_SEMANTIC_SKIPPED_MAX_NOTES: &str = "KMS_WARN_SEMANTIC_SKIPPED_MAX_NOTES";
const WARN_SEMANTIC_PAGE_CLAMPED: &str = "KMS_WARN_SEMANTIC_PAGE_CLAMPED";
const WARN_SEMANTIC_PAGE_ONLY: &str = "KMS_WARN_SEMANTIC_PAGE_ONLY";
const WARN_SEMANTIC_PAGE_NO_EMBEDDINGS: &str = "KMS_WARN_SEMANTIC_PAGE_NO_EMBEDDINGS";
const WARN_SEMANTIC_KNN_MAX_NOTES: &str = "KMS_WARN_SEMANTIC_KNN_MAX_NOTES";
const WARN_AI_BEAM_PAIR_BUDGET: &str = "KMS_WARN_AI_BEAM_PAIR_BUDGET";
const WARN_SEMANTIC_EMBED_LOAD_FAIL: &str = "KMS_WARN_SEMANTIC_EMBED_LOAD_FAIL";
const WARN_SEMANTIC_KNN_PAIR_BUDGET: &str = "KMS_WARN_SEMANTIC_KNN_PAIR_BUDGET";
const WARN_LEIDEN_WIKI_ONLY: &str = "KMS_WARN_LEIDEN_WIKI_ONLY";
const WARN_LEIDEN_SKIPPED: &str = "KMS_WARN_LEIDEN_SKIPPED";
const WARN_LEIDEN_LOCAL_WIKI_ONLY: &str = "KMS_WARN_LEIDEN_LOCAL_WIKI_ONLY";
const WARN_LEIDEN_LOCAL_SKIPPED: &str = "KMS_WARN_LEIDEN_LOCAL_SKIPPED";

fn graph_warning(code: &str, message: impl Into<String>) -> String {
    format!("{code}::{}", message.into())
}

fn push_graph_warning(warnings: &mut Vec<String>, code: &str, message: impl Into<String>) {
    warnings.push(graph_warning(code, message));
}

/// Builds [`KmsGraphSemanticParams`] from graph build settings (already clamped upstream).
pub fn kms_semantic_params_from_build(params: &KmsGraphBuildParams) -> KmsGraphSemanticParams {
    KmsGraphSemanticParams {
        enable_ai_beams: params.enable_ai_beams,
        k_means_max_k: params.k_means_max_k as usize,
        k_means_iterations: params.k_means_iterations,
        ai_beam_max_nodes: params.ai_beam_max_nodes,
        ai_beam_similarity_threshold: params.ai_beam_similarity_threshold,
        ai_beam_max_edges: params.ai_beam_max_edges,
        beam_max_pair_checks: params.beam_max_pair_checks,
    }
}

#[derive(Clone, Debug)]
pub struct BuiltGraphNode {
    pub id: i32,
    pub abs_path: String,
    /// Vault-relative path (SQLite `kms_notes.path`); used when persisting materialized PageRank.
    pub vault_rel_path: String,
    pub title: String,
    pub node_type: String,
    pub last_modified: String,
    pub folder_path: String,
    pub cluster_id: Option<i32>,
    /// Wiki PageRank loaded from `kms_notes.wiki_pagerank` when present; validated with link fingerprint.
    pub wiki_pagerank_stored: Option<f32>,
    /// Undirected wiki-link PageRank, normalized 0..1 (max node = 1.0).
    pub link_centrality: f32,
}

#[derive(Clone, Debug)]
pub struct BuiltGraphPagination {
    pub total_nodes: u32,
    pub offset: u32,
    pub limit: u32,
    pub returned_nodes: u32,
    pub has_more: bool,
}

/// One edge in the assembled graph (`wiki` vs `semantic_knn`).
#[derive(Clone, Debug, PartialEq)]
pub struct BuiltGraphEdge {
    pub source: String,
    pub target: String,
    pub kind: String,
    /// Option B: visualization channel from endpoint recency (0..1); `None` when disabled.
    pub edge_recency: Option<f32>,
}

#[derive(Clone, Debug)]
pub struct BuiltFullGraph {
    pub nodes: Vec<BuiltGraphNode>,
    pub edges: Vec<BuiltGraphEdge>,
    pub cluster_labels: Vec<(i32, String)>,
    pub beams: Vec<SemanticBeam>,
    pub warnings: Vec<String>,
    pub pagination: Option<BuiltGraphPagination>,
}

fn parse_note_mtime_utc(raw: &str) -> Option<DateTime<Utc>> {
    let t = raw.trim();
    if t.is_empty() {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(t) {
        return Some(dt.with_timezone(&Utc));
    }
    if let Ok(d) = NaiveDate::parse_from_str(t, "%Y-%m-%d") {
        return d
            .and_hms_opt(0, 0, 0)
            .map(|nd| DateTime::<Utc>::from_naive_utc_and_offset(nd, Utc));
    }
    None
}

fn resolve_effective_temporal_window(
    params: &KmsGraphBuildParams,
    rpc: &TemporalRpcOverride,
) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let now = Utc::now();
    let rpc_from = rpc
        .time_from_utc
        .as_deref()
        .and_then(parse_note_mtime_utc);
    let rpc_to = rpc.time_to_utc.as_deref().and_then(parse_note_mtime_utc);
    if rpc_from.is_some() || rpc_to.is_some() {
        let from = rpc_from.unwrap_or_else(|| now - Duration::days(365 * 50));
        let to = rpc_to.unwrap_or(now);
        return if from <= to { Some((from, to)) } else { None };
    }
    if params.temporal_window_enabled && params.temporal_default_days > 0 {
        let from = now - Duration::days(params.temporal_default_days as i64);
        return Some((from, now));
    }
    None
}

fn note_in_temporal_window(
    last_modified: &str,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    include_no_mtime: bool,
) -> bool {
    match parse_note_mtime_utc(last_modified) {
        None => include_no_mtime,
        Some(ts) => ts >= from && ts <= to,
    }
}

fn wiki_edge_recency_score(youngest_endpoint: DateTime<Utc>, now: DateTime<Utc>, half_life_days: f32) -> f32 {
    let hl = half_life_days.max(0.1f32);
    let secs = (now - youngest_endpoint).num_seconds().max(0) as f32;
    let days = secs / 86400.0;
    (-days / hl).exp().clamp(0.0, 1.0)
}

fn apply_temporal_window_filter(
    nodes: &mut Vec<BuiltGraphNode>,
    edges: &mut Vec<BuiltGraphEdge>,
    beams: &mut Vec<SemanticBeam>,
    window: (DateTime<Utc>, DateTime<Utc>),
    include_no_mtime: bool,
    warnings: &mut Vec<String>,
) {
    let (from, to) = window;
    let before = nodes.len();
    nodes.retain(|n| note_in_temporal_window(&n.last_modified, from, to, include_no_mtime));
    let keep: HashSet<String> = nodes.iter().map(|n| n.abs_path.clone()).collect();
    edges.retain(|e| keep.contains(&e.source) && keep.contains(&e.target));
    beams.retain(|b| keep.contains(&b.source_path) && keep.contains(&b.target_path));
    if nodes.len() < before {
        warnings.push(format!(
            "Time window filter active: showing {} of {} notes (Option A). Link centrality still reflects full-vault PageRank.",
            nodes.len(),
            before
        ));
    }
}

fn apply_edge_recency_channel(
    edges: &mut [BuiltGraphEdge],
    nodes: &[BuiltGraphNode],
    params: &KmsGraphBuildParams,
    now: DateTime<Utc>,
) {
    if !params.temporal_edge_recency_enabled {
        for e in edges.iter_mut() {
            e.edge_recency = None;
        }
        return;
    }
    let half = params.temporal_edge_recency_half_life_days.max(0.1);
    let strength = params.temporal_edge_recency_strength.clamp(0.0, 1.0);
    let m: HashMap<String, Option<DateTime<Utc>>> = nodes
        .iter()
        .map(|n| (n.abs_path.clone(), parse_note_mtime_utc(&n.last_modified)))
        .collect();
    for e in edges.iter_mut() {
        if e.kind != "wiki" {
            e.edge_recency = None;
            continue;
        }
        let sa = m.get(&e.source).copied().flatten();
        let sb = m.get(&e.target).copied().flatten();
        let base = match (sa, sb) {
            (Some(a), Some(b)) => wiki_edge_recency_score(a.max(b), now, half),
            (Some(a), None) | (None, Some(a)) => wiki_edge_recency_score(a, now, half),
            (None, None) => 0.35f32,
        };
        e.edge_recency = Some((base * strength).clamp(0.0, 1.0));
    }
}

/// Leiden communities on wiki + `semantic_knn` edges. Keys are `BuiltGraphNode.vault_rel_path`.
pub fn leiden_cluster_map_for_nodes(
    nodes: &[BuiltGraphNode],
    edges: &[BuiltGraphEdge],
) -> Result<HashMap<String, i32>, String> {
    let n = nodes.len();
    if n == 0 {
        return Ok(HashMap::new());
    }

    let mut idx: HashMap<String, usize> = HashMap::with_capacity(n);
    for (i, node) in nodes.iter().enumerate() {
        idx.insert(norm_rel_path(&node.abs_path), i);
    }

    let mut pair_w: HashMap<(usize, usize), f32> = HashMap::new();
    for e in edges {
        if e.kind != "wiki" && e.kind != "semantic_knn" {
            continue;
        }
        let Some(&ia) = idx.get(&norm_rel_path(&e.source)) else {
            continue;
        };
        let Some(&ib) = idx.get(&norm_rel_path(&e.target)) else {
            continue;
        };
        if ia == ib {
            continue;
        }
        let (lo, hi) = if ia < ib { (ia, ib) } else { (ib, ia) };
        *pair_w.entry((lo, hi)).or_insert(0.0) += 1.0;
    }

    let csr_edges: Vec<(usize, usize, f32)> =
        pair_w.into_iter().map(|((a, b), w)| (a, b, w)).collect();
    let node_weights = vec![1.0f32; n];
    let network = CSRNetwork::from_edges(&csr_edges, node_weights);

    let mut config = LeidenConfig::default();
    config.seed = Some(42);
    let mut opt = LeidenOptimizer::new(config);
    let mut partition: ModularityPartition<f32, VectorGrouping> = opt
        .find_partition(network)
        .map_err(|e| format!("Leiden community detection failed: {}", e))?;
    partition.renumber_communities();
    let membership = partition.membership_vector();
    if membership.len() != n {
        return Err("Leiden membership length mismatch".to_string());
    }

    let mut out = HashMap::with_capacity(n);
    for (i, node) in nodes.iter().enumerate() {
        out.insert(node.vault_rel_path.clone(), membership[i] as i32);
    }
    Ok(out)
}

/// Assembles the full wiki graph. `pagination` is `(offset, limit)`; **`limit == 0` means no slicing** (all nodes).
pub fn build_full_graph(
    vault: &Path,
    params: &KmsGraphBuildParams,
    pagination: Option<(u32, u32)>,
) -> Result<BuiltFullGraph, String> {
    build_full_graph_with_ports(
        vault,
        params,
        pagination,
        &KmsRepositoryGraphAdapter,
        &WikiLinkAdjacencyCacheAdapter,
        &KmsRepositoryEmbeddingsAdapter,
        &TemporalRpcOverride::default(),
    )
}

/// Same as [`build_full_graph`] but injects data ports and optional RPC temporal overrides.
pub fn build_full_graph_with_ports<N, L, E>(
    vault: &Path,
    params: &KmsGraphBuildParams,
    pagination: Option<(u32, u32)>,
    notes_port: &N,
    links_port: &L,
    embed_port: &E,
    temporal_rpc: &TemporalRpcOverride,
) -> Result<BuiltFullGraph, String>
where
    N: LoadNotesMinimalPort + ?Sized,
    L: LoadWikiLinksPort + ?Sized,
    E: LoadNoteEmbeddingsPort + ?Sized,
{
    let notes = notes_port.all_notes_minimal().map_err(|e| e.to_string())?;
    let links = links_port.all_wiki_link_pairs().map_err(|e| e.to_string())?;
    build_full_graph_from_notes_and_links(
        vault,
        notes,
        links,
        params,
        pagination,
        embed_port,
        temporal_rpc,
    )
}

/// Core graph assembly from preloaded notes and wiki links (`build_full_graph` + unit tests).
pub(crate) fn build_full_graph_from_notes_and_links<E: LoadNoteEmbeddingsPort + ?Sized>(
    vault: &Path,
    notes: Vec<KmsNoteMinimal>,
    links: Vec<(String, String)>,
    params: &KmsGraphBuildParams,
    pagination: Option<(u32, u32)>,
    embed_port: &E,
    temporal_rpc: &TemporalRpcOverride,
) -> Result<BuiltFullGraph, String> {
    let build_started = std::time::Instant::now();
    let note_count = notes.len();

    let mut warnings: Vec<String> = Vec::new();
    if params.warn_note_threshold > 0 && note_count >= params.warn_note_threshold as usize {
        push_graph_warning(&mut warnings, WARN_LARGE_VAULT, format!(
            "Large vault: {note_count} indexed notes (warning threshold {}). Graph interactions may feel slower.",
            params.warn_note_threshold
        ));
    }

    let paged_request = matches!(pagination, Some((_, lim)) if lim > 0);

    let page_rel_paths: Option<HashSet<String>> = match pagination {
        Some((offset, limit)) if limit > 0 && !notes.is_empty() => {
            let mut sorted = notes.clone();
            sorted.sort_by(|a, b| {
                abs_path_from_vault(vault, &a.path).cmp(&abs_path_from_vault(vault, &b.path))
            });
            let start = offset as usize;
            if start >= sorted.len() {
                Some(HashSet::new())
            } else {
                let end = (start + limit as usize).min(sorted.len());
                Some(sorted[start..end].iter().map(|n| n.path.clone()).collect())
            }
        }
        _ => None,
    };

    let semantics_skipped = !paged_request
        && params.enable_semantic_clustering
        && params.semantic_max_notes > 0
        && note_count > params.semantic_max_notes as usize;
    if semantics_skipped {
        push_graph_warning(&mut warnings, WARN_SEMANTIC_SKIPPED_MAX_NOTES, format!(
            "Semantic clustering and AI beams were skipped: {note_count} notes exceeds the configured maximum ({}) (Configurations and Settings > Knowledge Graph).",
            params.semantic_max_notes
        ));
    }

    if notes.is_empty() {
        KmsDiagnosticService::debug(
            "KMS graph: vault has no indexed notes",
            Some("kms_get_graph returned empty node set".to_string()),
        );
    }

    let defer_leiden_kmeans = params.enable_leiden_communities
        && params.enable_semantic_clustering
        && !semantics_skipped;

    let mut cluster_map: HashMap<String, i32> = HashMap::new();
    let mut global_beams: Vec<SemanticBeam> = Vec::new();
    let mut note_embeddings: Vec<(String, Vec<f32>)> = Vec::new();

    let load_embeddings = (params.enable_semantic_clustering && !semantics_skipped)
        || params.enable_semantic_knn_edges;

    if load_embeddings {
        match embed_port.load_all_note_embeddings() {
            Ok(mut embedding_data) => {
                if semantics_skipped {
                    embedding_data.clear();
                }

                if let Some(ref paths) = page_rel_paths {
                    if !paths.is_empty() {
                        embedding_data.retain(|(p, _)| paths.contains(p));
                        embedding_data.sort_by(|a, b| a.0.cmp(&b.0));
                        if params.semantic_max_notes > 0
                            && embedding_data.len() > params.semantic_max_notes as usize
                        {
                            push_graph_warning(&mut warnings, WARN_SEMANTIC_PAGE_CLAMPED, format!(
                                "This page had more embedded notes than the max-for-semantics cap ({}); clustering used the first {} by vault-relative path order.",
                                params.semantic_max_notes,
                                params.semantic_max_notes
                            ));
                            embedding_data.truncate(params.semantic_max_notes as usize);
                        }
                        if !embedding_data.is_empty() {
                            push_graph_warning(
                                &mut warnings,
                                WARN_SEMANTIC_PAGE_ONLY,
                                "Semantic clustering and AI beams were computed for this page only; cluster colors and beams are not comparable across pages."
                                    .to_string(),
                            );
                        } else if !paths.is_empty() {
                            push_graph_warning(
                                &mut warnings,
                                WARN_SEMANTIC_PAGE_NO_EMBEDDINGS,
                                "No embeddings found for notes on this page; semantic clustering was skipped for this page."
                                    .to_string(),
                            );
                        }
                    }
                }

                if !semantics_skipped
                    && page_rel_paths.is_none()
                    && params.semantic_max_notes > 0
                    && embedding_data.len() > params.semantic_max_notes as usize
                    && !params.enable_semantic_clustering
                    && params.enable_semantic_knn_edges
                {
                    embedding_data.sort_by(|a, b| a.0.cmp(&b.0));
                    embedding_data.truncate(params.semantic_max_notes as usize);
                    push_graph_warning(&mut warnings, WARN_SEMANTIC_KNN_MAX_NOTES, format!(
                        "Semantic kNN used only the first {} notes with embeddings (semantic max notes cap). Raise that cap or enable clustering to change scope.",
                        params.semantic_max_notes
                    ));
                }

                let embedding_count = embedding_data.len();
                if !embedding_data.is_empty() {
                    note_embeddings = embedding_data;
                    if params.enable_semantic_clustering && !semantics_skipped && !defer_leiden_kmeans {
                        let semantic_params = kms_semantic_params_from_build(params);
                        let layer = semantic_clustering_and_beams(
                            &note_embeddings,
                            &semantic_params,
                            |p| abs_path_from_vault(vault, p),
                        );
                        cluster_map = layer.cluster_map;
                        if layer.beam_pair_budget_exhausted && params.beam_max_pair_checks > 0 {
                            push_graph_warning(&mut warnings, WARN_AI_BEAM_PAIR_BUDGET, format!(
                                "AI beam search stopped after {} cosine pair comparisons (configurable limit).",
                                params.beam_max_pair_checks
                            ));
                        }
                        global_beams = layer.beams;
                        log::info!(
                            "[KMS][Graph] Semantic Clustering: {} embeddings in scope. Generated {} cluster assignments.",
                            embedding_count,
                            cluster_map.len()
                        );
                    }
                }
            }
            Err(e) => {
                KmsDiagnosticService::warn(
                    "KMS graph: failed to load note embeddings",
                    Some(e.to_string()),
                );
                push_graph_warning(&mut warnings, WARN_SEMANTIC_EMBED_LOAD_FAIL, format!(
                    "Could not load note embeddings; semantic clustering and kNN edges were skipped. ({})",
                    e
                ));
            }
        }
    }

    let path_to_title: HashMap<String, String> = notes
        .iter()
        .map(|n| (n.path.clone(), n.title.clone()))
        .collect();

    let mut nodes: Vec<BuiltGraphNode> = notes
        .into_iter()
        .map(|n| {
            let abs_path = abs_path_from_vault(vault, &n.path);
            let (node_type, folder_path) = node_type_and_folder(&n.path);
            let cluster_id = cluster_map
                .get(&n.path)
                .cloned()
                .or_else(|| cluster_map.get(&norm_vault_rel_path(&n.path)).cloned());
            BuiltGraphNode {
                id: n.id,
                abs_path,
                vault_rel_path: n.path.clone(),
                title: n.title,
                node_type,
                last_modified: n.last_modified.unwrap_or_default(),
                folder_path,
                cluster_id,
                wiki_pagerank_stored: n.wiki_pagerank,
                link_centrality: 0.0,
            }
        })
        .collect();

    let mut edges: Vec<BuiltGraphEdge> = links
        .into_iter()
        .map(|(s, t)| BuiltGraphEdge {
            source: abs_path_from_vault(vault, &s),
            target: abs_path_from_vault(vault, &t),
            kind: "wiki".to_string(),
            edge_recency: None,
        })
        .collect();

    if params.enable_semantic_knn_edges && !note_embeddings.is_empty() {
        let k = params.semantic_knn_per_note.max(1) as usize;
        let max_e = params.semantic_knn_max_edges as usize;
        let max_pairs = params.semantic_knn_max_pair_checks as usize;
        let min_sim = params.semantic_knn_min_similarity.clamp(0.0, 1.0);
        if max_e > 0 {
            let (mut knn_e, knn_budget) = semantic_knn_edges_from_embeddings(
                &note_embeddings,
                k,
                min_sim,
                max_e,
                max_pairs,
                |p| abs_path_from_vault(vault, p),
            );
            if knn_budget && params.semantic_knn_max_pair_checks > 0 {
                push_graph_warning(&mut warnings, WARN_SEMANTIC_KNN_PAIR_BUDGET, format!(
                    "Semantic kNN edge search stopped after {} cosine comparisons (configurable limit).",
                    params.semantic_knn_max_pair_checks
                ));
            }
            edges.append(&mut knn_e);
        }
    }

    if params.enable_leiden_communities && !nodes.is_empty() {
        match leiden_cluster_map_for_nodes(&nodes, &edges) {
            Ok(map) => {
                cluster_map = map;
                for n in &mut nodes {
                    n.cluster_id = cluster_map
                        .get(&n.vault_rel_path)
                        .cloned()
                        .or_else(|| {
                            cluster_map
                                .get(&norm_vault_rel_path(&n.vault_rel_path))
                                .cloned()
                        });
                }
                let knn_edges_present = edges.iter().any(|e| e.kind.as_str() == "semantic_knn");
                if !knn_edges_present {
                    push_graph_warning(
                        &mut warnings,
                        WARN_LEIDEN_WIKI_ONLY,
                        "Leiden communities used wiki links only (no semantic kNN edges in this graph). Enable semantic kNN edges to add embedding-based structure."
                            .to_string(),
                    );
                }
                if params.enable_ai_beams && !note_embeddings.is_empty() {
                    let semantic_params = kms_semantic_params_from_build(params);
                    let (beams, exhausted) = semantic_beams_from_cluster_assignments(
                        &note_embeddings,
                        &cluster_map,
                        &semantic_params,
                        |p| abs_path_from_vault(vault, p),
                    );
                    global_beams = beams;
                    if exhausted && params.beam_max_pair_checks > 0 {
                        push_graph_warning(&mut warnings, WARN_AI_BEAM_PAIR_BUDGET, format!(
                            "AI beam search stopped after {} cosine pair comparisons (configurable limit).",
                            params.beam_max_pair_checks
                        ));
                    }
                }
                let n_comm = cluster_map.values().copied().collect::<HashSet<_>>().len();
                log::info!(
                    "[KMS][Graph] Leiden communities: notes={} distinct_communities={}",
                    nodes.len(),
                    n_comm
                );
            }
            Err(e) => {
                push_graph_warning(&mut warnings, WARN_LEIDEN_SKIPPED, format!("Leiden communities skipped: {}", e));
                KmsDiagnosticService::warn(
                    "KMS graph: Leiden community detection failed",
                    Some(e.clone()),
                );
                if params.enable_semantic_clustering && !semantics_skipped && !note_embeddings.is_empty()
                {
                    let semantic_params = kms_semantic_params_from_build(params);
                    let layer = semantic_clustering_and_beams(
                        &note_embeddings,
                        &semantic_params,
                        |p| abs_path_from_vault(vault, p),
                    );
                    cluster_map = layer.cluster_map;
                    if layer.beam_pair_budget_exhausted && params.beam_max_pair_checks > 0 {
                        push_graph_warning(&mut warnings, WARN_AI_BEAM_PAIR_BUDGET, format!(
                            "AI beam search stopped after {} cosine pair comparisons (configurable limit).",
                            params.beam_max_pair_checks
                        ));
                    }
                    global_beams = layer.beams;
                    for n in &mut nodes {
                        n.cluster_id = cluster_map
                            .get(&n.vault_rel_path)
                            .cloned()
                            .or_else(|| {
                                cluster_map
                                    .get(&norm_vault_rel_path(&n.vault_rel_path))
                                    .cloned()
                            });
                    }
                    log::info!(
                        "[KMS][Graph] Fallback k-means after Leiden failure: {} cluster assignments.",
                        cluster_map.len()
                    );
                }
            }
        }
    }

    let pr_mode = resolve_pagerank_scope(&params.pagerank_scope, paged_request);
    let pr_before_slice = match pr_mode {
        KmsPagerankScopeMode::Off => false,
        KmsPagerankScopeMode::FullVault => true,
        KmsPagerankScopeMode::PageSubgraph => !paged_request,
    };
    let pr_after_slice = paged_request && matches!(pr_mode, KmsPagerankScopeMode::PageSubgraph);

    let pr_it = params.pagerank_iterations.max(4) as usize;
    let d = params.pagerank_damping.clamp(0.5, 0.99);

    if pr_before_slice && !nodes.is_empty() {
        let wiki_pairs_full = wiki_edge_pairs(&edges);
        let cur_fp = wiki_link_graph_fingerprint(&wiki_pairs_full);
        let stored_fp = kms_repository::kms_graph_meta_get(kms_repository::KMS_GRAPH_META_WIKI_PR_FP)
            .map_err(|e| e.to_string())?;
        let fingerprint_ok = stored_fp.as_deref() == Some(cur_fp.as_str());
        let all_nodes_have_pr = nodes.iter().all(|n| n.wiki_pagerank_stored.is_some());
        if fingerprint_ok && all_nodes_have_pr {
            for n in &mut nodes {
                n.link_centrality = n.wiki_pagerank_stored.unwrap_or(0.0);
            }
            log::info!(
                "[KMS][Graph] wiki PageRank: materialized hit for {} nodes (fingerprint match)",
                nodes.len()
            );
        } else {
            let paths_full: Vec<String> = nodes.iter().map(|n| n.abs_path.clone()).collect();
            let pr_scores = undirected_pagerank(&paths_full, &wiki_pairs_full, pr_it, d);
            let persist: Vec<(String, f32)> = nodes
                .iter()
                .zip(pr_scores.iter().copied())
                .map(|(n, s)| (n.vault_rel_path.clone(), s))
                .collect();
            for (node, s) in nodes.iter_mut().zip(pr_scores.iter().copied()) {
                node.link_centrality = s;
            }
            if let Err(e) = kms_repository::bulk_set_wiki_pagerank(&persist) {
                log::warn!(
                    "[KMS][Graph] materialized wiki PageRank persist failed: {}",
                    e
                );
            } else if let Err(e) = kms_repository::kms_graph_meta_upsert(
                kms_repository::KMS_GRAPH_META_WIKI_PR_FP,
                &cur_fp,
            ) {
                log::warn!(
                    "[KMS][Graph] materialized wiki PageRank fingerprint upsert failed: {}",
                    e
                );
            } else {
                log::info!(
                    "[KMS][Graph] wiki PageRank: recomputed and persisted for {} nodes",
                    persist.len()
                );
            }
        }
    }

    if let Some(w) = resolve_effective_temporal_window(params, temporal_rpc) {
        apply_temporal_window_filter(
            &mut nodes,
            &mut edges,
            &mut global_beams,
            w,
            params.temporal_include_notes_without_mtime,
            &mut warnings,
        );
    }
    apply_edge_recency_channel(&mut edges, &nodes, params, Utc::now());

    let label_inputs: Vec<NodeTitleCluster> = nodes
        .iter()
        .map(|n| NodeTitleCluster {
            cluster_id: n.cluster_id,
            title: n.title.clone(),
        })
        .collect();
    let keyword_pairs = cluster_keyword_labels(&label_inputs);
    let medoids = if !note_embeddings.is_empty() && !cluster_map.is_empty() {
        cluster_medoid_titles_from_embeddings(&cluster_map, &note_embeddings, &path_to_title)
    } else {
        HashMap::new()
    };
    let mut cluster_labels: Vec<(i32, String)> =
        merge_medoid_and_keyword_cluster_labels(medoids, keyword_pairs);

    let full_total = nodes.len() as u32;
    let mut page_meta: Option<BuiltGraphPagination> = None;

    if let Some((offset, limit)) = pagination {
        if limit > 0 && !nodes.is_empty() {
            nodes.sort_by(|a, b| a.abs_path.cmp(&b.abs_path));
            let start = offset as usize;
            if start >= nodes.len() {
                nodes.clear();
                edges.clear();
                global_beams.clear();
                cluster_labels.clear();
            } else {
                let end = (start + limit as usize).min(nodes.len());
                nodes = nodes[start..end].to_vec();
                let path_set: HashSet<String> = nodes.iter().map(|n| n.abs_path.clone()).collect();
                edges.retain(|e| path_set.contains(&e.source) && path_set.contains(&e.target));
                global_beams.retain(|b| {
                    path_set.contains(&b.source_path) && path_set.contains(&b.target_path)
                });
                let cluster_ids: HashSet<i32> = nodes.iter().filter_map(|n| n.cluster_id).collect();
                cluster_labels.retain(|(id, _)| cluster_ids.contains(id));
            }
            page_meta = Some(BuiltGraphPagination {
                total_nodes: full_total,
                offset,
                limit,
                returned_nodes: nodes.len() as u32,
                has_more: offset.saturating_add(limit) < full_total,
            });
        }
    }

    if pr_after_slice && !nodes.is_empty() {
        let paths_page: Vec<String> = nodes.iter().map(|n| n.abs_path.clone()).collect();
        let wiki_pairs = wiki_edge_pairs(&edges);
        let pr_scores = undirected_pagerank(&paths_page, &wiki_pairs, pr_it, d);
        for (node, s) in nodes.iter_mut().zip(pr_scores.into_iter()) {
            node.link_centrality = s;
        }
    }

    log::info!(
        "[KMS][Graph] build_full_graph: vault_note_count={} returned_nodes={} returned_edges={} ai_beams={} warning_count={} paged={} pagerank_scope={:?} semantics_skipped_vaultwide={} duration_ms={}",
        note_count,
        nodes.len(),
        edges.len(),
        global_beams.len(),
        warnings.len(),
        page_meta.is_some(),
        pr_mode,
        semantics_skipped,
        build_started.elapsed().as_millis(),
    );

    KmsDiagnosticService::info(
        &format!(
            "[KMS][Graph] PageRank effective mode {:?} (paginated={})",
            pr_mode,
            page_meta.is_some()
        ),
        None,
    );

    Ok(BuiltFullGraph {
        nodes,
        edges,
        cluster_labels,
        beams: global_beams,
        warnings,
        pagination: page_meta,
    })
}

/// Local subgraph around one indexed note (SQL-scoped BFS + optional clustering + PageRank).
#[derive(Clone, Debug)]
pub struct BuiltLocalGraph {
    pub nodes: Vec<BuiltGraphNode>,
    pub edges: Vec<BuiltGraphEdge>,
    pub cluster_labels: Vec<(i32, String)>,
    pub warnings: Vec<String>,
}

pub fn build_local_graph<E: LoadNoteEmbeddingsPort + ?Sized>(
    vault: &Path,
    center_canonical_rel: &str,
    depth: u32,
    params: &KmsGraphBuildParams,
    embeddings: &E,
) -> Result<BuiltLocalGraph, String> {
    let build_started = std::time::Instant::now();
    let (_visited_abs, local_links_raw, visited_rels) = local_neighborhood_edges_incremental(
        center_canonical_rel,
        depth,
        |rel| abs_path_from_vault(vault, rel),
        |paths| {
            kms_repository::get_links_for_notes(paths).map_err(|e| e.to_string())
        },
    )?;

    let filtered =
        kms_repository::get_notes_minimal_by_paths(&visited_rels).map_err(|e| e.to_string())?;

    let mut warnings: Vec<String> = Vec::new();
    let defer_leiden_local =
        params.enable_leiden_communities && params.enable_semantic_clustering;

    let path_to_title: HashMap<String, String> = filtered
        .iter()
        .map(|n| (n.path.clone(), n.title.clone()))
        .collect();
    let rel_in_hood: HashSet<String> = filtered.iter().map(|n| n.path.clone()).collect();

    let mut local_emb: Vec<(String, Vec<f32>)> = Vec::new();
    let hood_paths: Vec<String> = rel_in_hood.iter().cloned().collect();
    let (mut cluster_map, mut cluster_label_pairs) =
        if params.enable_semantic_clustering || params.enable_semantic_knn_edges {
            match embeddings.load_note_embeddings_for_paths(&hood_paths) {
                Ok(le) => {
                    local_emb = le.clone();
                    if params.enable_semantic_clustering && !defer_leiden_local {
                        cluster_subgraph_from_embeddings(
                            le,
                            &path_to_title,
                            params,
                            |rp| abs_path_from_vault(vault, rp),
                        )
                    } else {
                        (HashMap::new(), Vec::new())
                    }
                }
                Err(e) => {
                    KmsDiagnosticService::warn(
                        "KMS local graph: failed to load note embeddings",
                        Some(e.to_string()),
                    );
                    push_graph_warning(&mut warnings, WARN_SEMANTIC_EMBED_LOAD_FAIL, format!(
                        "Could not load note embeddings; semantic clustering and kNN edges were skipped. ({})",
                        e
                    ));
                    (HashMap::new(), Vec::new())
                }
            }
        } else {
            (HashMap::new(), Vec::new())
        };

    let mut nodes: Vec<BuiltGraphNode> = filtered
        .into_iter()
        .map(|n| {
            let abs_path = abs_path_from_vault(vault, &n.path);
            let (node_type, folder_path) = node_type_and_folder(&n.path);
            BuiltGraphNode {
                id: n.id,
                abs_path,
                vault_rel_path: n.path.clone(),
                title: n.title,
                node_type,
                last_modified: n.last_modified.unwrap_or_default(),
                folder_path,
                cluster_id: cluster_map.get(&n.path).copied(),
                wiki_pagerank_stored: n.wiki_pagerank,
                link_centrality: 0.0,
            }
        })
        .collect();

    let mut edges: Vec<BuiltGraphEdge> = local_links_raw
        .into_iter()
        .map(|(s, t)| BuiltGraphEdge {
            source: abs_path_from_vault(vault, &s),
            target: abs_path_from_vault(vault, &t),
            kind: "wiki".to_string(),
            edge_recency: None,
        })
        .collect();

    if params.enable_semantic_knn_edges && !local_emb.is_empty() {
        let max_e = params.semantic_knn_max_edges as usize;
        if max_e > 0 {
            let k = params.semantic_knn_per_note.max(1) as usize;
            let max_pairs = params.semantic_knn_max_pair_checks as usize;
            let min_sim = params.semantic_knn_min_similarity.clamp(0.0, 1.0);
            let (mut knn_e, knn_budget) = semantic_knn_edges_from_embeddings(
                &local_emb,
                k,
                min_sim,
                max_e,
                max_pairs,
                |p| abs_path_from_vault(vault, p),
            );
            if knn_budget && params.semantic_knn_max_pair_checks > 0 {
                push_graph_warning(&mut warnings, WARN_SEMANTIC_KNN_PAIR_BUDGET, format!(
                    "Semantic kNN (local): stopped after {} cosine comparisons (limit).",
                    params.semantic_knn_max_pair_checks
                ));
            }
            edges.append(&mut knn_e);
        }
    }

    if params.enable_leiden_communities && !nodes.is_empty() {
        match leiden_cluster_map_for_nodes(&nodes, &edges) {
            Ok(map) => {
                cluster_map = map;
                for n in &mut nodes {
                    n.cluster_id = cluster_map
                        .get(&n.vault_rel_path)
                        .cloned()
                        .or_else(|| {
                            cluster_map
                                .get(&norm_vault_rel_path(&n.vault_rel_path))
                                .cloned()
                        });
                }
                if !edges.iter().any(|e| e.kind.as_str() == "semantic_knn") {
                    push_graph_warning(
                        &mut warnings,
                        WARN_LEIDEN_LOCAL_WIKI_ONLY,
                        "Leiden (local graph): wiki links only (no semantic kNN edges). Enable semantic kNN for embedding-based structure."
                            .to_string(),
                    );
                }
                cluster_label_pairs =
                    cluster_labels_from_map_and_embeddings(&cluster_map, &local_emb, &path_to_title);
            }
            Err(e) => {
                push_graph_warning(&mut warnings, WARN_LEIDEN_LOCAL_SKIPPED, format!("Leiden (local graph) skipped: {}", e));
                if defer_leiden_local && !local_emb.is_empty() {
                    let (m, labels) = cluster_subgraph_from_embeddings(
                        local_emb.clone(),
                        &path_to_title,
                        params,
                        |rp| abs_path_from_vault(vault, rp),
                    );
                    cluster_map = m;
                    cluster_label_pairs = labels;
                    for n in &mut nodes {
                        n.cluster_id = cluster_map
                            .get(&n.vault_rel_path)
                            .cloned()
                            .or_else(|| {
                                cluster_map
                                    .get(&norm_vault_rel_path(&n.vault_rel_path))
                                    .cloned()
                            });
                    }
                }
            }
        }
    }

    let mut no_beams: Vec<SemanticBeam> = Vec::new();
    if let Some(w) = resolve_effective_temporal_window(params, &TemporalRpcOverride::default()) {
        apply_temporal_window_filter(
            &mut nodes,
            &mut edges,
            &mut no_beams,
            w,
            params.temporal_include_notes_without_mtime,
            &mut warnings,
        );
    }
    apply_edge_recency_channel(&mut edges, &nodes, params, Utc::now());

    let paths_loc: Vec<String> = nodes.iter().map(|n| n.abs_path.clone()).collect();
    let pr_it = params.pagerank_local_iterations.max(4) as usize;
    let d = params.pagerank_damping.clamp(0.5, 0.99);
    let wiki_pairs = wiki_edge_pairs(&edges);
    let pr_scores = undirected_pagerank(&paths_loc, &wiki_pairs, pr_it, d);
    for (node, s) in nodes.iter_mut().zip(pr_scores.into_iter()) {
        node.link_centrality = s;
    }

    log::info!(
        "[KMS][Graph] build_local_graph: nodes={} edges={} depth={} warning_count={} duration_ms={}",
        nodes.len(),
        edges.len(),
        depth,
        warnings.len(),
        build_started.elapsed().as_millis(),
    );

    Ok(BuiltLocalGraph {
        nodes,
        edges,
        cluster_labels: cluster_label_pairs,
        warnings,
    })
}

fn norm_rel_path(s: &str) -> String {
    s.replace('\\', "/").to_lowercase()
}

/// Undirected PageRank on the wiki link graph (normalized paths as keys). `paths` order must match nodes.
pub fn undirected_pagerank(paths: &[String], edges: &[(String, String)], iterations: usize, d: f32) -> Vec<f32> {
    let n = paths.len();
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![1.0];
    }

    let mut idx: HashMap<String, usize> = HashMap::with_capacity(n);
    for (i, p) in paths.iter().enumerate() {
        idx.insert(norm_rel_path(p), i);
    }

    let mut adj: Vec<Vec<usize>> = vec![vec![]; n];
    for (a, b) in edges {
        let Some(&ia) = idx.get(&norm_rel_path(a)) else {
            continue;
        };
        let Some(&ib) = idx.get(&norm_rel_path(b)) else {
            continue;
        };
        if ia != ib {
            adj[ia].push(ib);
            adj[ib].push(ia);
        }
    }
    for v in &mut adj {
        v.sort_unstable();
        v.dedup();
    }

    let deg: Vec<usize> = adj.iter().map(|v| v.len()).collect();

    let mut pr = vec![1.0f32 / n as f32; n];
    let inv_n = 1.0f32 / n as f32;
    let one_minus_d = 1.0f32 - d;

    for _ in 0..iterations {
        let mut dangling_sum = 0.0f32;
        for j in 0..n {
            if deg[j] == 0 {
                dangling_sum += pr[j];
            }
        }
        let dangling_term = d * dangling_sum * inv_n;

        let mut next = vec![0.0f32; n];
        for i in 0..n {
            let mut s = one_minus_d * inv_n + dangling_term;
            for &j in &adj[i] {
                let dj = deg[j] as f32;
                if dj > 0.0 {
                    s += d * pr[j] / dj;
                }
            }
            next[i] = s;
        }
        pr = next;
    }

    let max_pr = pr.iter().cloned().fold(0.0f32, f32::max);
    if max_pr > 1e-12 {
        for x in &mut pr {
            *x /= max_pr;
        }
    }
    pr
}

/// Shortest path between two vault-relative note paths using **undirected** wiki links.
/// Returns ordered vault-relative paths from `from_rel` to `to_rel`, inclusive.
#[allow(dead_code)]
pub fn shortest_path_undirected_wiki(
    all_links: &[(String, String)],
    from_rel: &str,
    to_rel: &str,
) -> Option<Vec<String>> {
    shortest_path_undirected_wiki_with_budget(all_links, from_rel, to_rel, None).chain_rel
}

#[derive(Clone, Debug, Default)]
pub struct ShortestPathBudgetResult {
    pub chain_rel: Option<Vec<String>>,
    pub visited_nodes: usize,
    pub budget_exhausted: bool,
}

/// Budget-aware shortest path traversal over undirected wiki links.
/// `max_visited_nodes = None` means no guardrail budget.
pub fn shortest_path_undirected_wiki_with_budget(
    all_links: &[(String, String)],
    from_rel: &str,
    to_rel: &str,
    max_visited_nodes: Option<usize>,
) -> ShortestPathBudgetResult {
    let mut out = ShortestPathBudgetResult::default();
    let start = norm_rel_path(from_rel.trim());
    let end = norm_rel_path(to_rel.trim());
    if start.is_empty() || end.is_empty() {
        return out;
    }
    if start == end {
        out.chain_rel = Some(vec![from_rel.trim().to_string()]);
        out.visited_nodes = 1;
        return out;
    }

    let mut canon: HashMap<String, String> = HashMap::new();
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();

    for (a, b) in all_links {
        let na = norm_rel_path(a);
        let nb = norm_rel_path(b);
        canon.entry(na.clone()).or_insert_with(|| a.clone());
        canon.entry(nb.clone()).or_insert_with(|| b.clone());
        adj.entry(na.clone()).or_default().push(b.clone());
        adj.entry(nb.clone()).or_default().push(a.clone());
    }

    canon
        .entry(start.clone())
        .or_insert_with(|| from_rel.trim().to_string());
    canon
        .entry(end.clone())
        .or_insert_with(|| to_rel.trim().to_string());

    if !adj.contains_key(&start) || !adj.contains_key(&end) {
        return out;
    }

    let mut visited: HashSet<String> = HashSet::new();
    let mut parent: HashMap<String, String> = HashMap::new();
    let mut queue: VecDeque<String> = VecDeque::new();

    visited.insert(start.clone());
    queue.push_back(start.clone());
    out.visited_nodes = 1;

    while let Some(u) = queue.pop_front() {
        if u == end {
            let mut chain_norm: Vec<String> = Vec::new();
            let mut cur = end.clone();
            loop {
                chain_norm.push(cur.clone());
                if cur == start {
                    break;
                }
                let Some(prev) = parent.get(&cur) else {
                    out.visited_nodes = visited.len();
                    return out;
                };
                cur = prev.clone();
            }
            chain_norm.reverse();
            out.chain_rel = Some(
                chain_norm
                    .into_iter()
                    .map(|k| canon.get(&k).cloned().unwrap_or(k))
                    .collect(),
            );
            out.visited_nodes = visited.len();
            return out;
        }

        let neighbors = adj.get(&u).cloned().unwrap_or_default();
        for nbr_rel in neighbors {
            let vn = norm_rel_path(&nbr_rel);
            if visited.insert(vn.clone()) {
                if let Some(max_nodes) = max_visited_nodes {
                    if visited.len() > max_nodes {
                        out.visited_nodes = visited.len();
                        out.budget_exhausted = true;
                        return out;
                    }
                }
                parent.insert(vn.clone(), u.clone());
                queue.push_back(vn);
            }
        }
    }

    out.visited_nodes = visited.len();
    out
}

/// Stable key for per-vault graph override map (canonical path, lowercase on Windows).
pub fn vault_graph_settings_key(vault_root: &Path) -> String {
    let canon = std::fs::canonicalize(vault_root).unwrap_or_else(|_| vault_root.to_path_buf());
    let s = canon.to_string_lossy().replace('\\', "/");
    #[cfg(windows)]
    {
        s.to_lowercase()
    }
    #[cfg(not(windows))]
    {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use digicore_kms_ports::LoadNoteEmbeddingsPort;

    struct EmptyEmbeddings;
    impl LoadNoteEmbeddingsPort for EmptyEmbeddings {
        fn load_all_note_embeddings(
            &self,
        ) -> digicore_kms_ports::GraphLoadResult<Vec<(String, Vec<f32>)>> {
            Ok(vec![])
        }

        fn load_note_embeddings_for_paths(
            &self,
            _paths: &[String],
        ) -> digicore_kms_ports::GraphLoadResult<Vec<(String, Vec<f32>)>> {
            Ok(vec![])
        }
    }

    #[test]
    fn shortest_path_direct_edge() {
        let links = vec![("a.md".to_string(), "b.md".to_string())];
        let p = shortest_path_undirected_wiki(&links, "a.md", "b.md").unwrap();
        assert_eq!(p, vec!["a.md", "b.md"]);
    }

    #[test]
    fn shortest_path_chain() {
        let links = vec![
            ("a.md".to_string(), "b.md".to_string()),
            ("b.md".to_string(), "c.md".to_string()),
        ];
        let p = shortest_path_undirected_wiki(&links, "a.md", "c.md").unwrap();
        assert_eq!(p, vec!["a.md", "b.md", "c.md"]);
    }

    #[test]
    fn shortest_path_none_when_disconnected() {
        let links = vec![("a.md".to_string(), "b.md".to_string())];
        assert!(shortest_path_undirected_wiki(&links, "a.md", "z.md").is_none());
    }

    #[test]
    fn pagerank_chain_middle_not_less_than_endpoints() {
        let paths = vec![
            "/v/a.md".to_string(),
            "/v/b.md".to_string(),
            "/v/c.md".to_string(),
        ];
        let edges = vec![
            (paths[0].clone(), paths[1].clone()),
            (paths[1].clone(), paths[2].clone()),
        ];
        let pr = undirected_pagerank(&paths, &edges, 64, 0.85);
        assert_eq!(pr.len(), 3);
        // Middle of a path has at least as much mass as endpoints in an undirected chain.
        assert!(pr[1] + 1e-6 >= pr[0]);
        assert!(pr[1] + 1e-6 >= pr[2]);
    }

    #[test]
    fn pagerank_single_node_is_one() {
        let pr = undirected_pagerank(&["only.md".to_string()], &[], 10, 0.85);
        assert_eq!(pr, vec![1.0]);
    }

    #[test]
    fn leiden_two_components_gives_two_communities() {
        let nodes = vec![
            BuiltGraphNode {
                id: 1,
                abs_path: "/vault/a.md".into(),
                vault_rel_path: "a.md".into(),
                title: String::new(),
                node_type: "note".into(),
                last_modified: String::new(),
                folder_path: String::new(),
                cluster_id: None,
                wiki_pagerank_stored: None,
                link_centrality: 0.0,
            },
            BuiltGraphNode {
                id: 2,
                abs_path: "/vault/b.md".into(),
                vault_rel_path: "b.md".into(),
                title: String::new(),
                node_type: "note".into(),
                last_modified: String::new(),
                folder_path: String::new(),
                cluster_id: None,
                wiki_pagerank_stored: None,
                link_centrality: 0.0,
            },
            BuiltGraphNode {
                id: 3,
                abs_path: "/vault/c.md".into(),
                vault_rel_path: "c.md".into(),
                title: String::new(),
                node_type: "note".into(),
                last_modified: String::new(),
                folder_path: String::new(),
                cluster_id: None,
                wiki_pagerank_stored: None,
                link_centrality: 0.0,
            },
            BuiltGraphNode {
                id: 4,
                abs_path: "/vault/d.md".into(),
                vault_rel_path: "d.md".into(),
                title: String::new(),
                node_type: "note".into(),
                last_modified: String::new(),
                folder_path: String::new(),
                cluster_id: None,
                wiki_pagerank_stored: None,
                link_centrality: 0.0,
            },
        ];
        let edges = vec![
            BuiltGraphEdge {
                source: "/vault/a.md".into(),
                target: "/vault/b.md".into(),
                kind: "wiki".into(),
                edge_recency: None,
            },
            BuiltGraphEdge {
                source: "/vault/c.md".into(),
                target: "/vault/d.md".into(),
                kind: "wiki".into(),
                edge_recency: None,
            },
        ];
        let map = leiden_cluster_map_for_nodes(&nodes, &edges).expect("leiden");
        assert_eq!(map.get("a.md"), map.get("b.md"));
        assert_eq!(map.get("c.md"), map.get("d.md"));
        assert_ne!(map.get("a.md"), map.get("c.md"));
    }

    #[test]
    fn merge_cluster_labels_prefers_medoid_title() {
        let mut medoids = HashMap::new();
        medoids.insert(2, "Embedding Medoid Title".to_string());
        let kw = vec![(2, "Keyword Fallback".to_string())];
        let merged = merge_medoid_and_keyword_cluster_labels(medoids, kw);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].1, "Embedding Medoid Title");
    }

    #[test]
    fn resolve_pagerank_scope_auto_uses_page_subgraph_when_paged() {
        assert_eq!(
            resolve_pagerank_scope("auto", true),
            KmsPagerankScopeMode::PageSubgraph
        );
        assert_eq!(
            resolve_pagerank_scope("auto", false),
            KmsPagerankScopeMode::FullVault
        );
    }

    #[test]
    fn resolve_pagerank_scope_explicit_modes() {
        assert_eq!(
            resolve_pagerank_scope("full_vault", true),
            KmsPagerankScopeMode::FullVault
        );
        assert_eq!(
            resolve_pagerank_scope("page_subgraph", false),
            KmsPagerankScopeMode::PageSubgraph
        );
        assert_eq!(resolve_pagerank_scope("off", true), KmsPagerankScopeMode::Off);
    }

    #[test]
    fn cluster_medoid_picks_closest_to_centroid() {
        let mut cluster_map = HashMap::new();
        cluster_map.insert("a.md".to_string(), 0);
        cluster_map.insert("b.md".to_string(), 0);
        let emb = vec![
            ("a.md".to_string(), vec![0.2f32, 0.8f32, 0.0]),
            ("b.md".to_string(), vec![0.95f32, 0.05f32, 0.0]),
        ];
        let mut titles = HashMap::new();
        titles.insert("a.md".to_string(), "Off Centroid".to_string());
        titles.insert("b.md".to_string(), "Medoid Title".to_string());
        let m = cluster_medoid_titles_from_embeddings(&cluster_map, &emb, &titles);
        assert_eq!(m.get(&0).map(String::as_str), Some("Medoid Title"));
    }

    #[test]
    fn cluster_subgraph_local_neighborhood_labels() {
        let emb = vec![
            ("n1.md".to_string(), vec![1.0f32, 0.0, 0.0]),
            ("n2.md".to_string(), vec![0.95f32, 0.05, 0.0]),
            ("n3.md".to_string(), vec![0.0f32, 1.0, 0.0]),
        ];
        let mut titles = HashMap::new();
        titles.insert("n1.md".to_string(), "Alpha One".to_string());
        titles.insert("n2.md".to_string(), "Alpha Two".to_string());
        titles.insert("n3.md".to_string(), "Beta One".to_string());
        let params = KmsGraphBuildParams {
            enable_semantic_clustering: true,
            enable_leiden_communities: false,
            enable_ai_beams: false,
            k_means_max_k: 8,
            k_means_iterations: 10,
            ai_beam_max_nodes: 50,
            ai_beam_similarity_threshold: 0.9,
            ai_beam_max_edges: 5,
            semantic_max_notes: 0,
            warn_note_threshold: 0,
            beam_max_pair_checks: 0,
            pagerank_iterations: 48,
            pagerank_local_iterations: 32,
            pagerank_damping: 0.85,
            pagerank_scope: "auto".to_string(),
            background_wiki_pagerank_enabled: true,
            enable_semantic_knn_edges: false,
            semantic_knn_per_note: 5,
            semantic_knn_min_similarity: 0.82,
            semantic_knn_max_edges: 0,
            semantic_knn_max_pair_checks: 0,
            temporal_window_enabled: false,
            temporal_default_days: 0,
            temporal_include_notes_without_mtime: true,
            temporal_edge_recency_enabled: false,
            temporal_edge_recency_strength: 1.0,
            temporal_edge_recency_half_life_days: 30.0,
        };
        let (map, labels) =
            cluster_subgraph_from_embeddings(emb, &titles, &params, |p| format!("/v/{p}"));
        assert!(map.contains_key("n1.md"));
        assert!(!labels.is_empty());
    }

    fn test_vault_abs(rel: &str) -> String {
        format!("/vault/{}", rel.replace('\\', "/"))
    }

    #[test]
    fn local_neighborhood_dedupes_duplicate_directed_rows() {
        let links = vec![
            ("a.md".to_string(), "b.md".to_string()),
            ("a.md".to_string(), "b.md".to_string()),
        ];
        let (_, edges) = local_neighborhood_edges(&links, test_vault_abs, "a.md", 1);
        assert_eq!(edges.len(), 1);
    }

    #[test]
    fn incremental_bfs_matches_full_neighborhood_for_chain() {
        let links = vec![
            ("a.md".to_string(), "b.md".to_string()),
            ("b.md".to_string(), "c.md".to_string()),
        ];
        let (v_full, e_full) = local_neighborhood_edges(&links, test_vault_abs, "a.md", 2);
        let links_clone = links.clone();
        let fetch = move |paths: &[String]| {
            let mut out = Vec::new();
            for (s, t) in &links_clone {
                for p in paths {
                    if s == p || t == p {
                        out.push((s.clone(), t.clone()));
                    }
                }
            }
            Ok(out)
        };
        let (v_inc, e_inc, _) =
            local_neighborhood_edges_incremental("a.md", 2, test_vault_abs, fetch).unwrap();
        assert_eq!(v_full, v_inc);
        let mut ae: Vec<_> = e_full.iter().cloned().collect();
        let mut be: Vec<_> = e_inc.iter().cloned().collect();
        ae.sort();
        be.sort();
        assert_eq!(ae, be);
    }

    #[test]
    fn build_full_graph_from_notes_respects_pagination_meta() {
        let vault = Path::new("/vault");
        let notes = vec![
            KmsNoteMinimal {
                id: 1,
                path: "a.md".into(),
                title: "A".into(),
                last_modified: None,
                wiki_pagerank: None,
            },
            KmsNoteMinimal {
                id: 2,
                path: "b.md".into(),
                title: "B".into(),
                last_modified: None,
                wiki_pagerank: None,
            },
            KmsNoteMinimal {
                id: 3,
                path: "c.md".into(),
                title: "C".into(),
                last_modified: None,
                wiki_pagerank: None,
            },
        ];
        let links = vec![
            ("a.md".to_string(), "b.md".to_string()),
            ("b.md".to_string(), "c.md".to_string()),
        ];
        let params = KmsGraphBuildParams {
            enable_semantic_clustering: false,
            enable_leiden_communities: false,
            enable_ai_beams: false,
            k_means_max_k: 4,
            k_means_iterations: 5,
            ai_beam_max_nodes: 10,
            ai_beam_similarity_threshold: 0.9,
            ai_beam_max_edges: 5,
            semantic_max_notes: 0,
            warn_note_threshold: 0,
            beam_max_pair_checks: 0,
            pagerank_iterations: 16,
            pagerank_local_iterations: 16,
            pagerank_damping: 0.85,
            pagerank_scope: "auto".to_string(),
            background_wiki_pagerank_enabled: true,
            enable_semantic_knn_edges: false,
            semantic_knn_per_note: 5,
            semantic_knn_min_similarity: 0.82,
            semantic_knn_max_edges: 0,
            semantic_knn_max_pair_checks: 0,
            temporal_window_enabled: false,
            temporal_default_days: 0,
            temporal_include_notes_without_mtime: true,
            temporal_edge_recency_enabled: false,
            temporal_edge_recency_strength: 1.0,
            temporal_edge_recency_half_life_days: 30.0,
        };
        let built = build_full_graph_from_notes_and_links(
            vault,
            notes,
            links,
            &params,
            Some((0, 2)),
            &EmptyEmbeddings,
            &TemporalRpcOverride::default(),
        )
        .expect("graph build");
        let page = built.pagination.expect("pagination meta");
        assert_eq!(page.total_nodes, 3);
        assert_eq!(page.returned_nodes, 2);
        assert!(page.has_more);
        assert_eq!(built.nodes.len(), 2);
        assert_eq!(built.edges.len(), 1);
    }

    #[test]
    fn build_full_graph_leiden_wiki_two_components_no_db() {
        let vault = Path::new("/vault");
        let notes = vec![
            KmsNoteMinimal {
                id: 1,
                path: "a.md".into(),
                title: "A".into(),
                last_modified: None,
                wiki_pagerank: None,
            },
            KmsNoteMinimal {
                id: 2,
                path: "b.md".into(),
                title: "B".into(),
                last_modified: None,
                wiki_pagerank: None,
            },
            KmsNoteMinimal {
                id: 3,
                path: "c.md".into(),
                title: "C".into(),
                last_modified: None,
                wiki_pagerank: None,
            },
            KmsNoteMinimal {
                id: 4,
                path: "d.md".into(),
                title: "D".into(),
                last_modified: None,
                wiki_pagerank: None,
            },
        ];
        let links = vec![
            ("a.md".to_string(), "b.md".to_string()),
            ("c.md".to_string(), "d.md".to_string()),
        ];
        let params = KmsGraphBuildParams {
            enable_semantic_clustering: false,
            enable_leiden_communities: true,
            enable_ai_beams: false,
            k_means_max_k: 4,
            k_means_iterations: 5,
            ai_beam_max_nodes: 10,
            ai_beam_similarity_threshold: 0.9,
            ai_beam_max_edges: 5,
            semantic_max_notes: 0,
            warn_note_threshold: 0,
            beam_max_pair_checks: 0,
            pagerank_iterations: 16,
            pagerank_local_iterations: 16,
            pagerank_damping: 0.85,
            pagerank_scope: "off".to_string(),
            background_wiki_pagerank_enabled: true,
            enable_semantic_knn_edges: false,
            semantic_knn_per_note: 5,
            semantic_knn_min_similarity: 0.82,
            semantic_knn_max_edges: 0,
            semantic_knn_max_pair_checks: 0,
            temporal_window_enabled: false,
            temporal_default_days: 0,
            temporal_include_notes_without_mtime: true,
            temporal_edge_recency_enabled: false,
            temporal_edge_recency_strength: 1.0,
            temporal_edge_recency_half_life_days: 30.0,
        };
        let built = build_full_graph_from_notes_and_links(
            vault,
            notes,
            links,
            &params,
            None,
            &EmptyEmbeddings,
            &TemporalRpcOverride::default(),
        )
        .expect("build");
        assert_eq!(built.nodes.len(), 4);
        let cid = |rel: &str| {
            built
                .nodes
                .iter()
                .find(|n| n.vault_rel_path == rel)
                .expect("node")
                .cluster_id
        };
        assert_eq!(cid("a.md"), cid("b.md"));
        assert_eq!(cid("c.md"), cid("d.md"));
        assert_ne!(cid("a.md"), cid("c.md"));
        assert!(built.warnings.iter().any(|w| w.contains("wiki links only")));
    }

    /// Temp SQLite + real `kms_notes` / `kms_links` rows (separate from synthetic `build_full_graph_from_notes` tests).
    #[test]
    fn repo_backed_full_graph_pagination_and_shortest_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("kms_graph_integration.sqlite");
        kms_repository::init(db_path).expect("kms init");
        kms_repository::ensure_kms_notes_links_schema().expect("schema");

        kms_repository::upsert_note("a.md", "A", "", "indexed", None, &[]).unwrap();
        kms_repository::upsert_note("b.md", "B", "", "indexed", None, &[]).unwrap();
        kms_repository::upsert_note("c.md", "C", "", "indexed", None, &[]).unwrap();
        kms_repository::upsert_link("a.md", "b.md").unwrap();
        kms_repository::upsert_link("b.md", "c.md").unwrap();

        let vault = dir.path();
        let params = KmsGraphBuildParams {
            enable_semantic_clustering: false,
            enable_leiden_communities: false,
            enable_ai_beams: false,
            k_means_max_k: 4,
            k_means_iterations: 5,
            ai_beam_max_nodes: 10,
            ai_beam_similarity_threshold: 0.9,
            ai_beam_max_edges: 5,
            semantic_max_notes: 0,
            warn_note_threshold: 0,
            beam_max_pair_checks: 0,
            pagerank_iterations: 16,
            pagerank_local_iterations: 16,
            pagerank_damping: 0.85,
            pagerank_scope: "auto".to_string(),
            background_wiki_pagerank_enabled: true,
            enable_semantic_knn_edges: false,
            semantic_knn_per_note: 5,
            semantic_knn_min_similarity: 0.82,
            semantic_knn_max_edges: 0,
            semantic_knn_max_pair_checks: 0,
            temporal_window_enabled: false,
            temporal_default_days: 0,
            temporal_include_notes_without_mtime: true,
            temporal_edge_recency_enabled: false,
            temporal_edge_recency_strength: 1.0,
            temporal_edge_recency_half_life_days: 30.0,
        };

        let built = build_full_graph(vault, &params, Some((0, 2))).expect("build_full_graph");
        let page = built.pagination.expect("pagination");
        assert_eq!(page.total_nodes, 3);
        assert_eq!(page.returned_nodes, 2);

        let links = kms_repository::get_all_links().expect("links");
        let chain = shortest_path_undirected_wiki(&links, "a.md", "c.md").expect("path");
        assert_eq!(chain, vec!["a.md", "b.md", "c.md"]);
    }

    #[test]
    fn wiki_link_graph_fingerprint_is_order_independent() {
        let a = vec![
            ("x".to_string(), "y".to_string()),
            ("m".to_string(), "n".to_string()),
        ];
        let b = vec![
            ("n".to_string(), "m".to_string()),
            ("y".to_string(), "x".to_string()),
        ];
        assert_eq!(wiki_link_graph_fingerprint(&a), wiki_link_graph_fingerprint(&b));
    }

    #[test]
    fn materialized_wiki_pagerank_persists_and_reuses() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("kms_pr_materialized.sqlite");
        kms_repository::init(db_path).expect("kms init");
        kms_repository::ensure_kms_notes_links_schema().expect("schema");

        kms_repository::upsert_note("a.md", "A", "", "indexed", None, &[]).unwrap();
        kms_repository::upsert_note("b.md", "B", "", "indexed", None, &[]).unwrap();
        kms_repository::upsert_note("c.md", "C", "", "indexed", None, &[]).unwrap();
        kms_repository::upsert_link("a.md", "b.md").unwrap();
        kms_repository::upsert_link("b.md", "c.md").unwrap();

        let vault = dir.path();
        let params = KmsGraphBuildParams {
            enable_semantic_clustering: false,
            enable_leiden_communities: false,
            enable_ai_beams: false,
            k_means_max_k: 4,
            k_means_iterations: 5,
            ai_beam_max_nodes: 10,
            ai_beam_similarity_threshold: 0.9,
            ai_beam_max_edges: 5,
            semantic_max_notes: 0,
            warn_note_threshold: 0,
            beam_max_pair_checks: 0,
            pagerank_iterations: 16,
            pagerank_local_iterations: 16,
            pagerank_damping: 0.85,
            pagerank_scope: "full_vault".to_string(),
            background_wiki_pagerank_enabled: true,
            enable_semantic_knn_edges: false,
            semantic_knn_per_note: 5,
            semantic_knn_min_similarity: 0.82,
            semantic_knn_max_edges: 0,
            semantic_knn_max_pair_checks: 0,
            temporal_window_enabled: false,
            temporal_default_days: 0,
            temporal_include_notes_without_mtime: true,
            temporal_edge_recency_enabled: false,
            temporal_edge_recency_strength: 1.0,
            temporal_edge_recency_half_life_days: 30.0,
        };

        let b1 = build_full_graph(vault, &params, None).expect("build1");
        assert_eq!(b1.nodes.len(), 3);
        let notes = kms_repository::get_all_notes_minimal().expect("notes");
        assert!(notes.iter().all(|n| n.wiki_pagerank.is_some()));
        assert!(
            kms_repository::kms_graph_meta_get(kms_repository::KMS_GRAPH_META_WIKI_PR_FP)
                .expect("meta")
                .is_some()
        );

        let b2 = build_full_graph(vault, &params, None).expect("build2");
        let mut c1: Vec<(String, f32)> = b1
            .nodes
            .iter()
            .map(|n| (n.abs_path.clone(), n.link_centrality))
            .collect();
        let mut c2: Vec<(String, f32)> = b2
            .nodes
            .iter()
            .map(|n| (n.abs_path.clone(), n.link_centrality))
            .collect();
        c1.sort_by(|a, b| a.0.cmp(&b.0));
        c2.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(c1.len(), c2.len());
        for ((p1, s1), (p2, s2)) in c1.iter().zip(c2.iter()) {
            assert_eq!(p1, p2);
            assert!((s1 - s2).abs() < 1e-5, "centrality mismatch for {}", p1);
        }
    }
}
