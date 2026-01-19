import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../app/app_state.dart';
import '../app/desktop_profile.dart';
import '../app/app_config.dart';
import '../protocol/messages.dart';
import '../protocol/pairing_url.dart';
import 'desktop_screen.dart';
import 'desktop_setup_screen.dart';
import 'pairing_screen.dart';
import 'qr_scan_screen.dart';

class HomeScreen extends StatefulWidget {
  const HomeScreen({super.key});

  @override
  State<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends State<HomeScreen> {
  bool _autoReconnectTriggered = false;

  @override
  void initState() {
    super.initState();
    // Check if we should auto-reconnect after the first frame
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _checkAutoReconnect();
    });
  }

  void _checkAutoReconnect() {
    if (_autoReconnectTriggered) return;
    _autoReconnectTriggered = true;
    
    final state = context.read<AppState>();
    final lastDesktop = state.lastConnectedDesktop;
    
    if (lastDesktop != null) {
      Navigator.of(context).push(
        MaterialPageRoute(
          builder: (_) => DesktopScreen(desktopId: lastDesktop.id),
        ),
      );
    }
  }

  Future<void> _addViaQr(BuildContext context) async {
    final raw = await Navigator.of(context).push<String>(
      MaterialPageRoute(builder: (_) => const QrScanScreen()),
    );
    if (raw == null) return;

    PairingPayload payload;
    try {
      payload = parsePairingUrl(raw);
    } catch (e) {
      if (!context.mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Invalid Lucidity QR: $e')),
      );
      return;
    }

    if (!context.mounted) return;
    final added = await Navigator.of(context).push<DesktopProfile>(
      MaterialPageRoute(
        builder: (_) => PairingScreen(payload: payload),
      ),
    );
    if (added == null || !context.mounted) return;

    Navigator.of(context).push(
      MaterialPageRoute(builder: (_) => DesktopScreen(desktopId: added.id)),
    );
  }

  Future<void> _addManual(BuildContext context) async {
    final added = await Navigator.of(context).push<DesktopProfile>(
      MaterialPageRoute(builder: (_) => const DesktopSetupScreen()),
    );
    if (added == null || !context.mounted) return;
    Navigator.of(context).push(
      MaterialPageRoute(builder: (_) => DesktopScreen(desktopId: added.id)),
    );
  }
  
  void _showSettings(BuildContext context) {
    showModalBottomSheet(
      context: context,
      builder: (ctx) => SafeArea(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            ListTile(
              leading: const Icon(Icons.logout),
              title: const Text('Clear Saved Session'),
              subtitle: const Text('Stop auto-reconnecting on app launch'),
              onTap: () async {
                Navigator.of(ctx).pop();
                await context.read<AppState>().clearLastSession();
                if (context.mounted) {
                  ScaffoldMessenger.of(context).showSnackBar(
                    const SnackBar(content: Text('Saved session cleared')),
                  );
                }
              },
            ),
            Consumer<AppState>(
              builder: (ctx, state, _) => SwitchListTile(
                secondary: const Icon(Icons.sync),
                title: const Text('Auto-Reconnect'),
                subtitle: const Text('Auto-connect to last desktop on launch'),
                value: state.autoReconnect,
                onChanged: (val) {
                  state.autoReconnect = val;
                },
              ),
            ),
            ListTile(
              leading: const Icon(Icons.feedback_outlined),
              title: const Text('Send Feedback'),
              subtitle: const Text('Report bugs or suggest features'),
              onTap: () {
                // TODO: Launch email or feedback form
                Navigator.of(ctx).pop();
                if (context.mounted) {
                  ScaffoldMessenger.of(context).showSnackBar(
                    const SnackBar(content: Text('Please email beta@lucidity.app')),
                  );
                }
              },
            ),
          ],
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Lucidity'),
        actions: [
          IconButton(
            icon: const Icon(Icons.settings),
            tooltip: 'Settings',
            onPressed: () => _showSettings(context),
          ),
        ],
      ),
      body: Consumer<AppState>(
        builder: (context, state, _) {
          if (!state.ready) {
            return const Center(child: CircularProgressIndicator());
          }

          final desktops = state.desktops;
          return Padding(
            padding: const EdgeInsets.all(16),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                if (desktops.isEmpty)
                  const _EmptyState()
                else
                  Expanded(
                    child: ListView.separated(
                      itemCount: desktops.length,
                      separatorBuilder: (_, __) => const Divider(height: 1),
                      itemBuilder: (context, index) {
                        final d = desktops[index];
                        return _DesktopTile(
                          desktop: d,
                          onOpen: () {
                            Navigator.of(context).push(
                              MaterialPageRoute(
                                builder: (_) => DesktopScreen(desktopId: d.id),
                              ),
                            );
                          },
                          onDelete: () async {
                            final ok = await showDialog<bool>(
                              context: context,
                              builder: (_) => AlertDialog(
                                title: const Text('Remove desktop?'),
                                content: Text('Remove "${d.displayName}" from this phone?'),
                                actions: [
                                  TextButton(
                                    onPressed: () => Navigator.of(context).pop(false),
                                    child: const Text('Cancel'),
                                  ),
                                  FilledButton(
                                    onPressed: () => Navigator.of(context).pop(true),
                                    child: const Text('Remove'),
                                  ),
                                ],
                              ),
                            );
                            if (ok != true || !context.mounted) return;
                            await context.read<AppState>().deleteDesktop(d.id);
                          },
                        );
                      },
                    ),
                  ),
                const SizedBox(height: 12),
                Row(
                  children: [
                    Expanded(
                      child: FilledButton.icon(
                        onPressed: () => _addViaQr(context),
                        icon: const Icon(Icons.qr_code_scanner),
                        label: const Text('Scan QR'),
                      ),
                    ),
                    const SizedBox(width: 12),
                    Expanded(
                      child: OutlinedButton.icon(
                        onPressed: () => _addManual(context),
                        icon: const Icon(Icons.add),
                        label: const Text('Add Desktop'),
                      ),
                    ),
                  ],
                ),
              ],
            ),
          );
        },
      ),
    );
  }
}

class _DesktopTile extends StatelessWidget {
  final DesktopProfile desktop;
  final VoidCallback onOpen;
  final VoidCallback onDelete;

  const _DesktopTile({
    required this.desktop,
    required this.onOpen,
    required this.onDelete,
  });

  @override
  Widget build(BuildContext context) {
    final subtitle = '${desktop.host}:${desktop.port}'
        '${desktop.desktopFingerprintShort.isEmpty ? '' : ' • ${desktop.desktopFingerprintShort}'}';
    final subtitleText = desktop.isPaired
        ? 'Online • ${desktop.relayId}'
            '${desktop.desktopFingerprintShort.isEmpty ? '' : ' • ${desktop.desktopFingerprintShort}'}'
        : subtitle;

    return ListTile(
      leading: const Icon(Icons.terminal),
      title: Text(desktop.displayName),
      subtitle: Text(subtitleText),
      onTap: onOpen,
      trailing: IconButton(
        tooltip: 'Remove',
        icon: const Icon(Icons.delete_outline),
        onPressed: onDelete,
      ),
    );
  }
}

class _EmptyState extends StatelessWidget {
  const _EmptyState();

  @override
  Widget build(BuildContext context) {
    return Expanded(
      child: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 420),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              const Icon(Icons.phone_iphone, size: 56),
              const SizedBox(height: 12),
              Text(
                'Pair your desktop',
                style: Theme.of(context).textTheme.headlineSmall,
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 8),
              Text(
                'Open WezTerm on your computer. When the Lucidity QR shows up, scan it here.',
                style: Theme.of(context).textTheme.bodyMedium,
                textAlign: TextAlign.center,
              ),
            ],
          ),
        ),
      ),
    );
  }
}

// Remote connectivity is handled via the Lucidity relay in internet mode.
