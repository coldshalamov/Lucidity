# Lucidity Systematic Testing Regime

This document outlines the comprehensive testing program for Lucidity. Every release or major architectural change MUST pass these tests to ensure stability across P2P and Relay connection methods.

## 1. Automated Test Suite
Run these commands after any change to core logic.

```powershell
# Unit Tests (Protocol, Pairing, Local Host)
cargo test -p lucidity-proto -p lucidity-host -p lucidity-pairing -p lucidity-client

# Relay Server Tests
cargo test -p lucidity-relay

# Mobile Tests (Flutter)
cd lucidity-mobile
flutter test
```

---

## 2. Integration Smoke Tests (Headless)
Verifies that the protocol and networking layers can handshake without the full GUI.

### HEADLESS_SMOKE_01: Basic TCP Handshake
1. Start headless host: `cargo run -p lucidity-host --bin lucidity-host-server` (if available, else via wezterm-gui)
2. Run client: `cargo run -p lucidity-client -- --addr 127.0.0.1:9797`
3. **Success**: Client lists panes and successfully attaches to one.

### HEADLESS_SMOKE_02: Relay Roundtrip
1. Start relay: `cargo run -p lucidity-relay`
2. Configure host for relay: `LUCIDITY_RELAY_URL=ws://localhost:9090 LUCIDITY_RELAY_ID=test cargo run -p wezterm-gui`
3. Connect client via relay: (Requires client relay support)
4. **Success**: Bytes flow through relay from host to client.

---

## 3. Manual Integration Tests (Real World)
These tests require a physical mobile device or a well-configured emulator.

### REAL_WORLD_01: QR Pairing Flow
1. Open Lucidity on desktop.
2. Press `Ctrl+Shift+L` (or trigger via menu) to show QR code.
3. Open Lucidity on mobile.
4. Scan the QR code.
5. On desktop, click **Approve** in the overlay.
6. **Success**: Terminal output appears on the phone.

### REAL_WORLD_02: Connectivity Cascade
1. **LAN Test**: Connect phone to same Wi-Fi. Connection indicator should show **LAN**.
2. **P2P Test**: Switch phone to Cellular data. If UPnP is active on router, connection indicator should show **Direct** or **UPnP**.
3. **Relay Test**: Block port 9797 on computer firewall or disable UPnP. Connection should move to **Relay**.
4. **Success**: Connection stays active or successfully reconnects during network shifts.

### REAL_WORLD_03: Interaction & Hardware
1. **Typing**: Verify special keys (Esc, Tab, Ctrl+C) on mobile toolbar work correctly.
2. **Resizing**: Rotate phone to landscape. Verify the desktop terminal adjusts its columns/rows.
3. **Pasting**: Copy text on phone, use **Paste** button in mobile toolbar. Verify it appears on desktop.
4. **Gestures**: Swipe left/right on mobile terminal. Verify it switches between active Lucidity tabs.

---

## 4. Security & Edge Case Tests
### SEC_01: Invalid Pairing
1. Manually tamper with a QR payload (change public key).
2. Attempt to pair.
3. **Success**: Mobile or Host rejects the handshake with an error.

### SEC_02: Expired QR
1. Generate QR code on desktop.
2. Wait > 5 minutes.
3. Attempt to scan.
4. **Success**: Pairing is rejected due to expired timestamp.

### SEC_03: Max Clients
1. Connect 4 mobile devices to the same host.
2. Attempt to connect a 5th.
3. **Success**: Host rejects the 5th connection (default limit is 4).

### SEC_04: LAN Warning
1. Set `LUCIDITY_LISTEN=0.0.0.0:9797`.
2. Start host.
3. **Success**: Check logs for `SECURITY WARNING: Lucidity host listening on 0.0.0.0`.

---

## 5. Test Environment Matrix
| OS | Mobile | Connection |
| :--- | :--- | :--- |
| Windows 11 | Android 13 | Wi-Fi (LAN) |
| macOS | iOS 17 | Cellular (P2P/Relay) |
| Linux | Android 14 | Tailscale / Corporate NAT (Relay) |
