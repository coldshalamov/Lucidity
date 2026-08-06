# Provisional Interfaces and Ownership Boundaries

Status: technical Wave 0 contracts and the controller-adjudicated AT-093/094 native design are accepted. Any implementation-time change requires a controller ADR before dependent work continues.

## Repository topology

This checkout already is the WezTerm repository root. The accepted additive topology is:

```text
Lucidity/
├── agent-protocol/        # dependency-light domain and wire contracts
├── agent-backends/        # adapter registry, events, history/usage providers, mock binary
├── agent-terminal/        # native host, catalog, lifecycle, chrome, settings, CLI bins
├── wezterm-gui/           # existing package; add a library seam while preserving stock bin
├── window/                # existing native window package
├── mux/                   # existing pane/tab/process model
└── extensions/vscode/     # thin TypeScript client
```

Do not move existing packages beneath `upstream/`, `crates/`, or `apps/`. Do not create a crate per feature before a real dependency, compile-time, or ownership boundary exists.

Responsibility boundaries:

- `agent-protocol`: IDs, domain enums, serializable views, IPC requests/responses/events, adapter manifest schema types, and OSC envelope types. No GUI, SQLite, process, or provider dependencies.
- `agent-backends`: adapter discovery, manifest validation, command construction, history import, event decoding/reduction, usage providers, fixtures, and deterministic mock harness target. No WezTerm renderer ownership and no direct UI state.
- `agent-terminal`: catalog persistence, host authority, runtime attachments, tray lifecycle, local IPC server, pure sidebar/query view models, native application chrome, settings surfaces, and user-facing binaries.
- `wezterm-gui`: reusable stock GUI bootstrap and the single-owned terminal viewport seam. Stock WezTerm remains a thin binary over the same library.
- `extensions/vscode`: one client of the host protocol; owns no lifecycle or persistence truth.

If implementation evidence proves that SQLite, tray lifetime, or pure sidebar logic require separate packages for independent compilation/testing, split only those proven seams through an ADR.

## Domain identity

```rust
pub struct ConversationId(pub Uuid);
pub struct AdapterId(pub String);
pub struct ProfileId(pub Uuid);
pub struct HostInstanceId(pub Uuid);

pub struct NativeConversationKey {
    pub adapter_id: AdapterId,
    pub profile_id: ProfileId,
    pub native_session_id: String,
}
```

`NativeConversationKey` is unique. An alias or path is not identity.

```rust
pub enum OrganizationState {
    Active,
    Settled,
}

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
```

Organization transitions and runtime transitions are different commands and different reducer paths. No shared helper may make one imply the other.

```rust
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

pub struct RuntimeAttachment {
    pub conversation_id: ConversationId,
    pub pane_id: PaneId,
    pub process_id: Option<u32>,
    pub process_owner: Option<ProcessOwnerId>,
    pub host_instance_id: HostInstanceId,
    pub attachment_generation: u64, // monotonic within this host instance
    pub attached_at: DateTime<Utc>,
}

pub enum ProcessOwnerId {
    WindowsJob { opaque_id: String },
    DirectChild { pid: u32 },
}

pub struct RuntimeSnapshot {
    pub conversation_id: ConversationId,
    pub state: RuntimeState,
    pub source: ObservationSource,
    pub confidence: Confidence,
    pub observed_at: DateTime<Utc>,
    pub last_error: Option<String>,
}
```

The database may persist a labeled last-known runtime observation for recovery, but the host must never present it as current liveness without current evidence. On every new `HostInstanceId`, all persisted live attachments are marked stale and their current state becomes `NotRunning` until current-process evidence creates a new attachment. Pane IDs and generations from a prior host process are never reused as current truth.

## Native identity transition

Authoritative evidence order:

1. Versioned structured event containing the new native ID.
2. Provider structured API event.
3. Session-store change correlated to the owned attachment/process.
4. Submitted identity-changing command candidate plus store confirmation.
5. Bounded screen observation.
6. User binding choice when still ambiguous.

Candidate input does not rebind immediately. Each `(host_instance_id, pane_id)` has at most one pending candidate:

```rust
pub struct PendingIdentity {
    pub host_instance_id: HostInstanceId,
    pub pane_id: PaneId,
    pub candidate_native_session_id: String,
    pub evidence_id: EvidenceId,
    pub observed_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}
```

The default confirmation window is 30 seconds using an injected monotonic clock in tests. A newer candidate supersedes the older candidate. Expiry clears the candidate and leaves the current binding unchanged. A process crash clears the candidate and applies the observed exit to the current conversation. Structured or store-backed native-ID evidence may create and confirm a candidate atomically; submitted text alone never confirms it. Stable evidence IDs are mandatory: structured events use `(adapter_id, profile_id, event_id)`, while a store scanner derives an ID from the adapter/profile/source record key and content hash.

The confirmed reducer operation is atomic from clients' perspective:

```text
upsert conversation B
detach pane P from conversation A at generation N
attach pane P to B at generation N+1
publish conversation/runtime/catalog changes
```

On confirmed A→B rebinding, A is detached with reason `NativeIdentitySuperseded` and its current runtime becomes `NotRunning`; its last observation remains historical. B inherits the exact pane, PID/process owner, and receives generation N+1. Duplicate, stale-generation, superseded-candidate, or out-of-order evidence is idempotent or rejected deterministically. An attachment version is `(host_instance_id, attachment_generation)`; a counter from a prior host never orders a current attachment. Rebind evidence matches the emitting `(host_instance_id, pane_id, native_session_id)`. After pane P is rebound from A to B, a late `session_end(A)` may update A's historical record but must never detach or change B. Tests cover hint→crash, hint B→hint C→confirm C, confirm-without-hint, duplicate cross-source evidence, timeout, and late old-ID end.

## Agent event envelope

Transport is frozen by ADR-002: OSC 1337 `SetUserVar` with reserved name `lucidity.agent-event.v1` and a base64-encoded UTF-8 JSON value. The current parser decodes the value and publishes `Alert::SetUserVar` with the emitting pane; the Agent host consumes that existing mux notification. `MAX_AGENT_EVENT_JSON_BYTES = 32_768` decoded bytes and `MAX_PENDING_AGENT_EVENTS = 256`.

Minimum envelope fields:

```json
{
  "v": 1,
  "eventId": "adapter-scoped-dedupe-id",
  "adapter": "claude-code",
  "profileId": "uuid",
  "event": "session_start",
  "sessionId": "native-id",
  "cwd": "C:\\work\\repo",
  "transcriptPath": null,
  "occurredAt": "2026-08-06T00:00:00Z",
  "data": {}
}
```

Core events: `session_start`, `prompt_submit`, `tool_complete`, `turn_complete`, `turn_failed`, `permission_request`, `permission_replied`, `question_asked`, `idle_prompt`, `context_updated`, `usage_updated`, and `session_end`.

The mux callback performs only reserved-name, size, ownership, and non-blocking enqueue checks. JSON parsing, dedupe, store correlation, and persistence run off the GUI/mux callback thread. Queue overflow marks the attachment `event_resync_required` and schedules bounded adapter/store reconciliation; terminal output is never blocked. The decoder is version-rejecting, UTF-8-validating, and tolerant of duplicate delivery. Unknown future event variants are quarantined/logged without panicking the terminal or host. The reserved value is untrusted evidence and is accepted only from a currently owned, adapter-compatible pane.

OSC events do not carry trusted attachment generations. The host associates an event with the pane and current host instance that delivered it, then applies the native-ID rebind rules above. Text submitted to the TUI remains candidate evidence only. Usage fixtures emit `usage_updated`; the host never injects a hidden `/usage` command into a live TUI.

## Catalog order, recovery, and exit truth

Conversation pages use descending keyset order `(last_activity_at, conversation_id)` and a `catalog_snapshot_version`. A cursor is valid only for the snapshot version that issued it; a changed version produces `resync_required`, not duplicates or skipped rows. UI activation keys are durable `ConversationId` and live `TabId`, never positional row/tab indexes.

The catalog is single-writer. Before a destructive migration or corruption recovery, the host closes the database and creates a uniquely named, never-overwritten quarantine backup. It then creates a fresh schema and deterministically reimports adapter histories, surfacing a recovery warning for non-reconstructable aliases/organization state. No corrupt file is silently deleted.

The mux/PTY seam publishes one typed process-exit observation—exit code plus whether Lucidity requested termination—before `PaneRemoved`. The host maps clean natural exit to `CompletedIdle`, non-zero/unexpected exit to `Failed`, and explicit Stop/Quit to `NotRunning` with its termination reason. A bare pane-removal event is never sufficient to invent success.

## Adapter package contract

Default community adapters are declarative packages:

```text
<id>.agent-adapter/
├── adapter.toml
├── settings.schema.json
├── icon.svg
├── fixtures/
└── hooks/                 # optional, separately reviewed and user-approved
```

The manifest may describe executable discovery, version probes, direct executable-plus-argument launch/resume templates, history sources, settings bindings, event capabilities, usage/context providers, and fixtures. Shell interpolation is off by default. Executable hooks require explicit user approval, visible changes, backups, rollback, and bounded destinations.

## Local control protocol

Transport is a same-user Windows named pipe with little-endian `u32` length-prefixed UTF-8 JSON messages. Frozen constants are `MAX_FRAME_BYTES = 1_048_576` and `MAX_PENDING_EVENTS_PER_SUBSCRIBER = 256`. A loopback transport may be added only if the explicit Node extension-host platform spike proves it necessary.

On Windows the pipe name is versioned and derived from a hash of the current owner SID; its name is not a security boundary. The server sets `PIPE_REJECT_REMOTE_CLIENTS` and an explicit DACL permitting only the current owner SID plus the minimum required system identity. After connection it obtains the client PID, opens the client token, and compares its user SID to the owner before accepting a handshake. The default inherited DACL or an unverified pipe name is not acceptance evidence. Slow subscribers use a bounded queue; overflow closes or marks the stream `resync_required`, after which the client performs `host.hello` and fetches a complete paginated snapshot. Push events are edge hints, not a durable journal; replay across disconnects is not promised for the MVP.

Handshake:

```text
host.hello(client_name, min_version, max_version)
  -> negotiated_version, host_instance_id, capabilities
```

Minimum request families:

- `host.status`, `host.openUi`, `host.quit`
- `conversation.list`, `conversation.get`, `conversation.new`, `conversation.open`
- `conversation.setOrganizationState`, `conversation.stop`
- `conversation.refreshUsage`
- `adapter.list`, `adapter.scan`
- `event.subscribe`

Minimum push events:

- `catalog.changed`
- `conversation.changed`
- `runtime.attached`, `runtime.detached`
- `usage.changed`
- `adapter.changed`

The Cursor/VS Code MVP does not attach to terminal byte streams. It asks the desktop host to open/focus the native terminal.

Every reconnect performs a new handshake. If `host_instance_id` changed—or event continuity is otherwise unknown—the client discards incremental assumptions and refetches the full snapshot. Named-pipe feasibility from the Node extension host remains a platform test, not a frozen fact.

## GUI/library seam

The existing `wezterm-gui` package is binary-only. The accepted sequence is:

1. Add `wezterm-gui/src/lib.rs` exposing the minimum bootstrap, `GuiFrontEnd`, and `TermWindow` modules.
2. Keep `wezterm-gui/src/main.rs` as a thin stock wrapper.
3. Add a separate `agent-terminal` package/binary using the library with distinct application chrome and Windows resources.

Cargo auto-discovers a library target from `src/lib.rs`; no `[lib]` stanza is required. AT-101 creates the buildable library source and moves the reusable bootstrap behind it without editing `wezterm-gui/Cargo.toml`. AT-099 owns only workspace/package skeletons, frozen protocol types, module roots, dependencies, and product resource scaffolding.

The product binary has an explicit Windows resource contract: `windows_subsystem="windows"`, its own icon/resource ID compatible with the current `window` lookup, a PerMonitorV2/UTF-8 manifest, version data assigned before argument parsing, required ConPTY/ANGLE/Mesa provisioning with freshness/hash checks, and a distinct AppUserModelID/window class/title/application identity. Bootstrap and main-thread designation are explicit once-only calls. Agent mode hard-disables the upstream update checker; Gate 1 measures network activity as well as inspecting PE/process behavior.

Geometry must be explicit:

```rust
pub struct AppLayout {
    pub window: Rect,
    pub header: Rect,
    pub sidebar: Rect,
    pub terminal: Rect,
    pub stock_tab_bar: Option<Rect>,
    pub maximize_button: Option<Rect>,
    pub overlay: Option<Rect>,
}
```

One computed layout must drive initial dimensions, resize/PTY dimensions, Windows resize increments, render bounds/origins, chrome and terminal hit routing, mouse-to-cell conversion, selection, hyperlinks, scrollbar, cursor/IME, Win11 maximize-button/non-client hit geometry and snap-layout hover, maximize/restore, and DPI changes. Padding inflation is explicitly forbidden as the sidebar mechanism.

Stock mode must calculate the same terminal geometry as stock-derived numeric oracles at the pinned base. Agent mode may reserve chrome. Cursor/IME and mouse mapping share border/origin inputs. Sidebar actions store `TabId` and resolve its current position at dispatch; a stale positional tab index is forbidden. Suppressing the stock tab bar happens only after real OS-input sidebar activation is verified.

## Windows process-tree policy

ADR-003 freezes two spawn policies. Stock commands use `DirectChild`. Agent-owned commands use `OwnedJob`: create the process suspended, assign it to a Job Object configured with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, and resume only after successful assignment. The job handle lives with the PTY child/killer; Stop and Quit terminate the job, hide preserves it, and host-process death closes the final handle. Assignment/nested-job/breakaway failure aborts launch rather than falling back to an unowned tree.

## Windows close and lifecycle seam

```rust
pub enum CloseIntent {
    StockClose,
    HideToTray,
    QuitOwned,
}
```

`CloseIntent` is an internal `agent-terminal` lifecycle type, not an `agent-protocol` dependency of stock `wezterm-gui`. ADR-004 requires a product-agnostic GUI close seam and an explicit mapping at integration. The intent is selected by product mode and explicit user command, not inferred from mux emptiness. Agent mode explicitly sets `quit_when_all_windows_are_closed = false` and an exit behavior that does not prune a live agent pane. `HideToTray` maps to GUI hide-preserving-mux behavior and marks the GUI mapping deliberately hidden so `reconcile_workspace` cannot recreate it. `QuitOwned` is the only tray/app quit path and delegates exact Job Object termination to the host before explicit GUI close. `StockClose` maps to stock WezTerm behavior. The native window layer exposes distinct true hide, show/focus, explicit close, and minimize operations; minimize is not a `CloseIntent` and is never implemented through hide.

For the single-process protected demo, a UI-crash receipt means loss/destruction of the native window or render context while the process/host loop survives. An unhandled process crash is a host crash; it must invalidate attachments and close Agent-owned jobs. No independent UI-process fault isolation is claimed.

## Shared-file mutex

Only the integration owner may change and regenerate:

- `Cargo.toml`
- `Cargo.lock`
- `wezterm-gui/Cargo.toml`
- `wezterm-gui/build.rs`
- `pty/Cargo.toml`
- `.cargo/config.toml`
- Windows packaging/workflow files and the root `Makefile`

Single GUI owner:

- `wezterm-gui/src/main.rs`
- `wezterm-gui/src/lib.rs`
- `wezterm-gui/src/frontend.rs`
- `wezterm-gui/src/termwindow/**`
- `wezterm-gui/src/tabbar.rs`
- `wezterm-gui/src/resize_increment_calculator.rs`

Native lifecycle changes in `window/src/lib.rs`, `window/src/os/windows/**`, `frontend.rs`, and the close-policy portion of `termwindow/mod.rs` are exclusively leased to AT-105 before viewport chrome. AT-101 starts only after AT-105 releases those paths. AT-108 later re-leases the smallest tray-binding subset after host/Job APIs exist. AT-106 exclusively owns typed mux exit publication; AT-107 exclusively owns the Windows PTY/Job Object seam. `mux/**` otherwise stays unchanged.

AT-099 owns one advance declaration in `pty/Cargo.toml`: enable the already-pinned WinAPI `jobapi2` module needed by ADR-003. AT-107 remains manifest-free and owns all Job Object behavior. No other task may opportunistically change the PTY manifest.

Per ADR-004, AT-101 receives a later serialized sublease for `agent-terminal/Cargo.toml` and the mechanical `Cargo.lock` delta only to add the existing `wezterm-gui` path dependency after the real library target exists. This happens after AT-102/103 and the native foundations are integrated. It may not add a package, external dependency, or unrelated lockfile churn.
