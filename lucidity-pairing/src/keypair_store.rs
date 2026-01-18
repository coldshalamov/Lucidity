use crate::Keypair;
use anyhow::Context;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct KeypairStore {
    path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
struct KeypairFileV1 {
    version: u8,
    secret_key_b64: String,
}

impl KeypairStore {
    pub fn open(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    pub fn load(&self) -> anyhow::Result<Option<Keypair>> {
        let bytes = match fs::read(&self.path) {
            Ok(b) => b,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => {
                return Err(err).with_context(|| format!("reading {}", self.path.display()))
            }
        };

        let json = String::from_utf8(bytes).context("keypair store file is not utf-8")?;
        let file: KeypairFileV1 = serde_json::from_str(&json).context("parsing keypair json")?;

        if file.version != 1 {
            anyhow::bail!("unsupported keypair store version: {}", file.version);
        }

        let secret = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(file.secret_key_b64.as_bytes())
            .context("decoding base64 secret key")?;
        if secret.len() != 32 {
            anyhow::bail!("invalid secret key length: {}", secret.len());
        }

        let mut arr = [0u8; 32];
        arr.copy_from_slice(&secret);
        Ok(Some(Keypair::from_bytes(&arr)))
    }

    pub fn save(&self, keypair: &Keypair) -> anyhow::Result<()> {
        let parent = self.path.parent();
        if let Some(parent) = parent {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }

        let file = KeypairFileV1 {
            version: 1,
            secret_key_b64: base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(keypair.to_bytes()),
        };

        let json = serde_json::to_string_pretty(&file)?;
        fs::write(&self.path, json).with_context(|| format!("writing {}", self.path.display()))?;
        Ok(())
    }

    pub fn load_or_generate(&self) -> anyhow::Result<Keypair> {
        if let Some(k) = self.load()? {
            return Ok(k);
        }
        let k = Keypair::generate();
        self.save(&k)?;
        Ok(k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_load_or_generate_is_stable() {
        let dir = tempfile::tempdir().unwrap();
        let store = KeypairStore::open(dir.path().join("host_key.json"));

        let a = store.load_or_generate().unwrap();
        let b = store.load_or_generate().unwrap();

        assert_eq!(a.public_key(), b.public_key());
    }
}
