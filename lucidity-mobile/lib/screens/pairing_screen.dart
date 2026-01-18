import 'dart:async';
import 'dart:io';

import 'package:device_info_plus/device_info_plus.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../app/app_state.dart';
import '../app/auth_state.dart';
import '../protocol/base64url_nopad.dart';
import '../protocol/lucidity_client.dart';
import '../protocol/messages.dart';
import '../protocol/mobile_identity.dart';

class PairingScreen extends StatefulWidget {
  final PairingPayload payload;
  final String relayBase;

  const PairingScreen({
    super.key,
    required this.payload,
    required this.relayBase,
  });

  @override
  State<PairingScreen> createState() => _PairingScreenState();
}

class _PairingScreenState extends State<PairingScreen> {
  final _emailController = TextEditingController();
  final _deviceNameController = TextEditingController();

  bool _pairing = false;
  String? _status;

  @override
  void initState() {
    super.initState();
    unawaited(_initDeviceName());
  }

  Future<void> _initDeviceName() async {
    final info = DeviceInfoPlugin();
    String name = 'Lucidity Mobile';

    try {
      if (Platform.isAndroid) {
        final d = await info.androidInfo;
        name = '${d.manufacturer} ${d.model}'.trim();
      } else if (Platform.isIOS) {
        final d = await info.iosInfo;
        name = d.name;
      } else if (Platform.isMacOS) {
        final d = await info.macOsInfo;
        name = d.computerName;
      } else if (Platform.isWindows) {
        final d = await info.windowsInfo;
        name = d.computerName;
      }
    } catch (_) {
      // ignore
    }

    if (!mounted) return;
    _deviceNameController.text = name;
  }

  @override
  void dispose() {
    _emailController.dispose();
    _deviceNameController.dispose();
    super.dispose();
  }

  String _fingerprintShort(String b64) {
    if (b64.length <= 16) return b64;
    final prefix = b64.substring(0, 8);
    final suffix = b64.substring(b64.length - 6);
    return '$prefix...$suffix';
  }

  Future<void> _pair() async {
    final email = _emailController.text.trim().isEmpty ? 'unknown' : _emailController.text.trim();
    final deviceName = _deviceNameController.text.trim().isEmpty
        ? 'Lucidity Mobile'
        : _deviceNameController.text.trim();

    setState(() {
      _pairing = true;
      _status = 'Connecting...';
    });

    final client = LucidityClient();
    try {
      await client.connectRelay(
        relayBase: widget.relayBase,
        relayId: widget.payload.relayId,
        authToken: context.read<AuthState>().token,
      );

      setState(() => _status = 'Verifying desktop identity...');

      final hostPayload = await client.pairingPayload();
      if (hostPayload.desktopPublicKey != widget.payload.desktopPublicKey) {
        throw StateError('Connected desktop does not match scanned QR');
      }

      setState(() => _status = 'Creating pairing request...');

      final identity = MobileIdentity();
      final keypair = await identity.loadOrCreate();

      final ts = DateTime.now().millisecondsSinceEpoch ~/ 1000;
      final desktopPub = Base64UrlNoPad.decode(widget.payload.desktopPublicKey);
      final sig = await identity.signDesktopKeyAndTimestamp(
        identity: keypair,
        desktopPublicKey: desktopPub,
        timestampSeconds: ts,
      );

      final req = PairingRequest(
        mobilePublicKey: identity.publicKeyBase64UrlNoPad(keypair),
        signature: Base64UrlNoPad.encode(sig),
        userEmail: email,
        deviceName: deviceName,
        timestamp: ts,
      );

      setState(() => _status = 'Waiting for approval on desktop...');
      final resp = await client.pairingSubmit(req);

      if (!mounted) return;
      if (resp.approved) {
        setState(() => _status = 'Paired!');
        final profile = await context.read<AppState>().upsertFromPairing(
              payload: widget.payload,
              host: '',
              port: 0,
            );
        if (!mounted) return;
        Navigator.of(context).pop(profile);
      } else {
        setState(() => _status = 'Rejected: ${resp.reason ?? 'unknown reason'}');
      }
    } catch (e) {
      if (!mounted) return;
      setState(() => _status = 'Pairing failed: $e');
    } finally {
      client.dispose();
      if (mounted) setState(() => _pairing = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final payload = widget.payload;

    return Scaffold(
      appBar: AppBar(title: const Text('Pair Desktop')),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(
              'Desktop key: ${_fingerprintShort(payload.desktopPublicKey)}',
              style: Theme.of(context).textTheme.bodySmall,
            ),
            const SizedBox(height: 4),
            Text(
              'Code: ${payload.relayId}',
              style: Theme.of(context).textTheme.bodySmall,
            ),
            const SizedBox(height: 16),
            TextField(
              controller: _emailController,
              decoration: const InputDecoration(
                labelText: 'User Email (display only)',
                hintText: 'user@example.com',
              ),
              enabled: !_pairing,
            ),
            const SizedBox(height: 12),
            TextField(
              controller: _deviceNameController,
              decoration: const InputDecoration(
                labelText: 'Device Name',
                hintText: 'My Phone',
              ),
              enabled: !_pairing,
            ),
            const SizedBox(height: 16),
            FilledButton(
              onPressed: _pairing ? null : _pair,
              child: _pairing
                  ? const SizedBox(
                      height: 18,
                      width: 18,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Text('Request Pairing'),
            ),
            const SizedBox(height: 12),
            if (_status != null)
              Text(
                _status!,
                style: Theme.of(context).textTheme.bodySmall,
              ),
          ],
        ),
      ),
    );
  }
}
