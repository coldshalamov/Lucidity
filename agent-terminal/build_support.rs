use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContentFingerprint {
    pub byte_len: u64,
    pub sha256: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProvisionOutcome {
    pub copied: bool,
    pub source: ContentFingerprint,
}

pub fn profile_output_dir(out_dir: &Path) -> Result<PathBuf> {
    out_dir
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .context("OUT_DIR does not have Cargo's <profile>/build/<package>/out shape")
}

pub fn content_fingerprint(path: &Path) -> Result<ContentFingerprint> {
    let mut file = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let byte_len = file.metadata()?.len();
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(ContentFingerprint {
        byte_len,
        sha256: hasher.finalize().into(),
    })
}

pub fn provision_if_stale(source: &Path, destination: &Path) -> Result<ProvisionOutcome> {
    let source_fingerprint = content_fingerprint(source)?;
    let is_current = destination
        .is_file()
        .then(|| content_fingerprint(destination))
        .transpose()?
        .is_some_and(|destination_fingerprint| destination_fingerprint == source_fingerprint);

    if !is_current {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source, destination)
            .with_context(|| format!("copy {} -> {}", source.display(), destination.display()))?;
        let copied_fingerprint = content_fingerprint(destination)?;
        anyhow::ensure!(
            copied_fingerprint == source_fingerprint,
            "copied runtime file failed content-hash verification: {}",
            destination.display()
        );
    }

    Ok(ProvisionOutcome {
        copied: !is_current,
        source: source_fingerprint,
    })
}
