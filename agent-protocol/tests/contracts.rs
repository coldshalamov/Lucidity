use agent_protocol::{
    AdapterBinding, AdapterManifest, AgentEventEnvelope, AgentEventKind, ExecutableHookManifest,
    ExitResult, FixtureManifest, HostEvent, HostRequest, HostResult, OrganizationState,
    ProcessExitObservation, ProcessOwnerId, RequestedTermination, RuntimeState,
    AGENT_EVENT_VERSION, MAX_AGENT_EVENT_JSON_BYTES, MAX_PENDING_AGENT_EVENTS,
    RESERVED_AGENT_EVENT_USER_VAR,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct StateAxes {
    organization_state: OrganizationState,
    runtime_state: RuntimeState,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EnumStructVariantFixture {
    process_owners: Vec<ProcessOwnerId>,
    exit_results: Vec<ExitResult>,
    host_requests: Vec<HostRequest>,
    host_results: Vec<HostResult>,
    host_events: Vec<HostEvent>,
}

#[test]
fn agent_event_v1_fixture_is_serialization_stable() {
    let fixture = include_str!("fixtures/agent_event_v1.json");
    let event: AgentEventEnvelope = serde_json::from_str(fixture).unwrap();

    assert_eq!(event.v, AGENT_EVENT_VERSION);
    assert_eq!(event.event, AgentEventKind::SessionStart);
    assert_eq!(RESERVED_AGENT_EVENT_USER_VAR, "lucidity.agent-event.v1");
    assert_eq!(MAX_AGENT_EVENT_JSON_BYTES, 32_768);
    assert_eq!(MAX_PENDING_AGENT_EVENTS, 256);

    let expected: serde_json::Value = serde_json::from_str(fixture).unwrap();
    assert_eq!(serde_json::to_value(event).unwrap(), expected);
}

#[test]
fn organization_and_runtime_are_orthogonal_axes() {
    let fixture = include_str!("fixtures/state_orthogonality.json");
    let states: Vec<StateAxes> = serde_json::from_str(fixture).unwrap();
    let unique = states
        .iter()
        .map(|state| {
            (
                format!("{:?}", state.organization_state),
                format!("{:?}", state.runtime_state),
            )
        })
        .collect::<BTreeSet<_>>();

    assert_eq!(states.len(), 2 * 8);
    assert_eq!(unique.len(), states.len());

    for organization_state in [OrganizationState::Active, OrganizationState::Settled] {
        for runtime_state in [
            RuntimeState::NotRunning,
            RuntimeState::Starting,
            RuntimeState::Working,
            RuntimeState::WaitingForInput,
            RuntimeState::AwaitingApproval,
            RuntimeState::CompletedIdle,
            RuntimeState::Failed,
            RuntimeState::UnknownExternal,
        ] {
            assert!(states.contains(&StateAxes {
                organization_state,
                runtime_state,
            }));
        }
    }
}

#[test]
fn unknown_event_kinds_round_trip_for_quarantine() {
    let fixture =
        include_str!("fixtures/agent_event_v1.json").replace("session_start", "future_event");
    let event: AgentEventEnvelope = serde_json::from_str(&fixture).unwrap();

    assert_eq!(
        event.event,
        AgentEventKind::Unknown("future_event".to_owned())
    );
    assert_eq!(
        serde_json::to_value(event).unwrap()["event"],
        "future_event"
    );
}

#[test]
fn typed_exit_fixture_preserves_unsigned_windows_exit_codes_and_kill_owner() {
    let fixture = include_str!("fixtures/process_exit.json");
    let observation: ProcessExitObservation = serde_json::from_str(fixture).unwrap();

    assert_eq!(
        observation.result,
        ExitResult::Exited {
            code: Some(u32::MAX),
            success: false,
        }
    );
    assert_eq!(
        observation.requested_termination,
        RequestedTermination::WezTermKill
    );
    let expected: serde_json::Value = serde_json::from_str(fixture).unwrap();
    assert_eq!(serde_json::to_value(observation).unwrap(), expected);
}

#[test]
fn tagged_enum_struct_variants_round_trip_the_camel_case_golden_fixture() {
    let fixture = include_str!("fixtures/enum_struct_variants.json");
    let values: EnumStructVariantFixture = serde_json::from_str(fixture).unwrap();

    assert_eq!(values.process_owners.len(), 2);
    assert_eq!(values.exit_results.len(), 2);
    assert_eq!(values.host_requests.len(), 14);
    assert_eq!(values.host_results.len(), 6);
    assert_eq!(values.host_events.len(), 6);

    let expected: serde_json::Value = serde_json::from_str(fixture).unwrap();
    assert_eq!(serde_json::to_value(values).unwrap(), expected);
    assert_eq!(expected["processOwners"][0]["opaqueId"], "job-fixture");
    assert_eq!(
        expected["hostRequests"][6]["params"]["projectPath"],
        "projects/lucidity"
    );
    assert_eq!(expected["hostResults"][5]["data"]["shuttingDown"], false);
    assert_eq!(
        expected["hostEvents"][0]["data"]["catalogSnapshotVersion"],
        10
    );
}

#[test]
fn full_adapter_manifest_fixture_round_trips_every_typed_surface() {
    let fixture = include_str!("fixtures/adapter_manifest_full.json");
    let manifest: AdapterManifest = serde_json::from_str(fixture).unwrap();

    assert!(manifest.settings_bindings.contains_key("approvalMode"));
    assert_eq!(manifest.usage_provider.as_ref().unwrap().kind, "command");
    assert_eq!(
        manifest.context_provider.as_ref().unwrap().kind,
        "session_store"
    );
    assert!(manifest.fixtures.contains_key("eventStream"));
    assert!(manifest.hooks.contains_key("installEvents"));
    assert_eq!(
        manifest.extensions["futureContract"]["nested"]["revision"],
        2
    );

    let expected: serde_json::Value = serde_json::from_str(fixture).unwrap();
    assert_eq!(serde_json::to_value(manifest).unwrap(), expected);
}

#[test]
fn minimal_adapter_manifest_fixture_is_the_canonical_defaulted_shape() {
    let fixture = include_str!("fixtures/adapter_manifest_minimal.json");
    let canonical: serde_json::Value = serde_json::from_str(fixture).unwrap();
    let expected: AdapterManifest = serde_json::from_value(canonical.clone()).unwrap();
    let mut omitted = canonical.clone();
    let object = omitted.as_object_mut().unwrap();
    for field in [
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
    ] {
        object.remove(field);
    }
    let executable = object["executable"].as_object_mut().unwrap();
    executable.remove("names");
    executable.remove("explicitPaths");
    executable.remove("versionProbe");
    let launch = object["launch"].as_object_mut().unwrap();
    launch.remove("args");
    launch.remove("shell");

    let defaulted: AdapterManifest = serde_json::from_value(omitted).unwrap();
    assert_eq!(defaulted, expected);
    assert_eq!(serde_json::to_value(defaulted).unwrap(), canonical);
}

#[test]
fn adapter_manifest_rejects_top_level_typos_and_nested_unknown_fields() {
    let mut typo: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/adapter_manifest_full.json")).unwrap();
    let object = typo.as_object_mut().unwrap();
    let usage_provider = object.remove("usageProvider").unwrap();
    object.insert("usageProvder".to_owned(), usage_provider);
    let error = serde_json::from_value::<AdapterManifest>(typo)
        .unwrap_err()
        .to_string();
    assert!(error.contains("usageProvder"), "unexpected error: {error}");

    assert!(serde_json::from_str::<AdapterBinding>(
        r#"{"kind":"command","config":{},"unexpected":true}"#
    )
    .is_err());
    assert!(serde_json::from_str::<FixtureManifest>(
        r#"{"kind":"jsonl","path":"fixture.jsonl","unexpected":true}"#
    )
    .is_err());
    assert!(serde_json::from_str::<ExecutableHookManifest>(
        r#"{
            "description":"fixture",
            "apply":{"executable":"agent","args":[],"shell":false},
            "rollback":{"executable":"agent","args":[],"shell":false},
            "destinations":[],
            "unexpected":true
        }"#
    )
    .is_err());
}
