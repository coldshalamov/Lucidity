# Lucidity FAQ

Frequently asked questions about Lucidity.

---

## General

### What is Lucidity?
Lucidity is a mobile app that lets you control your desktop terminal (WezTerm) from your phone. You can view terminal output and type commands remotely.

### Is Lucidity free?
Yes, Lucidity is free and open source. The app and all components are available on GitHub.

### Do I need an account to use Lucidity?
No. Lucidity uses device-based pairing with QR codes. No sign-up, login, or account required.

### What platforms are supported?
- **Mobile**: iOS 14+ and Android 8+
- **Desktop**: Windows, macOS, and Linux (anywhere WezTerm runs)

---

## Setup & Pairing

### How do I pair my phone with my desktop?
1. Run WezTerm on your desktop.
2. Press **Ctrl+Shift+L** to show the QR code.
3. Open Lucidity on your phone and tap **Scan QR**.
4. Approve the connection on your desktop.

### Can I pair multiple phones to one desktop?
Yes. Each phone you pair will be added to the desktop's trusted devices list.

### Can I pair one phone to multiple desktops?
Yes. Each paired desktop appears as a separate entry in your phone's app.

### How do I remove a paired device?
On mobile: Swipe left on the desktop and tap Delete.
On desktop: Use the CLI command `lucidity-host devices --revoke <device_id>` (or delete from `devices.db`).

---

## Using the App

### Why is there a delay in the terminal output?
Small delays are normal, especially over the internet. For best performance:
- Use the same Wi-Fi network (LAN mode is fastest, ~1ms).
- UPnP/STUN direct connections are also fast (~50ms).
- If P2P fails and relay is used, expect slightly higher latency (~100ms+).

### How do I send special keys like Ctrl, Escape, or arrows?
Use the accessory bar above the keyboard. It includes buttons for:
- Esc
- Tab
- Ctrl+C
- Arrow keys (↑ ↓ ← →)

### Can I copy/paste in the terminal?
Currently, clipboard sync is not implemented. It's planned for a future release.

### Why can't I see all my terminal panes?
Make sure you're connected to the correct desktop. Tap the menu to refresh or switch panes.

---

## Security & Privacy

### Is my terminal data stored on your servers?
**No.** Terminal data is transmitted directly between your devices using P2P connections (LAN, UPnP, or STUN). If P2P fails, the fallback relay server only routes encrypted traffic and does not log or store session content.

### How is the connection secured?
- **TLS**: All connections use TLS (HTTPS/WSS) encryption.
- **Ed25519 Authentication**: Devices mutually authenticate using cryptographic signatures.
- **Pairing Approval**: You must approve each new device on your desktop.

### What data do you collect?
We collect minimal anonymous usage statistics (e.g., number of sessions) to improve the app. We never collect terminal content, keystrokes, or personal data.

### Can someone else access my terminal?
Only devices you explicitly approve can connect. Each device has a unique cryptographic key stored securely on your phone.

---

## Troubleshooting

### "Desktop Not Found"
- Ensure WezTerm is running.
- For LAN: Check that both devices are on the same network.
- For Internet: Check that UPnP is enabled on your router, or relay is configured.
- Try regenerating the QR code.

### "Connection Timed Out"
- Check your internet connection.
- The desktop may be behind a firewall.
- Try reconnecting.

### The keyboard doesn't appear
- Tap directly on the terminal area.
- Try rotating your phone.
- Restart the app.

### I lost my connection and can't reconnect
- Go back to the home screen and try connecting again.
- If auto-reconnect is enabled, the app will retry automatically.
- Check if the desktop is still running.

For more help, see the [Troubleshooting Guide](troubleshooting.md).

---

## Future Features

### Will you add clipboard sync?
Yes, clipboard sharing is planned for Phase 4.

### Will you add file transfer?
Yes, file upload/download is on the roadmap.

### Can I use this with terminals other than WezTerm?
Currently, Lucidity only works with WezTerm. Support for other terminals may be considered in the future.

---

## Contact

- **GitHub**: [github.com/lucidity-app/lucidity](https://github.com/lucidity-app/lucidity)
- **Email**: beta@lucidity.app
- **Security Issues**: security@lucidity.app
