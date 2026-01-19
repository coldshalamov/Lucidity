class PaneInfo {
  final int paneId;
  final String title;

  const PaneInfo({required this.paneId, required this.title});

  factory PaneInfo.fromJson(Map<String, dynamic> json) {
    final paneId = json['pane_id'];
    final title = json['title'];
    if (paneId is! int) {
      throw FormatException('pane_id is not int: $paneId');
    }
    if (title is! String) {
      throw FormatException('title is not string: $title');
    }
    return PaneInfo(paneId: paneId, title: title);
  }
}

class PairingPayload {
  final String desktopPublicKey; // base64url(no pad), 32 bytes
  final String relayId;
  final int timestamp; // unix seconds
  final int version;
  final String? lanAddr;
  final String? externalAddr;
  final String? relayUrl;
  final String? relaySecret;
  final List<String> capabilities;

  const PairingPayload({
    required this.desktopPublicKey,
    required this.relayId,
    required this.timestamp,
    required this.version,
    this.lanAddr,
    this.externalAddr,
    this.relayUrl,
    this.relaySecret,
    this.capabilities = const [],
  });

  /// Whether this payload supports relay fallback
  bool get supportsRelay => relayUrl != null && relayUrl!.isNotEmpty;
  
  /// Whether this payload supports direct P2P
  bool get supportsP2P => externalAddr != null && externalAddr!.isNotEmpty;
  
  /// Whether this payload supports LAN connections
  bool get supportsLan => lanAddr != null && lanAddr!.isNotEmpty;

  factory PairingPayload.fromJson(Map<String, dynamic> json) {
    return PairingPayload(
      desktopPublicKey: json['desktop_public_key'] as String,
      relayId: json['relay_id'] as String,
      timestamp: json['timestamp'] as int,
      version: json['version'] as int,
      lanAddr: json['lan_addr'] as String?,
      externalAddr: json['external_addr'] as String?,
      relayUrl: json['relay_url'] as String?,
      relaySecret: json['relay_secret'] as String?,
      capabilities: (json['capabilities'] as List?)?.whereType<String>().toList() ?? const [],
    );
  }
}

class PairingRequest {
  final String mobilePublicKey; // base64url(no pad), 32 bytes
  final String signature; // base64url(no pad), 64 bytes
  final String userEmail;
  final String deviceName;
  final int timestamp; // unix seconds

  const PairingRequest({
    required this.mobilePublicKey,
    required this.signature,
    required this.userEmail,
    required this.deviceName,
    required this.timestamp,
  });

  Map<String, Object?> toJson() => {
        'mobile_public_key': mobilePublicKey,
        'signature': signature,
        'user_email': userEmail,
        'device_name': deviceName,
        'timestamp': timestamp,
      };
}

class PairingResponse {
  final bool approved;
  final String? reason;

  const PairingResponse({required this.approved, required this.reason});

  factory PairingResponse.fromJson(Map<String, dynamic> json) {
    final approved = json['approved'];
    final reason = json['reason'];

    if (approved is! bool) {
      throw FormatException('approved is not bool: $approved');
    }
    if (reason != null && reason is! String) {
      throw FormatException('reason is not string?: $reason');
    }

    return PairingResponse(approved: approved, reason: reason as String?);
  }
}

class AuthChallenge {
  final String nonce;

  const AuthChallenge({required this.nonce});

  factory AuthChallenge.fromJson(Map<String, dynamic> json) {
    final nonce = json['nonce'];
    if (nonce is! String) {
      throw FormatException('nonce is not string: $nonce');
    }
    return AuthChallenge(nonce: nonce);
  }
}

class AuthResponse {
  final String publicKey;
  final String signature;
  final String? clientNonce;

  const AuthResponse({
    required this.publicKey,
    required this.signature,
    this.clientNonce,
  });

  Map<String, Object?> toJson() => {
        'op': 'auth_response',
        'public_key': publicKey,
        'signature': signature,
        if (clientNonce != null) 'client_nonce': clientNonce,
      };
}

class AuthSuccess {
  final String? signature;

  const AuthSuccess({this.signature});

  factory AuthSuccess.fromJson(Map<String, dynamic> json) {
    return AuthSuccess(signature: json['signature'] as String?);
  }
}
