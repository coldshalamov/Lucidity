# Lucidity / Agent Terminal Charter

Status: canonical product intent; technical and native-design Wave 0 contracts are accepted. Implementation is gated by the remaining human stock-input receipt and the control-plane freeze commit.

## Mission

Build a sleek, Windows-native, MIT-licensed terminal for coding-agent work. Native harness TUIs remain the conversation surface. Lucidity adds the missing control plane: durable conversation discovery, active/history organization, live attachment visibility, close-to-tray persistence, graphical settings, usage/context visibility, modular adapters, and a thin Cursor/VS Code client.

The user-facing promise is simple: one lightweight application shows every configured coding-agent conversation, makes its real runtime state understandable, and resumes it without forcing the user to remember harness-specific names, timestamps, or slash-command behavior.

## Authority order

When sources disagree, use this order:

1. The user's explicit decisions in the source planning conversation and current task.
2. Accepted ADRs and frozen contracts in `.orchestrator/`.
3. The current task packet.
4. The distinct documents in `HACKATHON/`.
5. Worker narration or speculative design notes.

The early planning suggestion that settling should stop a process is rejected. The user's later correction and this charter are authoritative: settling is visual organization only.

## Product boundary

- Native Windows first. No WSL or tmux installation or dependency.
- Rust owns the desktop product, durable state, lifecycle, adapters, usage, settings, and local control plane.
- Native Claude Code, Codex, Kimi, OpenCode, Grok, Cline, and other TUIs remain unmodified as the visible chat interface.
- The Cursor/VS Code extension is necessarily TypeScript but remains a thin client of the Rust host.
- No Electron, Tauri, desktop WebView, or Node.js runtime in the desktop product.
- No custom chat renderer, provider subscription, cloud account, public relay, transcript duplication, or multi-agent worktree coordinator in the protected first demo.
- Preserve the existing WezTerm root topology and upstream mergeability unless an accepted ADR proves that a different layout buys a concrete boundary.
- MIT-compatible source only. AGPL Warp application code may inform architecture but must not be copied.

## Non-negotiable domain invariants

1. One catalog conversation maps to exactly one adapter/profile/native-session identity.
2. A live pane is a temporary runtime attachment, never the durable conversation record.
3. `OrganizationState` and `RuntimeState` are independent axes.
4. Settling or unsetting a conversation never stops, interrupts, pauses, detaches, launches, or resumes a process.
5. Stopping a conversation changes runtime/process state only and never settles it.
6. Imported history allocates no PTY, pane, process, terminal buffer, or full transcript until explicitly opened.
7. Opening an already attached conversation focuses the existing pane; it does not create a duplicate process.
8. `/new`, `/resume`, `/clear`, and fork-like transitions rebind a pane only after a new native identity is confirmed by a structured event, provider API, or correlated native session-store evidence. Submitted text is a hint, not truth.
9. Closing the visible window leaves the tray host, owned mux panes, and owned agents alive. Explicit tray Exit is the real quit path.
10. Imported external histories are never labeled live from timestamps alone. Unproven liveness is `UnknownExternal` or equivalent.
11. Desktop UI, CLI, and extension use one versioned local control protocol. They never read SQLite or adapter internals directly.
12. No fixed background status, history, provider, Git, or usage polling. Prefer events, filesystem notifications, stale-on-open refresh, and explicit actions.
13. Stock WezTerm behavior remains available and must not regress when Lucidity chrome is disabled.

## Experience principles

- Use plain product language: Conversations, Active, Needs input, History, Running, Reopen, Stop. Mux terminology stays implementation-internal.
- Make agent identity unmistakable through icon, title, project, and runtime state.
- Keep active work prominent and settled history compact without hiding runtime badges.
- Preserve granular user intent: focus, settle, unsettle, stop, resume, and refresh are distinct actions.
- Design one intentional native visual system, not a collage of worker preferences. It must be refined, compact, accessible, and credible beside a high-performance terminal.
- Treat keyboard navigation, DPI, IME, screen-reader semantics, focus routing, and error/empty states as product behavior, not polish.

## Performance rules

- Measure release builds against the pinned stock WezTerm baseline at the same commit and environment.
- Idle CPU should remain approximately stock when no terminal output or UI event occurs.
- The initial candidate budget is no more than roughly 20 MiB idle RSS over stock; this is a measurement target, not a claim.
- Switching an already-live conversation must not serialize or replay its full terminal state.
- Sidebar data access is paginated and stable; 10,000 metadata rows must remain responsive without loading transcripts.
- Off-screen history owns no render widgets or terminal state.
- No continuous redraw when terminal and chrome state are unchanged.
- Geometry has one source of truth consumed by row/column sizing, rendering bounds, mouse mapping, selection, hyperlinks, cursor/IME, scrollbar, and resize propagation.

## Protected demo corridor

1. Launch the native Windows application.
2. Start the deterministic mock coding agent from `+`.
3. Observe the correct icon and Working state.
4. Close the UI while the tray host and exact mock process/pane continue.
5. Reopen and observe the same terminal.
6. Enter `/new`; confirm a new native ID, create a second durable row, and atomically rebind the same pane/process.
7. Settle the working conversation without changing its runtime.
8. Open usage and display quota and context as separate, source-labeled values with stale-on-open refresh.
9. See and control the same catalog from the Cursor/VS Code extension.
10. Restart/reconcile and resume one historical conversation.

Everything else is subordinate until this corridor is green. If the integrated renderer seam misses its bounded gate, use the documented native controller-window fallback without changing the host, protocol, catalog, adapters, or extension.

## Controller and worker authority

- The primary Codex session is the conductor. It owns scope, contracts, task graph, worktrees, review assignment, integration gates, and final acceptance.
- The conductor edits only `.orchestrator/**`, `docs/**`, and task-control metadata.
- Product implementation and integration occur in separate, isolated worktrees.
- Workers produce candidates and receipts. They cannot mark themselves accepted or merge themselves.
- Every product commit is reviewed by a different model family before integration.
- Claims are evidence-graded: controller-verified, worker-reported, or unproven.
