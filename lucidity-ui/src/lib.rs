//! Lucidity desktop shell — real GUI controls, not terminal-painted labels.

mod app;
mod model;
mod route;
mod theme;

pub use app::{LucidityShell, ShellAction};
pub use model::{
    AdapterOption, RuntimeBadge, SessionRowView, SettingsDraft, ShellRoute, UiSnapshot, UsageView,
};
pub use route::SettingsPage;
pub use theme::LucidityTheme;

/// Logical layout the terminal host must reserve around the embedded surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShellLayoutSpec {
    pub sidebar_width_points: f32,
    pub header_height_points: f32,
    pub bottom_bar_height_points: f32,
    pub terminal_visible: bool,
}

impl Default for ShellLayoutSpec {
    fn default() -> Self {
        Self {
            sidebar_width_points: 288.0,
            header_height_points: 52.0,
            bottom_bar_height_points: 0.0,
            terminal_visible: true,
        }
    }
}

/// Response from one shell frame.
#[derive(Clone, Debug, Default)]
pub struct ShellFrameResponse {
    pub layout_changed: bool,
    pub terminal_focus_requested: bool,
    pub repaint_after_secs: Option<f32>,
    pub actions: Vec<ShellAction>,
}
