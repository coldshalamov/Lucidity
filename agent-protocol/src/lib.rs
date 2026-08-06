//! Dependency-light domain and wire contracts shared by Lucidity components.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;
use uuid::Uuid;

pub const AGENT_EVENT_VERSION: u32 = 1;
pub const IPC_PROTOCOL_MIN_VERSION: u32 = 1;
pub const IPC_PROTOCOL_MAX_VERSION: u32 = 1;
pub const RESERVED_AGENT_EVENT_USER_VAR: &str = "lucidity.agent-event.v1";
pub const MAX_AGENT_EVENT_JSON_BYTES: usize = 32_768;
pub const MAX_PENDING_AGENT_EVENTS: usize = 256;
pub const MAX_FRAME_BYTES: usize = 1_048_576;
pub const MAX_PENDING_EVENTS_PER_SUBSCRIBER: usize = 256;
pub const PENDING_IDENTITY_CONFIRMATION_WINDOW_SECONDS: u64 = 30;

macro_rules! uuid_id {
    ($name:ident) => {
        #[derive(
            Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(value).map(Self)
            }
        }
    };
}

macro_rules! string_id {
    ($name:ident) => {
        #[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }
    };
}

uuid_id!(ConversationId);
uuid_id!(ProfileId);
uuid_id!(HostInstanceId);
string_id!(AdapterId);
string_id!(EvidenceId);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct PaneId(pub u64);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct TabId(pub u64);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeConversationKey {
    pub adapter_id: AdapterId,
    pub profile_id: ProfileId,
    pub native_session_id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationState {
    Active,
    Settled,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeState {
    NotRunning,
    Starting,
    Working,
    WaitingForInput,
    AwaitingApproval,
    CompletedIdle,
    Failed,
    UnknownExternal,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationRecord {
    pub id: ConversationId,
    pub native: NativeConversationKey,
    pub native_session_path: Option<PathBuf>,
    pub title: String,
    pub user_alias: Option<String>,
    pub project_path: Option<PathBuf>,
    pub created_at: DateTime<Utc>,
    pub last_activity_at: DateTime<Utc>,
    pub organization_state: OrganizationState,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ProcessOwnerId {
    WindowsJob { opaque_id: String },
    DirectChild { pid: u32 },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAttachment {
    pub conversation_id: ConversationId,
    pub pane_id: PaneId,
    pub process_id: Option<u32>,
    pub process_owner: Option<ProcessOwnerId>,
    pub host_instance_id: HostInstanceId,
    pub attachment_generation: u64,
    pub attached_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationSource {
    StructuredEvent,
    ProviderApi,
    SessionStore,
    SubmittedCommand,
    ScreenObservation,
    UserBinding,
    ProcessExit,
    Recovery,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Authoritative,
    Correlated,
    Heuristic,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSnapshot {
    pub conversation_id: ConversationId,
    pub state: RuntimeState,
    pub source: ObservationSource,
    pub confidence: Confidence,
    pub observed_at: DateTime<Utc>,
    pub last_error: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentVersion {
    pub host_instance_id: HostInstanceId,
    pub attachment_generation: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingIdentity {
    pub host_instance_id: HostInstanceId,
    pub pane_id: PaneId,
    pub candidate_native_session_id: String,
    pub evidence_id: EvidenceId,
    pub observed_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestedTermination {
    None,
    WezTermKill,
    LucidityStop,
    LucidityQuit,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ExitResult {
    Exited { code: Option<u32>, success: bool },
    WaiterError { message: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessExitObservation {
    pub pane_id: PaneId,
    pub result: ExitResult,
    pub requested_termination: RequestedTermination,
    pub observed_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum AgentEventKind {
    SessionStart,
    PromptSubmit,
    ToolComplete,
    TurnComplete,
    TurnFailed,
    PermissionRequest,
    PermissionReplied,
    QuestionAsked,
    IdlePrompt,
    ContextUpdated,
    UsageUpdated,
    SessionEnd,
    Unknown(String),
}

impl AgentEventKind {
    pub fn as_str(&self) -> &str {
        match self {
            Self::SessionStart => "session_start",
            Self::PromptSubmit => "prompt_submit",
            Self::ToolComplete => "tool_complete",
            Self::TurnComplete => "turn_complete",
            Self::TurnFailed => "turn_failed",
            Self::PermissionRequest => "permission_request",
            Self::PermissionReplied => "permission_replied",
            Self::QuestionAsked => "question_asked",
            Self::IdlePrompt => "idle_prompt",
            Self::ContextUpdated => "context_updated",
            Self::UsageUpdated => "usage_updated",
            Self::SessionEnd => "session_end",
            Self::Unknown(value) => value,
        }
    }
}

impl From<&str> for AgentEventKind {
    fn from(value: &str) -> Self {
        match value {
            "session_start" => Self::SessionStart,
            "prompt_submit" => Self::PromptSubmit,
            "tool_complete" => Self::ToolComplete,
            "turn_complete" => Self::TurnComplete,
            "turn_failed" => Self::TurnFailed,
            "permission_request" => Self::PermissionRequest,
            "permission_replied" => Self::PermissionReplied,
            "question_asked" => Self::QuestionAsked,
            "idle_prompt" => Self::IdlePrompt,
            "context_updated" => Self::ContextUpdated,
            "usage_updated" => Self::UsageUpdated,
            "session_end" => Self::SessionEnd,
            other => Self::Unknown(other.to_owned()),
        }
    }
}

impl Serialize for AgentEventKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for AgentEventKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer).map(|value| Self::from(value.as_str()))
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEventEnvelope {
    pub v: u32,
    pub event_id: String,
    pub adapter: AdapterId,
    pub profile_id: ProfileId,
    pub event: AgentEventKind,
    pub session_id: String,
    pub cwd: PathBuf,
    pub transcript_path: Option<PathBuf>,
    pub occurred_at: DateTime<Utc>,
    #[serde(default)]
    pub data: Value,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandTemplate {
    pub executable: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub shell: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutableDiscovery {
    #[serde(default)]
    pub names: Vec<String>,
    #[serde(default)]
    pub explicit_paths: Vec<PathBuf>,
    pub version_probe: Option<CommandTemplate>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySource {
    pub kind: String,
    pub path_template: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterManifest {
    pub schema_version: u32,
    pub id: AdapterId,
    pub display_name: String,
    pub executable: ExecutableDiscovery,
    pub launch: CommandTemplate,
    pub resume: Option<CommandTemplate>,
    #[serde(default)]
    pub history_sources: Vec<HistorySource>,
    #[serde(default)]
    pub event_capabilities: Vec<AgentEventKind>,
    pub settings_schema: Option<PathBuf>,
    #[serde(default)]
    pub extensions: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostHelloRequest {
    pub client_name: String,
    pub min_version: u32,
    pub max_version: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostHelloResponse {
    pub negotiated_version: u32,
    pub host_instance_id: HostInstanceId,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogCursor {
    pub catalog_snapshot_version: u64,
    pub last_activity_at: DateTime<Utc>,
    pub conversation_id: ConversationId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationPage {
    pub catalog_snapshot_version: u64,
    pub conversations: Vec<ConversationRecord>,
    pub next_cursor: Option<CatalogCursor>,
    pub resync_required: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "method", content = "params")]
pub enum HostRequest {
    #[serde(rename = "host.hello")]
    HostHello(HostHelloRequest),
    #[serde(rename = "host.status")]
    HostStatus,
    #[serde(rename = "host.openUi")]
    HostOpenUi,
    #[serde(rename = "host.quit")]
    HostQuit,
    #[serde(rename = "conversation.list")]
    ConversationList {
        cursor: Option<CatalogCursor>,
        limit: u32,
    },
    #[serde(rename = "conversation.get")]
    ConversationGet { conversation_id: ConversationId },
    #[serde(rename = "conversation.new")]
    ConversationNew {
        adapter_id: AdapterId,
        profile_id: ProfileId,
        project_path: Option<PathBuf>,
    },
    #[serde(rename = "conversation.open")]
    ConversationOpen { conversation_id: ConversationId },
    #[serde(rename = "conversation.setOrganizationState")]
    ConversationSetOrganizationState {
        conversation_id: ConversationId,
        organization_state: OrganizationState,
    },
    #[serde(rename = "conversation.stop")]
    ConversationStop { conversation_id: ConversationId },
    #[serde(rename = "conversation.refreshUsage")]
    ConversationRefreshUsage { conversation_id: ConversationId },
    #[serde(rename = "adapter.list")]
    AdapterList,
    #[serde(rename = "adapter.scan")]
    AdapterScan,
    #[serde(rename = "event.subscribe")]
    EventSubscribe,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IpcRequest {
    pub id: u64,
    #[serde(flatten)]
    pub request: HostRequest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "data")]
pub enum HostResult {
    Hello(HostHelloResponse),
    Accepted,
    Conversation(Option<ConversationRecord>),
    Conversations(ConversationPage),
    Adapters(Vec<AdapterManifest>),
    Status {
        host_instance_id: HostInstanceId,
        shutting_down: bool,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IpcError {
    pub code: String,
    pub message: String,
    pub resync_required: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "status", content = "payload")]
pub enum IpcResponsePayload {
    Ok(HostResult),
    Error(IpcError),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IpcResponse {
    pub id: u64,
    #[serde(flatten)]
    pub payload: IpcResponsePayload,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "event", content = "data")]
pub enum HostEvent {
    #[serde(rename = "catalog.changed")]
    CatalogChanged { catalog_snapshot_version: u64 },
    #[serde(rename = "conversation.changed")]
    ConversationChanged { conversation_id: ConversationId },
    #[serde(rename = "runtime.attached")]
    RuntimeAttached { attachment: RuntimeAttachment },
    #[serde(rename = "runtime.detached")]
    RuntimeDetached {
        conversation_id: ConversationId,
        version: AttachmentVersion,
    },
    #[serde(rename = "usage.changed")]
    UsageChanged { conversation_id: ConversationId },
    #[serde(rename = "adapter.changed")]
    AdapterChanged { adapter_id: AdapterId },
}
