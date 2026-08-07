//! Dependency-light control seam for native products embedding the GUI.

use crate::frontend::{front_end, try_front_end, WindowClosePolicy};
use crate::product_ui::ProductUiFactory;
use config::keyassignment::SpawnTabDomain;
use mux::Mux;
use portable_pty::CommandBuilder;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};
use wezterm_term::TerminalSize;
use window::WindowOps;

/// Product lifecycle callbacks. All callbacks run on the GUI main thread.
#[derive(Clone, Default)]
pub struct ProductGuiHooks {
    pub close_to_tray: bool,
    pub on_ready: Option<Arc<dyn Fn() + Send + Sync>>,
    pub on_window_hidden: Option<Arc<dyn Fn() + Send + Sync>>,
    /// When set, product mode hosts a real GUI controller (egui) instead of
    /// the legacy hand-painted chrome.
    pub ui_factory: Option<ProductUiFactory>,
}

#[derive(Clone, Debug)]
pub struct ProductSpawnRequest {
    pub command: CommandBuilder,
    pub cwd: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductSpawnOutcome {
    Spawned { pane_id: usize },
    Failed { message: String },
}

type SpawnCallback = Arc<dyn Fn(ProductSpawnOutcome) + Send + Sync>;

fn hooks_slot() -> &'static RwLock<Option<ProductGuiHooks>> {
    static HOOKS: OnceLock<RwLock<Option<ProductGuiHooks>>> = OnceLock::new();
    HOOKS.get_or_init(|| RwLock::new(None))
}

pub(crate) fn install_hooks(hooks: ProductGuiHooks) -> anyhow::Result<()> {
    let mut slot = hooks_slot().write().unwrap();
    if slot.is_some() {
        anyhow::bail!("the product GUI lifecycle hooks may only be installed once");
    }
    slot.replace(hooks);
    Ok(())
}

fn hooks() -> Option<ProductGuiHooks> {
    hooks_slot().read().unwrap().clone()
}

/// Clone the product UI factory for window construction (GUI thread).
pub fn product_ui_factory() -> Option<ProductUiFactory> {
    hooks().and_then(|hooks| hooks.ui_factory.clone())
}

/// Whether product mode installed a real GUI factory (egui shell).
pub fn product_ui_enabled() -> bool {
    hooks().is_some_and(|hooks| hooks.ui_factory.is_some())
}

/// Build the config override pairs that product mode applies for terminal chrome.
///
/// Pure function so settings → WezTerm mapping is unit-testable without a GUI.
pub fn terminal_appearance_overrides(
    font_size: f64,
    font_family: &str,
    update_check_enabled: bool,
) -> Vec<(String, String)> {
    let family = font_family
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    vec![
        (
            "check_for_updates".to_string(),
            if update_check_enabled {
                "true".to_string()
            } else {
                "false".to_string()
            },
        ),
        ("front_end".to_string(), "\"WebGpu\"".to_string()),
        ("font_size".to_string(), format!("{font_size}")),
        (
            "font".to_string(),
            format!("{{ family = \"{family}\" }}"),
        ),
    ]
}

/// Apply terminal font size/family to the live product configuration and reload.
///
/// Runs on the GUI main thread so TermWindow config subscriptions fire.
pub fn request_product_terminal_appearance(font_size: f64, font_family: String) {
    promise::spawn::spawn_into_main_thread(async move {
        let overrides = terminal_appearance_overrides(font_size, &font_family, false);
        if let Err(error) = config::set_config_overrides(&overrides) {
            log::error!("failed to apply terminal appearance overrides: {error:#}");
            return;
        }
        config::reload();
        log::info!(
            "applied product terminal appearance font_size={font_size} family={font_family}"
        );
    })
    .detach();
}

pub(crate) fn notify_ready() {
    let Some(hooks) = hooks() else {
        return;
    };
    if let Some(frontend) = try_front_end() {
        frontend.set_window_close_policy(if hooks.close_to_tray {
            WindowClosePolicy::HidePreservingMux
        } else {
            WindowClosePolicy::Stock
        });
    }
    if let Some(callback) = hooks.on_ready {
        callback();
    }
}

pub(crate) fn notify_window_hidden() {
    if let Some(callback) = hooks().and_then(|hooks| hooks.on_window_hidden) {
        callback();
    }
}

/// Restore and focus the existing product window without creating a new mux.
pub fn request_product_window_open() {
    promise::spawn::spawn_into_main_thread(async move {
        let Some(frontend) = try_front_end() else {
            return;
        };
        if let Some(window) = frontend.gui_windows().into_iter().next() {
            frontend.show_and_focus_mux_window(window.mux_window_id);
        } else {
            frontend.reconcile_workspace();
        }
    })
    .detach();
}

/// Repaint existing product windows after product-owned model state changes.
pub fn request_product_redraw() {
    promise::spawn::spawn_into_main_thread(async move {
        let Some(frontend) = try_front_end() else {
            return;
        };
        for window in frontend.gui_windows() {
            window.window.invalidate();
        }
    })
    .detach();
}

/// Focus a pane and its existing product window.
pub fn request_product_focus_pane(pane_id: usize) {
    promise::spawn::spawn_into_main_thread(async move {
        let mux = Mux::get();
        let mux_window_id = mux
            .resolve_pane_id(pane_id)
            .map(|(_, window_id, _)| window_id);
        if let Err(error) = mux.focus_pane_and_containing_tab(pane_id) {
            log::warn!("cannot focus product pane {pane_id}: {error:#}");
            return;
        }
        if let Some(mux_window_id) = mux_window_id {
            front_end().show_and_focus_mux_window(mux_window_id);
        }
    })
    .detach();
}

/// Spawn a direct command as a tab in the existing product mux window.
pub fn request_product_spawn(request: ProductSpawnRequest, callback: SpawnCallback) {
    promise::spawn::spawn_into_main_thread(async move {
        promise::spawn::spawn(async move {
            let mux = Mux::get();
            let existing_window = mux.iter_windows().into_iter().next();
            let workspace = mux.active_workspace();
            let cwd = request
                .cwd
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned());
            let result = mux
                .spawn_tab_or_window(
                    existing_window,
                    SpawnTabDomain::DefaultDomain,
                    Some(request.command),
                    cwd,
                    TerminalSize::default(),
                    None,
                    workspace,
                    None,
                )
                .await;
            match result {
                Ok((_tab, pane, mux_window_id)) => {
                    front_end().show_and_focus_mux_window(mux_window_id);
                    callback(ProductSpawnOutcome::Spawned {
                        pane_id: pane.pane_id(),
                    });
                }
                Err(error) => callback(ProductSpawnOutcome::Failed {
                    message: format!("{error:#}"),
                }),
            }
        })
        .detach();
    })
    .detach();
}

/// Terminate Agent-owned pane trees and close their mux windows.
pub fn request_product_exit() {
    promise::spawn::spawn_into_main_thread(async move {
        if let Some(frontend) = try_front_end() {
            frontend.set_window_close_policy(WindowClosePolicy::Stock);
        }
        if let Some(mux) = Mux::try_get() {
            for pane in mux.iter_panes() {
                pane.kill();
            }
            for window_id in mux.iter_windows() {
                mux.kill_window(window_id);
            }
        }
        // Do not post WM_QUIT while native windows still own GL resources.
        // The established MuxNotification::Empty path closes each GUI window
        // first, then terminates the message loop once those resources have
        // been released. Posting it here raced that sequence and made glium
        // drop a context after its HWND was already gone.
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_hooks_default_to_stock_policy() {
        let hooks = ProductGuiHooks::default();
        assert!(!hooks.close_to_tray);
        assert!(hooks.on_ready.is_none());
        assert!(hooks.on_window_hidden.is_none());
        assert!(hooks.ui_factory.is_none());
    }

    #[test]
    fn terminal_appearance_overrides_include_font_size_and_family() {
        let overrides = terminal_appearance_overrides(14.0, "Cascadia Mono", false);
        let map: std::collections::HashMap<_, _> = overrides.into_iter().collect();
        assert_eq!(map.get("font_size").map(String::as_str), Some("14"));
        assert_eq!(map.get("front_end").map(String::as_str), Some("\"WebGpu\""));
        assert_eq!(map.get("check_for_updates").map(String::as_str), Some("false"));
        let font = map.get("font").expect("font override");
        assert!(
            font.contains("Cascadia Mono"),
            "font override missing family: {font}"
        );
    }
}
