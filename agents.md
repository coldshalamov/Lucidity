# Agents Guide to Lucidity Repo

This repository is **Lucidity**, a fork of [WezTerm](https://github.com/wez/wezterm) that adds a mobile connectivity layer.

## 🚨 CRITICAL INSTRUCTION FOR AGENTS 🚨

**Do NOT attempt to read or analyze the entire `wezterm` codebase.** It is massive and mostly irrelevant to your tasks.

## 🚨 CRITICAL ARCHITECTURE RULES (DO NOT HALLUCINATE) 🚨

1.  **NO VPNs / Tailscale / WireGuard**: Never suggest the user install a 3rd party VPN to make this work. The app must work "native" over the internet using our own infrastructure.
2.  **NO "LAN Only" Defaults**: While LAN discovery is a nice-to-have optimization, the *core* architecture is Internet-first via a Relay.
3.  **The Only Allowed Architecture: Custom Relay (`lucidity-relay`)**
    *   **Desktop**: Connects outbound to `lucidity-relay` (WebSocket/TLS).
    *   **Mobile**: Connects outbound to `lucidity-relay` (WebSocket/TLS).
    *   **Relay**: Brokers the connection. No incoming ports needed on desktop.
    *   **Auth**: Keypair-based. QR code exchange verifies identity; Relay enforces it.

**If you are planning "Phase 2" or "Remote Access", you MUST implement `lucidity-relay`. Do not look for shortcuts.**

You are likely here to work on the **Lucidity Mobile Integration**.

### 1. Where to Focus (The "Lucidity" Layer)
These directories contain the new code we are building. **Spend 95% of your tokens here.**

| Directory | Purpose |
|-----------|---------|
| `lucidity-host/` | The core host service running inside WezTerm. Handles network bridging. |
| `lucidity-relay/` | **[REQUIRED]** The internet relay server (Rust + Tokio). Brokers connections. |
| `lucidity-mobile/` | **[ACTIVE]** The Flutter mobile app (iOS/Android). |
| `lucidity-client/` | Client library for the mobile app (and potential CLI tools). |
| `lucidity-pairing/` | **[ACTIVE]** Pairing protocol, QR codes, and crypto handshake. |
| `lucidity-proto/` | Shared Protocol Buffers / Struct definitions. |

### 2. Integration Points (The "Glue")
We have surgically modified WezTerm to hook Lucidity in. Only look at these specific files if you are debugging startup or UI integration.

*   `wezterm-gui/src/main.rs`:
    *   Starts the `lucidity_host` service (`lucidity_host::autostart_in_process()`).
*   `wezterm-gui/src/termwindow/mod.rs`:
    *   Hooks pairing requests (`GuiPairingApprover`).
    *   Shows the initial splash screen (`maybe_show_lucidity_pairing_splash`).
*   `wezterm-gui/src/pairing_handler.rs`:
    *   **NEW**: Handles the async flow of pairing approvals.
*   `wezterm-gui/src/overlay/`:
    *   `lucidity_pair.rs`: The QR code splash screen.
    *   `lucidity_pair_approve.rs`: The "Approve this device?" dialog.

### 3. The "WezTerm Core" (IGNORE THIS)
**Treat these as read-only libraries.** Do not scan them unless specifically asked to debug deep terminal emulation issues.

*   `wezterm-*` (except `wezterm-gui`)
*   `termwiz`, `term`, `mux`, `window`
*   `deps/`, `test-data/`

### 4. Build & Test Cheatsheet

### Current Project Status (as of 2026-01-18)

**Implemented + building on Windows:**
- Phase 1 (Local mirroring proof): `lucidity-host`, `lucidity-client`, `lucidity-proto`
- Phase 2 (Mobile App): `lucidity-mobile` (Flutter) - **Basic UI & QR Scanning implemented.**
- Phase 3 (Pairing UX MVP): `lucidity-pairing` + `wezterm-gui` overlays + approve/reject flow

**NOT implemented (major product gaps):**
- `lucidity-relay` (The dedicated relay server is missing - **PRIORITY #1**)
- Remote connectivity logic in host/mobile
- Account/OAuth + device management UX for production shipping

If the goal is “App Store tonight”, this repo is not there yet: you still need to build the actual mobile apps and the security model for non-LAN usage.

*   **Build everything:**
    ```powershell
    cargo build --package wezterm-gui
    ```
*   **Test Pairing Only (Fast):**
    ```powershell
    cargo test -p lucidity-pairing
    ```
*   **Run WezTerm with Lucidity:**
    ```powershell
    # Windows
    target/debug/wezterm-gui.exe
    ```
*   **Env Vars:**
    *   `LUCIDITY_DISABLE_SPLASH=1`: Skip the QR code at startup.
    *   `LUCIDITY_LISTEN=0.0.0.0:9797`: Listen on all interfaces (default is localhost).

### Verification Commands (known-good)

If you’re unsure what’s broken, start here (fast, Lucidity-focused):

```powershell
cargo test -p lucidity-proto -p lucidity-host -p lucidity-pairing -p lucidity-client
cargo build -p wezterm-gui
```

### Dependency / License Audit

For security + compliance checks in CI:

```powershell
cargo audit
cargo deny check advisories
cargo deny check licenses
```

---
*If you are unsure where a file lives, assume it is in `lucidity-*` first. If you need to change how the terminal renders text, you are probably in the wrong place.*
