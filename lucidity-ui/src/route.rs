//! Shell navigation routes.

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum SettingsPage {
    General,
    Agents,
    Terminal,
    Appearance,
    Keybindings,
    Sessions,
    Usage,
    Advanced,
}

impl SettingsPage {
    pub const ALL: [SettingsPage; 8] = [
        SettingsPage::General,
        SettingsPage::Agents,
        SettingsPage::Terminal,
        SettingsPage::Appearance,
        SettingsPage::Keybindings,
        SettingsPage::Sessions,
        SettingsPage::Usage,
        SettingsPage::Advanced,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Agents => "Agents",
            Self::Terminal => "Terminal",
            Self::Appearance => "Appearance",
            Self::Keybindings => "Keybindings",
            Self::Sessions => "Sessions",
            Self::Usage => "Usage",
            Self::Advanced => "Advanced",
        }
    }
}
