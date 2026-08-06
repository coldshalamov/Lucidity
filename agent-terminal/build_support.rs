use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub const BUILD_VERSION_ENV: &str = "LUCIDITY_BUILD_VERSION";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuildVersionSource {
    Environment,
    Tag,
    Git,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitWatchPaths {
    pub head: PathBuf,
    pub symbolic_ref: Option<PathBuf>,
    pub packed_refs: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildVersionResolution {
    pub version: String,
    pub source: BuildVersionSource,
    pub tag_path: PathBuf,
    pub git_watch_paths: Option<GitWatchPaths>,
}

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

pub fn resolve_build_version(repo_dir: &Path) -> Result<BuildVersionResolution> {
    let tag_path = repo_dir.join(".tag");
    let git_watch_paths = discover_git_watch_paths(repo_dir)?;
    let explicit_version = match env::var(BUILD_VERSION_ENV) {
        Ok(version) => Some(version),
        Err(env::VarError::NotPresent) => None,
        Err(env::VarError::NotUnicode(_)) => {
            bail!("{BUILD_VERSION_ENV} is not valid Unicode")
        }
    };
    let tag_version = match fs::read_to_string(&tag_path) {
        Ok(version) => Some(version),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(error)
                .with_context(|| format!("read source archive tag {}", tag_path.display()))
        }
    };

    let (version, source) = match select_declared_build_version(
        explicit_version.as_deref(),
        tag_version.as_deref(),
    )? {
        Some(version) => version,
        None if git_watch_paths.is_some() => (git_build_version(repo_dir)?, BuildVersionSource::Git),
        None => bail!(
            "cannot resolve Lucidity build version: {BUILD_VERSION_ENV} is unset, {} is absent, and Git metadata is unavailable",
            tag_path.display()
        ),
    };

    Ok(BuildVersionResolution {
        version,
        source,
        tag_path,
        git_watch_paths,
    })
}

pub fn select_declared_build_version(
    explicit_version: Option<&str>,
    tag_version: Option<&str>,
) -> Result<Option<(String, BuildVersionSource)>> {
    if let Some(version) = explicit_version {
        return Ok(Some((
            validate_build_version(version, BUILD_VERSION_ENV)?,
            BuildVersionSource::Environment,
        )));
    }
    if let Some(version) = tag_version {
        return Ok(Some((
            validate_build_version(version.trim(), ".tag")?,
            BuildVersionSource::Tag,
        )));
    }
    Ok(None)
}

pub fn validate_build_version(version: &str, source: &str) -> Result<String> {
    anyhow::ensure!(!version.is_empty(), "{source} build version is empty");
    anyhow::ensure!(
        !version.eq_ignore_ascii_case("UNKNOWN"),
        "{source} build version may not use the UNKNOWN sentinel"
    );
    anyhow::ensure!(
        version.len() <= 128,
        "{source} build version exceeds 128 bytes"
    );
    anyhow::ensure!(
        version.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_' | '+')
        }),
        "{source} build version contains characters unsafe for a Windows version resource"
    );
    Ok(version.to_owned())
}

pub fn discover_git_watch_paths(repo_dir: &Path) -> Result<Option<GitWatchPaths>> {
    let Some(repository_probe) =
        optional_git_output(repo_dir, &["rev-parse", "--is-inside-work-tree"])
    else {
        return Ok(None);
    };
    if !repository_probe.status.success()
        || String::from_utf8_lossy(&repository_probe.stdout).trim() != "true"
    {
        return Ok(None);
    }

    let head = git_path(repo_dir, "HEAD")?;
    let packed_refs = git_path(repo_dir, "packed-refs")?;
    let symbolic_ref_output = required_git_output(repo_dir, &["symbolic-ref", "-q", "HEAD"])?;
    let symbolic_ref = if symbolic_ref_output.status.success() {
        let reference = String::from_utf8(symbolic_ref_output.stdout)
            .context("Git symbolic ref is not UTF-8")?;
        let reference = reference.trim();
        anyhow::ensure!(!reference.is_empty(), "Git returned an empty symbolic ref");
        Some(git_path(repo_dir, reference)?)
    } else if symbolic_ref_output.status.code() == Some(1) {
        None
    } else {
        bail!(
            "git symbolic-ref -q HEAD failed: {}",
            String::from_utf8_lossy(&symbolic_ref_output.stderr).trim()
        )
    };

    Ok(Some(GitWatchPaths {
        head,
        symbolic_ref,
        packed_refs,
    }))
}

fn git_path(repo_dir: &Path, name: &str) -> Result<PathBuf> {
    let output = required_git_output(
        repo_dir,
        &["rev-parse", "--path-format=absolute", "--git-path", name],
    )?;
    anyhow::ensure!(
        output.status.success(),
        "git rev-parse --git-path {name} failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    let path = PathBuf::from(
        String::from_utf8(output.stdout)
            .context("Git metadata path is not UTF-8")?
            .trim(),
    );
    anyhow::ensure!(
        path.is_absolute(),
        "Git metadata path is not absolute: {}",
        path.display()
    );
    Ok(path)
}

fn git_build_version(repo_dir: &Path) -> Result<String> {
    let output = required_git_output(
        repo_dir,
        &[
            "-c",
            "core.abbrev=8",
            "show",
            "-s",
            "--format=%cd-%h",
            "--date=format:%Y%m%d-%H%M%S",
        ],
    )?;
    anyhow::ensure!(
        output.status.success(),
        "Git could not resolve the Lucidity build version: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    let version = String::from_utf8(output.stdout).context("Git build version is not UTF-8")?;
    validate_build_version(version.trim(), "Git")
}

fn optional_git_output(repo_dir: &Path, args: &[&str]) -> Option<Output> {
    Command::new("git")
        .current_dir(repo_dir)
        .args(args)
        .output()
        .ok()
}

fn required_git_output(repo_dir: &Path, args: &[&str]) -> Result<Output> {
    Command::new("git")
        .current_dir(repo_dir)
        .args(args)
        .output()
        .with_context(|| format!("run git {}", args.join(" ")))
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
