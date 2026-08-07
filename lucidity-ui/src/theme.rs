//! Desktop-proportional theme tokens (not terminal aesthetics).

#[derive(Clone, Copy, Debug)]
pub struct LucidityTheme {
    pub sidebar_bg: egui::Color32,
    pub content_bg: egui::Color32,
    pub panel_bg: egui::Color32,
    pub row_hover: egui::Color32,
    pub row_selected: egui::Color32,
    pub text_primary: egui::Color32,
    pub text_secondary: egui::Color32,
    pub accent: egui::Color32,
    pub danger: egui::Color32,
    pub success: egui::Color32,
    pub warning: egui::Color32,
    pub border: egui::Color32,
    pub corner_radius: f32,
}

impl Default for LucidityTheme {
    fn default() -> Self {
        Self {
            sidebar_bg: egui::Color32::from_rgb(28, 30, 34),
            content_bg: egui::Color32::from_rgb(18, 19, 22),
            panel_bg: egui::Color32::from_rgb(34, 36, 41),
            row_hover: egui::Color32::from_rgb(42, 45, 52),
            row_selected: egui::Color32::from_rgb(48, 70, 110),
            text_primary: egui::Color32::from_rgb(236, 238, 242),
            text_secondary: egui::Color32::from_rgb(156, 163, 175),
            accent: egui::Color32::from_rgb(88, 166, 255),
            danger: egui::Color32::from_rgb(248, 113, 113),
            success: egui::Color32::from_rgb(74, 222, 128),
            warning: egui::Color32::from_rgb(251, 191, 36),
            border: egui::Color32::from_rgb(55, 58, 66),
            corner_radius: 8.0,
        }
    }
}

impl LucidityTheme {
    pub fn apply(&self, ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(10.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        style.visuals.window_fill = self.content_bg;
        style.visuals.panel_fill = self.sidebar_bg;
        style.visuals.override_text_color = Some(self.text_primary);
        style.visuals.widgets.inactive.bg_fill = self.panel_bg;
        style.visuals.widgets.hovered.bg_fill = self.row_hover;
        style.visuals.widgets.active.bg_fill = self.row_selected;
        style.visuals.selection.bg_fill = self.row_selected;
        style.visuals.widgets.noninteractive.bg_stroke =
            egui::Stroke::new(1.0, self.border);
        ctx.set_style(style);
    }

    pub fn status_color(self, badge: crate::RuntimeBadge) -> egui::Color32 {
        match badge {
            crate::RuntimeBadge::NotRunning => self.text_secondary,
            crate::RuntimeBadge::Starting => self.accent,
            crate::RuntimeBadge::Working => self.success,
            crate::RuntimeBadge::WaitingForInput => self.warning,
            crate::RuntimeBadge::AwaitingApproval => self.warning,
            crate::RuntimeBadge::CompletedIdle => self.text_secondary,
            crate::RuntimeBadge::Failed => self.danger,
            crate::RuntimeBadge::UnknownExternal => self.text_secondary,
        }
    }
}
