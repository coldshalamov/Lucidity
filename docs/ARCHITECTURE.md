# Lucidity Architecture

This document describes the technical architecture of Lucidity, including the P2P-first connectivity model and relay fallback system.

## System Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           LUCIDITY ARCHITECTURE                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌──────────────┐                                    ┌──────────────┐       │
│  │   MOBILE     │                                    │   DESKTOP    │       │
│  │  (Flutter)   │                                    │  (WezTerm)   │       │
│  ├──────────────┤                                    ├──────────────┤       │
│  │ xterm.dart   │                                    │ lucidity-host│       │
│  │ Terminal     │◄─────── TCP/TLS Direct ──────────►│ TCP Server   │       │
│  │ Renderer     │     (LAN / UPnP / STUN)           │              │       │
│  │              │                                    │ Real PTY     │       │
│  │ MobileId     │                                    │ Bridge       │       │
│  │ Keypair      │                                    │              │       │
│  └──────────────┘                                    │ Ed25519      │       │
│         │                                            │ Keypair      │       │
│         │         ┌──────────────┐                   │              │       │
│         │         │    RELAY     │                   │ P2P Module   │       │
│         └────────►│  (FALLBACK)  │◄──────────────────│ UPnP/STUN   │       │
│       WebSocket   │              │   WebSocket       └──────────────┘       │
│                   │ Session Mgmt │                                          │
│                   │ No Data Log  │                                          │
│                   └──────────────┘                                          │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Connection Strategy

### Priority Order

| Priority | Method | When Used | Latency |
|----------|--------|-----------|---------|
| 1 | LAN Direct | Same network | ~1ms |
| 2 | UPnP/External | Router supports UPnP | ~50ms |
| 3 | STUN/NAT-PMP | NAT hole-punching | ~50ms |
| 4 | Relay | When P2P fails | ~100ms+ |

### Connection Flow

```
Mobile App                                              Desktop Host
    │                                                        │
    │  1. Parse QR payload                                   │
    │     ├── desktop_pubkey                                 │
    │     ├── lan_addr (e.g., 192.168.1.100:9797)          │
    │     ├── external_addr (e.g., 203.0.113.50:9797)      │
    │     └── relay_url (optional)                          │
    │                                                        │
    │  2. Try LAN Direct                                     │
    │  ──────────────────────────────────────────────────►  │
    │     TCP connect to lan_addr                           │
    │                                                        │
    │  3. If failed, try External                           │
    │  ──────────────────────────────────────────────────►  │
    │     TCP connect to external_addr                      │
    │                                                        │
    │  4. If failed, try Relay                              │
    │  ─────────────┐                                       │
    │               ▼                                       │
    │         ┌─────────┐                                   │
    │         │  RELAY  │◄────────────────────────────────  │
    │         └─────────┘  Desktop connects when P2P fails  │
    │                                                        │
    │  5. Auth Handshake (same for all methods)             │
    │  ◄─────── AuthChallenge(nonce) ───────────────────────│
    │  ──────── AuthResponse(signed_nonce) ─────────────────►
    │  ◄─────── AuthSuccess(host_signature) ────────────────│
    │                                                        │
    │  6. Session established                               │
    │  ◄═══════════ PTY I/O ═══════════════════════════════►│
```

## Components

### Desktop: lucidity-host

**Location**: `lucidity-host/`

**Responsibilities**:
- TCP server on port 9797
- PTY bridge (connect panes to network)
- P2P discovery (UPnP port mapping, STUN)
- Pairing API (QR payload generation)
- Device trust store (SQLite)
- Authentication handshake

**Key Files**:
```
src/
├── lib.rs           # Public API
├── server.rs        # TCP server, frame routing
├── bridge.rs        # PTY abstraction
├── pairing_api.rs   # QR payload, pairing approval
├── p2p.rs           # UPnP + STUN discovery
└── protocol.rs      # Frame constants
```

**External Connections**:
- UPnP gateway discovery (local network)
- STUN server (stun.l.google.com:19302)
- Public IP services (ipify.org, ifconfig.me)

### Mobile: lucidity-mobile

**Location**: `lucidity-mobile/`

**Responsibilities**:
- QR code scanning
- Terminal rendering (xterm.dart)
- Keyboard input handling
- Connection management
- Device profile storage

**Key Files**:
```
lib/
├── main.dart                    # App entry
├── app/
│   ├── app_state.dart          # Global state
│   └── desktop_profile.dart    # Saved desktops
├── protocol/
│   ├── lucidity_client.dart    # TCP client
│   ├── frame.dart              # Frame codec
│   └── mobile_identity.dart    # Ed25519 keypair
└── screens/
    ├── home_screen.dart        # Device list
    ├── pairing_screen.dart     # QR scanner
    └── desktop_screen.dart     # Terminal view
```

### Relay: lucidity-relay (FALLBACK)

**Location**: `lucidity-relay/` (to be implemented)

**Responsibilities**:
- WebSocket server
- Session management (pair desktop ↔ mobile)
- Traffic routing (no data inspection)
- Optional: auth token validation

**Design Principles**:
1. **Stateless**: No session data persisted
2. **Untrusted**: Cannot read payload content
3. **Simple**: Just routes WebSocket messages
4. **Fallback**: Only used when P2P fails

**API**:
```
GET  /health                    # Health check
WS   /desktop/{relay_id}        # Desktop connects here
WS   /mobile/{relay_id}         # Mobile connects here
```

## Protocol

### Frame Format

```
┌────────────┬────────────┬─────────────────────┐
│  Length    │   Type     │      Payload        │
│  (4 bytes) │  (1 byte)  │  (variable)         │
│  LE u32    │            │                     │
└────────────┴────────────┴─────────────────────┘
```

### Frame Types

| Type | Value | Description |
|------|-------|-------------|
| JSON | 1 | Control messages (JSON-encoded) |
| PANE_OUTPUT | 2 | PTY output bytes |
| PANE_INPUT | 3 | Keyboard input bytes |
| PING | 4 | Keep-alive |

### Control Messages

```json
// List panes request
{"type": "list_panes"}

// List panes response
{"type": "list_panes_response", "panes": [{"id": 1, "title": "bash"}]}

// Attach to pane
{"type": "attach", "pane_id": 1}

// Auth challenge (host → mobile)
{"type": "auth_challenge", "nonce": "base64..."}

// Auth response (mobile → host)
{"type": "auth_response", "nonce": "base64...", "client_nonce": "base64...", "signature": "base64..."}

// Auth success (host → mobile)
{"type": "auth_success", "signature": "base64..."}
```

## Security

### Cryptographic Primitives

| Component | Algorithm | Key Size |
|-----------|-----------|----------|
| Keypairs | Ed25519 | 256-bit |
| Signatures | Ed25519 | 512-bit |
| Nonces | Random | 256-bit |

### Pairing Flow

```
1. Desktop generates QR payload:
   ├── desktop_pubkey (Ed25519 public key, base64)
   ├── lan_addr (optional)
   ├── external_addr (optional)
   └── timestamp + signature

2. Mobile scans QR, extracts payload

3. Mobile generates PairingRequest:
   ├── device_name
   ├── mobile_pubkey
   ├── signature = sign(desktop_pubkey || timestamp)
   └── timestamp

4. Mobile sends PairingRequest to desktop

5. Desktop shows approval dialog:
   "Allow 'iPhone 15' to connect?"
   [Device fingerprint: ABC123...]

6. User approves → Desktop stores mobile_pubkey in devices.db

7. Mobile stores desktop_pubkey in secure storage
```

### Session Authentication

Every connection (regardless of transport) requires authentication:

```
Mobile                                              Desktop
   │                                                    │
   │──────────── Connect (TCP or WebSocket) ──────────►│
   │                                                    │
   │◄─────────── AuthChallenge(server_nonce) ──────────│
   │                                                    │
   │  Create auth_response:                             │
   │  - client_nonce = random()                         │
   │  - signature = sign(server_nonce || client_nonce)  │
   │                                                    │
   │────────── AuthResponse(client_nonce, sig) ────────►│
   │                                                    │
   │  Desktop verifies:                                 │
   │  - mobile_pubkey in devices.db                     │
   │  - signature valid                                 │
   │                                                    │
   │  Desktop creates auth_success:                     │
   │  - signature = sign(client_nonce)                  │
   │                                                    │
   │◄─────────── AuthSuccess(host_signature) ──────────│
   │                                                    │
   │  Mobile verifies:                                  │
   │  - host_signature valid for desktop_pubkey         │
   │                                                    │
   │══════════════ Session established ════════════════│
```

## Data Flow

### Terminal I/O

```
┌──────────────┐      Frame       ┌──────────────┐      PTY       ┌─────────┐
│              │   TYPE_INPUT     │              │    write()     │         │
│    Mobile    │─────────────────►│lucidity-host │───────────────►│  Shell  │
│   Keyboard   │                  │              │                │ (bash)  │
│              │                  │              │                │         │
└──────────────┘                  └──────────────┘                └─────────┘
       ▲                                 │                              │
       │        Frame                    │         PTY                  │
       │     TYPE_OUTPUT                 │        read()                │
       └─────────────────────────────────┴──────────────────────────────┘
```

### Rendering Pipeline

```
PTY Output (raw bytes)
    │
    ▼
Mobile receives TYPE_PANE_OUTPUT frame
    │
    ▼
xterm.dart ANSI/VT parser
    │
    ├── Character grid updates
    ├── Color/style changes
    ├── Cursor movements
    └── Screen commands (clear, scroll)
    │
    ▼
Flutter CustomPainter renders to Canvas
    │
    ▼
GPU composition → Screen
```

## File Locations

| Component | Path | Description |
|-----------|------|-------------|
| Desktop host | `lucidity-host/` | Rust TCP server |
| Protocol | `lucidity-proto/` | Shared structs |
| Pairing crypto | `lucidity-pairing/` | Ed25519, QR |
| Test client | `lucidity-client/` | CLI tool |
| Mobile app | `lucidity-mobile/` | Flutter |
| Relay (TODO) | `lucidity-relay/` | WebSocket relay |

## Environment Variables

### Desktop

| Variable | Default | Description |
|----------|---------|-------------|
| `LUCIDITY_LISTEN` | `127.0.0.1:9797` | Host listen address |
| `LUCIDITY_DISABLE_SPLASH` | `false` | Skip QR overlay |
| `LUCIDITY_RELAY_URL` | - | Relay server (fallback) |
| `LUCIDITY_RELAY_ID` | auto | Desktop ID for relay |

### Relay

| Variable | Default | Description |
|----------|---------|-------------|
| `LUCIDITY_RELAY_LISTEN` | `0.0.0.0:9090` | Server listen address |
| `LUCIDITY_RELAY_NO_AUTH` | `false` | Disable auth (dev only) |
| `LUCIDITY_RELAY_DESKTOP_SECRET` | - | Auth secret |

## Testing

### Unit Tests

```bash
cargo test -p lucidity-proto
cargo test -p lucidity-host
cargo test -p lucidity-pairing
cargo test -p lucidity-client
```

### Integration Testing

1. **LAN Direct**: Desktop and mobile on same network
2. **UPnP**: Desktop behind router with UPnP enabled
3. **Relay**: Desktop behind symmetric NAT, force relay fallback

### Manual Smoke Test

```bash
# Terminal 1: Start desktop
cargo run -p wezterm-gui

# Terminal 2: View logs
RUST_LOG=lucidity=debug cargo run -p wezterm-gui

# Mobile: Scan QR, verify connection
```
