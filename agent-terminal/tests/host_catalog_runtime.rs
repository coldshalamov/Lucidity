use agent_protocol::{
    AdapterId, AgentEventKind, HostInstanceId, HostRequest, HostResult, IpcRequest,
    IpcResponsePayload, NativeConversationKey, OrganizationState, PaneId, ProcessOwnerId,
    RuntimeState,
};
use agent_terminal::catalog::{
    AttachmentInput, CatalogStore, ConversationUpsert, CURRENT_SCHEMA_VERSION,
};
use agent_terminal::host::{HostController, OpenConversationDecision, TrayAction, TrayVisibility};
use agent_terminal::runtime::{
    pending_identity_from_submitted_hint, reduce_queued_event, AgentEventIngress, EventReduction,
    IngressOutcome, QueuedAgentEvent,
};
use chrono::{Duration, TimeZone, Utc};
use serde_json::json;
use std::time::Instant;
use uuid::Uuid;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn host(n: u128) -> HostInstanceId {
    HostInstanceId(id(n))
}

fn profile(n: u128) -> agent_protocol::ProfileId {
    agent_protocol::ProfileId(id(n))
}

fn conversation(n: u128) -> agent_protocol::ConversationId {
    agent_protocol::ConversationId(id(n))
}

fn at(second: i64) -> chrono::DateTime<Utc> {
    Utc.timestamp_opt(1_800_000_000 + second, 0).unwrap()
}

fn native(session: &str) -> NativeConversationKey {
    NativeConversationKey {
        adapter_id: AdapterId("claude-code".to_owned()),
        profile_id: profile(10),
        native_session_id: session.to_owned(),
    }
}

fn upsert(
    store: &mut CatalogStore,
    cid: agent_protocol::ConversationId,
    session: &str,
    offset: i64,
) -> agent_protocol::ConversationRecord {
    let now = at(offset);
    store
        .upsert_conversation(ConversationUpsert {
            id: Some(cid),
            native: native(session),
            native_session_path: None,
            title: format!("conversation {session}"),
            user_alias: None,
            project_path: Some(format!(r"C:\work\{session}").into()),
            created_at: now,
            last_activity_at: now,
            organization_state: OrganizationState::Active,
        })
        .unwrap()
}

fn attach(
    store: &mut CatalogStore,
    hid: HostInstanceId,
    cid: agent_protocol::ConversationId,
    pane: u64,
) -> agent_protocol::RuntimeAttachment {
    store
        .register_attachment(
            hid,
            AttachmentInput {
                conversation_id: cid,
                pane_id: PaneId(pane),
                process_id: Some(4_000 + pane as u32),
                process_owner: Some(ProcessOwnerId::WindowsJob {
                    opaque_id: format!("job-{pane}"),
                }),
                attached_at: at(100 + pane as i64),
            },
        )
        .unwrap()
}

fn queued_event(
    hid: HostInstanceId,
    pane: u64,
    event_id: &str,
    event: AgentEventKind,
    session: &str,
    offset: i64,
    data: serde_json::Value,
) -> QueuedAgentEvent {
    let occurred_at = at(offset);
    let json = json!({
        "v": 1,
        "eventId": event_id,
        "adapter": "claude-code",
        "profileId": profile(10),
        "event": event.as_str(),
        "sessionId": session,
        "cwd": r"C:\work\repo",
        "transcriptPath": null,
        "occurredAt": occurred_at,
        "data": data
    })
    .to_string();
    QueuedAgentEvent {
        host_instance_id: hid,
        pane_id: PaneId(pane),
        json,
        enqueued_at: occurred_at,
    }
}

#[test]
fn migrations_upsert_dedupe_and_snapshot_keyset_paging_are_deterministic() {
    let mut store = CatalogStore::in_memory().unwrap();
    let migrations = store.applied_migrations().unwrap();
    assert_eq!(migrations.len(), CURRENT_SCHEMA_VERSION as usize);
    assert_eq!(migrations[0].0, 1);
    assert_eq!(migrations[1].0, 2);
    assert_eq!(migrations[2].0, 3);

    let a = upsert(&mut store, conversation(1), "a", 1);
    let b = upsert(&mut store, conversation(2), "b", 2);
    let c = upsert(&mut store, conversation(3), "c", 3);

    let duplicate = store
        .upsert_conversation(ConversationUpsert {
            id: Some(conversation(99)),
            native: native("b"),
            native_session_path: None,
            title: "conversation b updated".to_owned(),
            user_alias: Some("work-b".to_owned()),
            project_path: None,
            created_at: at(0),
            last_activity_at: at(4),
            organization_state: OrganizationState::Settled,
        })
        .unwrap();
    assert_eq!(duplicate.id, b.id);
    assert_eq!(duplicate.user_alias.as_deref(), Some("work-b"));
    assert_eq!(duplicate.organization_state, OrganizationState::Settled);

    let first = store.list_conversations(None, 2).unwrap();
    assert!(!first.resync_required);
    assert_eq!(
        first.conversations.iter().map(|c| c.id).collect::<Vec<_>>(),
        vec![b.id, c.id]
    );
    let cursor = first.next_cursor.clone().unwrap();

    let second = store.list_conversations(Some(cursor.clone()), 2).unwrap();
    assert_eq!(
        second
            .conversations
            .iter()
            .map(|c| c.id)
            .collect::<Vec<_>>(),
        vec![a.id]
    );

    upsert(&mut store, conversation(4), "d", 5);
    let changed_snapshot = store.list_conversations(Some(cursor), 2).unwrap();
    assert!(changed_snapshot.resync_required);
    assert!(changed_snapshot.conversations.is_empty());
}

#[test]
fn organization_and_runtime_reducers_are_orthogonal() {
    let mut store = CatalogStore::in_memory().unwrap();
    let record = upsert(&mut store, conversation(10), "runtime", 10);
    let hid = host(1);
    let attachment = attach(&mut store, hid, record.id, 7);

    store
        .set_organization_state(record.id, OrganizationState::Settled, at(20))
        .unwrap();
    let after_settle = store.get_conversation(record.id).unwrap().unwrap();
    assert_eq!(after_settle.organization_state, OrganizationState::Settled);
    assert_eq!(
        store.current_attachment(record.id).unwrap().unwrap(),
        attachment
    );
    assert_eq!(
        store.runtime_snapshot(record.id).unwrap().unwrap().state,
        RuntimeState::Starting
    );

    let stopped = store.stop_conversation(record.id, at(30)).unwrap();
    assert!(stopped.is_some());
    assert_eq!(
        store
            .get_conversation(record.id)
            .unwrap()
            .unwrap()
            .organization_state,
        OrganizationState::Settled
    );
    assert!(store.current_attachment(record.id).unwrap().is_none());
    assert_eq!(
        store.runtime_snapshot(record.id).unwrap().unwrap().state,
        RuntimeState::NotRunning
    );
}

#[test]
fn startup_reconciliation_and_generation_checks_reject_stale_versions() {
    let mut store = CatalogStore::in_memory().unwrap();
    let record = upsert(&mut store, conversation(20), "restart", 1);
    let old_host = host(100);
    let new_host = host(200);
    let old_attachment = attach(&mut store, old_host, record.id, 9);

    let reconciliation = store.startup_reconcile(new_host, at(50)).unwrap();
    assert_eq!(reconciliation.stale_attachments_invalidated, 1);
    assert!(store.current_attachment(record.id).unwrap().is_none());
    assert_eq!(
        store.runtime_snapshot(record.id).unwrap().unwrap().state,
        RuntimeState::NotRunning
    );

    let stale_detach = store
        .detach_attachment(
            record.id,
            agent_protocol::AttachmentVersion {
                host_instance_id: old_attachment.host_instance_id,
                attachment_generation: old_attachment.attachment_generation,
            },
            "late old-host detach",
            at(60),
        )
        .unwrap();
    assert!(!stale_detach);

    let new_attachment = attach(&mut store, new_host, record.id, 9);
    assert_eq!(new_attachment.attachment_generation, 1);
    assert_eq!(new_attachment.host_instance_id, new_host);
}

#[test]
fn structured_event_rebinds_once_and_late_old_session_end_cannot_detach_new_binding() {
    let mut store = CatalogStore::in_memory().unwrap();
    let a = upsert(&mut store, conversation(30), "session-a", 1);
    let hid = host(300);
    let attached_a = attach(&mut store, hid, a.id, 11);

    let rebind = reduce_queued_event(
        &mut store,
        queued_event(
            hid,
            11,
            "event-b-start",
            AgentEventKind::SessionStart,
            "session-b",
            20,
            json!({}),
        ),
    )
    .unwrap();
    let EventReduction::Applied {
        events,
        conversation_id,
    } = rebind
    else {
        panic!("expected applied rebind");
    };
    assert_eq!(events.len(), 3);
    assert_ne!(conversation_id, a.id);
    assert!(store.current_attachment(a.id).unwrap().is_none());
    let b_attachment = store.current_attachment(conversation_id).unwrap().unwrap();
    assert_eq!(b_attachment.pane_id, attached_a.pane_id);
    assert_eq!(b_attachment.process_owner, attached_a.process_owner);
    assert_eq!(
        b_attachment.attachment_generation,
        attached_a.attachment_generation + 1
    );
    assert_eq!(
        store.runtime_snapshot(a.id).unwrap().unwrap().state,
        RuntimeState::NotRunning
    );

    let late_end = reduce_queued_event(
        &mut store,
        queued_event(
            hid,
            11,
            "event-a-end",
            AgentEventKind::SessionEnd,
            "session-a",
            25,
            json!({}),
        ),
    )
    .unwrap();
    assert!(matches!(late_end, EventReduction::Applied { .. }));
    assert_eq!(
        store
            .current_attachment(conversation_id)
            .unwrap()
            .unwrap()
            .conversation_id,
        conversation_id
    );
}

#[test]
fn superseded_pending_identity_is_not_exposed_as_a_duplicate_conversation() {
    let mut store = CatalogStore::in_memory().unwrap();
    let pending = upsert(&mut store, conversation(31), "pending:launch", 1);
    let hid = host(301);

    assert!(store
        .list_conversations(None, 10)
        .unwrap()
        .conversations
        .is_empty());
    attach(&mut store, hid, pending.id, 12);
    assert_eq!(
        store
            .list_conversations(None, 10)
            .unwrap()
            .conversations
            .iter()
            .map(|record| record.id)
            .collect::<Vec<_>>(),
        vec![pending.id]
    );

    let EventReduction::Applied {
        conversation_id: rebound,
        ..
    } = reduce_queued_event(
        &mut store,
        queued_event(
            hid,
            12,
            "event-promote-pending",
            AgentEventKind::SessionStart,
            "session-promoted",
            20,
            json!({}),
        ),
    )
    .unwrap()
    else {
        panic!("expected applied pending identity promotion");
    };

    let listed = store.list_conversations(None, 10).unwrap().conversations;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, rebound);
    assert_eq!(listed[0].native.native_session_id, "session-promoted");
    assert!(store.get_conversation(pending.id).unwrap().is_some());
}

#[test]
fn bounded_ingress_overflow_marks_resync_without_blocking_callback_path() {
    let mut store = CatalogStore::in_memory().unwrap();
    let record = upsert(&mut store, conversation(40), "overflow", 1);
    let hid = host(400);
    attach(&mut store, hid, record.id, 12);

    let ingress = AgentEventIngress::with_capacity(hid, 1);
    ingress.mark_owned(PaneId(12));
    let payload = queued_event(
        hid,
        12,
        "overflow-1",
        AgentEventKind::PromptSubmit,
        "overflow",
        10,
        json!({}),
    )
    .json;
    assert_eq!(
        ingress.enqueue_user_var(
            PaneId(12),
            agent_protocol::RESERVED_AGENT_EVENT_USER_VAR,
            &payload,
            at(10)
        ),
        IngressOutcome::Enqueued
    );
    assert_eq!(
        ingress.enqueue_user_var(
            PaneId(12),
            agent_protocol::RESERVED_AGENT_EVENT_USER_VAR,
            &payload,
            at(11)
        ),
        IngressOutcome::Overflow {
            pane_id: PaneId(12)
        }
    );
    let overflowed = ingress.take_overflowed_panes();
    assert_eq!(overflowed, vec![PaneId(12)]);
    store
        .mark_event_resync_required(hid, overflowed[0], at(12))
        .unwrap();
    assert!(store.event_resync_required(record.id).unwrap());

    let oversized = "x".repeat(agent_protocol::MAX_AGENT_EVENT_JSON_BYTES + 1);
    assert_eq!(
        ingress.enqueue_user_var(
            PaneId(12),
            agent_protocol::RESERVED_AGENT_EVENT_USER_VAR,
            &oversized,
            at(13)
        ),
        IngressOutcome::RejectedOversized {
            bytes: oversized.len()
        }
    );
}

#[test]
fn pending_identity_hint_has_single_frozen_confirmation_window() {
    let now = at(1);
    let pending =
        pending_identity_from_submitted_hint(host(500), PaneId(55), "candidate", "ev-1", now);
    assert_eq!(pending.candidate_native_session_id, "candidate");
    assert_eq!(
        pending.expires_at - pending.observed_at,
        Duration::seconds(30)
    );
}

#[test]
fn host_commands_and_tray_state_are_pure_until_gui_binding() {
    let mut store = CatalogStore::in_memory().unwrap();
    let record = upsert(&mut store, conversation(60), "host", 1);
    let hid = host(600);
    attach(&mut store, hid, record.id, 13);
    let mut host = HostController::new(store, hid).unwrap();

    let status = host.handle_host_request(HostRequest::HostStatus).unwrap();
    assert!(
        matches!(status, HostResult::Status { host_instance_id, shutting_down: false } if host_instance_id == hid)
    );

    host.handle_host_request(HostRequest::ConversationOpen {
        conversation_id: record.id,
    })
    .unwrap();
    assert_eq!(
        host.take_open_decisions(),
        vec![(record.id, OpenConversationDecision::FocusExisting)]
    );

    host.handle_host_request(HostRequest::ConversationSetOrganizationState {
        conversation_id: record.id,
        organization_state: OrganizationState::Settled,
    })
    .unwrap();
    host.handle_host_request(HostRequest::ConversationStop {
        conversation_id: record.id,
    })
    .unwrap();
    assert_eq!(
        host.store()
            .get_conversation(record.id)
            .unwrap()
            .unwrap()
            .organization_state,
        OrganizationState::Settled
    );

    let (intent, action) = host.tray_mut().ui_close_requested();
    assert_eq!(intent, agent_terminal::host::CloseIntent::HideToTray);
    assert_eq!(action, TrayAction::HideUi);
    assert_eq!(host.tray().visibility(), TrayVisibility::HiddenToTray);
    assert_eq!(host.tray_mut().reopen_requested(), TrayAction::ShowUi);
    assert_eq!(host.tray().visibility(), TrayVisibility::Visible);
    assert_eq!(host.tray_mut().stop_all_requested(), TrayAction::StopAll);
    assert!(host.tray_mut().take_stop_all_requested());

    let response = host.handle_ipc_request(IpcRequest {
        id: 7,
        request: HostRequest::HostQuit,
    });
    assert!(matches!(
        response.payload,
        IpcResponsePayload::Ok(HostResult::Accepted)
    ));
    let status = host.handle_host_request(HostRequest::HostStatus).unwrap();
    assert!(matches!(
        status,
        HostResult::Status {
            shutting_down: true,
            ..
        }
    ));
}

#[test]
fn ten_thousand_row_first_page_query_has_bounded_deserialization_work() {
    let mut store = CatalogStore::in_memory().unwrap();
    for index in 0..10_000u128 {
        upsert(
            &mut store,
            conversation(10_000 + index),
            &format!("bulk-{index:05}"),
            index as i64,
        );
    }

    let start = Instant::now();
    let page = store.list_conversations(None, 50).unwrap();
    let query_elapsed = start.elapsed();
    assert_eq!(page.conversations.len(), 50);
    assert!(page.next_cursor.is_some());

    let serialized = serde_json::to_vec(&page).unwrap();
    let deserialize_start = Instant::now();
    let decoded: agent_protocol::ConversationPage = serde_json::from_slice(&serialized).unwrap();
    let deserialize_elapsed = deserialize_start.elapsed();
    assert_eq!(decoded.conversations.len(), 50);
    assert!(serialized.len() < 64 * 1024);
    println!(
        "AT-102 10000-row catalog: first_page_query_ms={} first_page_json_bytes={} deserialize_us={}",
        query_elapsed.as_secs_f64() * 1000.0,
        serialized.len(),
        deserialize_elapsed.as_micros()
    );
}
