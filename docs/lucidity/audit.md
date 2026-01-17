# Lucidity Repo Audit (2026-01-17)

This audit compares the repository state against the docs in `docs/lucidity/` and the Phase 1/Phase 3 progress notes.

## What This Repo Is

Lucidity is a fork of WezTerm with one product goal: mirror a *live* desktop PTY session to a phone client (terminal emulator + keystroke injection), not remote desktop.

## Implemented (Verified)

### Phase 1: Local mirroring proof

- Desktop host bridge (`lucidity-host`):
  - lists panes
  - attaches to a pane
  - streams raw PTY output bytes
  - injects input bytes into the same PTY
- CLI test client (`lucidity-client`) connects over TCP-framed protocol.

### Phase 3: Pairing UX (local-only MVP)

- GUI pairing splash overlay with ASCII QR (`lucidity://pair?data=...`) + short code.
- Pairing API on the host (`pairing_payload`, `pairing_submit`, `pairing_list_trusted_devices`).
- GUI approve/reject prompt for pairing requests (when GUI is running).
- Approved devices are stored in a local SQLite trust store.

### Safety/Hardening Added

- Warn when binding host to all interfaces (`LUCIDITY_LISTEN=0.0.0.0:...`).
- Basic client connection cap via `LUCIDITY_MAX_CLIENTS` (default 4).

## Not Implemented Yet (Major Gaps vs Product Vision)

These are explicitly planned in `docs/lucidity/cloud-architecture.md` but are not present in code:

- iOS app
- Android app
- Cloud relay service (rendezvous + broker)
- Google OAuth (mobile sign-in) and account system
- Subscription/quotas/monetization enforcement
- End-to-end encryption + authenticated transport for remote connections

## Verification Commands Actually Run

Core Lucidity crates:

- `cargo test -p lucidity-host -p lucidity-proto -p lucidity-pairing`

Workspace builds:

- `cargo build -p wezterm`
- `cargo build -p wezterm-gui`

Note: on Windows, building `wezterm`/`wezterm-gui` requires a full Perl toolchain for vendored OpenSSL.

## Recommended Next Steps

1. Phase 2 (Mobile MVP): build a minimal iOS/Android client that can connect on LAN and speak the Phase 1 TCP protocol.
2. Phase 4 (Remote): implement cloud relay + authenticated, encrypted tunnel (Noise/similar) so the phone works over the public Internet.
3. Monetization: decide quotas/subscription tiers and implement enforcement at the relay.
