use agent_backends::discovery::{ExecutableLocator, SearchPaths};
use agent_backends::events::{
    decode_json, decode_osc_sequence, decode_user_var, encode_json, encode_osc_sequence,
    encode_user_var_value, parse_osc_sequence, EventDecodeError, EventEncodeError,
    OSC_1337_SET_USER_VAR_PREFIX,
};
use agent_backends::manifest::{load_manifest, load_package, parse_manifest, ManifestError};
use agent_backends::template::{expand_command, validate_template, TemplateContext, TemplateError};
use agent_protocol::{
    AdapterId, AgentEventEnvelope, AgentEventKind, CommandTemplate, ExecutableDiscovery,
    AGENT_EVENT_VERSION, MAX_AGENT_EVENT_JSON_BYTES, RESERVED_AGENT_EVENT_USER_VAR,
};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde_json::json;
use std::fs;

fn minimal_manifest(overrides: &str) -> String {
    format!(
        r#"
schemaVersion = 1
id = "mock-agent"
displayName = "Mock Agent"
{overrides}

[executable]
names = []
explicitPaths = []

[launch]
executable = "mock-agent"
args = []
shell = false
"#
    )
}

fn fixture_event(name: &str) -> AgentEventEnvelope {
    let json = match name {
        "session_start" => include_str!("../fixtures/events/session_start.json"),
        "usage_updated" => include_str!("../fixtures/events/usage_updated.json"),
        "unknown_kind" => include_str!("../fixtures/events/unknown_kind.json"),
        "future_version" => include_str!("../fixtures/events/future_version.json"),
        "missing_event_id" => include_str!("../fixtures/events/missing_event_id.json"),
        other => panic!("unknown fixture {other}"),
    };
    serde_json::from_str(json).expect("fixture parses")
}

#[test]
fn packaged_mock_adapter_loads_and_validates() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("adapters/mock-agent.agent-adapter");
    let manifest = load_package(&root).expect("mock adapter package validates");
    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.id, AdapterId::from("mock-agent"));
    assert!(manifest
        .event_capabilities
        .contains(&AgentEventKind::UsageUpdated));
    assert_eq!(
        manifest
            .fixtures
            .get("eventStream")
            .expect("event stream fixture")
            .path,
        std::path::PathBuf::from("fixtures/events.jsonl")
    );

    let settings_schema = fs::read_to_string(root.join("settings.schema.json")).unwrap();
    let settings_schema: serde_json::Value = serde_json::from_str(&settings_schema).unwrap();
    assert_eq!(settings_schema["additionalProperties"], false);
}

#[test]
fn manifest_parser_rejects_schema_typos_and_semantic_negatives() {
    let typo = minimal_manifest("usageProvder = { kind = \"command\" }");
    assert!(matches!(
        parse_manifest(&typo),
        Err(ManifestError::UnknownField { field }) if field == "usageProvder"
    ));

    let nested_typo = minimal_manifest(
        r#"
[usageProvider]
kind = "command"
unexpected = true
"#,
    );
    assert!(matches!(
        parse_manifest(&nested_typo),
        Err(ManifestError::UnknownField { field }) if field == "usageProvider.unexpected"
    ));

    let shell = minimal_manifest("");
    let shell = shell.replace("shell = false", "shell = true");
    let errors = load_manifest(&shell).unwrap_err();
    assert!(errors.iter().any(|error| {
        matches!(
            error,
            ManifestError::InvalidTemplate {
                error: TemplateError::ShellInterpolationForbidden,
                ..
            }
        )
    }));

    let unknown_event = minimal_manifest("eventCapabilities = [\"future_metric\"]");
    let errors = load_manifest(&unknown_event).unwrap_err();
    assert!(errors.iter().any(|error| matches!(
        error,
        ManifestError::UnknownEventCapability { name } if name == "future_metric"
    )));

    let escaping_path = minimal_manifest("settingsSchema = \"../settings.schema.json\"");
    let errors = load_manifest(&escaping_path).unwrap_err();
    assert!(errors.iter().any(|error| matches!(
        error,
        ManifestError::PathEscapesPackage { field, .. } if field == "settingsSchema"
    )));

    let missing_resume_placeholder = format!(
        r#"{}

[resume]
executable = "mock-agent"
args = ["resume"]
shell = false
"#,
        minimal_manifest("")
    );
    let errors = load_manifest(&missing_resume_placeholder).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| matches!(error, ManifestError::ResumeMissingSessionPlaceholder)));
}

#[test]
fn template_expansion_returns_direct_argv_and_preserves_hostile_text() {
    let hostile = r#"C:\repo & whoami ; $(rm -rf .) "quoted" %PATH%"#;
    let template = CommandTemplate {
        executable: "lucidity-mock-agent".to_owned(),
        args: vec![
            "--cwd".to_owned(),
            "{projectPath}".to_owned(),
            "--session".to_owned(),
            "prefix-{nativeSessionId}-suffix".to_owned(),
            hostile.to_owned(),
        ],
        shell: false,
    };
    let expanded = expand_command(
        &template,
        &TemplateContext {
            project_path: Some(hostile),
            native_session_id: Some("native-42"),
            profile_id: None,
        },
    )
    .unwrap();

    assert_eq!(expanded.executable, "lucidity-mock-agent");
    assert_eq!(expanded.args[1], hostile);
    assert_eq!(expanded.args[3], "prefix-native-42-suffix");
    assert_eq!(expanded.args[4], hostile);
    assert!(!expanded.args.join("\0").contains("cmd.exe"));
}

#[test]
fn template_validation_rejects_malformed_or_missing_placeholders() {
    let shell = CommandTemplate {
        executable: "agent".to_owned(),
        args: vec![],
        shell: true,
    };
    assert_eq!(
        validate_template(&shell),
        Err(TemplateError::ShellInterpolationForbidden)
    );

    let unexpected_close = CommandTemplate {
        executable: "agent".to_owned(),
        args: vec!["bad}".to_owned()],
        shell: false,
    };
    assert!(matches!(
        validate_template(&unexpected_close),
        Err(TemplateError::UnexpectedClosingBrace { .. })
    ));

    let unknown = CommandTemplate {
        executable: "agent".to_owned(),
        args: vec!["{unknown}".to_owned()],
        shell: false,
    };
    assert!(matches!(
        validate_template(&unknown),
        Err(TemplateError::UnknownPlaceholder { name, .. }) if name == "unknown"
    ));

    let missing = CommandTemplate {
        executable: "agent".to_owned(),
        args: vec!["{profileId}".to_owned()],
        shell: false,
    };
    assert!(matches!(
        expand_command(&missing, &TemplateContext::default()),
        Err(TemplateError::MissingValue { name }) if name == "profileId"
    ));
}

#[test]
fn deterministic_search_paths_resolve_explicit_paths_before_names() {
    let temp = tempfile::tempdir().unwrap();
    let bin_dir = temp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let explicit = bin_dir.join("explicit-agent");
    let named = bin_dir.join("named-agent");
    fs::write(&explicit, b"").unwrap();
    fs::write(&named, b"").unwrap();

    let discovery = ExecutableDiscovery {
        names: vec!["named-agent".to_owned()],
        explicit_paths: vec![std::path::PathBuf::from("explicit-agent")],
        version_probe: None,
    };
    let locator = SearchPaths::new([bin_dir]);
    assert_eq!(locator.locate(&discovery).unwrap(), explicit);
}

#[test]
fn version_parser_extracts_first_semver_token() {
    assert_eq!(
        agent_backends::discovery::parse_version("tool build v0.29.1 (abc)"),
        Some("0.29.1".to_owned())
    );
    assert_eq!(
        agent_backends::discovery::parse_version("no numeric version here"),
        None
    );
}

#[test]
fn event_codec_round_trips_reserved_osc_set_user_var() {
    let event = fixture_event("session_start");
    let json = encode_json(&event).unwrap();
    assert!(json.len() <= MAX_AGENT_EVENT_JSON_BYTES);

    let sequence = encode_osc_sequence(&event).unwrap();
    let (name, value) = parse_osc_sequence(&sequence).unwrap();
    assert_eq!(name, RESERVED_AGENT_EVENT_USER_VAR);
    assert_eq!(decode_user_var(&name, &value).unwrap(), event);

    let bel_sequence =
        format!("{OSC_1337_SET_USER_VAR_PREFIX}{RESERVED_AGENT_EVENT_USER_VAR}={value}\x07");
    assert_eq!(decode_osc_sequence(&bel_sequence).unwrap(), event);
}

#[test]
fn event_decoder_rejects_reserved_name_size_utf8_malformed_and_future_version() {
    let event = fixture_event("session_start");
    let value = encode_user_var_value(&event).unwrap();
    assert!(matches!(
        decode_user_var("lucidity.agent-event.v2", &value),
        Err(EventDecodeError::WrongVariableName { name }) if name == "lucidity.agent-event.v2"
    ));

    let invalid_utf8 = include_str!("../fixtures/events/invalid_utf8.b64").trim();
    assert_eq!(
        decode_user_var(RESERVED_AGENT_EVENT_USER_VAR, invalid_utf8),
        Err(EventDecodeError::InvalidUtf8)
    );

    let malformed = include_str!("../fixtures/events/malformed_truncated.json");
    assert!(matches!(
        decode_json(malformed),
        Err(EventDecodeError::MalformedJson { .. })
    ));
    let missing = include_str!("../fixtures/events/missing_event_id.json");
    assert!(matches!(
        decode_json(missing),
        Err(EventDecodeError::MalformedJson { .. })
    ));
    let future = include_str!("../fixtures/events/future_version.json");
    assert!(matches!(
        decode_json(future),
        Err(EventDecodeError::UnsupportedVersion { found: 99 })
    ));

    let oversized_fixture: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/events/oversized.json")).unwrap();
    assert_eq!(oversized_fixture["decodedByteLength"], 32769);
    let oversized = BASE64.encode("x".repeat(MAX_AGENT_EVENT_JSON_BYTES + 1));
    assert!(matches!(
        decode_user_var(RESERVED_AGENT_EVENT_USER_VAR, &oversized),
        Err(EventDecodeError::Oversized { bytes }) if bytes == MAX_AGENT_EVENT_JSON_BYTES + 1
    ));
}

#[test]
fn event_codec_accepts_exact_max_decoded_boundary_and_rejects_max_plus_one() {
    let mut envelope = fixture_event("session_start");
    envelope.event_id = "evt-boundary".to_owned();
    envelope.data = json!({ "pad": "" });
    let empty_len = serde_json::to_string(&envelope).unwrap().len();
    let pad_len = MAX_AGENT_EVENT_JSON_BYTES - empty_len;

    envelope.data = json!({ "pad": "x".repeat(pad_len) });
    assert_eq!(
        serde_json::to_string(&envelope).unwrap().len(),
        MAX_AGENT_EVENT_JSON_BYTES
    );
    assert!(encode_user_var_value(&envelope).is_ok());

    envelope.data = json!({ "pad": "x".repeat(pad_len + 1) });
    assert!(matches!(
        encode_user_var_value(&envelope),
        Err(EventEncodeError::Oversized { bytes }) if bytes == MAX_AGENT_EVENT_JSON_BYTES + 1
    ));
}

#[test]
fn unknown_event_kinds_decode_for_reducer_quarantine() {
    let event = fixture_event("unknown_kind");
    assert_eq!(
        event.event,
        AgentEventKind::Unknown("future_metric".to_owned())
    );
    assert_eq!(event.v, AGENT_EVENT_VERSION);
}

#[test]
fn reordered_duplicate_event_fixture_decodes_without_order_assumptions() {
    let events = include_str!("../fixtures/events/reordered_duplicate.jsonl")
        .lines()
        .map(|line| decode_json(line).unwrap())
        .collect::<Vec<_>>();

    assert_eq!(events.len(), 4);
    assert_eq!(events[0].event_id, "evt-0002");
    assert_eq!(events[1].event_id, "evt-0001");
    assert_eq!(events[2].event_id, events[3].event_id);
}
