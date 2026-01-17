use anyhow::Result;
use base64::Engine;
use ed25519_dalek::{Signer, Verifier};
use serde::{Deserialize, Serialize};


/// Ed25519 public key for device identity
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicKey(#[serde(with = "base64_serde")] [u8; 32]);

/// Ed25519 signature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signature(#[serde(with = "base64_serde")] [u8; 64]);

/// Ed25519 keypair for device identity
pub struct Keypair {
    signing_key: ed25519_dalek::SigningKey,
}

impl Keypair {
    /// Generate a new random keypair
    pub fn generate() -> Self {
        let mut rng = rand::thread_rng();
        let signing_key = ed25519_dalek::SigningKey::generate(&mut rng);
        Self { signing_key }
    }

    /// Load keypair from secret bytes
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        let signing_key = ed25519_dalek::SigningKey::from_bytes(bytes);
        Self { signing_key }
    }

    /// Get the secret key bytes (for storage)
    pub fn to_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }

    /// Get the public key
    pub fn public_key(&self) -> PublicKey {
        let verifying_key = self.signing_key.verifying_key();
        PublicKey(verifying_key.to_bytes())
    }

    /// Sign a message
    pub fn sign(&self, message: &[u8]) -> Signature {
        let sig = self.signing_key.sign(message);
        Signature(sig.to_bytes())
    }
}

impl PublicKey {
    /// Verify a signature on a message
    pub fn verify(&self, message: &[u8], signature: &Signature) -> Result<()> {
        let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(&self.0)?;
        let sig = ed25519_dalek::Signature::from_bytes(&signature.0);
        verifying_key
            .verify(message, &sig)
            .map_err(|e| anyhow::anyhow!("signature verification failed: {}", e))
    }

    /// Convert to base64 string for QR codes
    pub fn to_base64(&self) -> String {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(self.0)
    }

    /// A short, human-readable fingerprint suitable for UI display.
    pub fn fingerprint_short(&self) -> String {
        let b64 = self.to_base64();
        if b64.len() <= 16 {
            return b64;
        }

        let prefix: String = b64.chars().take(8).collect();
        let suffix: String = b64.chars().rev().take(6).collect::<String>().chars().rev().collect();
        format!("{prefix}…{suffix}")
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Parse from base64 string
    pub fn from_base64(s: &str) -> Result<Self> {
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(s)?;
        if bytes.len() != 32 {
            anyhow::bail!("invalid public key length: {}", bytes.len());
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self::from_bytes(arr))
    }

    /// Get raw bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

}

impl Signature {
    /// Convert to base64 string
    pub fn to_base64(&self) -> String {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(self.0)
    }

    /// Parse from base64 string
    pub fn from_base64(s: &str) -> Result<Self> {
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(s)?;
        if bytes.len() != 64 {
            anyhow::bail!("invalid signature length: {}", bytes.len());
        }
        let mut arr = [0u8; 64];
        arr.copy_from_slice(&bytes);
        Ok(Signature(arr))
    }
}

// Helper module for base64 serialization
mod base64_serde {
    use base64::Engine;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S, const N: usize>(bytes: &[u8; N], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        serializer.serialize_str(&encoded)
    }

    pub fn deserialize<'de, D, const N: usize>(deserializer: D) -> Result<[u8; N], D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(s)
            .map_err(serde::de::Error::custom)?;
        if bytes.len() != N {
            return Err(serde::de::Error::custom(format!(
                "expected {} bytes, got {}",
                N,
                bytes.len()
            )));
        }
        let mut arr = [0u8; N];
        arr.copy_from_slice(&bytes);
        Ok(arr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keypair_sign_verify() {
        let keypair = Keypair::generate();
        let message = b"hello world";
        let signature = keypair.sign(message);

        let public_key = keypair.public_key();
        public_key.verify(message, &signature).unwrap();
    }

    #[test]
    fn public_key_base64_roundtrip() {
        let keypair = Keypair::generate();
        let public_key = keypair.public_key();

        let encoded = public_key.to_base64();
        let decoded = PublicKey::from_base64(&encoded).unwrap();

        assert_eq!(public_key, decoded);
    }

    #[test]
    fn signature_base64_roundtrip() {
        let keypair = Keypair::generate();
        let signature = keypair.sign(b"test");

        let encoded = signature.to_base64();
        let decoded = Signature::from_base64(&encoded).unwrap();

        // Verify the decoded signature works
        keypair.public_key().verify(b"test", &decoded).unwrap();
    }
}
