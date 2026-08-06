# Lucidity for VS Code and Cursor

Lucidity is a thin native client for Lucidity Agent Terminal. It contributes an Activity Bar container, three native Tree Views, commands, and a status-bar usage summary. It does not use a WebView or read provider session files.

The extension defaults to a deterministic in-process fixture host so it can be compiled, tested, and demonstrated before the desktop IPC transport is available. Set `lucidity.hostMode` to `local` to exercise the explicit missing-host guidance; the local transport is intentionally deferred to the host-integration task.

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

The test suite uses only the fixture host and does not require the desktop application.
