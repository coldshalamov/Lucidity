//! Native product composition and chrome ownership boundary.

mod app;

pub const APP_USER_MODEL_ID: &str = "io.lucidity.agent-terminal";
pub const WINDOW_CLASS: &str = "io.lucidity.agent-terminal";
pub const WINDOW_TITLE: &str = "Lucidity";
pub const APPLICATION_NAME: &str = "Lucidity Agent Terminal";
pub const UPSTREAM_UPDATE_CHECK_ENABLED: bool = false;
pub const PRODUCT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run() -> anyhow::Result<()> {
    let application = app::ProductApplication::bootstrap()?;
    let hooks = application.gui_hooks();
    let mut chrome = wezterm_gui::ProductChromeConfig::default();
    chrome.sidebar_provider = Some(application.sidebar_provider());
    let result = wezterm_gui::run_product_with_hooks(
        wezterm_gui::ProductGuiConfig {
            application_name: APPLICATION_NAME.to_string(),
            app_user_model_id: APP_USER_MODEL_ID.to_string(),
            window_class: WINDOW_CLASS.to_string(),
            window_title: WINDOW_TITLE.to_string(),
            version: PRODUCT_VERSION.to_string(),
            update_check_enabled: UPSTREAM_UPDATE_CHECK_ENABLED,
            chrome: Some(chrome),
        },
        hooks,
    );
    application.shutdown();
    result
}
