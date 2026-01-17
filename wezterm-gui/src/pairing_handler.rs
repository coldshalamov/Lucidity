use crate::termwindow::TermWindowNotif;
use lucidity_host::{PairingApproval, PairingApprover};
use lucidity_pairing::PairingRequest;
use std::sync::{mpsc, Mutex};
use std::time::Duration;
use window::{Window, WindowOps};

pub struct GuiPairingApprover {
    window: Window,
    prompt_lock: Mutex<()>,
}

impl GuiPairingApprover {
    pub fn new(window: Window) -> Self {
        Self {
            window,
            prompt_lock: Mutex::new(()),
        }
    }
}

impl PairingApprover for GuiPairingApprover {
    fn approve_pairing(&self, request: &PairingRequest) -> anyhow::Result<PairingApproval> {
        let _lock = self.prompt_lock.lock().unwrap();

        let (tx, rx) = mpsc::channel();
        let request = request.clone();
        
        self.window.notify(TermWindowNotif::Apply(Box::new(move |term_window| {
            term_window.show_lucidity_pairing_approval(request, tx);
        })));

        let approved = rx
            .recv_timeout(Duration::from_secs(300))
            .unwrap_or(false);

        Ok(if approved {
            PairingApproval::approved()
        } else {
            PairingApproval::rejected("pairing request rejected")
        })
    }
}
