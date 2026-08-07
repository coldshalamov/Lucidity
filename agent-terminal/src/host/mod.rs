//! Host command dispatch and product lifecycle state.

use crate::catalog::{CatalogStore, ConversationUpsert};
use agent_protocol::{
    AdapterManifest, HostEvent, HostHelloResponse, HostInstanceId, HostRequest, HostResult,
    IpcError, IpcRequest, IpcResponse, IpcResponsePayload, NativeConversationKey,
    OrganizationState, RuntimeState, IPC_PROTOCOL_MAX_VERSION, IPC_PROTOCOL_MIN_VERSION,
};
use chrono::Utc;
use std::collections::VecDeque;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseIntent {
    StockClose,
    HideToTray,
    QuitOwned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrayVisibility {
    Visible,
    HiddenToTray,
    Quitting,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrayAction {
    None,
    HideUi,
    ShowUi,
    StopAll,
    QuitOwned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenConversationDecision {
    FocusExisting,
    LaunchNewRuntime,
    Missing,
}

#[derive(Clone, Debug)]
pub struct TrayController {
    visibility: TrayVisibility,
    stop_all_requested: bool,
}

#[derive(Debug)]
pub struct HostController {
    store: CatalogStore,
    host_instance_id: HostInstanceId,
    shutting_down: bool,
    adapters: Vec<AdapterManifest>,
    tray: TrayController,
    events: VecDeque<HostEvent>,
    event_revision: u64,
    open_decisions: Vec<(agent_protocol::ConversationId, OpenConversationDecision)>,
}

impl TrayController {
    pub fn new() -> Self {
        Self {
            visibility: TrayVisibility::Visible,
            stop_all_requested: false,
        }
    }

    pub fn visibility(&self) -> TrayVisibility {
        self.visibility
    }

    pub fn ui_close_requested(&mut self) -> (CloseIntent, TrayAction) {
        if self.visibility == TrayVisibility::Quitting {
            return (CloseIntent::QuitOwned, TrayAction::QuitOwned);
        }
        self.visibility = TrayVisibility::HiddenToTray;
        (CloseIntent::HideToTray, TrayAction::HideUi)
    }

    pub fn reopen_requested(&mut self) -> TrayAction {
        if self.visibility == TrayVisibility::Quitting {
            return TrayAction::None;
        }
        self.visibility = TrayVisibility::Visible;
        TrayAction::ShowUi
    }

    pub fn explicit_quit_requested(&mut self) -> (CloseIntent, TrayAction) {
        self.visibility = TrayVisibility::Quitting;
        (CloseIntent::QuitOwned, TrayAction::QuitOwned)
    }

    pub fn stop_all_requested(&mut self) -> TrayAction {
        self.stop_all_requested = true;
        TrayAction::StopAll
    }

    pub fn take_stop_all_requested(&mut self) -> bool {
        let requested = self.stop_all_requested;
        self.stop_all_requested = false;
        requested
    }
}

impl Default for TrayController {
    fn default() -> Self {
        Self::new()
    }
}

impl HostController {
    pub fn new(mut store: CatalogStore, host_instance_id: HostInstanceId) -> anyhow::Result<Self> {
        store.startup_reconcile(host_instance_id, Utc::now())?;
        Ok(Self {
            store,
            host_instance_id,
            shutting_down: false,
            adapters: Vec::new(),
            tray: TrayController::new(),
            events: VecDeque::new(),
            event_revision: 0,
            open_decisions: Vec::new(),
        })
    }

    pub fn store(&self) -> &CatalogStore {
        &self.store
    }

    pub fn store_mut(&mut self) -> &mut CatalogStore {
        &mut self.store
    }

    pub fn host_instance_id(&self) -> HostInstanceId {
        self.host_instance_id
    }

    pub fn replace_adapters(&mut self, adapters: Vec<AdapterManifest>) {
        self.adapters = adapters;
    }

    pub fn tray(&self) -> &TrayController {
        &self.tray
    }

    pub fn tray_mut(&mut self) -> &mut TrayController {
        &mut self.tray
    }

    pub fn push_event(&mut self, event: HostEvent) {
        self.events.push_back(event);
        self.event_revision = self.event_revision.wrapping_add(1);
    }

    pub fn event_revision(&self) -> u64 {
        self.event_revision
    }

    pub fn pop_event(&mut self) -> Option<HostEvent> {
        self.events.pop_front()
    }

    pub fn take_open_decisions(
        &mut self,
    ) -> Vec<(agent_protocol::ConversationId, OpenConversationDecision)> {
        self.open_decisions.drain(..).collect()
    }

    pub fn handle_ipc_request(&mut self, request: IpcRequest) -> IpcResponse {
        match self.handle_host_request(request.request) {
            Ok(result) => IpcResponse {
                id: request.id,
                payload: IpcResponsePayload::Ok(result),
            },
            Err(error) => IpcResponse {
                id: request.id,
                payload: IpcResponsePayload::Error(IpcError {
                    code: "host_error".to_owned(),
                    message: error.to_string(),
                    resync_required: false,
                }),
            },
        }
    }

    pub fn handle_host_request(&mut self, request: HostRequest) -> anyhow::Result<HostResult> {
        match request {
            HostRequest::HostHello(request) => {
                if request.max_version < IPC_PROTOCOL_MIN_VERSION
                    || request.min_version > IPC_PROTOCOL_MAX_VERSION
                {
                    anyhow::bail!(
                        "unsupported IPC version range {}..{}",
                        request.min_version,
                        request.max_version
                    );
                }
                Ok(HostResult::Hello(HostHelloResponse {
                    negotiated_version: IPC_PROTOCOL_MAX_VERSION.min(request.max_version),
                    host_instance_id: self.host_instance_id,
                    capabilities: vec![
                        "catalog".to_owned(),
                        "runtime".to_owned(),
                        "events".to_owned(),
                        "named_pipe".to_owned(),
                    ],
                }))
            }
            HostRequest::HostStatus => Ok(HostResult::Status {
                host_instance_id: self.host_instance_id,
                shutting_down: self.shutting_down,
            }),
            HostRequest::HostOpenUi => {
                let _ = self.tray.reopen_requested();
                Ok(HostResult::Accepted)
            }
            HostRequest::HostQuit => {
                let _ = self.tray.explicit_quit_requested();
                self.shutting_down = true;
                Ok(HostResult::Accepted)
            }
            HostRequest::ConversationList { cursor, limit } => {
                let page = self.store.list_conversations(cursor, limit)?;
                Ok(HostResult::Conversations(page))
            }
            HostRequest::ConversationGet { conversation_id } => Ok(HostResult::Conversation(
                self.store.get_conversation(conversation_id)?,
            )),
            HostRequest::ConversationNew {
                adapter_id,
                profile_id,
                project_path,
            } => {
                let now = Utc::now();
                self.store
                    .ensure_profile(&adapter_id, profile_id, None, now)?;
                let synthetic_native_id = format!("pending:{}", Uuid::new_v4());
                let record = self.store.upsert_conversation(ConversationUpsert {
                    id: None,
                    native: NativeConversationKey {
                        adapter_id,
                        profile_id,
                        native_session_id: synthetic_native_id,
                    },
                    native_session_path: None,
                    title: "New conversation".to_owned(),
                    user_alias: None,
                    project_path,
                    created_at: now,
                    last_activity_at: now,
                    organization_state: OrganizationState::Active,
                })?;
                self.push_event(HostEvent::CatalogChanged {
                    catalog_snapshot_version: self.store.catalog_snapshot_version()?,
                });
                self.push_event(HostEvent::ConversationChanged {
                    conversation_id: record.id,
                });
                self.open_decisions
                    .push((record.id, OpenConversationDecision::LaunchNewRuntime));
                Ok(HostResult::Conversation(Some(record)))
            }
            HostRequest::ConversationOpen { conversation_id } => {
                let decision = if self.store.get_conversation(conversation_id)?.is_none() {
                    OpenConversationDecision::Missing
                } else if self.store.current_attachment(conversation_id)?.is_some() {
                    OpenConversationDecision::FocusExisting
                } else {
                    OpenConversationDecision::LaunchNewRuntime
                };
                self.open_decisions.push((conversation_id, decision));
                match decision {
                    OpenConversationDecision::Missing => Ok(HostResult::Conversation(None)),
                    OpenConversationDecision::FocusExisting
                    | OpenConversationDecision::LaunchNewRuntime => Ok(HostResult::Accepted),
                }
            }
            HostRequest::ConversationSetOrganizationState {
                conversation_id,
                organization_state,
            } => {
                let changed = self.store.set_organization_state(
                    conversation_id,
                    organization_state,
                    Utc::now(),
                )?;
                if changed {
                    self.push_event(HostEvent::CatalogChanged {
                        catalog_snapshot_version: self.store.catalog_snapshot_version()?,
                    });
                    self.push_event(HostEvent::ConversationChanged { conversation_id });
                }
                Ok(HostResult::Accepted)
            }
            HostRequest::ConversationStop { conversation_id } => {
                if let Some(version) = self.store.stop_conversation(conversation_id, Utc::now())? {
                    self.push_event(HostEvent::RuntimeDetached {
                        conversation_id,
                        version,
                    });
                }
                let _ = self
                    .store
                    .runtime_snapshot(conversation_id)?
                    .is_some_and(|snapshot| snapshot.state == RuntimeState::NotRunning);
                Ok(HostResult::Accepted)
            }
            HostRequest::ConversationRefreshUsage { conversation_id } => {
                self.store.record_usage_snapshot(
                    conversation_id,
                    Utc::now(),
                    "manual_refresh",
                    &serde_json::json!({ "status": "refresh_requested" }),
                    None,
                )?;
                self.push_event(HostEvent::UsageChanged { conversation_id });
                Ok(HostResult::Accepted)
            }
            HostRequest::AdapterList | HostRequest::AdapterScan => {
                Ok(HostResult::Adapters(self.adapters.clone()))
            }
            HostRequest::EventSubscribe => Ok(HostResult::Accepted),
        }
    }
}
