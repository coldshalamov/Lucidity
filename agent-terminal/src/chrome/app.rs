use crate::catalog::{AttachmentInput, CatalogStore, ConversationUpsert};
use crate::chrome::controller::LucidityUiController;
use crate::chrome::{demo_mode_enabled, PRODUCT_VERSION};
use crate::host::{HostController, OpenConversationDecision, TrayVisibility};
use crate::importers::{import_all_known, imported_to_record};
#[cfg(windows)]
use crate::ipc::service::HostService;
use crate::runtime::{
    install_mux_agent_event_subscriber, reduce_queued_event, AgentEventIngress, EventReduction,
};
use crate::settings::{
    executable_override_for, extra_args_for, load_settings, save_settings, settings_path,
};
use crate::ui_bridge::{build_ui_snapshot, BridgeWake, UiBridge, UiCommand};
use crate::usage::usage_view_from_snapshot;
use agent_backends::discovery::{ExecutableLocator, SystemPathLocator};
use agent_backends::manifest::load_manifest;
use agent_backends::template::{expand_command, TemplateContext};
use agent_protocol::{
    AdapterId, AdapterManifest, Confidence, ConversationId, ConversationRecord, HostEvent,
    HostInstanceId, HostRequest, NativeConversationKey, ObservationSource, OrganizationState,
    PaneId, ProcessOwnerId, ProfileId, RuntimeSnapshot, RuntimeState, RESERVED_AGENT_EVENT_USER_VAR,
};
use chrono::Utc;
use lucidity_ui::{SettingsDraft, UsageView};
use parking_lot::Mutex;
#[cfg(windows)]
use portable_pty::cmdbuilder::WindowsProcessTreePolicy;
use portable_pty::CommandBuilder;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use uuid::Uuid;

#[cfg(windows)]
use crate::windows::{TrayCallbacks, TrayController as NativeTrayController};

/// Idle wait uses a long timeout; wake channels drive work. Not a 25ms spin.
const HOST_IDLE_WAIT: Duration = Duration::from_millis(250);
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
    last_ui_event_revision: AtomicU64,
    pump: Mutex<Option<JoinHandle<()>>>,
    bridge: Arc<UiBridge>,
    settings: Mutex<SettingsDraft>,
    usage: Mutex<UsageView>,
    launch_error: Mutex<Option<String>>,
    status_message: Mutex<Option<String>>,
    snapshot_revision: AtomicU64,
    demo_mode: bool,
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

        let settings = load_settings(&settings_path());
        let bridge = Arc::new(UiBridge::new());
        let demo_mode = demo_mode_enabled();

        Ok(Arc::new(Self {
            host,
            adapters,
            ingress,
            launching: Mutex::new(HashSet::new()),
            selected: Mutex::new(None),
            shutdown_requested: AtomicBool::new(false),
            window_hidden: AtomicBool::new(false),
            last_ui_event_revision: AtomicU64::new(0),
            pump: Mutex::new(None),
            bridge,
            settings: Mutex::new(settings),
            usage: Mutex::new(UsageView::default()),
            launch_error: Mutex::new(None),
            status_message: Mutex::new(None),
            snapshot_revision: AtomicU64::new(0),
            demo_mode,
            #[cfg(windows)]
            tray: Mutex::new(None),
            #[cfg(windows)]
            host_service: Mutex::new(Some(host_service)),
        }))
    }

    pub(super) fn gui_hooks(self: &Arc<Self>) -> wezterm_gui::ProductGuiHooks {
        let ready = Arc::downgrade(self);
        let hidden = Arc::downgrade(self);
        let bridge = Arc::clone(&self.bridge);
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
            ui_factory: Some(Arc::new(move || {
                Box::new(LucidityUiController::new(Arc::clone(&bridge)))
            })),
        }
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

        // Mock auto-seed is demo-only. Ordinary product launch never creates mock rows.
        if self.demo_mode {
            self.ensure_mock_runtime();
            *self.status_message.lock() = Some("Demo mode: mock agent enabled".to_owned());
        }

        self.publish_ui_snapshot();
        self.start_action_pump();
    }

    fn ensure_mock_runtime(&self) {
        let mut host = self.host.lock();
        if !self
            .adapters
            .contains_key(&AdapterId::from(MOCK_ADAPTER_ID))
        {
            return;
        }
        let existing = match host.store().list_conversations(None, 200) {
            Ok(page) => page.conversations.into_iter().find(|conversation| {
                conversation.native.adapter_id.0 == MOCK_ADAPTER_ID
                    && conversation.organization_state == OrganizationState::Active
            }),
            Err(error) => {
                log::error!("failed to inspect the catalog for a mock Agent session: {error:#}");
                return;
            }
        };
        let request = match existing {
            Some(conversation) => HostRequest::ConversationOpen {
                conversation_id: conversation.id,
            },
            None => HostRequest::ConversationNew {
                adapter_id: AdapterId::from(MOCK_ADAPTER_ID),
                profile_id: ProfileId(Uuid::new_v4()),
                project_path: std::env::current_dir().ok(),
            },
        };
        if let Err(error) = host.handle_host_request(request) {
            log::error!("failed to ensure the mock Agent session: {error:#}");
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
                match application.bridge.wait_for_work(HOST_IDLE_WAIT) {
                    BridgeWake::Shutdown => break,
                    BridgeWake::Command(command) => {
                        application.handle_ui_command(command);
                        application.pump_host_actions();
                        application.publish_ui_snapshot();
                    }
                    BridgeWake::StateChanged | BridgeWake::Timeout => {
                        application.pump_host_actions();
                        // Only republish when host event revision advanced.
                        let revision = application.host.lock().event_revision();
                        if application
                            .last_ui_event_revision
                            .load(Ordering::Acquire)
                            != revision
                        {
                            application.publish_ui_snapshot();
                        }
                    }
                }
            }) {
            Ok(thread) => {
                self.pump.lock().replace(thread);
            }
            Err(error) => log::error!("failed to start product action pump: {error:#}"),
        }
    }

    fn publish_ui_snapshot(&self) {
        let selected = *self.selected.lock();
        let host = self.host.lock();
        let conversations = match host.store().list_conversations(None, 500) {
            Ok(page) => page.conversations,
            Err(error) => {
                log::error!("failed to list conversations for UI snapshot: {error:#}");
                return;
            }
        };
        let mut runtime = HashMap::new();
        let mut attachments = HashMap::new();
        for conversation in &conversations {
            if let Ok(Some(snap)) = host.store().runtime_snapshot(conversation.id) {
                runtime.insert(conversation.id, snap.state);
            }
            let attached = host
                .store()
                .current_attachment(conversation.id)
                .ok()
                .flatten()
                .is_some();
            attachments.insert(conversation.id, attached);
        }
        let adapters: Vec<AdapterManifest> = self.adapters.values().cloned().collect();
        let revision = self.snapshot_revision.fetch_add(1, Ordering::AcqRel) + 1;
        let settings = self.settings.lock().clone();
        let usage = self.usage.lock().clone();
        let launch_error = self.launch_error.lock().clone();
        let status_message = self.status_message.lock().clone();
        drop(host);

        let snapshot = build_ui_snapshot(
            revision,
            PRODUCT_VERSION,
            self.demo_mode,
            &conversations,
            &runtime,
            &attachments,
            &adapters,
            selected,
            usage,
            settings,
            status_message,
            launch_error,
        );
        self.bridge.publish(snapshot);
        wezterm_gui::request_product_redraw();
    }

    fn handle_ui_command(self: &Arc<Self>, command: UiCommand) {
        match command {
            UiCommand::NewConversation {
                adapter_id,
                project_path,
                title,
                extra_args,
            } => {
                let adapter_id = AdapterId::from(adapter_id.as_str());
                if adapter_id.0 == MOCK_ADAPTER_ID && !self.demo_mode {
                    *self.launch_error.lock() =
                        Some("Mock agent is demo-only. Launch with --demo to enable.".into());
                    return;
                }
                let project = PathBuf::from(project_path);
                let result = {
                    let mut host = self.host.lock();
                    host.handle_host_request(HostRequest::ConversationNew {
                        adapter_id: adapter_id.clone(),
                        profile_id: ProfileId(Uuid::new_v4()),
                        project_path: Some(project.clone()),
                    })
                };
                match result {
                    Ok(agent_protocol::HostResult::Conversation(Some(mut record))) => {
                        if let Some(title) = title {
                            let _ = self.rename_conversation(record.id, title);
                            if let Ok(Some(updated)) =
                                self.host.lock().store().get_conversation(record.id)
                            {
                                record = updated;
                            }
                        }
                        if let Some(extra) = extra_args {
                            let mut settings = self.settings.lock();
                            if let Some(entry) = settings
                                .agent_extra_args
                                .iter_mut()
                                .find(|(id, _)| id == &adapter_id.0)
                            {
                                entry.1 = extra;
                            } else {
                                settings
                                    .agent_extra_args
                                    .push((adapter_id.0.clone(), extra));
                            }
                        }
                        self.selected.lock().replace(record.id);
                        *self.launch_error.lock() = None;
                    }
                    Ok(_) => {}
                    Err(error) => {
                        *self.launch_error.lock() = Some(format!("Failed to create session: {error:#}"));
                    }
                }
            }
            UiCommand::OpenConversation(id) | UiCommand::Resume(id) => {
                let conversation_id = ConversationId(id);
                self.selected.lock().replace(conversation_id);
                let result = {
                    let mut host = self.host.lock();
                    host.handle_host_request(HostRequest::ConversationOpen { conversation_id })
                };
                if let Err(error) = result {
                    *self.launch_error.lock() = Some(format!("Open failed: {error:#}"));
                }
            }
            UiCommand::Settle(id) => {
                let conversation_id = ConversationId(id);
                let _ = self.host.lock().handle_host_request(
                    HostRequest::ConversationSetOrganizationState {
                        conversation_id,
                        organization_state: OrganizationState::Settled,
                    },
                );
            }
            UiCommand::Unsettle(id) => {
                let conversation_id = ConversationId(id);
                let _ = self.host.lock().handle_host_request(
                    HostRequest::ConversationSetOrganizationState {
                        conversation_id,
                        organization_state: OrganizationState::Active,
                    },
                );
            }
            UiCommand::Stop(id) => {
                let conversation_id = ConversationId(id);
                let _ = self
                    .host
                    .lock()
                    .handle_host_request(HostRequest::ConversationStop { conversation_id });
            }
            UiCommand::Rename { id, title } => {
                let _ = self.rename_conversation(ConversationId(id), title);
            }
            UiCommand::RefreshUsage(id) => {
                let conversation_id = ConversationId(id);
                let _ = self.host.lock().handle_host_request(
                    HostRequest::ConversationRefreshUsage { conversation_id },
                );
                // Surface dual usage panels from last stored snapshot if any.
                let title = self
                    .host
                    .lock()
                    .store()
                    .get_conversation(conversation_id)
                    .ok()
                    .flatten()
                    .map(|c| c.title);
                // Stored usage is provider-specific JSON; map through usage module.
                let json = serde_json::json!({"status": "refresh_requested"});
                *self.usage.lock() =
                    usage_view_from_snapshot(Some(&json), title.as_deref());
            }
            UiCommand::ApplySettings(draft) => {
                if let Err(error) = save_settings(&settings_path(), &draft) {
                    *self.launch_error.lock() =
                        Some(format!("Failed to save settings: {error}"));
                } else {
                    *self.settings.lock() = draft;
                    *self.status_message.lock() = Some("Settings saved".into());
                }
            }
            UiCommand::PickProjectFolder => {
                // Handled inside the UI controller on the GUI thread.
            }
            UiCommand::ImportHistories => {
                self.import_histories();
            }
            UiCommand::Quit => {
                self.request_quit();
            }
        }
    }

    fn rename_conversation(&self, id: ConversationId, title: String) -> anyhow::Result<()> {
        let mut host = self.host.lock();
        let Some(existing) = host.store().get_conversation(id)? else {
            return Ok(());
        };
        host.store_mut().upsert_conversation(ConversationUpsert {
            id: Some(existing.id),
            native: existing.native,
            native_session_path: existing.native_session_path,
            title: existing.title,
            user_alias: Some(title),
            project_path: existing.project_path,
            created_at: existing.created_at,
            last_activity_at: Utc::now(),
            organization_state: existing.organization_state,
        })?;
        Ok(())
    }

    fn import_histories(&self) {
        let home = dirs_home();
        let imported = import_all_known(&home);
        if imported.is_empty() {
            *self.status_message.lock() = Some(
                "No Claude/Codex histories found under the user profile (or paths missing)."
                    .into(),
            );
            return;
        }
        let mut host = self.host.lock();
        let mut count = 0usize;
        for item in imported {
            let profile = ProfileId(Uuid::new_v4());
            let record = imported_to_record(&item, profile);
            // Avoid PTY: catalog-only upsert via ConversationNew path is wrong
            // because it queues LaunchNewRuntime. Use store upsert directly.
            let existing = host
                .store()
                .list_conversations(None, 10_000)
                .ok()
                .map(|page| {
                    page.conversations.iter().any(|c| {
                        c.native.adapter_id == record.native.adapter_id
                            && c.native.native_session_id == record.native.native_session_id
                    })
                })
                .unwrap_or(false);
            if existing {
                continue;
            }
            let _ = host.store_mut().ensure_profile(
                &record.native.adapter_id,
                record.native.profile_id,
                None,
                record.created_at,
            );
            if host
                .store_mut()
                .upsert_conversation(ConversationUpsert {
                    id: None,
                    native: NativeConversationKey {
                        adapter_id: record.native.adapter_id.clone(),
                        profile_id: record.native.profile_id,
                        native_session_id: record.native.native_session_id.clone(),
                    },
                    native_session_path: record.native_session_path.clone(),
                    title: record.title.clone(),
                    user_alias: None,
                    project_path: record.project_path.clone(),
                    created_at: record.created_at,
                    last_activity_at: record.last_activity_at,
                    organization_state: OrganizationState::Settled,
                })
                .is_ok()
            {
                count += 1;
            }
        }
        drop(host);
        *self.status_message.lock() = Some(format!("Imported {count} history conversation(s)"));
    }

    fn pump_host_actions(self: &Arc<Self>) {
        while let Some(event) = self.ingress.try_recv() {
            let selected = *self.selected.lock();
            let selection_update = {
                let mut host = self.host.lock();
                match reduce_queued_event(host.store_mut(), event) {
                    Ok(EventReduction::Applied {
                        conversation_id,
                        events,
                    }) => {
                        let selection_update =
                            selected_rebind_target(selected, conversation_id, &events);
                        for event in events {
                            host.push_event(event);
                        }
                        selection_update
                    }
                    Ok(EventReduction::Duplicate) => None,
                    Ok(EventReduction::Quarantined { reason })
                    | Ok(EventReduction::Rejected { reason }) => {
                        log::warn!("Agent event ignored: {reason}");
                        None
                    }
                    Err(error) => {
                        log::error!("failed to reduce Agent event: {error:#}");
                        None
                    }
                }
            };
            if let Some(conversation_id) = selection_update {
                self.selected.lock().replace(conversation_id);
            }
        }

        let (actions, event_revision) = {
            let mut host = self.host.lock();
            let actions = host
                .take_open_decisions()
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
                .collect::<Vec<_>>();
            (actions, host.event_revision())
        };

        if self
            .last_ui_event_revision
            .swap(event_revision, Ordering::AcqRel)
            != event_revision
        {
            wezterm_gui::request_product_redraw();
        }

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
        let settings = self.settings.lock();
        let executable = executable_override_for(&settings, &record.native.adapter_id.0)
            .unwrap_or_else(|| resolve_executable(manifest, &expanded.executable));
        if !executable.exists() && record.native.adapter_id.0 != MOCK_ADAPTER_ID {
            anyhow::bail!(
                "Executable not found for {} (looked for {}). Configure the path in Settings → Agents.",
                record.native.adapter_id,
                executable.display()
            );
        }
        let mut args = expanded.args;
        args.extend(extra_args_for(&settings, &record.native.adapter_id.0));
        drop(settings);
        let mut command = CommandBuilder::new(executable);
        command.args(args);
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
                self.replay_latest_agent_event(pane_id);
                host.push_event(HostEvent::RuntimeAttached { attachment });
                host.push_event(HostEvent::ConversationChanged { conversation_id });
            }
            Err(error) => log::error!("failed to attach pane to {conversation_id}: {error:#}"),
        }
    }

    fn record_launch_failure(&self, conversation_id: ConversationId, message: String) {
        *self.launch_error.lock() = Some(message.clone());
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

    fn replay_latest_agent_event(&self, pane_id: PaneId) {
        let Some(mux) = mux::Mux::try_get() else {
            return;
        };
        let Some(pane) = mux.get_pane(pane_id.0 as usize) else {
            return;
        };
        let Some(value) = pane
            .copy_user_vars()
            .get(RESERVED_AGENT_EVENT_USER_VAR)
            .cloned()
        else {
            return;
        };
        let _ = self.ingress.enqueue_user_var(
            pane_id,
            RESERVED_AGENT_EVENT_USER_VAR,
            &value,
            Utc::now(),
        );
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
        self.bridge.request_shutdown();
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

fn selected_rebind_target(
    selected: Option<ConversationId>,
    reduced_conversation_id: ConversationId,
    events: &[HostEvent],
) -> Option<ConversationId> {
    let selected_was_detached = events.iter().any(|event| {
        matches!(
            event,
            HostEvent::RuntimeDetached {
                conversation_id,
                ..
            } if Some(*conversation_id) == selected
        )
    });
    let reduced_was_attached = events.iter().any(|event| {
        matches!(
            event,
            HostEvent::RuntimeAttached { attachment }
                if attachment.conversation_id == reduced_conversation_id
        )
    });
    (selected_was_detached && reduced_was_attached).then_some(reduced_conversation_id)
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

fn dirs_home() -> PathBuf {
    if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        return PathBuf::from(home);
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
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

    #[test]
    fn claude_launch_includes_skip_permissions() {
        let adapters = load_bundled_adapters().unwrap();
        let claude = adapters.iter().find(|a| a.id.0 == "claude").unwrap();
        assert!(claude
            .launch
            .args
            .iter()
            .any(|a| a == "--dangerously-skip-permissions"));
    }

    #[test]
    fn kimi_launch_includes_yolo() {
        let adapters = load_bundled_adapters().unwrap();
        let kimi = adapters.iter().find(|a| a.id.0 == "kimi").unwrap();
        assert!(kimi.launch.args.iter().any(|a| a == "-yolo"));
    }

    #[test]
    fn demo_mode_is_opt_in_only() {
        // Ordinary process args in tests do not include --demo.
        assert!(!std::env::args().any(|a| a == "--demo"));
    }
}
