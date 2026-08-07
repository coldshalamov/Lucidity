//! Runtime attachment, event-ingest, and reducer logic for Agent panes.

use crate::catalog::{AttachmentInput, CatalogStore, ConversationUpsert};
use agent_protocol::{
    AgentEventEnvelope, AgentEventKind, AttachmentVersion, Confidence, ConversationId, HostEvent,
    HostInstanceId, NativeConversationKey, ObservationSource, PaneId, PendingIdentity,
    RuntimeSnapshot, RuntimeState, AGENT_EVENT_VERSION, MAX_AGENT_EVENT_JSON_BYTES,
    MAX_PENDING_AGENT_EVENTS, PENDING_IDENTITY_CONFIRMATION_WINDOW_SECONDS,
    RESERVED_AGENT_EVENT_USER_VAR,
};
use anyhow::{bail, Result};
use async_channel::{bounded, Receiver, Sender, TryRecvError, TrySendError};
use chrono::{DateTime, Duration, Utc};
use parking_lot::Mutex;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedAgentEvent {
    pub host_instance_id: HostInstanceId,
    pub pane_id: PaneId,
    pub json: String,
    pub enqueued_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IngressOutcome {
    IgnoredName,
    IgnoredUnownedPane,
    RejectedOversized { bytes: usize },
    Enqueued,
    Overflow { pane_id: PaneId },
}

#[derive(Clone, Debug)]
pub struct AgentEventIngress {
    host_instance_id: HostInstanceId,
    sender: Sender<QueuedAgentEvent>,
    receiver: Receiver<QueuedAgentEvent>,
    owned_panes: Arc<Mutex<HashSet<PaneId>>>,
    overflowed_panes: Arc<Mutex<VecDeque<PaneId>>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventReduction {
    Applied {
        conversation_id: ConversationId,
        events: Vec<HostEvent>,
    },
    Duplicate,
    Quarantined {
        reason: String,
    },
    Rejected {
        reason: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdentityEvidence {
    Candidate(PendingIdentity),
    Confirmed {
        from: ConversationId,
        to: ConversationId,
        version: AttachmentVersion,
    },
    Ignored {
        reason: String,
    },
}

impl AgentEventIngress {
    pub fn new(host_instance_id: HostInstanceId) -> Self {
        Self::with_capacity(host_instance_id, MAX_PENDING_AGENT_EVENTS)
    }

    pub fn with_capacity(host_instance_id: HostInstanceId, capacity: usize) -> Self {
        let (sender, receiver) = bounded(capacity);
        Self {
            host_instance_id,
            sender,
            receiver,
            owned_panes: Arc::new(Mutex::new(HashSet::new())),
            overflowed_panes: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn mark_owned(&self, pane_id: PaneId) {
        self.owned_panes.lock().insert(pane_id);
    }

    pub fn mark_unowned(&self, pane_id: PaneId) {
        self.owned_panes.lock().remove(&pane_id);
    }

    pub fn enqueue_user_var(
        &self,
        pane_id: PaneId,
        name: &str,
        value: &str,
        now: DateTime<Utc>,
    ) -> IngressOutcome {
        if name != RESERVED_AGENT_EVENT_USER_VAR {
            return IngressOutcome::IgnoredName;
        }
        if value.len() > MAX_AGENT_EVENT_JSON_BYTES {
            return IngressOutcome::RejectedOversized { bytes: value.len() };
        }
        if !self.owned_panes.lock().contains(&pane_id) {
            return IngressOutcome::IgnoredUnownedPane;
        }

        let queued = QueuedAgentEvent {
            host_instance_id: self.host_instance_id,
            pane_id,
            json: value.to_owned(),
            enqueued_at: now,
        };
        match self.sender.try_send(queued) {
            Ok(()) => IngressOutcome::Enqueued,
            Err(TrySendError::Full(event)) => {
                self.overflowed_panes.lock().push_back(event.pane_id);
                IngressOutcome::Overflow {
                    pane_id: event.pane_id,
                }
            }
            Err(TrySendError::Closed(event)) => {
                self.overflowed_panes.lock().push_back(event.pane_id);
                IngressOutcome::Overflow {
                    pane_id: event.pane_id,
                }
            }
        }
    }

    pub fn try_recv(&self) -> Option<QueuedAgentEvent> {
        match self.receiver.try_recv() {
            Ok(event) => Some(event),
            Err(TryRecvError::Empty | TryRecvError::Closed) => None,
        }
    }

    pub fn take_overflowed_panes(&self) -> Vec<PaneId> {
        self.overflowed_panes.lock().drain(..).collect()
    }

    pub fn pending_len(&self) -> usize {
        self.receiver.len()
    }
}

pub fn record_pending_identity(store: &mut CatalogStore, pending: PendingIdentity) -> Result<()> {
    store.connection().execute(
        r#"
        INSERT INTO pending_identities(
          host_instance_id, pane_id, candidate_native_session_id, evidence_id, observed_at, expires_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT(host_instance_id, pane_id) DO UPDATE SET
          candidate_native_session_id = excluded.candidate_native_session_id,
          evidence_id = excluded.evidence_id,
          observed_at = excluded.observed_at,
          expires_at = excluded.expires_at
        "#,
        rusqlite::params![
            pending.host_instance_id.to_string(),
            pending.pane_id.0 as i64,
            pending.candidate_native_session_id,
            pending.evidence_id.0,
            pending.observed_at.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
            pending.expires_at.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        ],
    )?;
    Ok(())
}

pub fn pending_identity_from_submitted_hint(
    host_instance_id: HostInstanceId,
    pane_id: PaneId,
    native_session_id: impl Into<String>,
    evidence_id: impl Into<String>,
    now: DateTime<Utc>,
) -> PendingIdentity {
    PendingIdentity {
        host_instance_id,
        pane_id,
        candidate_native_session_id: native_session_id.into(),
        evidence_id: agent_protocol::EvidenceId(evidence_id.into()),
        observed_at: now,
        expires_at: now + Duration::seconds(PENDING_IDENTITY_CONFIRMATION_WINDOW_SECONDS as i64),
    }
}

pub fn reduce_queued_event(
    store: &mut CatalogStore,
    queued: QueuedAgentEvent,
) -> Result<EventReduction> {
    if queued.json.len() > MAX_AGENT_EVENT_JSON_BYTES {
        return Ok(EventReduction::Rejected {
            reason: "oversized event payload".to_owned(),
        });
    }

    let envelope: AgentEventEnvelope = match serde_json::from_str(&queued.json) {
        Ok(envelope) => envelope,
        Err(error) => {
            return Ok(EventReduction::Quarantined {
                reason: format!("malformed event JSON: {error}"),
            });
        }
    };
    if envelope.v != AGENT_EVENT_VERSION {
        return Ok(EventReduction::Rejected {
            reason: format!("unsupported event version {}", envelope.v),
        });
    }
    if matches!(envelope.event, AgentEventKind::Unknown(_)) {
        return Ok(EventReduction::Quarantined {
            reason: format!("unknown event kind {}", envelope.event.as_str()),
        });
    }

    let current =
        match store.current_attachment_for_pane(queued.host_instance_id, queued.pane_id)? {
            Some(attachment) => attachment,
            None => {
                return Ok(EventReduction::Rejected {
                    reason: "event from unowned pane".to_owned(),
                });
            }
        };

    let Some(current_record) = store.get_conversation(current.conversation_id)? else {
        return Ok(EventReduction::Rejected {
            reason: "event attachment references missing conversation".to_owned(),
        });
    };
    if current_record.native.adapter_id != envelope.adapter
        || current_record.native.profile_id != envelope.profile_id
    {
        return Ok(EventReduction::Rejected {
            reason: "event adapter/profile does not match current attachment".to_owned(),
        });
    }

    let mut target_conversation_id = current.conversation_id;
    let mut host_events = Vec::new();

    if envelope.event == AgentEventKind::SessionEnd
        && envelope.session_id != current_record.native.native_session_id
    {
        let native = NativeConversationKey {
            adapter_id: envelope.adapter.clone(),
            profile_id: envelope.profile_id,
            native_session_id: envelope.session_id.clone(),
        };
        if let Some(record) = store.conversation_by_native(&native)? {
            target_conversation_id = record.id;
        }
    } else if envelope.session_id != current_record.native.native_session_id {
        let target = store.upsert_conversation(ConversationUpsert {
            id: None,
            native: NativeConversationKey {
                adapter_id: envelope.adapter.clone(),
                profile_id: envelope.profile_id,
                native_session_id: envelope.session_id.clone(),
            },
            native_session_path: envelope.transcript_path.clone(),
            title: envelope.session_id.clone(),
            user_alias: None,
            project_path: Some(envelope.cwd.clone()),
            created_at: envelope.occurred_at,
            last_activity_at: envelope.occurred_at,
            organization_state: current_record.organization_state,
        })?;
        let version = AttachmentVersion {
            host_instance_id: current.host_instance_id,
            attachment_generation: current.attachment_generation,
        };
        if let Some(rebind) = store.rebind_attachment(
            current.conversation_id,
            target.id,
            version,
            envelope.occurred_at,
        )? {
            host_events.push(HostEvent::RuntimeDetached {
                conversation_id: current.conversation_id,
                version: rebind.detached,
            });
            host_events.push(HostEvent::RuntimeAttached {
                attachment: rebind.attached,
            });
            target_conversation_id = target.id;
        }
    }

    let inserted = store.record_agent_event(
        &envelope.adapter,
        envelope.profile_id,
        &envelope.event_id,
        Some(target_conversation_id),
        envelope.occurred_at,
    )?;
    if !inserted {
        return Ok(EventReduction::Duplicate);
    }

    if envelope.event == AgentEventKind::UsageUpdated {
        store.record_usage_snapshot(
            target_conversation_id,
            envelope.occurred_at,
            "structured_event",
            &envelope.data,
            None,
        )?;
        host_events.push(HostEvent::UsageChanged {
            conversation_id: target_conversation_id,
        });
    }

    if envelope.event == AgentEventKind::SessionEnd {
        apply_session_end(
            store,
            queued,
            envelope,
            target_conversation_id,
            &mut host_events,
        )?;
        return Ok(EventReduction::Applied {
            conversation_id: target_conversation_id,
            events: host_events,
        });
    }

    let runtime_state = runtime_state_for_event(&envelope.event);
    store.update_runtime_snapshot(RuntimeSnapshot {
        conversation_id: target_conversation_id,
        state: runtime_state,
        source: ObservationSource::StructuredEvent,
        confidence: Confidence::Authoritative,
        observed_at: envelope.occurred_at,
        last_error: if runtime_state == RuntimeState::Failed {
            Some("turn failed".to_owned())
        } else {
            None
        },
    })?;
    store.touch_conversation(target_conversation_id, envelope.occurred_at)?;
    host_events.push(HostEvent::ConversationChanged {
        conversation_id: target_conversation_id,
    });

    Ok(EventReduction::Applied {
        conversation_id: target_conversation_id,
        events: host_events,
    })
}

fn apply_session_end(
    store: &mut CatalogStore,
    queued: QueuedAgentEvent,
    envelope: AgentEventEnvelope,
    event_conversation_id: ConversationId,
    host_events: &mut Vec<HostEvent>,
) -> Result<()> {
    let current = store.current_attachment_for_pane(queued.host_instance_id, queued.pane_id)?;
    if let Some(current) = current {
        let current_record = store
            .get_conversation(current.conversation_id)?
            .ok_or_else(|| anyhow::anyhow!("current attachment references missing conversation"))?;
        if current_record.native.native_session_id == envelope.session_id {
            let version = AttachmentVersion {
                host_instance_id: current.host_instance_id,
                attachment_generation: current.attachment_generation,
            };
            if store.detach_attachment(
                current.conversation_id,
                version,
                "SessionEnd",
                envelope.occurred_at,
            )? {
                host_events.push(HostEvent::RuntimeDetached {
                    conversation_id: current.conversation_id,
                    version,
                });
            }
        }
    }
    store.update_runtime_snapshot(RuntimeSnapshot {
        conversation_id: event_conversation_id,
        state: RuntimeState::NotRunning,
        source: ObservationSource::StructuredEvent,
        confidence: Confidence::Correlated,
        observed_at: envelope.occurred_at,
        last_error: None,
    })?;
    store.touch_conversation(event_conversation_id, envelope.occurred_at)?;
    host_events.push(HostEvent::ConversationChanged {
        conversation_id: event_conversation_id,
    });
    Ok(())
}

fn runtime_state_for_event(kind: &AgentEventKind) -> RuntimeState {
    match kind {
        AgentEventKind::SessionStart
        | AgentEventKind::PromptSubmit
        | AgentEventKind::ToolComplete
        | AgentEventKind::ContextUpdated
        | AgentEventKind::UsageUpdated => RuntimeState::Working,
        AgentEventKind::PermissionRequest => RuntimeState::AwaitingApproval,
        AgentEventKind::PermissionReplied
        | AgentEventKind::QuestionAsked
        | AgentEventKind::IdlePrompt => RuntimeState::WaitingForInput,
        AgentEventKind::TurnComplete => RuntimeState::CompletedIdle,
        AgentEventKind::TurnFailed => RuntimeState::Failed,
        AgentEventKind::SessionEnd => RuntimeState::NotRunning,
        AgentEventKind::Unknown(_) => RuntimeState::UnknownExternal,
    }
}

pub fn install_mux_agent_event_subscriber(ingress: AgentEventIngress) -> Result<()> {
    let mux = mux::Mux::try_get().ok_or_else(|| anyhow::anyhow!("mux is not initialized"))?;
    mux.subscribe(move |notification| {
        if let mux::MuxNotification::Alert { pane_id, alert } = notification {
            if let Some((name, value)) = extract_set_user_var_from_debug(&format!("{alert:?}")) {
                let _ = ingress.enqueue_user_var(PaneId(pane_id as u64), &name, &value, Utc::now());
            }
        }
        true
    });
    Ok(())
}

fn extract_set_user_var_from_debug(debug: &str) -> Option<(String, String)> {
    if !debug.starts_with("SetUserVar ") {
        return None;
    }
    let name = extract_debug_field(debug, "name")?;
    let value = extract_debug_field(debug, "value")?;
    Some((name, value))
}

fn extract_debug_field(debug: &str, field: &str) -> Option<String> {
    let marker = format!("{field}: \"");
    let start = debug.find(&marker)? + marker.len();
    let mut output = String::new();
    let mut escaped = false;
    for ch in debug[start..].chars() {
        if escaped {
            output.push(match ch {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                '"' => '"',
                '\\' => '\\',
                other => other,
            });
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => return Some(output),
            other => output.push(other),
        }
    }
    None
}
