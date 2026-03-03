//! EguiTimerAdapter - TimerPort using egui::Context.
//!
//! Create each frame with EguiTimerAdapter::new(ctx).

use crate::ports::TimerPort;
use std::time::Duration;

/// Adapter that wraps egui's request_repaint_after.
pub struct EguiTimerAdapter<'a> {
    ctx: &'a egui::Context,
}

impl<'a> EguiTimerAdapter<'a> {
    pub fn new(ctx: &'a egui::Context) -> Self {
        Self { ctx }
    }
}

impl TimerPort for EguiTimerAdapter<'_> {
    fn schedule_repaint_after(&self, duration: Duration) {
        self.ctx.request_repaint_after(duration);
    }
}
