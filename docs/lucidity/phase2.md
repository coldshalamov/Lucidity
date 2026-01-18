# Phase 2: Mobile MVP (LAN client)

Phase 2 is where Lucidity becomes a real product: an iOS/Android app that can connect to a desktop on the same LAN, render the terminal output locally (ANSI/VT), and send input back to the same PTY.

## What’s already in this repo (foundation you can build on)

- **Phase 1 (implemented):** a TCP host bridge that can list panes, attach to a pane, stream raw PTY bytes, and inject input bytes (`lucidity-host`, `lucidity-proto`, `lucidity-client`).
- **Phase 3 (implemented, local-only MVP):** QR pairing splash + approve/reject dialog + SQLite trust store (`lucidity-pairing` + `wezterm-gui` overlays).

Important: **pairing does not gate Phase 1 connections yet**. For a production mobile app, you’ll almost certainly want pairing to become an authentication step (Phase 4-ish security brought forward).

## Phase 2 Deliverables

### 1) Mobile app skeleton (iOS + Android)

- [ ] Choose stack:
  - Native (Swift + Kotlin)
  - Flutter
  - React Native
  - Rust core + UniFFI bindings
- [ ] Create mobile repo(s) (likely a new top-level `lucidity-mobile/` or separate repo).
- [ ] CI builds for iOS + Android (even if no release signing yet).

### 2) LAN connection + protocol client

- [ ] Implement the Phase 1 framing protocol (see `docs/lucidity/protocol.md`).
- [ ] Connect to a desktop host over LAN (TCP).
- [ ] Request `list_panes`, show UI list, and select a pane.
- [ ] Send `attach`, then stream PTY output frames to the renderer.
- [ ] Send input frames (keystrokes/paste) back to the host.
- [ ] Handle reconnect, timeouts, and network changes (Wi‑Fi → cellular, sleep/wake).

### 3) Terminal emulation on-device

- [ ] Choose terminal emulator implementation:
  - Embed an existing terminal emulator widget/library
  - Use a Rust terminal core + platform UI view
- [ ] Render bytes → ANSI/VT parser → grid → GPU/Canvas render.
- [ ] Basic UX: scrollback, copy/paste, selection, links.
- [ ] Soft keyboard integration + “special keys” row (Esc/Ctrl/Alt/Tab/Arrows).
- [ ] Window resize events: send size changes to the desktop pane.

### 4) Pairing UX on mobile (Phase 3 consumer)

- [ ] QR scanner reads `lucidity://pair?data=...`.
- [ ] Verify QR payload signature + expiry rules (`lucidity-pairing` logic).
- [ ] Submit a `PairingRequest` to the desktop and wait for approval.
- [ ] Persist mobile keypair + trusted desktop(s) locally (Keychain/Keystore).
- [ ] “Manage devices” UI (list trusted desktops, remove, re-pair).

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

