# Superseded plans

**Status:** historical only — not execution authority  
**Effective:** Lucidity V2 recovery plan adoption  
**Canonical authority:** `docs/LUCIDITY_V2_PLAN.md` (same content as `docs/LUCIDITY_V2_RECOVERY_PLAN.md`)

## Superseded documents

The following are preserved for history and audit. They must not guide new implementation:

| Path | Role when written | Status now |
|---|---|---|
| `HACKATHON/AGENT_TERMINAL_HACKATHON_PLAN.md` | Original architecture / hackathon plan | Historical |
| `HACKATHON/AGENT_TERMINAL_HACKATHON_PLAN (1).md` | Duplicate of the above | Historical |
| `HACKATHON/AGENT_TERMINAL_CONDUCTOR_PROTOCOL.md` | Multi-agent conductor protocol | Historical |
| `HACKATHON/AGENT_TERMINAL_MASTER_CONDUCTOR_PROMPT.md` | Conductor bootstrap prompt | Historical |
| `HACKATHON/AGENT_TERMINAL_PARALLEL_SPRINT_PROMPTS.md` | Parallel sprint track prompts | Historical |
| `.orchestrator/DESIGN/NATIVE_UI_CONTRACT.md` | Hand-painted native chrome contract | Superseded as product UI authority |
| `.orchestrator/CHARTER.md` | Wave-0 product charter | Partially retained for domain invariants; UI authority is V2 |

## What went wrong (brief)

The prior execution plan optimized for a **WezTerm product mode that paints terminal-looking sidebar/status chrome**. That completed a control-plane vertical slice (catalog, tray, IPC, mock harness) but did **not** deliver the user-facing desktop GUI Lucidity is supposed to be.

V2 keeps the useful runtime foundation and replaces the presentation architecture with a real GUI layer (`egui` in the existing WebGPU path).

## Current authority order

1. User product charter (multi-agent coding workspace GUI around native TUIs)
2. `docs/LUCIDITY_V2_PLAN.md` / `docs/LUCIDITY_V2_RECOVERY_PLAN.md`
3. `docs/LUCIDITY_V2_MASTER_CONDUCTOR_PROMPT.md`
4. `docs/LUCIDITY_V2_INITIAL_TASK_PACKETS.md`
5. Accepted ADRs under `.orchestrator/ADRS/` that do not conflict with V2
6. Historical HACKATHON documents (reference only)

## Baseline

- Git tag: `lucidity-v1-baseline-b215b572d` → commit `b215b572dbe778fa634a9ee973a7d0d3ebab18c8`
- Recovery branch: `lucidity/v2-gui-shell`
