# ADR-003 — Windows Process-Tree and Crash Ownership

Status: accepted architecture; implementation remains gated.
Date: 2026-08-06
Decision owner: controller after AT-092 source reproduction.

## Context

The stock Windows PTY path creates a process normally and `WinChild::kill` calls `TerminateProcess` on only that direct process. A coding-agent harness can spawn children or grandchildren, so direct-child termination cannot satisfy Stop, Quit, or hard-host-crash behavior. The protected demo also uses a single native process for host and GUI, which constrains what “UI crash while host remains” can honestly mean.

## Decision

- Stock launches retain the current direct-child policy unchanged.
- Agent-owned Windows launches opt into an `OwnedJob` process-tree policy carried explicitly by the PTY command/spawn contract.
- `OwnedJob` creates a Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, creates the root process suspended, assigns it to the job, then resumes the main thread. If creation, assignment, or resume fails, launch fails and cleans up; it never falls back to an unowned process.
- The job handle lives as long as the PTY child/killer ownership. Explicit Stop and tray Quit terminate the Job Object. Normal UI hide does not drop the handle. A hard host-process crash closes the last job handle and terminates the owned tree.
- Nested-job, breakaway, access-denied, and partial-start behavior must be tested on Windows. An unsupported environment produces a visible launch failure rather than weakened ownership.
- In the single-process MVP, “UI crash with host survival” means native window/render-context destruction or loss while the process and host loop survive. An unhandled process crash is a host crash and is tested as such. A separately fault-isolated UI process is not claimed.

## Consequences

- AT-107 exclusively owns the Windows PTY/Job Object spawn and termination seam. AT-106 separately owns the pane-correlated, typed mux exit publication before `PaneRemoved`. AT-102 consumes the public Job policy and maps AT-106's exit event into host/protocol runtime views; it invents neither a second process owner nor a second exit signal.
- AT-105 owns close/hide/reconciliation policy before viewport chrome. AT-108 later binds the tray icon, reopen, and explicit Quit to the accepted host and job-tree APIs.
- Gate 2 must use a mock parent, child, and grandchild; prove close/reopen preserves them; prove Stop/Quit and hard host crash remove the owned tree; and prove stock launches keep stock behavior.
