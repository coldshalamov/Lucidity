# Initial Repository and Control-Plane Audit

## A. Live repository

- This checkout is the WezTerm repository root, not a wrapper repository.
- Pinned base: `4b1c3c151eb530e569f867e1461693c56fe89695` on `main`, matching audited origin/upstream heads.
- Cargo metadata reports 67 packages; 45 are root-level siblings. Generic `crates/` and `apps/` conventions do not exist here.
- All recursive submodules were initialized at recorded commits.
- No `AGENTS.md`, lease, queue, pre-existing orchestrator, or extra worktree was found.
- `HACKATHON/` is user-owned untracked input. `.orchestrator/` is the conductor's control surface. No product source has been edited.
- Root MIT license and bundled font notices were read.
- Windows MSVC Rust and Visual Studio build tools are present. Stock build attempt 1 proved the only immediate blocker was missing Perl in vendored OpenSSL; a verified portable Strawberry distribution supplied it and the identical release build then succeeded at the pinned HEAD.
- A packaged native launch was observed at the same HEAD through WezTerm's own mux: pane output and injected terminal input round-tripped, the native window capture showed both markers, the stock window directly reported PerMonitorV2, direct resize propagated to native rows/columns and pixels, all controller-owned launch trees closed cleanly, and an idle process-tree sample was recorded. Mouse selection and system clipboard remain explicit deferred manual acceptance items rather than implementation prerequisites.

## B. Wave 1 dependency graph

The architecture attacks are adjudicated. The corrected ownership graph is:

```text
stock build + topology evidence + Fable/Kimi/Opus reviews
                            ↓
              control-plane freeze commit
                            ↓
             AT-099 baseline candidate
                            ↓
         AT-100 Fable review + controller acceptance
        ┌──────────────┬──────────────┬──────────────┐
        ↓              ↓              ↓              ↓
  AT-105 close     AT-107 Job    AT-103 adapter   AT-104 extension
   foundation         owner          + mock        candidate held
        └──────────────┴──────────────┘
                            ↓
                 AT-106 typed mux exit
                            ↓
                    AT-102 host/catalog
                            ↓
                 AT-101 viewport Gate 1
                            ↓
                  AT-108 tray + Gate 2
```

Shared Cargo/workspace files and GUI/native/mux lifecycle hot zones are serialized mutexes. Wave 1 workers own disjoint directories and do not merge themselves. ADR-002 uses the existing SetUserVar alert path; ADR-003 assigns Windows Job Object ownership explicitly.

## C. Four implementation packets

- `TASKS/AT-101.md`: stock-preserving `wezterm-gui` library and viewport seam.
- `TASKS/AT-102.md`: host, catalog, runtime state, tray-controller abstraction, and local IPC without GUI hot-zone edits.
- `TASKS/AT-103.md`: adapter contract, event reducer, deterministic mock harness, and fixtures.
- `TASKS/AT-104.md`: frozen native UI specification plus fixture-host Cursor/VS Code extension skeleton.

AT-093/094 are complete and controller-adjudicated. The control plane commits first and AT-099 creates the accepted baseline. AT-105/107/103 then run on disjoint leases; AT-106 follows their reviewed integration, AT-102 follows the exit seam, AT-101 owns Gate 1, and AT-108 performs the final tray binding. The deferred manual clipboard receipt does not delay those lanes.

## D. Architecture attack

`TASKS/AT-090.md` is the exact Fable 5 Extra High assignment. It attacks—not decorates—the root-sibling topology, GUI library seam, viewport geometry, close-to-tray ownership, domain state, event ordering, IPC/security, accessibility, and verification plan. Fable has no write authority.

Two successful Kimi K3 provider packets independently reconstructed topology and lifecycle contracts; a third requested endpoint failed with a preserved zero-token provider-unavailable receipt. The successful audits are accepted in `REVIEWS/AT-091R-kimi-topology.md` and `REVIEWS/AT-092-kimi-lifecycle.md`. Their agreement is useful evidence but not cross-family review.

Both exact Claude Opus 5/xhigh passes are complete. The controller read the full 986-line v1 and 947-line v2, verified their receipts and critical live-source claims, struck v2's unsupported advisor-call preamble, reconciled ADR-004, and froze the accepted result in `.orchestrator/DESIGN/NATIVE_UI_CONTRACT.md`. The retained direction is the native State Rail/Weld system, full-bleed typed hit coverage, one authoritative layout, conflict-aware minimal shortcuts, one internal-stack modal, and zero scheduled Lucidity animation.

## E. Merge and verification order

1. AT-099 baseline workspace/protocol/resource candidate, then mandatory AT-100 Fable review and controller acceptance.
2. AT-105 close/hide foundation, AT-107 Job owner, and AT-103 deterministic event/mock contract on disjoint bases, each different-family reviewed; AT-104 may produce a reviewed extension fixture candidate that remains held.
3. AT-106 typed mux exit signal from the integrated foundations.
4. AT-102 host/catalog/runtime implementation against accepted event/exit/job contracts.
5. AT-101 viewport seam and native Gate 1.
6. AT-108 tray binding and mock Gate 2 with exact process/pane/tree receipts.
7. Integrate the accepted extension fixture and then bind it to the accepted host protocol.
8. Product-shape tasks one accepted commit at a time.

Every branch receives a different-model review. The controller reproduces findings and tests before integration. A contract disagreement creates an ADR; it does not create an integration-branch adapter hack.

## F. First visible demo

The protected corridor in `CHARTER.md` is the release story: native launch, mock agent, visible status, close-to-tray persistence, same-pane reopen, `/new` identity rebind, settle-without-stop, separate usage/context, Cursor mirror, and restart/resume.

The fallback is a separate lightweight native controller window beside stock/forked WezTerm using the same Rust host, catalog, mock, protocol, and extension. Electron/Tauri/WebView remain forbidden even in fallback.
