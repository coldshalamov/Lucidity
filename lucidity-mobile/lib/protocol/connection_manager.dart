import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';

import '../app/desktop_profile.dart';

/// Connection type indicating how we're connected to the desktop
enum ConnectionType {
  /// Direct LAN connection (same network)
  lan,
  /// Direct internet connection via UPnP/STUN
  external,
  /// Connection via relay server
  relay,
}

/// Result of a successful connection attempt
class ConnectionResult {
  final ConnectionType type;
  final String address;
  final DateTime connectedAt;
  
  const ConnectionResult({
    required this.type,
    required this.address,
    required this.connectedAt,
  });
  
  String get typeLabel {
    switch (type) {
      case ConnectionType.lan:
        return 'LAN';
      case ConnectionType.external:
        return 'Direct';
      case ConnectionType.relay:
        return 'Relay';
    }
  }
}

/// Connection attempt result
sealed class ConnectionAttempt {}

class ConnectionSuccess extends ConnectionAttempt {
  final ConnectionResult result;
  final Socket? socket;
  final dynamic webSocket; // WebSocketChannel for relay
  
  ConnectionSuccess({
    required this.result,
    this.socket,
    this.webSocket,
  });
}

class ConnectionFailure extends ConnectionAttempt {
  final ConnectionType attemptedType;
  final String address;
  final String error;
  
  ConnectionFailure({
    required this.attemptedType,
    required this.address,
    required this.error,
  });
}

/// Manages connection strategy with cascade fallback:
/// 1. LAN Direct → fastest, same network
/// 2. External (UPnP/STUN) → direct over internet
/// 3. Relay → when P2P fails (symmetric NAT, firewalls)
class ConnectionManager extends ChangeNotifier {
  final DesktopProfile profile;
  
  ConnectionType? _currentType;
  String? _currentAddress;
  bool _isConnecting = false;
  String? _statusMessage;
  final List<ConnectionFailure> _failedAttempts = [];
  
  /// Timeout for each connection attempt
  final Duration attemptTimeout;
  
  ConnectionManager({
    required this.profile,
    this.attemptTimeout = const Duration(seconds: 5),
  });
  
  /// Current connection type (null if not connected)
  ConnectionType? get currentType => _currentType;
  
  /// Current address we're connected to
  String? get currentAddress => _currentAddress;
  
  /// Whether currently attempting to connect
  bool get isConnecting => _isConnecting;
  
  /// Status message for UI
  String? get statusMessage => _statusMessage;
  
  /// List of failed attempts from last connection cascade
  List<ConnectionFailure> get failedAttempts => List.unmodifiable(_failedAttempts);
  
  /// Attempt to connect using cascade strategy
  /// Returns the successful connection result or throws if all methods fail
  Future<ConnectionSuccess> connectWithCascade() async {
    _isConnecting = true;
    _failedAttempts.clear();
    notifyListeners();
    
    try {
      // Strategy 1: Try LAN direct (fastest, ~1ms latency)
      if (profile.lanAddr != null && profile.lanAddr!.isNotEmpty) {
        _updateStatus('Trying LAN connection...');
        
        final result = await _tryDirectConnection(
          profile.lanAddr!,
          ConnectionType.lan,
        );
        
        if (result is ConnectionSuccess) {
          _setConnected(result.result);
          return result;
        } else if (result is ConnectionFailure) {
          _failedAttempts.add(result);
          debugPrint('[ConnectionManager] LAN failed: ${result.error}');
        }
      }
      
      // Strategy 2: Try external (UPnP/STUN) for remote P2P
      if (profile.externalAddr != null && profile.externalAddr!.isNotEmpty) {
        _updateStatus('Trying direct connection...');
        
        final result = await _tryDirectConnection(
          profile.externalAddr!,
          ConnectionType.external,
        );
        
        if (result is ConnectionSuccess) {
          _setConnected(result.result);
          return result;
        } else if (result is ConnectionFailure) {
          _failedAttempts.add(result);
          debugPrint('[ConnectionManager] External failed: ${result.error}');
        }
      }
      
      // Strategy 3: Try relay (fallback when P2P fails)
      if (profile.supportsRelay) {
        _updateStatus('Connecting via relay...');
        
        final result = await _tryRelayConnection();
        
        if (result is ConnectionSuccess) {
          _setConnected(result.result);
          return result;
        } else if (result is ConnectionFailure) {
          _failedAttempts.add(result);
          debugPrint('[ConnectionManager] Relay failed: ${result.error}');
        }
      }
      
      // All strategies failed
      _updateStatus('All connection methods failed');
      throw ConnectionException(
        'Could not connect to desktop',
        failedAttempts: _failedAttempts,
      );
      
    } finally {
      _isConnecting = false;
      notifyListeners();
    }
  }
  
  /// Try a direct TCP connection
  Future<ConnectionAttempt> _tryDirectConnection(
    String address,
    ConnectionType type,
  ) async {
    try {
      final parts = address.split(':');
      if (parts.length != 2) {
        return ConnectionFailure(
          attemptedType: type,
          address: address,
          error: 'Invalid address format',
        );
      }
      
      final host = parts[0];
      final port = int.tryParse(parts[1]) ?? 9797;
      
      final socket = await Socket.connect(
        host,
        port,
        timeout: attemptTimeout,
      );
      socket.setOption(SocketOption.tcpNoDelay, true);
      
      return ConnectionSuccess(
        result: ConnectionResult(
          type: type,
          address: address,
          connectedAt: DateTime.now(),
        ),
        socket: socket,
      );
      
    } on SocketException catch (e) {
      return ConnectionFailure(
        attemptedType: type,
        address: address,
        error: 'Socket error: ${e.message}',
      );
    } on TimeoutException {
      return ConnectionFailure(
        attemptedType: type,
        address: address,
        error: 'Connection timed out',
      );
    } catch (e) {
      return ConnectionFailure(
        attemptedType: type,
        address: address,
        error: e.toString(),
      );
    }
  }
  
  /// Try connecting via relay server
  Future<ConnectionAttempt> _tryRelayConnection() async {
    final relayUrl = profile.relayUrl;
    final relayId = profile.relayId;
    
    if (relayUrl == null || relayId == null) {
      return ConnectionFailure(
        attemptedType: ConnectionType.relay,
        address: 'N/A',
        error: 'Relay not configured',
      );
    }
    
    try {
      // Build WebSocket URL
      String wsUrl = relayUrl;
      if (wsUrl.startsWith('http://')) {
        wsUrl = 'ws://${wsUrl.substring(7)}';
      } else if (wsUrl.startsWith('https://')) {
        wsUrl = 'wss://${wsUrl.substring(8)}';
      } else if (!wsUrl.startsWith('ws://') && !wsUrl.startsWith('wss://')) {
        wsUrl = 'ws://$wsUrl';
      }
      
      final fullUrl = '$wsUrl/mobile/$relayId';
      debugPrint('[ConnectionManager] Connecting to relay: $fullUrl');
      
      // TODO: Implement actual WebSocket connection
      // For now, return failure until web_socket_channel is integrated
      return ConnectionFailure(
        attemptedType: ConnectionType.relay,
        address: fullUrl,
        error: 'Relay WebSocket not yet implemented',
      );
      
    } catch (e) {
      return ConnectionFailure(
        attemptedType: ConnectionType.relay,
        address: '$relayUrl/mobile/$relayId',
        error: e.toString(),
      );
    }
  }
  
  void _updateStatus(String message) {
    _statusMessage = message;
    notifyListeners();
  }
  
  void _setConnected(ConnectionResult result) {
    _currentType = result.type;
    _currentAddress = result.address;
    _statusMessage = 'Connected via ${result.typeLabel}';
    notifyListeners();
  }
  
  /// Reset connection state
  void reset() {
    _currentType = null;
    _currentAddress = null;
    _isConnecting = false;
    _statusMessage = null;
    _failedAttempts.clear();
    notifyListeners();
  }
}

/// Exception thrown when all connection methods fail
class ConnectionException implements Exception {
  final String message;
  final List<ConnectionFailure> failedAttempts;
  
  const ConnectionException(
    this.message, {
    this.failedAttempts = const [],
  });
  
  @override
  String toString() {
    if (failedAttempts.isEmpty) {
      return 'ConnectionException: $message';
    }
    
    final attempts = failedAttempts
        .map((f) => '  - ${f.attemptedType.name}: ${f.error}')
        .join('\n');
    return 'ConnectionException: $message\nAttempts:\n$attempts';
  }
  
  /// Get a user-friendly error message
  String get userMessage {
    if (failedAttempts.isEmpty) {
      return message;
    }
    
    // Check if all attempts timed out (likely offline)
    final allTimeout = failedAttempts.every(
      (f) => f.error.contains('timed out') || f.error.contains('timeout'),
    );
    if (allTimeout) {
      return 'Desktop appears to be offline or unreachable';
    }
    
    // Check if connection refused (desktop not running)
    final anyRefused = failedAttempts.any(
      (f) => f.error.contains('refused') || f.error.contains('ECONNREFUSED'),
    );
    if (anyRefused) {
      return 'Lucidity is not running on the desktop';
    }
    
    return message;
  }
}
