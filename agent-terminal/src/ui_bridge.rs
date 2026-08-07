//! Event-driven UI bridge: immutable snapshots + command channel.
//!
//! The render path must never query SQLite. Host rebuilds the snapshot only
//! when catalog/runtime revision changes and wakes waiters via channels.

use agent_backends::discovery::{ExecutableLocator, SystemPathLocator};
use agent_protocol::{
    AdapterId, AdapterManifest, ConversationId, ConversationRecord, OrganizationState,
    RuntimeState,
};
use lucidity_ui::{
    AdapterOption, RuntimeBadge, SessionRowView, SettingsDraft, UiSnapshot, UsageView,
};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

/// Commands emitted by the Lucidity shell for the product host.
#[derive(Clone, Debug, PartialEq)]
pub enum UiCommand {
    NewConversation {
        adapter_id: String,
        project_path: String,
        title: Option<String>,
        extra_args: Option<String>,
    },
    OpenConversation(Uuid),
    Settle(Uuid),
    Unsettle(Uuid),
    Stop(Uuid),
    Resume(Uuid),
    Rename {
        id: Uuid,
        title: String,
    },
    RefreshUsage(Uuid),
    ApplySettings(SettingsDraft),
    PickProjectFolder,
    ImportHistories,
    Quit,
}

/// Shared, revisioned UI state.
#[derive(Clone)]
pub struct UiBridge {
    snapshot: Arc<RwLock<Arc<UiSnapshot>>>,
    command_tx: Sender<UiCommand>,
    command_rx: Arc<parking_lot::Mutex<Receiver<UiCommand>>>,
    wake_tx: Sender<()>,
    wake_rx: Arc<parking_lot::Mutex<Receiver<()>>>,
    shutdown: Arc<AtomicBool>,
    last_build_revision: Arc<AtomicU64>,
}

impl UiBridge {
    pub fn new() -> Self {
        let (command_tx, command_rx) = mpsc::channel();
        let (wake_tx, wake_rx) = mpsc::channel();
        Self {
            snapshot: Arc::new(RwLock::new(Arc::new(UiSnapshot::default()))),
            command_tx,
            command_rx: Arc::new(parking_lot::Mutex::new(command_rx)),
            wake_tx,
            wake_rx: Arc::new(parking_lot::Mutex::new(wake_rx)),
            shutdown: Arc::new(AtomicBool::new(false)),
            last_build_revision: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn command_sender(&self) -> Sender<UiCommand> {
        self.command_tx.clone()
    }

    pub fn snapshot(&self) -> Arc<UiSnapshot> {
        Arc::clone(&self.snapshot.read())
    }

    pub fn publish(&self, next: UiSnapshot) {
        let revision = next.revision;
        let mut slot = self.snapshot.write();
        if slot.revision == revision && revision != 0 {
            // Identical revision: keep Arc stable (no unnecessary redraw churn).
            return;
        }
        *slot = Arc::new(next);
        self.last_build_revision.store(revision, Ordering::Release);
        let _ = self.wake_tx.send(());
    }

    pub fn wake(&self) {
        let _ = self.wake_tx.send(());
    }

    pub fn request_shutdown(&self) {
        self.shutdown.store(true, Ordering::Release);
        self.wake();
    }

    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }

    /// Block until a wake, command, timeout, or shutdown.
    pub fn wait_for_work(&self, timeout: Duration) -> BridgeWake {
        if self.is_shutdown() {
            return BridgeWake::Shutdown;
        }
        // Prefer pending commands
        {
            let rx = self.command_rx.lock();
            match rx.try_recv() {
                Ok(cmd) => return BridgeWake::Command(cmd),
                Err(TryRecvError::Disconnected) => return BridgeWake::Shutdown,
                Err(TryRecvError::Empty) => {}
            }
        }
        let wake_rx = self.wake_rx.lock();
        match wake_rx.recv_timeout(timeout) {
            Ok(()) => {
                // Drain extra wakes
                while wake_rx.try_recv().is_ok() {}
                // Check commands after wake
                drop(wake_rx);
                let rx = self.command_rx.lock();
                match rx.try_recv() {
                    Ok(cmd) => BridgeWake::Command(cmd),
                    Err(TryRecvError::Empty) => BridgeWake::StateChanged,
                    Err(TryRecvError::Disconnected) => BridgeWake::Shutdown,
                }
            }
            Err(RecvTimeoutError::Timeout) => BridgeWake::Timeout,
            Err(RecvTimeoutError::Disconnected) => BridgeWake::Shutdown,
        }
    }

    pub fn try_recv_command(&self) -> Option<UiCommand> {
        match self.command_rx.lock().try_recv() {
            Ok(cmd) => Some(cmd),
            Err(_) => None,
        }
    }

    pub fn push_command_for_test(&self, command: UiCommand) {
        let _ = self.command_tx.send(command);
        self.wake();
    }
}

impl Default for UiBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub enum BridgeWake {
    Command(UiCommand),
    StateChanged,
    Timeout,
    Shutdown,
}

/// Build a UI snapshot from catalog rows without retaining store locks.
pub fn build_ui_snapshot(
    revision: u64,
    product_version: &str,
    demo_mode: bool,
    conversations: &[ConversationRecord],
    runtime: &HashMap<ConversationId, RuntimeState>,
    attachments: &HashMap<ConversationId, bool>,
    adapters: &[AdapterManifest],
    selected: Option<ConversationId>,
    usage: UsageView,
    settings: SettingsDraft,
    status_message: Option<String>,
    launch_error: Option<String>,
) -> UiSnapshot {
    let adapter_names: HashMap<String, String> = adapters
        .iter()
        .map(|a| (a.id.0.clone(), a.display_name.clone()))
        .collect();

    let mut active = Vec::new();
    let mut history = Vec::new();
    for conversation in conversations {
        let state = runtime
            .get(&conversation.id)
            .copied()
            .unwrap_or(RuntimeState::NotRunning);
        let attached = attachments
            .get(&conversation.id)
            .copied()
            .unwrap_or(false);
        let row = SessionRowView {
            id: conversation.id.0,
            title: conversation
                .user_alias
                .clone()
                .unwrap_or_else(|| conversation.title.clone()),
            adapter_id: conversation.native.adapter_id.0.clone(),
            adapter_display_name: adapter_names
                .get(&conversation.native.adapter_id.0)
                .cloned()
                .unwrap_or_else(|| conversation.native.adapter_id.0.clone()),
            project_label: project_label(conversation.project_path.as_deref()),
            status: runtime_badge(state),
            attached,
            settled: conversation.organization_state == OrganizationState::Settled,
            can_resume: !attached && state != RuntimeState::Starting,
            has_live_runtime: attached
                || matches!(
                    state,
                    RuntimeState::Starting
                        | RuntimeState::Working
                        | RuntimeState::WaitingForInput
                        | RuntimeState::AwaitingApproval
                ),
        };
        match conversation.organization_state {
            OrganizationState::Active => active.push(row),
            OrganizationState::Settled => history.push(row),
        }
    }

    let adapter_options = adapters
        .iter()
        .filter(|a| demo_mode || a.id.0 != "mock-agent")
        .map(|a| AdapterOption {
            id: a.id.0.clone(),
            display_name: a.display_name.clone(),
            executable_found: executable_found(a),
            default_args: a.launch.args.clone(),
            missing_hint: if executable_found(a) {
                None
            } else {
                Some(format!(
                    "Install `{}` or set an executable override in Settings → Agents.",
                    a.executable.names.first().cloned().unwrap_or_else(|| a.id.0.clone())
                ))
            },
        })
        .collect();

    // In demo mode include mock; also always include configured adapters list for settings.
    let mut adapters_for_ui = adapter_options;
    if demo_mode {
        // already includes mock via filter
    } else {
        // Settings still needs mock hidden from New dialog (handled in UI); adapters list above filters mock.
        let _ = AdapterId::from("mock-agent");
    }

    // Re-add full adapter list for settings pages (including mock visibility only in demo).
    // `adapters_for_ui` already correct.

    UiSnapshot {
        revision,
        product_version: product_version.to_owned(),
        demo_mode,
        active,
        history,
        adapters: adapters_for_ui,
        selected: selected.map(|id| id.0),
        usage,
        settings,
        status_message,
        launch_error,
    }
}

fn project_label(path: Option<&Path>) -> String {
    path.and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "no project".to_owned())
}

fn runtime_badge(state: RuntimeState) -> RuntimeBadge {
    match state {
        RuntimeState::NotRunning => RuntimeBadge::NotRunning,
        RuntimeState::Starting => RuntimeBadge::Starting,
        RuntimeState::Working => RuntimeBadge::Working,
        RuntimeState::WaitingForInput => RuntimeBadge::WaitingForInput,
        RuntimeState::AwaitingApproval => RuntimeBadge::AwaitingApproval,
        RuntimeState::CompletedIdle => RuntimeBadge::CompletedIdle,
        RuntimeState::Failed => RuntimeBadge::Failed,
        RuntimeState::UnknownExternal => RuntimeBadge::UnknownExternal,
    }
}

fn executable_found(manifest: &AdapterManifest) -> bool {
    if manifest.id.0 == "mock-agent" {
        return mock_agent_available();
    }
    SystemPathLocator.locate(&manifest.executable).is_ok()
}

fn mock_agent_available() -> bool {
    if std::env::var_os("LUCIDITY_MOCK_AGENT").is_some() {
        return true;
    }
    if let Ok(current) = std::env::current_exe() {
        let sibling = current.with_file_name(if cfg!(windows) {
            "lucidity-mock-agent.exe"
        } else {
            "lucidity-mock-agent"
        });
        if sibling.is_file() {
            return true;
        }
    }
    false
}

/// Map shell actions into bridge commands.
pub fn shell_action_to_command(action: lucidity_ui::ShellAction) -> UiCommand {
    match action {
        lucidity_ui::ShellAction::NewConversation {
            adapter_id,
            project_path,
            title,
            extra_args,
        } => UiCommand::NewConversation {
            adapter_id,
            project_path,
            title,
            extra_args,
        },
        lucidity_ui::ShellAction::OpenConversation(id) => UiCommand::OpenConversation(id),
        lucidity_ui::ShellAction::Settle(id) => UiCommand::Settle(id),
        lucidity_ui::ShellAction::Unsettle(id) => UiCommand::Unsettle(id),
        lucidity_ui::ShellAction::Stop(id) => UiCommand::Stop(id),
        lucidity_ui::ShellAction::Resume(id) => UiCommand::Resume(id),
        lucidity_ui::ShellAction::Rename { id, title } => UiCommand::Rename { id, title },
        lucidity_ui::ShellAction::RefreshUsage(id) => UiCommand::RefreshUsage(id),
        lucidity_ui::ShellAction::ApplySettings(settings) => UiCommand::ApplySettings(settings),
        lucidity_ui::ShellAction::PickProjectFolder => UiCommand::PickProjectFolder,
        lucidity_ui::ShellAction::ImportHistories => UiCommand::ImportHistories,
        lucidity_ui::ShellAction::Quit => UiCommand::Quit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_protocol::{NativeConversationKey, ProfileId};
    use chrono::Utc;

    fn sample_conversation(title: &str, settled: bool) -> ConversationRecord {
        ConversationRecord {
            id: ConversationId(Uuid::new_v4()),
            native: NativeConversationKey {
                adapter_id: AdapterId::from("claude"),
                profile_id: ProfileId(Uuid::new_v4()),
                native_session_id: "sess-1".into(),
            },
            native_session_path: None,
            title: title.into(),
            user_alias: None,
            project_path: Some(std::path::PathBuf::from(r"C:\work\Lucidity")),
            created_at: Utc::now(),
            last_activity_at: Utc::now(),
            organization_state: if settled {
                OrganizationState::Settled
            } else {
                OrganizationState::Active
            },
        }
    }

    #[test]
    fn snapshot_pointer_stable_for_same_revision() {
        let bridge = UiBridge::new();
        let mut snap = UiSnapshot::default();
        snap.revision = 3;
        bridge.publish(snap.clone());
        let first = bridge.snapshot();
        bridge.publish(snap);
        let second = bridge.snapshot();
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn build_separates_active_and_history() {
        let active = sample_conversation("A", false);
        let settled = sample_conversation("B", true);
        let mut runtime = HashMap::new();
        runtime.insert(active.id, RuntimeState::Working);
        runtime.insert(settled.id, RuntimeState::Working);
        let mut attachments = HashMap::new();
        attachments.insert(active.id, true);
        attachments.insert(settled.id, true);
        let snap = build_ui_snapshot(
            1,
            "0.1.0",
            false,
            &[active.clone(), settled.clone()],
            &runtime,
            &attachments,
            &[],
            Some(active.id),
            UsageView::default(),
            SettingsDraft::default(),
            None,
            None,
        );
        assert_eq!(snap.active.len(), 1);
        assert_eq!(snap.history.len(), 1);
        assert!(snap.history[0].has_live_runtime);
        assert_eq!(snap.history[0].status, RuntimeBadge::Working);
    }

    #[test]
    fn commands_preserve_order() {
        let bridge = UiBridge::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        bridge.push_command_for_test(UiCommand::Settle(a));
        bridge.push_command_for_test(UiCommand::Stop(b));
        assert_eq!(bridge.try_recv_command(), Some(UiCommand::Settle(a)));
        assert_eq!(bridge.try_recv_command(), Some(UiCommand::Stop(b)));
    }
}
