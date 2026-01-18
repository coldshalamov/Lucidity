# Lucidity Mobile (Flutter)

This is the Flutter mobile client for Lucidity.

## What works (MVP)

- Connect to a desktop host over TCP (LAN).
- List panes.
- Attach to a pane.
- Render live terminal output using `xterm`.
- Send keystrokes back to the desktop.
- QR pairing (scan the desktop splash QR and request approval).
- Multiple terminal tabs (each tab attaches to a pane).

## Desktop prerequisites

By default, the desktop host listens on `127.0.0.1:9797`, which is **not reachable from your phone**.

To test from a phone on the same LAN, you must make the desktop listen on an external interface:

```powershell
$env:LUCIDITY_LISTEN = "0.0.0.0:9797"
target/debug/wezterm-gui.exe
```

Security warning: anyone on your LAN could connect and inject keystrokes when you do this.

## Internet mode (required for “works from anywhere”)

The “over the internet” version requires a running `lucidity-relay` service and the desktop-side bridge (`lucidity-relay-agent`).

- Start the relay server (cloud/VPS or local dev):
  - `cargo run -p lucidity-relay`
- Start the desktop bridge (or let the desktop app spawn it when configured):
  - Set `LUCIDITY_RELAY_BASE` to your relay WebSocket base, then run `lucidity-relay-agent`

Build the mobile app pointing at the relay:

```powershell
flutter run --dart-define=LUCIDITY_RELAY_BASE=ws://YOUR_RELAY_HOST:9090
```

## Run (developer)

From `d:\GitHub\Lucidity\lucidity-mobile`:

```powershell
flutter pub get
flutter run
```

## Protocol notes

Lucidity frames are:

- 4 bytes: little-endian length = `1 + payload_len`
- 1 byte: type (`1=JSON`, `2=Output`, `3=Input`)
- payload bytes

JSON messages use `{ "op": "..." }` (for example, `{ "op": "list_panes" }`).

## QR pairing

From the Home screen, tap `Scan QR`, scan the QR shown in the desktop Lucidity splash, then approve the pairing on desktop.

In internet mode, pairing and the terminal session are carried via the relay.
