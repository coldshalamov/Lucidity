# Agent Terminal — Parallel Sprint Prompts

These prompts assume the baseline repository and protocol contracts have already been committed.

## Shared rules for every agent

- Read `docs/ARCHITECTURE.md`, `docs/HACKATHON_BUILD_MAP.md`, and `crates/agent-protocol` before editing.
- Work in your assigned Git worktree and branch.
- Do not change public protocol contracts without opening a small proposal in `docs/proposals/` and notifying the integration owner.
- Do not edit files owned by another track.
- Add tests with every behavioral change.
- Commit small, coherent slices.
- Report exact commands run and their results.
- Never claim a live harness state from a timestamp alone.
- `OrganizationState::Settled` is visual organization only and must never stop or interrupt a process.
- Do not add fixed polling loops.
- Do not copy AGPL Warp application code into this MIT project.
- Prefer deterministic fixtures and the mock harness over live-service assumptions.

---

# Track A — WezTerm viewport and application shell

## Ownership

You are the only agent allowed to edit:

```text
upstream/wezterm/wezterm-gui/src/termwindow/**
upstream/wezterm/wezterm-gui/src/frontend.rs
upstream/wezterm/wezterm-gui/src/tabbar.rs
upstream/wezterm/window/**
apps/desktop/src/shell/**
```

Do not edit catalog, adapter, usage, extension, or SQLite code.

## Goal

Refactor the WezTerm GUI so a native application sidebar and header can coexist with a correctly clipped terminal viewport in one OS window and one GPU surface.

## Required implementation

1. Introduce an `AppLayout` value with `sidebar`, `header`, `terminal`, and optional `overlay` rectangles.
2. Use `layout.terminal` for terminal rows/columns, rendering origin, clipping, mouse-to-cell conversion, scrollbar, cursor/IME placement, and resize propagation.
3. Disable the stock tab bar in the Agent Terminal binary.
4. Render a fixed-width placeholder sidebar and header using existing WezTerm native box-model primitives.
5. Add a minimal hit target that activates one of multiple mux tabs.
6. Preserve stock WezTerm behavior when application chrome is disabled.
7. Add close-to-tray callback integration point; do not implement tray persistence itself.

## Done when

- The terminal cannot draw into the sidebar.
- Clicking/selecting terminal cells remains correct after sidebar resizing.
- Wheel, selection, links, clipboard paste, maximize/restore, and DPI changes work.
- Two mux tabs can be selected from two sidebar rows.
- Existing relevant WezTerm tests pass.
- A Windows smoke build succeeds.

## Verification report

Include:

```text
cargo fmt --check
cargo test -p wezterm-gui <relevant tests>
cargo build -p agent-terminal-desktop --release
manual DPI tested:
manual resize tested:
mouse selection tested:
hyperlink hit tested:
```

---

# Track B — Tray host, catalog, lifecycle, and IPC

## Ownership

```text
crates/agent-catalog/**
crates/agent-runtime/**
crates/agent-tray/**
crates/agent-protocol/**
apps/cli/**
apps/desktop/src/host/**
tests/integration/host_*
```

Do not edit upstream WezTerm renderer/window files.

## Goal

Build the durable host model that survives UI-window closure and gives all clients one source of truth.

## Required implementation

1. SQLite schema/migrations for profiles, conversations, runtime attachments, aliases, and usage snapshots.
2. Independent `OrganizationState` and `RuntimeState`.
3. CRUD/query API with cursor pagination and filters.
4. Runtime attachment registration/deregistration.
5. Local same-user IPC endpoint and version handshake.
6. Tray lifecycle model:
   - close UI does not quit host;
   - reopen command emits UI-open request;
   - tray quit exposes stop-all-and-quit operation.
7. Startup reconciliation for stale runtime attachments.
8. CLI commands:
   - `status`
   - `list`
   - `new`
   - `open`
   - `settle`
   - `unsettle`
   - `stop`

## Critical invariants

- Settling changes only `organization_state`.
- Stopping changes only runtime/process state.
- A catalog row may exist without a runtime attachment.
- IPC clients never access SQLite directly.
- Pagination order is deterministic.

## Done when

- Integration tests prove close/reopen state.
- 10,000 synthetic conversations query rapidly.
- Settle on a live mock attachment leaves it live.
- Stale attachments are reconciled after host restart.

---

# Track C — Adapter framework and deterministic mock harness

## Ownership

```text
crates/agent-adapters/**
crates/agent-events/**
crates/mock-harness/**
adapters/examples/**
tests/fixtures/mock-harness/**
```

Do not edit renderer, tray, extension, or real vendor adapter crates.

## Goal

Create the versioned adapter contract and a deterministic coding-agent TUI simulator that makes the difficult lifecycle testable.

## Required implementation

1. TOML adapter manifest parser and JSON Schema validation.
2. Executable discovery and version probe.
3. Windows-safe command-template expansion without shell interpolation by default.
4. Versioned OSC 777 event protocol with sentinel `agent-terminal://event`.
5. Event decoder and state reducer.
6. Mock harness that:
   - runs in a PTY;
   - writes a fake native session store;
   - supports `/new`, `/work`, `/ask`, `/approve`, `/fail`, `/complete`, `/usage`, `/quit`;
   - emits structured events;
   - supports `--resume <native-id>`;
   - spawns an optional child process for process-tree tests.
7. Contract tests and malformed-event fuzz cases.

## `/new` behavior

When `/new` is submitted:

- allocate a new native session ID;
- create a new fake session-store record;
- emit `session_start` carrying the new ID;
- continue in the same process and PTY.

This is the canonical test for pane rebinding.

## Done when

A test can launch the mock harness, submit `/new`, and observe two durable catalog identities while only one process/pane exists.

---

# Track D — Unified sidebar domain model

## Ownership

```text
crates/agent-sidebar/**
tests/fixtures/sidebar/**
```

Do not write renderer code.

## Goal

Port the proven session-list semantics into a pure Rust view-model crate that any UI can consume.

## Required implementation

1. Partition active versus settled.
2. Stable deterministic ordering.
3. Status priority and attention badges.
4. Harness/project/runtime filters.
5. text search over normalized metadata.
6. Cursor pagination.
7. user alias title precedence.
8. list virtualization interface.
9. test fixtures with 10,000 conversations.
10. regression test: settling a working row moves sections without changing its runtime badge.

## Output

Expose view models only:

```rust
struct SidebarSectionVm
struct SidebarRowVm
struct SidebarBadgeVm
struct SidebarQuery
```

No dependency on WezTerm types.

---

# Track E — History catalog importers

## Ownership

```text
crates/adapter-claude/**
crates/adapter-codex/**
tests/fixtures/claude/**
tests/fixtures/codex/**
```

Do not edit the generic adapter manifest parser or UI.

## Goal

Produce one reliable real history vertical slice and a fixture-backed second adapter.

## Work order

1. Define normalized fixture records.
2. Implement recent-page scan and dedupe from fixtures.
3. Implement lazy preview reads.
4. Implement resume-command resolution.
5. Add incremental watcher/event ingestion.
6. Connect to a real local installation only after fixtures pass.

## Claude target

- locate configured session root without assuming only the default directory;
- index native IDs, timestamps, cwd/project, and prompt/title preview;
- accept structured hook events carrying session ID/transcript path;
- do not rewrite native transcripts.

## Codex target

Prefer app-server structured APIs:

- `thread/list` for catalog;
- `thread/read` for lazy detail;
- `thread/resume` or the native CLI resume contract for attachment;
- preserve pagination cursor.

## Done when

- Repeated imports are idempotent.
- Recent-page scan is bounded.
- Missing/corrupt records are quarantined, not fatal.
- At least one installed harness can launch and resume through the normalized adapter interface.

---

# Track F — Cursor / VS Code extension

## Ownership

```text
extensions/vscode/**
tests/fixtures/extension-host/**
```

Do not edit Rust host or protocol definitions unless a proposal is approved.

## Goal

Build a thin Cursor/VS Code client that mirrors the tray host's session catalog.

## Required implementation

1. Activity Bar view container.
2. `TreeDataProvider` for:
   - Current Workspace
   - Other Projects
   - Settled
3. Runtime/status icons and agent icons.
4. local IPC/WebSocket client with reconnect.
5. commands:
   - New Agent in Current Workspace
   - Open/Focus
   - Settle
   - Unsettle
   - Stop
   - Refresh Usage
6. status bar usage summary.
7. mock-host mode for development.
8. extension tests.

## MVP constraint

Clicking a conversation asks the desktop host to open/focus it. Do not implement a `Pseudoterminal` attachment unless every earlier gate is green.

## Done when

The extension can launch a mock agent using the current Cursor workspace path, observe status changes, settle it without stopping it, and focus the desktop session.

---

# Track G — Usage and context

## Ownership

```text
crates/agent-usage/**
tests/fixtures/usage/**
apps/desktop/src/usage/**
```

Do not inject commands into a real TUI until the mock provider is complete.

## Goal

Normalize account quota and conversation context without conflating them or creating idle traffic.

## Required implementation

1. `UsageSnapshot`, `UsageBucket`, `ContextSnapshot`, source/confidence types.
2. cache and stale-on-open policy.
3. mock provider.
4. dropdown view model.
5. declarative TUI parser schema.
6. parser fixture tests.
7. controlled transaction state machine against mock harness `/usage`.
8. manual refresh.
9. source, age, and last-good-data display.

## Hard rules

- Account quota and context are separate surfaces.
- No fixed background polling.
- No hidden `/usage` while a harness is busy or awaiting input.
- Failures keep and label the last good snapshot.
- A real provider is a stretch goal, not permission to destabilize the core demo.

---

# Integration owner

## Responsibilities

- Maintain the canonical build map.
- Merge only at completed gates.
- Resolve protocol changes before code changes.
- Run the full test matrix after each merge.
- Keep Track A's renderer hot zone single-owned.
- Reject any change that couples settling to process termination.
- Keep a source/license ledger.
- Maintain a working demo branch.
- Activate the fallback plan if the WezTerm viewport seam is not green early enough.

## Fallback demo plan

If integrated sidebar rendering remains unstable:

- run the same host/catalog/adapters;
- present a separate lightweight controller window beside stock/forked WezTerm;
- use the host to launch/focus mux sessions;
- preserve the Cursor extension and session demo.

This fallback proves the product model while the integrated renderer continues as a post-hackathon track.
