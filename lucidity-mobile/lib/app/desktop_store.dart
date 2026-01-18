import 'dart:convert';

import 'package:shared_preferences/shared_preferences.dart';

import 'desktop_profile.dart';

class DesktopStore {
  static const _kDesktopsKey = 'lucidity.desktops.v1';
  static const _kLastDesktopIdKey = 'lucidity.last_desktop_id.v1';

  static Future<List<DesktopProfile>> loadDesktops() async {
    final prefs = await SharedPreferences.getInstance();
    final raw = prefs.getString(_kDesktopsKey);
    if (raw == null || raw.isEmpty) return const <DesktopProfile>[];

    final decoded = jsonDecode(raw);
    if (decoded is! List) return const <DesktopProfile>[];

    final out = <DesktopProfile>[];
    for (final item in decoded) {
      if (item is Map) {
        out.add(DesktopProfile.fromJson(Map<String, Object?>.from(item)));
      }
    }
    return out;
  }

  static Future<void> saveDesktops(List<DesktopProfile> desktops) async {
    final prefs = await SharedPreferences.getInstance();
    final payload = desktops.map((d) => d.toJson()).toList(growable: false);
    await prefs.setString(_kDesktopsKey, jsonEncode(payload));
  }

  static Future<String?> loadLastDesktopId() async {
    final prefs = await SharedPreferences.getInstance();
    return prefs.getString(_kLastDesktopIdKey);
  }

  static Future<void> saveLastDesktopId(String? id) async {
    final prefs = await SharedPreferences.getInstance();
    if (id == null || id.isEmpty) {
      await prefs.remove(_kLastDesktopIdKey);
      return;
    }
    await prefs.setString(_kLastDesktopIdKey, id);
  }
}

