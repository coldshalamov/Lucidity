# Lucidity Security Model

This document outlines the security architecture for the Lucidity remote access system, covering threat models, cryptographic primitives, and defense mechanisms.

## Architecture Note

**Lucidity uses P2P-first connectivity.** The relay server is a FALLBACK only used when direct connections fail (symmetric NAT, corporate firewalls). Most connections will be direct P2P (LAN, UPnP, or STUN).

## 1. System Components & Trust Boundaries

| Component | Trust Level | Description |
|-----------|-------------|-------------|
| **Mobile App** | Trusted | Holds private key (Ed25519) in secure storage. Authenticates user via fingerprint/biometrics. |
| **WezTerm Host** | Trusted | Runs on user's desktop. Holds private key (Ed25519) and trust store (sqlite). Controls PTY access. |
| **Relay Server** | **Untrusted** | FALLBACK broker when P2P fails. Routes encrypted traffic. Does **not** terminate session encryption. |

### Trust Model
*   **P2P Primary**: Direct connections (LAN, UPnP, STUN) are preferred and don't involve any third-party server.
*   **End-to-End Auth**: Both endpoints verify each other's identity via Ed25519 signatures, regardless of transport (direct or relay).
*   **Relay is Untrusted**: The relay server cannot read session data. It only routes opaque WebSocket messages.
*   **Mutual Authentication**: Desktop and Mobile mutually authenticate via Ed25519 signatures on every connection.

## 2. Authentication & Pairing

### Device Pairing (LAN Only)
Pairing establishes initial trust between a Mobile device and a Desktop host.

1.  **Discovery**: Mobile scans QR code on Desktop screen containing `(DesktopIP, DesktopPort, DesktopPublicKey, PairingParams)`.
2.  **Handshake**:
    *   Mobile connects to Desktop via LAN (TLS/TCP).
    *   Mobile generates ephemeral `PairingRequest` signed with its new Ed25519 key.
    *   Desktop verifies signature and user approval (GUI prompt).
3.  **Trust Store**:
    *   Desktop stores `MobilePublicKey` in `devices.db`.
    *   Mobile stores `DesktopPublicKey` and `DesktopRelayID` in secure storage.

### Session Authentication (All Connections)
The same authentication handshake is used for all connection types (LAN, UPnP, STUN, or Relay):

1.  **Connection**: Mobile connects to Desktop (directly or via Relay)
2.  **End-to-End Auth Handshake** (Implemented in `lucidity-host`):
    *   **Challenge**: Host sends `AuthChallenge` with random nonce.
    *   **Response**: Mobile signs `(server_nonce || client_nonce)` with its Ed25519 private key.
    *   **Verification**: Host verifies signature against `devices.db`. If valid, proceeds. If invalid, drops connection.
    *   **Host Verification**: Host signs `client_nonce` and sends `AuthSuccess`.
    *   **Mobile Verification**: Mobile verifies host's signature using stored `desktop_pubkey`.

This mutual authentication ensures both sides are who they claim to be, regardless of transport.

## 3. Threat Mitigation

| Threat | Mitigation | Implementation Status |
|--------|------------|-----------------------|
| **Man-in-the-Middle (Relay)** | Relay only sees encrypted websocket traffic (TLS). End-to-end auth prevents relay from spoofing mobile. | ✅ Host Auth Handshake |
| **Desktop Impersonation** | Mobile verifies Desktop's signature during pairing. Relay enforces `DesktopSecret`. | ✅ Relay Auth Modes |
| **Relay Abuse** | Relay requires `LUCIDITY_RELAY_JWT_SECRET` for mobiles and shared secret for desktops. | ✅ Relay Security |
| **Replay Attacks** | Auth handshake uses unique nonces. | ✅ Host Nonce Logic |
| **Unpaired Access** | Host rejects non-localhost connections that fail signature verification. | ✅ Host Bridge Logic |

## 4. Operational Security

*   **Relay Configuration**:
    *   **TLS Required**: `LUCIDITY_RELAY_REQUIRE_TLS=1` enforces HTTPS/WSS.
    *   **Auth Required**: Default mode. `LUCIDITY_RELAY_NO_AUTH=true` is only for development and prints warnings.
*   **Host Configuration**:
    *   **Bind Address**: Defaults to localhost. Binding to `0.0.0.0` triggers security warnings.
    *   **Device Management**: Users can list/revoke trusted devices via CLI or future GUI.

## 5. Future Hardening (Post-MVP)

*   **Payload Encryption**: Currently relying on TLS. Plan to implement `ChaCha20-Poly1305` for inner tunnel payload encryption using keys derived from the Ed25519 exchange.
*   **Perfect Forward Secrecy**: Ephemeral session keys rotated per session.

---

## 6. Production Security Checklist

Before deploying Lucidity to production, verify:

### Relay Server
- [ ] `LUCIDITY_RELAY_REQUIRE_TLS=true` is set
- [ ] `LUCIDITY_RELAY_NO_AUTH` is NOT set (or set to `false`)
- [ ] `LUCIDITY_RELAY_DESKTOP_SECRET` is a strong random string (32+ bytes)
- [ ] TLS certificate is valid and from a trusted CA
- [ ] Relay is behind a firewall allowing only ports 443 (WSS)
- [ ] Rate limiting is configured (if applicable)
- [ ] Logs are configured to not expose tokens

### Desktop Host
- [ ] `LUCIDITY_LISTEN` is set to `127.0.0.1:9797` (localhost only) unless LAN access is explicitly intended
- [ ] Trust store (`devices.db`) is protected with appropriate file permissions
- [ ] Only approved devices are in the trust store

### Mobile App
- [ ] Secure storage is enabled for keys (Keychain/Keystore)
- [ ] App is built in release mode with obfuscation
- [ ] No debug logging in production builds
- [ ] Certificate pinning enabled (if using custom relay domain)

### Network
- [ ] All connections use WSS (TLS)
- [ ] DNS is secured (DNSSEC or trusted resolver)
- [ ] Desktop firewall allows outbound to relay only

---

## 7. Incident Response

If you suspect a security breach:

1. **Revoke Access**: Remove all devices from the Desktop trust store.
2. **Rotate Secrets**: Generate new `LUCIDITY_RELAY_DESKTOP_SECRET`.
3. **Check Logs**: Review relay logs for unusual connection patterns.
4. **Re-Pair Devices**: Re-establish trust with known devices only.
5. **Report**: If you discover a vulnerability, please email **security@lucidity.app**.

