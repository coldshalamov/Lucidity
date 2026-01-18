import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../app/auth_state.dart';
import 'home_screen.dart';
import 'login_screen.dart';

class RootScreen extends StatelessWidget {
  const RootScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return Consumer<AuthState>(
      builder: (context, auth, _) {
        if (!auth.ready) {
          return const Scaffold(body: Center(child: CircularProgressIndicator()));
        }
        if (auth.token == null) {
          return const LoginScreen();
        }
        return const HomeScreen();
      },
    );
  }
}

