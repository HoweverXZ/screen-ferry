import 'package:flutter/material.dart';

import 'l10n/l10n.dart';
import 'shell/home_shell.dart';
import 'state/app_scope.dart';
import 'state/app_state.dart';
import 'theme/theme.dart';
import 'theme/tokens.dart';

class ScreenFerryApp extends StatelessWidget {
  const ScreenFerryApp({super.key, required this.state, this.windowFrame});

  final AppState state;

  /// 是否自己画窗口按钮；为 null 时按系统决定。测试里用来在 macOS 上检查 Windows 的外框。
  final bool? windowFrame;

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
            locale: state.locale,
            supportedLocales: AppLocalizations.supportedLocales,
            localizationsDelegates: AppLocalizations.localizationsDelegates,
            themeMode: state.appearance.mode,
            theme: buildTheme(SfColors.light.withAccent(accent.light, dark: false), Brightness.light),
            darkTheme: buildTheme(SfColors.dark.withAccent(accent.dark, dark: true), Brightness.dark),
            home: HomeShell(windowFrame: windowFrame),
          );
        },
      ),
    );
  }
}
