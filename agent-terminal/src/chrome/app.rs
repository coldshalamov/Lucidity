use crate::catalog::{AttachmentInput, CatalogStore};
use crate::host::{HostController, OpenConversationDecision, TrayVisibility};
#[cfg(windows)]
use crate::ipc::service::HostService;
use crate::runtime::{
    install_mux_agent_event_subscriber, reduce_queued_event, AgentEventIngress, EventReduction,
};
use agent_backends::discovery::{ExecutableLocator, SystemPathLocator};
use agent_backends::manifest::load_manifest;
use agent_backends::template::{expand_command, TemplateContext};
use agent_protocol::{
    AdapterId, AdapterManifest, Confidence, ConversationId, ConversationRecord, HostEvent,
    HostInstanceId, HostRequest, ObservationSource, OrganizationState, PaneId, ProcessOwnerId,
    ProfileId, RuntimeSnapshot, RuntimeState,
};
use chrono::Utc;
use parking_lot::Mutex;
#[cfg(windows)]
use portable_pty::cmdbuilder::WindowsProcessTreePolicy;
use portable_pty::CommandBuilder;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use uuid::Uuid;

#[cfg(windows)]
use crate::windows::{TrayCallbacks, TrayController as NativeTrayController};

const ACTION_PUMP_INTERVAL: Duration = Duration::from_millis(25);
const MOCK_ADAPTER_ID: &str = "mock-agent";

const BUNDLED_ADAPTER_MANIFESTS: &[(&str, &str)] = &[
    (
        "claude.agent-adapter",
        include_str!("../../../agent-backends/adapters/claude.agent-adapter/adapter.toml"),
    ),
    (
        "codex.agent-adapter",
        include_str!("../../../agent-backends/adapters/codex.agent-adapter/adapter.toml"),
    ),
    (
        "kimi.agent-adapter",
        include_str!("../../../agent-backends/adapters/kimi.agent-adapter/adapter.toml"),
    ),
    (
        "mock-agent.agent-adapter",
        include_str!("../../../agent-backends/adapters/mock-agent.agent-adapter/adapter.toml"),
    ),
];

pub(super) struct ProductApplication {
    host: Arc<Mutex<HostController>>,
    adapters: HashMap<AdapterId, AdapterManifest>,
    ingress: AgentEventIngress,
    launching: Mutex<HashSet<ConversationId>>,
    selected: Mutex<Option<ConversationId>>,
    shutdown_requested: AtomicBool,
    window_hidden: AtomicBool,
    pump: Mutex<Option<JoinHandle<()>>>,
    #[cfg(windows)]
    tray: Mutex<Option<NativeTrayController>>,
    #[cfg(windows)]
    host_service: Mutex<Option<HostService>>,
}

impl ProductApplication {
    pub(super) fn bootstrap() -> anyhow::Result<Arc<Self>> {
        let catalog_path = catalog_path()?;
        if let Some(parent) = catalog_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let recovery = CatalogStore::open_or_rebuild(&catalog_path)?;
        if let Some(warning) = recovery.warning.as_deref() {
            log::warn!("catalog recovery: {warning}");
        }

        let host_instance_id = HostInstanceId(Uuid::new_v4());
        let ingress = AgentEventIngress::new(host_instance_id);
        let adapter_manifests = load_bundled_adapters()?;
        let adapters = adapter_manifests
            .iter()
            .cloned()
            .map(|manifest| (manifest.id.clone(), manifest))
            .collect();
        let mut host = HostController::new(recovery.store, host_instance_id)?;
        host.replace_adapters(adapter_manifests);

        let host = Arc::new(Mutex::new(host));
        #[cfg(windows)]
        let host_service = HostService::spawn_shared(Arc::clone(&host))?;

        Ok(Arc::new(Self {
            host,
            adapters,
            ingress,
            launching: Mutex::new(HashSet::new()),
            selected: Mutex::new(None),
            shutdown_requested: AtomicBool::new(false),
            window_hidden: AtomicBool::new(false),
            pump: Mutex::new(None),
            #[cfg(windows)]
            tray: Mutex::new(None),
            #[cfg(windows)]
            host_service: Mutex::new(Some(host_service)),
        }))
    }

    pub(super) fn gui_hooks(self: &Arc<Self>) -> wezterm_gui::ProductGuiHooks {
        let ready = Arc::downgrade(self);
        let hidden = Arc::downgrade(self);
        wezterm_gui::ProductGuiHooks {
            close_to_tray: cfg!(windows),
            on_ready: Some(Arc::new(move || {
                if let Some(application) = ready.upgrade() {
                    application.gui_ready();
                }
            })),
            on_window_hidden: Some(Arc::new(move || {
                if let Some(application) = hidden.upgrade() {
                    application.window_hidden();
                }
            })),
        }
    }

    pub(super) fn sidebar_provider(self: &Arc<Self>) -> wezterm_gui::ProductSidebarProvider {
        let snapshot = Arc::downgrade(self);
        let activate = Arc::downgrade(self);
        wezterm_gui::ProductSidebarProvider::new(
            move || {
                snapshot
                    .upgrade()
                    .map(|application| application.sidebar_snapshot())
                    .unwrap_or_default()
            },
            move |row_id| {
                let Some(application) = activate.upgrade() else {
                    return;
                };
                let Ok(conversation_id) = row_id.as_str().parse::<ConversationId>() else {
                    log::warn!("invalid sidebar conversation id {row_id}");
                    return;
                };
                let result = {
                    let mut host = application.host.lock();
                    host.handle_host_request(HostRequest::ConversationOpen { conversation_id })
                };
                if let Err(error) = result {
                    log::error!("failed to open sidebar conversation {conversation_id}: {error:#}");
                }
            },
        )
    }

    fn sidebar_snapshot(&self) -> wezterm_gui::ProductSidebarSnapshot {
        let selected = *self.selected.lock();
        let host = self.host.lock();
        let conversations = match host.store().list_conversations(None, 200) {
            Ok(page) => page.conversations,
            Err(error) => {
                log::error!("failed to build product sidebar: {error:#}");
                return wezterm_gui::ProductSidebarSnapshot::default();
            }
        };
        let mut snapshot = wezterm_gui::ProductSidebarSnapshot::default();
        for conversation in conversations {
            let status = host
                .store()
                .runtime_snapshot(conversation.id)
                .ok()
                .flatten()
                .map(|runtime| sidebar_status(runtime.state))
                .unwrap_or(wezterm_gui::ProductSidebarStatus::NotRunning);
            let row = wezterm_gui::ProductSidebarRow {
                id: conversation.id.to_string().into(),
                title: conversation
                    .user_alias
                    .clone()
                    .unwrap_or_else(|| conversation.title.clone()),
                metadata: sidebar_metadata(&conversation),
                status,
                attached: selected == Some(conversation.id),
            };
            match conversation.organization_state {
                OrganizationState::Active => snapshot.active.push(row),
                OrganizationState::Settled => snapshot.settled.push(row),
            }
        }
        snapshot
    }

    fn gui_ready(self: &Arc<Self>) {
        if let Err(error) = install_mux_agent_event_subscriber(self.ingress.clone()) {
            log::error!("failed to install Agent event bridge: {error:#}");
        }

        #[cfg(windows)]
        {
            let callbacks: Arc<dyn TrayCallbacks> = Arc::new(ProductTrayCallbacks {
                application: Arc::downgrade(self),
            });
            match NativeTrayController::start(callbacks) {
                Ok(controller) => {
                    self.tray.lock().replace(controller);
                }
                Err(error) => log::error!("failed to start Lucidity tray: {error:#}"),
            }
        }

        self.seed_mock_if_empty();
        self.start_action_pump();
    }

    fn seed_mock_if_empty(&self) {
        let mut host = self.host.lock();
        let empty = host
            .store()
            .list_conversations(None, 1)
            .map(|page| page.conversations.is_empty())
            .unwrap_or(false);
        if !empty
            || !self
                .adapters
                .contains_key(&AdapterId::from(MOCK_ADAPTER_ID))
        {
            return;
        }
        let request = HostRequest::ConversationNew {
            adapter_id: AdapterId::from(MOCK_ADAPTER_ID),
            profile_id: ProfileId(Uuid::new_v4()),
            project_path: std::env::current_dir().ok(),
        };
        if let Err(error) = host.handle_host_request(request) {
            log::error!("failed to seed the mock Agent session: {error:#}");
        }
    }

    fn start_action_pump(self: &Arc<Self>) {
        if self.pump.lock().is_some() {
            return;
        }
        let application = Arc::downgrade(self);
        match thread::Builder::new()
            .name("lucidity-product-actions".to_owned())
            .spawn(move || loop {
                let Some(application) = application.upgrade() else {
                    break;
                };
                if application.shutdown_requested.load(Ordering::Acquire) {
                    break;
                }
                application.pump_host_actions();
                thread::sleep(ACTION_PUMP_INTERVAL);
            }) {
            Ok(thread) => {
                self.pump.lock().replace(thread);
            }
            Err(error) => log::error!("failed to start product action pump: {error:#}"),
        }
    }

    fn pump_host_actions(self: &Arc<Self>) {
        while let Some(event) = self.ingress.try_recv() {
            let mut host = self.host.lock();
            match reduce_queued_event(host.store_mut(), event) {
                Ok(EventReduction::Applied { events, .. }) => {
                    for event in events {
                        host.push_event(event);
                    }
                }
                Ok(EventReduction::Duplicate) => {}
                Ok(EventReduction::Quarantined { reason })
                | Ok(EventReduction::Rejected { reason }) => {
                    log::warn!("Agent event ignored: {reason}");
                }
                Err(error) => log::error!("failed to reduce Agent event: {error:#}"),
            }
        }

        let actions = {
            let mut host = self.host.lock();
            host.take_open_decisions()
                .into_iter()
                .filter_map(|(conversation_id, decision)| {
                    let record = match host.store().get_conversation(conversation_id) {
                        Ok(Some(record)) => record,
                        Ok(None) => return None,
                        Err(error) => {
                            log::error!("cannot load conversation {conversation_id}: {error:#}");
                            return None;
                        }
                    };
                    let pane = host
                        .store()
                        .current_attachment(conversation_id)
                        .ok()
                        .flatten()
                        .map(|attachment| attachment.pane_id);
                    Some((record, decision, pane))
                })
                .collect::<Vec<_>>()
        };

        for (record, decision, pane) in actions {
            match (decision, pane) {
                (OpenConversationDecision::FocusExisting, Some(pane_id)) => {
                    self.selected.lock().replace(record.id);
                    wezterm_gui::request_product_focus_pane(pane_id.0 as usize);
                }
                (OpenConversationDecision::Missing, _) => {}
                (OpenConversationDecision::FocusExisting, None)
                | (OpenConversationDecision::LaunchNewRuntime, _) => {
                    self.launch_conversation(record);
                }
            }
        }

        let tray_visibility = self.host.lock().tray().visibility();
        match tray_visibility {
            TrayVisibility::Visible if self.window_hidden.swap(false, Ordering::AcqRel) => {
                wezterm_gui::request_product_window_open();
            }
            TrayVisibility::Quitting => self.request_quit(),
            TrayVisibility::Visible | TrayVisibility::HiddenToTray => {}
        }
    }

    fn launch_conversation(self: &Arc<Self>, record: ConversationRecord) {
        if !self.launching.lock().insert(record.id) {
            return;
        }
        self.selected.lock().replace(record.id);
        let request = match self.spawn_request(&record) {
            Ok(request) => request,
            Err(error) => {
                self.launching.lock().remove(&record.id);
                self.record_launch_failure(record.id, format!("{error:#}"));
                return;
            }
        };
        let application = Arc::downgrade(self);
        let conversation_id = record.id;
        wezterm_gui::request_product_spawn(
            request,
            Arc::new(move |outcome| {
                let Some(application) = application.upgrade() else {
                    return;
                };
                application.launching.lock().remove(&conversation_id);
                match outcome {
                    wezterm_gui::ProductSpawnOutcome::Spawned { pane_id } => {
                        application.attach_spawned_pane(conversation_id, pane_id)
                    }
                    wezterm_gui::ProductSpawnOutcome::Failed { message } => {
                        application.record_launch_failure(conversation_id, message)
                    }
                }
            }),
        );
    }

    fn spawn_request(
        &self,
        record: &ConversationRecord,
    ) -> anyhow::Result<wezterm_gui::ProductSpawnRequest> {
        let manifest = self
            .adapters
            .get(&record.native.adapter_id)
            .ok_or_else(|| {
                anyhow::anyhow!("adapter {} is not installed", record.native.adapter_id)
            })?;
        let project_path = record
            .project_path
            .clone()
            .or_else(|| std::env::current_dir().ok())
            .ok_or_else(|| anyhow::anyhow!("conversation has no project directory"))?;
        let project_text = project_path.to_string_lossy();
        let profile_text = record.native.profile_id.to_string();
        let is_pending = record.native.native_session_id.starts_with("pending:");
        let template = if is_pending {
            &manifest.launch
        } else {
            manifest.resume.as_ref().unwrap_or(&manifest.launch)
        };
        let expanded = expand_command(
            template,
            &TemplateContext {
                project_path: Some(project_text.as_ref()),
                native_session_id: (!is_pending)
                    .then_some(record.native.native_session_id.as_str()),
                profile_id: Some(profile_text.as_str()),
            },
        )?;
        let executable = resolve_executable(manifest, &expanded.executable);
        let mut command = CommandBuilder::new(executable);
        command.args(expanded.args);
        command.cwd(&project_path);
        #[cfg(windows)]
        command.set_windows_process_tree_policy(WindowsProcessTreePolicy::OwnedJob);
        Ok(wezterm_gui::ProductSpawnRequest {
            command,
            cwd: Some(project_path),
        })
    }

    fn attach_spawned_pane(&self, conversation_id: ConversationId, pane_id: usize) {
        let pane_id = PaneId(pane_id as u64);
        let mut host = self.host.lock();
        let host_instance_id = host.host_instance_id();
        let attachment = host.store_mut().register_attachment(
            host_instance_id,
            AttachmentInput {
                conversation_id,
                pane_id,
                process_id: None,
                process_owner: Some(ProcessOwnerId::WindowsJob {
                    opaque_id: format!("mux-pane:{}", pane_id.0),
                }),
                attached_at: Utc::now(),
            },
        );
        match attachment {
            Ok(attachment) => {
                self.selected.lock().replace(conversation_id);
                self.ingress.mark_owned(pane_id);
                host.push_event(HostEvent::RuntimeAttached { attachment });
                host.push_event(HostEvent::ConversationChanged { conversation_id });
            }
            Err(error) => log::error!("failed to attach pane to {conversation_id}: {error:#}"),
        }
    }

    fn record_launch_failure(&self, conversation_id: ConversationId, message: String) {
        let mut host = self.host.lock();
        if let Err(error) = host.store_mut().update_runtime_snapshot(RuntimeSnapshot {
            conversation_id,
            state: RuntimeState::Failed,
            source: ObservationSource::ProviderApi,
            confidence: Confidence::Authoritative,
            observed_at: Utc::now(),
            last_error: Some(message.clone()),
        }) {
            log::error!("failed to record launch error for {conversation_id}: {error:#}");
        }
        host.push_event(HostEvent::ConversationChanged { conversation_id });
        log::error!("failed to launch conversation {conversation_id}: {message}");
    }

    fn window_hidden(&self) {
        self.window_hidden.store(true, Ordering::Release);
        let _ = self.host.lock().tray_mut().ui_close_requested();
    }

    fn request_open(&self) {
        self.window_hidden.store(false, Ordering::Release);
        let _ = self.host.lock().tray_mut().reopen_requested();
        wezterm_gui::request_product_window_open();
    }

    fn request_quit(&self) {
        if self
            .shutdown_requested
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        {
            let mut host = self.host.lock();
            let _ = host.tray_mut().stop_all_requested();
            if let Err(error) = host.handle_host_request(HostRequest::HostQuit) {
                log::error!("failed to transition host to quit: {error:#}");
            }
        }
        wezterm_gui::request_product_exit();
    }

    pub(super) fn shutdown(&self) {
        self.shutdown_requested.store(true, Ordering::Release);
        #[cfg(windows)]
        if let Some(mut service) = self.host_service.lock().take() {
            if let Err(error) = service.stop() {
                log::error!("failed to stop Lucidity control pipe: {error:#}");
            }
        }
        if let Some(thread) = self.pump.lock().take() {
            if thread.thread().id() != thread::current().id() {
                let _ = thread.join();
            }
        }
        #[cfg(windows)]
        if let Some(tray) = self.tray.lock().take() {
            if let Err(error) = tray.shutdown() {
                log::error!("failed to stop Lucidity tray: {error:#}");
            }
        }
    }
}

#[cfg(windows)]
struct ProductTrayCallbacks {
    application: Weak<ProductApplication>,
}

#[cfg(windows)]
impl TrayCallbacks for ProductTrayCallbacks {
    fn request_open_existing_window(&self) {
        if let Some(application) = self.application.upgrade() {
            application.request_open();
        }
    }

    fn request_quit_owned(&self) {
        if let Some(application) = self.application.upgrade() {
            application.request_quit();
        }
    }
}

fn load_bundled_adapters() -> anyhow::Result<Vec<AdapterManifest>> {
    let mut manifests = Vec::with_capacity(BUNDLED_ADAPTER_MANIFESTS.len());
    for (package, document) in BUNDLED_ADAPTER_MANIFESTS {
        let manifest = load_manifest(document)
            .map_err(|errors| anyhow::anyhow!("invalid bundled adapter {package}: {errors:?}"))?;
        manifests.push(manifest);
    }
    manifests.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(manifests)
}

fn sidebar_status(state: RuntimeState) -> wezterm_gui::ProductSidebarStatus {
    match state {
        RuntimeState::NotRunning => wezterm_gui::ProductSidebarStatus::NotRunning,
        RuntimeState::Starting => wezterm_gui::ProductSidebarStatus::Starting,
        RuntimeState::Working => wezterm_gui::ProductSidebarStatus::Working,
        RuntimeState::WaitingForInput => wezterm_gui::ProductSidebarStatus::WaitingForInput,
        RuntimeState::AwaitingApproval => wezterm_gui::ProductSidebarStatus::AwaitingApproval,
        RuntimeState::CompletedIdle => wezterm_gui::ProductSidebarStatus::CompletedIdle,
        RuntimeState::Failed => wezterm_gui::ProductSidebarStatus::Failed,
        RuntimeState::UnknownExternal => wezterm_gui::ProductSidebarStatus::UnknownExternal,
    }
}

fn sidebar_metadata(conversation: &ConversationRecord) -> String {
    let project = conversation
        .project_path
        .as_deref()
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "no project".to_owned());
    let metadata = format!("{} · {}", conversation.native.adapter_id, project);
    metadata.chars().take(72).collect()
}

fn resolve_executable(manifest: &AdapterManifest, fallback: &str) -> PathBuf {
    if manifest.id.0 == MOCK_ADAPTER_ID {
        if let Some(path) = std::env::var_os("LUCIDITY_MOCK_AGENT")
            .map(PathBuf::from)
            .filter(|path| path.is_file())
        {
            return path;
        }
        if let Ok(current_exe) = std::env::current_exe() {
            let sibling = current_exe.with_file_name(mock_executable_name());
            if sibling.is_file() {
                return sibling;
            }
        }
        let development = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")))
            .join("target")
            .join("debug")
            .join(mock_executable_name());
        if development.is_file() {
            return development;
        }
    }
    SystemPathLocator
        .locate(&manifest.executable)
        .unwrap_or_else(|_| PathBuf::from(fallback))
}

fn mock_executable_name() -> &'static str {
    if cfg!(windows) {
        "lucidity-mock-agent.exe"
    } else {
        "lucidity-mock-agent"
    }
}

fn catalog_path() -> anyhow::Result<PathBuf> {
    if let Some(path) = std::env::var_os("LUCIDITY_CATALOG_PATH") {
        return Ok(PathBuf::from(path));
    }
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("LOCALAPPDATA").or_else(|| std::env::var_os("APPDATA")) {
        return Ok(PathBuf::from(root).join("Lucidity").join("catalog.sqlite3"));
    }
    #[cfg(not(windows))]
    {
        if let Some(root) = std::env::var_os("XDG_DATA_HOME") {
            return Ok(PathBuf::from(root).join("lucidity").join("catalog.sqlite3"));
        }
        if let Some(home) = std::env::var_os("HOME") {
            return Ok(PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("lucidity")
                .join("catalog.sqlite3"));
        }
    }
    Ok(std::env::current_dir()?
        .join(".lucidity")
        .join("catalog.sqlite3"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_adapter_set_is_complete_and_unique() {
        let adapters = load_bundled_adapters().unwrap();
        let ids = adapters
            .iter()
            .map(|adapter| adapter.id.0.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["claude", "codex", "kimi", "mock-agent"]);
    }
}
