# Lucidity Mobile - Architecture & Status

**Goal**: A complete, working Android/iOS terminal client for Lucidity (WezTerm).
**Stack**: Flutter (Dart) using `xterm.dart`.

## Current Status: Phase 3 (Pairing & Relay) ✅

The mobile application is currently in **Phase 3**, with full implementation of the secure pairing handshake and relay-based connectivity.

### Core Components

#### 1. Protocol (`lib/protocol/`)
- **Frames**: Custom 4-byte LE Length + 1-byte Type framing.
- **Relay Support**: secure WebSocket connection to Lucidity Relay.
- **Pairing**: Ed25519 signature generation for device authentication.

#### 2. Terminal Emulation (`lib/screens/terminal_screen.dart`)
- Uses `xterm.dart` for rendering.
- Supports resizing, input forwarding, and standard terminal sequences.

#### 3. Security & Pairing (`lib/screens/pairing_screen.dart`)
- Scans QR codes containing Desktop Public Key + Relay ID.
- Generates ephemeral device keypair.
- Signs pairing request with timestamp to prevent replay attacks.
- Persists paired desktop profiles.

## Verification Checklist (Ready for Testing)

- [x] **Protocol**: JSON control messages and PTY I/O.
- [x] **Relay**: Connection via `wss://` to relay server.
- [x] **Pairing**: Full cryptographic handshake.
- [ ] **End-to-End Test**: Verify "Scan -> Approve -> Control" flow on Windows (Pending Build Fix).

## Build Instructions

```bash
cd lucidity-mobile
flutter pub get
flutter run -d windows
```
