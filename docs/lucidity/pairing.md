# Pairing (Phase 3)

Phase 3 adds the **desktop pairing splash**:

- shows a QR code
- shows a short pairing code (`LUC-xxxxxx`)
- “Press Enter to continue locally” (no sign-in required)

This is intentionally a **local-only** pairing MVP:
- no cloud rendezvous yet
- no device trust store yet
- no end-to-end encryption yet

## Desktop splash behavior

On first window open, Lucidity shows an overlay with:

- QR payload (JSON) including `host_id`, `pairing_token`, and LAN candidates
- short `pairing_code` derived from the token

Keys:
- `Enter` closes the splash and continues locally
- `R` refreshes (generates a new token/code)
- `Esc` closes the splash

Disable the splash:

```powershell
$env:LUCIDITY_DISABLE_SPLASH = '1'
```

## Pairing endpoints (host service)

The embedded host service supports:

- `{"op":"pair_info"}` → returns current pairing info
- `{"op":"pair_claim","code":"LUC-123456"}` → returns pairing info if code matches current token

These are for building the mobile app UX (scan QR or enter code) without requiring any cloud services yet.

## Important security note

Until end-to-end encryption + device authentication are implemented, treat LAN binding as unsafe:

- Default bind is localhost-only.
- If you set `LUCIDITY_LISTEN=0.0.0.0:9797`, anyone on your LAN can connect to the host bridge and inject input.

