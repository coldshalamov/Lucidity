use agent_backends::reducer::{runtime_name, DetachReason, Evidence, Reducer};
use agent_protocol::{
    AdapterId, AgentEventEnvelope, EvidenceId, HostInstanceId, PaneId, ProfileId, RuntimeState,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::BTreeMap;

const FIXTURES: &[(&str, &str)] = &[
    (
        "attach_and_duplicate",
        include_str!("../fixtures/reducer/attach_and_duplicate.json"),
    ),
    (
        "confirm_without_hint",
        include_str!("../fixtures/reducer/confirm_without_hint.json"),
    ),
    (
        "cross_source_dedupe",
        include_str!("../fixtures/reducer/cross_source_dedupe.json"),
    ),
    (
        "event_ordering",
        include_str!("../fixtures/reducer/event_ordering.json"),
    ),
    (
        "hint_crash",
        include_str!("../fixtures/reducer/hint_crash.json"),
    ),
    (
        "hint_expiry",
        include_str!("../fixtures/reducer/hint_expiry.json"),
    ),
    (
        "hint_supersede_confirm",
        include_str!("../fixtures/reducer/hint_supersede_confirm.json"),
    ),
    (
        "stale_host_and_scope",
        include_str!("../fixtures/reducer/stale_host_and_scope.json"),
    ),
    (
        "store_confirm_rebind_late_old_end",
        include_str!("../fixtures/reducer/store_confirm_rebind_late_old_end.json"),
    ),
    (
        "superseded_candidate_rejected",
        include_str!("../fixtures/reducer/superseded_candidate_rejected.json"),
    ),
    (
        "unknown_event_quarantine",
        include_str!("../fixtures/reducer/unknown_event_quarantine.json"),
    ),
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Scenario {
    scenario: String,
    adapter: String,
    profile_id: ProfileId,
    host_instance_id: HostInstanceId,
    steps: Vec<Step>,
    #[serde(rename = "final")]
    final_state: FinalState,
}

#[derive(Debug, Deserialize)]
struct Step {
    apply: ApplyFixture,
    expect: String,
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum ApplyFixture {
    Structured {
        pane: u64,
        host: HostInstanceId,
        envelope: AgentEventEnvelope,
    },
    Submitted {
        pane: u64,
        host: HostInstanceId,
        candidate: String,
        evidence_id: EvidenceId,
        observed_at: DateTime<Utc>,
    },
    Store {
        pane: u64,
        host: HostInstanceId,
        session: String,
        evidence_id: EvidenceId,
        observed_at: DateTime<Utc>,
    },
    Exit {
        pane: u64,
        host: HostInstanceId,
        success: bool,
        observed_at: DateTime<Utc>,
    },
    Tick {
        now: DateTime<Utc>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FinalState {
    conversations: BTreeMap<String, ExpectedConversation>,
    panes: BTreeMap<String, ExpectedPane>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExpectedConversation {
    runtime: String,
    ended: bool,
    detach_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExpectedPane {
    bound: Option<String>,
    generation: u64,
    pending: bool,
}

#[test]
fn reducer_json_fixtures_match_expected_outcomes_and_final_state() {
    for (name, json) in FIXTURES {
        let scenario: Scenario = serde_json::from_str(json).unwrap_or_else(|error| {
            panic!("fixture {name} failed to parse: {error}");
        });
        let mut reducer = Reducer::new(
            AdapterId::from(scenario.adapter.as_str()),
            scenario.profile_id,
            scenario.host_instance_id,
        );

        for (index, step) in scenario.steps.into_iter().enumerate() {
            let outcome = reducer.apply(step.apply.into_evidence());
            assert_eq!(
                outcome.label(),
                step.expect,
                "fixture {name} step {index}: {}",
                scenario.scenario
            );
        }

        assert_final_state(name, &reducer, &scenario.final_state);
    }
}

impl ApplyFixture {
    fn into_evidence(self) -> Evidence {
        match self {
            Self::Structured {
                pane,
                host,
                envelope,
            } => Evidence::Structured {
                host,
                pane: PaneId(pane),
                envelope: Box::new(envelope),
            },
            Self::Submitted {
                pane,
                host,
                candidate,
                evidence_id,
                observed_at,
            } => Evidence::SubmittedCandidate {
                host,
                pane: PaneId(pane),
                candidate_native_session_id: candidate,
                evidence_id,
                observed_at,
            },
            Self::Store {
                pane,
                host,
                session,
                evidence_id,
                observed_at,
            } => Evidence::StoreChange {
                host,
                pane: PaneId(pane),
                native_session_id: session,
                evidence_id,
                observed_at,
            },
            Self::Exit {
                pane,
                host,
                success,
                observed_at,
            } => Evidence::ProcessExit {
                host,
                pane: PaneId(pane),
                success,
                observed_at,
            },
            Self::Tick { now } => Evidence::Tick { now },
        }
    }
}

fn assert_final_state(name: &str, reducer: &Reducer, expected: &FinalState) {
    assert_eq!(
        reducer.conversations().len(),
        expected.conversations.len(),
        "fixture {name} conversation count"
    );
    for (session, expected) in &expected.conversations {
        let actual = reducer
            .conversation(session)
            .unwrap_or_else(|| panic!("fixture {name} missing conversation {session}"));
        assert_eq!(
            runtime_name(actual.runtime),
            expected.runtime,
            "fixture {name} runtime for {session}"
        );
        assert_eq!(
            actual.ended, expected.ended,
            "fixture {name} ended for {session}"
        );
        assert_eq!(
            detach_reason_name(actual.detach_reason),
            expected.detach_reason.as_deref(),
            "fixture {name} detach reason for {session}"
        );
    }

    assert_eq!(
        reducer.panes().len(),
        expected.panes.len(),
        "fixture {name} pane count"
    );
    for (pane, expected) in &expected.panes {
        let pane_id = PaneId(pane.parse().expect("pane key is numeric"));
        let actual = reducer
            .pane(pane_id)
            .unwrap_or_else(|| panic!("fixture {name} missing pane {pane}"));
        assert_eq!(actual.bound, expected.bound, "fixture {name} pane {pane}");
        assert_eq!(
            actual.generation, expected.generation,
            "fixture {name} pane {pane} generation"
        );
        assert_eq!(
            actual.pending.is_some(),
            expected.pending,
            "fixture {name} pane {pane} pending"
        );
    }
}

fn detach_reason_name(reason: Option<DetachReason>) -> Option<&'static str> {
    reason.map(|reason| reason.as_str())
}

#[test]
fn two_durable_identities_share_one_pane_after_new_rebind() {
    let host: HostInstanceId = "33333333-3333-4333-8333-333333333333".parse().unwrap();
    let profile: ProfileId = "11111111-1111-4111-8111-111111111111".parse().unwrap();
    let pane = PaneId(7);
    let mut reducer = Reducer::new(AdapterId::from("mock-agent"), profile, host);

    assert_eq!(
        reducer
            .apply(Evidence::Structured {
                host,
                pane,
                envelope: Box::new(event("evt-0001", "mock-1-0", "session_start", 1)),
            })
            .label(),
        "attach:mock-1-0:g1"
    );
    assert_eq!(
        reducer
            .apply(Evidence::Structured {
                host,
                pane,
                envelope: Box::new(event("evt-0002", "mock-1-1", "session_start", 2)),
            })
            .label(),
        "rebind:mock-1-0->mock-1-1:g2"
    );

    assert_eq!(
        reducer.conversation("mock-1-0").unwrap().runtime,
        RuntimeState::NotRunning
    );
    assert_eq!(
        reducer.conversation("mock-1-1").unwrap().runtime,
        RuntimeState::Starting
    );
    let binding = reducer.pane(pane).unwrap();
    assert_eq!(binding.bound.as_deref(), Some("mock-1-1"));
    assert_eq!(binding.generation, 2);
}

fn event(event_id: &str, session_id: &str, event: &str, seq: u64) -> AgentEventEnvelope {
    serde_json::from_value(serde_json::json!({
        "v": 1,
        "eventId": event_id,
        "adapter": "mock-agent",
        "profileId": "11111111-1111-4111-8111-111111111111",
        "event": event,
        "sessionId": session_id,
        "cwd": "C:\\work\\mock",
        "transcriptPath": null,
        "occurredAt": "2026-08-06T00:00:01Z",
        "data": { "seq": seq }
    }))
    .unwrap()
}
