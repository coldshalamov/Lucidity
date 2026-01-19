# Comprehensive Testing Strategy

To ensure Lucidity is ready for the App Store, we need to verify every layer of the stack. Since mobile development (iOS/Android) involves complex toolchains, we use a **simulation-first** approach to verify logic on the desktop before deploying to phones.

## 1. Automated Testing Layers

### ✅ Protocol Layer (Unit Tests)
**Goal**: Verify that wire messages are encoded/decoded correctly.
*   **Target**: `lucidity-proto`
*   **Command**: `cargo test -p lucidity-proto`
*   **Content**: Roundtrip tests for every frame type.

### ⚠️ Binding Layer (Integration Tests)
**Goal**: Verify the Host Server correctly interfaces with WezTerm's internal Mux.
*   **Target**: `lucidity-host`
*   **Command**: `cargo test -p lucidity-host`
*   **Content**: 
    *   `pairing_test.rs`: Verifies cryptographic handshake logic.
    *   `tcp_smoke.rs`: Verifies server accepts connections and routes messages using a "Fake Bridge".

## 2. Simulation Strategy (The "Mock Client")

Since you may not always have a phone connected, we use `lucidity-client` as a **Mock Mobile Device**.

> **Why?** If `lucidity-client` can pair, connect, and render output, then the *only* thing left to verify on the real phone is the UI layer (Flutter). The logic is identical.

### Plan: Upgrade `lucidity-client`
We need to upgrade the current CLI client to support:
1.  **Pairing Mode**: Accept a pasted QR code string (or URL) and perform the full Ed25519 exchange.
2.  **Identity Storage**: Save the generated client keypair to a local file (simulating the phone's Keychain).
3.  **LAN Connection**: Connect using the IP/Port from the pairing data.

## 3. Manual Verification (End-to-End)

Once the mock client is ready, here is the full test script:

### Step 1: Start the Host
**Option A: Real WezTerm (Requires stopping existing instances)**
1.  Run WezTerm with the host enabled.
    ```powershell
    $env:LUCIDITY_LISTEN="127.0.0.1:9797"
    cargo run -p wezterm-gui
    ```

**Option B: Standalone Host (Safe for Dev)**
1.  Use the temporary standalone host binary (creates a fake pane).
    ```powershell
    cargo run -p lucidity-host --bin standalone_host
    ```
2.  **Verify**: Log output shows `Standalone Lucidity Host listening on 127.0.0.1:9798` and prints a Pairing URL.

### Step 2: Simulate "Scanning"
1.  Copy the `lucidity://` URL from the host output.
2.  Run the client in **pairing mode**:
    ```powershell
    cargo run -p lucidity-client -- pair "lucidity://..."
    ```
3.  **Action**:
    *   *Real Host*: Click "Approve" in the GUI.
    *   *Standalone Host*: Auto-approves immediately.
4.  **Result**: Client says "✅ Pairing APPROVED!" and saves credentials.

### Step 3: Connect & Control
1.  Run the client in **connect mode**:
    ```powershell
    cargo run -p lucidity-client -- connect --identity mock_device.json
    ```
2.  **Result**: The client terminal should mirror the host terminal. Type `ls` in the client; it appears on the host.

## 4. Mobile Verification (Real Device)

Once the above passes, the final step is:
1.  Build `lucidity-mobile` (Flutter).
2.  Run on physical iPhone/Android.
3.  Scan the *actual* QR code on screen.
4.  Verify the same flow.
