# AT-095 Controller Review — Pre-Freeze Control-Plane Audit

Disposition: **accepted; all nine findings reproduced and corrected**
Date: 2026-08-06
Mode: separate Codex/Sol read-only audit; no source/product edits

## Receipt

The auditor read the current `.orchestrator/**` control plane, re-snapshotted after noticing a concurrent controller correction to AT-108, verified the live HEAD/status/worktree list, and returned findings-first. It did not access `HACKATHON/**`, inspect processes, or edit files.

Verified state at return:

- HEAD `4b1c3c151eb530e569f867e1461693c56fe89695` on `main`;
- only `.orchestrator/` and protected `HACKATHON/` untracked;
- one registered worktree and no planned branch refs/worktree directories;
- AT-094 artifact/receipt hashes matched and HG-001 remained open.

## Findings and controller corrections

1. **Exit owner conflict:** ADR-003 incorrectly assigned host exit publication to AT-107. Corrected: AT-107 owns PTY/Job, AT-106 owns typed mux exit-before-removal, and AT-102 maps it into host/protocol views. Historical reviews carry explicit supersession notes.
2. **Compile-unsafe AT-099 handoff:** later workers could not legally create missing top-level modules or repair the binary wrapper. Corrected: AT-099 creates compile-only top-level `mod.rs` files and a thin immutable wrapper calling `agent_terminal::chrome::run() -> anyhow::Result<()>`; directory owners may replace only their leased compile stub.
3. **Impossible AT-105 product probe:** product intent/dependency wiring does not exist at its base. Corrected: AT-105 proves a test-only, product-agnostic generic GUI policy within its lease; Gate 2 product/tray evidence remains AT-108.
4. **Self-referential base SHA:** AT-099 could not embed its containing control-plane commit SHA. Corrected: task packets hold base criteria; exact launch SHAs are asserted in the external ledger and worktree receipts.
5. **Superseded status/sequencing:** current authority files and historical reviews mixed old Opus/AT-105/AT-099/Grok assumptions. Corrected current status and added explicit historical supersession notes without rewriting sealed worker evidence.
6. **Narrow Usage unreachable:** the full chip disappeared below 900 pixels without an exact trigger. Corrected: one stable `lucidity.titlebar.usage` target renders a full chip at 900+ and a compact `USG` button below; focus returns to the same semantic target across resize.
7. **Ambiguous AT-108 window/path lease:** corrected to permit exactly one message-only tray HWND, forbid another top-level host window, and require one concrete GUI path in the external ledger/launch receipt before dispatch.
8. **Icon before mark:** froze the original achromatic 16×16 Weld Frame geometry and ICO plane requirements; AT-099 must implement it rather than copy the stock icon.
9. **Missing cross-family baseline review:** added AT-100, an exact Fable 5 xhigh fresh read-only review. No AT-099 dependent fan-out occurs until the controller reproduces that review and accepts/integrates the baseline.

## Freeze condition

With these corrections, the control plane may freeze only after HG-001 closes and a final read-only consistency/status check shows the same pinned product parent, no product diff, and only the intended `.orchestrator/**` stage set. No implementation worker has yet been dispatched.

A narrow follow-up recheck found one remaining historical sequencing mismatch in `AUDIT.md` and AT-080. The controller updated the audit graph/order to match `BUILD_MAP.md` and added an explicit AT-080 supersession note. The auditor then confirmed all other findings, owners, base criteria, leases, AT-100 gate, HEAD/status, and worktree state were consistent; after that correction its verdict was **ready to freeze once HG-001 closes**.
