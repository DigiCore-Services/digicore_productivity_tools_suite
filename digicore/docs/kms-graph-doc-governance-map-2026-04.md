# KMS/Graph Documentation Governance Map (2026-04)

## Purpose

This file defines canonical sources, mirrors, and stale/legacy status for KMS and Knowledge Graph documentation. It is the reference used for `P2-H2` governance and drift control.

## Canonical Sources

- `knowledge-graph-comprehensive-audit-and-roadmap-2026-03.md`
  - Canonical technical baseline for graph architecture, roadmap sequencing, and execution queue.
- `kms-notebook-capabilities-audit-and-implementation-plan-2026-04.md`
  - Canonical implementation tracker for the current KMS/notebook execution cycle (P0/P1/P2 progress and checkpoints).

## Authoritative Companion

- `kms-embeddings-temporal-graph-semantic-search-hexagonal-plan-2026-03.md`
  - Authoritative detail for embeddings/semantic-search/seq-12 planning and status.
  - Should not duplicate roadmap queue ownership; must reference canonical roadmap for shared status.

## Mirrors

- `kms-graph-prd-progress.md`
  - Lightweight progress mirror; keep synced with canonical roadmap and current implementation tracker.

## Legacy / Stale-First (Reference Only)

- `kms-knowledge-graph-and-local-graph-audit-2026-03.md`
- `knowledge-graph-features-audit-and-implementation-plan.md`
- `kms_graph_3.0_roadmap.md`

These files remain useful historical context, but should not be treated as primary status authority when conflicts exist.

## Update Rules

- When code changes graph/KMS behavior, update:
  - `kms-notebook-capabilities-audit-and-implementation-plan-2026-04.md` (checkpoint/progress)
  - `knowledge-graph-comprehensive-audit-and-roadmap-2026-03.md` (if roadmap/queue implications changed)
  - `kms-graph-prd-progress.md` (mirror status)
- Add/maintain a top-of-file governance banner in major graph/KMS docs.
- If a document becomes stale, mark it explicitly with `Status: Legacy/Stale-first` and link this governance map.

## Governance Review Metadata

- Last governance review: 2026-04-02
- Owner: DigiCore KMS platform/docs maintainers
- Next review trigger: any roadmap status disagreement or major architecture refactor

