import 'dart:convert';
import 'dart:typed_data';

/// URL-safe Base64 without padding, compatible with Rust
/// `base64::engine::general_purpose::URL_SAFE_NO_PAD`.
class Base64UrlNoPad {
  static String encode(List<int> bytes) {
    final s = base64Url.encode(bytes);
    return s.replaceAll('=', '');
  }

  static Uint8List decode(String s) {
    // Add padding back if needed.
    final mod = s.length % 4;
    final padded = mod == 0 ? s : (s + ('=' * (4 - mod)));
    return Uint8List.fromList(base64Url.decode(padded));
  }
}

