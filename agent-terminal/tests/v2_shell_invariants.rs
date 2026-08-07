//! V2 host/shell invariants exercised against shipped catalog + host code.

use agent_protocol::{
    AdapterId, ConversationId, HostInstanceId, HostRequest, NativeConversationKey,
    OrganizationState, PaneId, ProcessOwnerId, ProfileId, RuntimeState,
};
use agent_terminal::catalog::{AttachmentInput, CatalogStore, ConversationUpsert};
use agent_terminal::host::{HostController, OpenConversationDecision};
use agent_terminal::ui_bridge::{build_ui_snapshot, UiBridge, UiCommand};
use chrono::{TimeZone, Utc};
use lucidity_ui::{SettingsDraft, UsageView};
use std::collections::HashMap;
use tempfile::tempdir;
use uuid::Uuid;

fn at(second: i64) -> chrono::DateTime<Utc> {
    Utc.timestamp_opt(1_800_000_000 + second, 0).unwrap()
}

fn open_store() -> CatalogStore {
    let dir = tempdir().unwrap();
    CatalogStore::open_or_rebuild(&dir.path().join("catalog.sqlite3"))
        .unwrap()
        .store
}

#[test]
fn settle_does_not_stop_runtime_and_stop_does_not_settle() {
    let mut store = open_store();
    let host_id = HostInstanceId(Uuid::new_v4());
    let adapter = AdapterId::from("claude");
    let profile = ProfileId(Uuid::new_v4());
    store
        .ensure_profile(&adapter, profile, None, at(1))
        .unwrap();
    let record = store
        .upsert_conversation(ConversationUpsert {
            id: None,
            native: NativeConversationKey {
                adapter_id: adapter.clone(),
                profile_id: profile,
                native_session_id: "sess-1".into(),
            },
            native_session_path: None,
            title: "Work".into(),
            user_alias: None,
            project_path: Some(std::path::PathBuf::from(r"C:\proj")),
            created_at: at(1),
            last_activity_at: at(1),
            organization_state: OrganizationState::Active,
        })
        .unwrap();

    store
        .register_attachment(
            host_id,
            AttachmentInput {
                conversation_id: record.id,
                pane_id: PaneId(9),
                process_id: Some(42),
                process_owner: Some(ProcessOwnerId::DirectChild { pid: 42 }),
                attached_at: at(2),
            },
        )
        .unwrap();
    store
        .update_runtime_snapshot(agent_protocol::RuntimeSnapshot {
            conversation_id: record.id,
            state: RuntimeState::Working,
            source: agent_protocol::ObservationSource::StructuredEvent,
            confidence: agent_protocol::Confidence::Authoritative,
            observed_at: at(2),
            last_error: None,
        })
        .unwrap();

    let mut host = HostController::new(store, host_id).unwrap();
    host.handle_host_request(HostRequest::ConversationSetOrganizationState {
        conversation_id: record.id,
        organization_state: OrganizationState::Settled,
    })
    .unwrap();

    let settled = host.store().get_conversation(record.id).unwrap().unwrap();
    assert_eq!(settled.organization_state, OrganizationState::Settled);
    assert!(host.store().current_attachment(record.id).unwrap().is_some());
    assert_eq!(
        host.store()
            .runtime_snapshot(record.id)
            .unwrap()
            .unwrap()
            .state,
        RuntimeState::Working
    );

    host.handle_host_request(HostRequest::ConversationStop {
        conversation_id: record.id,
    })
    .unwrap();
    let after_stop = host.store().get_conversation(record.id).unwrap().unwrap();
    assert_eq!(after_stop.organization_state, OrganizationState::Settled);
    assert!(host
        .store()
        .current_attachment(record.id)
        .unwrap()
        .is_none());
}

#[test]
fn open_attached_conversation_focuses_without_second_launch_decision() {
    let mut store = open_store();
    let host_id = HostInstanceId(Uuid::new_v4());
    let adapter = AdapterId::from("codex");
    let profile = ProfileId(Uuid::new_v4());
    store
        .ensure_profile(&adapter, profile, None, at(1))
        .unwrap();
    let record = store
        .upsert_conversation(ConversationUpsert {
            id: None,
            native: NativeConversationKey {
                adapter_id: adapter,
                profile_id: profile,
                native_session_id: "codex-1".into(),
            },
            native_session_path: None,
            title: "Tests".into(),
            user_alias: None,
            project_path: None,
            created_at: at(1),
            last_activity_at: at(1),
            organization_state: OrganizationState::Active,
        })
        .unwrap();
    store
        .register_attachment(
            host_id,
            AttachmentInput {
                conversation_id: record.id,
                pane_id: PaneId(3),
                process_id: Some(7),
                process_owner: None,
                attached_at: at(2),
            },
        )
        .unwrap();

    let mut host = HostController::new(store, host_id).unwrap();
    host.handle_host_request(HostRequest::ConversationOpen {
        conversation_id: record.id,
    })
    .unwrap();
    let decisions = host.take_open_decisions();
    assert_eq!(decisions.len(), 1);
    assert_eq!(decisions[0].0, record.id);
    assert_eq!(decisions[0].1, OpenConversationDecision::FocusExisting);
}

#[test]
fn ui_snapshot_hides_mock_unless_demo() {
    let conversations = Vec::new();
    let runtime = HashMap::new();
    let attachments = HashMap::new();
    let adapters = vec![agent_protocol::AdapterManifest {
        schema_version: 1,
        id: AdapterId::from("mock-agent"),
        display_name: "Mock".into(),
        executable: agent_protocol::ExecutableDiscovery {
            names: vec!["lucidity-mock-agent".into()],
            explicit_paths: vec![],
            version_probe: None,
        },
        launch: agent_protocol::CommandTemplate {
            executable: "lucidity-mock-agent".into(),
            args: vec![],
            shell: false,
        },
        resume: None,
        history_sources: vec![],
        event_capabilities: vec![],
        settings_schema: None,
        settings_bindings: Default::default(),
        usage_provider: None,
        context_provider: None,
        fixtures: Default::default(),
        hooks: Default::default(),
        extensions: Default::default(),
    }];
    let normal = build_ui_snapshot(
        1,
        "0.1.0",
        false,
        &conversations,
        &runtime,
        &attachments,
        &adapters,
        None,
        UsageView::default(),
        SettingsDraft::default(),
        None,
        None,
    );
    assert!(normal.adapters.iter().all(|a| a.id != "mock-agent"));

    let demo = build_ui_snapshot(
        2,
        "0.1.0",
        true,
        &conversations,
        &runtime,
        &attachments,
        &adapters,
        None,
        UsageView::default(),
        SettingsDraft::default(),
        None,
        None,
    );
    assert!(demo.adapters.iter().any(|a| a.id == "mock-agent"));
}

#[test]
fn bridge_commands_include_new_open_settle_stop() {
    let bridge = UiBridge::new();
    let id = Uuid::new_v4();
    bridge.push_command_for_test(UiCommand::NewConversation {
        adapter_id: "claude".into(),
        project_path: r"C:\work".into(),
        title: Some("T".into()),
        extra_args: None,
    });
    bridge.push_command_for_test(UiCommand::OpenConversation(id));
    bridge.push_command_for_test(UiCommand::Settle(id));
    bridge.push_command_for_test(UiCommand::Stop(id));
    assert!(matches!(
        bridge.try_recv_command(),
        Some(UiCommand::NewConversation { .. })
    ));
    assert_eq!(
        bridge.try_recv_command(),
        Some(UiCommand::OpenConversation(id))
    );
    assert_eq!(bridge.try_recv_command(), Some(UiCommand::Settle(id)));
    assert_eq!(bridge.try_recv_command(), Some(UiCommand::Stop(id)));
}

#[test]
fn conversation_id_roundtrip_for_shell_rows() {
    let id = ConversationId(Uuid::new_v4());
    let parsed: ConversationId = id.to_string().parse().unwrap();
    assert_eq!(id, parsed);
}
