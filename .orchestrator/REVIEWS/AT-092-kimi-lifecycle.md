# AT-092 Controller Review — ClinePass Kimi Lifecycle Audit

## Verdict

Accepted with one recorded narration correction at base `4b1c3c151eb530e569f867e1461693c56fe89695`.

The audit falsified the proposed event transport, proved that direct-child process ownership is insufficient, and exposed missing deterministic identity, exit, pagination, and crash semantics. The controller reproduced the critical source paths before accepting them.

## Run receipt

- OpenCode 1.18.9, exact model `cline-pass/cline-pass/kimi-k3`, variant `max`, `plan` agent.
- Session `ses_02a08f5e1ffeyxVZ3jlOPRRX7p`; one continuation of the same session after the controller wrapper timed out; final exit 0.
- Across 53 step-finish events: 528,997 input, 17,564 output, 23,356 reasoning, 4,028,160 cache-read, zero cache-write; reported aggregate cost USD 3.409239. Repeated context/cache accounting makes the reported 4,598,077 total non-unique.
- No edits, subagents, web, model switching, or tracked/staged diff.
- Full audit: `C:\Users\93rob\.codex\delegations\lucidity-hackathon-20260806\AT-092\audit.md` (SHA-256 `CF101137C2AFC90DA5F5272DEF8C5F279E9F5FAD119F19B175750C7255A42D82`).

The sealed worker audit says its Cargo metadata command used `--frozen`. Authoritative raw JSON shows `cargo metadata --no-deps --format-version 1 --locked --offline`. The locked/offline and clean-diff claims stand; only the flag spelling in worker prose is corrected here.

## Controller reproduction

**Supersession note (controller, 2026-08-06).** Item 4 below correctly identifies the lost exit data but its AT-107 assignment is historical. ADR-003 and the frozen task map now assign PTY/Job ownership to AT-107, typed mux exit publication to AT-106, and host/protocol mapping to AT-102.

1. `wezterm-escape-parser/src/osc.rs` maps 777 to `RxvtExtension`; `term/src/terminalstate/performer.rs` handles only `params[0] == "notify"` and otherwise does nothing. `vtparse` tracks at most 64 OSC fields. The original raw-JSON OSC 777 proposal cannot reach the host.
2. The existing OSC 1337 `SetUserVar` path already base64-decodes a value, emits `Alert::SetUserVar`, and `mux/src/localpane.rs` forwards it with the emitting pane ID. ADR-002 selects that smaller stock-preserving route.
3. `pty/src/win/psuedocon.rs` calls `CreateProcessW` without suspended/job flags. `WinChild` and its cloned killer use `TerminateProcess` on the direct process. No Job Object API exists in the Rust tree. ADR-003 and AT-107 now own the missing atomic tree policy.
4. `PaneRemoved(PaneId)` has no exit data. `LocalPane::is_dead` observes `ExitStatus`, converts it to display text/state, and removal later discards it. The current frozen owner is AT-106, which must publish a typed one-shot exit signal before removal; AT-107 owns only the PTY/Job seam.
5. The controller re-confirmed close-kill, minimize-not-hide, reconciliation respawn, and mux-empty termination paths already recorded by AT-090.
6. Tab positions are mutable while stable `TabId` lookup exists. Sidebar actions must carry `TabId` and resolve it at dispatch time.

## Disposition

Accepted and frozen before Wave 1:

- ADR-002 replaces OSC 777 with the bounded reserved SetUserVar path.
- ADR-003 adds an Agent-only atomic Windows Job Object policy; stock launch semantics remain unchanged.
- A one-candidate `PendingIdentity` state, supersession/expiry/crash rules, stable evidence IDs, and deterministic old-conversation transition are part of the host contract.
- A new host instance invalidates every persisted live attachment until current evidence reattaches it.
- Natural/killed/non-zero exits must reach the host before pane removal.
- Catalog pagination is keyset-based on `(last_activity_at, conversation_id)` with a snapshot version; corruption is quarantined/backed up before deterministic rebuild.
- Named-pipe security validates the connected client token/SID in addition to server DACL and remote rejection. Events are edge hints, never a durable journal.
- Sidebar activation is by `TabId`, never a stored positional index.
- In the single-process MVP, window/context loss with process survival is the UI-crash test; process death is host crash.
- Usage demo data arrives as a structured mock event. No hidden TUI command transaction is allowed.

Qualified:

- A non-guessable pipe name is not treated as a security control. Lucidity uses a per-user SID-derived name plus explicit DACL, remote rejection, and client-token SID validation; authorization never depends on obscurity.
- The audit recommended a lifecycle prototype before Gate 1. The controller splits lifecycle ownership: AT-105 lands close/hide/reconciliation foundations before viewport chrome, while AT-108 binds tray/reopen/Quit after host and Job Object APIs exist.

Rejected: none of the reproduced critical findings.

## Remaining gate

Architecture review is now sufficient for Opus design. Product implementation still waits on Opus v1/v2, frozen packet updates, and the named human Gate 0 mouse/system-clipboard receipt.
