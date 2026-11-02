// 设计令牌：浅色、深色两套颜色，以及动效曲线和连续曲率圆角。

import 'package:flutter/material.dart';

/// 界面用到的全部颜色。组件只从这里取色，不写死颜色值。
@immutable
class SfColors extends ThemeExtension<SfColors> {
  const SfColors({
    required this.bg,
    required this.side,
    required this.card,
    required this.card2,
    required this.card3,
    required this.navPill,
    required this.navHover,
    required this.closeHover,
    required this.fg,
    required this.muted,
    required this.line,
    required this.accent,
    required this.onAccent,
    required this.accentSoft,
    required this.goodBg,
    required this.goodInk,
    required this.danger,
    required this.track,
    required this.knobOff,
    required this.bloom,
    required this.shadow,
  });

  final Color bg;
  final Color side;
  final Color card;
  final Color card2;
  final Color card3;

  /// 侧栏：选中项的胶囊、悬停时的底色。两者要和侧栏底色拉开差距，悬停比选中浅。
  final Color navPill;
  final Color navHover;

  /// Windows 窗口“关闭”按钮悬停时的红色。
  final Color closeHover;
  final Color fg;
  final Color muted;
  final Color line;
  final Color accent;
  final Color onAccent;
  final Color accentSoft;
  final Color goodBg;
  final Color goodInk;
  final Color danger;
  final Color track;
  final Color knobOff;

  /// 花朵在三种淡色之间缓慢循环。
  final List<Color> bloom;
  final Color shadow;

  static const light = SfColors(
    bg: Color(0xFFEFEDF0),
    side: Color(0xFFE5E3E7),
    card: Color(0xFFFAF8FB),
    card2: Color(0xFFEEECF0),
    card3: Color(0xFFE1DFE4),
    navPill: Color(0xFFD2CFD7),
    navHover: Color(0xFFE0DDE3),
    closeHover: Color(0xFFE5484D),
    fg: Color(0xFF1C1B1F),
    muted: Color(0xFF625F67),
    line: Color(0xFFE2DFE4),
    accent: Color(0xFF0A6CFF),
    onAccent: Color(0xFFFFFFFF),
    accentSoft: Color(0xFFDCE8FF),
    goodBg: Color(0xFFA8EDA3),
    goodInk: Color(0xFF14561B),
    danger: Color(0xFFC62828),
    track: Color(0xFFDFDDE2),
    knobOff: Color(0xFF5D6067),
    bloom: [Color(0xFFD9E2FA), Color(0xFFD7EEE6), Color(0xFFECDFF3)],
    shadow: Color(0x14302448),
  );

  static const dark = SfColors(
    bg: Color(0xFF141316),
    side: Color(0xFF1B1A1E),
    card: Color(0xFF222025),
    card2: Color(0xFF2B2930),
    card3: Color(0xFF35333A),
    navPill: Color(0xFF34323B),
    navHover: Color(0xFF242328),
    closeHover: Color(0xFFE5484D),
    fg: Color(0xFFEEECF1),
    muted: Color(0xFFA9A5AF),
    line: Color(0xFF302E35),
    accent: Color(0xFF1F6FEB),
    onAccent: Color(0xFFFFFFFF),
    accentSoft: Color(0xFF1C2E52),
    goodBg: Color(0xFF1F4B24),
    goodInk: Color(0xFFA8EDA3),
    danger: Color(0xFFFF8A80),
    track: Color(0xFF3A383F),
    knobOff: Color(0xFFA19DA7),
    bloom: [Color(0xFF2B3352), Color(0xFF243A3A), Color(0xFF3A3049)],
    shadow: Color(0x4D000000),
  );

  static SfColors of(BuildContext context) => Theme.of(context).extension<SfColors>()!;

  /// 换一种强调色：浅底色由强调色和卡片色混合得到。
  SfColors withAccent(Color accent, {required bool dark}) =>
      copyWith(accent: accent, accentSoft: Color.lerp(card, accent, dark ? 0.30 : 0.16));

  @override
  SfColors copyWith({Color? accent, Color? accentSoft}) => SfColors(
    bg: bg,
    side: side,
    card: card,
    card2: card2,
    card3: card3,
    navPill: navPill,
    navHover: navHover,
    closeHover: closeHover,
    fg: fg,
    muted: muted,
    line: line,
    accent: accent ?? this.accent,
    onAccent: onAccent,
    accentSoft: accentSoft ?? this.accentSoft,
    goodBg: goodBg,
    goodInk: goodInk,
    danger: danger,
    track: track,
    knobOff: knobOff,
    bloom: bloom,
    shadow: shadow,
  );

  @override
  SfColors lerp(SfColors? other, double t) {
    if (other == null) return this;
    Color c(Color a, Color b) => Color.lerp(a, b, t)!;
    return SfColors(
      bg: c(bg, other.bg),
      side: c(side, other.side),
      card: c(card, other.card),
      card2: c(card2, other.card2),
      card3: c(card3, other.card3),
      navPill: c(navPill, other.navPill),
      navHover: c(navHover, other.navHover),
      closeHover: c(closeHover, other.closeHover),
      fg: c(fg, other.fg),
      muted: c(muted, other.muted),
      line: c(line, other.line),
      accent: c(accent, other.accent),
      onAccent: c(onAccent, other.onAccent),
      accentSoft: c(accentSoft, other.accentSoft),
      goodBg: c(goodBg, other.goodBg),
      goodInk: c(goodInk, other.goodInk),
      danger: c(danger, other.danger),
      track: c(track, other.track),
      knobOff: c(knobOff, other.knobOff),
      bloom: [for (var i = 0; i < bloom.length; i++) c(bloom[i], other.bloom[i])],
      shadow: c(shadow, other.shadow),
    );
  }
}

/// 可选的强调色：浅色模式与深色模式各一个值，白字在上面都清晰可读。
class AccentChoice {
  const AccentChoice(this.name, this.light, this.dark);

  /// 颜色的标识（`blue` 等），界面按它显示当前语言的名字。
  final String name;
  final Color light;
  final Color dark;

  static const all = [
    AccentChoice('blue', Color(0xFF0A6CFF), Color(0xFF1F6FEB)),
    AccentChoice('teal', Color(0xFF00796B), Color(0xFF00796B)),
    AccentChoice('purple', Color(0xFF6A4BD8), Color(0xFF6A4BD8)),
    AccentChoice('orange', Color(0xFFC2410C), Color(0xFFC2410C)),
    AccentChoice('pink', Color(0xFFC2185B), Color(0xFFC2185B)),
  ];
}

/// 动效：一般过渡用平滑减速，按钮、开关、指示块用略带回弹的弹簧。
abstract final class Motion {
  static const ease = Cubic(0.2, 0.8, 0.2, 1);
  static const spring = Cubic(0.34, 1.56, 0.64, 1);
  static const fast = Duration(milliseconds: 220);
  static const medium = Duration(milliseconds: 380);
  static const slow = Duration(milliseconds: 460);
}

/// 连续曲率圆角（类似 Apple 的圆角），比普通圆角过渡更柔和。
OutlinedBorder continuous(double radius, {BorderSide side = BorderSide.none}) =>
    RoundedSuperellipseBorder(borderRadius: BorderRadius.circular(radius), side: side);

/// 等宽字体：macOS 用 Menlo，Windows 上没有它，退到 Consolas。
const monoFont = 'Menlo';
const monoFallback = ['Consolas', 'Cascadia Mono'];
const tabular = [FontFeature.tabularFigures()];
