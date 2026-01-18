class AppConfig {
  /// WebSocket base URL for the Lucidity relay service.
  ///
  /// Configure at build time:
  /// `flutter run --dart-define=LUCIDITY_RELAY_BASE=wss://relay.example.com`
  static const String relayBase = String.fromEnvironment(
    'LUCIDITY_RELAY_BASE',
    defaultValue: 'ws://127.0.0.1:9090',
  );

  /// Base URL for the auth service (HTTP).
  ///
  /// Configure at build time:
  /// `flutter run --dart-define=LUCIDITY_AUTH_BASE=http://127.0.0.1:9091`
  static const String authBase = String.fromEnvironment(
    'LUCIDITY_AUTH_BASE',
    defaultValue: 'http://127.0.0.1:9091',
  );
}
