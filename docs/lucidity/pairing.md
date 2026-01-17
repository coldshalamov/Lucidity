# Pairing (Phase 3)

Phase 3 adds the **desktop pairing splash**:

- shows a QR code
- shows a short pairing code (currently the payload `relay_id`)
- “Press Enter to continue locally” (no sign-in required)

This is intentionally a **local-only** pairing MVP:
- no cloud rendezvous yet
- device trust store is local-only (SQLite)
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

## Pairing API (local only)

The desktop host service exposes JSON ops:

- `pairing_payload` → returns the current `PairingPayload` (desktop public key, relay_id, timestamp)
- `pairing_submit` → accepts a `PairingRequest` and returns `PairingResponse`
  - when the GUI is running, the desktop shows an approve/reject prompt
  - when no approver is registered (headless host), requests are rejected
- `pairing_list_trusted_devices` → lists stored `TrustedDevice` entries

### Trust store paths

- Desktop host keypair: `DATA_DIR/lucidity/host_keypair.json` (override `LUCIDITY_HOST_KEYPAIR`)
- Trusted devices DB: `DATA_DIR/lucidity/devices.db` (override `LUCIDITY_DEVICE_TRUST_DB`)
