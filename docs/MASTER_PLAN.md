# Lucidity Master Implementation Plan

This document provides a complete, step-by-step implementation plan for achieving the Lucidity product vision. Agents should follow this plan sequentially unless explicitly told otherwise.

## Product Vision Recap

**Goal**: Open desktop terminal → open mobile app → scan QR → control terminal from anywhere in the world.

**Architecture**: P2P-first with relay fallback. No mandatory server bottleneck.

## Current State (as of 2026-01-19)

### What Works
- Desktop host bridge with PTY streaming
- P2P connectivity via UPnP + STUN
- QR code pairing with Ed25519 signatures
- Device trust store (SQLite)
- Mutual authentication handshake
- Flutter mobile app with terminal rendering
- Premium UI (theme, gestures, keyboard toolbar)

### What's Missing
- Relay server as fallback for when P2P fails
- Automatic connection cascade (P2P → Relay)
- Device management UI
- App store release preparation

---

## Phase 5: Production Readiness

### Task 5.1: Relay Server Implementation

**Goal**: Build a stateless relay server that routes traffic when P2P fails.

**Files to Create**:
```
lucidity-relay/
├── Cargo.toml
├── src/
│   ├── main.rs           # Server entry point
│   ├── lib.rs            # Library exports
│   ├── session.rs        # Session management (desktop-mobile pairing)
│   ├── websocket.rs      # WebSocket connection handling
│   └── auth.rs           # Token validation
└── Dockerfile
```

**Implementation Steps**:

1. **Create `lucidity-relay/Cargo.toml`**:
   ```toml
   [package]
   name = "lucidity-relay"
   version = "0.1.0"
   edition = "2021"

   [dependencies]
   tokio = { version = "1", features = ["full"] }
   tokio-tungstenite = "0.21"
   warp = "0.3"
   serde = { version = "1", features = ["derive"] }
   serde_json = "1"
   uuid = { version = "1", features = ["v4"] }
   log = "0.4"
   env_logger = "0.10"
   dashmap = "5"
   ```

2. **Implement `src/main.rs`**:
   - Listen on `LUCIDITY_RELAY_LISTEN` (default `0.0.0.0:9090`)
   - Endpoints:
     - `GET /health` - Health check
     - `WS /desktop/{relay_id}` - Desktop connects here
     - `WS /mobile/{relay_id}` - Mobile connects here
   - When both desktop and mobile connect to same `relay_id`, bridge their WebSocket streams

3. **Implement `src/session.rs`**:
   ```rust
   struct Session {
       relay_id: String,
       desktop_tx: Option<mpsc::Sender<Message>>,
       mobile_tx: Option<mpsc::Sender<Message>>,
   }

   // When message from desktop → forward to mobile
   // When message from mobile → forward to desktop
   ```

4. **Implement `src/auth.rs`**:
   - Validate desktop secret: `LUCIDITY_RELAY_DESKTOP_SECRET`
   - Validate mobile JWT (optional for premium features)
   - No auth required for development (`LUCIDITY_RELAY_NO_AUTH=true`)

5. **Create `Dockerfile`**:
   ```dockerfile
   FROM rust:1.75 as builder
   WORKDIR /app
   COPY . .
   RUN cargo build --release -p lucidity-relay

   FROM debian:bookworm-slim
   COPY --from=builder /app/target/release/lucidity-relay /usr/local/bin/
   EXPOSE 9090
   CMD ["lucidity-relay"]
   ```

**Verification**:
```bash
cargo test -p lucidity-relay
cargo run -p lucidity-relay
# Should start on port 9090
```

---

### Task 5.2: Desktop Relay Integration

**Goal**: Desktop detects P2P failure and connects to relay as fallback.

**Files to Modify**:
```
lucidity-host/
├── src/
│   ├── relay_client.rs   # NEW: WebSocket client to relay
│   ├── server.rs         # Add relay fallback logic
│   └── lib.rs            # Export relay module
```

**Implementation Steps**:

1. **Create `lucidity-host/src/relay_client.rs`**:
   ```rust
   pub struct RelayClient {
       relay_url: String,
       relay_id: String,
       desktop_secret: String,
   }

   impl RelayClient {
       /// Connect to relay server via WebSocket
       pub async fn connect(&mut self) -> Result<()> {
           let url = format!("{}/desktop/{}?secret={}",
               self.relay_url, self.relay_id, self.desktop_secret);
           // Establish WebSocket connection
           // Bridge to local host server
       }

       /// Forward frames from relay to local pane handler
       pub async fn handle_relay_messages(&mut self) { ... }
   }
   ```

2. **Modify `lucidity-host/src/server.rs`**:
   ```rust
   // After P2P initialization
   let p2p_result = p2p.initialize();

   if p2p_result.is_err() {
       // P2P failed, try relay
       if let Some(relay_url) = std::env::var("LUCIDITY_RELAY_URL").ok() {
           log::info!("P2P failed, connecting to relay: {}", relay_url);
           relay_client.connect().await?;
       }
   }
   ```

3. **Update `lucidity-host/Cargo.toml`**:
   ```toml
   tokio-tungstenite = "0.21"  # Add WebSocket client
   ```

4. **Update pairing payload** to include relay info:
   ```rust
   struct PairingPayload {
       desktop_pubkey: String,
       lan_addr: Option<String>,
       external_addr: Option<String>,
       relay_url: Option<String>,    // NEW
       relay_id: Option<String>,     // NEW
   }
   ```

**Verification**:
```bash
# Start relay
LUCIDITY_RELAY_LISTEN=0.0.0.0:9090 cargo run -p lucidity-relay

# Start desktop with P2P disabled (force relay)
LUCIDITY_RELAY_URL=ws://localhost:9090 \
  LUCIDITY_RELAY_ID=test-desktop \
  cargo run -p wezterm-gui
```

---

### Task 5.3: Mobile Relay Integration

**Goal**: Mobile detects direct connection failure and falls back to relay.

**Files to Modify**:
```
lucidity-mobile/lib/
├── protocol/
│   ├── lucidity_client.dart   # Add relay WebSocket client
│   ├── relay_client.dart      # NEW: Dedicated relay client
│   └── connection_manager.dart # NEW: Connection strategy manager
├── app/
│   └── desktop_profile.dart   # Add relay_url, relay_id fields
```

**Implementation Steps**:

1. **Update `desktop_profile.dart`**:
   ```dart
   class DesktopProfile {
     final String id;
     final String name;
     final String publicKey;
     final String? lanAddr;
     final String? externalAddr;
     final String? relayUrl;       // NEW
     final String? relayId;        // NEW
     // ...
   }
   ```

2. **Create `lib/protocol/relay_client.dart`**:
   ```dart
   class RelayClient {
     final String relayUrl;
     final String relayId;
     WebSocketChannel? _channel;

     Future<void> connect() async {
       final url = '$relayUrl/mobile/$relayId';
       _channel = WebSocketChannel.connect(Uri.parse(url));
     }

     Stream<Uint8List> get dataStream =>
         _channel!.stream.map((data) => data as Uint8List);

     void sendData(Uint8List data) {
       _channel!.sink.add(data);
     }
   }
   ```

3. **Create `lib/protocol/connection_manager.dart`**:
   ```dart
   class ConnectionManager {
     final DesktopProfile profile;

     Future<ConnectionResult> connectWithCascade() async {
       // 1. Try LAN direct
       if (profile.lanAddr != null) {
         try {
           return await _connectDirect(profile.lanAddr!);
         } catch (e) {
           log('LAN connection failed: $e');
         }
       }

       // 2. Try external/UPnP
       if (profile.externalAddr != null) {
         try {
           return await _connectDirect(profile.externalAddr!);
         } catch (e) {
           log('External connection failed: $e');
         }
       }

       // 3. Try relay
       if (profile.relayUrl != null && profile.relayId != null) {
         try {
           return await _connectViaRelay(
             profile.relayUrl!, profile.relayId!);
         } catch (e) {
           log('Relay connection failed: $e');
         }
       }

       throw ConnectionError('All connection methods failed');
     }
   }
   ```

4. **Update `lib/protocol/lucidity_client.dart`**:
   - Replace direct socket code with `ConnectionManager`
   - Handle both direct TCP and WebSocket relay transparently

5. **Update pairing QR parsing** to extract relay info:
   ```dart
   // In pairing_screen.dart or messages.dart
   PairingPayload.fromQr(String data) {
     // Parse relay_url and relay_id from QR payload
   }
   ```

**Verification**:
```bash
# Build and run mobile app
cd lucidity-mobile
flutter run

# Test connection cascade:
# 1. With desktop on same LAN → should connect directly
# 2. With desktop on different network + UPnP → should connect via external
# 3. With desktop behind symmetric NAT → should fall back to relay
```

---

### Task 5.4: Connection Status UI

**Goal**: Show user how they're connected (LAN / Internet / Relay).

**Files to Modify**:
```
lucidity-mobile/lib/
├── screens/desktop_screen.dart   # Add connection indicator
└── protocol/connection_state.dart # Add connection type enum
```

**Implementation Steps**:

1. **Update `connection_state.dart`**:
   ```dart
   enum ConnectionType {
     lan,      // Direct LAN connection
     external, // Direct internet (UPnP/STUN)
     relay,    // Via relay server
   }

   class ConnectionState {
     final bool connected;
     final ConnectionType? type;
     final int? latencyMs;
   }
   ```

2. **Update `desktop_screen.dart`**:
   ```dart
   // In the app bar or status area
   Widget _buildConnectionIndicator() {
     final state = context.watch<ConnectionState>();

     final icon = switch (state.type) {
       ConnectionType.lan => Icons.wifi,
       ConnectionType.external => Icons.public,
       ConnectionType.relay => Icons.cloud,
       null => Icons.cloud_off,
     };

     final label = switch (state.type) {
       ConnectionType.lan => 'LAN',
       ConnectionType.external => 'Direct',
       ConnectionType.relay => 'Relay',
       null => 'Offline',
     };

     return Chip(
       avatar: Icon(icon),
       label: Text('$label ${state.latencyMs}ms'),
     );
   }
   ```

---

### Task 5.5: Device Management UI

**Goal**: Users can list, rename, and remove paired devices.

**Files to Modify/Create**:
```
lucidity-mobile/lib/
├── screens/
│   ├── device_list_screen.dart    # NEW: List of paired desktops
│   └── device_detail_screen.dart  # NEW: Edit/remove single device
├── app/
│   └── desktop_store.dart         # Add rename/remove methods

lucidity-host/src/
├── device_manager.rs              # NEW: CLI device management
```

**Implementation Steps**:

1. **Mobile - Device List Screen**:
   ```dart
   class DeviceListScreen extends StatelessWidget {
     @override
     Widget build(BuildContext context) {
       final devices = context.watch<AppState>().pairedDesktops;

       return ListView.builder(
         itemCount: devices.length,
         itemBuilder: (context, index) {
           final device = devices[index];
           return ListTile(
             title: Text(device.name),
             subtitle: Text(_formatLastSeen(device.lastConnected)),
             trailing: PopupMenuButton(
               itemBuilder: (_) => [
                 PopupMenuItem(value: 'rename', child: Text('Rename')),
                 PopupMenuItem(value: 'remove', child: Text('Remove')),
               ],
               onSelected: (action) => _handleAction(context, device, action),
             ),
           );
         },
       );
     }
   }
   ```

2. **Mobile - Device Detail Screen**:
   - Show device fingerprint
   - Last connection time
   - Rename option
   - Remove button with confirmation

3. **Desktop - Device Manager CLI**:
   ```rust
   // lucidity-host/src/device_manager.rs
   pub fn list_devices(db: &DeviceStore) -> Vec<TrustedDevice> { ... }
   pub fn revoke_device(db: &DeviceStore, fingerprint: &str) -> Result<()> { ... }
   pub fn rename_device(db: &DeviceStore, fingerprint: &str, name: &str) -> Result<()> { ... }
   ```

4. **Desktop - CLI Commands**:
   ```bash
   # Add to lucidity-client or new lucidity-admin binary
   lucidity devices list
   lucidity devices revoke <fingerprint>
   lucidity devices rename <fingerprint> "My iPhone"
   ```

---

### Task 5.6: App Store Preparation

**Goal**: Prepare iOS and Android builds for store submission.

**Files to Create/Modify**:
```
lucidity-mobile/
├── android/
│   ├── app/
│   │   ├── build.gradle           # Signing config
│   │   └── src/main/
│   │       └── AndroidManifest.xml # Permissions
│   └── key.properties             # Signing keys (gitignored)
├── ios/
│   ├── Runner/
│   │   ├── Info.plist             # App metadata
│   │   └── Assets.xcassets/       # App icons
│   └── ExportOptions.plist        # Archive settings
├── assets/
│   ├── icon/                      # Source icons
│   └── splash/                    # Splash screens
└── pubspec.yaml                   # App metadata
```

**Implementation Steps**:

1. **App Icons** (1024x1024 source):
   - Generate adaptive icons for Android
   - Generate iOS icon set
   - Use `flutter_launcher_icons` package

2. **Splash Screens**:
   - Use `flutter_native_splash` package
   - Configure in `pubspec.yaml`

3. **Android Setup**:
   ```groovy
   // android/app/build.gradle
   android {
       signingConfigs {
           release {
               keyAlias keystoreProperties['keyAlias']
               keyPassword keystoreProperties['keyPassword']
               storeFile file(keystoreProperties['storeFile'])
               storePassword keystoreProperties['storePassword']
           }
       }
       buildTypes {
           release {
               signingConfig signingConfigs.release
               minifyEnabled true
               proguardFiles getDefaultProguardFile('proguard-android.txt'), 'proguard-rules.pro'
           }
       }
   }
   ```

4. **iOS Setup**:
   - Create App Store Connect entry
   - Configure provisioning profiles
   - Set bundle identifier: `app.lucidity.mobile`

5. **Privacy Policy** (`docs/legal/privacy-policy.md`):
   - Data collection: None (all local/device-to-device)
   - Network: Direct P2P, optional relay
   - Permissions: Camera (QR scanning), Network

6. **Build Commands**:
   ```bash
   # Android
   flutter build appbundle --release

   # iOS
   flutter build ios --release
   ```

---

## Phase 6: Polish & Reliability

### Task 6.1: Auto-Reconnect

**Goal**: Automatically reconnect when connection drops.

**Implementation**:
- Detect socket close/error
- Exponential backoff retry (1s, 2s, 4s, 8s, max 30s)
- Show "Reconnecting..." UI state
- Preserve pane selection across reconnects

### Task 6.2: Clipboard Sync

**Goal**: Share clipboard between desktop and mobile.

**Protocol Extension**:
```rust
// New frame types
TYPE_CLIPBOARD_PUSH = 5;  // Send clipboard content
TYPE_CLIPBOARD_PULL = 6;  // Request clipboard content
```

**Security**: Only sync clipboard for active/focused pane.

### Task 6.3: Window Resize

**Goal**: Sync terminal size when mobile screen rotates.

**Implementation**:
- Mobile sends resize events when terminal view size changes
- Desktop resizes PTY accordingly
- Protocol: `TYPE_RESIZE = 7; { cols: u16, rows: u16 }`

### Task 6.4: Multiple Tabs

**Goal**: Open/switch tabs from mobile.

**Protocol Extension**:
```rust
TYPE_TAB_NEW = 8;      // Create new tab
TYPE_TAB_CLOSE = 9;    // Close tab
TYPE_TAB_SWITCH = 10;  // Switch to tab by ID
```

---

## Verification Checklist (Full Product)

Before declaring "App Store Ready":

- [ ] **P2P Works**: LAN connection, UPnP, STUN discovery
- [ ] **Relay Works**: Falls back when P2P fails
- [ ] **Pairing Works**: QR scan → approve → connected
- [ ] **Terminal Works**: Input, output, special keys
- [ ] **Profiles Persist**: Can reconnect without re-pairing
- [ ] **UI Polish**: Theme, gestures, keyboard toolbar
- [ ] **Device Management**: List, rename, remove devices
- [ ] **Error Handling**: Graceful failures, reconnect logic
- [ ] **Security**: Ed25519 auth, no data logging on relay
- [ ] **App Store**: Icons, splash, privacy policy, signing

---

## File Index (Quick Reference)

| Component | Key Files |
|-----------|-----------|
| Desktop Host | `lucidity-host/src/server.rs`, `p2p.rs`, `pairing_api.rs` |
| Relay Server | `lucidity-relay/src/main.rs`, `session.rs` |
| Mobile App | `lucidity-mobile/lib/main.dart`, `screens/`, `protocol/` |
| Pairing | `lucidity-pairing/src/pairing.rs`, `keypair.rs`, `qr.rs` |
| Protocol | `lucidity-proto/src/lib.rs` |
| GUI Integration | `wezterm-gui/src/overlay/`, `termwindow/mod.rs` |

---

## Summary

This plan takes Lucidity from "working P2P prototype" to "App Store ready product". The key insight is:

**P2P is PRIMARY** → UPnP/STUN handles most cases
**Relay is FALLBACK** → Only used when P2P fails (symmetric NAT, corporate firewall)

The relay server is SIMPLE - it just routes WebSocket messages between desktop and mobile. All crypto/auth happens at the endpoints.
