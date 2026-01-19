//! Pairing API tests
//!
//! Note: These tests must run with --test-threads=1 because they share
//! global state (PAIRING_APPROVER) and use environment variables.

use k9::assert_equal;
use lucidity_host::{PairingApproval, PairingApprover, set_pairing_approver};
use lucidity_pairing::{DeviceTrustStore, Keypair, KeypairStore, PairingPayload, PairingRequest};
use std::sync::Arc;

struct TestPairingApprover {
    approve: bool,
    rejection_reason: Option<String>,
}

impl TestPairingApprover {
    fn new_approve() -> Self {
        Self {
            approve: true,
            rejection_reason: None,
        }
    }

    fn new_reject(reason: impl Into<String>) -> Self {
        Self {
            approve: false,
            rejection_reason: Some(reason.into()),
        }
    }
}

impl PairingApprover for TestPairingApprover {
    fn approve_pairing(&self, _request: &PairingRequest) -> anyhow::Result<PairingApproval> {
        Ok(if self.approve {
            PairingApproval::approved()
        } else {
            PairingApproval::rejected(
                self.rejection_reason
                    .clone()
                    .unwrap_or_else(|| "rejected by test".to_string()),
            )
        })
    }
}

/// Test that directly tests the pairing approval flow without relying on global state
#[test]
fn test_pairing_approval_types() {
    let approved = PairingApproval::approved();
    assert_equal!(approved.approved, true);
    assert_equal!(approved.reason, None);

    let rejected = PairingApproval::rejected("test reason");
    assert_equal!(rejected.approved, false);
    assert_equal!(rejected.reason, Some("test reason".to_string()));
}

/// Test that the TestPairingApprover works correctly
#[test]
fn test_test_pairing_approver() {
    let approver = TestPairingApprover::new_approve();
    let mobile_keypair = Keypair::generate();
    let host_keypair = Keypair::generate();
    let request = PairingRequest::new(
        &mobile_keypair,
        &host_keypair.public_key(),
        "test@example.com".to_string(),
        "Test Device".to_string(),
    );

    let result = approver.approve_pairing(&request).unwrap();
    assert_equal!(result.approved, true);

    let rejecter = TestPairingApprover::new_reject("not allowed");
    let result2 = rejecter.approve_pairing(&request).unwrap();
    assert_equal!(result2.approved, false);
    assert_equal!(result2.reason, Some("not allowed".to_string()));
}

/// Test PairingRequest signature verification (doesn't need global state)
#[test]
fn test_pairing_request_verification() {
    let mobile_keypair = Keypair::generate();
    let host_keypair = Keypair::generate();

    let request = PairingRequest::new(
        &mobile_keypair,
        &host_keypair.public_key(),
        "test@example.com".to_string(),
        "Test Device".to_string(),
    );

    // Should verify against correct host key
    assert!(request.verify(&host_keypair.public_key()).is_ok());

    // Should fail against wrong host key
    let wrong_keypair = Keypair::generate();
    assert!(request.verify(&wrong_keypair.public_key()).is_err());
}

/// Test KeypairStore persistence (doesn't need global state)
#[test]
fn test_keypair_store_persistence() {
    let dir = tempfile::tempdir().unwrap();
    let keypair_path = dir.path().join("test_keypair.json");

    // First call should create a new keypair
    let store1 = KeypairStore::open(&keypair_path);
    let keypair1 = store1.load_or_generate().unwrap();
    let public_key1 = keypair1.public_key();

    // Verify the file was actually created
    assert!(keypair_path.exists());

    // Second call should load the same keypair
    let store2 = KeypairStore::open(&keypair_path);
    let keypair2 = store2.load_or_generate().unwrap();
    let public_key2 = keypair2.public_key();

    // Verify they're the same
    assert_equal!(public_key1, public_key2);
}

/// Test PairingPayload generation
#[test]
fn test_pairing_payload_generation() {
    let keypair = Keypair::generate();
    let payload = PairingPayload::new(keypair.public_key());

    assert_equal!(payload.desktop_public_key, keypair.public_key());
    assert_equal!(payload.version, 2);
    assert!(payload.is_valid()); // Should not be expired immediately
}

/// Test DeviceTrustStore operations (doesn't need global state)
#[test]
fn test_device_trust_store_operations() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test_devices.db");

    let store = DeviceTrustStore::open(&db_path).unwrap();

    // Initially empty
    let devices = store.list_devices().unwrap();
    assert_equal!(devices.len(), 0);

    // Add a device
    let mobile_keypair = Keypair::generate();
    let now = chrono::Utc::now().timestamp();
    let device = lucidity_pairing::TrustedDevice {
        public_key: mobile_keypair.public_key(),
        user_email: "test@example.com".to_string(),
        device_name: "Test Device".to_string(),
        paired_at: now,
        last_seen: Some(now),
    };
    store.add_device(&device).unwrap();

    // Verify it was added
    let devices = store.list_devices().unwrap();
    assert_equal!(devices.len(), 1);
    assert_equal!(devices[0].user_email, "test@example.com");
    assert_equal!(devices[0].device_name, "Test Device");
    assert_equal!(devices[0].public_key, mobile_keypair.public_key());

    // Check is_trusted
    assert!(store.is_trusted(&mobile_keypair.public_key()).unwrap());

    // Check get_device
    let retrieved = store.get_device(&mobile_keypair.public_key()).unwrap();
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_equal!(retrieved.user_email, "test@example.com");

    // Remove device
    store.remove_device(&mobile_keypair.public_key()).unwrap();
    let devices = store.list_devices().unwrap();
    assert_equal!(devices.len(), 0);
}

/// Test the full pairing flow with local test doubles (no global state)
#[test]
fn test_full_pairing_flow_local() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test_devices.db");

    // Simulate host keypair
    let host_keypair = Keypair::generate();

    // Simulate mobile creating a pairing request
    let mobile_keypair = Keypair::generate();
    let request = PairingRequest::new(
        &mobile_keypair,
        &host_keypair.public_key(),
        "mobile@example.com".to_string(),
        "iPhone 15".to_string(),
    );

    // Verify the request signature
    assert!(request.verify(&host_keypair.public_key()).is_ok());

    // Simulate approval
    let approver = TestPairingApprover::new_approve();
    let approval = approver.approve_pairing(&request).unwrap();
    assert!(approval.approved);

    // Store the trusted device
    let store = DeviceTrustStore::open(&db_path).unwrap();
    let now = chrono::Utc::now().timestamp();
    store
        .add_device(&lucidity_pairing::TrustedDevice {
            public_key: request.mobile_public_key.clone(),
            user_email: request.user_email.clone(),
            device_name: request.device_name.clone(),
            paired_at: now,
            last_seen: Some(now),
        })
        .unwrap();

    // Verify device is now trusted
    assert!(store.is_trusted(&mobile_keypair.public_key()).unwrap());
}

/// Test rejection flow
#[test]
fn test_pairing_rejection_flow() {
    let host_keypair = Keypair::generate();
    let mobile_keypair = Keypair::generate();

    let request = PairingRequest::new(
        &mobile_keypair,
        &host_keypair.public_key(),
        "untrusted@example.com".to_string(),
        "Unknown Device".to_string(),
    );

    // Simulate rejection
    let approver = TestPairingApprover::new_reject("Device not recognized");
    let approval = approver.approve_pairing(&request).unwrap();

    assert_equal!(approval.approved, false);
    assert_equal!(approval.reason, Some("Device not recognized".to_string()));
}

/// Test invalid signature handling
#[test]
fn test_invalid_signature_rejected() {
    let host_keypair = Keypair::generate();
    let mobile_keypair = Keypair::generate();
    let wrong_host_keypair = Keypair::generate();

    // Create request signed for wrong host
    let request = PairingRequest::new(
        &mobile_keypair,
        &wrong_host_keypair.public_key(), // Wrong key!
        "attacker@example.com".to_string(),
        "Attacker Device".to_string(),
    );

    // Verification should fail
    let result = request.verify(&host_keypair.public_key());
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("signature"));
}
