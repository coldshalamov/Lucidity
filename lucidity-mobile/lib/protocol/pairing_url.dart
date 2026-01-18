import 'dart:convert';

import 'base64url_nopad.dart';
import 'messages.dart';

/// Parses the Lucidity pairing URL encoded in the desktop QR code.
///
/// Desktop encodes: `lucidity://pair?data=<base64url_no_pad(JSON)>`
PairingPayload parsePairingUrl(String url) {
  final uri = Uri.parse(url);
  if (uri.scheme != 'lucidity') {
    throw FormatException('invalid scheme: ${uri.scheme}');
  }
  if (uri.host != 'pair') {
    throw FormatException('invalid host: ${uri.host}');
  }

  final data = uri.queryParameters['data'];
  if (data == null || data.isEmpty) {
    throw FormatException('missing data parameter');
  }

  final jsonText = utf8.decode(Base64UrlNoPad.decode(data));
  final obj = jsonDecode(jsonText);
  if (obj is! Map<String, dynamic>) {
    throw FormatException('pairing payload is not an object');
  }
  return PairingPayload.fromJson(obj);
}
