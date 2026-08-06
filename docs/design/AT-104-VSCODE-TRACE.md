# AT-104 native VS Code/Cursor trace

Status: implementation trace for the deterministic extension candidate

Authority: `.orchestrator/DESIGN/NATIVE_UI_CONTRACT.md` and protocol v1 fixtures in `agent-protocol/tests/fixtures`

## Boundary

The extension is a client. The desktop host owns catalog, lifecycle, runtime, usage, and persistence truth. The fixture host is an in-process substitute for that authority during tests and the pre-host demo; the extension retains only the current render snapshot and connection state in memory. It does not read provider files or persist a second catalog.

The UI uses only these VS Code APIs:

- Activity Bar `viewsContainers` contribution;
- three native `TreeView` instances backed by `TreeDataProvider`;
- native commands, context menus, notifications, and modal stop confirmation;
- one native `StatusBarItem`;
- `ThemeIcon`, `ThemeColor`, and workbench accessibility information.

There is no WebView, custom CSS, xterm attachment, background polling, ambient animation, or scheduled state animation. Reconnect delay is transport recovery, not visual animation.

## Component trace

| Extension surface | Native contract surface | Implementation |
|---|---|---|
| Activity Bar container | `lucidity.sidebar` | `viewsContainers.activitybar`, achromatic Weld Frame SVG using `currentColor` |
| Current Workspace view | `.section.active` | Exact normalized workspace-path match; host order retained |
| Other Projects view | `.section.active` | Active conversations not matching the current workspace |
| Settled view | `.section.history` | Organization state only; runtime remains visible and unchanged |
| Conversation item | `.row.{ConversationId}` | Stable ID, title, project, real event tally, runtime token, icon/cap label, adapter, and WELD text |
| Open/Focus | row activation | Native TreeItem command and context action |
| Settle/Unsettle | `.action.{settle,unsettle}` | Exact `conversation.setOrganizationState` request; does not stop |
| Stop | `.action.stop` | Native modal confirmation followed by `conversation.stop` |
| New | `.action.new` | Exact active-workspace filesystem path plus configured adapter/profile IDs |
| Usage | `lucidity.titlebar.usage` | Native status item with separate `Quota` and `Context` labels |
| Offline/recovery | `lucidity.notice.offline/recovery` | View message, welcome guidance, plug icon, and explicit Reconnect command |

The full machine-readable mapping lives in `lucidity-vscode-tokens.json`.

## Runtime trace

Every runtime state has four independent carriers: a token, a human label, a cap-shape description in the accessible name, and a Codicon. Theme colors are supplemental and never carry state alone.

| Runtime | Token | Codicon | Native theme color | Shape phrase |
|---|---|---|---|---|
| NotRunning | `IDLE` | `circle-outline` | `descriptionForeground` | centered hollow cap |
| Starting | `START` | `run` | `charts.blue` | solid top third |
| Working | `WORK` | `pulse` | `charts.blue` | full solid with event tick |
| WaitingForInput | `NEEDS` | `question` | `charts.yellow` | full with one notch |
| AwaitingApproval | `APPR` | `shield` | `charts.orange` | full with two notches |
| CompletedIdle | `DONE` | `pass-filled` | `testing.iconPassed` | centered solid tick |
| Failed | `FAIL` | `error` | `testing.iconFailed` | full with cross |
| UnknownExternal | `EXT?` | `circle-large-outline` | `descriptionForeground` | dashed cap |

VS Code owns selection, focus, hover, keyboard navigation, reduced motion, high-contrast resolution, and screen-reader exposure. The extension does not hard-code the native Rust surface colors into the workbench; it maps semantic state to workbench theme roles and preserves the contract through text and shape labels.

## Focus, keyboard, and accessibility

- Tree Views retain native visual/logical order and native Arrow, Home, End, Page, Enter, Tab, Shift+Tab, and context-menu behavior.
- Enter invokes the TreeItem Open/Focus command.
- Every row has an explicit accessible label containing title, attachment, runtime token and label, cap shape, organization state, adapter, project, and event tally.
- Destructive Stop is never a one-key action and always uses a native modal confirmation.
- All commands remain available from the Command Palette and native item context menus.
- Offline/error text names the missing host and the Reconnect action instead of presenting color-only failure.
- The status item has an explicit button role and keeps account quota separate from conversation context in both visible and accessible text.

The deterministic tests cover all 16 organization/runtime combinations and assert that each carries token, shape, icon, theme role, and accessible wording.

## Protocol and recovery

The client sends the exact protocol-v1 spellings frozen by `agent-protocol`:

1. `host.hello` with min/max version 1;
2. `conversation.list` with a null cursor and bounded limit;
3. `event.subscribe`;
4. typed incremental refresh requests for catalog, conversation, usage, and adapter events.

Response IDs must match request IDs, error payloads fail closed, and negotiated versions outside v1 are rejected. A disconnect moves the views/status to explicit offline state, schedules a bounded reconnect attempt, then repeats the complete handshake and snapshot. The fixture host exposes deterministic availability and event controls for tests.

`fixture-presentation-v1` is an in-process fixture capability supplying runtime and usage presentation data that the frozen public v1 response set does not yet expose. It is not claimed as a new wire method. AT-204 binds the client to the accepted desktop IPC for catalog, commands, attachment edges, and event hints while rendering conservative fallbacks for the presentation fields protocol v1 still cannot supply.

### AT-204 Windows transport

Local mode uses Node's `net.Socket` against a local Windows named pipe. Frames are a little-endian `u32` byte count plus fatal-decoded UTF-8 JSON and are bounded to `1_048_576` body bytes in both directions. The transport keeps one pending-request entry per safe-integer ID, accepts replies only for a live matching ID, distinguishes typed events from responses, rejects unknown envelopes, rejects remote pipe paths, rejects all pending work on loss, and suppresses disconnect notifications for intentional disposal.

The Rust host pipe-name implementation is not present in the integrated base, so the extension exposes `lucidity.pipeName` as the compatibility authority. It accepts a simple local name or complete local `\\.\pipe\...` path. `lucidity-control-v1` is the sole shared default until the desktop host publishes its owner-SID-hash derivation; the pipe name is not treated as a security boundary.

Every initial connection and reconnect repeats `host.hello`, follows every `conversation.list.nextCursor` until the complete bounded snapshot is loaded, then subscribes. A repeated cursor, `resyncRequired` page, malformed/oversized frame, unknown response ID, disconnect, or protocol decode failure fails closed and returns to the same full-snapshot recovery path.

## Fixed-frame receipts

Native Tree Views do not expose a supported API for programmatically forcing workbench pixel dimensions or taking deterministic screenshots. The harness therefore keeps semantic fixed-frame fixtures for the accepted extension sizes:

| Fixture | Purpose |
|---|---|
| 600×320 | minimum client corridor |
| 900×600 | compact corridor |
| 1280×800 | default corridor |
| 1920×1080 | wide corridor |

For each frame, tests freeze section membership/order and status text. These are semantic snapshots, not a claim of screenshot or pixel-render acceptance. A real Cursor/VS Code integration harness may add screenshots later without changing the client model.

## Package receipt

The extension compiles with TypeScript, lints with ESLint, runs Node's deterministic tests without the desktop host, and packages with `@vscode/vsce`. Activation is limited to contributed views and commands; there is no eager `*` activation.
