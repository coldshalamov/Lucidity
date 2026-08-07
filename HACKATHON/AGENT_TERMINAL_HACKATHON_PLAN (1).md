# Agent Terminal — Windows-Native Architecture and Hackathon Build Plan

**Status:** Architecture baseline for the Friday, August 7, 2026 Cursor Hackathon  
**Working title:** Agent Terminal  
**License target:** MIT  
**Primary platform:** Native Windows, no WSL dependency  
**Terminal foundation:** WezTerm fork  
**Product boundary:** Native coding-agent TUIs remain the chat interface. The app adds session organization, durable recovery, settings, usage visibility, and cross-harness navigation.

---

## 1. Product definition

Agent Terminal is not a new agent harness and not a replacement chat renderer.

It is a Windows-native terminal built specifically for coding-agent workflows:

- It launches and renders the normal Claude Code, Codex, Kimi, OpenCode, Grok, Cline, Cursor CLI, and other native TUIs.
- It presents every configured harness's native conversations in one unified sidebar.
- It makes conversation identity, liveness, history, settings, usage, and recovery visible without requiring users to memorize session names, timestamps, or slash-command behavior.
- It keeps live sessions running when the main window is closed by remaining resident in the Windows system tray.
- It provides a modular adapter system so support for another harness can be shared as a small package.
- It exposes one local control plane that the desktop UI, CLI, Cursor/VS Code extension, and later phone client all use.

The shortest accurate description is:

> **WezTerm, reshaped into a session operating system for native coding-agent TUIs.**

The product should feel like one coding environment with many engines, rather than seven unrelated terminal applications with seven unrelated session stores.

---

## 2. Non-goals for the first build

Do not build these into the hackathon MVP:

- A custom chat renderer.
- A replacement agent protocol.
- A new model subscription.
- Cross-agent orchestration.
- Automatic merge resolution.
- Cloud accounts or a production relay.
- WSL, tmux, SSH, or remote execution.
- Full support for every harness.
- A general-purpose terminal competitor to Warp.
- A complete graphical editor for every possible WezTerm option.
- Arbitrary executable community adapter plugins.
- Transcript duplication into the app database.

Those are all plausible later capabilities. They are also excellent ways to bury the core product before it breathes.

---

## 3. Foundational decisions

### 3.1 Native Windows only

The first release uses Windows PTYs through WezTerm's existing Windows terminal stack. It must not require WSL.

WezTerm's own multiplexer provides the tmux-like concept:

- multiple persistent terminal panes;
- detach and reattach;
- one native Windows process supervising sessions;
- no Unix compatibility layer.

The app may support WSL later as an optional execution target, never as a prerequisite.

### 3.2 Native TUI remains the conversation view

The terminal pane is the actual harness:

```text
ConPTY
└── claude.exe / codex.exe / opencode.exe / kimi.exe / ...
```

The app does not translate the harness into its own chat UI. Native slash commands, permissions, keyboard shortcuts, plugins, subagent views, and new harness features continue to work immediately.

### 3.3 Close-to-tray

Closing the last visible window:

1. hides or destroys the UI window;
2. leaves the tray host alive;
3. leaves application-owned agent processes and WezTerm mux panes alive;
4. allows the window to be reconstructed and reattached later.

Exiting from the tray is the actual application quit operation.

Recommended quit choices:

```text
Stop running agents and quit
Cancel
```

"Quit but leave agents running" is not useful unless another daemon remains to own them. The tray host is that daemon, so quitting it should be explicit and unsurprising.

### 3.4 Settled is purely organizational

The app must maintain two independent state axes:

```rust
enum OrganizationState {
    Active,
    Settled,
}

enum RuntimeState {
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

Invariants:

- Settling never stops, interrupts, pauses, or detaches an agent.
- A settled conversation may still be working.
- An active conversation may be stopped.
- Imported historical conversations default to `Settled + NotRunning`.
- A conversation newly created or opened in the app defaults to `Active`.
- New runtime activity may optionally surface a settled row through an attention badge, but it must not silently change the user's organization choice unless that behavior is explicitly enabled.

### 3.5 Catalog records are not live terminals

Separate durable native conversation identity from temporary process attachment:

```rust
struct NativeConversation {
    id: ConversationId,
    adapter_id: AdapterId,
    profile_id: ProfileId,
    native_session_id: String,
    native_session_path: Option<PathBuf>,

    title: String,
    user_alias: Option<String>,
    project_path: Option<PathBuf>,
    created_at: DateTime<Utc>,
    last_activity_at: DateTime<Utc>,

    organization_state: OrganizationState,
    runtime_state: RuntimeState,

    latest_prompt_preview: Option<String>,
    summary_preview: Option<String>,
}

struct RuntimeAttachment {
    conversation_id: ConversationId,
    pane_id: PaneId,
    process_id: Option<u32>,
    process_group_id: Option<String>,
    attached_at: DateTime<Utc>,
}
```

Consequences:

- Ten thousand imported conversations are cheap database rows.
- Only live conversations consume PTYs, scrollback buffers, and GPU state.
- Clicking a live row activates its existing pane.
- Clicking a historical row launches the harness's native resume operation and creates a new runtime attachment.
- Closing a pane does not erase the conversation record.
- The native harness remains the transcript source of truth.

---

## 4. Process and crate architecture

### 4.1 Target physical architecture

```text
┌──────────────────────────────────────────────────────────────────┐
│ agent-terminal.exe                                               │
│                                                                  │
│  Windows tray host                                               │
│  ├── local control server / named-pipe endpoint                  │
│  ├── SQLite catalog                                              │
│  ├── adapter registry                                            │
│  ├── session-store watchers                                      │
│  ├── usage providers                                             │
│  ├── WezTerm mux/process ownership                               │
│  └── zero or more UI windows                                     │
│       ├── native sidebar/header/settings                         │
│       └── native WezTerm terminal viewport                       │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
             ▲                           ▲
             │ local authenticated IPC   │
             │                           │
      agent-terminal CLI          Cursor/VS Code extension
```

For the MVP this can be one process with clean internal service boundaries. Later, the host can become a separate background process without changing protocol or data models.

### 4.2 Proposed repository layout

```text
agent-terminal/
├── upstream/
│   └── wezterm/                         # pinned fork/subtree
│
├── crates/
│   ├── agent-protocol/                  # IPC/event/adapter contracts
│   ├── agent-catalog/                   # SQLite + normalized conversations
│   ├── agent-runtime/                   # PTY/mux/process attachments
│   ├── agent-adapters/                  # registry + declarative loader
│   ├── agent-events/                    # OSC event protocol + reducers
│   ├── agent-usage/                     # account/context usage abstraction
│   ├── agent-settings/                  # safe config bindings and edits
│   ├── agent-sidebar/                   # pure sidebar model/ordering
│   ├── agent-tray/                      # Windows tray lifecycle
│   ├── adapter-claude/
│   ├── adapter-codex/
│   ├── adapter-opencode/
│   └── mock-harness/                    # deterministic test agent
│
├── apps/
│   ├── desktop/                         # WezTerm-derived executable
│   └── cli/                             # list/new/open/settle/status
│
├── extensions/
│   └── vscode/                          # Cursor/VS Code thin client
│
├── adapters/
│   └── examples/
│
├── tests/
│   ├── fixtures/
│   ├── integration/
│   └── windows-smoke/
│
└── docs/
    ├── ARCHITECTURE.md
    ├── ADAPTER_SPEC.md
    ├── SOURCE_LEDGER.md
    └── HACKATHON_BUILD_MAP.md
```

### 4.3 Local control protocol

Freeze a small versioned protocol before parallel work begins.

Minimum methods:

```text
host.hello
host.status

conversation.list
conversation.get
conversation.open
conversation.new
conversation.setOrganizationState
conversation.stop
conversation.refreshUsage

adapter.list
adapter.scan
adapter.installDraft

event.subscribe
terminal.attach        # later / optional for extension
terminal.input         # later / optional for extension
terminal.resize        # later / optional for extension
```

Minimum push events:

```text
catalog.changed
conversation.changed
runtime.attached
runtime.detached
runtime.outputAvailable
usage.changed
adapter.changed
```

The desktop UI and extension must never reach directly into SQLite or adapter internals.

---

## 5. WezTerm integration

### 5.1 Do not embed an external WezTerm process

Fork and reuse the renderer, terminal model, PTY, mux, font, clipboard, input, selection, search, hyperlink, and window crates directly.

The app should own one native OS window and one render surface.

```text
┌─────────────────────────────────────────────────────────────────┐
│ app chrome                                                      │
├──────────────────────┬──────────────────────────────────────────┤
│ native sidebar       │ session header / usage                   │
│                      ├──────────────────────────────────────────┤
│ active sessions      │                                          │
│ settled history      │ native WezTerm terminal viewport         │
│ filters/settings     │                                          │
│                      │                                          │
└──────────────────────┴──────────────────────────────────────────┘
```

### 5.2 Required WezTerm refactor

Introduce a first-class viewport layout:

```rust
pub struct AppLayout {
    pub sidebar: Rect,
    pub header: Rect,
    pub terminal: Rect,
    pub overlay: Option<Rect>,
}
```

All terminal calculations must use `layout.terminal`, not the whole client area.

Touch points:

- rows/columns from pixels;
- pane draw origin;
- GPU clipping/scissor;
- mouse-to-cell translation;
- scroll bar;
- selection;
- hyperlink hit testing;
- cursor and IME position;
- pane resize propagation;
- drag/drop and context-menu coordinates;
- DPI changes;
- fullscreen/maximize transitions.

### 5.3 Renderer decision

Use WezTerm's renderer for the main sidebar and header.

Advantages:

- one GPU surface;
- no Electron or WebView;
- no duplicate text/font pipeline;
- exact terminal clipping and focus behavior;
- low process and memory overhead;
- visual status updates can share WezTerm's existing invalidation loop;
- sidebar activation maps naturally to mux tab activation.

Limitations:

- WezTerm's box model is not a complete application widget framework;
- settings forms, accessibility, text editing, focus traversal, dropdowns, and screen-reader behavior require additional work;
- combining WarpUI, GPUI, egui, or another renderer in the same main window creates a second event/render/focus system.

Hackathon choice:

1. Use WezTerm's native box-model system for sidebar, header, status badges, buttons, and the first settings shell.
2. Implement only the widgets required by the vertical slice.
3. Re-evaluate a separate settings window after the main session experience works.
4. Do not integrate WarpUI or GPUI during the hackathon.

### 5.4 Mux model

Initial mapping:

```text
one app window
  └── one WezTerm mux window
       ├── hidden tab/pane: Claude conversation A
       ├── hidden tab/pane: Codex conversation B
       └── hidden tab/pane: OpenCode conversation C
```

The built-in tab bar is disabled. The sidebar is the tab/session navigator.

A live native conversation has at most one application-owned runtime attachment unless an adapter explicitly supports multiple clients.

### 5.5 Windows process ownership

Application-launched harnesses should enter a Windows Job Object owned by the tray host.

Desired behavior:

- closing the UI window does not affect the Job Object;
- explicit Stop terminates the selected process tree;
- tray Quit can terminate all owned process trees;
- a hard host crash does not leave child trees indefinitely orphaned;
- native session files remain resumable even after process death.

Process state must come from owned handles and structured harness events, not terminal start timestamps.

---

## 6. Unified history and sidebar

### 6.1 Session catalog adapters

Each harness implements:

```rust
trait SessionCatalogAdapter {
    fn scan_recent(&self, cursor: Option<CatalogCursor>)
        -> Result<CatalogPage>;

    fn watch(&self) -> Result<BoxStream<'static, CatalogEvent>>;

    fn read_metadata(&self, native_id: &str)
        -> Result<NativeConversationMetadata>;

    fn read_preview(&self, native_id: &str)
        -> Result<ConversationPreview>;

    fn resume_command(&self, native_id: &str, cwd: Option<&Path>)
        -> Result<CommandSpec>;
}
```

Use structured APIs where available. Use session-store files where necessary.

### 6.2 Performance rules

- Initial scan loads only the newest page per configured profile.
- Normalize metadata into SQLite.
- Do not load full transcripts for sidebar rows.
- Load preview text only for visible rows or tooltips.
- Load full history only on explicit inspection.
- Use stable cursor pagination.
- Dedupe by `(adapter_id, profile_id, native_session_id)`.
- Use filesystem watching or provider events for incremental changes.
- Never rescan every transcript on every launch.
- Virtualize the sidebar list.
- Keep active rows and settled rows as separate indexed queries.

### 6.3 Global ordering and filtering

Default global ordering:

```text
Active:
  pinned first
  then last activity descending

Settled:
  settled-at descending
  fallback to last activity descending
```

Filters:

- All agents
- Claude
- Codex
- OpenCode
- Kimi
- Grok
- Cline
- project/repository
- active/settled
- running/needs-input/failed
- text search over normalized metadata

Every row shows:

```text
[agent icon] title
project or cwd
runtime badge
relative time
usage/context warning when relevant
```

### 6.4 Titles

Title precedence:

1. user-defined application alias;
2. native session title;
3. latest meaningful user prompt;
4. first user prompt;
5. native summary;
6. harness name + timestamp fallback.

An application alias does not rewrite the native transcript.

### 6.5 Imported liveness

Imported histories are not assumed active merely because their last timestamp is recent.

Possible runtime labels:

```text
Not running
Running here
Needs input here
Possibly running externally
Unknown
```

Only claim precise live status when supported by:

- an application-owned process attachment;
- a structured event bridge;
- a reliable provider API;
- a reliable process/session lock correlation.

---

## 7. Correct handling of `/new`, `/resume`, `/clear`, and forks

### 7.1 Do not trust raw slash-command interception

Watching input bytes for `/new` is useful only as a hint.

It is not authoritative because:

- the user can edit the line before submitting;
- paste and terminal bracketed-paste alter byte sequences;
- TUI input history can replay commands;
- slash commands can open menus before choosing an action;
- different harnesses use different commands;
- a harness may create a new native identity without a slash command.

### 7.2 Preferred identity detection hierarchy

1. Structured session event with a new native session ID.
2. Provider API event.
3. Session-store watcher observing a new/activated native session correlated to the owned process.
4. Submitted slash-command candidate plus session-store confirmation.
5. Screen-state heuristic.
6. User binding prompt only when still ambiguous.

### 7.3 Pane rebinding algorithm

When the currently attached harness changes native conversation identity:

```text
old conversation A owns pane P

new native session ID B observed
        ↓
catalog upsert B
        ↓
detach RuntimeAttachment(P) from A
        ↓
attach RuntimeAttachment(P) to B
        ↓
A remains in catalog
B becomes active/unsettled and selected
sidebar redraws without restarting the TUI
```

The user sees the new thread appear immediately while the native TUI remains untouched.

### 7.4 Structured agent event bridge

Define a versioned terminal escape-sequence protocol inspired by the strongest existing approach:

```text
OSC 777
title: agent-terminal://event
body: JSON
```

Example payload:

```json
{
  "v": 1,
  "adapter": "claude-code",
  "event": "session_start",
  "sessionId": "native-id",
  "cwd": "C:\\work\\repo",
  "project": "repo",
  "transcriptPath": "C:\\...\\session.jsonl",
  "summary": null,
  "query": null,
  "response": null
}
```

Core event types:

```text
session_start
prompt_submit
tool_complete
turn_complete
turn_failed
permission_request
permission_replied
question_asked
idle_prompt
context_updated
usage_updated
session_end
```

The protocol is adapter-neutral. Harness-specific hooks translate native events into this shape.

### 7.5 Fallback detectors

Port the useful CCManager-style state detectors as a fallback:

- inspect a bounded window of WezTerm's terminal model;
- debounce after pane output;
- no fixed 100 ms polling;
- emit confidence and evidence;
- never let a low-confidence screen match override an authoritative structured event.

---

## 8. Adapter package format

### 8.1 Package layout

```text
claude-code.agent-adapter/
├── adapter.toml
├── settings.schema.json
├── icon.svg
├── fixtures/
│   ├── sessions/
│   ├── events/
│   └── usage/
└── hooks/
    └── optional reviewed integration files
```

### 8.2 Manifest sketch

```toml
schema_version = 1
id = "claude-code"
display_name = "Claude Code"
platforms = ["windows"]
icon = "icon.svg"

[detect]
executables = ["claude.exe", "claude.cmd", "claude"]
version_args = ["--version"]

[launch]
new = ["{{executable}}", "--dangerously-skip-permissions"]
resume = ["{{executable}}", "--resume", "{{native_session_id}}"]
cwd_mode = "process"

[catalog]
kind = "filesystem"
roots = ["{{env.CLAUDE_CONFIG_DIR|default:%USERPROFILE%\\.claude}}"]
parser = "claude-session-v1"
watch = true
page_size = 50

[events]
transport = "osc777"
sentinel = "agent-terminal://event"
installer = "builtin:claude-hooks-v1"

[settings]
schema = "settings.schema.json"

[[settings.files]]
scope = "global"
format = "json"
path = "{{env.CLAUDE_CONFIG_DIR|default:%USERPROFILE%\\.claude}}\\settings.json"

[[usage.providers]]
kind = "structured_hook"
priority = 100

[[usage.providers]]
kind = "tui_transaction"
priority = 20
command = "/usage"
stale_after_seconds = 300
only_when_idle = true
```

The exact launch/resume flags belong in versioned adapter fixtures and tests; they must not be guessed at runtime.

### 8.3 Declarative versus executable support

Community packages are declarative by default.

Safe declarative capabilities:

- executable discovery;
- command templates;
- known config paths;
- JSON/TOML/YAML bindings;
- session file patterns;
- regular-expression metadata extractors;
- OSC event schemas;
- usage parser schemas;
- icons;
- test fixtures.

Executable hook installers require:

- explicit user approval;
- visible diff/commands;
- backup and rollback;
- restricted known destinations;
- signature/trust metadata;
- clear source attribution.

Never silently execute LLM-generated scripts from a downloaded adapter.

### 8.4 AI adapter-builder skill

The bundled skill should:

1. Find installed executables and versions.
2. inspect `--help` and subcommand help.
3. locate config and session stores.
4. inspect official documentation or open source.
5. identify new/resume/history/settings/usage capabilities.
6. generate a draft adapter package.
7. generate fixtures and contract tests.
8. run the contract suite.
9. show every proposed file/config/hook change.
10. install only after explicit approval.

Generated integration logic should be declarative whenever possible. The skill may use an installed coding agent for inference, but the host validates the result mechanically.

---

## 9. Settings architecture

### 9.1 App-owned terminal settings

Agent Terminal should own a structured terminal configuration rather than editing the user's arbitrary `wezterm.lua`.

Store structured values in the app database or an app TOML file, then materialize a WezTerm `Config` at runtime.

Benefits:

- safe GUI editing;
- typed defaults;
- validation;
- reset controls;
- no Lua source rewriting;
- no accidental collision with a separate WezTerm installation.

Optional later features:

- import compatible values from an existing WezTerm config;
- advanced Lua extension layer;
- export generated config.

### 9.2 Agent settings bindings

An adapter's JSON Schema describes UI and validation. Binding metadata maps fields to native files.

Example:

```json
{
  "id": "permissionMode",
  "label": "Permission mode",
  "type": "enum",
  "values": ["default", "acceptEdits", "bypassPermissions"],
  "bindings": [
    {
      "scope": "global",
      "file": "settings.json",
      "pointer": "/permissions/defaultMode",
      "restart": "session"
    }
  ]
}
```

Config-edit invariants:

- create a timestamped backup;
- parse before editing;
- preserve unrelated fields;
- write a temporary file;
- validate the result;
- replace atomically;
- roll back on failure;
- show the exact change;
- never display or log secrets;
- respect global versus project scope.

### 9.3 Settings sections

```text
General
Agents
Terminal
Appearance
Keybindings
Sessions
Usage
Integrations
Archive
Advanced
```

Hackathon settings scope:

- theme;
- font/font size;
- sidebar width;
- default agent;
- configured agent executable;
- default launch args;
- project cwd;
- settle/history behavior;
- Ctrl+V and app keybindings.

Do not attempt hundreds of settings before the session experience works.

---

## 10. Usage and context

### 10.1 Keep the quantities separate

```text
Account limits:
  session bucket
  weekly bucket
  credits/cost where available

Conversation limits:
  current context tokens
  context-window percentage
  compaction state
  turn/session token totals
```

Never combine account quota and context consumption into one percentage.

### 10.2 Provider hierarchy

Use the strongest source available:

1. Structured provider API/SDK.
2. Structured local session events.
3. Native session/transcript metadata.
4. Noninteractive CLI command.
5. Structured hooks.
6. Controlled native TUI transaction.
7. Screen parser fallback.

### 10.3 Refresh policy

Default:

- opening the usage dropdown refreshes when the cache is older than five minutes;
- a refresh button always forces an update;
- structured providers may refresh after a turn-complete event;
- no account API polling while the usage surface is closed;
- TUI `/usage` injection is on-demand and only while the harness is confidently idle;
- all failures retain the last good snapshot and display its age.

```rust
struct UsageSnapshot {
    profile_id: ProfileId,
    account_buckets: Vec<UsageBucket>,
    context: Option<ContextSnapshot>,
    source: UsageSource,
    confidence: Confidence,
    sampled_at: DateTime<Utc>,
    error: Option<String>,
}
```

### 10.4 Controlled TUI usage transaction

Only for adapters without structured data:

1. Confirm idle state and no approval/question modal.
2. Save current screen/viewport state.
3. submit the configured usage command.
4. wait for a known usage-screen signature;
5. wait for stable terminal output;
6. capture a bounded cell region;
7. parse the adapter's declarative schema;
8. send the configured close key;
9. restore focus;
10. cache the snapshot.

No periodic hidden command injection.

### 10.5 Teach-usage workflow

The adapter-builder may generate a declarative parser:

```json
{
  "formatVersion": 1,
  "versionFingerprint": "agent 5.x / en-US",
  "openCommand": "/usage",
  "closeKeys": ["Escape"],
  "readyPatterns": ["Usage", "Weekly"],
  "stableForMs": 250,
  "fields": [
    {
      "id": "weekly_percent",
      "labelPattern": "^Weekly",
      "valuePattern": "(\\d+(?:\\.\\d+)?)%",
      "type": "percent"
    }
  ]
}
```

Store original captures as fixtures. If the harness version changes and fixtures no longer match, mark the provider degraded and offer to teach the new format.

---

## 11. Cursor / VS Code extension

### 11.1 Architectural rule

The extension is a client of the tray host. It is not another session manager.

### 11.2 MVP surface

Add an Activity Bar container with a native Tree View:

```text
AGENT SESSIONS

Current Workspace
  Claude · refactor renderer       Working
  Codex · tests                    Needs input

Other Projects
  OpenCode · API migration         Stopped

Settled
  Claude · old investigation
```

Commands:

```text
New Agent in Current Workspace
Open / Focus Conversation
Settle
Unsettle
Stop
Refresh Usage
Open Agent Terminal
Configure Agents
```

Status bar:

```text
Claude session 64% · weekly 41%
```

### 11.3 Hackathon behavior

Clicking a conversation:

- asks the local host to open/focus it in Agent Terminal;
- does not spawn a duplicate harness;
- keeps one source of runtime truth.

"New Agent in Current Workspace":

- sends current workspace folder and chosen adapter to the host;
- host creates the native session;
- extension tree updates from push events.

This is enough to make the project meaningfully Cursor-integrated without creating a second terminal runtime.

### 11.4 Later terminal attachment

VS Code supports extension-controlled pseudoterminals. A later extension can attach to the host's terminal stream and display the same session inside Cursor.

Requirements before this is safe:

- one authoritative PTY size or negotiated view sizes;
- replay/snapshot on attachment;
- backpressure;
- terminal input ownership;
- multiple-client focus policy;
- detach without killing;
- clipboard and link behavior.

Do not make this a hackathon blocker.

---

## 12. Mobile control

### 12.1 Same-LAN zero-setup mode

Later phase:

1. tray host starts a local authenticated HTTP/WebSocket server;
2. it discovers reachable LAN addresses;
3. user clicks Phone Control;
4. app creates a short-lived one-time pairing token;
5. QR contains the LAN URL and token;
6. phone opens the web client;
7. token is exchanged for a scoped device credential;
8. device appears in a revocation list.

This requires no Tailscale account. The phone and computer must be able to reach each other on the same network, and Windows may show a firewall permission prompt.

### 12.2 Remote mode

A computer behind NAT cannot be reached from arbitrary cellular/Wi-Fi networks with literally no relay, tunnel, VPN, port forward, or public server.

Practical options:

- temporary Quick Tunnel for demos and short-lived sharing;
- a future Agent Terminal relay service;
- optional user-managed named tunnel;
- optional private overlay for advanced users.

Quick Tunnel is suitable for a late demo mode, not the permanent security architecture.

### 12.3 Mobile is not part of the hackathon critical path

The local control protocol should be designed so the mobile client can be added without changing the host, but no mobile implementation should compete with the Windows session vertical slice.

---

## 13. Future multi-agent coordination

Reserve metadata now:

```rust
struct CoordinationMetadata {
    worktree_path: Option<PathBuf>,
    branch: Option<String>,
    coordination_group: Option<String>,
    write_scope: Option<Vec<PathPattern>>,
}
```

Future layers can add:

- one worktree per agent;
- overlapping-diff warnings;
- file or directory leases;
- shared task graph;
- agent-to-agent messages;
- reviewer/implementer roles;
- merge queue.

Do not make agents coordinate by injecting prose into each other's TUIs in the first version. Start with filesystem and git isolation because it is observable and enforceable.

---

## 14. Hackathon parallel sprint

### 14.1 First hour: freeze contracts

Before parallel coding:

1. Pin a WezTerm commit.
2. Build unmodified WezTerm on Windows.
3. Create the repository/workspace.
4. Commit `agent-protocol` types.
5. Commit SQLite schema v1.
6. Commit mock-harness event protocol.
7. Assign exclusive file ownership.
8. Create one Git worktree per track.

No agent starts implementation until this baseline commit exists.

### 14.2 Workstream ownership

#### Track A — WezTerm viewport and shell

**Exclusive owner of upstream WezTerm GUI/window/render files.**

Deliver:

- `AppLayout`;
- fixed-width sidebar;
- terminal viewport clipping;
- mouse/cell mapping;
- resize/DPI correctness;
- hidden native tab bar;
- sidebar row activates mux tab;
- minimal header;
- close-to-tray window behavior.

Must not edit catalog, adapters, extension, or usage crates.

#### Track B — Host, catalog, and tray lifecycle

Deliver in new crates:

- SQLite migrations;
- `NativeConversation`;
- `RuntimeAttachment`;
- organization/runtime orthogonality;
- tray menu and host lifecycle;
- local control protocol;
- create/list/open/settle/stop methods;
- startup reconciliation.

Must not edit upstream renderer files.

#### Track C — Adapter framework and mock harness

Deliver:

- adapter manifest schema;
- registry;
- executable discovery;
- command template expansion;
- mock adapter;
- deterministic mock harness executable;
- OSC 777 event encoder;
- adapter contract tests.

The mock harness should support:

```text
/new
/work
/ask
/approve
/fail
/complete
/usage
/quit
```

and write a fake native session store.

#### Track D — Unified sidebar model

Deliver pure Rust code, porting concepts/tests rather than React:

- active/settled partition;
- stable ordering;
- filter/search model;
- status priority;
- unread/attention markers;
- pagination;
- 10,000-row performance fixture;
- row view models.

No rendering code.

#### Track E — Real history import vertical slice

Deliver:

- Claude recent-session fixture importer;
- Codex app-server/thread-list importer or recorded protocol fixture;
- normalized metadata;
- dedupe;
- cursor pagination;
- lazy preview;
- resume command contract.

Use fixtures first so Track E does not depend on installed credentials.

#### Track F — Cursor/VS Code extension

Deliver:

- Activity Bar view container;
- TreeDataProvider;
- local protocol client;
- mock-host mode;
- New Agent in Current Workspace;
- Open/Focus;
- Settle/Unsettle;
- runtime status icons;
- extension smoke tests.

It must compile before the real host exists by using a protocol fixture server.

#### Track G — Usage model

Optional but parallel-safe:

- normalized account/context usage contracts;
- cache and stale policy;
- mock provider;
- usage dropdown view model;
- parser fixture tests;
- one structured Codex provider only if the core demo is already green.

### 14.3 Hot-zone rule

Only Track A touches:

```text
wezterm-gui/src/termwindow/**
wezterm-gui/src/frontend.rs
wezterm-gui/src/tabbar.rs
window/**
```

Every other track works in newly-created crates/apps.

This prevents seven agents from turning the renderer into a communal knife fight.

---

## 15. Merge gates

### Gate 0 — Stock baseline

- pinned WezTerm commit builds on Windows;
- tests used by the changed crates pass;
- clean startup and terminal interaction recorded.

### Gate 1 — Viewport seam

- hard-coded sidebar is visible;
- terminal occupies remaining rectangle;
- resize, maximize, DPI, mouse selection, wheel, hyperlinks, cursor, and Ctrl+V work;
- no clipping into sidebar.

This is the highest-risk gate.

### Gate 2 — Mock live session

- plus button launches mock harness;
- row appears with icon/title;
- pane activates;
- mock event changes Working → Needs Input → Complete;
- close window leaves mock process alive;
- tray reopen restores same terminal.

### Gate 3 — Catalog and settle

- fixture histories import as settled;
- active and settled queries are paginated;
- settle moves a live row without changing runtime/process;
- open historical row resumes or launches mock native session;
- 10,000 catalog rows remain responsive.

### Gate 4 — Native identity transition

- mock `/new` creates a new native ID;
- structured event is received;
- live pane is atomically rebound;
- new row appears active;
- old row remains catalogued;
- no duplicate process is launched.

### Gate 5 — One real harness

Preferred target: Claude or Codex, whichever yields the faster reliable vertical slice.

- installed executable detected;
- session launched;
- native TUI unmodified;
- native session ID captured;
- recent history imported;
- one status transition observed;
- resume works.

### Gate 6 — Cursor extension

- extension shows the same catalog;
- command launches a session in current Cursor workspace;
- status update appears in the tree;
- clicking row focuses desktop session;
- settle action is reflected in both UIs.

### Gate 7 — Usage

- mock usage displays account and context separately;
- stale-on-open refresh works;
- source and sample age shown;
- no idle polling;
- one real structured provider only if reliable.

---

## 16. Verification plan

### 16.1 Unit tests

- manifest schema and version rejection;
- command-template escaping on Windows;
- catalog normalization;
- dedupe keys;
- pagination cursor stability;
- active/settled partition;
- settle does not mutate runtime state;
- runtime state does not mutate organization state;
- event decoding and state priority;
- pane rebinding;
- usage normalization;
- config binding and atomic edit plans;
- title fallback rules.

### 16.2 Deterministic integration harness

The mock harness is mandatory, not optional test decoration.

It should:

- run under ConPTY;
- render a basic TUI;
- persist fake sessions;
- emit structured OSC events;
- switch native IDs on `/new`;
- simulate completion, approval, input, failure, crash, and usage;
- support resume by native ID;
- spawn a child process for Job Object tests.

This makes the most difficult behavior testable without depending on network services, rate limits, credentials, or changing vendor TUIs.

### 16.3 Windows smoke tests

PowerShell-driven smoke suite:

```text
build release
start host
wait for ready endpoint
launch mock session
assert process and catalog row
close window
assert host and child still alive
reopen window
assert same pane/session
settle
assert child still alive
stop
assert child tree exits
quit tray
assert no owned process remains
```

### 16.4 Renderer verification

For each sidebar width and DPI scale:

- terminal rows/columns match visible pixels;
- mouse selection starts at correct cell;
- hyperlink hit testing is correct;
- scroll bar stays inside terminal rect;
- cursor and IME candidate window align;
- maximize/restore does not produce stale dimensions;
- active pane receives keyboard input;
- sidebar controls do not leak keys to TUI;
- raw/pass-through mode works.

### 16.5 Recovery tests

- UI window crash while host remains;
- agent process crash;
- host restart with native transcript intact;
- database missing/corrupt row;
- session file deleted;
- two profiles expose same native ID;
- adapter version changes;
- config edit interrupted halfway;
- new session event arrives before session file;
- session file arrives before event;
- duplicate event delivery.

### 16.6 Extension tests

- extension activates only when view/command is used;
- mock protocol reconnect;
- tree refresh and incremental updates;
- current-workspace launch payload;
- missing desktop host offers launch/install guidance;
- no webview required for MVP.

### 16.7 Performance budgets

Measure, do not merely hope:

- idle CPU approximately zero with no running output;
- no fixed session-status polling;
- no provider/API calls while usage UI is closed;
- sidebar row rendering virtualized;
- initial catalog query bounded;
- full transcripts not loaded for list rows;
- settled/stopped records own no PTY;
- switching existing live pane does not serialize/replay the full terminal;
- memory reported separately for tray host, UI window, and each harness;
- target UI/sidebar action latency under one display frame.

---

## 17. Hackathon demo script

1. Launch Agent Terminal.
2. It imports fixture or real Claude and Codex history into a unified settled section.
3. Open Cursor and the Agent Sessions Activity Bar view.
4. Press **New Agent in Current Workspace**.
5. Agent Terminal opens/focuses and launches the native harness TUI.
6. Sidebar shows the correct agent icon and project.
7. The mock or real event bridge changes the row to **Working**.
8. Trigger a permission/input event; row changes to **Needs input**.
9. Run `/new`; a new conversation row appears and the existing pane is rebound.
10. Settle the still-running conversation; it moves to compact history while continuing to run.
11. Close Agent Terminal's window.
12. Show that the tray icon and agent process remain.
13. Reopen from tray; same live terminal is present.
14. Cursor's tree shows the same state and can focus the conversation.
15. Open usage dropdown; separate account and context bars appear from cached/mock data.

That demo proves the product. Everything else is garnish.

---

## 18. Cut line

### Must ship for the hackathon

- Windows-native build.
- WezTerm terminal viewport plus sidebar.
- tray close/reopen.
- mock harness.
- session catalog.
- active versus settled.
- structured event bridge.
- `/new` pane rebinding through mock harness.
- one real harness launch/import attempt.
- Cursor Tree View client.

### Strong stretch goals

- second real harness;
- real Claude hook bridge;
- Codex structured history/usage;
- minimal settings page;
- usage dropdown with one real provider.

### Explicitly defer

- mobile;
- public relay;
- complete settings editor;
- arbitrary adapter generation installation;
- full transcript viewer;
- multiple attached terminal clients;
- multi-agent coordination;
- automatic worktrees;
- macOS/Linux.

---

## 19. Source-reuse policy

Maintain `docs/SOURCE_LEDGER.md` with:

- source repository;
- exact commit;
- original path;
- destination path;
- license;
- copied, translated, or concept-only classification.

Recommended policy:

- WezTerm MIT code: fork and modify with notice.
- T3 Code MIT code: port pure state logic and visual concepts with notice.
- CCManager MIT code: port detector logic and fixtures with notice.
- Claude Code Warp integration MIT repository: reuse/rename protocol and hooks with notice.
- Warp application AGPL code: study architecture only; do not copy into the MIT codebase.
- WarpUI MIT crates: reconsider later, not in hackathon MVP.
- Zed/GPUI: only separately-licensed components may be considered, and only after the WezTerm-native shell is proven.

---

## 20. Final architectural invariant

Every implementation decision should preserve this sentence:

> **The native harness owns the conversation; Agent Terminal owns its discoverability, attachment, organization, and visibility.**

When that boundary is honored, new harness features arrive for free, users stop losing threads, and the application remains a lightweight terminal rather than growing into another proprietary AI platform.
