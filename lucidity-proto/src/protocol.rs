use serde::{Deserialize, Serialize};
use lucidity_pairing::{PairingRequest, PairingPayload, PairingResponse, TrustedDevice};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneInfo {
    pub pane_id: usize,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum JsonRequest {
    ListPanes,
    Attach {
        pane_id: usize,
    },
    PairingPayload,
    PairingSubmit {
        request: PairingRequest,
    },
    PairingListTrustedDevices,
    AuthResponse {
        public_key: String,
        signature: String,
        client_nonce: Option<String>,
    },
    Paste {
        pane_id: usize,
        text: String,
    },
    Resize {
        pane_id: usize,
        rows: usize,
        cols: usize,
    },
    /// Revoke this device's trust on the host
    RevokeDevice {
        public_key: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum JsonResponse {
    ListPanes {
        panes: Vec<PaneInfo>,
    },
    AttachOk {
        pane_id: usize,
    },
    PairingPayload {
        payload: PairingPayload,
    },
    PairingResponse {
        response: PairingResponse,
    },
    PairingTrustedDevices {
        devices: Vec<TrustedDevice>,
    },
    AuthChallenge {
        nonce: String,
    },
    AuthSuccess {
        signature: Option<String>,
    },
    Error {
        message: String,
    },
    /// Host pushes clipboard changes to mobile
    ClipboardPush {
        text: String,
    },
}
