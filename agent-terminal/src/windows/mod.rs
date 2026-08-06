//! Compile-safe Windows integration ownership boundary.

mod tray_state;

#[cfg(windows)]
mod tray;

#[cfg(windows)]
pub use tray::{TrayCallbacks, TrayController};
