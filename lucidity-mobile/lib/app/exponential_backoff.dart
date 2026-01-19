class ExponentialBackoff {
  final Duration _initialDelay;
  final Duration _maxDelay;
  final double _multiplier;
  int _attempts = 0;
  
  ExponentialBackoff({
    Duration initialTo = const Duration(seconds: 1),
    Duration maxDelay = const Duration(seconds: 30),
    double multiplier = 1.5,
  }) : _initialDelay = initialTo,
       _maxDelay = maxDelay,
       _multiplier = multiplier;
       
  Duration get nextDelay {
    _attempts++;
    
    // Calculate delay: initial * multiplier^(attempts-1)
    if (_attempts == 1) return _initialDelay;
    
    double ms = _initialDelay.inMilliseconds.toDouble();
    for (int i = 1; i < _attempts; i++) {
        ms *= _multiplier;
        if (ms > _maxDelay.inMilliseconds) {
          ms = _maxDelay.inMilliseconds.toDouble();
          break;
        }
    }
    
    return Duration(milliseconds: ms.toInt());
  }
  
  void reset() {
    _attempts = 0;
  }
  
  int get attempts => _attempts;
}
