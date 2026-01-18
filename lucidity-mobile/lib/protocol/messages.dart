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

  const PairingPayload({
    required this.desktopPublicKey,
    required this.relayId,
    required this.timestamp,
    required this.version,
  });

  factory PairingPayload.fromJson(Map<String, dynamic> json) {
    final desktopPublicKey = json['desktop_public_key'];
    final relayId = json['relay_id'];
    final timestamp = json['timestamp'];
    final version = json['version'];

    if (desktopPublicKey is! String) {
      throw FormatException('desktop_public_key is not string: $desktopPublicKey');
    }
    if (relayId is! String) {
      throw FormatException('relay_id is not string: $relayId');
    }
    if (timestamp is! int) {
      throw FormatException('timestamp is not int: $timestamp');
    }
    if (version is! int) {
      throw FormatException('version is not int: $version');
    }

    return PairingPayload(
      desktopPublicKey: desktopPublicKey,
      relayId: relayId,
      timestamp: timestamp,
      version: version,
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
