import 'package:flutter_test/flutter_test.dart';

import 'package:lucidity_mobile/app/desktop_profile.dart';

void main() {
  test('DesktopProfile JSON roundtrip', () {
    const d = DesktopProfile(
      id: 'id-1',
      displayName: 'My Desktop',
      host: '100.64.0.1',
      port: 9797,
      desktopPublicKey: 'abcd',
      relayId: 'relay',
      createdAtSeconds: 1700000000,
      lastConnectedAtSeconds: 1700000001,
    );

    final decoded = DesktopProfile.fromJson(d.toJson());
    expect(decoded.id, d.id);
    expect(decoded.displayName, d.displayName);
    expect(decoded.host, d.host);
    expect(decoded.port, d.port);
    expect(decoded.desktopPublicKey, d.desktopPublicKey);
    expect(decoded.relayId, d.relayId);
    expect(decoded.createdAtSeconds, d.createdAtSeconds);
    expect(decoded.lastConnectedAtSeconds, d.lastConnectedAtSeconds);
  });
}

