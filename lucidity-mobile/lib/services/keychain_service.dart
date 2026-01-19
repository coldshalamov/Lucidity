import 'dart:convert';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';

class KeychainService {
  final _storage = const FlutterSecureStorage();

  static const _keyPairSeedKey = 'lucidity_identity_seed';
  static const _trustedDesktopsKey = 'lucidity_trusted_desktops';

  Future<void> saveSeed(String seedBase64) async {
    await _storage.write(key: _keyPairSeedKey, value: seedBase64);
  }

  Future<String?> readSeed() async {
    return await _storage.read(key: _keyPairSeedKey);
  }

  Future<void> deleteSeed() async {
    await _storage.delete(key: _keyPairSeedKey);
  }

  Future<void> saveTrustedDesktop(String id, String name, String fingerprint) async {
    final desktops = await listTrustedDesktops();
    desktops[id] = {'name': name, 'fingerprint': fingerprint, 'lastConnected': DateTime.now().toIso8601String()};
    
    await _storage.write(key: _trustedDesktopsKey, value: jsonEncode(desktops));
  }

  Future<Map<String, Map<String, dynamic>>> listTrustedDesktops() async {
    final jsonStr = await _storage.read(key: _trustedDesktopsKey);
    if (jsonStr == null) return {};
    
    try {
      final Map<String, dynamic> raw = jsonDecode(jsonStr);
      return raw.map((key, value) => MapEntry(key, value as Map<String, dynamic>));
    } catch (e) {
      return {};
    }
  }

  Future<void> deleteTrustedDesktop(String id) async {
    final desktops = await listTrustedDesktops();
    desktops.remove(id);
    await _storage.write(key: _trustedDesktopsKey, value: jsonEncode(desktops));
  }
}

