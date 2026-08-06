//! Windows-safe direct executable-plus-args template expansion.
//!
//! Adapter manifests describe launch, resume, and version-probe commands as
//! [`CommandTemplate`] values. Expansion substitutes `{placeholder}` tokens
//! with caller-supplied values and returns an argv vector suitable for direct
//! process creation. There is intentionally no shell: values are inserted
//! verbatim, never quoted, escaped, globbed, or re-parsed, so hostile
//! argument content (metacharacters, quotes, percent signs, command
//! substitution syntax) cannot change the meaning of the command line.
//!
//! `shell = true` is rejected here and at manifest validation time; the
//! declarative contract is direct executable-plus-args only.

use agent_protocol::CommandTemplate;
use std::fmt;

/// Placeholders recognized by the adapter contract.
pub const KNOWN_PLACEHOLDERS: [&str; 3] = ["projectPath", "nativeSessionId", "profileId"];

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TemplateError {
    /// The template opts into shell interpolation, which is forbidden.
    ShellInterpolationForbidden,
    /// The executable name/path is empty.
    EmptyExecutable,
    /// An arg contains `{` without a closing `}`.
    UnterminatedPlaceholder { arg: String },
    /// An arg contains `}` without a matching `{`.
    UnexpectedClosingBrace { arg: String },
    /// An arg contains an empty `{}` placeholder.
    EmptyPlaceholder { arg: String },
    /// An arg references a placeholder outside the contract set.
    UnknownPlaceholder { name: String, arg: String },
    /// Expansion requires a value the caller did not supply.
    MissingValue { name: String },
}

impl fmt::Display for TemplateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ShellInterpolationForbidden => {
                write!(
                    formatter,
                    "shell interpolation is forbidden by the adapter contract"
                )
            }
            Self::EmptyExecutable => write!(formatter, "command template executable is empty"),
            Self::UnterminatedPlaceholder { arg } => {
                write!(formatter, "unterminated placeholder in argument {arg:?}")
            }
            Self::UnexpectedClosingBrace { arg } => {
                write!(
                    formatter,
                    "unexpected closing placeholder brace in argument {arg:?}"
                )
            }
            Self::EmptyPlaceholder { arg } => {
                write!(formatter, "empty placeholder in argument {arg:?}")
            }
            Self::UnknownPlaceholder { name, arg } => {
                write!(
                    formatter,
                    "unknown placeholder {{{name}}} in argument {arg:?}"
                )
            }
            Self::MissingValue { name } => {
                write!(formatter, "no value supplied for placeholder {{{name}}}")
            }
        }
    }
}

impl std::error::Error for TemplateError {}

/// Values available to template expansion. All fields are optional; a
/// template referencing an absent value fails with
/// [`TemplateError::MissingValue`] rather than substituting empty text.
#[derive(Clone, Copy, Debug, Default)]
pub struct TemplateContext<'a> {
    pub project_path: Option<&'a str>,
    pub native_session_id: Option<&'a str>,
    pub profile_id: Option<&'a str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpandedCommand {
    pub executable: String,
    pub args: Vec<String>,
}

impl<'a> TemplateContext<'a> {
    fn value(&self, name: &str) -> Result<&'a str, TemplateError> {
        let value = match name {
            "projectPath" => self.project_path,
            "nativeSessionId" => self.native_session_id,
            "profileId" => self.profile_id,
            _ => None,
        };
        value.ok_or_else(|| TemplateError::MissingValue {
            name: name.to_owned(),
        })
    }
}

/// Validate the static shape of a command template without expanding it.
///
/// This rejects shell interpolation, empty executables, and malformed or
/// unknown placeholders. Manifest validation runs this over every template
/// in the package.
pub fn validate_template(template: &CommandTemplate) -> Result<(), TemplateError> {
    if template.shell {
        return Err(TemplateError::ShellInterpolationForbidden);
    }
    if template.executable.trim().is_empty() {
        return Err(TemplateError::EmptyExecutable);
    }
    scan_placeholders(&template.executable)?;
    for arg in &template.args {
        scan_placeholders(arg)?;
    }
    Ok(())
}

/// Expand a command template into the exact executable string plus argv
/// vector passed to process creation. No shell quoting or command-line
/// string is produced.
pub fn expand_command(
    template: &CommandTemplate,
    context: &TemplateContext<'_>,
) -> Result<ExpandedCommand, TemplateError> {
    validate_template(template)?;
    Ok(ExpandedCommand {
        executable: expand_arg(&template.executable, context)?,
        args: template
            .args
            .iter()
            .map(|arg| expand_arg(arg, context))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

/// Expand a template into an argv vector (without the executable) using the
/// supplied context. The output is passed to direct process creation as-is.
pub fn expand_args(
    template: &CommandTemplate,
    context: &TemplateContext<'_>,
) -> Result<Vec<String>, TemplateError> {
    validate_template(template)?;
    template
        .args
        .iter()
        .map(|arg| expand_arg(arg, context))
        .collect()
}

/// Expand a single argument string.
pub fn expand_arg(arg: &str, context: &TemplateContext<'_>) -> Result<String, TemplateError> {
    scan_placeholders(arg)?;
    let mut output = String::with_capacity(arg.len());
    let mut rest = arg;
    while let Some(open) = rest.find('{') {
        output.push_str(&rest[..open]);
        let close = rest[open..]
            .find('}')
            .expect("scan_placeholders rejected unterminated placeholders");
        let name = &rest[open + 1..open + close];
        output.push_str(context.value(name)?);
        rest = &rest[open + close + 1..];
    }
    output.push_str(rest);
    Ok(output)
}

/// Check every `{...}` token in `arg` is a well-formed known placeholder.
fn scan_placeholders(arg: &str) -> Result<(), TemplateError> {
    let mut rest = arg;
    while let Some(open) = rest.find('{') {
        if rest[..open].contains('}') {
            return Err(TemplateError::UnexpectedClosingBrace {
                arg: arg.to_owned(),
            });
        }
        let after_open = &rest[open + 1..];
        let Some(close) = after_open.find('}') else {
            return Err(TemplateError::UnterminatedPlaceholder {
                arg: arg.to_owned(),
            });
        };
        let name = &after_open[..close];
        if name.is_empty() {
            return Err(TemplateError::EmptyPlaceholder {
                arg: arg.to_owned(),
            });
        }
        if !KNOWN_PLACEHOLDERS.contains(&name) {
            return Err(TemplateError::UnknownPlaceholder {
                name: name.to_owned(),
                arg: arg.to_owned(),
            });
        }
        rest = &after_open[close + 1..];
    }
    if rest.contains('}') {
        return Err(TemplateError::UnexpectedClosingBrace {
            arg: arg.to_owned(),
        });
    }
    Ok(())
}
