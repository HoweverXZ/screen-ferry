// 侧栏图标：圆角、扁平的双色图形。主体用实色，衬底用同一颜色的浅色。
// 都画在 24×24 的网格上，按需要的尺寸缩放。

import 'package:flutter/material.dart';

import '../theme/tokens.dart';

enum Glyph { sidebar, dashboard, devices, layout, log, settings }

/// 换颜色时（比如选中）平滑过渡。
class GlyphIcon extends StatelessWidget {
  const GlyphIcon(this.glyph, {super.key, required this.color, this.size = 22});

  final Glyph glyph;
  final Color color;
  final double size;

  @override
  Widget build(BuildContext context) {
    return TweenAnimationBuilder<Color?>(
      tween: ColorTween(end: color),
      duration: Motion.fast,
      curve: Motion.ease,
      builder: (context, value, _) =>
          CustomPaint(size: Size.square(size), painter: _GlyphPainter(glyph, value ?? color)),
    );
  }
}

class _GlyphPainter extends CustomPainter {
  _GlyphPainter(this.glyph, this.color);

  final Glyph glyph;
  final Color color;

  /// 实色图形周围留出的缝，让叠在一起的两块分得开。
  static const _gap = 1.3;

  static RRect _box(double l, double t, double r, double b, double radius) =>
      RRect.fromLTRBR(l, t, r, b, Radius.circular(radius));

  static Path _cut(Path base, Path hole) => Path.combine(PathOperation.difference, base, hole);

  @override
  void paint(Canvas canvas, Size size) {
    canvas.scale(size.width / 24);
    final solid = Paint()..color = color;
    final soft = Paint()..color = color.withValues(alpha: color.a * 0.3);

    switch (glyph) {
      case Glyph.sidebar:
        // 窗口作衬底，里面靠左一条实色的侧栏。
        canvas.drawRRect(_box(2.5, 4, 21.5, 20, 4.5), soft);
        canvas.drawRRect(_box(5.5, 7, 10.5, 17, 1.8), solid);

      case Glyph.dashboard:
        // 四个小组件：左上实色，其余浅色，右上是圆形。
        canvas.drawRRect(_box(3, 3, 11, 11, 3), solid);
        canvas.drawCircle(const Offset(17, 7), 4, soft);
        canvas.drawRRect(_box(3, 13, 11, 21, 3), soft);
        canvas.drawRRect(_box(13, 13, 21, 21, 3), soft);

      case Glyph.devices:
        // 显示器作衬底，手机用实色叠在右下。
        final phone = _box(14, 8.5, 22, 20.5, 2.5);
        final monitor = Path()
          ..addRRect(_box(2, 4, 18, 15.5, 3))
          ..addRRect(_box(6, 17.5, 14, 19.5, 1));
        canvas.drawPath(_cut(monitor, Path()..addRRect(phone.inflate(_gap))), soft);
        canvas.drawRRect(phone, solid);

      case Glyph.layout:
        // 两块屏幕并排：左边小的是本机，右边大的是另一台电脑。
        canvas.drawRRect(_box(2, 8, 10.5, 16, 2.5), soft);
        canvas.drawRRect(_box(12.5, 5.5, 22, 18.5, 3), solid);

      case Glyph.log:
        // 一张纸，上面三行字。
        canvas.drawRRect(_box(4, 2.5, 20, 21.5, 4.5), soft);
        canvas.drawRRect(_box(8, 7.4, 16, 9.6, 1.1), solid);
        canvas.drawRRect(_box(8, 10.9, 16, 13.1, 1.1), solid);
        canvas.drawRRect(_box(8, 14.4, 12.5, 16.6, 1.1), solid);

      case Glyph.settings:
        // 两条滑杆：轨道浅色，圆钮实色，钮的四周留缝。
        const knobs = [Offset(13.5, 8), Offset(10.5, 16)];
        const knob = 3.0;
        final tracks = Path()
          ..addRRect(_box(3, 6.7, 21, 9.3, 1.3))
          ..addRRect(_box(3, 14.7, 21, 17.3, 1.3));
        final holes = Path();
        for (final k in knobs) {
          holes.addOval(Rect.fromCircle(center: k, radius: knob + _gap));
        }
        canvas.drawPath(_cut(tracks, holes), soft);
        for (final k in knobs) {
          canvas.drawCircle(k, knob, solid);
        }
    }
  }

  @override
  bool shouldRepaint(_GlyphPainter old) => old.glyph != glyph || old.color != color;
}
