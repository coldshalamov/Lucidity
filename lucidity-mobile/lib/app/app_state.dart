import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:uuid/uuid.dart';

import 'package:cryptography/cryptography.dart';

import '../protocol/messages.dart';
import '../protocol/mobile_identity.dart';
import 'desktop_profile.dart';
import 'desktop_store.dart';

class AppState extends ChangeNotifier {
  final Uuid _uuid = const Uuid();

  bool _ready = false;
  bool get ready => _ready;

  List<DesktopProfile> _desktops = const <DesktopProfile>[];
  List<DesktopProfile> get desktops => _desktops;

  String? _lastDesktopId;
  String? get lastDesktopId => _lastDesktopId;

  AppState() {
    unawaited(_load());
  }

  DesktopProfile? desktopById(String id) {
    for (final d in _desktops) {
      if (d.id == id) return d;
    }
    return null;
  }

  DesktopProfile? desktopByPublicKey(String desktopPublicKey) {
    for (final d in _desktops) {
      if (d.desktopPublicKey == desktopPublicKey) return d;
    }
    return null;
  }

  bool _autoReconnect = true;
  bool get autoReconnect => _autoReconnect;
  
  set autoReconnect(bool value) {
    _autoReconnect = value;
    notifyListeners();
  }
  
  /// Returns the last connected desktop if available and auto-reconnect is enabled.
  DesktopProfile? get lastConnectedDesktop {
    if (_lastDesktopId == null || !_autoReconnect) return null;
    return desktopById(_lastDesktopId!);
  }

  SimpleKeyPairData? _identity;
  SimpleKeyPairData? get identity => _identity;

  Future<void> _load() async {
    try {
      final loaded = await DesktopStore.loadDesktops();
      final last = await DesktopStore.loadLastDesktopId();
      final id = await MobileIdentity().loadOrCreate();
      
      _desktops = loaded;
      _lastDesktopId = last;
      _identity = id;
    } finally {
      _ready = true;
      notifyListeners();
    }
  }

  Future<void> _persist() async {
    await DesktopStore.saveDesktops(_desktops);
    await DesktopStore.saveLastDesktopId(_lastDesktopId);
  }
  
  /// Clears the saved session (last connected desktop).
  Future<void> clearLastSession() async {
    _lastDesktopId = null;
    notifyListeners();
    await DesktopStore.saveLastDesktopId(null);
  }

  Future<DesktopProfile> addManualDesktop({
    required String displayName,
    required String host,
    required int port,
  }) async {
    final now = DateTime.now().millisecondsSinceEpoch ~/ 1000;
    final d = DesktopProfile(
      id: _uuid.v4(),
      displayName: displayName,
      host: host,
      port: port,
      desktopPublicKey: null,
      relayId: null,
      createdAtSeconds: now,
      lastConnectedAtSeconds: null,
    );

    _desktops = <DesktopProfile>[d, ..._desktops];
    _lastDesktopId = d.id;
    notifyListeners();
    await _persist();
    return d;
  }

  Future<DesktopProfile> upsertFromPairing({
    required PairingPayload payload,
    required String host,
    required int port,
    String? displayName,
  }) async {
    final now = DateTime.now().millisecondsSinceEpoch ~/ 1000;
    final existing = desktopByPublicKey(payload.desktopPublicKey);
    
    // Default name if none provided
    final name = (displayName == null || displayName.trim().isEmpty)
        ? (existing?.displayName ?? 'WezTerm Desktop')
        : displayName.trim();

    final next = (existing ?? DesktopProfile(
      id: _uuid.v4(),
      displayName: name,
      host: host,
      port: port,
      desktopPublicKey: payload.desktopPublicKey,
      relayId: payload.relayId,
      createdAtSeconds: now,
      lastConnectedAtSeconds: null,
    )).copyWith(
      displayName: name,
      host: host,
      port: port,
      desktopPublicKey: payload.desktopPublicKey,
      relayId: payload.relayId,
      relayUrl: payload.relayUrl,
      relaySecret: payload.relaySecret,
      lanAddr: payload.lanAddr,
      externalAddr: payload.externalAddr,
      lastConnectedAtSeconds: now,
    );

    _desktops = <DesktopProfile>[
      next,
      ..._desktops.where((d) => d.id != next.id),
    ];
    _lastDesktopId = next.id;
    notifyListeners();
    await _persist();
    return next;
  }

  Future<void> updateDesktop(DesktopProfile updated) async {
    _desktops = _desktops.map((d) => d.id == updated.id ? updated : d).toList(growable: false);
    _lastDesktopId = updated.id;
    notifyListeners();
    await _persist();
  }

  Future<void> deleteDesktop(String id) async {
    _desktops = _desktops.where((d) => d.id != id).toList(growable: false);
    if (_lastDesktopId == id) _lastDesktopId = null;
    notifyListeners();
    await _persist();
  }
}

