# Lucidity Phase 1 - Recommended Improvements

## Priority 1: Security Warning (5 minutes)

**File:** `lucidity-host/src/server.rs`  
**Location:** In `autostart_in_process()` function, after parsing listen address

```rust
let listen = std::env::var("LUCIDITY_LISTEN")
    .ok()
    .and_then(|s| s.parse::&lt;SocketAddr&gt;().ok())
    .unwrap_or_else(|| HostConfig::default().listen);

// Add this warning:
if listen.ip().is_unspecified() {
    log::warn!(
        "⚠️  Lucidity host listening on {} - anyone on your LAN can inject keystrokes! \
         Set LUCIDITY_LISTEN=127.0.0.1:9797 for localhost-only.",
        listen
    );
}
```

**Why:** Users should be explicitly warned when they expose the server to LAN.

---

## Priority 2: Update Plan Document (2 minutes)

**File:** `docs/plans/2026-01-16-lucidity-phase1-host-bridge.md`  
**Changes:**

Line 7: Change "WebSocket API" → "TCP API"  
Line 9: Change "WebSocket server" → "TCP server"  
Line 92: Change "WebSocket server test" → "TCP server test"

**Why:** Implementation uses plain TCP, not WebSocket. This is actually a good simplification.

---

## Priority 3: Connection Limit (10 minutes)

**File:** `lucidity-host/src/server.rs`  
**Location:** In `serve_blocking()` function

```rust
const MAX_CONNECTIONS: usize = 10;
let active_connections = Arc::new(AtomicUsize::new(0));

pub fn serve_blocking(listener: TcpListener, bridge: Arc&lt;dyn PaneBridge&gt;) -&gt; anyhow::Result&lt;()&gt; {
    for conn in listener.incoming() {
        let stream = match conn {
            Ok(s) =&gt; s,
            Err(err) =&gt; {
                log::warn!("lucidity-host accept failed: {err:#}");
                continue;
            }
        };
        
        // Add connection limit check:
        let count = active_connections.fetch_add(1, Ordering::Relaxed);
        if count &gt;= MAX_CONNECTIONS {
            log::warn!("lucidity-host: connection limit reached ({MAX_CONNECTIONS}), rejecting");
            active_connections.fetch_sub(1, Ordering::Relaxed);
            continue;
        }
        
        let bridge = Arc::clone(&amp;bridge);
        let conn_counter = Arc::clone(&amp;active_connections);
        thread::spawn(move || {
            if let Err(err) = handle_client(stream, bridge) {
                log::debug!("lucidity-host client ended: {err:#}");
            }
            conn_counter.fetch_sub(1, Ordering::Relaxed);
        });
    }
    Ok(())
}
```

**Why:** Prevents resource exhaustion from too many concurrent connections.

---

## Priority 4: Better Logging (5 minutes)

**File:** `lucidity-host/src/server.rs`  
**Location:** In `handle_client()` function

```rust
fn handle_client(stream: TcpStream, bridge: Arc&lt;dyn PaneBridge&gt;) -&gt; anyhow::Result&lt;()&gt; {
    let peer = stream.peer_addr().ok();
    log::info!("lucidity-host: client connected from {:?}", peer);
    
    // ... existing code ...
    
    // At the end, before Ok(()):
    log::info!("lucidity-host: client disconnected from {:?}", peer);
    Ok(())
}
```

**Why:** Helps with debugging and monitoring active connections.

---

## Optional: Metrics (Phase 1.5)

**File:** `lucidity-host/src/server.rs`

Add metrics for:
- Active connection count
- Total bytes sent/received
- Dropped frame count (when subscriber can't keep up)
- Attach/detach events

**Implementation:** Use existing `metrics` crate (already in workspace dependencies).

---

## Testing Recommendations

After implementing improvements, run:

```powershell
# Build
cargo build -p lucidity-proto -p lucidity-host -p lucidity-client

# Unit tests
cargo test -p lucidity-proto -p lucidity-host

# Integration test
# Terminal 1:
cargo run -p wezterm-gui

# Terminal 2:
cargo run -p lucidity-client -- --addr 127.0.0.1:9797

# Test connection limit (Terminal 3-13):
# Run 11 clients simultaneously to verify limit
for ($i=1; $i -le 11; $i++) {
    Start-Process powershell -ArgumentList "-Command", "cargo run -p lucidity-client -- --addr 127.0.0.1:9797"
}
```

---

## Summary

**Total effort:** ~25 minutes  
**Impact:** Significantly improved security, reliability, and observability

All improvements are backward-compatible and non-breaking.
