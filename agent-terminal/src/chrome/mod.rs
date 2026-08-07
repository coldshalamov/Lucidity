//! Native product composition and chrome ownership boundary.

mod app;
mod controller;

pub const APP_USER_MODEL_ID: &str = "io.lucidity.agent-terminal";
pub const WINDOW_CLASS: &str = "io.lucidity.agent-terminal";
pub const WINDOW_TITLE: &str = "Lucidity";
pub const APPLICATION_NAME: &str = "Lucidity";
pub const UPSTREAM_UPDATE_CHECK_ENABLED: bool = false;
pub const PRODUCT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Explicit demo mode: `--demo` flag or `LUCIDITY_DEMO=1`.
pub fn demo_mode_enabled() -> bool {
    if std::env::args().any(|arg| arg == "--demo") {
        return true;
    }
    match std::env::var("LUCIDITY_DEMO") {
        Ok(value) => {
            let value = value.trim();
            !value.is_empty() && value != "0" && !value.eq_ignore_ascii_case("false")
        }
        Err(_) => false,
    }
}

pub fn run() -> anyhow::Result<()> {
    let application = app::ProductApplication::bootstrap()?;
    let hooks = application.gui_hooks();
    // V2: no legacy hand-painted chrome provider. Real GUI is ui_factory.
    let result = wezterm_gui::run_product_with_hooks(
        wezterm_gui::ProductGuiConfig {
            application_name: APPLICATION_NAME.to_string(),
            app_user_model_id: APP_USER_MODEL_ID.to_string(),
            window_class: WINDOW_CLASS.to_string(),
            window_title: WINDOW_TITLE.to_string(),
            version: PRODUCT_VERSION.to_string(),
            update_check_enabled: UPSTREAM_UPDATE_CHECK_ENABLED,
            chrome: None,
        },
        hooks,
    );
    application.shutdown();
    result
}
