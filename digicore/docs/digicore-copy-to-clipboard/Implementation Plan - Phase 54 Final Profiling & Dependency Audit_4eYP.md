# Implementation Plan - Phase 54: Final Profiling & Dependency Audit

## Goal Description
As the final phase of the OCR Roadmap v3, the objective is to solidify the application's production readiness. This involves extracting hardcoded heuristic triggers into a flexible configuration layer, auditing our Rust dependencies for bloat or vulnerabilities, profiling the hot paths in our Layout-Aware parsing engine to minimize memory allocations, and finalizing the architectural documentation.

## Proposed Changes

### 1. Configuration-First Architecture (`digicore-core` & `digicore-text-expander`)
- **Runtime YAML Config**: Implement a `_runtime_config.yaml` to manage all configurable extraction items, ensuring hardcoded values serve only as fallbacks.
- **Configurable Heuristics**: 
    - Move table semantic triggers (e.g., `["total", "sum", "subtotal", "balance"]`) into the YAML configuration.
    - Move structural threshold gates (e.g. `significant_gap_gate` multipliers) into the configuration model.
    - Introduce hot-loading or startup-parsing of this YAML file so the OCR engine can be tuned without recompiling.

### 2. Dependency Audit & Optimization (`Cargo.toml`)
- Ensure minimal features are enabled for heavy crates like `image`, `windows`, and `tokio`.
- Verify no conflicting or duplicate dependencies across Workspace members (`digicore-core` vs `digicore-text-expander`).
- Run `cargo clippy` and structural checks to validate optimal borrows and references.

### 3. Hot-Path Profiling (`windows_ocr.rs` & `corpus_generator.rs`)
- **Allocation Reduction**: The layout builder in `windows_ocr.rs` does heavy string concatenation in tight heuristic loops. Review and replace `.clone()` operations with string slices (`&str`) or pre-allocated `String::with_capacity()` where feasible.
- **Async Execution**: Ensure standard blocking operations are appropriately isolated from the `tokio` multi-threaded executor pool to prevent starvation.

### 4. Documentation
- Update the primary `README_Testing_OCR-Text-Extraction-Engine-Regressions.md` to formally document the new Architecture (Hexagonal Ports, Adpaters), Heuristic Fuzzing, semantic Table Detection, Configuration YAMLs, and the "One-Click" Corpus Generation Utility.

## Verification Plan

### Automated Tests
- `cargo check` and `cargo test --all` to ensure no functionality is broken by optimization.
- Check the output of `cargo clippy --workspace -- -D warnings` to verify strict adherence to Rust canonical guidelines.

### Manual Verification
- Edit `_runtime_config.yaml` manually and verify the regression tests reflect the updated heuristic behaviors automatically.
- Manual review of the updated documentation.
