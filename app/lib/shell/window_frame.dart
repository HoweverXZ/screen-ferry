// 窗口外框：Windows 上没有系统标题栏，右上角的三个按钮和顶部的拖动区域由这里画。
// macOS 用系统的红绿灯和透明标题栏，这里什么也不画。两边页面布局完全相同：
// 顶部 32 点在 macOS 上是红绿灯那一行，在 Windows 上是拖动区域和窗口按钮。

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../theme/tokens.dart';
import '../widgets/pressable.dart';

/// 是否由界面自己画窗口按钮。
final bool drawsWindowFrame = Platform.isWindows;

/// 顶部留给窗口按钮与拖动的高度，与 macOS 标题栏那一行一致。
const windowBarHeight = 32.0;

/// 与原生窗口之间的通道，见 `windows/runner/flutter_window.cpp`。
abstract final class NativeWindow {
  static const _channel = MethodChannel('screenferry/window');

  /// 窗口是否最大化。最大化或还原时由原生一侧通知。
  static final maximized = ValueNotifier(false);
  static bool _listening = false;

  /// 开始接收原生一侧的通知，并读一次当前状态。
  static void _listen() {
    if (!_listening) {
      _listening = true;
      _channel.setMethodCallHandler((call) async {
        if (call.method == 'maximized') maximized.value = call.arguments == true;
      });
    }
    _readMaximized();
  }

  static Future<void> _readMaximized() async {
    try {
      maximized.value = await _channel.invokeMethod<bool>('isMaximized') ?? false;
    } on MissingPluginException {
      // 没有原生一侧（例如测试中）。
    } on PlatformException {
      // 原生一侧出错时按未最大化处理。
    }
  }

  static Future<void> _call(String method) async {
    try {
      await _channel.invokeMethod<void>(method);
    } on MissingPluginException {
      // 没有原生一侧（例如测试中）。
    } on PlatformException {
      // 原生一侧没能执行，窗口保持原样。
    }
  }

  static Future<void> minimize() => _call('minimize');
  static Future<void> toggleMaximize() => _call('toggleMaximize');
  static Future<void> close() => _call('close');
  static Future<void> startDrag() => _call('startDrag');
}

/// 盖在页面最上面的窗口外框：拖动区域加右上角的窗口按钮。
class WindowFrame extends StatefulWidget {
  const WindowFrame({super.key});

  @override
  State<WindowFrame> createState() => _WindowFrameState();
}

class _WindowFrameState extends State<WindowFrame> {
  @override
  void initState() {
    super.initState();
    NativeWindow._listen();
  }

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: windowBarHeight,
      child: Row(
        children: [
          // 拖动移动窗口，双击最大化或还原，与系统标题栏的行为一致。
          Expanded(
            child: GestureDetector(
              behavior: HitTestBehavior.opaque,
              onPanStart: (_) => NativeWindow.startDrag(),
              onDoubleTap: NativeWindow.toggleMaximize,
            ),
          ),
          const _WindowButtons(),
        ],
      ),
    );
  }
}

class _WindowButtons extends StatelessWidget {
  const _WindowButtons();

  @override
  Widget build(BuildContext context) {
    return ValueListenableBuilder(
      valueListenable: NativeWindow.maximized,
      builder: (context, maximized, _) => Padding(
        padding: const EdgeInsets.only(top: 4, right: 6),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            _WindowButton(glyph: _Glyph.minimize, label: '最小化', onTap: NativeWindow.minimize),
            _WindowButton(
              glyph: maximized ? _Glyph.restore : _Glyph.maximize,
              label: maximized ? '还原' : '最大化',
              onTap: NativeWindow.toggleMaximize,
            ),
            _WindowButton(glyph: _Glyph.close, label: '关闭', onTap: NativeWindow.close, danger: true),
          ],
        ),
      ),
    );
  }
}

enum _Glyph { minimize, maximize, restore, close }

class _WindowButton extends StatefulWidget {
  const _WindowButton({required this.glyph, required this.label, required this.onTap, this.danger = false});

  final _Glyph glyph;
  final String label;
  final VoidCallback onTap;

  /// 关闭按钮：悬停时变红，图标变白。
  final bool danger;

  @override
  State<_WindowButton> createState() => _WindowButtonState();
}

class _WindowButtonState extends State<_WindowButton> {
  bool _hover = false;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    final ink = widget.danger && _hover ? c.onAccent : c.fg;
    return MouseRegion(
      onEnter: (_) => setState(() => _hover = true),
      onExit: (_) => setState(() => _hover = false),
      child: Pressable(
        onTap: widget.onTap,
        hoverColor: widget.danger ? c.closeHover : c.navHover,
        radius: 8,
        pressScale: 0.94,
        semanticLabel: widget.label,
        tooltip: widget.label,
        child: SizedBox(width: 40, height: 26, child: CustomPaint(painter: _GlyphPainter(widget.glyph, ink))),
      ),
    );
  }
}

/// 细线条的窗口按钮图标，10 点见方，画在按钮正中。
class _GlyphPainter extends CustomPainter {
  _GlyphPainter(this.glyph, this.color);

  final _Glyph glyph;
  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    const s = 10.0;
    canvas.translate((size.width - s) / 2, (size.height - s) / 2);
    final p = Paint()
      ..color = color
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.1
      ..strokeCap = StrokeCap.round;
    switch (glyph) {
      case _Glyph.minimize:
        canvas.drawLine(const Offset(0, s / 2), const Offset(s, s / 2), p);
      case _Glyph.maximize:
        canvas.drawRRect(RRect.fromLTRBR(0.5, 0.5, s - 0.5, s - 0.5, const Radius.circular(2)), p);
      case _Glyph.restore:
        canvas.drawRRect(RRect.fromLTRBR(0.5, 2.5, s - 2.5, s - 0.5, const Radius.circular(1.6)), p);
        canvas.drawPath(
          Path()
            ..moveTo(2.5, 0.5)
            ..lineTo(s - 1.5, 0.5)
            ..quadraticBezierTo(s - 0.5, 0.5, s - 0.5, 1.5)
            ..lineTo(s - 0.5, s - 2.5),
          p,
        );
      case _Glyph.close:
        canvas.drawLine(Offset.zero, const Offset(s, s), p);
        canvas.drawLine(const Offset(s, 0), const Offset(0, s), p);
    }
  }

  @override
  bool shouldRepaint(_GlyphPainter old) => old.glyph != glyph || old.color != color;
}

/// 测试用：不经原生一侧，直接改最大化状态。
@visibleForTesting
void debugSetMaximized(bool value) => NativeWindow.maximized.value = value;
