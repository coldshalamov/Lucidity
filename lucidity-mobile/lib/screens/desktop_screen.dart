import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/material.dart';
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
      body: Column(
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

  bool _connecting = false;
  Object? _connectError;

  List<PaneInfo> _panes = const <PaneInfo>[];
  int? _paneId;
  String _paneTitle = 'New';

  @override
  void initState() {
    super.initState();

    _terminal = Terminal(
      maxLines: 10000,
      onOutput: (data) {
        _client.sendInput(data);
      },
      onResize: (w, h, pw, ph) {},
    );

    unawaited(_connectAndLoadPanes());
  }

  @override
  void dispose() {
    unawaited(_sub?.cancel());
    _coalescer?.dispose();
    _client.dispose();
    super.dispose();
  }

  Future<void> _connectAndLoadPanes() async {
    setState(() {
      _connecting = true;
      _connectError = null;
      _panes = const <PaneInfo>[];
      _paneId = null;
      _paneTitle = 'New';
    });
    widget.onTitleChanged(_paneTitle);

    try {
      final d = widget.desktop;
      if (d.isPaired) {
        await _client.connectRelay(
          relayBase: AppConfig.relayBase,
          relayId: d.relayId!,
          clientId: d.id,
          authToken: context.read<AuthState>().token,
        );
      } else {
        await _client.connectTcp(d.host, d.port);
      }
      final panes = await _client.listPanesOnce();
      if (!mounted) return;
      setState(() {
        _panes = panes;
        _connecting = false;
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _connecting = false;
        _connectError = e;
      });
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
    // A WezTerm-ish dark palette (Tokyo Night vibe).
    return const TerminalTheme(
      cursor: Color(0xFFC0CAF5),
      selection: Color(0x335D87FF),
      foreground: Color(0xFFC0CAF5),
      background: Color(0xFF1A1B26),
      black: Color(0xFF15161E),
      red: Color(0xFFF7768E),
      green: Color(0xFF9ECE6A),
      yellow: Color(0xFFE0AF68),
      blue: Color(0xFF7AA2F7),
      magenta: Color(0xFFBB9AF7),
      cyan: Color(0xFF7DCFFF),
      white: Color(0xFFA9B1D6),
      brightBlack: Color(0xFF414868),
      brightRed: Color(0xFFF7768E),
      brightGreen: Color(0xFF9ECE6A),
      brightYellow: Color(0xFFE0AF68),
      brightBlue: Color(0xFF7AA2F7),
      brightMagenta: Color(0xFFBB9AF7),
      brightCyan: Color(0xFF7DCFFF),
      brightWhite: Color(0xFFC0CAF5),
    );
  }

  @override
  Widget build(BuildContext context) {
    if (_connecting) {
      return const Center(child: CircularProgressIndicator());
    }

    if (_connectError != null) {
      return Center(
        child: Padding(
          padding: const EdgeInsets.all(16),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              const Icon(Icons.link_off, size: 48),
              const SizedBox(height: 12),
              Text(
                'Could not connect',
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 8),
              Text(
                'Error: $_connectError',
                style: Theme.of(context).textTheme.bodySmall,
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 12),
              FilledButton.icon(
                onPressed: _connectAndLoadPanes,
                icon: const Icon(Icons.refresh),
                label: const Text('Retry'),
              ),
            ],
          ),
        ),
      );
    }

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
        _AccessoryBar(onKey: _client.sendInput),
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

  const _AccessoryBar({required this.onKey});

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      top: false,
      child: Container(
        color: Theme.of(context).colorScheme.surface,
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
        child: Row(
          mainAxisAlignment: MainAxisAlignment.spaceEvenly,
          children: [
            _KeyButton(label: 'Esc', send: '\u001b', onKey: onKey),
            _KeyButton(label: 'Tab', send: '\t', onKey: onKey),
            _KeyButton(label: 'Ctrl+C', send: '\u0003', onKey: onKey),
            _KeyButton(label: '↑', send: '\u001b[A', onKey: onKey),
            _KeyButton(label: '↓', send: '\u001b[B', onKey: onKey),
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
    return OutlinedButton(
      onPressed: () => onKey(send),
      child: Text(label),
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
