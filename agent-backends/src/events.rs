//! ADR-002 structured agent event transport codec.
//!
//! Wire shape (frozen):
//!
//! ```text
//! OSC 1337 ; SetUserVar=lucidity.agent-event.v1=<base64(UTF-8 JSON envelope)> ST
//! ```
//!
//! The encoder refuses envelopes whose serialized JSON exceeds
//! [`MAX_AGENT_EVENT_JSON_BYTES`] decoded bytes. The decoder checks the
//! reserved variable name, base64-decodes, enforces the same decoded size
//! bound *before* JSON parsing, validates UTF-8, rejects unknown envelope
//! versions, and surfaces malformed payloads as typed errors. Every failure
//! path is a `Result`; nothing here panics on untrusted terminal input, and
//! duplicate delivery is tolerated by the caller through the stable
//! [`evidence_id`].

use agent_protocol::{
    AdapterId, AgentEventEnvelope, EvidenceId, ProfileId, AGENT_EVENT_VERSION,
    MAX_AGENT_EVENT_JSON_BYTES, RESERVED_AGENT_EVENT_USER_VAR,
};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use std::fmt;

/// OSC introducer for the frozen transport.
pub const OSC_1337_SET_USER_VAR_PREFIX: &str = "\x1b]1337;SetUserVar=";
/// String terminator used by the encoder.
pub const OSC_ST: &str = "\x1b\\";
/// BEL is accepted as an alternate OSC terminator when decoding, matching
/// the terminal parser's behavior.
pub const OSC_BEL: char = '\x07';

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventEncodeError {
    /// The serialized envelope exceeds the decoded byte bound.
    Oversized { bytes: usize },
    /// Serialization failed (not expected for the frozen envelope type).
    Serialize(String),
}

impl fmt::Display for EventEncodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Oversized { bytes } => write!(
                formatter,
                "envelope is {bytes} bytes, exceeding the {MAX_AGENT_EVENT_JSON_BYTES}-byte bound"
            ),
            Self::Serialize(message) => {
                write!(formatter, "envelope serialization failed: {message}")
            }
        }
    }
}

impl std::error::Error for EventEncodeError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventDecodeError {
    /// The user variable name is not the reserved agent-event name. This is
    /// an ordinary user variable, not an agent event.
    WrongVariableName { name: String },
    /// The value is not valid base64.
    InvalidBase64,
    /// The decoded payload exceeds the byte bound; checked before JSON work.
    Oversized { bytes: usize },
    /// The decoded payload is not valid UTF-8.
    InvalidUtf8,
    /// The payload is not well-formed JSON or misses mandatory fields.
    MalformedJson { message: String },
    /// The envelope version is not supported (e.g. a future version).
    UnsupportedVersion { found: u32 },
}

impl fmt::Display for EventDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongVariableName { name } => {
                write!(formatter, "user variable {name:?} is not the reserved agent-event name")
            }
            Self::InvalidBase64 => write!(formatter, "value is not valid base64"),
            Self::Oversized { bytes } => write!(
                formatter,
                "decoded value is {bytes} bytes, exceeding the {MAX_AGENT_EVENT_JSON_BYTES}-byte bound"
            ),
            Self::InvalidUtf8 => write!(formatter, "decoded value is not valid UTF-8"),
            Self::MalformedJson { message } => {
                write!(formatter, "malformed event JSON: {message}")
            }
            Self::UnsupportedVersion { found } => write!(
                formatter,
                "unsupported event envelope version {found} (expected {AGENT_EVENT_VERSION})"
            ),
        }
    }
}

impl std::error::Error for EventDecodeError {}

/// Serialize an envelope and enforce the decoded byte bound.
pub fn encode_json(envelope: &AgentEventEnvelope) -> Result<String, EventEncodeError> {
    let json = serde_json::to_string(envelope)
        .map_err(|error| EventEncodeError::Serialize(error.to_string()))?;
    if json.len() > MAX_AGENT_EVENT_JSON_BYTES {
        return Err(EventEncodeError::Oversized { bytes: json.len() });
    }
    Ok(json)
}

/// Encode an envelope as the base64 user-variable value.
pub fn encode_user_var_value(envelope: &AgentEventEnvelope) -> Result<String, EventEncodeError> {
    Ok(BASE64.encode(encode_json(envelope)?))
}

/// Encode the complete OSC 1337 sequence (including the ST terminator)
/// ready to be written to the terminal output stream.
pub fn encode_osc_sequence(envelope: &AgentEventEnvelope) -> Result<String, EventEncodeError> {
    let value = encode_user_var_value(envelope)?;
    Ok(format!(
        "{OSC_1337_SET_USER_VAR_PREFIX}{RESERVED_AGENT_EVENT_USER_VAR}={value}{OSC_ST}"
    ))
}

/// Decode a `SetUserVar` name/value pair into an envelope. The reserved name
/// check happens first; the size bound is enforced on the decoded bytes
/// before any JSON parsing, per ADR-002.
pub fn decode_user_var(
    name: &str,
    value_base64: &str,
) -> Result<AgentEventEnvelope, EventDecodeError> {
    if name != RESERVED_AGENT_EVENT_USER_VAR {
        return Err(EventDecodeError::WrongVariableName {
            name: name.to_owned(),
        });
    }
    let bytes = BASE64
        .decode(value_base64)
        .map_err(|_| EventDecodeError::InvalidBase64)?;
    if bytes.len() > MAX_AGENT_EVENT_JSON_BYTES {
        return Err(EventDecodeError::Oversized { bytes: bytes.len() });
    }
    let json = String::from_utf8(bytes).map_err(|_| EventDecodeError::InvalidUtf8)?;
    decode_json(&json)
}

/// Decode an envelope from its JSON text, enforcing the version gate.
pub fn decode_json(json: &str) -> Result<AgentEventEnvelope, EventDecodeError> {
    let envelope: AgentEventEnvelope =
        serde_json::from_str(json).map_err(|error| EventDecodeError::MalformedJson {
            message: error.to_string(),
        })?;
    if envelope.v != AGENT_EVENT_VERSION {
        return Err(EventDecodeError::UnsupportedVersion { found: envelope.v });
    }
    Ok(envelope)
}

/// Parse a complete OSC sequence back into `(name, value)`. Accepts both ST
/// and BEL terminators. Returns `None` for sequences that are not the OSC
/// 1337 `SetUserVar` form.
pub fn parse_osc_sequence(sequence: &str) -> Option<(String, String)> {
    let body = sequence.strip_prefix(OSC_1337_SET_USER_VAR_PREFIX)?;
    let body = body
        .strip_suffix(OSC_ST)
        .or_else(|| body.strip_suffix(OSC_BEL))?;
    let (name, value) = body.split_once('=')?;
    Some((name.to_owned(), value.to_owned()))
}

/// Decode a complete OSC sequence into an envelope.
pub fn decode_osc_sequence(sequence: &str) -> Result<AgentEventEnvelope, EventDecodeError> {
    match parse_osc_sequence(sequence) {
        Some((name, value)) => decode_user_var(&name, &value),
        None => Err(EventDecodeError::WrongVariableName {
            name: sequence
                .strip_prefix(OSC_1337_SET_USER_VAR_PREFIX)
                .unwrap_or(sequence)
                .chars()
                .take(64)
                .collect(),
        }),
    }
}

/// The stable dedupe identity of a structured event:
/// `(adapter_id, profile_id, event_id)` formatted unambiguously.
pub fn evidence_id(envelope: &AgentEventEnvelope) -> EvidenceId {
    structured_evidence_id(&envelope.adapter, &envelope.profile_id, &envelope.event_id)
}

/// Stable evidence identity for structured events.
pub fn structured_evidence_id(
    adapter: &AdapterId,
    profile: &ProfileId,
    event_id: &str,
) -> EvidenceId {
    EvidenceId(format!("structured:{adapter}:{profile}:{event_id}"))
}

/// Stable evidence identity derived by a session-store scanner from the
/// adapter/profile, the source record key, and a content hash, per the
/// frozen identity rules.
pub fn store_evidence_id(
    adapter: &AdapterId,
    profile: &ProfileId,
    record_key: &str,
    content_hash: &str,
) -> EvidenceId {
    EvidenceId(format!(
        "store:{adapter}:{profile}:{record_key}:{content_hash}"
    ))
}
