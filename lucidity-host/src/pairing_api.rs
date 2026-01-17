use anyhow::Context;
use lucidity_pairing::{
    DeviceTrustStore, Keypair, KeypairStore, PairingPayload, PairingRequest, PairingResponse,
    TrustedDevice,
};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};

#[derive(Debug, Clone)]
pub struct PairingApproval {
    pub approved: bool,
    pub reason: Option<String>,
}

impl PairingApproval {
    pub fn approved() -> Self {
        Self {
            approved: true,
            reason: None,
        }
    }

    pub fn rejected(reason: impl Into<String>) -> Self {
        Self {
            approved: false,
            reason: Some(reason.into()),
        }
    }
}

pub trait PairingApprover: Send + Sync {
    fn approve_pairing(&self, request: &PairingRequest) -> anyhow::Result<PairingApproval>;
}

static PAIRING_APPROVER: OnceLock<RwLock<Option<Arc<dyn PairingApprover>>>> = OnceLock::new();

fn pairing_approver_lock() -> &'static RwLock<Option<Arc<dyn PairingApprover>>> {
    PAIRING_APPROVER.get_or_init(|| RwLock::new(None))
}

pub fn set_pairing_approver(approver: Option<Arc<dyn PairingApprover>>) {
    *pairing_approver_lock().write().unwrap() = approver;
}

fn get_pairing_approver() -> Option<Arc<dyn PairingApprover>> {
    pairing_approver_lock()
        .read()
        .unwrap()
        .as_ref()
        .map(Arc::clone)
}

fn host_keypair_path() -> PathBuf {
    if let Ok(p) = std::env::var("LUCIDITY_HOST_KEYPAIR") {
        return PathBuf::from(p);
    }
    config::DATA_DIR
        .join("lucidity")
        .join("host_keypair.json")
}

fn device_trust_db_path() -> PathBuf {
    if let Ok(p) = std::env::var("LUCIDITY_DEVICE_TRUST_DB") {
        return PathBuf::from(p);
    }
    config::DATA_DIR
        .join("lucidity")
        .join("devices.db")
}

pub fn load_or_create_host_keypair() -> anyhow::Result<Keypair> {
    let store = KeypairStore::open(host_keypair_path());
    store.load_or_generate()
}

pub fn current_pairing_payload() -> anyhow::Result<PairingPayload> {
    let keypair = load_or_create_host_keypair()?;
    Ok(PairingPayload::new(keypair.public_key()))
}

pub fn handle_pairing_submit(req: PairingRequest) -> anyhow::Result<PairingResponse> {
    let host_keypair = load_or_create_host_keypair()?;
    let host_pub = host_keypair.public_key();

    req.verify(&host_pub)?;

    let approver = match get_pairing_approver() {
        Some(a) => a,
        None => {
            return Ok(PairingResponse::rejected(
                "pairing approval UI not available (GUI not running?)",
            ));
        }
    };

    let approval = approver.approve_pairing(&req)?;
    if !approval.approved {
        return Ok(PairingResponse::rejected(
            approval
                .reason
                .unwrap_or_else(|| "pairing request rejected".to_string()),
        ));
    }

    let db_path = device_trust_db_path();
    let store = DeviceTrustStore::open(&db_path)
        .with_context(|| format!("opening trust store {}", db_path.display()))?;

    let now = chrono::Utc::now().timestamp();
    store.add_device(&TrustedDevice {
        public_key: req.mobile_public_key.clone(),
        user_email: req.user_email.clone(),
        device_name: req.device_name.clone(),
        paired_at: now,
        last_seen: Some(now),
    })?;

    Ok(PairingResponse::approved())
}

pub fn list_trusted_devices() -> anyhow::Result<Vec<TrustedDevice>> {
    let db_path = device_trust_db_path();
    let store = DeviceTrustStore::open(&db_path)
        .with_context(|| format!("opening trust store {}", db_path.display()))?;
    store.list_devices()
}

