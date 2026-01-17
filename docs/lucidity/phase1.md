# Phase 1: Local Mirroring Proof (Desktop Host Bridge)

Phase 1 proves that a remote client can control a *live desktop pane* by sending input bytes into the same PTY and receiving the raw PTY output stream.

## What it does

- Lists panes from the running GUI process
- Attaches to a specific pane ID
- Streams the pane’s **raw PTY output bytes** to the client
- Accepts client input bytes and writes them into the same PTY

This means:
- The desktop window and the client both see the same session in real time.
- Typing in the client shows up on desktop (because the PTY echo / program behavior drives display).

## How it works (implementation note)

We hook the byte stream at `mux/src/lib.rs` in the `read_from_pane_pty(...)` loop:

- WezTerm already reads PTY output bytes in a dedicated thread
- Lucidity broadcasts a copy of those bytes to subscribers (`Mux::subscribe_to_pane_pty_output`)

The host server is started inside `wezterm-gui` and exposes a TCP-framed protocol.

## Running (dev)

Default listen:
- `127.0.0.1:9797` (localhost only)

Disable the embedded host server:

```powershell
$env:LUCIDITY_DISABLE_HOST = '1'
```

Enable LAN (may trigger firewall prompts):

```powershell
$env:LUCIDITY_LISTEN = '0.0.0.0:9797'
```

Connect with the test client:

```sh
cargo run -p lucidity-client -- --addr 127.0.0.1:9797
```

## Limitations (by design for Phase 1)

- No encryption (localhost-only by default).
- No session list UI; client attaches by pane id.
- No quotas/subscriptions.

Pairing (Phase 3) exists as a local-only MVP (QR/code + local trust store), but it is not used to gate Phase 1 TCP connections yet.

