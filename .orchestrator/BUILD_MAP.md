# Build Map

State vocabulary: `blocked`, `ready`, `running`, `review`, `accepted`, `integrated`, `verified`, `abandoned`.

## Dependency graph

```text
AT-001 stock Windows baseline
  ├── AT-080 live topology audit
  ├── AT-090 Fable architecture attack
  ├── AT-091 Kimi topology audit A
  ├── AT-092 Kimi topology audit B
  └── AT-093/094 Opus native design iterations
              ↓
        controller control-plane freeze commit
              ↓
        AT-099 contract/package/resource baseline
              ↓ AT-100 Fable cross-family baseline review + controller acceptance
          ├── AT-105 close/hide/reconcile foundation
          ├── AT-107 Windows Job Object ownership
          ├── AT-103 adapters/events/mock harness
          └── AT-104 extension fixture candidate (reviewed, held for later integration)
              ↓ product foundations integrated/reviewed; AT-104 held
          AT-106 typed pane-exit signal
              ↓ exit signal integrated and reviewed
          AT-102 host/catalog/runtime/IPC
              ↓ host/mock/foundations integrated
          AT-101 GUI library + serialized product path dependency + viewport seam
              ↓ different-family reviews
        Gate 1 viewport proof + mock contract receipts
              ↓ AT-108 tray/reopen/Quit binding
              ↓ controlled integration, one commit at a time
        Gate 2 mock live session and tray lifecycle
              ↓
        AT-201 sidebar/query model
        AT-202 first real history/launch/resume adapter
        AT-203 usage/context
        AT-204 extension host integration
              ↓
        Kimi whole-system audit + Fable system review
              ↓
        protected demo verification
```

## Current tasks

| ID | Owner/surface | State | Unlocks | Current evidence |
|---|---|---:|---|---|
| AT-001 | Controller, local Windows | accepted for implementation | all implementation | Release build plus native launch, mux output/input, resize propagation, DPI, screenshots, clean close, and matched idle metrics are direct; manual mouse/system clipboard acceptance is deferred |
| AT-080 | separate Codex/Sol read-only session | accepted | topology candidate | 67-package live-tree audit; no edits; root-sibling topology and exact GUI hot zone returned |
| AT-090 | Claude Code / Fable 5 xhigh | accepted | contract freeze | `PROCEED_AFTER_FIXES`; controller reproduced blockers and amended provisional contracts/tasks |
| AT-091 | OpenCode / Kimi K3 provider A | blocked | provider evidence only | Exact OpenCode Go endpoint returned unavailable with zero tokens and zero diff |
| AT-091R | OpenCode / direct Kimi for Coding K3 | accepted | independent topology attack | Clean exact-route audit accepted; topology, bootstrap, geometry, and native validation corrections folded |
| AT-092 | OpenCode / Kimi K3 provider B | accepted | dependency/lifecycle attack | Exact ClinePass Kimi audit accepted; event, Job, identity, exit, pagination, and crash contracts corrected |
| AT-093 | Claude Code / Opus 5 xhigh | accepted | visual direction | Full native v1 read and adjudicated; exact-model receipt and controller rulings recorded |
| AT-094 | Claude Code / Opus 5 xhigh fresh session | accepted | adversarial design refinement | Full v2 read and controller-adjudicated; corrected native contract frozen in-repository |
| AT-099 | separate Codex/Sol baseline implementer | ready | Wave 1 foundations | Design and control plane are frozen; dispatch from the committed control-plane parent |
| AT-100 | Claude Code / Fable 5 xhigh read-only reviewer | blocked | accepted Wave 1 parent | Depends on returned AT-099 candidate; controller reproduces and accepts before fan-out |
| AT-105 | Claude Fable 5 xhigh lifecycle owner | blocked | safe Agent window lifetime | Depends on AT-099; exact-model isolated mutation, then different-family review |
| AT-106 | Claude Code / Fable 5 xhigh | blocked | deterministic runtime exit truth | Depends on integrated AT-099/105/107/103; serialized mux/GUI notification lease; different-family review required |
| AT-107 | Codex/Sol Max Windows/Rust implementer | blocked | owned process-tree lifecycle | Depends on AT-099; isolated PTY lease, then different-family review |
| AT-101 | separate Codex/Sol GUI implementer | blocked | Gate 1 | Depends on integrated AT-099/103/102/105/106/107 and ADR-004's serialized path-dependency lease |
| AT-102 | Claude Code / Fable 5 xhigh | blocked | host vertical slice | Depends on integrated AT-099/103/105/106/107 contracts; different-family review required |
| AT-103 | native Kimi Code / managed K3 256k | blocked | deterministic lifecycle tests | Depends on AT-099; exact native harness/provider verified locally |
| AT-104 | separate Codex/Sol Max | blocked | extension fixture path | Depends on AT-099 and accepted in-repository design; different-family review required |
| AT-108 | separate Codex/Sol Max Windows integrator | blocked | Gate 2 close/reopen/tray proof | Depends on accepted Gate 1 plus host and Job APIs; different-family review required |

## Gates

### Gate 0 — pinned stock baseline

- Exact `HEAD` recorded.
- Submodules clean and initialized.
- Native Windows release build succeeds with the documented toolchain.
- Stock binary starts and basic terminal input/selection is observed.
- Real mouse selection plus system clipboard copy/paste remains a named deferred manual acceptance receipt and is not inferred from automation.
- Baseline binary size, idle CPU, working set/private bytes, and startup notes recorded.

### Gate 1 — stock-preserving viewport seam

- `wezterm-gui` library extraction preserves the stock binary.
- Agent binary renders a fixed header/sidebar and a bounded terminal viewport.
- Geometry, input, DPI, selection, wheel, hyperlinks, cursor/IME, splits, wide glyphs/images, maximize/restore, and tiny windows have direct receipts.
- Chrome clicks never leak into terminal cell `(0,0)`.
- Two mux tabs select through two sidebar rows.

### Gate 2 — mock live session and tray host

- Deterministic mock launches under ConPTY and emits structured state.
- The structured state traverses ADR-002's real SetUserVar→mux→host path, not a direct decoder test.
- Closing UI does not call the existing mux-kill path.
- Exact pane, process, and output sequence survive close/reopen.
- Explicit Stop and tray Exit terminate the owned process tree only.

### Gate 3 — catalog and organization

- Fixture histories import idempotently without PTYs.
- 10,000-row query and view-model checks meet measured budgets.
- Settle/unsettle does not alter runtime or process state.

### Gate 4 — native identity transition

- Mock `/new` confirms a new native ID.
- One pane/process is atomically rebound to a second durable conversation.
- Duplicate/out-of-order events do not duplicate records or attachments.

### Gate 5 — one real harness

- Installed executable detected.
- Native TUI launches unmodified.
- Native ID captured, recent history imported, one status transition observed, and resume verified.

### Gate 6 — Cursor/VS Code client

- One catalog appears in both clients.
- New-in-workspace, focus, settle/unsettle, stop, and usage commands round-trip through the host.

### Gate 7 — usage/context

- Account quota and conversation context remain separate.
- Stale-on-open and manual refresh work with source, confidence, and sample age.
- No idle provider call or hidden TUI injection occurs.

## Integration order

1. Create `codex/lucidity-control-plane` and commit only the accepted `.orchestrator/**` files. Keep user-owned `HACKATHON/**` untracked and out of the commit. This commit changes no product source and becomes AT-099's launch parent; record its exact SHA in the external ledger and worktree receipt, never self-referentially inside the same commit.
2. AT-099 baseline workspace/protocol/resource candidate commit.
3. AT-100 performs a fresh exact-Fable read-only review; the controller reproduces findings and accepts/integrates AT-099 before any dependent fan-out.
4. AT-105 close/hide/reconciliation foundation, AT-107 Job policy, and AT-103 deterministic event/mock work may run on disjoint paths; AT-104 may build its independent extension fixture candidate. Integrate product foundations one accepted commit at a time after different-family review; hold AT-104 until its protocol/client integration point.
5. Dispatch AT-106 from that integrated base, after AT-105 releases the exhaustive GUI notification match sites; review and integrate its typed exit signal.
6. AT-102 catalog/host state using the integrated event, exit, and process-owner contracts.
7. AT-101 library/bootstrap/viewport seam after AT-102/103 and the native foundations are integrated; its first isolated manifest commit adds only ADR-004's existing GUI path dependency.
8. AT-108 tray/reopen/Quit binding and integrated Gate 2 vertical slice.
9. Integrate the accepted AT-104 extension fixture skeleton, then bind it to the accepted host protocol.
10. Wave 2 features one accepted commit at a time.

No branch is merged because another branch is waiting. If contracts disagree, integration stops for an ADR.
