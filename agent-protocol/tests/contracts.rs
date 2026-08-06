use agent_protocol::{
    AgentEventEnvelope, AgentEventKind, ExitResult, OrganizationState, ProcessExitObservation,
    RequestedTermination, RuntimeState, AGENT_EVENT_VERSION, MAX_AGENT_EVENT_JSON_BYTES,
    MAX_PENDING_AGENT_EVENTS, RESERVED_AGENT_EVENT_USER_VAR,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct StateAxes {
    organization_state: OrganizationState,
    runtime_state: RuntimeState,
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
