// 花朵：两层波浪花瓣反向慢转、整体呼吸、颜色在三种淡色间渐变。
//
// 转动代表“正在共享”。为了省电：
// - 只按每秒 30 帧刷新（转得很慢，看不出差别），且只重绘自己这一块；
// - 暂停共享、窗口不在前台、系统要求减少动效时，完全停下。

import 'dart:async';
import 'dart:math';

import 'package:flutter/material.dart';

import '../theme/tokens.dart';

class Bloom extends StatefulWidget {
  const Bloom({super.key, required this.active, required this.size, required this.center});

  final bool active;
  final double size;
  final Widget center;

  @override
  State<Bloom> createState() => _BloomState();
}

class _BloomState extends State<Bloom> with SingleTickerProviderStateMixin {
  /// 动画时间（秒）。只在转动时增加，暂停后从原位置接着转。
  final _time = ValueNotifier<double>(0);
  final _clock = Stopwatch();
  double _base = 0;
  Timer? _frames;

  /// 0 为正常，1 为暂停时的淡灰。
  late final AnimationController _calm = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 800),
    value: widget.active ? 0 : 1,
  );
  late final AppLifecycleListener _life;
  bool _foreground = true;

  @override
  void initState() {
    super.initState();
    _life = AppLifecycleListener(
      onStateChange: (state) {
        _foreground = state == AppLifecycleState.resumed;
        _sync();
      },
    );
    WidgetsBinding.instance.addPostFrameCallback((_) => _sync());
  }

  @override
  void didUpdateWidget(Bloom old) {
    super.didUpdateWidget(old);
    if (old.active != widget.active) {
      _calm.animateTo(widget.active ? 0 : 1, curve: Motion.ease);
      _sync();
    }
  }

  void _sync() {
    if (!mounted) return;
    final reduce = MediaQuery.maybeDisableAnimationsOf(context) ?? false;
    final run = widget.active && _foreground && !reduce;
    if (run && _frames == null) {
      _clock
        ..reset()
        ..start();
      _frames = Timer.periodic(const Duration(milliseconds: 33), (_) {
        _time.value = _base + _clock.elapsedMicroseconds / 1e6;
      });
    } else if (!run && _frames != null) {
      _frames!.cancel();
      _frames = null;
      _clock.stop();
      _base += _clock.elapsedMicroseconds / 1e6;
    }
  }

  @override
  void dispose() {
    _frames?.cancel();
    _life.dispose();
    _calm.dispose();
    _time.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    return RepaintBoundary(
      child: SizedBox.square(
        dimension: widget.size,
        child: Stack(
          alignment: Alignment.center,
          children: [
            CustomPaint(
              size: Size.square(widget.size),
              painter: _BloomPainter(time: _time, calm: _calm, palette: c.bloom, rest: c.card2),
            ),
            widget.center,
          ],
        ),
      ),
    );
  }
}

class _BloomPainter extends CustomPainter {
  _BloomPainter({required this.time, required this.calm, required this.palette, required this.rest})
    : super(repaint: Listenable.merge([time, calm]));

  final ValueNotifier<double> time;
  final Animation<double> calm;
  final List<Color> palette;
  final Color rest;

  static final _cache = <(double, int, double), Path>{};

  /// 极坐标波浪：r = R · (1 + k·cos(nθ))，圆心在原点。
  static Path _petals(double radius, int n, double k) => _cache.putIfAbsent((radius, n, k), () {
    final path = Path();
    for (var i = 0; i <= 360; i++) {
      final t = i / 360 * 2 * pi;
      final r = radius * (1 + k * cos(n * t));
      final p = Offset(r * cos(t), r * sin(t));
      if (i == 0) {
        path.moveTo(p.dx, p.dy);
      } else {
        path.lineTo(p.dx, p.dy);
      }
    }
    return path..close();
  });

  /// 三种淡色依次过渡，一轮 24 秒。
  Color _tint(double seconds) {
    final phase = (seconds / 24) % 1 * palette.length;
    final i = phase.floor();
    final f = Curves.easeInOut.transform(phase - i);
    return Color.lerp(palette[i % palette.length], palette[(i + 1) % palette.length], f)!;
  }

  @override
  void paint(Canvas canvas, Size size) {
    final t = time.value;
    final quiet = calm.value;
    final half = size.width / 2;
    final unit = half / 120;
    final breathe = 0.99 + 0.04 * sin(2 * pi * t / 7);

    void layer(Path path, double angle, double tintOffset, double alpha) {
      final color = Color.lerp(_tint(t + tintOffset), rest, quiet)!;
      canvas
        ..save()
        ..translate(half, half)
        ..scale(breathe)
        ..rotate(angle);
      canvas.drawPath(path, Paint()..color = color.withValues(alpha: alpha * (1 - 0.4 * quiet)));
      canvas.restore();
    }

    layer(_petals(104 * unit, 9, 0.075), 2 * pi * t / 40, 0, 1);
    layer(_petals(80 * unit, 6, 0.1), -2 * pi * t / 28, 8, 0.8);
  }

  @override
  bool shouldRepaint(_BloomPainter old) => old.palette != palette || old.rest != rest;
}
