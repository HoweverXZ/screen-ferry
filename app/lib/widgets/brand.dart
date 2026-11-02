// 品牌图形：彩色的应用图标、单色的应用标识。形状与 assets/icons 里的源文件一致。

import 'dart:math';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';

import '../theme/tokens.dart';

/// 光标的七个顶点：尖端在原点，高 23。
const _pointer = [
  Offset(0, 0),
  Offset(14.8, 14.4),
  Offset(8.3, 14.4),
  Offset(11.4, 21.6),
  Offset(8.1, 23),
  Offset(4.9, 15.6),
  Offset(0, 20),
];

/// 把光标的顶点放到 [tip]、放大 [scale] 倍。
List<Offset> _placePointer(Offset tip, double scale) => [for (final p in _pointer) tip + p * scale];

/// 折线，中间的拐角倒成半径 [radius] 的圆角，首尾两点不倒。
Path _roundedPolyline(List<Offset> points, double radius) {
  final path = Path()..moveTo(points.first.dx, points.first.dy);
  for (var i = 1; i < points.length - 1; i++) {
    final (a, b, c) = (points[i - 1], points[i], points[i + 1]);
    final d1 = (a - b).distance;
    final d2 = (c - b).distance;
    final k = min(radius, min(d1, d2) / 2);
    final p1 = b + (a - b) * (k / d1);
    final p2 = b + (c - b) * (k / d2);
    path
      ..lineTo(p1.dx, p1.dy)
      ..quadraticBezierTo(b.dx, b.dy, p2.dx, p2.dy);
  }
  return path..lineTo(points.last.dx, points.last.dy);
}

/// 彩色的应用图标：一台显示器，左半刻在白瓷上，右半是实心的蓝色机身；
/// 白瓷和蓝色之间的分界线在屏幕中间拐出一个光标。坐标是 100 见方的格子。
class AppIconMark extends StatelessWidget {
  const AppIconMark({super.key, this.size = 64});

  final double size;

  @override
  Widget build(BuildContext context) => Semantics(
    image: true,
    label: 'ScreenFerry',
    child: CustomPaint(size: Size.square(size), painter: const _AppIconPainter()),
  );
}

class _AppIconPainter extends CustomPainter {
  const _AppIconPainter();

  static const _full = Rect.fromLTWH(0, 0, 100, 100);
  static final _body = RRect.fromLTRBR(12, 18, 88, 69, const Radius.circular(11));
  static final _screen = RRect.fromLTRBR(16.5, 22.5, 83.5, 64.5, const Radius.circular(7));
  static final _base = RRect.fromLTRBR(33, 77, 67, 82, const Radius.circular(2.5));
  static final _neck = Path()
    ..moveTo(45.5, 68)
    ..lineTo(54.5, 68)
    ..lineTo(56, 78)
    ..lineTo(44, 78)
    ..close();

  /// 分界线：x = 42 的竖线，到屏幕中间拐出光标再接着往下。
  static final _seam = _roundedPolyline([
    const Offset(42, -1),
    ..._placePointer(const Offset(42, 25.5), 1.6),
    const Offset(42, 101),
  ], 1.5);
  static final _left = Path.from(_seam)
    ..lineTo(-1, 101)
    ..lineTo(-1, -1)
    ..close();
  static final _right = Path.from(_seam)
    ..lineTo(101, 101)
    ..lineTo(101, -1)
    ..close();

  static Shader _linear(Rect r, double x2, Color from, Color to) =>
      ui.Gradient.linear(r.topLeft, Offset(r.left + r.width * x2, r.bottom), [from, to]);

  @override
  void paint(Canvas canvas, Size size) {
    canvas.scale(size.width / 100);
    final shape = continuous(22.5).getOuterPath(_full);
    // 和系统图标一样带一层软投影，放在白色卡片上也看得出白瓷那半的边。
    canvas.drawPath(
      shape.shift(const Offset(0, 1.4)),
      Paint()
        ..color = const Color(0x33000000)
        ..maskFilter = const MaskFilter.blur(BlurStyle.normal, 1.8),
    );
    canvas.save();
    canvas.clipPath(shape);

    // 白瓷
    canvas.drawRect(_full, Paint()..shader = _linear(_full, 0.35, const Color(0xFFFFFFFF), const Color(0xFFECEDF2)));

    // 左半：刻在瓷面上的显示器。下方一道白色反光，槽里左上内缘一道阴影。
    canvas.save();
    canvas.clipPath(_left);
    final groove = Path()
      ..addRRect(_body)
      ..moveTo(34.5, 79.5)
      ..lineTo(65.5, 79.5);
    Paint line(Color color) => Paint()
      ..color = color
      ..style = PaintingStyle.stroke
      ..strokeWidth = 3.6
      ..strokeCap = StrokeCap.round
      ..strokeJoin = StrokeJoin.round;
    canvas.drawPath(groove.shift(const Offset(0.4, 0.55)), line(const Color(0xFFFFFFFF)));
    canvas.saveLayer(_full, Paint());
    canvas.drawPath(groove, line(const Color(0xFFB4B6C6)));
    canvas.drawPath(
      groove.shift(const Offset(0.55, 0.75)),
      line(const Color(0xFFE0E1E8))..blendMode = BlendMode.srcATop,
    );
    canvas.restore();
    canvas.restore();

    // 蓝色那半比瓷面高一层，影子落在瓷面上，也落进光标里。
    canvas.drawPath(
      _right.shift(const Offset(-0.9, 0.9)),
      Paint()
        ..color = const Color(0x611D1F6B)
        ..maskFilter = const MaskFilter.blur(BlurStyle.normal, 1.2),
    );

    // 右半：实心的蓝色机身。
    canvas.save();
    canvas.clipPath(_right);
    canvas.drawRect(_full, Paint()..shader = _linear(_full, 0.3, const Color(0xFF5563EC), const Color(0xFF3236BC)));
    final stand = Path()
      ..addRRect(_body)
      ..addRRect(_base)
      ..addPath(_neck, Offset.zero);
    canvas.drawPath(
      stand.shift(const Offset(0.4, 1.8)),
      Paint()
        ..color = const Color(0x7314165A)
        ..maskFilter = const MaskFilter.blur(BlurStyle.normal, 1.6),
    );
    canvas.drawPath(_neck, Paint()..color = const Color(0xFF8EA6FB));
    final bodyPaint = Paint()..shader = _linear(_body.outerRect, 0.3, const Color(0xFFD3DEFF), const Color(0xFFA9BCFF));
    canvas.drawRRect(_base, bodyPaint);
    canvas.drawRRect(_body, bodyPaint);
    canvas.drawRRect(
      _screen,
      Paint()..shader = _linear(_screen.outerRect, 0.3, const Color(0xFF8FA7FF), const Color(0xFF6A84F6)),
    );
    canvas.drawLine(
      const Offset(22, 19.2),
      const Offset(78, 19.2),
      Paint()
        ..color = const Color(0xD9FFFFFF)
        ..strokeWidth = 0.8
        ..strokeCap = StrokeCap.round,
    );
    canvas.restore();
    canvas.restore();

    // 一圈很淡的描边，收住白瓷那半的边。
    canvas.drawPath(
      shape,
      Paint()
        ..color = const Color(0x14000000)
        ..style = PaintingStyle.stroke
        ..strokeWidth = 0.8,
    );
  }

  @override
  bool shouldRepaint(_AppIconPainter old) => false;
}

/// 单色的应用标识：一块屏幕，右下角一个光标，光标周围留一圈空隙。坐标是 24 见方的格子。
class FerryGlyph extends StatelessWidget {
  const FerryGlyph({super.key, this.size = 24, this.color});

  final double size;
  final Color? color;

  @override
  Widget build(BuildContext context) => CustomPaint(
    size: Size.square(size),
    painter: _FerryPainter(color ?? IconTheme.of(context).color ?? Colors.black),
  );
}

class _FerryPainter extends CustomPainter {
  _FerryPainter(this.color);

  final Color color;

  static const _scale = 0.43;
  static final _cursor = Path()..addPolygon(_placePointer(const Offset(15.4, 10.2), _scale), true);

  @override
  void paint(Canvas canvas, Size size) {
    canvas.scale(size.width / 24);
    canvas.saveLayer(Offset.zero & const Size.square(24), Paint());
    final line = Paint()
      ..color = color
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.6
      ..strokeCap = StrokeCap.round
      ..strokeJoin = StrokeJoin.round;
    canvas.drawRRect(RRect.fromLTRBR(2.6, 4.2, 19, 15.6, const Radius.circular(2.6)), line);
    canvas.drawPath(
      Path()
        ..moveTo(10.8, 15.6)
        ..lineTo(10.8, 18.8)
        ..moveTo(7.4, 19.2)
        ..lineTo(14.2, 19.2),
      line,
    );
    canvas.drawPath(
      _cursor,
      Paint()
        ..blendMode = BlendMode.clear
        ..strokeWidth = 8.5 * _scale
        ..strokeJoin = StrokeJoin.round
        ..style = PaintingStyle.stroke,
    );
    canvas.drawPath(_cursor, Paint()..blendMode = BlendMode.clear);
    canvas.restore();
    canvas.drawPath(
      _cursor,
      Paint()
        ..color = color
        ..strokeWidth = 2.4 * _scale
        ..strokeJoin = StrokeJoin.round
        ..style = PaintingStyle.stroke,
    );
    canvas.drawPath(_cursor, Paint()..color = color);
  }

  @override
  bool shouldRepaint(_FerryPainter old) => old.color != color;
}
