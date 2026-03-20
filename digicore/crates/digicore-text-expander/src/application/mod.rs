//! Application layer - expansion engine orchestration.
//!
//! Phase 0/1: app_state - framework-agnostic application state.

pub mod app_state;
pub mod clipboard_history;
pub mod discovery;
pub mod expansion_diagnostics;
pub mod expansion_engine;
pub mod expansion_stats;
pub mod ghost_follower;
pub mod ghost_suggestor;
#[cfg(feature = "gui-egui")]
pub mod js_syntax_highlighter;
pub mod scripting;
pub mod template_processor;
pub mod variable_input;
pub mod corpus_generator;
pub mod trigger_matcher;

