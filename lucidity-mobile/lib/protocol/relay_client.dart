import 'dart:async';
import 'dart:typed_data';

import 'package:flutter/foundation.dart';
import 'package:web_socket_channel/web_socket_channel.dart';

/// WebSocket-based relay client for connecting through the Lucidity relay server.
/// 
/// Used as a fallback when direct P2P connections fail (symmetric NAT, corporate firewalls).
class RelayClient extends ChangeNotifier {
  final String relayUrl;
  final String relayId;
  final String? relaySecret;
  
  WebSocketChannel? _channel;
  RelayStatus _status = RelayStatus.disconnected;
  String? _errorMessage;
  
  final StreamController<Uint8List> _dataController = StreamController<Uint8List>.broadcast();
  
  RelayClient({
    required this.relayUrl,
    required this.relayId,
    this.relaySecret,
  });
  
  /// Current connection status
  RelayStatus get status => _status;
  
  /// Error message if status is error
  String? get errorMessage => _errorMessage;
  
  /// Whether connected to relay
  bool get isConnected => _status == RelayStatus.connected;
  
  /// Stream of incoming data from relay
  Stream<Uint8List> get dataStream => _dataController.stream;
  
  /// Connect to the relay server
  Future<void> connect() async {
    if (_status == RelayStatus.connecting || _status == RelayStatus.connected) {
      return;
    }
    
    _updateStatus(RelayStatus.connecting);
    
    try {
      final wsUrl = _buildWebSocketUrl();
      debugPrint('[RelayClient] Connecting to relay: $wsUrl');
      
      final channel = WebSocketChannel.connect(Uri.parse(wsUrl));
      
      // Wait for connection to be ready (optional, but good for verification)
      await channel.ready;
      
      _channel = channel;
      _updateStatus(RelayStatus.connected);
      debugPrint('[RelayClient] Connected to relay: $relayId');
      
      // Listen for incoming messages
      _listenToMessages();
      
    } catch (e) {
      _errorMessage = e.toString();
      _updateStatus(RelayStatus.error);
      debugPrint('[RelayClient] Connection failed: $e');
      rethrow;
    }
  }
  
  /// Build the WebSocket URL for mobile client connection
  String _buildWebSocketUrl() {
    // Ensure we're using ws:// or wss:// scheme
    String wsUrl = relayUrl;
    if (wsUrl.startsWith('http://')) {
      wsUrl = 'ws://${wsUrl.substring(7)}';
    } else if (wsUrl.startsWith('https://')) {
      wsUrl = 'wss://${wsUrl.substring(8)}';
    } else if (!wsUrl.startsWith('ws://') && !wsUrl.startsWith('wss://')) {
      wsUrl = 'ws://$wsUrl';
    }
    
    var url = '$wsUrl/mobile/$relayId';
    if (relaySecret != null && relaySecret!.isNotEmpty) {
      url = '$url?secret=$relaySecret';
    }
    return url;
  }
  
  /// Listen for incoming WebSocket messages
  void _listenToMessages() {
    final channel = _channel;
    if (channel == null) return;
    
    channel.stream.listen(
      (dynamic data) {
        if (data is Uint8List) {
          _dataController.add(data);
        } else if (data is List<int>) {
          _dataController.add(Uint8List.fromList(data));
        } else if (data is String) {
          debugPrint('[RelayClient] Received text message: $data');
          // Ignored for now, protocol is binary
        }
      },
      onError: (Object error) {
        debugPrint('[RelayClient] WebSocket error: $error');
        _errorMessage = error.toString();
        _updateStatus(RelayStatus.error);
        disconnect();
      },
      onDone: () {
        debugPrint('[RelayClient] WebSocket closed');
        if (_status != RelayStatus.disconnected) {
          _updateStatus(RelayStatus.disconnected);
        }
      },
      cancelOnError: true,
    );
  }
  
  /// Send binary data to the relay
  void send(Uint8List data) {
    if (!isConnected || _channel == null) {
      debugPrint('[RelayClient] Not connected, cannot send');
      return;
    }
    
    try {
      _channel!.sink.add(data);
    } catch (e) {
      debugPrint('[RelayClient] Send error: $e');
      _errorMessage = e.toString();
      _updateStatus(RelayStatus.error);
    }
  }
  
  /// Send a text message to the relay (for control messages if needed)
  void sendText(String text) {
    if (!isConnected || _channel == null) {
      debugPrint('[RelayClient] Not connected, cannot send text');
      return;
    }
    
    try {
      _channel!.sink.add(text);
    } catch (e) {
      debugPrint('[RelayClient] Send text error: $e');
    }
  }
  
  /// Disconnect from the relay
  Future<void> disconnect() async {
    final channel = _channel;
    if (channel != null) {
      try {
        await channel.sink.close();
      } catch (e) {
        debugPrint('[RelayClient] Disconnect error: $e');
      }
      _channel = null;
    }
    
    _updateStatus(RelayStatus.disconnected);
    debugPrint('[RelayClient] Disconnected');
  }
  
  /// Update status and notify listeners
  void _updateStatus(RelayStatus newStatus) {
    if (_status != newStatus) {
      _status = newStatus;
      notifyListeners();
    }
  }
  
  @override
  void dispose() {
    disconnect();
    _dataController.close();
    super.dispose();
  }
}

/// Relay connection status
enum RelayStatus {
  disconnected,
  connecting,
  connected,
  error,
}

/// Extension methods for RelayStatus
extension RelayStatusExtension on RelayStatus {
  String get label {
    switch (this) {
      case RelayStatus.disconnected:
        return 'Disconnected';
      case RelayStatus.connecting:
        return 'Connecting...';
      case RelayStatus.connected:
        return 'Connected (Relay)';
      case RelayStatus.error:
        return 'Error';
    }
  }
  
  bool get isHealthy => this == RelayStatus.connected;
}
