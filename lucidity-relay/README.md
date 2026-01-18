# Lucidity Relay

This is the **mandatory** relay server for Lucidity.

## Purpose

It brokers connections between:
1.  The WezTerm Host (running on your desktop)
2.  The Lucidity Mobile App (running on your phone)

## Why does this exist?

To solve the "Works Anywhere" requirement **without VPNs**.
- Desktop connects OUTBOUND to this relay.
- Mobile connects OUTBOUND to this relay.
- The relay blindly forwards encrypted frames between them.

## Architecture Rules

1.  **No Logic**: This relay should be as "dumb" as possible. It just shuttles bytes.
2.  **No Storage**: It does not store session logs or history.
3.  **Ephemeral**: If the relay restarts, all connections drop (clients will reconnect).
4.  **Auth**:
    *   Desktop registers with a `RelayId` (derived from its Public Key).
    *   Mobile connects to `RelayId` and proves it has the paired signature.

## Tech Stack
*   Rust
*   Tokio (Async I/O)
*   WebSockets (for easy traversal through firewalls/proxies like Cloudflare/Nginx)
