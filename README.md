# Lucidity

*Lucidity is a fork of [WezTerm](https://github.com/wez/wezterm): a GPU-accelerated terminal emulator + multiplexer written in Rust.*

**Product goal:** Control your desktop terminal from your phone. Scan a QR code, and you're connected - from anywhere in the world.

## How It Works

1. **Open Lucidity on your desktop** - Shows a QR code (or press Enter to use as normal terminal)
2. **Open Lucidity on your phone** - Scan the QR code
3. **Approve on desktop** - Confirm the pairing
4. **Control your terminal** - Type on your phone, commands run on your desktop

Your paired devices are saved. After the first pairing, just reconnect from anywhere.

## Architecture

Lucidity uses **P2P-first connectivity** - your phone connects directly to your computer whenever possible. A relay server is only used as a fallback when direct connections fail.

```
Mobile App                                              Desktop (WezTerm)
┌─────────────────┐                                    ┌─────────────────┐
│ Terminal View   │                                    │ lucidity-host   │
│ (renders ANSI)  │◄──────── Direct P2P ──────────────►│ (TCP server)    │
│                 │     (LAN / UPnP / STUN)            │                 │
│ Keyboard Input  │                                    │ Real PTY/ConPTY │
└─────────────────┘                                    └─────────────────┘
         │                                                      │
         │              ┌─────────────────┐                     │
         └──────────────│  Relay Server   │─────────────────────┘
            (fallback)  │  (when P2P fails)│    (fallback)
                        └─────────────────┘
```

**Connection Priority:**
1. **LAN Direct** - Same Wi-Fi network (~1ms latency)
2. **UPnP/External** - Router port mapping (internet, direct)
3. **STUN/NAT-PMP** - NAT hole-punching (internet, direct)
4. **Relay** - Fallback only (when P2P fails)

## Status

| Component | Status | Description |
|-----------|--------|-------------|
| **Desktop Host** | Complete | PTY bridge, P2P (UPnP/STUN), frame protocol |
| **Mobile App** | Complete | Flutter, terminal rendering, QR pairing |
| **Pairing System** | Complete | Ed25519 signatures, device trust store |
| **P2P Connectivity** | Complete | UPnP, STUN, LAN discovery |
| **Relay Server** | In Progress | Fallback for symmetric NAT / corporate firewalls |

## Quick Start

### 1. Run Desktop
```bash
# Build and run (requires Rust toolchain)
cargo run -p wezterm-gui
```

### 2. Pair Mobile
1. Press `Ctrl+Shift+L` to show QR code
2. Scan with Lucidity mobile app
3. Approve pairing on desktop
4. Control terminal from phone!

### 3. Reconnect Anytime
Once paired, your desktop appears in the mobile app's device list. Tap to reconnect from anywhere.

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `LUCIDITY_LISTEN` | `127.0.0.1:9797` | Host bridge listen address |
| `LUCIDITY_DISABLE_SPLASH` | `false` | Skip QR overlay on startup |
| `LUCIDITY_RELAY_URL` | - | Relay server URL (fallback) |
| `LUCIDITY_DISABLE_HOST` | `false` | Disable host bridge entirely |

## Security

- **Ed25519 Signatures** - Both devices authenticate via cryptographic signatures
- **Mutual Auth** - Desktop verifies mobile, mobile verifies desktop
- **No Account Required** - Device-based pairing, no login needed
- **Relay is Untrusted** - Only routes encrypted traffic, cannot read content

See [Security Model](docs/lucidity/security-model.md) for details.

## Documentation

- [Project Vision](CLAUDE.md) - The absolute rules for this project
- [Master Plan](docs/MASTER_PLAN.md) - Complete implementation roadmap
- [Agent Guide](AGENTS.md) - For AI agents working on the codebase
- [Security Model](docs/lucidity/security-model.md) - Authentication & encryption
- [Troubleshooting](docs/lucidity/troubleshooting.md) - Common issues & fixes
- [FAQ](docs/lucidity/FAQ.md) - Frequently asked questions

## Building

### Desktop (Rust)
```bash
# Test Lucidity components
cargo test -p lucidity-proto -p lucidity-host -p lucidity-pairing -p lucidity-client

# Build WezTerm with Lucidity
cargo build -p wezterm-gui

# Run
./target/debug/wezterm-gui
```

### Mobile (Flutter)
```bash
cd lucidity-mobile
flutter pub get
flutter run
```

## Upstream

Lucidity is based on WezTerm. See [WezTerm's documentation](https://wezfurlong.org/wezterm/) for terminal features.

## License

MIT License - same as WezTerm.

## Contributing

See [AGENTS.md](AGENTS.md) for contribution guidelines and code navigation.
