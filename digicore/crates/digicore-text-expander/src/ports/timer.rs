//! TimerPort - framework-agnostic repaint/timer scheduling.
//!
//! Part of Phase 2 UI decoupling. Abstracts debounce and repaint scheduling.
//!
//! Implementations: EguiTimerAdapter (ctx.request_repaint_after), TauriTimerAdapter (channel).

use std::time::Duration;

/// Port for scheduling repaints or timers.
///
/// Used for debounce (e.g. Ghost Suggestor) and delayed UI updates.
/// Adapter is created per-frame with framework context (e.g. egui::Context).
pub trait TimerPort {
    /// Schedule a repaint after the given duration.
    fn schedule_repaint_after(&self, duration: Duration);
}
