import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:google_fonts/google_fonts.dart';
import 'package:provider/provider.dart';
import 'package:xterm/xterm.dart';

import '../app/app_state.dart';
import '../app/app_config.dart';
import '../app/desktop_profile.dart';
import '../app/auth_state.dart';
import '../protocol/lucidity_client.dart';
import '../protocol/messages.dart';
import 'desktop_setup_screen.dart';
import '../protocol/connection_state.dart';
import '../services/connectivity_service.dart';
import '../protocol/error_handler.dart';
import '../app/exponential_backoff.dart';

class DesktopScreen extends StatefulWidget {
  final String desktopId;

  const DesktopScreen({super.key, required this.desktopId});

  @override
  State<DesktopScreen> createState() => _DesktopScreenState();
}

class _DesktopScreenState extends State<DesktopScreen> {
  final List<_TabSpec> _tabs = <_TabSpec>[];
  int _active = 0;

  @override
  void initState() {
    super.initState();
    _tabs.add(_TabSpec.initial());
  }

  void _newTab() {
    setState(() {
      _tabs.add(_TabSpec.initial());
      _active = _tabs.length - 1;
    });
  }

  void _closeTab(int index) {
    setState(() {
      _tabs.removeAt(index);
      if (_tabs.isEmpty) {
        _tabs.add(_TabSpec.initial());
        _active = 0;
      } else {
        _active = _active.clamp(0, _tabs.length - 1).toInt();
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final state = context.watch<AppState>();
    final desktop = state.desktopById(widget.desktopId);
    if (desktop == null) {
      return Scaffold(
        appBar: AppBar(title: const Text('Desktop')),
        body: const Center(child: Text('Desktop not found.')),
      );
    }

    return Scaffold(
      appBar: AppBar(
        title: Text(desktop.displayName),
        actions: [
          IconButton(
            tooltip: 'Edit connection',
            icon: const Icon(Icons.settings),
            onPressed: () async {
              final updated = await Navigator.of(context).push<DesktopProfile>(
                MaterialPageRoute(
                  builder: (_) => DesktopSetupScreen(existing: desktop),
                ),
              );
              if (updated == null || !context.mounted) return;
              await context.read<AppState>().updateDesktop(updated);
            },
          ),
          IconButton(
            tooltip: 'New tab',
            icon: const Icon(Icons.add),
            onPressed: _newTab,
          ),
        ],
      ),
      body: GestureDetector(
        onHorizontalDragEnd: (details) {
          if (details.primaryVelocity == null) return;
          if (details.primaryVelocity! > 500) {
            // Swipe Right -> Previous Tab
            if (_active > 0) {
              setState(() => _active--);
            }
          } else if (details.primaryVelocity! < -500) {
            // Swipe Left -> Next Tab
            if (_active < _tabs.length - 1) {
              setState(() => _active++);
            }
          }
        },
        child: Column(
          children: [
            _TabStrip(
              tabs: _tabs,
              active: _active,
              onSelect: (i) => setState(() => _active = i),
              onClose: _closeTab,
              onNew: _newTab,
            ),
            Expanded(
              child: _TerminalTabView(
                key: ValueKey(_tabs[_active].id),
                desktop: desktop,
                onTitleChanged: (t) {
                  setState(() => _tabs[_active] = _tabs[_active].copyWith(title: t));
                },
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _TabSpec {
  final String id;
  final String title;

  const _TabSpec({required this.id, required this.title});

  factory _TabSpec.initial() => _TabSpec(
        id: DateTime.now().microsecondsSinceEpoch.toString(),
        title: 'New',
      );

  _TabSpec copyWith({String? id, String? title}) => _TabSpec(
        id: id ?? this.id,
        title: title ?? this.title,
      );
}

class _TabStrip extends StatelessWidget {
  final List<_TabSpec> tabs;
  final int active;
  final ValueChanged<int> onSelect;
  final ValueChanged<int> onClose;
  final VoidCallback onNew;

  const _TabStrip({
    required this.tabs,
    required this.active,
    required this.onSelect,
    required this.onClose,
    required this.onNew,
  });

  @override
  Widget build(BuildContext context) {
    return Material(
      color: Theme.of(context).colorScheme.surface,
      child: SizedBox(
        height: 44,
        child: Row(
          children: [
            Expanded(
              child: ListView.separated(
                padding: const EdgeInsets.symmetric(horizontal: 8),
                scrollDirection: Axis.horizontal,
                itemCount: tabs.length,
                separatorBuilder: (_, __) => const SizedBox(width: 8),
                itemBuilder: (context, index) {
                  final tab = tabs[index];
                  final selected = index == active;
                  return InputChip(
                    label: Text(
                      tab.title,
                      overflow: TextOverflow.ellipsis,
                    ),
                    selected: selected,
                    onSelected: (_) => onSelect(index),
                    onDeleted: tabs.length == 1 ? null : () => onClose(index),
                    deleteIcon: const Icon(Icons.close, size: 18),
                    visualDensity: VisualDensity.compact,
                  );
                },
              ),
            ),
            IconButton(
              tooltip: 'New tab',
              icon: const Icon(Icons.add),
              onPressed: onNew,
            ),
          ],
        ),
      ),
    );
  }
}

class _TerminalTabView extends StatefulWidget {
  final DesktopProfile desktop;
  final ValueChanged<String> onTitleChanged;

  const _TerminalTabView({
    super.key,
    required this.desktop,
    required this.onTitleChanged,
  });

  @override
  State<_TerminalTabView> createState() => _TerminalTabViewState();
}

class _TerminalTabViewState extends State<_TerminalTabView> {
  final LucidityClient _client = LucidityClient();
  late final Terminal _terminal;

  StreamSubscription<Uint8List>? _sub;
  _Utf8Coalescer? _coalescer;

  List<PaneInfo> _panes = const <PaneInfo>[];
  int? _paneId;
  String _paneTitle = 'New';
  
  // Auto-reconnection
  late final ConnectivityService _connectivity;
  final ExponentialBackoff _backoff = ExponentialBackoff();
  Timer? _reconnectTimer;
  bool _userDisconnected = false;

  @override
  void initState() {
    super.initState();

    _client.addListener(_onClientStateChanged);
    
    // Start connectivity monitoring
    _connectivity = ConnectivityService();
    _connectivity.addListener(_onConnectivityChanged);
    _connectivity.startMonitoring();

    _terminal = Terminal(
      maxLines: 10000,
      onOutput: (data) {
        _client.sendInput(data);
      },
      onResize: (w, h, pw, ph) {
        // w=cols, h=rows
        _client.sendResize(h, w);
      },
    );

    unawaited(_connectAndLoadPanes());
  }

  @override
  void dispose() {
    _reconnectTimer?.cancel();
    _connectivity.removeListener(_onConnectivityChanged);
    _connectivity.dispose();
    _client.removeListener(_onClientStateChanged);
    unawaited(_sub?.cancel());
    _coalescer?.dispose();
    _client.dispose();
    super.dispose();
  }

  void _onClientStateChanged() {
    if (!mounted) return;
    
    final state = _client.connectionState;
    
    if (state == LucidityConnectionState.connected) {
      _backoff.reset();
      _reconnectTimer?.cancel();
      if (_panes.isEmpty) {
        _panes = _client.panes;
      }
    } else if (state == LucidityConnectionState.disconnected || 
               state == LucidityConnectionState.error) {
      // Schedule auto-reconnect if not user-initiated disconnect
      if (!_userDisconnected && _connectivity.isOnline) {
        _scheduleReconnect();
      }
    }
    
    setState(() {});
  }
  
  void _onConnectivityChanged(bool isOnline) {
    if (!mounted) return;
    
    if (isOnline && !_client.connected && !_userDisconnected) {
      // Network came back online, attempt reconnect
      _backoff.reset();
      _scheduleReconnect();
    }
  }
  
  void _scheduleReconnect() {
    _reconnectTimer?.cancel();
    final delay = _backoff.nextDelay;
    
    if (mounted) {
      setState(() {
        _client._updateState(
          LucidityConnectionState.reconnecting, 
          'Reconnecting in ${delay.inSeconds}s (attempt ${_backoff.attempts})...',
        );
      });
    }
    
    _reconnectTimer = Timer(delay, () {
      if (mounted && !_client.connected) {
        unawaited(_connectAndLoadPanes());
      }
    });
  }

  Future<void> _connectAndLoadPanes() async {
    setState(() {
      _panes = const <PaneInfo>[];
      _paneId = null;
      _paneTitle = 'New';
    });
    widget.onTitleChanged(_paneTitle);

    try {
      final identity = context.read<AppState>().identity;
      if (identity == null) throw StateError("Mobile identity not loaded");

      final d = widget.desktop;
      await _client.connectWithStrategy(
        identity: identity,
        desktopPublicKey: d.desktopPublicKey,
        lanAddr: d.lanAddr ?? (d.host.isNotEmpty ? '${d.host}:${d.port}' : null),
        externalAddr: d.externalAddr,
        relayUrl: d.relayUrl,
        relayId: d.relayId,
        relaySecret: d.relaySecret,
      );
      // Panes are loaded automatically by client now
      final panes = await _client.listPanesOnce();
      if (mounted) {
        setState(() {
          _panes = panes;
        });
      }
    } catch (e) {
      // Error state is handled by client state listener
    }
  }

  Future<void> _attach(PaneInfo pane) async {
    final title = pane.title.isEmpty ? 'Pane ${pane.paneId}' : pane.title;
    setState(() {
      _paneId = pane.paneId;
      _paneTitle = title;
    });
    widget.onTitleChanged(title);

    try {
      await _client.attachAndWait(pane.paneId);
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _paneId = null;
        _paneTitle = 'New';
      });
      widget.onTitleChanged(_paneTitle);
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Attach failed: $e')),
      );
      return;
    }

    await _sub?.cancel();
    _coalescer?.dispose();
    _coalescer = _Utf8Coalescer(
      flushEvery: const Duration(milliseconds: 16),
      onText: (text) => _terminal.write(text),
    );
    _sub = _client.outputStream.listen(_coalescer!.addBytes);
    if (!mounted) return;
    setState(() {});
  }

  TerminalTheme _theme() {
    // Lucidity Premium Dark (OLED Black + Gold accents)
    return const TerminalTheme(
      cursor: Color(0xFFFFD700),
      selection: Color(0x33FFD700),
      foreground: Color(0xFFE0E0E0),
      background: Color(0xFF000000),
      black: Color(0xFF000000),
      red: Color(0xFFFF5252),
      green: Color(0xFF4CAF50),
      yellow: Color(0xFFFFD700),
      blue: Color(0xFF2196F3),
      magenta: Color(0xFFE040FB),
      cyan: Color(0xFF00BCD4),
      white: Color(0xFFEEEEEE),
      brightBlack: Color(0xFF757575),
      brightRed: Color(0xFFFF8A80),
      brightGreen: Color(0xFFB9F6CA),
      brightYellow: Color(0xFFFFFF8D),
      brightBlue: Color(0xFF82B1FF),
      brightMagenta: Color(0xFFEA80FC),
      brightCyan: Color(0xFF84FFFF),
      brightWhite: Color(0xFFFFFFFF),
    );
  }

  @override
  Widget build(BuildContext context) {
    // Connection State Handling
    final state = _client.connectionState;
    
    if (state == LucidityConnectionState.connecting || state == LucidityConnectionState.reconnecting) {
       return Column(
         mainAxisAlignment: MainAxisAlignment.center,
         children: [
           const CircularProgressIndicator(),
           const SizedBox(height: 16),
           Text(_client.statusMessage ?? 'Connecting...'),
         ],
       );
    }

    if (state == LucidityConnectionState.error || state == LucidityConnectionState.disconnected) {
       // Only show specific error UI if we are REALLY disconnected and not just initial state
       // But disconnected is initial state...
       // If statusMessage is set, likely an error or explicit disconnect.
       if (_client.statusMessage != null) {
          final errorMessage = LucidityErrorHandler.getErrorMessage(_client.statusMessage!);
          final suggestion = LucidityErrorHandler.getRecoverySuggestion(_client.statusMessage!);
          
          return Center(
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Icon(
                    state == LucidityConnectionState.error 
                        ? Icons.error_outline 
                        : Icons.cloud_off,
                    size: 64,
                    color: state == LucidityConnectionState.error 
                        ? Theme.of(context).colorScheme.error 
                        : null,
                  ),
                  const SizedBox(height: 16),
                  Text(
                    state == LucidityConnectionState.error ? 'Connection Error' : 'Disconnected',
                    style: Theme.of(context).textTheme.titleLarge,
                  ),
                  const SizedBox(height: 8),
                  Text(
                    errorMessage,
                    style: Theme.of(context).textTheme.bodyMedium,
                    textAlign: TextAlign.center,
                  ),
                  if (suggestion != null) ...[
                    const SizedBox(height: 12),
                    Container(
                      padding: const EdgeInsets.all(12),
                      decoration: BoxDecoration(
                        color: Theme.of(context).colorScheme.primaryContainer.withOpacity(0.3),
                        borderRadius: BorderRadius.circular(8),
                      ),
                      child: Row(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Icon(
                            Icons.lightbulb_outline,
                            size: 20,
                            color: Theme.of(context).colorScheme.primary,
                          ),
                          const SizedBox(width: 8),
                          Flexible(
                            child: Text(
                              suggestion,
                              style: Theme.of(context).textTheme.bodySmall,
                            ),
                          ),
                        ],
                      ),
                    ),
                  ],
                  const SizedBox(height: 20),
                  FilledButton.icon(
                    onPressed: _connectAndLoadPanes,
                    icon: const Icon(Icons.refresh),
                    label: const Text('Retry'),
                  ),
                  const SizedBox(height: 8),
                  TextButton(
                    onPressed: () => Navigator.of(context).pop(),
                    child: const Text('Go Back'),
                  ),
                ],
              ),
            ),
          );
       }
    }
    
    // If Connected:
    if (_paneId == null) {
      return _PanePicker(
        panes: _panes,
        onRefresh: _connectAndLoadPanes,
        onAttach: _attach,
      );
    }
    
    final mono = GoogleFonts.jetBrainsMono();
    return Column(
      children: [
         // Optional Status Bar if unstable?
         if (state != LucidityConnectionState.connected)
            Container(
              color: Colors.orange,
              padding: const EdgeInsets.all(4),
              width: double.infinity,
              child: Text(
                _client.connectionState.label, 
                style: const TextStyle(color: Colors.black, fontSize: 10),
                textAlign: TextAlign.center,
              ),
            ),

        Expanded(
          child: TerminalView(
            _terminal,
            autofocus: true,
            theme: _theme(),
            textStyle: TerminalStyle(
              fontFamily: mono.fontFamily ?? 'monospace',
              fontSize: 14,
            ),
          ),
        ),
        _AccessoryBar(
          onKey: _client.sendInput,
          onPaste: () async {
            final data = await Clipboard.getData(Clipboard.kTextPlain);
            if (data?.text != null) {
              _client.sendPaste(data!.text!);
            }
          },
        ),
      ],
    );
  }
}

class _PanePicker extends StatelessWidget {
  final List<PaneInfo> panes;
  final VoidCallback onRefresh;
  final Future<void> Function(PaneInfo pane) onAttach;

  const _PanePicker({
    required this.panes,
    required this.onRefresh,
    required this.onAttach,
  });

  @override
  Widget build(BuildContext context) {
    if (panes.isEmpty) {
      return Center(
        child: Padding(
          padding: const EdgeInsets.all(16),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              const Text('No panes found (or not loaded yet).'),
              const SizedBox(height: 12),
              FilledButton.icon(
                onPressed: onRefresh,
                icon: const Icon(Icons.refresh),
                label: const Text('Refresh'),
              ),
            ],
          ),
        ),
      );
    }

    return ListView.separated(
      itemCount: panes.length,
      separatorBuilder: (_, __) => const Divider(height: 1),
      itemBuilder: (context, index) {
        final p = panes[index];
        final title = p.title.isEmpty ? '(untitled)' : p.title;
        return ListTile(
          leading: const Icon(Icons.terminal),
          title: Text(title),
          subtitle: Text('Pane ${p.paneId}'),
          onTap: () => unawaited(onAttach(p)),
        );
      },
    );
  }
}

class _AccessoryBar extends StatelessWidget {
  final void Function(String text) onKey;
  final VoidCallback onPaste;

  const _AccessoryBar({required this.onKey, required this.onPaste});

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      top: false,
      child: Container(
        color: Theme.of(context).colorScheme.surface,
        height: 50,
        child: ListView(
          scrollDirection: Axis.horizontal,
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
          children: [
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 4),
              child: OutlinedButton.icon(
                style: OutlinedButton.styleFrom(
                  padding: const EdgeInsets.symmetric(horizontal: 12),
                  minimumSize: const Size(0, 36),
                  visualDensity: VisualDensity.compact,
                ),
                onPressed: () {
                  HapticFeedback.lightImpact();
                  onPaste();
                },
                icon: const Icon(Icons.paste, size: 16),
                label: const Text('Paste', style: TextStyle(fontWeight: FontWeight.bold)),
              ),
            ),
            _KeyButton(label: 'Esc', send: '\u001b', onKey: onKey),
            _KeyButton(label: 'Tab', send: '\t', onKey: onKey),
            _KeyButton(label: 'Ctrl+C', send: '\u0003', onKey: onKey),
            _KeyButton(label: 'Ctrl+D', send: '\u0004', onKey: onKey),
            _KeyButton(label: 'Ctrl+Z', send: '\u001a', onKey: onKey),
            _KeyButton(label: '↑', send: '\u001b[A', onKey: onKey),
            _KeyButton(label: '↓', send: '\u001b[B', onKey: onKey),
            _KeyButton(label: '←', send: '\u001b[D', onKey: onKey),
            _KeyButton(label: '→', send: '\u001b[C', onKey: onKey),
            _KeyButton(label: 'Home', send: '\u001b[H', onKey: onKey),
            _KeyButton(label: 'End', send: '\u001b[F', onKey: onKey),
            _KeyButton(label: 'PGUP', send: '\u001b[5~', onKey: onKey),
            _KeyButton(label: 'PGDN', send: '\u001b[6~', onKey: onKey),
          ],
        ),
      ),
    );
  }
}

class _KeyButton extends StatelessWidget {
  final String label;
  final String send;
  final void Function(String text) onKey;

  const _KeyButton({
    required this.label,
    required this.send,
    required this.onKey,
  });

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 4),
      child: OutlinedButton(
        style: OutlinedButton.styleFrom(
          padding: const EdgeInsets.symmetric(horizontal: 12),
          minimumSize: const Size(0, 36),
          visualDensity: VisualDensity.compact,
        ),
        onPressed: () {
          HapticFeedback.lightImpact();
          onKey(send);
        },
        child: Text(
          label,
          style: const TextStyle(fontWeight: FontWeight.bold),
        ),
      ),
    );
  }
}

class _Utf8Coalescer {
  final Duration flushEvery;
  final void Function(String text) onText;

  final BytesBuilder _pending = BytesBuilder(copy: false);
  final _Utf8PendingDecoder _decoder = _Utf8PendingDecoder();
  Timer? _timer;

  _Utf8Coalescer({required this.flushEvery, required this.onText});

  void addBytes(Uint8List bytes) {
    if (bytes.isEmpty) return;
    _pending.add(bytes);
    _timer ??= Timer(flushEvery, _flush);
  }

  void _flush() {
    _timer = null;
    if (_pending.length == 0) return;

    final bytes = _pending.takeBytes();
    final text = _decoder.pushAndDecode(bytes);
    if (text.isNotEmpty) {
      onText(text);
    }

    // If more bytes arrive while we were flushing, schedule again.
    if (_pending.length != 0) {
      _timer ??= Timer(flushEvery, _flush);
    }
  }

  void dispose() {
    _timer?.cancel();
    _timer = null;
  }
}

class _Utf8PendingDecoder {
  Uint8List _pending = Uint8List(0);

  String pushAndDecode(Uint8List next) {
    if (next.isEmpty && _pending.isEmpty) return '';
    _pending = Uint8List.fromList(<int>[..._pending, ...next]);

    // Try to decode the whole buffer; if it's incomplete UTF-8 at the end,
    // back off by up to 3 bytes and keep the remainder for the next flush.
    final maxBackoff = _pending.length < 3 ? _pending.length : 3;
    for (var backoff = 0; backoff <= maxBackoff; backoff++) {
      final end = _pending.length - backoff;
      if (end <= 0) break;
      try {
        final text = utf8.decode(_pending.sublist(0, end));
        _pending = Uint8List.fromList(_pending.sublist(end));
        return text;
      } catch (_) {
        // try backing off further
      }
    }

    // Last resort: don't get stuck. Decode with replacement and clear.
    final text = utf8.decode(_pending, allowMalformed: true);
    _pending = Uint8List(0);
    return text;
  }
}
