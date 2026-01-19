# Lucidity User Guide

Welcome to Lucidity! This guide will help you get started with remote terminal access from your phone.

---

## Table of Contents

1. [Getting Started](#getting-started)
2. [Pairing Your Phone](#pairing-your-phone)
3. [Using the Terminal](#using-the-terminal)
4. [Settings](#settings)
5. [Troubleshooting](#troubleshooting)
6. [FAQ](#faq)

---

## Getting Started

### What You Need

1. **Desktop**: A computer running WezTerm with Lucidity features enabled.
2. **Mobile**: An iPhone (iOS 14+) or Android phone (8+) with the Lucidity app installed.
3. **Network**: Both devices on the same Wi-Fi network (for local mode) or internet access (for relay mode).

### Installing WezTerm

1. Download WezTerm from [wezfurlong.org/wezterm](https://wezfurlong.org/wezterm).
2. Install and run it.
3. The Lucidity host starts automatically in the background.

---

## Pairing Your Phone

Pairing creates a secure link between your phone and desktop.

### Step 1: Show the QR Code

On your desktop, press **Ctrl+Shift+L** (or use the menu: `View > Show Lucidity QR`).

A QR code will appear on screen.

### Step 2: Scan with Your Phone

1. Open the Lucidity app on your phone.
2. Tap **Scan QR**.
3. Point your camera at the QR code.

### Step 3: Approve on Desktop

A prompt will appear on your desktop asking to approve the new device.

- Click **Approve** to allow access.
- Click **Deny** to reject.

### Step 4: Connected!

Once approved, your phone will show a list of open terminal panes. Tap one to connect.

---

## Using the Terminal

### Viewing Output

The terminal screen displays the same output as your desktop terminal in real-time.

### Typing

Tap anywhere on the terminal to bring up the keyboard. Everything you type is sent to the desktop.

### Special Keys

Use the accessory bar above the keyboard for:

| Button | Function |
|--------|----------|
| **Esc** | Escape key |
| **Tab** | Tab key |
| **Ctrl+C** | Interrupt (Ctrl+C) |
| **↑ ↓ ← →** | Arrow keys |

### Switching Panes

If you have multiple terminal panes open on your desktop:

1. Tap the **≡** menu button.
2. Select a different pane from the list.

---

## Settings

Access settings by tapping the **⚙️** icon on the home screen.

### Auto-Reconnect

When enabled, the app will automatically try to reconnect to your last desktop when opened.

### Clear Saved Session

Removes the saved connection. The app won't auto-connect on next launch.

### Send Feedback

Opens options to report bugs or suggest features.

---

## Troubleshooting

### "Desktop Not Found"

- Make sure WezTerm is running on your desktop.
- Check that both devices are on the same network (for local mode).
- Verify the relay is running (for internet mode).

### "Connection Timed Out"

- Check your internet connection.
- Try moving closer to your Wi-Fi router.
- The desktop may be behind a firewall blocking connections.

### "Pairing Request Not Appearing"

- Regenerate the QR code on the desktop (Ctrl+Shift+L).
- Ensure the Lucidity host is running (check `netstat -an | findstr 9797`).

### Keyboard Not Appearing

- Tap directly on the terminal area.
- Try rotating your phone.

For more help, see [Troubleshooting Guide](troubleshooting.md).

---

## FAQ

### Is my terminal data stored anywhere?

**No.** Terminal data is transmitted directly between your devices via an encrypted connection. Our relay server only routes traffic; it does not store or log session content.

### Do I need an account?

**No.** Lucidity uses device-based pairing. No sign-up required.

### Can I control multiple desktops?

**Yes.** Each desktop you pair will appear in a list on the home screen.

### Does it work over cellular data?

**Yes**, if your desktop is connected to the relay. The relay brokers connections over the internet.

### Is it open source?

**Yes.** Lucidity is open source. Visit our [GitHub repository](https://github.com/lucidity-app/lucidity).

---

## Need Help?

- **Documentation**: [docs/lucidity/](../lucidity/)
- **GitHub Issues**: Report bugs at [github.com/lucidity-app/lucidity/issues](https://github.com/lucidity-app/lucidity/issues)
- **Email**: beta@lucidity.app
