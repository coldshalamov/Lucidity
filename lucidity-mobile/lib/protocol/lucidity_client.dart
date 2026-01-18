import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter/foundation.dart';
import 'package:uuid/uuid.dart';

import 'constants.dart';
import 'frame.dart';
import 'messages.dart';

class LucidityClient extends ChangeNotifier {
  Socket? _socket;
  WebSocket? _relayControl;
  WebSocket? _relayData;
  final FrameDecoder _decoder = FrameDecoder();

  final StreamController<Uint8List> _outputController =
      StreamController<Uint8List>.broadcast();

  Completer<PairingPayload>? _pairingPayloadCompleter;
  Completer<PairingResponse>? _pairingResponseCompleter;
  Completer<List<PaneInfo>>? _listPanesCompleter;
  Completer<void>? _attachOkCompleter;

  bool _connected = false;
  bool get connected => _connected;

  String? _status;
  String? get status => _status;

  List<PaneInfo> _panes = const [];
  List<PaneInfo> get panes => _panes;

  int? _attachedPaneId;
  int? get attachedPaneId => _attachedPaneId;

  Stream<Uint8List> get outputStream => _outputController.stream;

  Future<void> connect(String host, int port) async {
    return connectTcp(host, port);
  }

  Future<void> connectTcp(String host, int port) async {
    await disconnect();

    _status = 'Connecting...';
    notifyListeners();

    final socket = await Socket.connect(
      host,
      port,
      timeout: const Duration(seconds: 5),
    );
    socket.setOption(SocketOption.tcpNoDelay, true);
    _socket = socket;
    _connected = true;
    _status = 'Connected';
    notifyListeners();

    socket.listen(
      (data) {
        _decoder.push(Uint8List.fromList(data));
        _processFrames();
      },
      onError: (Object err, StackTrace st) {
        _status = 'Socket error: $err';
        _connected = false;
        _failPending(StateError('socket error: $err'));
        notifyListeners();
      },
      onDone: () {
        _status = 'Disconnected';
        _connected = false;
        _failPending(StateError('socket closed'));
        notifyListeners();
      },
      cancelOnError: true,
    );

    // Automatically fetch panes on connect.
    await sendListPanes();
  }

  Future<void> connectRelay({
    required String relayBase,
    required String relayId,
    String? clientId,
    String? authToken,
  }) async {
    await disconnect();

    final cid = (clientId == null || clientId.trim().isEmpty)
        ? const Uuid().v4()
        : clientId.trim();

    _status = 'Connecting...';
    notifyListeners();

    final base = relayBase.endsWith('/') ? relayBase.substring(0, relayBase.length - 1) : relayBase;

    final headers = (authToken == null || authToken.trim().isEmpty)
        ? null
        : <String, dynamic>{
            'Authorization': 'Bearer ${authToken.trim()}',
          };

    final control = await WebSocket.connect(
      '$base/ws/mobile/$relayId',
      headers: headers,
    );
    _relayControl = control;

    final created = Completer<String>();
    final accepted = Completer<String>();

    late final StreamSubscription<dynamic> controlSub;
    controlSub = control.listen(
      (dynamic msg) {
        if (msg is! String) return;
        try {
          final obj = jsonDecode(msg);
          if (obj is! Map<String, dynamic>) return;
          if (obj['type'] != 'control') return;
          final code = obj['code'];
          final message = obj['message'];
          if (code is! int || message is! String) return;
          if (code != 200) {
            if (!created.isCompleted) created.completeError(StateError(message));
            if (!accepted.isCompleted) accepted.completeError(StateError(message));
            return;
          }
          if (message.startsWith('session_created:')) {
            final sid = message.substring('session_created:'.length);
            if (!created.isCompleted) created.complete(sid);
          } else if (message.startsWith('session_accepted:')) {
            final sid = message.substring('session_accepted:'.length);
            if (!accepted.isCompleted) accepted.complete(sid);
          }
        } catch (_) {
          // ignore
        }
      },
      onError: (Object err) {
        if (!created.isCompleted) created.completeError(err);
        if (!accepted.isCompleted) accepted.completeError(err);
      },
      onDone: () {
        if (!created.isCompleted) created.completeError(StateError('relay control closed'));
        if (!accepted.isCompleted) accepted.completeError(StateError('relay control closed'));
      },
      cancelOnError: true,
    );

    // Initiate session creation.
    control.add(jsonEncode({
      'type': 'connect',
      'relay_id': relayId,
      'pairing_client_id': cid,
    }));

    final sessionId = await created.future.timeout(const Duration(seconds: 10));
    final sessionAccepted = await accepted.future.timeout(const Duration(seconds: 60));
    if (sessionAccepted != sessionId) {
      throw StateError('relay accepted unexpected session id');
    }

    final data = await WebSocket.connect(
      '$base/ws/session/$sessionId?role=mobile',
      headers: headers,
    );
    _relayData = data;

    _connected = true;
    _status = 'Connected';
    notifyListeners();

    data.listen(
      (dynamic msg) {
        if (msg is List<int>) {
          _decoder.push(Uint8List.fromList(msg));
          _processFrames();
        }
      },
      onError: (Object err) {
        _status = 'Relay error: $err';
        _connected = false;
        _failPending(StateError('relay error: $err'));
        notifyListeners();
      },
      onDone: () {
        _status = 'Disconnected';
        _connected = false;
        _failPending(StateError('relay closed'));
        notifyListeners();
      },
      cancelOnError: true,
    );

    // Keep control open; we don't currently use it after session open.
    unawaited(controlSub.cancel());

    await sendListPanes();
  }

  Future<void> disconnect() async {
    _attachedPaneId = null;
    _panes = const [];
    _connected = false;
    _status = null;
    _failPending(StateError('disconnected'));
    notifyListeners();

    final s = _socket;
    _socket = null;
    if (s != null) {
      try {
        await s.close();
      } catch (_) {
        // ignore
      }
    }

    final d = _relayData;
    _relayData = null;
    if (d != null) {
      try {
        await d.close();
      } catch (_) {}
    }

    final c = _relayControl;
    _relayControl = null;
    if (c != null) {
      try {
        await c.close();
      } catch (_) {}
    }
  }

  Future<void> sendListPanes() async {
    await _sendJson({'op': 'list_panes'});
  }

  Future<List<PaneInfo>> listPanesOnce({
    Duration timeout = const Duration(seconds: 5),
  }) async {
    final socket = _socket;
    if (socket == null) return const <PaneInfo>[];

    final existing = _listPanesCompleter;
    if (existing != null && !existing.isCompleted) {
      return existing.future.timeout(timeout);
    }

    final c = Completer<List<PaneInfo>>();
    _listPanesCompleter = c;
    await sendListPanes();
    return c.future.timeout(timeout);
  }

  Future<void> attach(int paneId) async {
    _attachedPaneId = paneId;
    notifyListeners();
    await _sendJson({'op': 'attach', 'pane_id': paneId});
  }

  Future<void> attachAndWait(
    int paneId, {
    Duration timeout = const Duration(seconds: 5),
  }) async {
    final socket = _socket;
    if (socket == null) return;

    final existing = _attachOkCompleter;
    if (existing != null && !existing.isCompleted) {
      return existing.future.timeout(timeout);
    }

    final c = Completer<void>();
    _attachOkCompleter = c;
    await attach(paneId);
    return c.future.timeout(timeout);
  }

  Future<void> sendInput(String data) async {
    if (!_hasTransport) return;
    if (_attachedPaneId == null) {
      _status = 'Not attached to a pane yet';
      notifyListeners();
      return;
    }

    final payload = utf8.encode(data);
    _sendBytes(encodeFrame(type: typePaneInput, payload: payload));
  }

  Future<PairingPayload> pairingPayload() async {
    final existing = _pairingPayloadCompleter;
    if (existing != null && !existing.isCompleted) {
      return existing.future;
    }

    final c = Completer<PairingPayload>();
    _pairingPayloadCompleter = c;
    await _sendJson({'op': 'pairing_payload'});
    return c.future.timeout(const Duration(seconds: 10));
  }

  Future<PairingResponse> pairingSubmit(PairingRequest request) async {
    final c = Completer<PairingResponse>();
    _pairingResponseCompleter = c;
    await _sendJson({'op': 'pairing_submit', 'request': request.toJson()});
    return c.future.timeout(const Duration(seconds: 120));
  }

  Future<void> _sendJson(Map<String, Object?> msg) async {
    if (!_hasTransport) return;
    final payload = utf8.encode(jsonEncode(msg));
    _sendBytes(encodeFrame(type: typeJson, payload: payload));
  }

  bool get _hasTransport => _socket != null || _relayData != null;

  void _sendBytes(Uint8List bytes) {
    final socket = _socket;
    if (socket != null) {
      socket.add(bytes);
      return;
    }
    final ws = _relayData;
    if (ws != null) {
      ws.add(bytes);
    }
  }

  void _processFrames() {
    while (true) {
      final frame = _decoder.nextFrame();
      if (frame == null) return;

      switch (frame.type) {
        case typeJson:
          _handleJson(frame.payload);
          break;
        case typePaneOutput:
          _outputController.add(frame.payload);
          break;
        default:
          _status = 'Unsupported frame type: ${frame.type}';
          notifyListeners();
          break;
      }
    }
  }

  void _handleJson(Uint8List payload) {
    try {
      final text = utf8.decode(payload, allowMalformed: true);
      final obj = jsonDecode(text);
      if (obj is! Map<String, dynamic>) return;

      final op = obj['op'];
      if (op == 'list_panes') {
        final panesJson = obj['panes'];
        if (panesJson is List) {
          _panes = panesJson
              .whereType<Map>()
              .map((e) => PaneInfo.fromJson(Map<String, dynamic>.from(e)))
              .toList(growable: false);
          _status = 'Loaded ${_panes.length} panes';
          final c = _listPanesCompleter;
          _listPanesCompleter = null;
          if (c != null && !c.isCompleted) {
            c.complete(_panes);
          }
          notifyListeners();
        }
      } else if (op == 'error') {
        final msg = obj['message'];
        if (msg is String) {
          _status = 'Server error: $msg';
          _failPending(StateError(msg));
          notifyListeners();
        }
      } else if (op == 'attach_ok') {
        final c = _attachOkCompleter;
        _attachOkCompleter = null;
        if (c != null && !c.isCompleted) {
          c.complete();
        }
      } else if (op == 'pairing_payload') {
        final payloadJson = obj['payload'];
        if (payloadJson is Map<String, dynamic>) {
          final payload = PairingPayload.fromJson(payloadJson);
          _pairingPayloadCompleter?.complete(payload);
        }
      } else if (op == 'pairing_response') {
        final respJson = obj['response'];
        if (respJson is Map<String, dynamic>) {
          final resp = PairingResponse.fromJson(respJson);
          _pairingResponseCompleter?.complete(resp);
        }
      } else {
        // Ignore other ops for now (pairing_* etc.).
      }
    } catch (e) {
      _status = 'JSON decode failed: $e';
      notifyListeners();
    }
  }

  void _failPending(Object err) {
    final p = _pairingPayloadCompleter;
    _pairingPayloadCompleter = null;
    if (p != null && !p.isCompleted) {
      p.completeError(err);
    }

    final r = _pairingResponseCompleter;
    _pairingResponseCompleter = null;
    if (r != null && !r.isCompleted) {
      r.completeError(err);
    }

    final lp = _listPanesCompleter;
    _listPanesCompleter = null;
    if (lp != null && !lp.isCompleted) {
      lp.completeError(err);
    }

    final ao = _attachOkCompleter;
    _attachOkCompleter = null;
    if (ao != null && !ao.isCompleted) {
      ao.completeError(err);
    }
  }

  @override
  void dispose() {
    unawaited(disconnect());
    _outputController.close();
    super.dispose();
  }
}
