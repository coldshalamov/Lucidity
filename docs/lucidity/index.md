# Lucidity

Lucidity is a WezTerm fork with one specific goal:

> **Control your desktop terminal from your phone - from anywhere in the world.**
>
> Desktop runs a real PTY/ConPTY shell session. Your phone renders terminal output locally (ANSI/VT parsing) and sends keystrokes back in real time.

## Core Principles

1. **P2P-First** - Direct connections (LAN, UPnP, STUN) are PRIMARY. Relay is FALLBACK only.
2. **QR Pairing** - Scan a code, approve on desktop, done forever.
3. **No Account Required** - Device-based trust, no login for basic use.
4. **Desktop is Normal** - Press Enter to skip QR and use as regular terminal.
5. **Profiles Persist** - Paired devices saved, reconnect from anywhere.

## Architecture

```
Connection Priority:
1. LAN Direct    → Same network, fastest
2. UPnP/External → Router port mapping, internet direct
3. STUN/NAT-PMP  → NAT hole-punching, internet direct
4. Relay Server  → FALLBACK when P2P fails (symmetric NAT, firewalls)
```

The phone is a **terminal emulator**, not remote desktop streaming. It parses ANSI/VT codes and renders locally, just like the desktop does.

## Implementation Status

| Phase | Status | Description |
|-------|--------|-------------|
| Phase 1 | Complete | Desktop host bridge (PTY streaming, frame protocol) |
| Phase 2 | Complete | Mobile app (Flutter, LAN connection, terminal rendering) |
| Phase 3 | Complete | P2P connectivity (UPnP, STUN, mutual authentication) |
| Phase 4 | Complete | UI polish (gestures, keyboard toolbar, OLED theme) |
| Phase 5 | In Progress | Relay fallback, device management, app store |

## Key Documents

- [CLAUDE.md](../../CLAUDE.md) - Project vision and absolute rules
- [MASTER_PLAN.md](../MASTER_PLAN.md) - Complete implementation roadmap
- [AGENTS.md](../../AGENTS.md) - Agent coordination guide
- [security-model.md](security-model.md) - Authentication and encryption
- [phase2.md](phase2.md) - Mobile app implementation details
- [phase3.md](phase3.md) - P2P connectivity details
- [phase4.md](phase4.md) - UI/UX polish details

## Roadmap

- **Phase 5** - Relay server (fallback), connection cascade, device management
- **Phase 6** - Clipboard sync, window resize, multiple tabs from mobile
- **Phase 7** - App Store release (iOS, Android)

## Security Summary

- **Ed25519 keypairs** for both desktop and mobile
- **QR payload** contains desktop public key + addresses
- **Mutual authentication** - both sides verify signatures
- **Trust store** - SQLite database of approved devices
- **Relay untrusted** - only routes encrypted traffic
