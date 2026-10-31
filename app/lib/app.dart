import 'package:flutter/material.dart';

import 'shell/home_shell.dart';
import 'state/app_scope.dart';
import 'state/app_state.dart';
import 'theme/theme.dart';
import 'theme/tokens.dart';

class ScreenFerryApp extends StatelessWidget {
  const ScreenFerryApp({super.key, required this.state});

  final AppState state;

  @override
  Widget build(BuildContext context) {
    return AppScope(
      state: state,
      child: ListenableBuilder(
        listenable: state.appearance,
        builder: (context, _) {
          final accent = AccentChoice.all[state.appearance.accent];
          return MaterialApp(
            title: 'ScreenFerry',
            debugShowCheckedModeBanner: false,
            themeMode: state.appearance.mode,
            theme: buildTheme(SfColors.light.withAccent(accent.light, dark: false), Brightness.light),
            darkTheme: buildTheme(SfColors.dark.withAccent(accent.dark, dark: true), Brightness.dark),
            home: const HomeShell(),
          );
        },
      ),
    );
  }
}
