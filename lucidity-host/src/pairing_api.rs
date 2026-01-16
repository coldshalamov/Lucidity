use anyhow::Context;
use lucidity_pairing::{DeviceTrustStore, Keypair, KeypairStore, PairingPayload, PairingRequest, PairingResponse, TrustedDevice};
use std::path::PathBuf;

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

    let auto_approve = std::env::var("LUCIDITY_PAIRING_AUTO_APPROVE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    if !auto_approve {
        return Ok(PairingResponse::rejected(
            "pairing approval UI not implemented (set LUCIDITY_PAIRING_AUTO_APPROVE=1 for dev)",
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

