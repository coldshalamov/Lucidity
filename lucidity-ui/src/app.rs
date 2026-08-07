//! Main Lucidity shell drawn with egui.

use crate::model::{AdapterOption, SessionRowView, SettingsDraft, ShellRoute, UiSnapshot};
use crate::route::SettingsPage;
use crate::theme::LucidityTheme;
use crate::{ShellFrameResponse, ShellLayoutSpec};
use egui::{Align, Layout, RichText, ScrollArea, Sense, Ui};
use std::sync::Arc;
use uuid::Uuid;

/// Commands the shell emits for the product host to execute.
#[derive(Clone, Debug, PartialEq)]
pub enum ShellAction {
    NewConversation {
        adapter_id: String,
        project_path: String,
        title: Option<String>,
        extra_args: Option<String>,
    },
    OpenConversation(Uuid),
    Settle(Uuid),
    Unsettle(Uuid),
    Stop(Uuid),
    Resume(Uuid),
    Rename {
        id: Uuid,
        title: String,
    },
    RefreshUsage(Uuid),
    ApplySettings(SettingsDraft),
    PickProjectFolder,
    ImportHistories,
    Quit,
}

#[derive(Clone, Debug)]
struct NewSessionDraft {
    adapter_id: String,
    project_path: String,
    title: String,
    extra_args: String,
}

impl Default for NewSessionDraft {
    fn default() -> Self {
        Self {
            adapter_id: String::new(),
            project_path: String::new(),
            title: String::new(),
            extra_args: String::new(),
        }
    }
}

/// Stateful Lucidity desktop shell.
pub struct LucidityShell {
    theme: LucidityTheme,
    snapshot: Arc<UiSnapshot>,
    route: ShellRoute,
    search: String,
    sidebar_width: f32,
    show_new_dialog: bool,
    new_draft: NewSessionDraft,
    context_menu: Option<Uuid>,
    rename_target: Option<(Uuid, String)>,
    settings_draft: SettingsDraft,
    last_layout: ShellLayoutSpec,
    theme_applied: bool,
}

impl Default for LucidityShell {
    fn default() -> Self {
        Self {
            theme: LucidityTheme::default(),
            snapshot: Arc::new(UiSnapshot::default()),
            route: ShellRoute::Welcome,
            search: String::new(),
            sidebar_width: 288.0,
            show_new_dialog: false,
            new_draft: NewSessionDraft::default(),
            context_menu: None,
            rename_target: None,
            settings_draft: SettingsDraft::default(),
            last_layout: ShellLayoutSpec::default(),
            theme_applied: false,
        }
    }
}

impl LucidityShell {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn publish_snapshot(&mut self, snapshot: Arc<UiSnapshot>) {
        if self.settings_draft != snapshot.settings && !self.on_settings_route() {
            self.settings_draft = snapshot.settings.clone();
        }
        if let Some(selected) = snapshot.selected {
            if !matches!(self.route, ShellRoute::Settings(_)) {
                self.route = ShellRoute::Session(selected);
            }
        } else if matches!(self.route, ShellRoute::Session(_)) {
            self.route = ShellRoute::Welcome;
        }
        self.snapshot = snapshot;
    }

    pub fn snapshot(&self) -> &UiSnapshot {
        &self.snapshot
    }

    pub fn layout_spec(&self) -> ShellLayoutSpec {
        ShellLayoutSpec {
            sidebar_width_points: self.sidebar_width,
            header_height_points: 52.0,
            bottom_bar_height_points: 0.0,
            terminal_visible: matches!(self.route, ShellRoute::Session(_)),
        }
    }

    pub fn ui(&mut self, ctx: &egui::Context) -> ShellFrameResponse {
        if !self.theme_applied {
            self.theme.apply(ctx);
            self.theme_applied = true;
        }

        let mut response = ShellFrameResponse::default();
        let mut actions = Vec::new();

        let layout_before = self.layout_spec();

        egui::SidePanel::left("lucidity_sidebar")
            .resizable(true)
            .default_width(self.sidebar_width)
            .width_range(232.0..=420.0)
            .show(ctx, |ui| {
                self.draw_sidebar(ui, &mut actions);
            });

        // Capture resized width
        if let Some(panel) = ctx.memory(|m| m.area_rect(egui::Id::new("lucidity_sidebar"))) {
            let w = panel.width();
            if (w - self.sidebar_width).abs() > 0.5 {
                self.sidebar_width = w.clamp(232.0, 420.0);
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| match self.route.clone() {
            ShellRoute::Welcome => self.draw_welcome(ui, &mut actions),
            ShellRoute::Session(id) => self.draw_session_header(ui, id, &mut actions),
            ShellRoute::Settings(page) => self.draw_settings(ui, page, &mut actions),
        });

        if self.show_new_dialog {
            self.draw_new_dialog(ctx, &mut actions);
        }

        if let Some((id, mut title)) = self.rename_target.clone() {
            egui::Window::new("Rename conversation")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.text_edit_singleline(&mut title);
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            actions.push(ShellAction::Rename {
                                id,
                                title: title.clone(),
                            });
                            self.rename_target = None;
                        }
                        if ui.button("Cancel").clicked() {
                            self.rename_target = None;
                        }
                    });
                    self.rename_target = Some((id, title));
                });
        }

        // Global shortcuts
        ctx.input(|i| {
            if i.modifiers.command && i.key_pressed(egui::Key::N) {
                self.open_new_dialog();
            }
            if i.modifiers.command && i.key_pressed(egui::Key::Comma) {
                self.route = ShellRoute::Settings(SettingsPage::General);
            }
            if i.modifiers.command && i.key_pressed(egui::Key::K) {
                // focus handled by search field interest
            }
        });

        let layout_after = self.layout_spec();
        response.layout_changed = layout_before != layout_after;
        response.actions = actions;
        self.last_layout = layout_after;
        response
    }

    fn on_settings_route(&self) -> bool {
        matches!(self.route, ShellRoute::Settings(_))
    }

    fn open_new_dialog(&mut self) {
        if self.new_draft.adapter_id.is_empty() {
            if let Some(first) = self.snapshot.adapters.first() {
                self.new_draft.adapter_id = first.id.clone();
            }
        }
        if self.new_draft.project_path.is_empty() {
            self.new_draft.project_path = std::env::current_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
        }
        self.show_new_dialog = true;
    }

    fn draw_sidebar(&mut self, ui: &mut Ui, actions: &mut Vec<ShellAction>) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.heading(RichText::new("Lucidity").color(self.theme.text_primary));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add(egui::Button::new(RichText::new("+ New").strong()))
                    .clicked()
                {
                    self.open_new_dialog();
                }
            });
        });
        ui.add_space(6.0);
        ui.add(
            egui::TextEdit::singleline(&mut self.search)
                .hint_text("Search conversations")
                .desired_width(f32::INFINITY),
        );
        ui.add_space(8.0);
        ui.separator();

        let filter = self.search.to_lowercase();
        ScrollArea::vertical()
            .id_salt("sidebar_scroll")
            .show(ui, |ui| {
                ui.label(
                    RichText::new("Active")
                        .small()
                        .color(self.theme.text_secondary),
                );
                let active: Vec<_> = self
                    .snapshot
                    .active
                    .iter()
                    .filter(|row| row_matches(row, &filter))
                    .cloned()
                    .collect();
                if active.is_empty() {
                    ui.label(
                        RichText::new("No active conversations")
                            .color(self.theme.text_secondary)
                            .italics(),
                    );
                }
                for row in active {
                    self.draw_session_row(ui, &row, actions);
                }

                ui.add_space(12.0);
                ui.label(
                    RichText::new("History")
                        .small()
                        .color(self.theme.text_secondary),
                );
                let history: Vec<_> = self
                    .snapshot
                    .history
                    .iter()
                    .filter(|row| row_matches(row, &filter))
                    .cloned()
                    .collect();
                if history.is_empty() {
                    ui.label(
                        RichText::new("No settled history")
                            .color(self.theme.text_secondary)
                            .italics(),
                    );
                }
                for row in history {
                    self.draw_session_row(ui, &row, actions);
                }
            });

        ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Settings").clicked() {
                    self.route = ShellRoute::Settings(SettingsPage::General);
                }
                if ui.button("Import history").clicked() {
                    actions.push(ShellAction::ImportHistories);
                }
            });
            if self.snapshot.demo_mode {
                ui.label(
                    RichText::new("Demo mode")
                        .small()
                        .color(self.theme.warning),
                );
            }
            if let Some(msg) = &self.snapshot.status_message {
                ui.label(RichText::new(msg).small().color(self.theme.text_secondary));
            }
        });
    }

    fn draw_session_row(
        &mut self,
        ui: &mut Ui,
        row: &SessionRowView,
        actions: &mut Vec<ShellAction>,
    ) {
        let selected = self.snapshot.selected == Some(row.id)
            || matches!(self.route, ShellRoute::Session(id) if id == row.id);
        let fill = if selected {
            self.theme.row_selected
        } else {
            self.theme.sidebar_bg
        };
        let response = egui::Frame::new()
            .fill(fill)
            .corner_radius(self.theme.corner_radius)
            .inner_margin(egui::Margin::symmetric(8, 6))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(adapter_glyph(&row.adapter_id));
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&row.title).strong());
                        ui.label(
                            RichText::new(format!(
                                "{} · {}",
                                row.adapter_display_name, row.project_label
                            ))
                            .small()
                            .color(self.theme.text_secondary),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let color = self.theme.status_color(row.status);
                        ui.label(RichText::new(row.status.label()).small().color(color));
                    });
                });
                if ui.rect_contains_pointer(ui.max_rect()) || selected {
                    ui.horizontal(|ui| {
                        if !row.settled {
                            if ui.small_button("Settle").clicked() {
                                actions.push(ShellAction::Settle(row.id));
                            }
                        } else if ui.small_button("Unsettle").clicked() {
                            actions.push(ShellAction::Unsettle(row.id));
                        }
                        if row.has_live_runtime {
                            if ui.small_button("Stop").clicked() {
                                actions.push(ShellAction::Stop(row.id));
                            }
                        } else if row.can_resume && ui.small_button("Resume").clicked() {
                            actions.push(ShellAction::Resume(row.id));
                        }
                        if ui.small_button("⋯").clicked() {
                            self.context_menu = Some(row.id);
                        }
                    });
                }
            })
            .response
            .interact(Sense::click());

        if response.clicked() {
            self.route = ShellRoute::Session(row.id);
            actions.push(ShellAction::OpenConversation(row.id));
        }
        response.context_menu(|ui| {
            if ui.button("Rename").clicked() {
                self.rename_target = Some((row.id, row.title.clone()));
                ui.close();
            }
            if ui.button("Refresh usage").clicked() {
                actions.push(ShellAction::RefreshUsage(row.id));
                ui.close();
            }
            if row.has_live_runtime && ui.button("Stop").clicked() {
                actions.push(ShellAction::Stop(row.id));
                ui.close();
            }
        });
    }

    fn draw_welcome(&mut self, ui: &mut Ui, actions: &mut Vec<ShellAction>) {
        ui.vertical_centered(|ui| {
            ui.add_space(48.0);
            ui.heading("Welcome to Lucidity");
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "A multi-agent coding workspace. Launch Claude, Codex, Kimi, and other native terminal agents from one sidebar—without managing bare terminals.",
                )
                .color(self.theme.text_secondary),
            );
            ui.add_space(20.0);
            if ui
                .add_sized([200.0, 36.0], egui::Button::new("+ New conversation"))
                .clicked()
            {
                self.open_new_dialog();
            }
            ui.add_space(12.0);
            if ui.button("Import existing histories").clicked() {
                actions.push(ShellAction::ImportHistories);
            }
            if let Some(err) = &self.snapshot.launch_error {
                ui.add_space(16.0);
                ui.colored_label(self.theme.danger, err);
            }
            ui.add_space(24.0);
            ui.label(
                RichText::new(format!("v{}", self.snapshot.product_version))
                    .small()
                    .color(self.theme.text_secondary),
            );
        });
    }

    fn draw_session_header(&mut self, ui: &mut Ui, id: Uuid, actions: &mut Vec<ShellAction>) {
        let row = self
            .snapshot
            .active
            .iter()
            .chain(self.snapshot.history.iter())
            .find(|r| r.id == id)
            .cloned();

        egui::TopBottomPanel::top("session_header")
            .exact_height(52.0)
            .show_inside(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    if let Some(row) = &row {
                        ui.label(adapter_glyph(&row.adapter_id));
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&row.title).strong().size(16.0));
                            ui.label(
                                RichText::new(format!(
                                    "{} · {} · {}",
                                    row.adapter_display_name,
                                    row.project_label,
                                    row.status.label()
                                ))
                                .small()
                                .color(self.theme.text_secondary),
                            );
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("Usage").clicked() {
                                actions.push(ShellAction::RefreshUsage(id));
                                self.route = ShellRoute::Settings(SettingsPage::Usage);
                            }
                            if row.has_live_runtime {
                                if ui.button("Stop").clicked() {
                                    actions.push(ShellAction::Stop(id));
                                }
                            } else if ui.button("Resume").clicked() {
                                actions.push(ShellAction::Resume(id));
                            }
                            if !row.settled {
                                if ui.button("Settle").clicked() {
                                    actions.push(ShellAction::Settle(id));
                                }
                            }
                        });
                    } else {
                        ui.label("Conversation unavailable");
                    }
                });
            });

        // Terminal surface is reserved by the host below this header.
        ui.centered_and_justified(|ui| {
            if let Some(err) = &self.snapshot.launch_error {
                ui.colored_label(self.theme.danger, err);
            } else if row.as_ref().is_some_and(|r| !r.has_live_runtime && !r.attached) {
                ui.label(
                    RichText::new("No live terminal attached. Click Resume or create a new session.")
                        .color(self.theme.text_secondary),
                );
            } else {
                // Host paints the WezTerm surface in the remaining rectangle.
                ui.label(
                    RichText::new("")
                        .color(egui::Color32::TRANSPARENT),
                );
            }
        });

        // Usage popover summary
        if ui.input(|i| i.key_pressed(egui::Key::F1)) {
            self.route = ShellRoute::Settings(SettingsPage::Usage);
        }
    }

    fn draw_settings(&mut self, ui: &mut Ui, page: SettingsPage, actions: &mut Vec<ShellAction>) {
        ui.horizontal(|ui| {
            if ui.button("← Back").clicked() {
                self.route = self
                    .snapshot
                    .selected
                    .map(ShellRoute::Session)
                    .unwrap_or(ShellRoute::Welcome);
            }
            ui.heading("Settings");
        });
        ui.separator();
        ui.horizontal(|ui| {
            egui::ScrollArea::vertical()
                .id_salt("settings_nav")
                .max_width(180.0)
                .show(ui, |ui| {
                    for p in SettingsPage::ALL {
                        let selected = p == page;
                        if ui.selectable_label(selected, p.label()).clicked() {
                            self.route = ShellRoute::Settings(p);
                        }
                    }
                });
            ui.separator();
            ui.vertical(|ui| match page {
                SettingsPage::Terminal => {
                    ui.heading("Terminal");
                    ui.label("Font family");
                    ui.text_edit_singleline(&mut self.settings_draft.terminal_font_family);
                    ui.label("Font size");
                    ui.add(
                        egui::Slider::new(&mut self.settings_draft.terminal_font_size, 8.0..=24.0)
                            .suffix(" pt"),
                    );
                    if ui.button("Apply terminal settings").clicked() {
                        actions.push(ShellAction::ApplySettings(self.settings_draft.clone()));
                    }
                }
                SettingsPage::Agents => {
                    ui.heading("Agents");
                    for adapter in &self.snapshot.adapters {
                        ui.group(|ui| {
                            ui.label(RichText::new(&adapter.display_name).strong());
                            ui.label(format!("id: {}", adapter.id));
                            if adapter.executable_found {
                                ui.colored_label(self.theme.success, "Executable found on PATH");
                            } else {
                                ui.colored_label(
                                    self.theme.danger,
                                    adapter
                                        .missing_hint
                                        .clone()
                                        .unwrap_or_else(|| "Executable not found".to_owned()),
                                );
                            }
                            if !adapter.default_args.is_empty() {
                                ui.monospace(format!("args: {}", adapter.default_args.join(" ")));
                            }
                            let mut override_path = self
                                .settings_draft
                                .agent_executable_overrides
                                .iter()
                                .find(|(id, _)| id == &adapter.id)
                                .map(|(_, p)| p.clone())
                                .unwrap_or_default();
                            ui.horizontal(|ui| {
                                ui.label("Executable override");
                                if ui.text_edit_singleline(&mut override_path).changed() {
                                    upsert_pair(
                                        &mut self.settings_draft.agent_executable_overrides,
                                        &adapter.id,
                                        override_path,
                                    );
                                }
                            });
                            let mut extra = self
                                .settings_draft
                                .agent_extra_args
                                .iter()
                                .find(|(id, _)| id == &adapter.id)
                                .map(|(_, p)| p.clone())
                                .unwrap_or_default();
                            ui.horizontal(|ui| {
                                ui.label("Extra args");
                                if ui.text_edit_singleline(&mut extra).changed() {
                                    upsert_pair(
                                        &mut self.settings_draft.agent_extra_args,
                                        &adapter.id,
                                        extra,
                                    );
                                }
                            });
                        });
                    }
                    if ui.button("Save agent settings").clicked() {
                        actions.push(ShellAction::ApplySettings(self.settings_draft.clone()));
                    }
                }
                SettingsPage::Usage => {
                    ui.heading("Usage");
                    let usage = &self.snapshot.usage;
                    ui.group(|ui| {
                        ui.label(RichText::new(&usage.account_label).strong());
                        ui.label(&usage.account_detail);
                    });
                    ui.group(|ui| {
                        ui.label(RichText::new(&usage.context_label).strong());
                        ui.label(&usage.context_detail);
                    });
                    if usage.provider_unavailable {
                        ui.colored_label(
                            self.theme.warning,
                            "Provider usage unavailable — open a session and refresh, or configure the agent.",
                        );
                    }
                    if let Some(id) = self.snapshot.selected {
                        if ui.button("Refresh usage for selected session").clicked() {
                            actions.push(ShellAction::RefreshUsage(id));
                        }
                    }
                }
                SettingsPage::General => {
                    ui.heading("General");
                    ui.label("Lucidity keeps native agent TUIs intact and adds session management.");
                    ui.label(format!("Version {}", self.snapshot.product_version));
                    ui.label(if self.snapshot.demo_mode {
                        "Demo mode is ON (mock adapter allowed)."
                    } else {
                        "Demo mode is OFF (ordinary launch never auto-starts mock)."
                    });
                }
                SettingsPage::Appearance => {
                    ui.heading("Appearance");
                    ui.label("Application chrome uses a proportional desktop theme.");
                    ui.label("Terminal fonts are configured under Terminal.");
                }
                SettingsPage::Keybindings => {
                    ui.heading("Keybindings");
                    ui.monospace("Ctrl+N        New conversation");
                    ui.monospace("Ctrl+,        Settings");
                    ui.monospace("Ctrl+Tab      Next conversation (host)");
                    ui.monospace("Esc           Close dialog / pass to terminal");
                }
                SettingsPage::Sessions => {
                    ui.heading("Sessions");
                    ui.label(format!("Active: {}", self.snapshot.active.len()));
                    ui.label(format!("History: {}", self.snapshot.history.len()));
                    if ui.button("Import histories now").clicked() {
                        actions.push(ShellAction::ImportHistories);
                    }
                }
                SettingsPage::Advanced => {
                    ui.heading("Advanced");
                    ui.label("Catalog path: %LOCALAPPDATA%\\Lucidity\\catalog.sqlite3");
                    ui.label("Demo: launch with --demo or LUCIDITY_DEMO=1");
                }
            });
        });
    }

    fn draw_new_dialog(&mut self, ctx: &egui::Context, actions: &mut Vec<ShellAction>) {
        let mut open = self.show_new_dialog;
        egui::Window::new("New conversation")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(420.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("Agent");
                egui::ComboBox::from_id_salt("new_adapter")
                    .selected_text(selected_adapter_label(
                        &self.snapshot.adapters,
                        &self.new_draft.adapter_id,
                    ))
                    .show_ui(ui, |ui| {
                        for adapter in &self.snapshot.adapters {
                            // Hide mock unless demo mode
                            if adapter.id == "mock-agent" && !self.snapshot.demo_mode {
                                continue;
                            }
                            let label = if adapter.executable_found {
                                adapter.display_name.clone()
                            } else {
                                format!("{} (not found)", adapter.display_name)
                            };
                            ui.selectable_value(
                                &mut self.new_draft.adapter_id,
                                adapter.id.clone(),
                                label,
                            );
                        }
                    });

                ui.add_space(8.0);
                ui.label("Project folder");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.new_draft.project_path)
                            .desired_width(280.0),
                    );
                    if ui.button("Browse…").clicked() {
                        actions.push(ShellAction::PickProjectFolder);
                    }
                });

                ui.add_space(8.0);
                ui.label("Title (optional)");
                ui.text_edit_singleline(&mut self.new_draft.title);

                ui.label("Extra launch args (optional)");
                ui.text_edit_singleline(&mut self.new_draft.extra_args);

                if let Some(adapter) = self
                    .snapshot
                    .adapters
                    .iter()
                    .find(|a| a.id == self.new_draft.adapter_id)
                {
                    if !adapter.default_args.is_empty() {
                        ui.label(
                            RichText::new(format!(
                                "Default args: {}",
                                adapter.default_args.join(" ")
                            ))
                            .small()
                            .color(self.theme.text_secondary),
                        );
                    }
                    if !adapter.executable_found {
                        ui.colored_label(
                            self.theme.warning,
                            adapter.missing_hint.clone().unwrap_or_else(|| {
                                "Executable not found — session row will show configure state."
                                    .to_owned()
                            }),
                        );
                    }
                }

                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let can_create = !self.new_draft.adapter_id.is_empty()
                        && !self.new_draft.project_path.trim().is_empty();
                    if ui
                        .add_enabled(can_create, egui::Button::new("Create"))
                        .clicked()
                    {
                        actions.push(ShellAction::NewConversation {
                            adapter_id: self.new_draft.adapter_id.clone(),
                            project_path: self.new_draft.project_path.clone(),
                            title: non_empty(self.new_draft.title.clone()),
                            extra_args: non_empty(self.new_draft.extra_args.clone()),
                        });
                        self.show_new_dialog = false;
                    }
                    if ui.button("Cancel").clicked() {
                        self.show_new_dialog = false;
                    }
                });
            });
        self.show_new_dialog = open;
    }

    /// Apply a project path chosen by the native folder picker.
    pub fn set_picked_project(&mut self, path: String) {
        self.new_draft.project_path = path;
        self.show_new_dialog = true;
    }
}

fn row_matches(row: &SessionRowView, filter: &str) -> bool {
    if filter.is_empty() {
        return true;
    }
    row.title.to_lowercase().contains(filter)
        || row.adapter_id.to_lowercase().contains(filter)
        || row.project_label.to_lowercase().contains(filter)
        || row.adapter_display_name.to_lowercase().contains(filter)
}

fn adapter_glyph(adapter_id: &str) -> RichText {
    let glyph = match adapter_id {
        "claude" => "◈",
        "codex" => "▣",
        "kimi" => "◇",
        "mock-agent" => "○",
        "grok" => "✦",
        _ => "●",
    };
    RichText::new(glyph).size(18.0)
}

fn selected_adapter_label(adapters: &[AdapterOption], id: &str) -> String {
    adapters
        .iter()
        .find(|a| a.id == id)
        .map(|a| a.display_name.clone())
        .unwrap_or_else(|| "Select agent".to_owned())
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn upsert_pair(pairs: &mut Vec<(String, String)>, id: &str, value: String) {
    if let Some(entry) = pairs.iter_mut().find(|(existing, _)| existing == id) {
        entry.1 = value;
    } else {
        pairs.push((id.to_owned(), value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AdapterOption, RuntimeBadge, SessionRowView, UiSnapshot};

    #[test]
    fn welcome_layout_hides_terminal() {
        let shell = LucidityShell::new();
        let layout = shell.layout_spec();
        assert!(!layout.terminal_visible);
        assert!(layout.sidebar_width_points >= 232.0);
    }

    #[test]
    fn session_layout_shows_terminal() {
        let mut shell = LucidityShell::new();
        let id = Uuid::new_v4();
        shell.route = ShellRoute::Session(id);
        assert!(shell.layout_spec().terminal_visible);
    }

    #[test]
    fn new_dialog_filters_mock_unless_demo() {
        let mut snap = UiSnapshot::default();
        snap.adapters = vec![
            AdapterOption {
                id: "claude".into(),
                display_name: "Claude Code".into(),
                executable_found: true,
                default_args: vec!["--dangerously-skip-permissions".into()],
                missing_hint: None,
            },
            AdapterOption {
                id: "mock-agent".into(),
                display_name: "Mock".into(),
                executable_found: true,
                default_args: vec![],
                missing_hint: None,
            },
        ];
        snap.demo_mode = false;
        let mut shell = LucidityShell::new();
        shell.publish_snapshot(Arc::new(snap));
        // Opening dialog should prefer first non-empty adapter id from list
        shell.open_new_dialog();
        assert_eq!(shell.new_draft.adapter_id, "claude");
    }

    #[test]
    fn row_filter_matches_project() {
        let row = SessionRowView {
            id: Uuid::new_v4(),
            title: "Work".into(),
            adapter_id: "claude".into(),
            adapter_display_name: "Claude Code".into(),
            project_label: "Lucidity".into(),
            status: RuntimeBadge::Working,
            attached: true,
            settled: false,
            can_resume: false,
            has_live_runtime: true,
        };
        assert!(row_matches(&row, "lucid"));
        assert!(!row_matches(&row, "codex"));
    }
}
