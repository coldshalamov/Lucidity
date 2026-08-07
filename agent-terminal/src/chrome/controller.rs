//! Bridges LucidityShell (egui) to WezTerm's ProductUiController host.

use crate::ui_bridge::{shell_action_to_command, UiBridge};
use lucidity_ui::LucidityShell;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use wezterm_gui::{
    ProductLayoutSpec, ProductUiController, ProductUiFrame, ProductUiResponse, TerminalVisibility,
};

pub struct LucidityUiController {
    shell: LucidityShell,
    bridge: Arc<UiBridge>,
    command_tx: Sender<crate::ui_bridge::UiCommand>,
}

impl LucidityUiController {
    pub fn new(bridge: Arc<UiBridge>) -> Self {
        let command_tx = bridge.command_sender();
        let mut shell = LucidityShell::new();
        shell.publish_snapshot(bridge.snapshot());
        Self {
            shell,
            bridge,
            command_tx,
        }
    }
}

impl ProductUiController for LucidityUiController {
    fn layout_spec(&self) -> ProductLayoutSpec {
        let layout = self.shell.layout_spec();
        ProductLayoutSpec {
            sidebar_width_points: layout.sidebar_width_points,
            header_height_points: layout.header_height_points,
            bottom_bar_height_points: layout.bottom_bar_height_points,
            terminal_visibility: if layout.terminal_visible {
                TerminalVisibility::Visible
            } else {
                TerminalVisibility::CoveredByOpaqueUi
            },
        }
    }

    fn show(&mut self, ctx: &egui::Context, _frame: &ProductUiFrame) -> ProductUiResponse {
        // Pull latest snapshot each frame (Arc swap; no SQLite).
        self.shell.publish_snapshot(self.bridge.snapshot());
        let response = self.shell.ui(ctx);
        for action in response.actions {
            if matches!(action, lucidity_ui::ShellAction::PickProjectFolder) {
                if let Some(path) = native_folder_picker() {
                    self.shell.set_picked_project(path);
                }
                continue;
            }
            let _ = self.command_tx.send(shell_action_to_command(action));
            self.bridge.wake();
        }
        ProductUiResponse {
            layout_changed: response.layout_changed,
            terminal_focus_requested: response.terminal_focus_requested,
            repaint_after: response
                .repaint_after_secs
                .map(|s| std::time::Duration::from_secs_f32(s)),
            wants_pointer_input: ctx.wants_pointer_input(),
            wants_keyboard_input: ctx.wants_keyboard_input(),
        }
    }

    fn on_host_tick(&mut self) {
        self.shell.publish_snapshot(self.bridge.snapshot());
    }
}

fn native_folder_picker() -> Option<String> {
    #[cfg(windows)]
    {
        // Minimal PowerShell folder browser — avoids extra GUI crates.
        let output = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.FolderBrowserDialog; $d.Description = 'Select project folder'; if ($d.ShowDialog() -eq 'OK') { $d.SelectedPath }",
            ])
            .output()
            .ok()?;
        let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if path.is_empty() {
            None
        } else {
            Some(path)
        }
    }
    #[cfg(not(windows))]
    {
        None
    }
}
