# Lucidity Mobile - Implementation Plan

**Goal**: Build a complete, working Android/iOS terminal client for Lucidity (WezTerm) in "one shot".
**Stack**: Flutter (Dart)
**Key Libraries**: `xterm` (Terminal Emulator), `provider` (State Management)

## Phase 0: Verification (DONE)
- [x] Backend `lucidity-host` protocol verified.
- [x] Protocol defined: 4-byte LE Length + 1-byte Type + Payload.
- [x] Types: `1=JSON`, `2=Output`, `3=Input`.

**Note (important):** JSON messages use `{ "op": "..." }` (not `{ "cmd": "..." }`), and the desktop defaults to `127.0.0.1:9797`.

## Phase 1: Project Initialization (DONE)

This step must be run by the agent/user with `flutter` installed.

```bash
# In d:\GitHub\Lucidity\
cd lucidity-mobile
# Initialize project
flutter create . --project-name=lucidity_mobile --org=com.coldshalamov.lucidity
# Add dependencies
flutter pub add xterm google_fonts provider uuid
```

## Phase 2: Core Protocol Implementation (Pure Dart) (DONE)

We will re-implement the Lucidity Protocol in Dart to avoid complex Rust FFI bridging. Implementation is simple.

### 1. `lib/protocol/constants.dart`
```dart
const int maxFrameLen = 16 * 1024 * 1024;
const int typeJson = 1;
const int typePaneOutput = 2;
const int typePaneInput = 3;
```

### 2. `lib/protocol/frame.dart`
- **Function**: `Uint8List encodeFrame(int type, List<int> payload)`
  - Logic:
    1. Calculate `len = payload.length + 1`.
    2. Write `len` as 4 bytes (Little Endian).
    3. Write `type` as 1 byte.
    4. Write `payload`.
- **Class**: `FrameDecoder`
  - **State**: internal byte buffer.
  - **Method**: `void push(Uint8List data)`: Adds data to buffer.
  - **Method**: `Frame? nextFrame()`:
    - Checks if buffer has >= 4 bytes.
    - Reads first 4 bytes as `length` (LE).
    - Checks if buffer has `4 + length`.
    - Extracts `type` (5th byte) and `payload`.
    - Removes consumed bytes.
    - Returns `Frame(type, payload)`.

### 3. `lib/protocol/lucidity_client.dart`
- **Class**: `LucidityClient` (extends `ChangeNotifier`)
  - **State**:
    - `Socket? _socket`
    - `bool connected`
    - `List<PaneInfo> panes`
    - `FrameDecoder _decoder`
  - **Methods**:
    - `Future<void> connect(String host, int port)`:
      - Opens TCP socket.
      - Listens to socket -> `_decoder.push(data)` -> `_processFrames()`.
    - `void _processFrames()`:
      - While `frame = _decoder.nextFrame()`:
        - If `typeJson`: Parse map. Handle `list_panes` response.
        - If `typePaneOutput`: Broadcast raw bytes to the UI.
    - `void sendListPanes()`: Sends `{"op": "list_panes"}` as JSON frame.
    - `void attach(int paneId)`: Sends `{"op": "attach", "pane_id": paneId}`.
    - `void sendInput(String data)`: Sends `typePaneInput` frame (raw bytes). The desktop applies input to the currently attached pane for this connection (**attach-scoped I/O**).

## Phase 3: Terminal Emulation (The "Look and Feel") (DONE)

We DO NOT need to write a VT100 parser. We use `xterm.dart`.

Implementation is currently done directly inside `lib/screens/terminal_screen.dart`:
- Links `LucidityClient.outputStream` -> `Terminal.write()`.
- Links `Terminal.onOutput` (keystrokes) -> `LucidityClient.sendInput()`.

## Phase 4: UI Implementation (DONE)

### 1. `lib/main.dart`
- Setup `MultiProvider` for `LucidityClient`.
- Theme: Dark mode, "Inter" or "JetBrains Mono" font equivalent.

### 2. `lib/screens/connect_screen.dart`
- **UI**:
- TextField: `Host` (default: local IP).
  - TextField: `Port` (default: 9797).
  - Button: "Connect".
  - List: Shows `panes` fetched after connection.
  - Tap a pane -> Navigate to `TerminalScreen`.

### 3. `lib/screens/terminal_screen.dart`
- **UI**:
  - `TerminalView` widget (from `xterm` package).
  - Configured with `autofocus: true`.
  - On-screen accessory bar (optional): ESC, CTRL, TAB keys (mobile keyboard helpers).

## Phase 5: Execution Steps for Codex

1.  **Scaffold**: Execute Phase 1 commands.
2.  **Protocol**: Create `lib/protocol/*` files. Testing: Create a unit test `test/protocol_test.dart` that mimics the Rust `frame_roundtrip` test.
3.  **UI Construction**: Create the Screens.
4.  **Wiring**: Hook up `client.dart` to `xterm.dart`.
5.  **Run**: `flutter run -d windows` (to test on desktop first easily) or `flutter run -d chrome` (if web supported) or Android Emulator.

## Success Criteria for "One Shot"
- App launches.
- User enters IP:Port.
- Connects to `wezterm` (Lucidity Host).
- Shows pane list.
- Clicking pane shows live terminal.
- Typing in phone sends keys to WezTerm.
