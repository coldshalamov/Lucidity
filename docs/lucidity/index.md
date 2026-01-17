# Lucidity

Lucidity is a WezTerm fork with one specific goal:

> **A real terminal, mirrored to your phone.**  
> Desktop runs a real PTY/ConPTY shell session. The phone renders terminal output locally (a terminal emulator) and sends keystrokes to the same PTY in real time.

## Non‑negotiables (product rules)

- Desktop behaves like a normal local terminal if you “press Enter to continue locally” (no account required).
- Remote connection is “scan QR or enter code” pairing.
- The phone is a terminal emulator (ANSI/VT parsing), not remote desktop streaming.
- Sessions persist on desktop even if phone disconnects.

## What exists today

This repo is still largely WezTerm; Lucidity features are being added incrementally.

**Phase 1 proof (implemented):**
- A desktop-side host bridge (`lucidity-host`) that can:
  - list panes
  - attach to a pane
  - stream raw PTY output bytes
  - inject input bytes into the same PTY
- A minimal test client (`lucidity-client`) to connect and mirror.

**Implemented (local-only):**
- Pairing splash overlay (QR + short code)
- Pairing API (`pairing_payload` / `pairing_submit`) with GUI approve/reject prompt
- Device trust store (SQLite) for paired devices

**Not implemented yet (planned):**
- iOS/Android apps (terminal renderer + input UI)
- Cloud relay + auth + subscriptions + quota enforcement
- End-to-end encryption (Noise/libsodium-style)


## Roadmap

- **Phase 1:** Local mirroring proof (desktop host bridge + local client)
- **Phase 2:** Mobile MVP (LAN connect, render ANSI/VT, resize)
- **Phase 3:** Pairing splash UX (QR + code, no manual IP entry)
- **Phase 4:** Cloud relay + quotas + subscriptions
- **Phase 5:** Reliability + device management + polish

See also:
- `docs/lucidity/phase1.md`
- `docs/lucidity/pairing.md`
- `docs/lucidity/protocol.md`
- `docs/lucidity/security.md`
