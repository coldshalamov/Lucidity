# Lucidity for VS Code and Cursor

Lucidity is a thin native client for Lucidity Agent Terminal. It contributes an Activity Bar container, three native Tree Views, commands, and a status-bar usage summary. It does not use a WebView or read provider session files.

The extension defaults to a deterministic in-process fixture host for repeatable demos and tests. Set `lucidity.hostMode` to `local` to connect to Lucidity Agent Terminal over a local Windows named pipe. The shared default pipe name is `lucidity-control-v1`; set `lucidity.pipeName` when the desktop host uses its owner-SID-derived production name. The setting accepts either a simple name or a complete local `\\.\pipe\...` path and rejects remote paths.

The local transport uses the protocol-v1 little-endian `u32` byte-length prefix followed by UTF-8 JSON, rejects zero-length and over-1-MiB frames, correlates concurrent replies by request ID, and re-fetches every paginated conversation page after connection or continuity loss. Push events remain edge hints; reconnect always performs `host.hello`, a fresh complete snapshot, and `event.subscribe`.

Protocol v1 does not yet expose complete runtime, event-tally, or usage snapshots. Local rows therefore render catalog truth, attachment edges, and conservative runtime fallbacks; fixture-only presentation fields remain fixture-only rather than being invented on the wire.

## Commands

- New in Current Workspace
- Open or Focus Conversation
- Settle / Unsettle Conversation
- Stop Conversation
- Refresh Usage
- Open Lucidity
- Reconnect to Host

## Development

```powershell
npm ci
npm run check
npm run package
```

The test suite retains the fixture host and also creates real local Windows test pipes to exercise framing, response correlation, paginated reconnect snapshots, event delivery, size bounds, and clean disposal. It does not require the desktop application.
