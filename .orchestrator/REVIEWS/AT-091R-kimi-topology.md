# AT-091R Controller Review — Direct Kimi Topology Audit

## Verdict

Accepted with corrections at base `4b1c3c151eb530e569f867e1461693c56fe89695`.

The independent direct Kimi-for-Coding route confirmed the additive root-sibling topology and sharpened the Windows bootstrap/geometry matrix. It also corrected one controller assumption: Cargo auto-discovers a library target when `wezterm-gui/src/lib.rs` exists, so an explicit `[lib]` stanza is not required merely to create the target.

## Run receipt

- OpenCode 1.18.9, exact model `kimi-for-coding/k3`, variant `max`, edit-denied `plan` agent, `--pure`.
- Session `ses_029f73b57ffecSG5s8A9NKauZe`, exit 0, 1,191.5s wall time.
- 111,822 input, 27,774 output, 2,602,752 cache-read, zero reported reasoning tokens and zero reported cost.
- 41 reads, 25 greps, and 8 read-only shell commands; no edits, tasks, web, skills, or model switching.
- Before/after HEAD and status were identical; tracked/staged diff was empty.
- Full audit: `C:\Users\93rob\.codex\delegations\lucidity-hackathon-20260806\AT-091R\audit.md` (SHA-256 `A57FD2D3D27BD774DCEFF687A90DD0642E4190EAF5E1A68154DFD0754A7D3E19`).

## Controller reproduction and disposition

Accepted:

1. The existing root is the upstream WezTerm workspace; the three root-sibling product packages are the smallest justified compile/ownership split.
2. `src/lib.rs` is sufficient for Cargo library auto-discovery. AT-099 therefore does not edit `wezterm-gui/Cargo.toml`; AT-101 owns the library source extraction.
3. Agent bootstrap must parameterize AUMID, class/title, version initialization, main-thread/bootstrap order, and update-check behavior. The controller additionally confirmed `check_for_updates` defaults true unless `distro-defaults` is enabled, so Agent mode must hard-disable it and verify zero updater network activity.
4. The product binary needs its own Windows subsystem/resources and direct PE/process receipts; copied runtime DLL freshness must be checked rather than inferred from file existence.
5. `update_text_cursor` and mouse geometry do not currently share identical border offsets. One `AppLayout` must eliminate, not preserve, that asymmetry.
6. Gate 1 needs stock-derived numeric geometry oracles, real OS input receipts, per-renderer bleed evidence, cross-monitor DPI drag, resize-increment, snap-layout, and IME receipts.
7. The GUI lease must include `termwindow/spawn.rs`, renderer/backend geometry files, and the window-class source when a patch actually touches them.

Qualified:

- The audit reported AT-105 as undefined while the controller was concurrently adding that packet. The current control plane now defines AT-105; the underlying ownership finding is accepted, while the stale “missing task” statement is not current.
- SmartScreen and code signing remain release/distribution concerns outside the protected local demo, not Gate 1 blockers.

Rejected: none of the source-derived architecture findings.

## Remaining gate

AT-091R closes the independent topology review. Contract freeze still requires AT-092 adjudication and the two Opus design iterations; implementation remains blocked by the open human mouse/system-clipboard Gate 0 receipt.
