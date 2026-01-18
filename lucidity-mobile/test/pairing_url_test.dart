import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';

import 'package:lucidity_mobile/protocol/base64url_nopad.dart';
import 'package:lucidity_mobile/protocol/pairing_url.dart';

void main() {
  test('parsePairingUrl decodes lucidity://pair?data=... payload', () {
    final payloadJson = jsonEncode({
      'desktop_public_key': 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',
      'relay_id': 'relay1234',
      'timestamp': 123,
      'version': 1,
    });

    final url = 'lucidity://pair?data=${Base64UrlNoPad.encode(utf8.encode(payloadJson))}';
    final payload = parsePairingUrl(url);

    expect(payload.desktopPublicKey, 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA');
    expect(payload.relayId, 'relay1234');
    expect(payload.timestamp, 123);
    expect(payload.version, 1);
  });

  test('parsePairingUrl rejects non-lucidity URLs', () {
    expect(() => parsePairingUrl('http://example.com'), throwsFormatException);
    expect(() => parsePairingUrl('lucidity://nope?data=x'), throwsFormatException);
  });
}
