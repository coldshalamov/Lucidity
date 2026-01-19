# Lucidity Manual Verification Checklists

This document contains verification checklists for manual testing of the Lucidity system components.

---

## Desktop Verification Checklist

### Prerequisites
- [ ] WezTerm built with Lucidity features enabled
- [ ] Environment variables set (or defaults used)

### Basic Functionality
- [ ] WezTerm launches without errors
- [ ] Lucidity host starts automatically on launch
- [ ] QR code shown via Ctrl+Shift+L or menu
- [ ] Relay ID displayed in config overlay
- [ ] Relay URL displayed correctly

### Relay Agent (if relay mode enabled)
- [ ] LUCIDITY_RELAY_URL set correctly
- [ ] LUCIDITY_RELAY_DESKTOP_SECRET set
- [ ] Agent connects to relay within 5 seconds
- [ ] Agent auto-restarts if killed
- [ ] Agent shows "Connected" in logs

### Toggle Functionality
- [ ] "Enable/Disable Relay" toggle works
- [ ] Disabling immediately stops agent
- [ ] Re-enabling restarts agent
- [ ] State persists across restart

### Pairing Flow
- [ ] QR code scannable by mobile app
- [ ] Pairing request appears on desktop
- [ ] "Approve" adds device to trusted list
- [ ] "Deny" rejects connection

---

## Relay Verification Checklist

### Prerequisites
- [ ] Relay deployed (local or cloud)
- [ ] LUCIDITY_RELAY_LISTEN configured
- [ ] TLS certificate (if production)

### Basic Health
- [ ] `/healthz` returns 200 OK
- [ ] Relay logs show startup message
- [ ] No error logs on idle

### Desktop Registration
- [ ] Desktop connects via WebSocket
- [ ] "Desktop registered" log appears
- [ ] Duplicate relay ID rejected with 409

### Mobile Connection
- [ ] Mobile can connect to `/ws/mobile/{relay_id}`
- [ ] Session creation message sent to desktop
- [ ] 404 returned for offline desktops
- [ ] 401 returned for invalid auth (if auth enabled)

### Session Flow
- [ ] Desktop receives session request
- [ ] Desktop can accept session
- [ ] Desktop can reject session
- [ ] Mobile receives accept/reject notification

### Data Tunnel
- [ ] Binary data flows mobile → desktop
- [ ] Binary data flows desktop → mobile
- [ ] Low latency (< 100ms on LAN)
- [ ] Connection survives 10+ minutes idle

### Security
- [ ] TLS enforced when LUCIDITY_RELAY_REQUIRE_TLS=true
- [ ] Auth required when LUCIDITY_RELAY_NO_AUTH not set
- [ ] Invalid fingerprint rejected at tunnel

---

## Mobile Verification Checklist

### Prerequisites
- [ ] Flutter app installed on device
- [ ] Camera permissions granted
- [ ] Network connectivity

### App Launch
- [ ] App launches without crash
- [ ] Home screen displays
- [ ] Previously paired desktops shown

### QR Scanning
- [ ] Camera opens when "Scan QR" tapped
- [ ] QR code recognized within 2 seconds
- [ ] Invalid QR shows error message
- [ ] Success navigates to pairing screen

### Pairing Flow
- [ ] Pairing request sent to desktop
- [ ] Waiting screen shows progress
- [ ] Approval navigates to terminal
- [ ] Rejection shows message and returns

### Connection States
- [ ] "Connecting..." shown during connect
- [ ] "Connected" shown on success
- [ ] "Reconnecting..." on network drop
- [ ] "Error" with message on failure
- [ ] Retry button works

### Terminal Rendering
- [ ] Terminal fills screen
- [ ] Text renders correctly (no artifacts)
- [ ] Colors display properly
- [ ] Cursor visible and positioned
- [ ] Scrollback works

### Input Handling
- [ ] Keyboard input works
- [ ] Backspace deletes correctly
- [ ] Enter sends newline
- [ ] Accessory bar buttons work (Esc, Tab, Ctrl+C)
- [ ] Arrow keys navigate

### Session Management
- [ ] Multiple tabs can be opened
- [ ] Tab switching works
- [ ] "Close tab" works
- [ ] Last session remembered on restart

### Auto-Reconnect
- [ ] App reconnects after network drop
- [ ] Exponential backoff visible in UI
- [ ] Manual disconnect doesn't auto-reconnect
- [ ] "Clear Session" stops auto-connect on launch

### Error Handling
- [ ] Timeout shows user-friendly message
- [ ] Connection refused shows suggestion
- [ ] Network error shows recovery tip
- [ ] "Go Back" returns to home

---

## Security Verification Checklist

### Authentication
- [ ] Unpaired devices cannot control terminal
- [ ] Pairing requires desktop user approval
- [ ] Auth tokens validated on relay
- [ ] Invalid tokens rejected with 401

### Encryption
- [ ] TLS enabled on relay in production
- [ ] WebSocket connections use WSS
- [ ] No plaintext credentials in logs

### Defense in Depth
- [ ] Localhost binding warning shown
- [ ] All-interface binding warning shown
- [ ] Session fingerprint validated
- [ ] Rate limiting applied (if enabled)

### Data Protection
- [ ] Keypair stored securely on mobile
- [ ] Keypair stored securely on desktop
- [ ] Session data not logged
- [ ] No sensitive data in crash reports

---

## Test Results

| Component | Date | Tester | Pass/Fail | Notes |
|-----------|------|--------|-----------|-------|
| Desktop   |      |        |           |       |
| Relay     |      |        |           |       |
| Mobile    |      |        |           |       |
| Security  |      |        |           |       |

---

## Known Issues

(List any issues discovered during testing)

1. 

---

## Sign-off

Tested by: ___________________

Date: ___________________

Version: ___________________
