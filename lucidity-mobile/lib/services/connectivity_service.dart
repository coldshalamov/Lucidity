import 'dart:async';
import 'package:connectivity_plus/connectivity_plus.dart';

/// Monitors network connectivity and triggers reconnection attempts.
class ConnectivityService {
  final Connectivity _connectivity = Connectivity();
  StreamSubscription<List<ConnectivityResult>>? _subscription;
  
  final List<void Function(bool isOnline)> _listeners = [];
  
  bool _isOnline = true;
  bool get isOnline => _isOnline;
  
  /// Start monitoring connectivity changes.
  void startMonitoring() {
    _subscription?.cancel();
    _subscription = _connectivity.onConnectivityChanged.listen(_handleChange);
    // Check initial state
    _connectivity.checkConnectivity().then(_handleChange);
  }
  
  void _handleChange(List<ConnectivityResult> results) {
    final wasOnline = _isOnline;
    _isOnline = results.isNotEmpty && !results.contains(ConnectivityResult.none);
    
    if (wasOnline != _isOnline) {
      for (final listener in _listeners) {
        listener(_isOnline);
      }
    }
  }
  
  /// Register a callback for when connectivity changes.
  void addListener(void Function(bool isOnline) listener) {
    _listeners.add(listener);
  }
  
  void removeListener(void Function(bool isOnline) listener) {
    _listeners.remove(listener);
  }
  
  void dispose() {
    _subscription?.cancel();
    _subscription = null;
    _listeners.clear();
  }
}

/// Utility for exponential backoff retries.
class ExponentialBackoff {
  final Duration initialDelay;
  final Duration maxDelay;
  final double multiplier;
  
  Duration _currentDelay;
  int _attempts = 0;
  
  ExponentialBackoff({
    this.initialDelay = const Duration(seconds: 1),
    this.maxDelay = const Duration(seconds: 60),
    this.multiplier = 2.0,
  }) : _currentDelay = initialDelay;
  
  Duration get nextDelay {
    final delay = _currentDelay;
    _attempts++;
    _currentDelay = Duration(
      milliseconds: (_currentDelay.inMilliseconds * multiplier).round(),
    );
    if (_currentDelay > maxDelay) {
      _currentDelay = maxDelay;
    }
    return delay;
  }
  
  int get attempts => _attempts;
  
  void reset() {
    _currentDelay = initialDelay;
    _attempts = 0;
  }
}
