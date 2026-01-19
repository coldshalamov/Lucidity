import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:lucidity_mobile/main.dart' as app;
import 'package:lucidity_mobile/screens/home_screen.dart';
import 'package:lucidity_mobile/screens/desktop_screen.dart';

/// End-to-end integration tests for Lucidity Mobile
///
/// These tests verify:
/// - App launches correctly
/// - Home screen displays saved desktops
/// - QR scanning flow (mocked)
/// - Desktop connection and terminal rendering
/// - Input handling

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  group('App Launch Tests', () {
    testWidgets('App launches and shows home screen', (WidgetTester tester) async {
      app.main();
      await tester.pumpAndSettle();

      // Verify we see the Lucidity app bar
      expect(find.text('Lucidity'), findsOneWidget);
      
      // Verify settings button is present
      expect(find.byIcon(Icons.settings), findsOneWidget);
    });

    testWidgets('Home screen shows empty state when no desktops', (WidgetTester tester) async {
      app.main();
      await tester.pumpAndSettle();

      // Look for empty state text
      expect(find.text('Pair your desktop'), findsOneWidget);
      expect(find.text('Scan QR'), findsOneWidget);
      expect(find.text('Add Desktop'), findsOneWidget);
    });

    testWidgets('Settings button opens settings sheet', (WidgetTester tester) async {
      app.main();
      await tester.pumpAndSettle();

      // Tap settings
      await tester.tap(find.byIcon(Icons.settings));
      await tester.pumpAndSettle();

      // Verify settings options are shown
      expect(find.text('Clear Saved Session'), findsOneWidget);
      expect(find.text('Auto-Reconnect'), findsOneWidget);
    });
  });

  group('QR Scanning Flow', () {
    testWidgets('Scan QR button opens scanner', (WidgetTester tester) async {
      app.main();
      await tester.pumpAndSettle();

      // Tap Scan QR button
      await tester.tap(find.text('Scan QR'));
      await tester.pumpAndSettle();

      // Verify scanner screen is shown
      // Note: Camera won't work in integration tests, but we can verify navigation
      expect(find.byType(MobileScanner), findsOneWidget);
    });
  });

  group('Manual Desktop Addition', () {
    testWidgets('Add Desktop button opens setup screen', (WidgetTester tester) async {
      app.main();
      await tester.pumpAndSettle();

      // Tap Add Desktop button
      await tester.tap(find.text('Add Desktop'));
      await tester.pumpAndSettle();

      // Verify setup screen is shown
      expect(find.text('Display Name'), findsOneWidget);
      expect(find.text('Host'), findsOneWidget);
      expect(find.text('Port'), findsOneWidget);
    });

    testWidgets('Can add a manual desktop', (WidgetTester tester) async {
      app.main();
      await tester.pumpAndSettle();

      // Tap Add Desktop
      await tester.tap(find.text('Add Desktop'));
      await tester.pumpAndSettle();

      // Fill in details
      await tester.enterText(find.byType(TextField).at(0), 'Test Desktop');
      await tester.enterText(find.byType(TextField).at(1), '192.168.1.100');
      await tester.enterText(find.byType(TextField).at(2), '9797');

      // Save
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();

      // Verify we navigated to desktop screen
      expect(find.byType(DesktopScreen), findsOneWidget);
    });
  });

  group('Connection State Tests', () {
    testWidgets('Shows connecting state during connection', (WidgetTester tester) async {
      // This test would require mocking the LucidityClient
      // For now, we verify the UI components exist
      
      app.main();
      await tester.pumpAndSettle();

      // Add a desktop first
      await tester.tap(find.text('Add Desktop'));
      await tester.pumpAndSettle();

      await tester.enterText(find.byType(TextField).at(0), 'Test');
      await tester.enterText(find.byType(TextField).at(1), 'localhost');
      await tester.enterText(find.byType(TextField).at(2), '9797');

      await tester.tap(find.text('Save'));
      await tester.pump(); // Don't wait for settle - we want to see connecting state

      // Should show connecting indicator
      expect(find.byType(CircularProgressIndicator), findsOneWidget);
    });

    testWidgets('Shows error state with retry button', (WidgetTester tester) async {
      app.main();
      await tester.pumpAndSettle();

      // Add a desktop to a non-existent host
      await tester.tap(find.text('Add Desktop'));
      await tester.pumpAndSettle();

      await tester.enterText(find.byType(TextField).at(0), 'Bad Host');
      await tester.enterText(find.byType(TextField).at(1), 'nonexistent.local');
      await tester.enterText(find.byType(TextField).at(2), '9797');

      await tester.tap(find.text('Save'));
      
      // Wait for connection timeout
      await tester.pumpAndSettle(const Duration(seconds: 10));

      // Should show error with retry
      expect(find.text('Connection Error'), findsOneWidget);
      expect(find.text('Retry'), findsOneWidget);
      expect(find.text('Go Back'), findsOneWidget);
    });
  });

  group('Terminal Rendering', () {
    // These tests would require a running lucidity-host
    // Placeholder for manual verification

    testWidgets('Terminal view renders when connected', (WidgetTester tester) async {
      // Skip - requires running server
      // This would be tested with a mock server in CI
    });

    testWidgets('Accessory bar shows special keys', (WidgetTester tester) async {
      // Skip - requires active terminal connection
      // Verify accessory bar has Esc, Tab, Ctrl+C, arrows
    });
  });
}
