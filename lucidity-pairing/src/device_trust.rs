use crate::PublicKey;
use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// A trusted mobile device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedDevice {
    /// Device's public key
    pub public_key: PublicKey,
    /// User's email from Google OAuth
    pub user_email: String,
    /// Device name (e.g., "iPhone 15 Pro")
    pub device_name: String,
    /// When device was paired (unix timestamp)
    pub paired_at: i64,
    /// Last time device connected (unix timestamp)
    pub last_seen: Option<i64>,
}

/// Device trust store backed by SQLite
pub struct DeviceTrustStore {
    conn: Connection,
}

impl DeviceTrustStore {
    /// Open or create a device trust store at the given path
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        
        // Create tables if they don't exist
        conn.execute(
            "CREATE TABLE IF NOT EXISTS trusted_devices (
                public_key BLOB PRIMARY KEY,
                user_email TEXT NOT NULL,
                device_name TEXT NOT NULL,
                paired_at INTEGER NOT NULL,
                last_seen INTEGER
            )",
            [],
        )?;
        
        Ok(Self { conn })
    }

    /// Create an in-memory device trust store (for testing)
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        
        conn.execute(
            "CREATE TABLE trusted_devices (
                public_key BLOB PRIMARY KEY,
                user_email TEXT NOT NULL,
                device_name TEXT NOT NULL,
                paired_at INTEGER NOT NULL,
                last_seen INTEGER
            )",
            [],
        )?;
        
        Ok(Self { conn })
    }

    /// Add a trusted device
    pub fn add_device(&self, device: &TrustedDevice) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO trusted_devices 
             (public_key, user_email, device_name, paired_at, last_seen)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                device.public_key.as_bytes().as_slice(),
                device.user_email,
                device.device_name,
                device.paired_at,
                device.last_seen,
            ],
        )?;
        Ok(())
    }

    /// Get a trusted device by public key
    pub fn get_device(&self, public_key: &PublicKey) -> Result<Option<TrustedDevice>> {
        let mut stmt = self.conn.prepare(
            "SELECT public_key, user_email, device_name, paired_at, last_seen
             FROM trusted_devices
             WHERE public_key = ?1",
        )?;

        let mut rows = stmt.query(params![public_key.as_bytes().as_slice()])?;

        if let Some(row) = rows.next()? {
            let public_key_bytes: Vec<u8> = row.get(0)?;
            let mut public_key_arr = [0u8; 32];
            public_key_arr.copy_from_slice(&public_key_bytes);

            Ok(Some(TrustedDevice {
                public_key: PublicKey::from_base64(&base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(public_key_arr))?,
                user_email: row.get(1)?,
                device_name: row.get(2)?,
                paired_at: row.get(3)?,
                last_seen: row.get(4)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Check if a device is trusted
    pub fn is_trusted(&self, public_key: &PublicKey) -> Result<bool> {
        Ok(self.get_device(public_key)?.is_some())
    }

    /// List all trusted devices
    pub fn list_devices(&self) -> Result<Vec<TrustedDevice>> {
        let mut stmt = self.conn.prepare(
            "SELECT public_key, user_email, device_name, paired_at, last_seen
             FROM trusted_devices
             ORDER BY paired_at DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            let public_key_bytes: Vec<u8> = row.get(0)?;
            let mut public_key_arr = [0u8; 32];
            public_key_arr.copy_from_slice(&public_key_bytes);

            Ok(TrustedDevice {
                public_key: PublicKey::from_base64(&base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(public_key_arr))
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
                user_email: row.get(1)?,
                device_name: row.get(2)?,
                paired_at: row.get(3)?,
                last_seen: row.get(4)?,
            })
        })?;

        let mut devices = Vec::new();
        for device in rows {
            devices.push(device?);
        }
        Ok(devices)
    }

    /// Update last seen timestamp for a device
    pub fn update_last_seen(&self, public_key: &PublicKey, timestamp: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE trusted_devices SET last_seen = ?1 WHERE public_key = ?2",
            params![timestamp, public_key.as_bytes().as_slice()],
        )?;
        Ok(())
    }

    /// Remove a trusted device
    pub fn remove_device(&self, public_key: &PublicKey) -> Result<bool> {
        let rows_affected = self.conn.execute(
            "DELETE FROM trusted_devices WHERE public_key = ?1",
            params![public_key.as_bytes().as_slice()],
        )?;
        Ok(rows_affected > 0)
    }

    /// Count trusted devices
    pub fn count_devices(&self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM trusted_devices", [], |row| {
                row.get(0)
            })?;
        Ok(count as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Keypair;

    #[test]
    fn device_trust_store_crud() {
        let store = DeviceTrustStore::in_memory().unwrap();
        let keypair = Keypair::generate();

        let device = TrustedDevice {
            public_key: keypair.public_key(),
            user_email: "user@example.com".to_string(),
            device_name: "Test Device".to_string(),
            paired_at: chrono::Utc::now().timestamp(),
            last_seen: None,
        };

        // Add device
        store.add_device(&device).unwrap();
        assert_eq!(store.count_devices().unwrap(), 1);

        // Get device
        let retrieved = store.get_device(&device.public_key).unwrap().unwrap();
        assert_eq!(retrieved.user_email, device.user_email);
        assert_eq!(retrieved.device_name, device.device_name);

        // Check if trusted
        assert!(store.is_trusted(&device.public_key).unwrap());

        // Update last seen
        let now = chrono::Utc::now().timestamp();
        store.update_last_seen(&device.public_key, now).unwrap();
        let updated = store.get_device(&device.public_key).unwrap().unwrap();
        assert_eq!(updated.last_seen, Some(now));

        // Remove device
        assert!(store.remove_device(&device.public_key).unwrap());
        assert_eq!(store.count_devices().unwrap(), 0);
        assert!(!store.is_trusted(&device.public_key).unwrap());
    }

    #[test]
    fn list_devices_ordered() {
        let store = DeviceTrustStore::in_memory().unwrap();

        // Add devices with different timestamps
        for i in 0..3 {
            let keypair = Keypair::generate();
            let device = TrustedDevice {
                public_key: keypair.public_key(),
                user_email: format!("user{}@example.com", i),
                device_name: format!("Device {}", i),
                paired_at: 1000 + i,
                last_seen: None,
            };
            store.add_device(&device).unwrap();
        }

        let devices = store.list_devices().unwrap();
        assert_eq!(devices.len(), 3);
        
        // Should be ordered by paired_at DESC
        assert_eq!(devices[0].paired_at, 1002);
        assert_eq!(devices[1].paired_at, 1001);
        assert_eq!(devices[2].paired_at, 1000);
    }
}
