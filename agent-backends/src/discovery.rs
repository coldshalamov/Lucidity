//! Executable discovery and version probe abstractions.
//!
//! Discovery resolves a manifest's [`ExecutableDiscovery`] (candidate names
//! plus explicit paths) to a concrete executable location. The search
//! environment is injected: tests supply a deterministic
//! [`SearchPaths`], while production code may use
//! [`SystemPathLocator`] which reads the real `PATH`.
//!
//! Version probes run the manifest's probe template against the resolved
//! executable through direct process creation (never a shell). The raw
//! output is reduced by [`parse_version`], a pure function that extracts the
//! first semver-shaped token, so parsing is unit-testable without spawning.

use crate::template::{expand_args, TemplateContext, TemplateError};
use agent_protocol::{CommandTemplate, ExecutableDiscovery};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiscoveryError {
    /// No candidate resolved to an existing executable file.
    NotFound { attempted: Vec<PathBuf> },
}

impl fmt::Display for DiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { attempted } => write!(
                formatter,
                "no executable found; attempted: {}",
                attempted
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

impl std::error::Error for DiscoveryError {}

/// Resolves an [`ExecutableDiscovery`] to a concrete path.
pub trait ExecutableLocator {
    /// Return every candidate path in probe order, whether or not it exists.
    /// Used for diagnostics and deterministic negative tests.
    fn candidates(&self, discovery: &ExecutableDiscovery) -> Vec<PathBuf>;

    /// Return the first candidate that exists as a file.
    fn locate(&self, discovery: &ExecutableDiscovery) -> Result<PathBuf, DiscoveryError> {
        let attempted = self.candidates(discovery);
        attempted
            .iter()
            .find(|candidate| candidate.is_file())
            .cloned()
            .ok_or(DiscoveryError::NotFound { attempted })
    }
}

/// Deterministic locator over an explicit list of search directories.
/// Explicit manifest paths are checked first (relative to each search
/// directory unless already absolute), then each candidate name — with the
/// platform executable suffix applied on Windows.
#[derive(Clone, Debug, Default)]
pub struct SearchPaths {
    pub dirs: Vec<PathBuf>,
}

impl SearchPaths {
    pub fn new(dirs: impl IntoIterator<Item = PathBuf>) -> Self {
        Self {
            dirs: dirs.into_iter().collect(),
        }
    }

    /// Candidate file names for a discovery name on this platform.
    pub fn name_variants(name: &str) -> Vec<String> {
        #[cfg(windows)]
        {
            if name.to_ascii_lowercase().ends_with(".exe") {
                vec![name.to_owned()]
            } else {
                vec![name.to_owned(), format!("{name}.exe")]
            }
        }
        #[cfg(not(windows))]
        {
            vec![name.to_owned()]
        }
    }
}

impl ExecutableLocator for SearchPaths {
    fn candidates(&self, discovery: &ExecutableDiscovery) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for explicit in &discovery.explicit_paths {
            if explicit.is_absolute() {
                out.push(explicit.clone());
            } else {
                for dir in &self.dirs {
                    out.push(dir.join(explicit));
                }
            }
        }
        for name in &discovery.names {
            for dir in &self.dirs {
                for variant in Self::name_variants(name) {
                    out.push(dir.join(variant));
                }
            }
        }
        out
    }
}

/// Production locator that searches the process `PATH`. Explicit relative
/// paths are resolved against the current directory.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemPathLocator;

impl ExecutableLocator for SystemPathLocator {
    fn candidates(&self, discovery: &ExecutableDiscovery) -> Vec<PathBuf> {
        let mut dirs: Vec<PathBuf> = std::env::current_dir().into_iter().collect();
        dirs.extend(
            std::env::var_os("PATH")
                .map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
                .unwrap_or_default(),
        );
        SearchPaths::new(dirs).candidates(discovery)
    }
}

#[derive(Debug)]
pub enum ProbeError {
    /// The probe template failed validation or expansion.
    Template(TemplateError),
    /// The probe process could not be spawned.
    Spawn(io::Error),
    /// The probe exceeded its bounded runtime.
    Timeout,
    /// The probe exited non-zero or produced undecodable output.
    Failed { status: Option<i32>, stderr: String },
    /// The probe output contained no semver-shaped token.
    Unparseable { output: String },
}

impl fmt::Display for ProbeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Template(error) => write!(formatter, "probe template error: {error}"),
            Self::Spawn(error) => write!(formatter, "probe spawn failed: {error}"),
            Self::Timeout => write!(formatter, "probe timed out"),
            Self::Failed { status, stderr } => write!(
                formatter,
                "probe exited with status {status:?}: {}",
                stderr.trim()
            ),
            Self::Unparseable { output } => write!(
                formatter,
                "no version token found in probe output: {}",
                output.trim()
            ),
        }
    }
}

impl std::error::Error for ProbeError {}

/// Runs version probes through direct process creation with a bounded
/// runtime. Never involves a shell; arguments come from
/// [`crate::template::expand_args`] verbatim.
pub trait VersionProber {
    fn probe(&self, executable: &Path, probe: &CommandTemplate) -> Result<String, ProbeError>;
}

/// Default probe bound. Probing is a launch-time operation and must never
/// hang adapter scanning.
pub const DEFAULT_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug)]
pub struct CommandVersionProber {
    pub timeout: Duration,
}

impl Default for CommandVersionProber {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_PROBE_TIMEOUT,
        }
    }
}

impl VersionProber for CommandVersionProber {
    fn probe(&self, executable: &Path, probe: &CommandTemplate) -> Result<String, ProbeError> {
        let args = expand_args(probe, &TemplateContext::default()).map_err(ProbeError::Template)?;
        let mut child = Command::new(executable)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(ProbeError::Spawn)?;

        let deadline = Instant::now() + self.timeout;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(ProbeError::Timeout);
                }
                Err(error) => return Err(ProbeError::Spawn(error)),
            }
        }
        let output = child.wait_with_output().map_err(ProbeError::Spawn)?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        if !output.status.success() {
            return Err(ProbeError::Failed {
                status: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        parse_version(&stdout).ok_or(ProbeError::Unparseable { output: stdout })
    }
}

/// Extract the first semver-shaped token (`digits.digits[.digits][suffix]`)
/// from free-form probe output. Pure and deterministic.
pub fn parse_version(output: &str) -> Option<String> {
    for token in output.split(|c: char| c.is_whitespace() || c == ',') {
        let trimmed = token.trim_start_matches(['v', 'V']);
        let mut parts = trimmed.split('.');
        let first = parts.next()?;
        if first.is_empty() || !first.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let mut dotted = first.to_owned();
        let mut count = 1;
        for part in parts {
            let digits: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
            if digits.is_empty() {
                break;
            }
            dotted.push('.');
            dotted.push_str(&digits);
            count += 1;
            if part.len() > digits.len() {
                break;
            }
            if count == 3 {
                break;
            }
        }
        if count >= 2 {
            return Some(dotted);
        }
    }
    None
}
