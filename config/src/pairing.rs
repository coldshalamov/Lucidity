use wezterm_dynamic::{FromDynamic, ToDynamic};

#[derive(Debug, Default, Clone, FromDynamic, ToDynamic)]
pub struct PairingConfig {
    /// If set to true, the pairing splash screen will not be shown
    /// when the GUI starts.
    /// Default is false.
    #[dynamic(default)]
    pub disable_splash: bool,
}
