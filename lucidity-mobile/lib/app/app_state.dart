import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:uuid/uuid.dart';

import '../protocol/messages.dart';
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

  Future<void> _load() async {
    try {
      final loaded = await DesktopStore.loadDesktops();
      final last = await DesktopStore.loadLastDesktopId();
      _desktops = loaded;
      _lastDesktopId = last;
    } finally {
      _ready = true;
      notifyListeners();
    }
  }

  Future<void> _persist() async {
    await DesktopStore.saveDesktops(_desktops);
    await DesktopStore.saveLastDesktopId(_lastDesktopId);
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
    final name = (displayName == null || displayName.trim().isEmpty)
        ? 'WezTerm Desktop (${payload.relayId})'
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

