# Privacy Policy for Lucidity

**Last Updated:** January 19, 2026

## 1. Introduction
Lucidity ("we", "our", or "us") respects your privacy. This policy explains how we handle your data when you use the Lucidity mobile application and desktop agent.

## 2. Data Collection
Lucidity is designed as a **data-sovereign, local-first** remote control tool. 
- **We do not collect personal data.**
- **We do not track your usage.**
- **We do not have access to your terminal sessions or commands.**

## 3. How It Works
- **Direct Connection**: Lucidity connects your mobile device directly to your computer over your local network (LAN) or via peer-to-peer (P2P) technologies (UPnP/STUN).
- **End-to-End Encryption**: All traffic between your devices is encrypted using modern cryptographic standards (Ed25519/X25519). 
- **Relay Server**: If a direct connection cannot be established, an encrypted relay server may be used to carry sealed data packets. The relay server **cannot** decrypt your data; it only forwards encrypted bytes.

## 4. Third-Party Services
- **Google Play Services / Apple App Store**: The app is distributed via these platforms, which may collect basic download/usage statistics subject to their own privacy policies.
- **STUN Servers**: We may use public STUN servers (e.g., Google's) to discover your public IP address for P2P connectivity. Only your IP address and port are visible to these servers; no session data is shared.

## 5. Permissions
The app requests the following permissions for functional purposes only:
- **Camera**: To scan QR codes for device pairing.
- **Local Network**: To discover and connect to your desktop agent on the same Wi-Fi.
- **Notification**: To keep connections alive in the background (Android sticky notification).

## 6. Contact Us
If you have any questions about this Privacy Policy, please contact us at privacy@lucidity.app.
