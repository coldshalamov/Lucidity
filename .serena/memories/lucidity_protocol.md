# Lucidity host protocol (LAN MVP)

Host defaults to `127.0.0.1:9797` (set `LUCIDITY_LISTEN=0.0.0.0:9797` for phone-on-LAN testing).

Transport framing (see `lucidity-proto/src/frame.rs` and `lucidity-host/src/server.rs`):
- 4 bytes LE length = `1 + payload_len`
- 1 byte type
- payload bytes

Frame types (see `lucidity-host/src/protocol.rs`):
- `1` = JSON request/response
- `2` = pane output (server -> client)
- `3` = pane input (client -> server)

JSON uses `{ "op": "..." }` (snake_case), not `{ "cmd": "..." }`.

Key ops (see `lucidity-host/src/server.rs`):
- `list_panes` -> `{ op: "list_panes", panes: [...] }`
- `attach` + `pane_id` -> `{ op: "attach_ok", pane_id: ... }` then output frames stream
- `pairing_payload` -> `{ op: "pairing_payload", payload: { desktop_public_key, relay_id, timestamp, version } }`
- `pairing_submit` + `{ request: { mobile_public_key, signature, user_email, device_name, timestamp } }`
  -> `{ op: "pairing_response", response: { approved, reason? } }`

Pairing QR format (see `lucidity-pairing/src/qr.rs`):
- `lucidity://pair?data=<base64url_no_pad(JSON PairingPayload)>`

