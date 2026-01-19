enum LucidityConnectionState {
  disconnected,
  connecting,
  connected,
  reconnecting,
  error;

  String get label {
    switch (this) {
      case LucidityConnectionState.disconnected: return 'Disconnected';
      case LucidityConnectionState.connecting: return 'Connecting...';
      case LucidityConnectionState.connected: return 'Connected';
      case LucidityConnectionState.reconnecting: return 'Reconnecting...';
      case LucidityConnectionState.error: return 'Connection Error';
    }
  }
}
