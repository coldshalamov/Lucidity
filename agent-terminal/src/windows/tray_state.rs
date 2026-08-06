#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TrayPhase {
    Starting,
    Running,
    Quitting,
    Stopped,
}

#[derive(Debug)]
pub(super) struct TrayState {
    phase: TrayPhase,
    icon_present: bool,
    open_dispatching: bool,
}

impl TrayState {
    pub(super) fn new() -> Self {
        Self {
            phase: TrayPhase::Starting,
            icon_present: false,
            open_dispatching: false,
        }
    }

    pub(super) fn icon_added(&mut self) {
        if matches!(self.phase, TrayPhase::Starting | TrayPhase::Running) {
            self.phase = TrayPhase::Running;
            self.icon_present = true;
        }
    }

    /// Mark Explorer's old icon registration as lost and return whether it
    /// should be re-added for the still-running controller.
    pub(super) fn taskbar_created(&mut self) -> bool {
        if self.phase == TrayPhase::Running {
            self.icon_present = false;
            true
        } else {
            false
        }
    }

    /// Begin an Open dispatch. A nested or concurrent duplicate is ignored
    /// until the callback returns.
    pub(super) fn begin_open(&mut self) -> bool {
        if self.phase == TrayPhase::Running && !self.open_dispatching {
            self.open_dispatching = true;
            true
        } else {
            false
        }
    }

    pub(super) fn finish_open(&mut self) {
        self.open_dispatching = false;
    }

    /// Begin the one permitted owned-Quit transition and report whether the
    /// native icon must first be deleted.
    pub(super) fn begin_quit(&mut self) -> Option<bool> {
        if self.phase != TrayPhase::Running {
            return None;
        }
        self.phase = TrayPhase::Quitting;
        self.open_dispatching = false;
        Some(self.take_icon())
    }

    /// Stop only the tray controller. This does not imply an owned-Quit.
    pub(super) fn begin_shutdown(&mut self) -> Option<bool> {
        if matches!(self.phase, TrayPhase::Quitting | TrayPhase::Stopped) {
            return None;
        }
        self.phase = TrayPhase::Quitting;
        self.open_dispatching = false;
        Some(self.take_icon())
    }

    pub(super) fn stop(&mut self) -> bool {
        let delete_icon = self.take_icon();
        self.phase = TrayPhase::Stopped;
        self.open_dispatching = false;
        delete_icon
    }

    fn take_icon(&mut self) -> bool {
        std::mem::replace(&mut self.icon_present, false)
    }

    #[cfg(test)]
    fn snapshot(&self) -> (TrayPhase, bool, bool) {
        (self.phase, self.icon_present, self.open_dispatching)
    }
}

#[cfg(test)]
mod tests {
    use super::{TrayPhase, TrayState};

    #[test]
    fn explorer_restart_requires_one_readd_without_changing_lifecycle() {
        let mut state = TrayState::new();
        state.icon_added();
        assert_eq!(state.snapshot(), (TrayPhase::Running, true, false));

        assert!(state.taskbar_created());
        assert_eq!(state.snapshot(), (TrayPhase::Running, false, false));
        state.icon_added();
        assert_eq!(state.snapshot(), (TrayPhase::Running, true, false));
    }

    #[test]
    fn open_dispatch_is_reentrancy_safe() {
        let mut state = TrayState::new();
        state.icon_added();

        assert!(state.begin_open());
        assert!(!state.begin_open());
        state.finish_open();
        assert!(state.begin_open());
    }

    #[test]
    fn quit_is_single_shot_and_owns_icon_removal() {
        let mut state = TrayState::new();
        state.icon_added();

        assert_eq!(state.begin_quit(), Some(true));
        assert_eq!(state.begin_quit(), None);
        assert!(!state.taskbar_created());
        assert_eq!(state.snapshot(), (TrayPhase::Quitting, false, false));
    }

    #[test]
    fn controller_shutdown_does_not_become_owned_quit() {
        let mut state = TrayState::new();
        state.icon_added();

        assert_eq!(state.begin_shutdown(), Some(true));
        assert_eq!(state.begin_shutdown(), None);
        assert!(!state.stop());
        assert_eq!(state.snapshot(), (TrayPhase::Stopped, false, false));
    }
}
