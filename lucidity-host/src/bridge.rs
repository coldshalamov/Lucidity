use anyhow::{anyhow, Context};
use mux::pane::PaneId;
use mux::Mux;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneInfo {
    pub pane_id: PaneId,
    pub title: String,
}

pub trait OutputSubscription: Send {
    fn recv_timeout(&self, timeout: std::time::Duration) -> anyhow::Result<Option<Arc<[u8]>>>;
}

pub trait PaneBridge: Send + Sync + 'static {
    fn list_panes(&self) -> anyhow::Result<Vec<PaneInfo>>;
    fn subscribe_output(&self, pane_id: PaneId) -> anyhow::Result<Box<dyn OutputSubscription>>;
    fn send_input(&self, pane_id: PaneId, bytes: &[u8]) -> anyhow::Result<()>;
}

struct MuxOutputSubscription {
    sub: mux::PanePtyOutputSubscription,
}

impl OutputSubscription for MuxOutputSubscription {
    fn recv_timeout(&self, timeout: std::time::Duration) -> anyhow::Result<Option<Arc<[u8]>>> {
        self.sub
            .receiver()
            .recv_timeout(timeout)
            .map(Some)
            .or_else(|e| match e {
                crossbeam::channel::RecvTimeoutError::Timeout => Ok(None),
                crossbeam::channel::RecvTimeoutError::Disconnected => {
                    Err(anyhow!("pty output subscription ended"))
                }
            })
    }
}

#[derive(Default)]
pub struct MuxPaneBridge;

impl PaneBridge for MuxPaneBridge {
    fn list_panes(&self) -> anyhow::Result<Vec<PaneInfo>> {
        let mux = Mux::get();
        Ok(mux
            .iter_panes()
            .into_iter()
            .map(|p| PaneInfo {
                pane_id: p.pane_id(),
                title: p.get_title(),
            })
            .collect())
    }

    fn subscribe_output(&self, pane_id: PaneId) -> anyhow::Result<Box<dyn OutputSubscription>> {
        let mux = Mux::get();
        Ok(Box::new(MuxOutputSubscription {
            sub: mux.subscribe_to_pane_pty_output(pane_id),
        }))
    }

    fn send_input(&self, pane_id: PaneId, bytes: &[u8]) -> anyhow::Result<()> {
        let mux = Mux::get();
        let pane = mux
            .get_pane(pane_id)
            .ok_or_else(|| anyhow!("no such pane: {pane_id}"))?;
        let mut writer = pane.writer();
        writer
            .write_all(bytes)
            .with_context(|| format!("writing {} bytes to pane {pane_id}", bytes.len()))?;
        writer.flush().ok();
        Ok(())
    }
}

pub struct FakePaneBridge {
    panes: Mutex<Vec<PaneInfo>>,
    out: Mutex<std::collections::HashMap<PaneId, crossbeam::channel::Sender<Arc<[u8]>>>>,
    inputs: Mutex<Vec<(PaneId, Vec<u8>)>>,
}

impl FakePaneBridge {
    pub fn new(panes: Vec<PaneInfo>) -> Self {
        Self {
            panes: Mutex::new(panes),
            out: Mutex::new(std::collections::HashMap::new()),
            inputs: Mutex::new(Vec::new()),
        }
    }

    pub fn emit_output(&self, pane_id: PaneId, bytes: &[u8]) {
        if let Some(tx) = self.out.lock().unwrap().get(&pane_id) {
            let _ = tx.try_send(Arc::from(bytes));
        }
    }

    pub fn take_inputs(&self) -> Vec<(PaneId, Vec<u8>)> {
        std::mem::take(&mut *self.inputs.lock().unwrap())
    }
}

struct FakeOutputSubscription {
    rx: crossbeam::channel::Receiver<Arc<[u8]>>,
}

impl OutputSubscription for FakeOutputSubscription {
    fn recv_timeout(&self, timeout: std::time::Duration) -> anyhow::Result<Option<Arc<[u8]>>> {
        self.rx
            .recv_timeout(timeout)
            .map(Some)
            .or_else(|e| match e {
                crossbeam::channel::RecvTimeoutError::Timeout => Ok(None),
                crossbeam::channel::RecvTimeoutError::Disconnected => {
                    Err(anyhow!("fake output subscription ended"))
                }
            })
    }
}

impl PaneBridge for FakePaneBridge {
    fn list_panes(&self) -> anyhow::Result<Vec<PaneInfo>> {
        Ok(self.panes.lock().unwrap().clone())
    }

    fn subscribe_output(&self, pane_id: PaneId) -> anyhow::Result<Box<dyn OutputSubscription>> {
        let (tx, rx) = crossbeam::channel::bounded(256);
        self.out.lock().unwrap().insert(pane_id, tx);
        Ok(Box::new(FakeOutputSubscription { rx }))
    }

    fn send_input(&self, pane_id: PaneId, bytes: &[u8]) -> anyhow::Result<()> {
        self.inputs
            .lock()
            .unwrap()
            .push((pane_id, bytes.to_vec()));
        Ok(())
    }
}
