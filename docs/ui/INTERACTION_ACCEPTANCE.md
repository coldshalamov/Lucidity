# Interaction acceptance scripts

## Gate 2 desktop workflow

1. Launch Lucidity **without** `--demo` / `LUCIDITY_DEMO=1`.
2. Confirm welcome UI and **no** auto-created mock row.
3. Click `+ New` → choose agent → pick project → Create.
4. Confirm sidebar row + terminal shows native TUI or missing-executable error.
5. Create second session; click rows to switch; no duplicate attached runtime.
6. Settle a working row → appears under History; process still running.
7. Stop a row → runtime ends; row remains; Resume works.
8. Open Settings; change a terminal or agent path control; observe apply/persist.
9. Open Usage; see dual surfaces or unavailable state.
10. Close window → tray remains; reopen → same pane.
11. Optional: `LUCIDITY_DEMO=1` still launches mock for deterministic demos only.

## Input matrix (Gate 1+)

GUI text entry, GUI Ctrl+C/V, terminal Ctrl+C selection vs signal, Escape closes GUI popup first, terminal mouse selection, DPI 125/150/200%.
