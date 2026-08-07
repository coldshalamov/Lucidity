# Baseline B215 — pre-V2 product shell

**Commit:** `b215b572dbe778fa634a9ee973a7d0d3ebab18c8`  
**Tag:** `lucidity-v1-baseline-b215b572d`  
**Date recorded:** 2026-08-07

## What this baseline is

The last accepted “native product shell” state before V2 GUI recovery:

- Hand-painted Lucidity chrome (title bar, Active/Settled sidebar, status tokens)
- Automatic mock-agent session seed on ordinary launch
- Host catalog, tray close-to-tray, named-pipe IPC, adapter package foundation retained for V2

## User-visible defects (acceptance drivers for V2)

- No real `+ New` dialog / agent picker / project folder picker
- Product chrome reads as terminal labels, not a desktop GUI
- Ordinary launch auto-opens mock-agent into what often appears as an empty shell
- Settings / usage are not real graphical surfaces
- Fixed ~25 ms host action pump and SQLite-on-render-path debt

## Measurements

Populate at capture time (Gate 0):

| Metric | Value | Notes |
|---|---|---|
| Release `agent.exe` size | _pending package build_ | `scripts/package-windows.ps1` or `cargo build --release -p agent-terminal` |
| Cold startup | _pending_ | Stopwatch launch → first paint |
| Idle CPU (60s) | _pending_ | Owned process tree |
| RSS with mock | _pending_ | Working set after mock attach |
| Screenshot | `docs/baselines/` or scratch `ui-review/` | Current hand-painted UI |

## Recovery start

V2 work proceeds on `lucidity/v2-gui-shell` from current `main` (includes post-baseline launcher/docs commits) while treating `b215b572d` as the architectural baseline named in the recovery plan.
