//! EguiWindowAdapter - implements WindowPort using egui::Context.
//!
//! Created each frame in update() with EguiWindowAdapter::new(ctx).
//! Translates ViewportCommand to egui::ViewportCommand.

use crate::ports::{ViewportCommand, WindowLevel, WindowPort};
use egui::ViewportId;

/// Adapter that translates WindowPort calls to egui viewport commands.
///
/// Holds a reference to egui::Context. Create each frame: EguiWindowAdapter::new(ctx).
pub struct EguiWindowAdapter<'a> {
    ctx: &'a egui::Context,
}

impl<'a> EguiWindowAdapter<'a> {
    /// Create adapter for this frame. Pass egui context from update().
    pub fn new(ctx: &'a egui::Context) -> Self {
        Self { ctx }
    }

    fn viewport_id(id: &str) -> ViewportId {
        ViewportId::from_hash_of(id)
    }
}

impl WindowPort for EguiWindowAdapter<'_> {
    fn close_viewport(&self, id: &str) {
        self.ctx
            .send_viewport_cmd_to(Self::viewport_id(id), egui::ViewportCommand::Close);
    }

    fn send_viewport_command(&self, id: &str, cmd: ViewportCommand) {
        let vid = Self::viewport_id(id);
        match cmd {
            ViewportCommand::Visible(b) => {
                self.ctx.send_viewport_cmd_to(vid, egui::ViewportCommand::Visible(b));
            }
            ViewportCommand::Minimized(b) => {
                self.ctx.send_viewport_cmd_to(vid, egui::ViewportCommand::Minimized(b));
            }
            ViewportCommand::Maximized(b) => {
                self.ctx.send_viewport_cmd_to(vid, egui::ViewportCommand::Maximized(b));
            }
            ViewportCommand::Focus => {
                self.ctx.send_viewport_cmd_to(vid, egui::ViewportCommand::Focus);
            }
            ViewportCommand::Close => {
                self.ctx.send_viewport_cmd_to(vid, egui::ViewportCommand::Close);
            }
            ViewportCommand::WindowLevel(level) => {
                let egui_level = match level {
                    WindowLevel::Normal => egui::WindowLevel::Normal,
                    WindowLevel::AlwaysOnTop => egui::WindowLevel::AlwaysOnTop,
                };
                self.ctx
                    .send_viewport_cmd_to(vid, egui::ViewportCommand::WindowLevel(egui_level));
            }
        }
    }
}
