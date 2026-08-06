//! Compile-safe native chrome ownership boundary.

pub const APP_USER_MODEL_ID: &str = "io.lucidity.agent-terminal";
pub const WINDOW_CLASS: &str = "io.lucidity.agent-terminal";
pub const WINDOW_TITLE: &str = "Lucidity";
pub const APPLICATION_NAME: &str = "Lucidity Agent Terminal";
pub const UPSTREAM_UPDATE_CHECK_ENABLED: bool = false;

pub fn run() -> anyhow::Result<()> {
    Ok(())
}
