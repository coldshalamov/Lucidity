//! Compile-safe Windows integration ownership boundary and host lease helpers.

mod tray_state;

#[cfg(windows)]
mod tray;

#[cfg(windows)]
pub use tray::{TrayCallbacks, TrayController};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsHostSecurityProbe {
    pub current_user_sid: Option<String>,
    pub pipe_name: Option<String>,
    pub unavailable_reason: Option<String>,
}

#[cfg(windows)]
pub fn probe_named_pipe_security() -> WindowsHostSecurityProbe {
    match crate::ipc::named_pipe::current_user_sid_string() {
        Ok(sid) => WindowsHostSecurityProbe {
            pipe_name: Some(crate::ipc::pipe_name_for_owner_sid(&sid)),
            current_user_sid: Some(sid),
            unavailable_reason: None,
        },
        Err(error) => WindowsHostSecurityProbe {
            current_user_sid: None,
            pipe_name: None,
            unavailable_reason: Some(error.to_string()),
        },
    }
}

#[cfg(not(windows))]
pub fn probe_named_pipe_security() -> WindowsHostSecurityProbe {
    WindowsHostSecurityProbe {
        current_user_sid: None,
        pipe_name: None,
        unavailable_reason: Some(
            "Windows named-pipe security is unavailable off Windows".to_owned(),
        ),
    }
}
