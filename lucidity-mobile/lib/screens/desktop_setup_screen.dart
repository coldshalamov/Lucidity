import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../app/app_state.dart';
import '../app/desktop_profile.dart';

class DesktopSetupScreen extends StatefulWidget {
  final DesktopProfile? existing;

  const DesktopSetupScreen({super.key, this.existing});

  @override
  State<DesktopSetupScreen> createState() => _DesktopSetupScreenState();
}

class _DesktopSetupScreenState extends State<DesktopSetupScreen> {
  final _nameController = TextEditingController();
  final _hostController = TextEditingController();
  final _portController = TextEditingController();

  bool _busy = false;

  @override
  void initState() {
    super.initState();
    final e = widget.existing;
    if (e != null) {
      _nameController.text = e.displayName;
      _hostController.text = e.host;
      _portController.text = e.port.toString();
    } else {
      _nameController.text = 'WezTerm Desktop';
      _hostController.text = '127.0.0.1';
      _portController.text = '9797';
    }
  }

  @override
  void dispose() {
    _nameController.dispose();
    _hostController.dispose();
    _portController.dispose();
    super.dispose();
  }

  Future<void> _save() async {
    final name = _nameController.text.trim();
    final host = _hostController.text.trim();
    final port = int.tryParse(_portController.text.trim());
    if (name.isEmpty || host.isEmpty || port == null) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Enter a valid name, host, and port')),
      );
      return;
    }

    setState(() => _busy = true);
    try {
      final state = context.read<AppState>();
      DesktopProfile saved;
      final existing = widget.existing;
      if (existing == null) {
        saved = await state.addManualDesktop(displayName: name, host: host, port: port);
      } else {
        saved = existing.copyWith(displayName: name, host: host, port: port);
        await state.updateDesktop(saved);
      }

      if (!mounted) return;
      Navigator.of(context).pop(saved);
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final isEdit = widget.existing != null;
    return Scaffold(
      appBar: AppBar(
        title: Text(isEdit ? 'Edit Desktop' : 'Add Desktop'),
      ),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            TextField(
              controller: _nameController,
              enabled: !_busy,
              decoration: const InputDecoration(labelText: 'Name'),
              textInputAction: TextInputAction.next,
            ),
            const SizedBox(height: 12),
            TextField(
              controller: _hostController,
              enabled: !_busy,
              decoration: const InputDecoration(
                labelText: 'Host',
                hintText: 'LAN IP, public IP, or VPN hostname (Tailscale)',
              ),
              textInputAction: TextInputAction.next,
            ),
            const SizedBox(height: 12),
            TextField(
              controller: _portController,
              enabled: !_busy,
              decoration: const InputDecoration(labelText: 'Port'),
              keyboardType: TextInputType.number,
              textInputAction: TextInputAction.done,
              onSubmitted: (_) => _save(),
            ),
            const SizedBox(height: 16),
            FilledButton(
              onPressed: _busy ? null : _save,
              child: _busy
                  ? const SizedBox(
                      height: 18,
                      width: 18,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Text('Save'),
            ),
            const SizedBox(height: 12),
            Text(
              'Tip: For “works anywhere”, set Host to your desktop’s Tailscale IP or MagicDNS name.',
              style: Theme.of(context).textTheme.bodySmall,
            ),
          ],
        ),
      ),
    );
  }
}

