use crate::{PublicKey, Signature};
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Payload embedded in QR code for pairing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingPayload {
    /// Desktop's public key
    pub desktop_public_key: PublicKey,
    /// Relay ID (derived from desktop public key)
    pub relay_id: String,
    /// Timestamp when QR was generated (unix seconds)
    pub timestamp: i64,
    /// Protocol version
    pub version: u8,
    /// LAN address for local connections
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lan_addr: Option<String>,
    /// External address for remote connections (via UPnP)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_addr: Option<String>,
    /// Connection capabilities
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Relay URL if P2P fails
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relay_url: Option<String>,
    /// Secret for relay authentication (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relay_secret: Option<String>,
}

impl PairingPayload {
    /// Create a new pairing payload (basic, without connection info)
    pub fn new(desktop_public_key: PublicKey) -> Self {
        let relay_id = Self::derive_relay_id(&desktop_public_key);
        Self {
            desktop_public_key,
            relay_id,
            timestamp: chrono::Utc::now().timestamp(),
            version: 2,
            lan_addr: None,
            external_addr: None,
            relay_url: None,
            relay_secret: None,
            capabilities: vec![],
        }
    }

    /// Create a payload with connection info for P2P and Relay
    pub fn with_connection_info(
        desktop_public_key: PublicKey,
        lan_addr: Option<String>,
        external_addr: Option<String>,
        relay_url: Option<String>,
        relay_secret: Option<String>,
    ) -> Self {
        let relay_id = Self::derive_relay_id(&desktop_public_key);
        let mut capabilities = vec![];

        if lan_addr.is_some() {
            capabilities.push("lan".to_string());
        }
        if external_addr.is_some() {
            capabilities.push("upnp".to_string());
        }
        if relay_url.is_some() {
            capabilities.push("relay".to_string());
        }

        Self {
            desktop_public_key,
            relay_id,
            timestamp: chrono::Utc::now().timestamp(),
            version: 2,
            lan_addr,
            external_addr,
            relay_url,
            relay_secret,
            capabilities,
        }
    }

    /// Derive relay ID from public key (first 16 chars of base64)
    fn derive_relay_id(public_key: &PublicKey) -> String {
        let b64 = public_key.to_base64();
        b64.chars().take(16).collect()
    }

    /// Check if payload is still valid (not expired)
    pub fn is_valid(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        let age = now - self.timestamp;
        // QR code valid for 5 minutes
        age >= 0 && age < 300
    }

    /// Check if this payload supports direct P2P connections
    pub fn supports_p2p(&self) -> bool {
        self.external_addr.is_some()
    }

    /// Serialize to JSON for QR code
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(self)?)
    }

    /// Parse from JSON
    pub fn from_json(json: &str) -> Result<Self> {
        Ok(serde_json::from_str(json)?)
    }
}

/// Pairing request sent from mobile to desktop (via relay)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingRequest {
    /// Mobile device's public key
    pub mobile_public_key: PublicKey,
    /// Mobile device's signature over (desktop_pubkey || timestamp)
    pub signature: Signature,
    /// User's Google OAuth email (for display)
    pub user_email: String,
    /// Device name (e.g., "iPhone 15 Pro")
    pub device_name: String,
    /// Timestamp of request
    pub timestamp: i64,
}

impl PairingRequest {
    /// Create a new pairing request
    pub fn new(
        mobile_keypair: &crate::Keypair,
        desktop_public_key: &PublicKey,
        user_email: String,
        device_name: String,
    ) -> Self {
        let timestamp = chrono::Utc::now().timestamp();

        // Sign (desktop_pubkey || timestamp) to prove we scanned the QR
        let mut message = Vec::new();
        message.extend_from_slice(desktop_public_key.as_bytes());
        message.extend_from_slice(&timestamp.to_le_bytes());

        let signature = mobile_keypair.sign(&message);

        Self {
            mobile_public_key: mobile_keypair.public_key(),
            signature,
            user_email,
            device_name,
            timestamp,
        }
    }

    /// Verify the pairing request signature
    pub fn verify(&self, desktop_public_key: &PublicKey) -> Result<()> {
        // Reconstruct the signed message
        let mut message = Vec::new();
        message.extend_from_slice(desktop_public_key.as_bytes());
        message.extend_from_slice(&self.timestamp.to_le_bytes());

        self.mobile_public_key.verify(&message, &self.signature)?;

        // Check timestamp is recent (within 1 minute)
        let now = chrono::Utc::now().timestamp();
        let age = now - self.timestamp;
        if age < 0 || age > 60 {
            anyhow::bail!("pairing request timestamp is invalid or expired");
        }

        Ok(())
    }
}

/// Pairing response sent from desktop to mobile (via relay)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingResponse {
    /// Whether pairing was approved
    pub approved: bool,
    /// Optional rejection reason
    pub reason: Option<String>,
}

impl PairingResponse {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Keypair;

    #[test]
    fn pairing_payload_roundtrip() {
        let keypair = Keypair::generate();
        let payload = PairingPayload::new(keypair.public_key());

        let json = payload.to_json().unwrap();
        let decoded = PairingPayload::from_json(&json).unwrap();

        assert_eq!(payload.desktop_public_key, decoded.desktop_public_key);
        assert_eq!(payload.relay_id, decoded.relay_id);
        assert_eq!(payload.version, decoded.version);
    }

    #[test]
    fn pairing_request_verify() {
        let desktop_keypair = Keypair::generate();
        let mobile_keypair = Keypair::generate();

        let request = PairingRequest::new(
            &mobile_keypair,
            &desktop_keypair.public_key(),
            "user@example.com".to_string(),
            "Test Device".to_string(),
        );

        // Should verify successfully
        request.verify(&desktop_keypair.public_key()).unwrap();

        // Should fail with wrong desktop key
        let wrong_keypair = Keypair::generate();
        assert!(request.verify(&wrong_keypair.public_key()).is_err());
    }

    #[test]
    fn pairing_payload_expiry() {
        let keypair = Keypair::generate();
        let mut payload = PairingPayload::new(keypair.public_key());

        // Fresh payload should be valid
        assert!(payload.is_valid());

        // Expired payload should be invalid
        payload.timestamp = chrono::Utc::now().timestamp() - 400;
        assert!(!payload.is_valid());

        // Future payload should be invalid
        payload.timestamp = chrono::Utc::now().timestamp() + 100;
        assert!(!payload.is_valid());
    }
}
