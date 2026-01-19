import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:cryptography/cryptography.dart';
import 'package:flutter/foundation.dart';
import 'package:uuid/uuid.dart';

import 'constants.dart';
import 'frame.dart';
import 'messages.dart';
import 'mobile_identity.dart';

import 'connection_state.dart';

class LucidityClient extends ChangeNotifier {
  Socket? _socket;
  final FrameDecoder _decoder = FrameDecoder();

  final StreamController<Uint8List> _outputController =
      StreamController<Uint8List>.broadcast();

  Completer<PairingPayload>? _pairingPayloadCompleter;
  Completer<PairingResponse>? _pairingResponseCompleter;
  Completer<List<PaneInfo>>? _listPanesCompleter;
  Completer<void>? _attachOkCompleter;

  String? _expectedDesktopPublicKey;
  String? _clientNonce;

  LucidityConnectionState _connectionState = LucidityConnectionState.disconnected;
  LucidityConnectionState get connectionState => _connectionState;

  bool get connected => _connectionState == LucidityConnectionState.connected;

  String? _statusMessage;
  String? get statusMessage => _statusMessage;

  List<PaneInfo> _panes = const [];
  List<PaneInfo> get panes => _panes;

  int? _attachedPaneId;
  int? get attachedPaneId => _attachedPaneId;

  Stream<Uint8List> get outputStream => _outputController.stream;

  Future<void> connect(
    String host,
    int port, {
    required SimpleKeyPairData identity,
    String? expectedDesktopPublicKey,
  }) async {
    return connectTcp(
      host,
      port,
      identity: identity,
      expectedDesktopPublicKey: expectedDesktopPublicKey,
    );
  }

  /// Connect using the best available strategy from pairing info
  /// Tries: 1) LAN direct, 2) External (UPnP)
  Future<void> connectWithStrategy({
    required SimpleKeyPairData identity,
    String? desktopPublicKey,
    String? lanAddr,
    String? externalAddr,
  }) async {
    // Strategy 1: Try LAN connection first (fastest)
    if (lanAddr != null && lanAddr.isNotEmpty) {
      try {
        final parts = lanAddr.split(':');
        if (parts.length == 2) {
          final host = parts[0];
          final port = int.tryParse(parts[1]) ?? 9797;
          _updateState(LucidityConnectionState.connecting, 'Trying LAN connection...');
          await connectTcp(
            host,
            port,
            identity: identity,
            expectedDesktopPublicKey: desktopPublicKey,
          );
          if (connected) return;
        }
      } catch (e) {
        debugPrint('LAN connection failed: $e');
      }
    }

    // Strategy 2: Try external (UPnP) connection for remote access
    if (externalAddr != null && externalAddr.isNotEmpty) {
      try {
        final parts = externalAddr.split(':');
        if (parts.length == 2) {
          final host = parts[0];
          final port = int.tryParse(parts[1]) ?? 9797;
          _updateState(LucidityConnectionState.connecting, 'Connecting to Desktop...');
          await connectTcp(
            host,
            port,
            identity: identity,
            expectedDesktopPublicKey: desktopPublicKey,
          );
          if (connected) return;
        }
      } catch (e) {
        debugPrint('Direct remote connection failed: $e');
      }
    }

    // All strategies failed
    _updateState(LucidityConnectionState.error, 'Could not connect to Desktop');
    throw Exception('Could not connect: ensure your computer is online and Lucidity is active.');
  }

  Future<void> connectTcp(
    String host,
    int port, {
    SimpleKeyPairData? identity,
    String? expectedDesktopPublicKey,
    bool pairing = false,
  }) async {
    _expectedDesktopPublicKey = expectedDesktopPublicKey;
    if (!pairing && identity == null) {
      throw ArgumentError('identity required for non-pairing connection');
    }

    await disconnect();

    _updateState(LucidityConnectionState.connecting, 'Connecting to $host:$port...');

    try {
      final socket = await Socket.connect(
        host,
        port,
        timeout: const Duration(seconds: 8),
      );
      socket.setOption(SocketOption.tcpNoDelay, true);
      _socket = socket;
      
      _updateState(LucidityConnectionState.connected, 'Connected to $host');

      socket.listen(
        (data) {
          _decoder.push(Uint8List.fromList(data));
          _processFrames(identity);
        },
        onError: (Object err, StackTrace st) {
          _updateState(LucidityConnectionState.error, 'Socket error: $err');
          _failPending(StateError('socket error: $err'));
        },
        onDone: () {
          _updateState(LucidityConnectionState.disconnected, 'Socket closed');
          _failPending(StateError('socket closed'));
        },
        cancelOnError: true,
      );

      // If pairing, we don't auth or list panes automatically.
      if (!pairing) {
        await sendListPanes();
      }
    } catch (e) {
      _updateState(LucidityConnectionState.error, 'Connect failed: $e');
      rethrow;
    }
  }

  Future<void> disconnect() async {
    _attachedPaneId = null;
    _panes = const [];
    _updateState(LucidityConnectionState.disconnected, 'Disconnected');
    _failPending(StateError('disconnected'));

    final s = _socket;
    _socket = null;
    if (s != null) {
      try {
        await s.close();
      } catch (_) {}
    }
  }
  
  void _updateState(LucidityConnectionState state, String? msg) {
    if (_connectionState != state || _statusMessage != msg) {
      _connectionState = state;
      _statusMessage = msg;
      notifyListeners();
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

  Future<void> _processFrames(SimpleKeyPairData? identity) async {
    while (true) {
      final frame = _decoder.nextFrame();
      if (frame == null) return;

      switch (frame.type) {
        case typeJson:
          await _handleJson(frame.payload, identity);
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

  Future<void> _handleJson(Uint8List payload, SimpleKeyPairData? identity) async {
    try {
      final text = utf8.decode(payload, allowMalformed: true);
      final obj = jsonDecode(text);
      if (obj is! Map<String, dynamic>) return;

      final op = obj['op'];
      if (op == 'auth_challenge') {
        if (identity == null) return; // Ignore auth if no identity (pairing mode)
        final challenge = AuthChallenge.fromJson(obj);
        final signature = await MobileIdentity().sign(
          identity,
          utf8.encode(challenge.nonce),
        );
        final sigBase64 = Base64UrlNoPad.encode(signature);
        final pubKey = MobileIdentity().publicKeyBase64UrlNoPad(identity);
        _clientNonce = Uuid().v4();

        await _sendJson({
          'op': 'auth_response',
          'public_key': pubKey,
          'signature': sigBase64,
          'client_nonce': _clientNonce,
        });
      } else if (op == 'auth_success') {
        final success = AuthSuccess.fromJson(obj);
        final hostSig = success.signature;
        final expectedPub = _expectedDesktopPublicKey;
        final nonce = _clientNonce;

        if (hostSig != null && expectedPub != null && nonce != null) {
          final verified = await MobileIdentity().verify(
            Base64UrlNoPad.decode(expectedPub),
            utf8.encode(nonce),
            Base64UrlNoPad.decode(hostSig),
          );
          if (!verified) {
            _updateState(LucidityConnectionState.error, 'Host verification failed');
            await disconnect();
            return;
          }
          _status = 'Authenticated & Verified Host';
        } else {
          _status = 'Authenticated';
        }
        notifyListeners();
      } else if (op == 'list_panes') {
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
