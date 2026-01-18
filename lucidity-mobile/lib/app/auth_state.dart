import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'app_config.dart';

class AuthState extends ChangeNotifier {
  static const _kTokenKey = 'lucidity.auth.token.v1';

  bool _ready = false;
  bool get ready => _ready;

  String? _token;
  String? get token => _token;

  String? _email;
  String? get email => _email;

  bool _subscriptionActive = false;
  bool get subscriptionActive => _subscriptionActive;

  AuthState() {
    _load();
  }

  Future<void> _load() async {
    final prefs = await SharedPreferences.getInstance();
    _token = prefs.getString(_kTokenKey);
    _ready = true;
    notifyListeners();

    if (_token != null) {
      await refreshMe();
    }
  }

  Future<void> logout() async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.remove(_kTokenKey);
    _token = null;
    _email = null;
    _subscriptionActive = false;
    notifyListeners();
  }

  Future<void> signup({required String email, required String password}) async {
    final resp = await _postJson(
      '${AppConfig.authBase}/v1/signup',
      {'email': email, 'password': password},
    );
    final token = resp['token'];
    if (token is! String || token.isEmpty) {
      throw StateError('signup did not return token');
    }
    await _setToken(token);
    await refreshMe();
  }

  Future<void> login({required String email, required String password}) async {
    final resp = await _postJson(
      '${AppConfig.authBase}/v1/login',
      {'email': email, 'password': password},
    );
    final token = resp['token'];
    if (token is! String || token.isEmpty) {
      throw StateError('login did not return token');
    }
    await _setToken(token);
    await refreshMe();
  }

  Future<void> refreshMe() async {
    final t = _token;
    if (t == null) return;

    final uri = Uri.parse('${AppConfig.authBase}/v1/me');
    final client = HttpClient();
    try {
      final req = await client.getUrl(uri);
      req.headers.set('Authorization', 'Bearer $t');
      req.headers.set('Accept', 'application/json');
      final resp = await req.close();
      final body = await resp.transform(utf8.decoder).join();
      if (resp.statusCode != 200) {
        throw StateError('me failed (${resp.statusCode}): $body');
      }
      final obj = jsonDecode(body);
      if (obj is! Map<String, dynamic>) throw StateError('invalid me response');
      final email = obj['email'];
      final active = obj['subscription_active'];
      if (email is String) _email = email;
      if (active is bool) _subscriptionActive = active;
      notifyListeners();
    } finally {
      client.close(force: true);
    }
  }

  Future<void> _setToken(String token) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_kTokenKey, token);
    _token = token;
    notifyListeners();
  }

  Future<Map<String, dynamic>> _postJson(String url, Map<String, Object?> payload) async {
    final uri = Uri.parse(url);
    final client = HttpClient();
    try {
      final req = await client.postUrl(uri);
      req.headers.contentType = ContentType.json;
      req.headers.set('Accept', 'application/json');
      req.add(utf8.encode(jsonEncode(payload)));
      final resp = await req.close();
      final body = await resp.transform(utf8.decoder).join();
      if (resp.statusCode < 200 || resp.statusCode >= 300) {
        throw StateError('request failed (${resp.statusCode}): $body');
      }
      final obj = jsonDecode(body);
      if (obj is! Map<String, dynamic>) throw StateError('invalid response');
      return obj;
    } finally {
      client.close(force: true);
    }
  }
}

