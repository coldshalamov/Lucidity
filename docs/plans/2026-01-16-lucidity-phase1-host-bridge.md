# Lucidity Phase 1 (Desktop Host Bridge) Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a buildable “Phase 1” proof that a remote client can attach to an existing Lucidity/WezTerm pane and stream PTY bytes + inject input bytes in real time.

**Architecture:** Implement a small embedded host server (`lucidity-host`) that exposes a local TCP API (length-prefixed frames). The server uses a `PaneBridge` trait; a fake impl enables unit/integration tests without running the full GUI. A real impl uses existing WezTerm mux/pane APIs to list panes, subscribe to output, and send input.

**Tech Stack:** Rust (workspace crates), blocking `TcpListener`, serde/JSON for control messages, binary frames for output/input payloads.


---

## Scope (Phase 1 only)

In-scope:
- Desktop-side host server (Rust) that can:
  - list panes/sessions
  - attach to a pane
  - stream raw output bytes
  - accept input bytes and write to the same PTY
- Minimal CLI client for manual testing (connect, attach, forward stdin→input, print output).
- Protocol framing + basic fuzz/invalid-frame tests.
- Documentation updates for “what exists now” + roadmap for mobile/cloud.

Out-of-scope (document only):
- Mobile apps (iOS/Android)
- Cloud relay, auth, subscriptions, quotas
- Crypto handshake / E2E encryption (design docs only for v0.1)

---

### Task 1: Repo survey (find existing mux/pane hooks)

**Files:**
- Read: `Cargo.toml`
- Read: `wezterm-client/`
- Read: `mux/`
- Read: `wezterm-mux-server*/`
- Read: `wezterm-gui/` (only to locate “active pane” APIs; do not change yet)

**Step 1: Locate existing CLI → mux RPC APIs**
- Command: `rg -n \"list.*pane|pane.*list|write_to_pane|send_paste|subscribe\" wezterm-client mux wezterm-mux-server*`
- Goal: Identify functions/structs for:
  - enumerating panes
  - writing bytes to a pane
  - subscribing/streaming pane output

**Step 2: Capture a short “API map”**
- Write notes into `docs/lucidity/phase1-api-map.md` (create if missing).

---

### Task 2: Define a minimal Lucidity wire protocol crate

**Files:**
- Create: `lucidity-proto/Cargo.toml`
- Create: `lucidity-proto/src/lib.rs`
- Create: `lucidity-proto/src/frame.rs`
- Test: `lucidity-proto/tests/frame_roundtrip.rs`
- Modify: `Cargo.toml` (workspace members + deps)

**Step 1: RED — write failing tests for frame encode/decode**
- Test cases:
  - roundtrip encode→decode for `type + payload`
  - reject short buffer (<5 bytes)
  - reject length > max (e.g. 16 MiB)
  - reject declared length mismatch

**Step 2: GREEN — implement `Frame` + `decode_frames`**
- Format: `len(u32 LE) + type(u8) + payload[len-1]`

**Step 3: REFACTOR**
- Keep API minimal; no crypto yet.

---

### Task 3: Implement host server crate with a tested `PaneBridge` trait

**Files:**
- Create: `lucidity-host/Cargo.toml`
- Create: `lucidity-host/src/main.rs`
- Create: `lucidity-host/src/lib.rs`
- Create: `lucidity-host/src/server.rs`
- Create: `lucidity-host/src/bridge.rs`
- Test: `lucidity-host/tests/tcp_smoke.rs`
- Modify: `Cargo.toml` (workspace members)

**Step 1: Define control message schema (JSON)**
- `ListPanesRequest` → `ListPanesResponse { panes: [{ pane_id, title }] }`
- `AttachRequest { pane_id }` → `AttachOk`

**Step 2: RED — write a TCP server test using `FakePaneBridge`**
- Start server on ephemeral port.
- Connect a TCP client.
- Send `ListPanesRequest`, expect `ListPanesResponse`.
- Send `AttachRequest`, expect `AttachOk`.
- After attach, fake bridge emits output bytes; expect binary frame `TYPE_PANE_OUTPUT`.


**Step 3: GREEN — implement server routing + streaming**
- One TCP connection = one attached pane for now.

- Use binary frames for output/input payloads:
  - `TYPE_PANE_OUTPUT` (from host to client)
  - `TYPE_PANE_INPUT` (from client to host)


**Step 4: Add basic backpressure**
- If client can’t keep up, drop or disconnect (document behavior).

---

### Task 4: Real WezTerm integration bridge (best-effort)

**Files:**
- Modify: `lucidity-host/src/bridge.rs`
- Potentially modify: `wezterm-client/` (only if required; prefer using public APIs)

**Step 1: Implement `WeztermPaneBridge`**
- `list_panes()` should use existing mux client APIs.
- `send_input(pane_id, bytes)` should route to pane PTY (not to GUI rendering).
- `subscribe_output(pane_id)` should deliver raw PTY bytes if available; if not possible, document and implement a fallback (poll) behind a feature flag.

**Step 2: Integration sanity check**
- Run:
  - open WezTerm/Lucidity GUI (embedded host auto-starts)
  - `cargo run -p lucidity-client -- --addr 127.0.0.1:9797 --pane-id <id>`


---

### Task 5: Minimal CLI client for manual proof

**Files:**
- Create: `lucidity-client/Cargo.toml`
- Create: `lucidity-client/src/main.rs`
- Modify: `Cargo.toml` (workspace members)

**Behavior:**
- Connect to TCP host, list panes (optional), attach, then:
  - stdin → `TYPE_PANE_INPUT` frames
  - `TYPE_PANE_OUTPUT` frames → stdout


---

### Task 6: Docs sync (Lucidity overview + Phase 1 usage)

**Files:**
- Modify: `README.md`
- Create: `docs/lucidity/index.md`
- Create: `docs/lucidity/phase1.md`
- Create: `docs/lucidity/protocol.md`
- Create: `docs/lucidity/security.md`
- Modify: `docs/index.md` (link to Lucidity section)

**Content requirements:**
- Make it explicit what is implemented *today* vs “planned”.
- Include copy/paste commands to build + run `lucidity-host` and `lucidity-client`.
- Include a roadmap mapping to your Phase 1–5.

---

### Task 7: Verification

**Step 1: Unit/integration tests**
- Run: `cargo test -p lucidity-proto`
- Run: `cargo test -p lucidity-host`

**Step 2: Workspace build sanity**
- Run: `cargo build -p lucidity-host -p lucidity-client`

**Step 3: Manual smoke**
- Run embedded host + client, attach to a live pane, confirm echo/typing works.


