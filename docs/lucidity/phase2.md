# Phase 2: Mobile MVP (LAN client)

Phase 2 is where Lucidity becomes a real product: an iOS/Android app that can connect to a desktop on the same LAN, render the terminal output locally (ANSI/VT), and send input back to the same PTY.

## What’s already in this repo (foundation you can build on)

- **Phase 1 (implemented):** a TCP host bridge that can list panes, attach to a pane, stream raw PTY bytes, and inject input bytes (`lucidity-host`, `lucidity-proto`, `lucidity-client`).
- **Phase 3 (implemented, local-only MVP):** QR pairing splash + approve/reject dialog + SQLite trust store (`lucidity-pairing` + `wezterm-gui` overlays).

Important: **pairing does not gate Phase 1 connections yet**. For a production mobile app, you’ll almost certainly want pairing to become an authentication step (Phase 4-ish security brought forward).

## Phase 2 Deliverables

### 1) Mobile app skeleton (iOS + Android)
- [x] Choose stack: Flutter.
- [x] Create mobile repo (`lucidity-mobile`).
- [ ] CI builds (Pending).

### 2) LAN connection + protocol client
- [x] Implement the Phase 1 framing protocol.
- [x] Connect to a desktop host over LAN (TCP).
- [x] Request `list_panes`, show UI list, and select a pane.
- [x] Send `attach`, then stream PTY output frames to the renderer.
- [x] Send input frames (keystrokes/paste) back to the host.
- [ ] Handle reconnect (Code exists, unverified).

### 3) Terminal emulation on-device
- [x] Choose terminal emulator: `xterm.dart`.
- [x] Render bytes → ANSI/VT parser → grid → GPU/Canvas render.
- [x] Basic UX: scrollback, selection.
- [x] Soft keyboard integration.
- [ ] Window resize events.

### 4) Pairing UX on mobile (Phase 3 consumer)
- [x] QR scanner reads `lucidity://pair?data=...`.
- [x] Verify QR payload signature + expiry rules.
- [x] Submit a `PairingRequest` to the desktop.
- [x] Persist mobile keypair + trusted desktop(s) locally.
- [ ] “Manage devices” UI.

### 5) Production security gating (required before App Store)

Today, LAN TCP is effectively “trust the network.” For a real mobile app:

- [ ] Require pairing approval before allowing any non-localhost connection.
- [ ] Add authentication to the transport (e.g., signed challenge/response).
- [ ] Add encryption for the session (Noise-style handshake or TLS with pinning).
- [ ] Add replay protection + rate limits for pairing endpoints.
- [ ] Threat model: LAN attacker, rogue AP, compromised phone, etc.

## “What works vs. what doesn’t” checklist (repo-side)

### Known-good verification commands

```powershell
# Lucidity crates (fast)
cargo test -p lucidity-proto -p lucidity-host -p lucidity-pairing -p lucidity-client

# Desktop GUI build
cargo build -p wezterm-gui
```

### Manual sanity checks (desktop)

- [ ] Run `target/debug/wezterm-gui.exe` and confirm Lucidity host autostarts (see logs).
- [ ] Confirm pairing splash shows (unless `LUCIDITY_DISABLE_SPLASH=1`).
- [ ] Confirm approve/reject dialog appears when submitting a pairing request.
- [ ] Use `lucidity-client` to connect, list panes, attach, and mirror output.

## Reality check (App Store / Play Store)

As of **2026-01-18**, there is no iOS/Android app project in this repo yet, and no remote/authenticated transport. You can demo LAN mirroring and pairing UX on desktop, but shipping to App Store requires building the actual mobile apps + security hardening.

