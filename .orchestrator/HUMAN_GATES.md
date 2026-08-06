# Human Verification Gates

## HG-001 — Stock mouse selection and system clipboard

Status: deferred manual acceptance; not on the implementation critical path
Owner: user performs; controller prepares the pinned stock window and records the receipt.
Base: `4b1c3c151eb530e569f867e1461693c56fe89695`

### Active witness instance

- Launched from the pinned stock release tree; no foreign mux/window was reused.
- `wezterm.exe` PID 11388; `wezterm-gui.exe` PID 18288; pane `cmd.exe` PID 30812; `OpenConsole.exe` PID 39060.
- Native GUI HWND 46794136; mux window/tab/pane IDs are each `0`.
- Exact socket: `C:\Users\93rob\.local\share\wezterm\gui-sock-18288`.
- Copy marker: `LUCIDITY_GATE0_COPY_8BA12898BF93`.
- Paste marker: `LUCIDITY_GATE0_PASTE_8BA12898BF93`.
- Current state: no human witness was returned and the paste marker never appeared in mux text. At `2026-08-06T11:29:21-04:00`, the controller sent `CloseMainWindow` only to the owned GUI PID; it was accepted and PIDs 11388, 18288, 30812, and 39060 all exited. These nonces are retired and must not be reused.

This receipt remains human because the available Windows-control skill explicitly forbids automating terminal applications. Mux text injection, `get-text`, screenshots, and Win32 resize do not substitute for mouse selection or the system clipboard. The missing manual receipt is recorded honestly but does not delay implementation, builds, automated input/clipboard tests, or later native acceptance.

### Copy direction: terminal → system clipboard → chat

1. Controller launches an isolated pinned stock WezTerm window and prints a fresh one-time `LUCIDITY_GATE0_COPY_<nonce>` marker.
2. User selects exactly that marker with the mouse and copies it using the stock terminal interaction.
3. User pastes the marker into this Codex task. The controller checks exact equality and records the user message as the witness; the controller does not read the ambient Windows clipboard.

### Paste direction: chat → system clipboard → terminal

1. Controller provides a separate one-time `LUCIDITY_GATE0_PASTE_<nonce>` marker in this task.
2. User copies it from the task, pastes it into the controlled stock terminal using the stock terminal interaction, and submits it.
3. Controller uses the native mux `get-text` path to verify the exact marker appeared in the owned pane.

### Closeout

- Record window/pane/process IDs, exact markers, user-witness timestamp, mux output, and clean-close receipt in `STATUS.md` and `VERIFICATION.md`.
- Close only the controller-owned launch tree and verify zero descendants.
- Keep HG-001 unchecked until a later controlled manual acceptance run supplies the exact witness. Do not infer it, and do not make unrelated implementation wait for it.
