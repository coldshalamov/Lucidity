# Lucidity Protocol (Phase 1)

Phase 1 uses a minimal TCP framing protocol intended to be easy to evolve into the full v0.1 spec.

## Framing

Each message is:

- `len` (u32 little-endian): number of bytes that follow (including `type`)
- `type` (u8)
- `payload` (`len - 1` bytes)

See `lucidity-proto/src/frame.rs`.

## Message types (Phase 1)

- `TYPE_JSON = 1`: JSON request/response messages
- `TYPE_PANE_OUTPUT = 2`: raw PTY output bytes (host → client)
- `TYPE_PANE_INPUT = 3`: input bytes (client → host)

## JSON ops

Requests:

- `{"op":"list_panes"}`
- `{"op":"attach","pane_id":123}`

Responses:

- `{"op":"list_panes","panes":[{"pane_id":123,"title":"bash"}]}`
- `{"op":"attach_ok","pane_id":123}`
- `{"op":"error","message":"..."}`
