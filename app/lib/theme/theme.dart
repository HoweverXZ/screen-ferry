import 'dart:io';

import 'package:flutter/material.dart';

import 'tokens.dart';

ThemeData buildTheme(SfColors colors, Brightness brightness) {
  final scheme = ColorScheme.fromSeed(seedColor: colors.accent, brightness: brightness).copyWith(
    primary: colors.accent,
    onPrimary: colors.onAccent,
    surface: colors.bg,
    onSurface: colors.fg,
    onSurfaceVariant: colors.muted,
    outlineVariant: colors.line,
    error: colors.danger,
  );
  // 各用系统字体：macOS 默认就是苹方；Windows 指定微软雅黑，英文环境的系统里中文也不会落到日文字体上。
  final windows = Platform.isWindows;
  final base = ThemeData(
    useMaterial3: true,
    colorScheme: scheme,
    brightness: brightness,
    fontFamily: windows ? 'Microsoft YaHei UI' : null,
    fontFamilyFallback: windows ? const ['Microsoft YaHei', 'Segoe UI'] : null,
  );
  return base.copyWith(
    extensions: [colors],
    scaffoldBackgroundColor: colors.bg,
    splashFactory: NoSplash.splashFactory,
    highlightColor: Colors.transparent,
    textTheme: base.textTheme.apply(bodyColor: colors.fg, displayColor: colors.fg),
    iconTheme: IconThemeData(color: colors.fg, size: 20),
    sliderTheme: SliderThemeData(
      activeTrackColor: colors.accent,
      inactiveTrackColor: colors.track,
      thumbColor: colors.accent,
      overlayShape: SliderComponentShape.noOverlay,
      trackHeight: 4,
    ),
    tooltipTheme: TooltipThemeData(
      waitDuration: const Duration(milliseconds: 500),
      decoration: ShapeDecoration(color: colors.fg, shape: continuous(8)),
      textStyle: TextStyle(color: colors.card, fontSize: 12),
    ),
  );
}
