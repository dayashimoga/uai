import 'package:flutter/material.dart';

import 'core/assistant_controller.dart';
import 'core/ffi_bridge.dart';
import 'ui/adaptive_shell.dart';

void main() async {
  WidgetsFlutterBinding.ensureInitialized();

  // Initialize FFI Bridge with fallback for headless testing
  AissistantBridge bridge;
  try {
    final dylib = NativeFfiBridge.openLibrary();
    bridge = NativeFfiBridge(dylib);
  } catch (_) {
    // Fallback to test harness bridge if native binary is not yet placed in executable path
    bridge = TestHarnessBridge();
  }

  final controller = AssistantController(bridge);
  await controller.initialize();

  runApp(AissistantApp(controller: controller));
}

class AissistantApp extends StatelessWidget {
  final AssistantController controller;

  const AissistantApp({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) {
        ThemeMode themeMode;
        switch (controller.themeMode) {
          case 'light':
            themeMode = ThemeMode.light;
            break;
          case 'dark':
            themeMode = ThemeMode.dark;
            break;
          case 'system':
          default:
            themeMode = ThemeMode.system;
            break;
        }

        return MaterialApp(
          title: 'Universal AI Assistant',
          debugShowCheckedModeBanner: false,
          themeMode: themeMode,
          theme: ThemeData(
            useMaterial3: true,
            colorSchemeSeed: const Color(0xFF1A73E8),
            brightness: Brightness.light,
          ),
          darkTheme: ThemeData(
            useMaterial3: true,
            colorSchemeSeed: const Color(0xFF8AB4F8),
            brightness: Brightness.dark,
          ),
          home: AdaptiveShell(controller: controller),
        );
      },
    );
  }
}
