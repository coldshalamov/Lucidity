import 'dart:typed_data';

import 'package:cryptography/cryptography.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'base64url_nopad.dart';

class MobileIdentity {
  static const _prefsKey = 'lucidity_mobile_ed25519_seed_v1';

  final Ed25519 _algo = Ed25519();

  Future<SimpleKeyPairData> loadOrCreate() async {
    final prefs = await SharedPreferences.getInstance();
    final stored = prefs.getString(_prefsKey);
    if (stored != null && stored.isNotEmpty) {
      final seed = Base64UrlNoPad.decode(stored);
      if (seed.length != 32) {
        throw StateError('stored key seed is invalid length: ${seed.length}');
      }
      final kp = await _algo.newKeyPairFromSeed(seed);
      return await kp.extract() as SimpleKeyPairData;
    }

    final kp = await _algo.newKeyPair();
    final data = await kp.extract() as SimpleKeyPairData;
    if (data.bytes.length != 32) {
      throw StateError('unexpected private key length: ${data.bytes.length}');
    }

    await prefs.setString(_prefsKey, Base64UrlNoPad.encode(data.bytes));
    return data;
  }

  String publicKeyBase64UrlNoPad(SimpleKeyPairData data) {
    return Base64UrlNoPad.encode(data.publicKey.bytes);
  }

  /// Derive fingerprint from public key (SHA-256)
  Future<String> fingerprint(SimpleKeyPairData data) async {
    final sink = Sha256().newHashSink();
    sink.add(data.publicKey.bytes);
    sink.close();
    final hash = await sink.hash();
    return Base64UrlNoPad.encode(hash.bytes);
  }

  Future<Uint8List> signDesktopKeyAndTimestamp({
    required SimpleKeyPairData identity,
    required Uint8List desktopPublicKey,
    required int timestampSeconds,
  }) async {
    final ts = ByteData(8)..setInt64(0, timestampSeconds, Endian.little);
    final msg = Uint8List(desktopPublicKey.length + 8);
    msg.setRange(0, desktopPublicKey.length, desktopPublicKey);
    msg.setRange(desktopPublicKey.length, msg.length, ts.buffer.asUint8List());

    return sign(identity, msg);
  }

  Future<Uint8List> sign(SimpleKeyPairData identity, List<int> message) async {
    final sig = await _algo.sign(
      message,
      keyPair: identity,
    );
    return Uint8List.fromList(sig.bytes);
  }
}

