//! Deterministic identity and evidence reducer.
//!
//! This reducer implements the frozen native-identity transition rules for
//! one `(adapter_id, profile_id)` scope owned by one host instance:
//!
//! - Structured events carrying a native ID are authoritative evidence and
//!   may create and confirm an identity atomically.
//! - Submitted text (`/new`) is only ever a candidate: it creates at most
//!   one [`PendingIdentity`] per pane, superseding any older candidate, and
//!   never confirms by itself.
//! - Store-backed evidence confirms a matching pending candidate, or
//!   creates and confirms atomically when no candidate exists.
//! - Candidates expire after
//!   [`PENDING_IDENTITY_CONFIRMATION_WINDOW_SECONDS`] using caller-injected
//!   timestamps; expiry leaves the current binding unchanged.
//! - A process crash clears the candidate and applies the observed exit to
//!   the currently bound conversation.
//! - Evidence is deduplicated on its stable [`EvidenceId`] across sources.
//! - Evidence associated with a different host instance is stale: pane IDs
//!   and generation counters from a prior host process never order or
//!   mutate current truth.
//! - After a pane is rebound from native ID A to B, a late `session_end(A)`
//!   updates only A's historical record; it never detaches or alters B.
//!
//! The reducer is pure: it holds no clock, performs no I/O, and every input
//! timestamp is injected by the caller, so fixtures are fully deterministic.

use crate::events;
use agent_protocol::{
    AdapterId, AgentEventEnvelope, AgentEventKind, EvidenceId, HostInstanceId, PaneId,
    PendingIdentity, ProfileId, RuntimeState, PENDING_IDENTITY_CONFIRMATION_WINDOW_SECONDS,
};
use chrono::{DateTime, TimeDelta, Utc};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DetachReason {
    NativeIdentitySuperseded,
    SessionEnded,
    ProcessExit,
}

impl DetachReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NativeIdentitySuperseded => "native_identity_superseded",
            Self::SessionEnded => "session_ended",
            Self::ProcessExit => "process_exit",
        }
    }
}

/// Per-conversation reduced state, keyed by native session id.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversationState {
    pub native_session_id: String,
    pub runtime: RuntimeState,
    /// Highest structured-event sequence applied, for out-of-order evidence.
    pub last_seq: Option<u64>,
    /// A session end was observed (current or historical).
    pub ended: bool,
    /// Why this conversation lost its pane attachment, if it did.
    pub detach_reason: Option<DetachReason>,
}

/// Per-pane reduced state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaneBinding {
    /// Currently bound native session id, if any.
    pub bound: Option<String>,
    /// Current attachment generation, monotonic within this host instance.
    pub generation: u64,
    /// At most one pending identity candidate per pane.
    pub pending: Option<PendingIdentity>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Evidence {
    /// A decoded structured event delivered by `(host, pane)`.
    Structured {
        host: HostInstanceId,
        pane: PaneId,
        envelope: Box<AgentEventEnvelope>,
    },
    /// Raw text submitted to the TUI that is an identity-change candidate.
    /// Candidate input never rebinds immediately.
    SubmittedCandidate {
        host: HostInstanceId,
        pane: PaneId,
        candidate_native_session_id: String,
        evidence_id: EvidenceId,
        observed_at: DateTime<Utc>,
    },
    /// A session-store change correlated to the owned attachment/process.
    StoreChange {
        host: HostInstanceId,
        pane: PaneId,
        native_session_id: String,
        evidence_id: EvidenceId,
        observed_at: DateTime<Utc>,
    },
    /// The owned process exited; `success` distinguishes clean exit from
    /// non-zero/unexpected termination.
    ProcessExit {
        host: HostInstanceId,
        pane: PaneId,
        success: bool,
        observed_at: DateTime<Utc>,
    },
    /// Advance the injected clock to expire candidates.
    Tick { now: DateTime<Utc> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReducerOutcome {
    /// Pane attached to a conversation at the returned generation.
    Attached { session: String, generation: u64 },
    /// Authoritative structured evidence rebound the pane atomically.
    Rebound {
        old: String,
        new: String,
        generation: u64,
    },
    /// Store-backed evidence confirmed a pending candidate (or created and
    /// confirmed atomically when no candidate existed).
    Confirmed {
        old: Option<String>,
        new: String,
        generation: u64,
    },
    /// A new pending candidate was recorded.
    PendingCreated { session: String },
    /// A newer candidate superseded the older one.
    PendingSuperseded { old: String, new: String },
    /// A runtime state transition was applied to the bound conversation.
    RuntimeChanged {
        session: String,
        state: RuntimeState,
    },
    /// Telemetry (usage/context) was accepted without a runtime change.
    Telemetry { session: String },
    /// `session_end` applied to the currently bound conversation.
    Ended { session: String },
    /// A late end for a non-current native ID updated history only; the
    /// current binding is untouched.
    HistoricalOnly { session: String },
    /// Exact duplicate evidence delivery; idempotent no-op.
    DuplicateEvidence,
    /// Structured event with a non-increasing sequence for its session.
    OutOfOrder { session: String },
    /// Evidence from a different host instance, or for a session this pane
    /// is not bound to; rejected without mutating current truth.
    StaleEvidence,
    /// A pending candidate expired; the binding is unchanged.
    Expired { session: String },
    /// A process exit was applied; the candidate was cleared.
    CrashApplied {
        session: Option<String>,
        state: RuntimeState,
    },
    /// An unknown future event variant was quarantined without panic.
    Quarantined { kind: String },
    /// Deterministic no-op.
    Ignored { reason: String },
}

impl ReducerOutcome {
    /// A stable, compact label used by reducer fixture scripts.
    pub fn label(&self) -> String {
        match self {
            Self::Attached {
                session,
                generation,
            } => {
                format!("attach:{session}:g{generation}")
            }
            Self::Rebound {
                old,
                new,
                generation,
            } => format!("rebind:{old}->{new}:g{generation}"),
            Self::Confirmed {
                old,
                new,
                generation,
            } => {
                let old = old.as_deref().unwrap_or("-");
                format!("confirm:{old}->{new}:g{generation}")
            }
            Self::PendingCreated { session } => format!("pending:{session}"),
            Self::PendingSuperseded { old, new } => format!("superseded:{old}->{new}"),
            Self::RuntimeChanged { session, state } => {
                format!("runtime:{session}@{}", runtime_name(*state))
            }
            Self::Telemetry { session } => format!("telemetry:{session}"),
            Self::Ended { session } => format!("ended:{session}"),
            Self::HistoricalOnly { session } => format!("historical:{session}"),
            Self::DuplicateEvidence => "duplicate".to_owned(),
            Self::OutOfOrder { session } => format!("out-of-order:{session}"),
            Self::StaleEvidence => "stale".to_owned(),
            Self::Expired { session } => format!("expired:{session}"),
            Self::CrashApplied { session, state } => match session {
                Some(session) => format!("crash:{session}@{}", runtime_name(*state)),
                None => "crash:none".to_owned(),
            },
            Self::Quarantined { kind } => format!("quarantined:{kind}"),
            Self::Ignored { reason } => format!("ignored:{reason}"),
        }
    }
}

impl fmt::Display for ReducerOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.label())
    }
}

/// Serde-compatible snake_case name of a runtime state, for labels.
pub fn runtime_name(state: RuntimeState) -> &'static str {
    match state {
        RuntimeState::NotRunning => "not_running",
        RuntimeState::Starting => "starting",
        RuntimeState::Working => "working",
        RuntimeState::WaitingForInput => "waiting_for_input",
        RuntimeState::AwaitingApproval => "awaiting_approval",
        RuntimeState::CompletedIdle => "completed_idle",
        RuntimeState::Failed => "failed",
        RuntimeState::UnknownExternal => "unknown_external",
    }
}

/// The deterministic reducer for one adapter/profile scope on one host.
pub struct Reducer {
    pub adapter_id: AdapterId,
    pub profile_id: ProfileId,
    pub host_instance_id: HostInstanceId,
    next_generation: u64,
    conversations: BTreeMap<String, ConversationState>,
    panes: BTreeMap<PaneId, PaneBinding>,
    seen_evidence: BTreeSet<EvidenceId>,
}

impl Reducer {
    pub fn new(
        adapter_id: AdapterId,
        profile_id: ProfileId,
        host_instance_id: HostInstanceId,
    ) -> Self {
        Self {
            adapter_id,
            profile_id,
            host_instance_id,
            next_generation: 1,
            conversations: BTreeMap::new(),
            panes: BTreeMap::new(),
            seen_evidence: BTreeSet::new(),
        }
    }

    pub fn conversation(&self, native_session_id: &str) -> Option<&ConversationState> {
        self.conversations.get(native_session_id)
    }

    pub fn conversations(&self) -> &BTreeMap<String, ConversationState> {
        &self.conversations
    }

    pub fn panes(&self) -> &BTreeMap<PaneId, PaneBinding> {
        &self.panes
    }

    pub fn pane(&self, pane: PaneId) -> Option<&PaneBinding> {
        self.panes.get(&pane)
    }

    fn pane_mut(&mut self, pane: PaneId) -> &mut PaneBinding {
        self.panes.entry(pane).or_insert(PaneBinding {
            bound: None,
            generation: 0,
            pending: None,
        })
    }

    fn conversation_mut(&mut self, native_session_id: &str) -> &mut ConversationState {
        self.conversations
            .entry(native_session_id.to_owned())
            .or_insert_with(|| ConversationState {
                native_session_id: native_session_id.to_owned(),
                runtime: RuntimeState::Starting,
                last_seq: None,
                ended: false,
                detach_reason: None,
            })
    }

    /// Apply one piece of evidence and return the deterministic outcome.
    pub fn apply(&mut self, evidence: Evidence) -> ReducerOutcome {
        match evidence {
            Evidence::Structured {
                host,
                pane,
                envelope,
            } => self.apply_structured(host, pane, *envelope),
            Evidence::SubmittedCandidate {
                host,
                pane,
                candidate_native_session_id,
                evidence_id,
                observed_at,
            } => self.apply_candidate(
                host,
                pane,
                candidate_native_session_id,
                evidence_id,
                observed_at,
            ),
            Evidence::StoreChange {
                host,
                pane,
                native_session_id,
                evidence_id,
                observed_at,
            } => self.apply_store(host, pane, native_session_id, evidence_id, observed_at),
            Evidence::ProcessExit {
                host,
                pane,
                success,
                observed_at: _,
            } => self.apply_exit(host, pane, success),
            Evidence::Tick { now } => self.apply_tick(now),
        }
    }

    /// Evidence from another host instance is never current truth. This is
    /// what makes a stale `(old_host_instance_id, generation)` detach or
    /// rebind harmless after a host restart.
    fn check_host(&mut self, host: HostInstanceId) -> bool {
        host == self.host_instance_id
    }

    fn dedupe(&mut self, evidence_id: EvidenceId) -> bool {
        self.seen_evidence.insert(evidence_id)
    }

    fn apply_structured(
        &mut self,
        host: HostInstanceId,
        pane: PaneId,
        envelope: AgentEventEnvelope,
    ) -> ReducerOutcome {
        if !self.check_host(host) {
            return ReducerOutcome::StaleEvidence;
        }
        if envelope.adapter != self.adapter_id || envelope.profile_id != self.profile_id {
            return ReducerOutcome::StaleEvidence;
        }
        if !self.dedupe(events::evidence_id(&envelope)) {
            return ReducerOutcome::DuplicateEvidence;
        }

        if let AgentEventKind::Unknown(kind) = &envelope.event {
            return ReducerOutcome::Quarantined { kind: kind.clone() };
        }

        let session = envelope.session_id.clone();

        // Out-of-order delivery: a non-increasing sequence for an already
        // known session is idempotent and never regresses state.
        let seq = envelope.data.get("seq").and_then(serde_json::Value::as_u64);
        if let Some(seq) = seq {
            if let Some(conversation) = self.conversations.get(&session) {
                if conversation.last_seq.is_some_and(|last| seq <= last) {
                    return ReducerOutcome::OutOfOrder { session };
                }
            }
            self.conversation_mut(&session).last_seq = Some(seq);
        }

        match &envelope.event {
            AgentEventKind::SessionStart => self.apply_session_start(pane, session),
            AgentEventKind::SessionEnd => self.apply_session_end(pane, session),
            AgentEventKind::UsageUpdated | AgentEventKind::ContextUpdated => {
                if self.bound_session(pane).as_deref() == Some(session.as_str()) {
                    ReducerOutcome::Telemetry { session }
                } else {
                    ReducerOutcome::StaleEvidence
                }
            }
            kind => {
                let Some(state) = runtime_for(kind) else {
                    return ReducerOutcome::Ignored {
                        reason: "no-runtime-mapping".to_owned(),
                    };
                };
                if self.bound_session(pane).as_deref() != Some(session.as_str()) {
                    return ReducerOutcome::StaleEvidence;
                }
                self.conversation_mut(&session).runtime = state;
                ReducerOutcome::RuntimeChanged { session, state }
            }
        }
    }

    /// Atomic create-and-confirm from authoritative structured evidence.
    fn apply_session_start(&mut self, pane: PaneId, session: String) -> ReducerOutcome {
        self.conversation_mut(&session);
        let old = self.bound_session(pane);
        match old {
            None => {
                let generation = self.attach(pane, &session);
                ReducerOutcome::Attached {
                    session,
                    generation,
                }
            }
            Some(old) if old == session => ReducerOutcome::Ignored {
                reason: "already-bound".to_owned(),
            },
            Some(old) => {
                self.detach(&old, DetachReason::NativeIdentitySuperseded);
                let generation = self.attach(pane, &session);
                ReducerOutcome::Rebound {
                    old,
                    new: session,
                    generation,
                }
            }
        }
    }

    fn apply_session_end(&mut self, pane: PaneId, session: String) -> ReducerOutcome {
        let bound = self.bound_session(pane);
        if bound.as_deref() == Some(session.as_str()) {
            self.detach(&session, DetachReason::SessionEnded);
            let conversation = self.conversation_mut(&session);
            conversation.ended = true;
            conversation.runtime = RuntimeState::CompletedIdle;
            self.pane_mut(pane).bound = None;
            self.pane_mut(pane).pending = None;
            return ReducerOutcome::Ended { session };
        }
        // Late end for a non-current native ID: update the historical
        // record only. It must never detach or alter the new binding.
        if self.conversations.contains_key(&session) {
            self.conversation_mut(&session).ended = true;
            return ReducerOutcome::HistoricalOnly { session };
        }
        ReducerOutcome::Ignored {
            reason: "unknown-session".to_owned(),
        }
    }

    fn apply_candidate(
        &mut self,
        host: HostInstanceId,
        pane: PaneId,
        candidate: String,
        evidence_id: EvidenceId,
        observed_at: DateTime<Utc>,
    ) -> ReducerOutcome {
        if !self.check_host(host) {
            return ReducerOutcome::StaleEvidence;
        }
        if !self.dedupe(evidence_id.clone()) {
            return ReducerOutcome::DuplicateEvidence;
        }
        let pending = PendingIdentity {
            host_instance_id: self.host_instance_id,
            pane_id: pane,
            candidate_native_session_id: candidate.clone(),
            evidence_id,
            observed_at,
            expires_at: observed_at
                + TimeDelta::seconds(PENDING_IDENTITY_CONFIRMATION_WINDOW_SECONDS as i64),
        };
        let binding = self.pane_mut(pane);
        match &binding.pending {
            Some(existing) if existing.candidate_native_session_id == candidate => {
                // A newer hint for the same candidate refreshes the window.
                binding.pending = Some(pending);
                ReducerOutcome::PendingCreated { session: candidate }
            }
            Some(existing) => {
                let old = existing.candidate_native_session_id.clone();
                binding.pending = Some(pending);
                ReducerOutcome::PendingSuperseded {
                    old,
                    new: candidate,
                }
            }
            None => {
                binding.pending = Some(pending);
                ReducerOutcome::PendingCreated { session: candidate }
            }
        }
    }

    fn apply_store(
        &mut self,
        host: HostInstanceId,
        pane: PaneId,
        session: String,
        evidence_id: EvidenceId,
        observed_at: DateTime<Utc>,
    ) -> ReducerOutcome {
        if !self.check_host(host) {
            return ReducerOutcome::StaleEvidence;
        }
        if !self.dedupe(evidence_id) {
            return ReducerOutcome::DuplicateEvidence;
        }

        if let Some(binding) = self.panes.get_mut(&pane) {
            if binding
                .pending
                .as_ref()
                .is_some_and(|pending| observed_at >= pending.expires_at)
            {
                binding.pending = None;
            }
            if binding
                .pending
                .as_ref()
                .is_some_and(|pending| pending.candidate_native_session_id != session)
            {
                return ReducerOutcome::StaleEvidence;
            }
        }

        // Store-backed evidence confirms a matching pending candidate, or
        // creates and confirms atomically when no live candidate exists;
        // either path is the same atomic rebind operation. The 30-second
        // window constrains candidates, not store evidence.
        self.pane_mut(pane).pending = None;
        self.conversation_mut(&session);
        let old = self.bound_session(pane);
        match old {
            Some(old) if old == session => ReducerOutcome::Ignored {
                reason: "already-bound".to_owned(),
            },
            Some(old) => {
                self.detach(&old, DetachReason::NativeIdentitySuperseded);
                let generation = self.attach(pane, &session);
                ReducerOutcome::Confirmed {
                    old: Some(old),
                    new: session,
                    generation,
                }
            }
            None => {
                let generation = self.attach(pane, &session);
                ReducerOutcome::Confirmed {
                    old: None,
                    new: session,
                    generation,
                }
            }
        }
    }

    fn apply_exit(&mut self, host: HostInstanceId, pane: PaneId, success: bool) -> ReducerOutcome {
        if !self.check_host(host) {
            return ReducerOutcome::StaleEvidence;
        }
        // A crash/exit always clears the pending candidate.
        if let Some(binding) = self.panes.get_mut(&pane) {
            binding.pending = None;
        }
        let bound = self.bound_session(pane);
        match bound {
            None => ReducerOutcome::CrashApplied {
                session: None,
                state: RuntimeState::NotRunning,
            },
            Some(session) => {
                let state = if success {
                    RuntimeState::CompletedIdle
                } else {
                    RuntimeState::Failed
                };
                self.detach(&session, DetachReason::ProcessExit);
                let conversation = self.conversation_mut(&session);
                conversation.runtime = state;
                conversation.ended = true;
                self.pane_mut(pane).bound = None;
                ReducerOutcome::CrashApplied {
                    session: Some(session),
                    state,
                }
            }
        }
    }

    fn apply_tick(&mut self, now: DateTime<Utc>) -> ReducerOutcome {
        // BTreeMap iteration is in pane order, so the first expired
        // candidate found is the deterministic choice.
        let expired = self.panes.iter().find_map(|(pane, binding)| {
            binding
                .pending
                .as_ref()
                .filter(|pending| now >= pending.expires_at)
                .map(|pending| (*pane, pending.candidate_native_session_id.clone()))
        });
        match expired {
            Some((pane, session)) => {
                self.pane_mut(pane).pending = None;
                ReducerOutcome::Expired { session }
            }
            None => ReducerOutcome::Ignored {
                reason: "nothing-expired".to_owned(),
            },
        }
    }

    fn bound_session(&self, pane: PaneId) -> Option<String> {
        self.panes
            .get(&pane)
            .and_then(|binding| binding.bound.clone())
    }

    /// Attach `pane` to `session` at the next generation; clears any pending
    /// candidate because the identity is now confirmed.
    fn attach(&mut self, pane: PaneId, session: &str) -> u64 {
        let generation = self.next_generation;
        self.next_generation += 1;
        let binding = self.pane_mut(pane);
        binding.bound = Some(session.to_owned());
        binding.generation = generation;
        binding.pending = None;
        generation
    }

    fn detach(&mut self, session: &str, reason: DetachReason) {
        let conversation = self.conversation_mut(session);
        conversation.runtime = RuntimeState::NotRunning;
        conversation.detach_reason = Some(reason);
    }
}

/// Runtime mapping for structured event kinds that imply a state.
fn runtime_for(kind: &AgentEventKind) -> Option<RuntimeState> {
    match kind {
        AgentEventKind::PromptSubmit | AgentEventKind::ToolComplete => Some(RuntimeState::Working),
        AgentEventKind::PermissionReplied => Some(RuntimeState::Working),
        AgentEventKind::TurnComplete | AgentEventKind::IdlePrompt => {
            Some(RuntimeState::WaitingForInput)
        }
        AgentEventKind::QuestionAsked => Some(RuntimeState::WaitingForInput),
        AgentEventKind::PermissionRequest => Some(RuntimeState::AwaitingApproval),
        AgentEventKind::TurnFailed => Some(RuntimeState::Failed),
        _ => None,
    }
}
