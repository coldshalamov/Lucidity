# Phase 3: Direct Connectivity & P2P

This phase moved Lucidity away from a centralized relay model to a direct, secure P2P architecture.

## Completed Tasks

### 1. Relay Deprecation
- Entirely removed `lucidity-relay`, `lucidity-relay-agent`, and `lucidity-auth` crates.
- Updated `Cargo.toml` and workspace configuration to reflect these removals.
- Cleaned up integration points in `wezterm-gui`.

### 2. Direct Connection Strategy (Mobile)
- Implemented `connectWithStrategy` in `LucidityClient`.
- Mobile app now attempts connections in this order:
  1. **LAN Direct**: Uses the discovered LAN IP and port 9797.
  2. **External Direct (UPnP)**: Uses the public IP and port mapped via UPnP.
- Fallback to manual host/port entries for VPN/Tailscale users.

### 3. P2P Discovery & STUN Integration
- `lucidity-host` now uses `igd` for automatic UPnP port mapping.
- Integrated `stun` crate to discover public IP and port fallbacks when UPnP doesn't provide a clear external address.
- Pairing QR codes now embed `lanAddr` and `externalAddr` for zero-config setup.

### 4. Mutual Authentication & Security
- **Nonce-based Handshake**:
  1. Host sends `AuthChallenge` with a nonce.
  2. Mobile signs the nonce + its own client nonce, sends `AuthResponse`.
  3. Host verifies mobile's signature against `devices.db`.
  4. Host signs the mobile's client nonce and sends `AuthSuccess`.
  5. Mobile verifies host's signature using the public key from the pairing QR.
- This ensures both sides are who they claim to be without a third-party server.

## Remaining Work / Follow-Ups

### Technical Debt
- **STUN Blocking Call**: Current implementation uses `#[tokio::main(flavor = "current_thread")]` within a thread for STUN discovery. This is functional but could be refactored to use a shared async runtime.
- **Port Matching**: STUN discoveries often result in different external ports than the local port. We currently capture this, but some routers have symmetric NAT which might still cause issues without true hole-punching logic.

### Polish & UX
- **Connection Status UI**: The mobile app should show more detail about *how* it's connected (e.g., "Connected via LAN" vs "Connected via Internet").
- **Manual Peer Discovery**: For networks without UPnP or STUN success, a manual "Add via IP" flow is maintained but needs testing with Tailscale/Headscale.

### Reliability
- **Background Refresh**: The `lucidity-p2p-refresh` thread in `lucidity-host` handles UPnP lease renewal every 30 minutes. This needs long-term reliability verification.
