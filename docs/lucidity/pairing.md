# Pairing (Phase 3)

Phase 3 adds the **desktop pairing splash**:

- shows a QR code
- shows a short pairing code (currently the payload `relay_id`)
- “Press Enter to continue locally” (no sign-in required)

This is intentionally a **local-only** pairing MVP:
- no cloud rendezvous yet
- device trust store exists as a crate, but not wired into the GUI flow yet
- no end-to-end encryption yet

## Desktop splash behavior

On first window open, Lucidity shows an overlay with:

- a `lucidity://pair?data=<base64-json>` QR payload (see `lucidity-pairing`)
- a short `Code:` value (currently the `relay_id`, derived from the desktop public key)

Keys:
- `Enter` closes the splash and continues locally
- `R` refreshes the QR (new timestamp)
- `Esc` closes the splash

Disable the splash:

```powershell
$env:LUCIDITY_DISABLE_SPLASH = '1'
```

## Key storage (desktop identity)

The desktop pairing splash uses a persistent Ed25519 keypair stored under the WezTerm data directory:

- `DATA_DIR/lucidity/host_keypair.json`

## Important security note

Until end-to-end encryption + device authentication are implemented, treat LAN binding as unsafe:

- Default bind is localhost-only.
- If you set `LUCIDITY_LISTEN=0.0.0.0:9797`, anyone on your LAN can connect to the host bridge and inject input.
