import 'dart:math';

class DesktopProfile {
  final String id;
  final String displayName;
  String get name => displayName;
  final String host;
  final int port;

  /// From pairing QR payload (optional for manual desktops).
  final String? desktopPublicKey;
  final String? relayId;
  
  /// Relay server URL for fallback connection
  final String? relayUrl;
  final String? relaySecret;

  /// P2P connection addresses
  final String? lanAddr;
  final String? externalAddr;

  final int createdAtSeconds;
  final int? lastConnectedAtSeconds;

  const DesktopProfile({
    required this.id,
    required this.displayName,
    required this.host,
    required this.port,
    required this.createdAtSeconds,
    required this.lastConnectedAtSeconds,
    required this.desktopPublicKey,
    required this.relayId,
    this.relayUrl,
    this.relaySecret,
    this.lanAddr,
    this.externalAddr,
  });

  bool get isPaired => desktopPublicKey != null && relayId != null;

  /// Whether this profile supports direct P2P connections
  bool get supportsP2P => externalAddr != null && externalAddr!.isNotEmpty;
  
  /// Whether this profile supports relay fallback
  bool get supportsRelay => relayUrl != null && relayUrl!.isNotEmpty && relayId != null;

  String get desktopFingerprintShort {
    final k = desktopPublicKey;
    if (k == null || k.isEmpty) return '';
    if (k.length <= 16) return k;
    final prefix = k.substring(0, 8);
    final suffixLen = min(6, k.length);
    final suffix = k.substring(k.length - suffixLen);
    return '$prefix...$suffix';
  }

  DesktopProfile copyWith({
    String? id,
    String? displayName,
    String? host,
    int? port,
    String? desktopPublicKey,
    String? relayId,
    String? relayUrl,
    String? relaySecret,
    String? lanAddr,
    String? externalAddr,
    int? createdAtSeconds,
    int? lastConnectedAtSeconds,
  }) {
    return DesktopProfile(
      id: id ?? this.id,
      displayName: displayName ?? this.displayName,
      host: host ?? this.host,
      port: port ?? this.port,
      desktopPublicKey: desktopPublicKey ?? this.desktopPublicKey,
      relayId: relayId ?? this.relayId,
      relayUrl: relayUrl ?? this.relayUrl,
      relaySecret: relaySecret ?? this.relaySecret,
      lanAddr: lanAddr ?? this.lanAddr,
      externalAddr: externalAddr ?? this.externalAddr,
      createdAtSeconds: createdAtSeconds ?? this.createdAtSeconds,
      lastConnectedAtSeconds: lastConnectedAtSeconds ?? this.lastConnectedAtSeconds,
    );
  }

  Map<String, Object?> toJson() => {
        'id': id,
        'display_name': displayName,
        'host': host,
        'port': port,
        'desktop_public_key': desktopPublicKey,
        'relay_id': relayId,
        'relay_url': relayUrl,
        'relay_secret': relaySecret,
        'lan_addr': lanAddr,
        'external_addr': externalAddr,
        'created_at': createdAtSeconds,
        'last_connected_at': lastConnectedAtSeconds,
      };

  factory DesktopProfile.fromJson(Map<String, Object?> json) {
    final id = json['id'];
    final displayName = json['display_name'];
    final host = json['host'];
    final port = json['port'];
    final desktopPublicKey = json['desktop_public_key'];
    final relayId = json['relay_id'];
    final relayUrl = json['relay_url'];
    final relaySecret = json['relay_secret'];
    final lanAddr = json['lan_addr'];
    final externalAddr = json['external_addr'];
    final createdAt = json['created_at'];
    final lastConnectedAt = json['last_connected_at'];

    if (id is! String || id.isEmpty) throw FormatException('invalid id');
    if (displayName is! String || displayName.isEmpty) {
      throw FormatException('invalid display_name');
    }
    if (desktopPublicKey != null && desktopPublicKey is! String) {
      throw FormatException('invalid desktop_public_key');
    }
    if (relayId != null && relayId is! String) throw FormatException('invalid relay_id');
    if (relayUrl != null && relayUrl is! String) throw FormatException('invalid relay_url');
    if (relaySecret != null && relaySecret is! String) throw FormatException('invalid relay_secret');
    final isPaired = desktopPublicKey is String && relayId is String;

    if (host is! String) throw FormatException('invalid host');
    if (host.isEmpty && !isPaired) throw FormatException('invalid host');
    if (port is! int || port < 0 || port > 65535) {
      throw FormatException('invalid port');
    }
    if (port == 0 && !isPaired) throw FormatException('invalid port');
    if (createdAt is! int || createdAt <= 0) throw FormatException('invalid created_at');
    if (lastConnectedAt != null && lastConnectedAt is! int) {
      throw FormatException('invalid last_connected_at');
    }

    return DesktopProfile(
      id: id,
      displayName: displayName,
      host: host,
      port: port,
      desktopPublicKey: desktopPublicKey as String?,
      relayId: relayId as String?,
      relayUrl: relayUrl as String?,
      relaySecret: relaySecret as String?,
      lanAddr: lanAddr as String?,
      externalAddr: externalAddr as String?,
      createdAtSeconds: createdAt,
      lastConnectedAtSeconds: lastConnectedAt as int?,
    );
  }
}

