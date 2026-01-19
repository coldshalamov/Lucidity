/// Parses various error types into user-friendly messages with recovery suggestions.
class LucidityErrorHandler {
  /// Returns a user-friendly message for the given error.
  static String getErrorMessage(Object error) {
    final errorStr = error.toString().toLowerCase();
    
    // Timeout errors
    if (errorStr.contains('timeout')) {
      return 'Connection timed out. The desktop may be slow to respond or unreachable.';
    }
    
    // Connection refused
    if (errorStr.contains('connection refused') || errorStr.contains('refused')) {
      return 'Could not reach desktop. Make sure WezTerm is running with Lucidity enabled.';
    }
    
    // DNS/Host errors
    if (errorStr.contains('no address associated') || 
        errorStr.contains('failed host lookup') ||
        errorStr.contains('getaddrinfo') ||
        errorStr.contains('name or service not known')) {
      return 'Could not find host. Check the address and your internet connection.';
    }
    
    // Relay specific errors
    if (errorStr.contains('404') || errorStr.contains('not found') || errorStr.contains('desktop_offline')) {
      return 'Desktop not found. It may be offline or not connected to the relay.';
    }
    
    if (errorStr.contains('401') || errorStr.contains('unauthorized')) {
      return 'Authentication failed. Your session may have expired.';
    }
    
    if (errorStr.contains('403') || errorStr.contains('forbidden')) {
      return 'Access denied. You may need to re-pair with this desktop.';
    }
    
    if (errorStr.contains('session_rejected') || errorStr.contains('rejected')) {
      return 'Connection was rejected. The desktop user may have denied the request.';
    }
    
    if (errorStr.contains('session_expired')) {
      return 'Your session has expired. Please reconnect.';
    }
    
    if (errorStr.contains('invalid_fingerprint') || errorStr.contains('fingerprint')) {
      return 'Security verification failed. The desktop identity could not be confirmed.';
    }
    
    if (errorStr.contains('relay') && errorStr.contains('closed')) {
      return 'Lost connection to the relay server.';
    }
    
    if (errorStr.contains('socket closed') || errorStr.contains('disconnected') || errorStr.contains('broken pipe')) {
      return 'Connection was interrupted unexpectedly.';
    }
    
    // WebSocket errors
    if (errorStr.contains('websocket') || errorStr.contains('upgrade failed')) {
      return 'Could not establish secure connection. The server may be unavailable.';
    }
    
    // SSL/TLS errors
    if (errorStr.contains('certificate') || errorStr.contains('ssl') || errorStr.contains('tls') || errorStr.contains('handshake')) {
      return 'Secure connection failed. There may be a certificate issue.';
    }
    
    // Generic network errors
    if (errorStr.contains('network') || errorStr.contains('unreachable') || errorStr.contains('no route')) {
      return 'Network error. Check your internet connection.';
    }
    
    // Rate limiting
    if (errorStr.contains('429') || errorStr.contains('too many') || errorStr.contains('rate limit')) {
      return 'Too many connection attempts. Please wait a moment and try again.';
    }
    
    // Server errors
    if (errorStr.contains('500') || errorStr.contains('internal server')) {
      return 'The relay server encountered an error. Please try again later.';
    }
    
    if (errorStr.contains('503') || errorStr.contains('unavailable')) {
      return 'The relay server is temporarily unavailable. Please try again in a few minutes.';
    }
    
    // Fallback
    return 'Connection error: ${_sanitizeError(error)}';
  }
  
  /// Returns a recovery suggestion for the given error.
  static String? getRecoverySuggestion(Object error) {
    final errorStr = error.toString().toLowerCase();
    
    if (errorStr.contains('timeout')) {
      return 'Check if your desktop is on and WezTerm is running. Try again in a few seconds.';
    }
    
    if (errorStr.contains('unreachable') || errorStr.contains('network') || errorStr.contains('no route')) {
      return 'Check your Wi-Fi or mobile data connection. Try switching networks.';
    }
    
    if (errorStr.contains('connection refused')) {
      return 'Open WezTerm on your desktop and press Ctrl+Shift+L to show the QR code.';
    }
    
    if (errorStr.contains('401') || errorStr.contains('403') || errorStr.contains('rejected') || errorStr.contains('unauthorized')) {
      return 'Remove this desktop from your list and scan the QR code again to re-pair.';
    }
    
    if (errorStr.contains('404') || errorStr.contains('desktop_offline')) {
      return 'Make sure WezTerm is running and connected to the internet on your desktop.';
    }
    
    if (errorStr.contains('fingerprint') || errorStr.contains('certificate')) {
      return 'The desktop may have been reinstalled. Re-pair by scanning a new QR code.';
    }
    
    if (errorStr.contains('429') || errorStr.contains('rate limit')) {
      return 'Wait 30 seconds before trying again.';
    }
    
    if (errorStr.contains('500') || errorStr.contains('503') || errorStr.contains('unavailable')) {
      return 'This is likely a temporary issue. Try again in a few minutes.';
    }
    
    if (errorStr.contains('socket closed') || errorStr.contains('broken pipe') || errorStr.contains('disconnected')) {
      return 'The connection was lost. Tap Retry to reconnect.';
    }
    
    return null;
  }
  
  /// Returns an icon name suggestion for the error type
  static String getErrorIconName(Object error) {
    final errorStr = error.toString().toLowerCase();
    
    if (errorStr.contains('network') || errorStr.contains('unreachable') || errorStr.contains('no route')) {
      return 'wifi_off';
    }
    if (errorStr.contains('401') || errorStr.contains('403') || errorStr.contains('unauthorized')) {
      return 'lock_outline';
    }
    if (errorStr.contains('404') || errorStr.contains('desktop_offline')) {
      return 'desktop_access_disabled';
    }
    if (errorStr.contains('timeout')) {
      return 'timer_off';
    }
    if (errorStr.contains('certificate') || errorStr.contains('ssl') || errorStr.contains('fingerprint')) {
      return 'gpp_bad';
    }
    return 'error_outline';
  }
  
  static String _sanitizeError(Object error) {
    // Remove stack traces and internal details
    var msg = error.toString();
    if (msg.contains('\n')) {
      msg = msg.split('\n').first;
    }
    // Remove common prefixes
    msg = msg.replaceAll('Exception: ', '').replaceAll('Error: ', '');
    // Limit length
    if (msg.length > 80) {
      msg = '${msg.substring(0, 80)}...';
    }
    return msg;
  }
}
