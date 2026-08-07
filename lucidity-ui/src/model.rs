//! Immutable view models consumed by the Lucidity shell.

use crate::route::SettingsPage;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum RuntimeBadge {
    NotRunning,
    Starting,
    Working,
    WaitingForInput,
    AwaitingApproval,
    CompletedIdle,
    Failed,
    UnknownExternal,
}

impl RuntimeBadge {
    pub fn label(self) -> &'static str {
        match self {
            Self::NotRunning => "Idle",
            Self::Starting => "Starting",
            Self::Working => "Working",
            Self::WaitingForInput => "Needs input",
            Self::AwaitingApproval => "Needs approval",
            Self::CompletedIdle => "Done",
            Self::Failed => "Failed",
            Self::UnknownExternal => "External",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionRowView {
    pub id: Uuid,
    pub title: String,
    pub adapter_id: String,
    pub adapter_display_name: String,
    pub project_label: String,
    pub status: RuntimeBadge,
    pub attached: bool,
    pub settled: bool,
    pub can_resume: bool,
    pub has_live_runtime: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AdapterOption {
    pub id: String,
    pub display_name: String,
    pub executable_found: bool,
    pub default_args: Vec<String>,
    pub missing_hint: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UsageView {
    pub account_label: String,
    pub account_detail: String,
    pub context_label: String,
    pub context_detail: String,
    pub provider_unavailable: bool,
}

impl Default for UsageView {
    fn default() -> Self {
        Self {
            account_label: "Account".to_owned(),
            account_detail: "No provider data yet".to_owned(),
            context_label: "Conversation context".to_owned(),
            context_detail: "No context usage yet".to_owned(),
            provider_unavailable: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettingsDraft {
    pub terminal_font_size: f32,
    pub terminal_font_family: String,
    pub agent_executable_overrides: Vec<(String, String)>,
    pub agent_extra_args: Vec<(String, String)>,
}

impl Default for SettingsDraft {
    fn default() -> Self {
        Self {
            terminal_font_size: 12.0,
            terminal_font_family: "JetBrains Mono".to_owned(),
            agent_executable_overrides: Vec::new(),
            agent_extra_args: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShellRoute {
    Welcome,
    Session(Uuid),
    Settings(SettingsPage),
}

impl Default for ShellRoute {
    fn default() -> Self {
        Self::Welcome
    }
}

/// Immutable snapshot published by the host; render path never queries SQLite.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UiSnapshot {
    pub revision: u64,
    pub product_version: String,
    pub demo_mode: bool,
    pub active: Vec<SessionRowView>,
    pub history: Vec<SessionRowView>,
    pub adapters: Vec<AdapterOption>,
    pub selected: Option<Uuid>,
    pub usage: UsageView,
    pub settings: SettingsDraft,
    pub status_message: Option<String>,
    pub launch_error: Option<String>,
}

impl Default for UiSnapshot {
    fn default() -> Self {
        Self {
            revision: 0,
            product_version: "0.1.0".to_owned(),
            demo_mode: false,
            active: Vec::new(),
            history: Vec::new(),
            adapters: Vec::new(),
            selected: None,
            usage: UsageView::default(),
            settings: SettingsDraft::default(),
            status_message: None,
            launch_error: None,
        }
    }
}

impl UiSnapshot {
    /// Pointer stability helper for tests: identical revision means host may reuse Arc.
    pub fn is_same_revision(&self, other: &Self) -> bool {
        self.revision == other.revision
    }
}
