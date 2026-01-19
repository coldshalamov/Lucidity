# Lucidity Troubleshooting Guide

This guide helps diagnose and resolve common issues with the Lucidity system.

---

## Desktop Issues

### WezTerm won't start

**Symptoms**: Application crashes or fails to launch

**Solutions**:
1. Check for conflicting processes:
   ```sh
   # Windows
   tasklist | findstr wezterm
   
   # Kill if stuck
   taskkill /f /im wezterm-gui.exe
   ```

2. Check logs:
   ```sh
   # Set verbose logging
   set WEZTERM_LOG=debug
   wezterm-gui
   ```

3. Verify Rust/dependencies are up to date:
   ```sh
   rustup update
   cargo clean
   cargo build -p wezterm-gui
   ```

### QR code not showing

**Symptoms**: Pressing Ctrl+Shift+L does nothing

**Solutions**:
1. Check if splash is disabled:
   ```sh
   # Remove this env var if set
   unset LUCIDITY_DISABLE_SPLASH
   ```

2. Verify keybinding works:
   - Open command palette (Ctrl+Shift+P)
   - Search for "Lucidity" or "QR"

3. Check host is running:
   ```sh
   netstat -an | findstr 9797
   ```

### Relay agent not connecting

**Symptoms**: Agent logs show connection failures

**Solutions**:
1. Verify relay URL is correct:
   ```sh
   echo %LUCIDITY_RELAY_URL%
   # Should be: wss://your-relay.example.com
   ```

2. Check relay is reachable:
   ```sh
   curl https://your-relay.example.com/healthz
   ```

3. Verify secret is set:
   ```sh
   echo %LUCIDITY_RELAY_DESKTOP_SECRET%
   ```

4. Check firewall isn't blocking outbound WebSocket

---

## Relay Server Issues

### Relay won't start

**Symptoms**: Server fails to bind or crashes

**Solutions**:
1. Check port availability:
   ```sh
   netstat -an | findstr 9090
   ```

2. Verify environment:
   ```sh
   echo $LUCIDITY_RELAY_LISTEN
   # Should be: 0.0.0.0:9090
   ```

3. Check Docker logs (if containerized):
   ```sh
   docker logs lucidity-relay
   ```

### Desktops can't register

**Symptoms**: 401 or connection refused errors

**Solutions**:
1. Check auth mode:
   ```sh
   # For development only:
   export LUCIDITY_RELAY_NO_AUTH=true
   ```

2. Verify TLS configuration:
   ```sh
   # If behind reverse proxy:
   export LUCIDITY_RELAY_REQUIRE_TLS=false
   ```

3. Check desktop auth token is valid

### Sessions not connecting

**Symptoms**: Mobile gets "desktop offline" error

**Solutions**:
1. Verify desktop is registered:
   - Check relay logs for "Desktop registered: {relay_id}"

2. Ensure relay_id matches:
   - Desktop and mobile must use same relay_id

3. Check for session timeout:
   - Sessions expire after 60s if not accepted

---

## Mobile App Issues

### App crashes on launch

**Symptoms**: Immediate crash or white screen

**Solutions**:
1. Clear app data:
   - Settings → Apps → Lucidity → Clear Data

2. Reinstall app

3. Check device compatibility:
   - Requires iOS 14+ or Android 8+

### QR scanning not working

**Symptoms**: Camera opens but doesn't detect codes

**Solutions**:
1. Grant camera permission:
   - Settings → Apps → Lucidity → Permissions → Camera

2. Ensure good lighting

3. Try different QR code size/distance

4. Restart camera:
   - Close app completely and reopen

### Can't connect to desktop

**Symptoms**: "Connection Error" or timeout

**Solutions**:
1. **Check network**:
   - Ensure phone is on same network as desktop (for local)
   - Ensure internet connection (for relay)

2. **Verify desktop is online**:
   - Check if host is running on desktop
   - Check if relay agent is connected

3. **Check relay URL**:
   - Mobile and desktop must use same relay

4. **Re-pair**:
   - Delete desktop from mobile app
   - Scan QR code again

### Input not working

**Symptoms**: Keyboard appears but nothing happens

**Solutions**:
1. Check pane is attached:
   - Select a pane from the picker first

2. Try accessory bar keys:
   - Esc, Tab, Ctrl+C buttons

3. Reconnect:
   - Pull down to refresh
   - Or go back and reconnect

### Terminal rendering issues

**Symptoms**: Garbled text, missing characters, wrong colors

**Solutions**:
1. Check terminal encoding:
   - Ensure UTF-8 on desktop

2. Force redraw:
   - Resize window on desktop

3. Reconnect to refresh state

---

## Connection Issues

### Timeout errors

**Symptoms**: "Connection timed out" messages

**Causes & Solutions**:
1. **Network latency**: Use relay closer to your location
2. **Firewall blocking**: Check outbound port 443 (WSS)
3. **VPN interference**: Try without VPN
4. **Server overloaded**: Check relay health

### "Connection refused" errors

**Symptoms**: Immediate connection failure

**Causes & Solutions**:
1. **Wrong address**: Verify host/port
2. **Service not running**: Start lucidity-host
3. **Firewall**: Allow inbound connections

### Intermittent disconnections

**Symptoms**: Connection drops randomly

**Causes & Solutions**:
1. **Network instability**: Check Wi-Fi signal
2. **Server restarts**: Check relay uptime
3. **Idle timeout**: Keep session active
4. **Mobile sleep**: Disable battery optimization for app

---

## Security Issues

### "Unauthorized" errors (401)

**Symptoms**: Auth rejected by relay

**Solutions**:
1. Re-pair device with desktop
2. Check system clocks are synchronized
3. Verify relay secret is correct

### "Forbidden" errors (403)

**Symptoms**: Access denied

**Solutions**:
1. Check fingerprint is trusted
2. Re-approve device on desktop
3. Clear and re-pair

### Pairing not working

**Symptoms**: Request never appears on desktop

**Solutions**:
1. Ensure QR code is current (regenerate)
2. Check desktop is online
3. Verify relay connectivity

---

## Logs and Diagnostics

### Enable verbose logging

**Desktop**:
```sh
set WEZTERM_LOG=debug
set RUST_LOG=lucidity=debug,warp=info
```

**Relay**:
```sh
export RUST_LOG=lucidity_relay=debug
```

**Mobile**:
- Settings → Developer → Enable Debug Logs

### Collect diagnostic info

For bug reports, include:
1. OS and version
2. Lucidity version (`wezterm --version`)
3. Relay version
4. Mobile app version
5. Error messages
6. Steps to reproduce

---

## Getting Help

1. **Check documentation**: `docs/lucidity/`
2. **Search existing issues**: GitHub Issues
3. **Ask the community**: GitHub Discussions
4. **Report bugs**: Include diagnostic info above
