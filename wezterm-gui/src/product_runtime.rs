//! Dependency-light control seam for native products embedding the GUI.

use crate::frontend::{front_end, try_front_end, WindowClosePolicy};
use config::keyassignment::SpawnTabDomain;
use mux::Mux;
use portable_pty::CommandBuilder;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};
use wezterm_term::TerminalSize;
use window::{Connection, ConnectionOps};

/// Product lifecycle callbacks. All callbacks run on the GUI main thread.
#[derive(Clone, Default)]
pub struct ProductGuiHooks {
    pub close_to_tray: bool,
    pub on_ready: Option<Arc<dyn Fn() + Send + Sync>>,
    pub on_window_hidden: Option<Arc<dyn Fn() + Send + Sync>>,
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

/// Terminate Agent-owned pane trees, close mux windows, and stop the GUI loop.
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
        if let Some(connection) = Connection::get() {
            connection.terminate_message_loop();
        }
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
    }
}
