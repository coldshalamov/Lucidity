# Lucidity - Project Constitution

**This document is the ABSOLUTE LAW for this project. All agents MUST follow these rules.**

## The Product Vision (Non-Negotiable)

Lucidity is a mobile terminal client that pairs with a desktop terminal (WezTerm fork) via QR code. The user experience is:

1. **Open the desktop app** - Shows a QR code on first run (can press Enter to skip and use as normal terminal)
2. **Open the mobile app** - User can authenticate (for billing) or skip
3. **Scan the QR code** - Phone scans the QR displayed on desktop
4. **Terminal appears on phone** - Whatever you type on phone goes to the desktop's terminal
5. **Profile is saved** - After first pairing, the device is saved. User can reconnect from anywhere.

### Critical Requirements

| Requirement | Description | Status |
|-------------|-------------|--------|
| **Works from ANYWHERE** | Not just LAN. User can control their desktop from any location globally. | Required |
| **No mandatory relay bottleneck** | Use P2P (UPnP/STUN) as primary. Relay is FALLBACK only. | Required |
| **QR pairing creates persistent profile** | Once paired, the desktop appears in your "devices" list forever. | Required |
| **Phone sends commands to desktop** | Mobile is input device + renderer. Desktop runs actual PTY. | Required |
| **Multiple tabs/panes** | User can open new tabs from phone. | Required |
| **No account required for basic use** | Account only needed for premium features/billing. | Required |

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         CONNECTION PRIORITY ORDER                           │
├─────────────────────────────────────────────────────────────────────────────┤
│  1. LAN Direct    │ Same network, lowest latency (~1ms)                     │
│  2. UPnP/P2P      │ Router opens port, direct connection over internet      │
│  3. STUN/NAT-PMP  │ NAT hole-punching for direct connection                 │
│  4. Relay Server  │ FALLBACK ONLY - when P2P fails (symmetric NAT, etc)     │
└─────────────────────────────────────────────────────────────────────────────┘

Mobile App                                              Desktop (WezTerm)
┌─────────────────┐                                    ┌─────────────────┐
│ Terminal View   │◄──────── TCP/TLS ────────────────►│ lucidity-host   │
│ (xterm.dart)    │   (Direct P2P preferred)          │ (Rust server)   │
│                 │                                    │                 │
│ Keyboard Input  │                                    │ Real PTY/ConPTY │
│ Gesture Control │                                    │ Session State   │
└─────────────────┘                                    └─────────────────┘
         │                                                      │
         │              ┌─────────────────┐                     │
         └──────────────│  Relay Server   │─────────────────────┘
                        │  (FALLBACK)     │
                        │  - Only routes  │
                        │  - No data log  │
                        └─────────────────┘
```

## What Agents MUST NOT Do

1. **DO NOT** revert to "LAN-only" architecture. The app MUST work over the internet.
2. **DO NOT** suggest VPN/Tailscale/WireGuard as the solution. We build our own connectivity.
3. **DO NOT** make the relay server mandatory. It's a FALLBACK for when P2P fails.
4. **DO NOT** remove P2P/UPnP/STUN code. This is the PRIMARY connection method.
5. **DO NOT** add mandatory accounts/logins for basic functionality.
6. **DO NOT** modify core WezTerm files unless absolutely necessary.

## What Agents MUST Do

1. **Preserve P2P-first architecture** - UPnP and STUN are primary connection methods
2. **Implement relay as fallback** - Only used when P2P fails (symmetric NAT, corporate firewall)
3. **Keep the pairing flow simple** - Scan QR, approve on desktop, done forever
4. **Test with real internet scenarios** - Not just localhost or LAN
5. **Document any architectural decisions** in the appropriate phase doc

## Directory Structure (What to Touch)

### Lucidity Code (95% of your work)
```
lucidity-host/        # Desktop TCP server, P2P, pairing API
lucidity-mobile/      # Flutter iOS/Android app
lucidity-client/      # CLI test client
lucidity-pairing/     # Ed25519 crypto, QR codes, trust store
lucidity-proto/       # Shared protocol definitions
```

### WezTerm Integration (Minimal changes)
```
wezterm-gui/src/main.rs              # Starts lucidity-host
wezterm-gui/src/overlay/             # QR splash, approval dialogs
wezterm-gui/src/termwindow/mod.rs    # Pairing hooks
```

### DO NOT TOUCH
```
wezterm-*/ (except wezterm-gui)      # Core terminal - read-only
termwiz/, term/, mux/, window/       # Terminal libraries
deps/                                 # External dependencies
```

## Current Implementation Status

### Completed (Working)
- [x] Desktop host bridge with PTY streaming (`lucidity-host`)
- [x] P2P via UPnP port mapping + STUN discovery
- [x] QR code pairing with Ed25519 signatures
- [x] Device trust store (SQLite)
- [x] Mutual authentication handshake (nonce-based)
- [x] Flutter mobile app with terminal rendering
- [x] LAN and external address in pairing payload
- [x] Premium OLED theme, gestures, keyboard toolbar
- [x] Window resize events - Sync terminal size changes
- [x] Relay server as fallback (`lucidity-relay`)

### Missing (Must Build)
- [ ] **Relay agent on desktop** - Connect desktop to relay when UPnP/STUN fails
- [ ] **Mobile relay connection** - Connect to relay when P2P fails
- [ ] **Connection cascade** - Try P2P first, fallback to relay automatically
- [ ] **Device management UI** - List/revoke paired devices
- [ ] **App store builds** - iOS/Android release configurations
- [ ] **Clipboard sync** - Share clipboard between desktop and mobile (Host -> Mobile missing)

## Testing Commands

```powershell
# Fast Lucidity-only tests
cargo test -p lucidity-proto -p lucidity-host -p lucidity-pairing -p lucidity-client

# Build desktop
cargo build -p wezterm-gui

# Run desktop
target/debug/wezterm-gui.exe

# Mobile (Flutter)
cd lucidity-mobile && flutter run
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `LUCIDITY_LISTEN` | `127.0.0.1:9797` | Host bridge address |
| `LUCIDITY_DISABLE_SPLASH` | `false` | Skip QR overlay |
| `LUCIDITY_RELAY_URL` | - | Relay server (fallback) |
| `LUCIDITY_DISABLE_HOST` | `false` | Disable host bridge |

## Security Model (Summary)

- **Ed25519 keypairs** for both desktop and mobile
- **QR code contains**: Desktop public key + LAN address + External address
- **Pairing handshake**: Mobile signs `(desktop_pubkey || timestamp)`, desktop verifies
- **Session auth**: Nonce-based challenge-response on every connection
- **Trust store**: SQLite database of approved device public keys
- **Relay is UNTRUSTED**: Only routes encrypted traffic, cannot read content

## For AI Agents

When working on this project:

1. **Read this file first** before any implementation
2. **Check the phase docs** in `docs/lucidity/` for current status
3. **Run tests** before and after changes: `cargo test -p lucidity-*`
4. **Do not hallucinate features** - check what actually exists in code
5. **Keep the user experience simple** - QR scan → approve → done

**THE GOAL IS SIMPLE**: Open desktop app, open mobile app, scan QR, control terminal from anywhere in the world.
