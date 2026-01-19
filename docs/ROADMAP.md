# Lucidity Roadmap: Path to App Store

This document tracks the high-level progress of Lucidity from initial protocol design to public App Store release.

## Architecture Philosophy

**P2P-First, Relay-Fallback**

Lucidity prioritizes direct peer-to-peer connections. The relay server is only used when P2P fails (symmetric NAT, corporate firewalls, etc).

```
Connection Priority:
1. LAN Direct    → ~1ms latency, same network
2. UPnP/External → Direct over internet, router port mapping
3. STUN/NAT-PMP  → Direct over internet, NAT hole-punching
4. Relay Server  → FALLBACK ONLY (when P2P fails)
```

---

## Phase 1: Host Bridge (Foundations) - COMPLETE

**Goal**: Enable a remote client to control a desktop pane via TCP.

- [x] **Host Server**: `lucidity-host` crate (TCP, framing, JSON control)
- [x] **Protocol**: `lucidity-proto` defined (Version 2)
- [x] **Desktop Integration**: `wezterm-gui` auto-starts host
- [x] **CLI Client**: `lucidity-client` verifies connectivity
- [x] **Security Model**: Trusted device store (`devices.db`) & Ed25519 pairing logic

---

## Phase 2: Mobile LAN MVP - COMPLETE

**Goal**: A working mobile app that connects over local Wi-Fi.

- [x] **Mobile Skeleton**: Flutter project created (`lucidity-mobile`)
- [x] **Terminal Rendering**: `xterm.dart` integrated
- [x] **Connection**: Dart `LucidityClient` implemented (TCP + Auth)
- [x] **Pairing**: `PairingScreen` + `MobileIdentity` implemented
- [x] **Entry Point**: `main.dart` with Theme and Providers

---

## Phase 3: P2P Connectivity & NAT Traversal - COMPLETE

**Goal**: Secure, authenticated connections without mandatory relays.

- [x] **QR Pairing Logic**: Verified via simulated handshake
- [x] **LAN Connectivity**: Host & Client work over local Wi-Fi
- [x] **UPnP/NAT-PMP**: `lucidity-host` automatically maps port 9797
- [x] **P2P Discovery**: Host broadcasts LAN and External IP in pairing payload
- [x] **STUN Integration**: Public address discovery via Google STUN
- [x] **Mutual Authentication**: Host signs client nonce; mobile verifies host identity

---

## Phase 4: UI/UX Polish - COMPLETE

**Goal**: "It feels like a native, premium app."

- [x] **Gestures**: Swipe to switch tabs/panes
- [x] **Keyboard Toolbar**: Scrollable Ctrl/Alt/Esc/Tab/Arrow helpers
- [x] **Haptics**: Feedback on keypress
- [x] **Premium Theme**: OLED black + gold accents

---

## Phase 5: Production Readiness - IN PROGRESS

**Goal**: Works globally, handles edge cases.

### 5.1 Relay Fallback System
- [ ] **Relay Server**: WebSocket relay for when P2P fails
- [ ] **Desktop Agent**: Connect to relay when UPnP/STUN fails
- [ ] **Mobile Client**: Fall back to relay when direct fails
- [ ] **Connection Cascade**: Automatic P2P → Relay transition

### 5.2 Device Management
- [ ] **Mobile UI**: List, rename, remove paired desktops
- [ ] **Desktop CLI**: List, revoke paired mobiles
- [ ] **Sync**: Handle device renames across both sides

### 5.3 App Store Preparation
- [ ] **App Icons**: Adaptive icons, splash screens
- [ ] **Privacy Policy**: Required for submission
- [ ] **iOS Signing**: Apple Developer Account, provisioning
- [ ] **Android Signing**: Keystore setup
- [ ] **Beta Testing**: TestFlight (iOS) / Internal Track (Play Store)

---

## Phase 6: Enhanced Features

**Goal**: Feature parity with desktop usage.

- [ ] **Clipboard Sync**: Share clipboard between devices
- [ ] **Window Resize**: Sync terminal size on rotation
- [ ] **Multiple Tabs**: Open/close/switch tabs from mobile
- [ ] **Auto-Reconnect**: Exponential backoff retry logic

---

## Phase 7: App Store Release

**Goal**: Public availability.

- [ ] App Store review (iOS)
- [ ] Play Store review (Android)
- [ ] Marketing materials
- [ ] Support documentation

---

## Status Key

- [x] **Complete**
- [ ] **Pending**
