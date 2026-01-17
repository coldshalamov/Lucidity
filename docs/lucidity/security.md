# Security (current + planned)

## Phase 1 status

Phase 1 is a local developer proof.

- Transport is **plaintext TCP**.
- Default bind is **localhost-only** (`127.0.0.1:9797`).
- If you bind to `0.0.0.0`, anyone on your LAN who can reach the port can inject keystrokes.
- The pairing splash can be disabled via `LUCIDITY_DISABLE_SPLASH=1`.
- Pairing requests require explicit desktop approval when the GUI is running.

## v0.1 target (planned)

The intended v0.1 design:

- Pairing via QR or numeric code
- Device trust store (host stores paired phone keys; phone stores host key)
- Mutual auth on reconnect
- End-to-end encrypted transport (cloud relay never sees terminal bytes)

Until those are implemented, treat `LUCIDITY_LISTEN=0.0.0.0:*` as unsafe.
