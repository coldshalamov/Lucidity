# AT-090 Controller Review — Fable Architecture Attack

## Verdict

Accepted as `PROCEED_AFTER_FIXES` at base `4b1c3c151eb530e569f867e1461693c56fe89695`.

The worker did not approve architecture; it falsified four parts of the Wave 0 draft. The controller reproduced the source-specific mechanisms before accepting the findings. No product implementation may start until the accepted corrections are reflected in contracts and packets.

## Run receipt

- Claude Code 2.1.220, exact model `claude-fable-5`, effort `xhigh`.
- Session `543d2638-eff3-4be0-ad36-f3070edcead2`, exit 0, 643.788s wall time.
- 69 turns; 77 uncached input tokens, 176,791 cache-creation input tokens, 3,621,227 cache-read input tokens, and 41,615 output tokens; reported cost USD 9.238567.
- Exactly 43 Read, 6 Glob, and 19 Grep calls. Edit, Write, Bash, web, MCP, subagents, and model switching were unavailable.
- Raw artifact: `C:\Users\93rob\.codex\delegations\lucidity-hackathon-20260806\AT-090\stdout.stream.jsonl`.
- Full worker report: `C:\Users\93rob\.codex\delegations\lucidity-hackathon-20260806\AT-090\final-verdict.md`.
- Before/after: same HEAD, no tracked or staged diff. Concurrent untracked changes were controller-owned `.orchestrator/**`; the worker's authoritative operation log contains no write-capable call.

## Controller reproduction

The controller directly re-read the cited symbols and confirmed:

1. `window/src/os/windows/window.rs::hide` schedules minimize, while `WM_CLOSE` emits `CloseRequested` and suppresses default close.
2. `TermWindow::close_requested` kills the mux window before closing. `GuiFrontEnd::forget_known_window` immediately reconciles, and `reconcile_workspace` creates a new `TermWindow` for a still-live mux window with no GUI mapping. A naive close-without-kill therefore has a concrete respawn path.
3. `wezterm-gui` has no library target today. Its `Cargo.toml` is a mutex file, so AT-099 must create the library skeleton before AT-101 begins.
4. `wezterm-gui/build.rs` embeds Windows resource ID `0x101`, the PerMonitorV2/UTF-8 manifest, version metadata, and provisions ConPTY/ANGLE/Mesa files. The window crate hardcodes the same icon ID and the PTY layer prefers sideloaded ConPTY. A separate product binary needs an explicit equivalent resource contract and direct native verification.
5. `Opt::parse` requests `config::wezterm_version()` before `env_bootstrap::bootstrap()` assigns it, reproducing the standalone GUI version anomaly.
6. Resize increments and the Win11 maximize-button/snap-layout rectangle are real geometry consumers omitted from the first `AppLayout` list.
7. No Windows UI Automation or IAccessible provider exists in the inspected window/GUI sources. Both renderers also lack a current shared terminal scissor, and the glyph path still contains an explicit clipping TODO.

Commands were read-only `rg` searches plus line-bounded `Get-Content` over the exact cited sources. `git status`, `git diff --stat`, and `git diff --check` continued to show no tracked product change.

## Disposition

**Supersession note (controller, 2026-08-06).** The two sequencing bullets below are preserved as historical review evidence but are superseded by ADR-004 and the current Build Map: AT-105 now lands its generic close/hide foundation before Gate 1 and does not own tray/Quit; AT-099 does not create a `wezterm-gui` library skeleton, because AT-101 must first create the real auto-discovered library target before adding the product path dependency.

Accepted corrections:

- Add an exclusive AT-105 lifecycle task after Gate 1. It owns the close policy hook, reconciliation suppression, true Win32 hide, tray binding, and exact Quit path.
- AT-099 must pre-create the `wezterm-gui` library target/skeleton and all shared `agent-terminal` module/manifests/resources before parallel workers.
- Add resize increments and the Win11 caption/maximize hit rectangle to the one-layout contract.
- Scope attachment generations to a host instance and reject stale old-native-ID end events after `/new` rebinding.
- Freeze bounded length-prefixed pipe framing, owner-only local security, subscriber backpressure, and full-snapshot reconnect semantics before host work.
- Give `agent-terminal` an explicit Windows resource/application-identity/version-order contract and direct PerMonitorV2 receipt.
- Add the native/player-visible verification cases from the report, including a debounce window-count check after close-to-tray and chrome-on idle invalidation metrics.
- Record a protected-demo-only accessibility deferral in ADR-001 while preserving keyboard, focus, contrast, reduced-motion, semantic view-model, and post-demo UIA obligations.

Qualified findings:

- The worker's statement that a separate binary necessarily loses every dependency build artifact is treated as a release risk, not assumed fact. AT-099 must implement the explicit product resource path; controller acceptance requires PE resource, process DPI-awareness, icon, DLL-provisioning, and application-identity receipts.
- Node named-pipe feasibility and the best reconciliation-suppression mechanism remain hypotheses until their platform/spike tests.

Rejected findings: none.

## Remaining gate

AT-090 is accepted, but contract freeze still waits for the independent Kimi lifecycle audit and the explicit AT-091R provider reroute. Opus design may begin only after the controller publishes the corrected architecture/accessibility inputs.
