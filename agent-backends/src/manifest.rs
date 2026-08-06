//! Declarative adapter package parsing and schema/semantic validation.
//!
//! A community adapter package is:
//!
//! ```text
//! <id>.agent-adapter/
//! ├── adapter.toml
//! ├── settings.schema.json   (optional)
//! ├── icon.svg               (optional)
//! ├── fixtures/              (optional)
//! └── hooks/                 (optional, separately reviewed and approved)
//! ```
//!
//! [`parse_manifest`] decodes `adapter.toml` into the frozen
//! [`agent_protocol::AdapterManifest`] shape. [`validate_manifest`] then
//! applies the semantic rules the serde layer cannot express: supported
//! schema version, identifier charset, forbidden shell interpolation,
//! placeholder correctness, relative/bounded package paths, and known event
//! capability names. Validation collects every problem rather than stopping
//! at the first so adapter authors see a complete report.

use crate::template::{validate_template, TemplateError};
use agent_protocol::{AdapterManifest, AgentEventKind};
use std::fmt;
use std::path::{Component, Path, PathBuf};

/// The only adapter manifest schema version this implementation accepts.
pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;
/// Maximum number of hook destinations per hook; hooks are bounded by
/// contract so a compromised package cannot scatter writes.
pub const MAX_HOOK_DESTINATIONS: usize = 16;
/// Maximum accepted adapter id length.
pub const MAX_ADAPTER_ID_LEN: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManifestError {
    /// The TOML document could not be decoded into the frozen schema.
    Decode(String),
    /// The TOML document contains a key outside the frozen manifest schema.
    UnknownField { field: String },
    /// `schemaVersion` is not [`SUPPORTED_SCHEMA_VERSION`].
    UnsupportedSchemaVersion { found: u32 },
    /// The adapter id is empty, too long, or has invalid characters.
    InvalidId { id: String, reason: String },
    /// `displayName` is empty.
    EmptyDisplayName,
    /// A command template failed validation; `field` names its location.
    InvalidTemplate { field: String, error: TemplateError },
    /// The resume template exists but never references `{nativeSessionId}`.
    ResumeMissingSessionPlaceholder,
    /// A package-relative path is absolute, empty, or escapes the package.
    PathEscapesPackage { field: String, path: PathBuf },
    /// A history source, binding, provider, or fixture declared an empty or
    /// unknown kind string.
    InvalidKind { field: String, kind: String },
    /// `eventCapabilities` contains a name outside the frozen core set.
    UnknownEventCapability { name: String },
    /// A hook has no description, no destinations, or too many destinations.
    InvalidHook { name: String, reason: String },
    /// An expected package file is missing on disk.
    MissingPackageFile { path: PathBuf },
    /// An I/O error occurred while reading the package.
    Io { path: PathBuf, message: String },
}

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(message) => write!(formatter, "adapter.toml decode error: {message}"),
            Self::UnknownField { field } => {
                write!(formatter, "unknown adapter manifest field {field}")
            }
            Self::UnsupportedSchemaVersion { found } => write!(
                formatter,
                "unsupported adapter schema version {found} (expected {SUPPORTED_SCHEMA_VERSION})"
            ),
            Self::InvalidId { id, reason } => {
                write!(formatter, "invalid adapter id {id:?}: {reason}")
            }
            Self::EmptyDisplayName => write!(formatter, "displayName must not be empty"),
            Self::InvalidTemplate { field, error } => {
                write!(formatter, "invalid command template at {field}: {error}")
            }
            Self::ResumeMissingSessionPlaceholder => write!(
                formatter,
                "resume template must reference {{nativeSessionId}}"
            ),
            Self::PathEscapesPackage { field, path } => write!(
                formatter,
                "path at {field} must be relative and stay inside the package: {}",
                path.display()
            ),
            Self::InvalidKind { field, kind } => {
                write!(formatter, "empty or unsupported kind {kind:?} at {field}")
            }
            Self::UnknownEventCapability { name } => {
                write!(formatter, "unknown event capability {name:?}")
            }
            Self::InvalidHook { name, reason } => {
                write!(formatter, "invalid hook {name:?}: {reason}")
            }
            Self::MissingPackageFile { path } => {
                write!(formatter, "package file is missing: {}", path.display())
            }
            Self::Io { path, message } => {
                write!(formatter, "I/O error at {}: {message}", path.display())
            }
        }
    }
}

impl std::error::Error for ManifestError {}

/// Decode an `adapter.toml` document. Schema-shape errors (unknown fields,
/// wrong types, missing mandatory fields) surface here.
pub fn parse_manifest(document: &str) -> Result<AdapterManifest, ManifestError> {
    reject_unknown_fields(document)?;
    toml::from_str(document).map_err(|error| ManifestError::Decode(error.to_string()))
}

/// History source kinds accepted for the protected demo.
pub const KNOWN_HISTORY_KINDS: [&str; 2] = ["jsonl", "directory"];
/// Fixture kinds accepted for the protected demo.
pub const KNOWN_FIXTURE_KINDS: [&str; 3] = ["jsonl", "json", "transcript"];
/// Provider/binding kinds accepted for the protected demo.
pub const KNOWN_BINDING_KINDS: [&str; 3] = ["argument", "command", "session_store"];

/// Apply every semantic rule, returning all violations found.
pub fn validate_manifest(manifest: &AdapterManifest) -> Result<(), Vec<ManifestError>> {
    let mut errors = Vec::new();

    if manifest.schema_version != SUPPORTED_SCHEMA_VERSION {
        errors.push(ManifestError::UnsupportedSchemaVersion {
            found: manifest.schema_version,
        });
    }

    validate_id(&manifest.id.0, &mut errors);

    if manifest.display_name.trim().is_empty() {
        errors.push(ManifestError::EmptyDisplayName);
    }

    check_template("launch", &manifest.launch, &mut errors);
    if let Some(resume) = &manifest.resume {
        check_template("resume", resume, &mut errors);
        let references_session = resume
            .args
            .iter()
            .any(|arg| arg.contains("{nativeSessionId}"));
        if !references_session {
            errors.push(ManifestError::ResumeMissingSessionPlaceholder);
        }
    }
    if let Some(probe) = &manifest.executable.version_probe {
        check_template("executable.versionProbe", probe, &mut errors);
    }

    for (index, source) in manifest.history_sources.iter().enumerate() {
        let field = format!("historySources[{index}]");
        if !KNOWN_HISTORY_KINDS.contains(&source.kind.as_str()) {
            errors.push(ManifestError::InvalidKind {
                field: field.clone(),
                kind: source.kind.clone(),
            });
        }
        check_relative(&field, &source.path_template, &mut errors);
    }

    if let Some(schema) = &manifest.settings_schema {
        check_relative("settingsSchema", schema, &mut errors);
    }

    for (name, binding) in &manifest.settings_bindings {
        check_binding(&format!("settingsBindings.{name}"), binding, &mut errors);
    }
    if let Some(provider) = &manifest.usage_provider {
        check_binding("usageProvider", provider, &mut errors);
    }
    if let Some(provider) = &manifest.context_provider {
        check_binding("contextProvider", provider, &mut errors);
    }

    for capability in &manifest.event_capabilities {
        if let AgentEventKind::Unknown(name) = capability {
            errors.push(ManifestError::UnknownEventCapability { name: name.clone() });
        }
    }

    for (name, fixture) in &manifest.fixtures {
        let field = format!("fixtures.{name}");
        if !KNOWN_FIXTURE_KINDS.contains(&fixture.kind.as_str()) {
            errors.push(ManifestError::InvalidKind {
                field: field.clone(),
                kind: fixture.kind.clone(),
            });
        }
        check_relative(&field, &fixture.path, &mut errors);
    }

    for (name, hook) in &manifest.hooks {
        if hook.description.trim().is_empty() {
            errors.push(ManifestError::InvalidHook {
                name: name.clone(),
                reason: "description must not be empty".to_owned(),
            });
        }
        if hook.destinations.is_empty() {
            errors.push(ManifestError::InvalidHook {
                name: name.clone(),
                reason: "at least one destination is required".to_owned(),
            });
        }
        if hook.destinations.len() > MAX_HOOK_DESTINATIONS {
            errors.push(ManifestError::InvalidHook {
                name: name.clone(),
                reason: format!(
                    "at most {MAX_HOOK_DESTINATIONS} destinations are allowed, found {}",
                    hook.destinations.len()
                ),
            });
        }
        check_template(&format!("hooks.{name}.apply"), &hook.apply, &mut errors);
        check_template(
            &format!("hooks.{name}.rollback"),
            &hook.rollback,
            &mut errors,
        );
        for destination in &hook.destinations {
            check_relative(
                &format!("hooks.{name}.destinations"),
                destination,
                &mut errors,
            );
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Parse and semantically validate an `adapter.toml` document in one step.
pub fn load_manifest(document: &str) -> Result<AdapterManifest, Vec<ManifestError>> {
    let manifest = parse_manifest(document).map_err(|error| vec![error])?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

/// Load and validate a complete adapter package directory. Beyond manifest
/// validation this verifies that the package files the manifest references
/// (settings schema, fixtures) actually exist inside the package root.
pub fn load_package(root: &Path) -> Result<AdapterManifest, Vec<ManifestError>> {
    let manifest_path = root.join("adapter.toml");
    let document = std::fs::read_to_string(&manifest_path).map_err(|error| {
        vec![ManifestError::Io {
            path: manifest_path.clone(),
            message: error.to_string(),
        }]
    })?;
    let manifest = load_manifest(&document)?;

    let mut errors = Vec::new();
    let mut require_file = |relative: &PathBuf| {
        let candidate = root.join(relative);
        if !candidate.is_file() {
            errors.push(ManifestError::MissingPackageFile { path: candidate });
        }
    };
    if let Some(schema) = &manifest.settings_schema {
        require_file(schema);
    }
    for fixture in manifest.fixtures.values() {
        require_file(&fixture.path);
    }
    if errors.is_empty() {
        Ok(manifest)
    } else {
        Err(errors)
    }
}

fn validate_id(id: &str, errors: &mut Vec<ManifestError>) {
    if id.is_empty() {
        errors.push(ManifestError::InvalidId {
            id: id.to_owned(),
            reason: "must not be empty".to_owned(),
        });
        return;
    }
    if id.len() > MAX_ADAPTER_ID_LEN {
        errors.push(ManifestError::InvalidId {
            id: id.to_owned(),
            reason: format!("must be at most {MAX_ADAPTER_ID_LEN} characters"),
        });
    }
    let valid = id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !id.starts_with('-')
        && !id.ends_with('-');
    if !valid {
        errors.push(ManifestError::InvalidId {
            id: id.to_owned(),
            reason: "must match [a-z0-9] segments joined by single dashes".to_owned(),
        });
    }
}

fn check_template(
    field: &str,
    template: &agent_protocol::CommandTemplate,
    errors: &mut Vec<ManifestError>,
) {
    if let Err(error) = validate_template(template) {
        errors.push(ManifestError::InvalidTemplate {
            field: field.to_owned(),
            error,
        });
    }
}

fn check_binding(
    field: &str,
    binding: &agent_protocol::AdapterBinding,
    errors: &mut Vec<ManifestError>,
) {
    if !KNOWN_BINDING_KINDS.contains(&binding.kind.as_str()) {
        errors.push(ManifestError::InvalidKind {
            field: field.to_owned(),
            kind: binding.kind.clone(),
        });
    }
}

/// A package path must be relative, non-empty, and free of `..`/prefix
/// components so it cannot escape the package root.
fn check_relative(field: &str, path: &Path, errors: &mut Vec<ManifestError>) {
    let ok = !path.as_os_str().is_empty()
        && path.is_relative()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
    if !ok {
        errors.push(ManifestError::PathEscapesPackage {
            field: field.to_owned(),
            path: path.to_path_buf(),
        });
    }
}

fn reject_unknown_fields(document: &str) -> Result<(), ManifestError> {
    let value = document
        .parse::<toml::Value>()
        .map_err(|error| ManifestError::Decode(error.to_string()))?;
    let Some(root) = value.as_table() else {
        return Err(ManifestError::Decode(
            "adapter.toml must be a table".to_owned(),
        ));
    };

    check_table(
        "",
        root,
        &[
            "schemaVersion",
            "id",
            "displayName",
            "executable",
            "launch",
            "resume",
            "historySources",
            "eventCapabilities",
            "settingsSchema",
            "settingsBindings",
            "usageProvider",
            "contextProvider",
            "fixtures",
            "hooks",
            "extensions",
        ],
    )?;

    if let Some(table) = root.get("executable").and_then(toml::Value::as_table) {
        check_table(
            "executable",
            table,
            &["names", "explicitPaths", "versionProbe"],
        )?;
        if let Some(probe) = table.get("versionProbe").and_then(toml::Value::as_table) {
            check_command_template("executable.versionProbe", probe)?;
        }
    }
    if let Some(table) = root.get("launch").and_then(toml::Value::as_table) {
        check_command_template("launch", table)?;
    }
    if let Some(table) = root.get("resume").and_then(toml::Value::as_table) {
        check_command_template("resume", table)?;
    }
    if let Some(items) = root.get("historySources").and_then(toml::Value::as_array) {
        for (index, item) in items.iter().enumerate() {
            if let Some(table) = item.as_table() {
                check_table(
                    &format!("historySources[{index}]"),
                    table,
                    &["kind", "pathTemplate"],
                )?;
            }
        }
    }
    if let Some(table) = root.get("settingsBindings").and_then(toml::Value::as_table) {
        for (name, binding) in table {
            if let Some(binding) = binding.as_table() {
                check_binding_table(&format!("settingsBindings.{name}"), binding)?;
            }
        }
    }
    for provider in ["usageProvider", "contextProvider"] {
        if let Some(table) = root.get(provider).and_then(toml::Value::as_table) {
            check_binding_table(provider, table)?;
        }
    }
    if let Some(table) = root.get("fixtures").and_then(toml::Value::as_table) {
        for (name, fixture) in table {
            if let Some(fixture) = fixture.as_table() {
                check_table(&format!("fixtures.{name}"), fixture, &["kind", "path"])?;
            }
        }
    }
    if let Some(table) = root.get("hooks").and_then(toml::Value::as_table) {
        for (name, hook) in table {
            if let Some(hook) = hook.as_table() {
                let prefix = format!("hooks.{name}");
                check_table(
                    &prefix,
                    hook,
                    &["description", "apply", "rollback", "destinations"],
                )?;
                if let Some(apply) = hook.get("apply").and_then(toml::Value::as_table) {
                    check_command_template(&format!("{prefix}.apply"), apply)?;
                }
                if let Some(rollback) = hook.get("rollback").and_then(toml::Value::as_table) {
                    check_command_template(&format!("{prefix}.rollback"), rollback)?;
                }
            }
        }
    }

    Ok(())
}

fn check_binding_table(
    field: &str,
    table: &toml::map::Map<String, toml::Value>,
) -> Result<(), ManifestError> {
    check_table(field, table, &["kind", "config"])
}

fn check_command_template(
    field: &str,
    table: &toml::map::Map<String, toml::Value>,
) -> Result<(), ManifestError> {
    check_table(field, table, &["executable", "args", "shell"])
}

fn check_table(
    prefix: &str,
    table: &toml::map::Map<String, toml::Value>,
    allowed: &[&str],
) -> Result<(), ManifestError> {
    for key in table.keys() {
        if !allowed.contains(&key.as_str()) {
            let field = if prefix.is_empty() {
                key.to_owned()
            } else {
                format!("{prefix}.{key}")
            };
            return Err(ManifestError::UnknownField { field });
        }
    }
    Ok(())
}
