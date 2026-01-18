import 'package:flutter/material.dart';
import 'package:google_fonts/google_fonts.dart';
import 'package:provider/provider.dart';

import 'app/app_state.dart';
import 'app/auth_state.dart';
import 'screens/root_screen.dart';

void main() {
  WidgetsFlutterBinding.ensureInitialized();

  runApp(
    MultiProvider(
      providers: [
        ChangeNotifierProvider(create: (_) => AppState()),
        ChangeNotifierProvider(create: (_) => AuthState()),
      ],
      child: const LucidityApp(),
    ),
  );
}

class LucidityApp extends StatelessWidget {
  const LucidityApp({super.key});

  @override
  Widget build(BuildContext context) {
    final base = ThemeData.dark(useMaterial3: true);
    return MaterialApp(
      title: 'Lucidity',
      theme: base.copyWith(
        textTheme: GoogleFonts.interTextTheme(base.textTheme),
        scaffoldBackgroundColor: const Color(0xFF0E0F12),
        colorScheme: base.colorScheme.copyWith(
          surface: const Color(0xFF14161B),
          primary: const Color(0xFF7AA2F7),
        ),
      ),
      home: const RootScreen(),
    );
  }
}
