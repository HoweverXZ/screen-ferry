// 布局：把对方的屏幕拖到本机的上下左右，松手后弹性吸附到最近的一侧。

import 'dart:math';

import 'package:flutter/material.dart';

import '../state/app_scope.dart';
import '../state/app_state.dart';
import '../theme/tokens.dart';
import '../widgets/controls.dart';
import '../widgets/group.dart';
import '../widgets/page_scaffold.dart';
import '../widgets/surface.dart';

class LayoutPage extends StatelessWidget {
  const LayoutPage({super.key});

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = SfColors.of(context);
    if (!state.isServer) {
      // 屏幕怎么摆由键鼠所在的那台（服务端）决定。
      return PageScaffold(
        title: '布局',
        children: [
          SfCard(
            child: Row(
              children: [
                Icon(Icons.splitscreen_rounded, size: 28, color: c.accent),
                const SizedBox(width: 16),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text('屏幕位置在 ${state.peerName} 上设置', style: const TextStyle(fontWeight: FontWeight.w600)),
                      Text('键盘鼠标在那台电脑上，由它决定从哪条边切过来。', style: TextStyle(color: c.muted)),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ],
      );
    }
    return PageScaffold(
      title: '布局',
      children: [
        const _Stage(),
        const SfGroup(
          children: [
            SfRow(
              title: '切换方式',
              subtitle: '停留一下再切换，可以避免鼠标甩到边缘时误切 · 即将支持',
              trailing: SfSegmented<SwitchMode>(
                label: '切换方式',
                segments: [(SwitchMode.instant, '立即'), (SwitchMode.dwell, '停留 0.15 秒')],
                value: SwitchMode.instant,
                onChanged: null,
              ),
            ),
            SfRow(
              title: '拖拽时不切换',
              subtitle: '按住鼠标键时，光标不会切到另一台电脑 · 目前始终开启',
              trailing: SfSwitch(value: true, label: '拖拽时不切换', onChanged: null),
            ),
            SfRow(title: '回到本机', subtitle: '光标卡在对方屏幕上时，按它立即回来', trailing: KeyCombo(['Ctrl', 'Option', 'Esc'])),
          ],
        ),
      ],
    );
  }
}

class _Stage extends StatefulWidget {
  const _Stage();

  @override
  State<_Stage> createState() => _StageState();
}

class _StageState extends State<_Stage> {
  static const _gap = 8.0;

  /// 拖动中对方屏幕的中心；不在拖动时为 `null`。
  Offset? _drag;

  /// 屏幕框的大小随画布宽度变化：本机居中，旁边那台（宽度的 1.5 倍处）也要完整放进画布。
  Size _screen(double width) {
    final w = min(180.0, ((width / 2 - 20) / 1.5).floorToDouble());
    return Size(w, (w * 0.6).roundToDouble());
  }

  Offset _slot(Offset center, ScreenSide side, Size s) => switch (side) {
    ScreenSide.right => center + Offset(s.width + _gap, 0),
    ScreenSide.left => center - Offset(s.width + _gap, 0),
    ScreenSide.top => center - Offset(0, s.height + _gap),
    ScreenSide.bottom => center + Offset(0, s.height + _gap),
  };

  ScreenSide _nearest(Offset fromCenter, Size s) {
    if (fromCenter.dx.abs() * s.height >= fromCenter.dy.abs() * s.width) {
      return fromCenter.dx >= 0 ? ScreenSide.right : ScreenSide.left;
    }
    return fromCenter.dy >= 0 ? ScreenSide.bottom : ScreenSide.top;
  }

  Rect _edge(Rect local, ScreenSide side) {
    const t = 4.0, inset = 16.0;
    return switch (side) {
      ScreenSide.right => Rect.fromLTWH(local.right + 2, local.top + inset, t, local.height - inset * 2),
      ScreenSide.left => Rect.fromLTWH(local.left - t - 2, local.top + inset, t, local.height - inset * 2),
      ScreenSide.top => Rect.fromLTWH(local.left + inset, local.top - t - 2, local.width - inset * 2, t),
      ScreenSide.bottom => Rect.fromLTWH(local.left + inset, local.bottom + 2, local.width - inset * 2, t),
    };
  }

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = SfColors.of(context);
    return SizedBox(
      height: 380,
      child: DecoratedBox(
        decoration: ShapeDecoration(color: c.card, shape: continuous(26)),
        child: ClipRSuperellipse(
          borderRadius: BorderRadius.circular(26),
          child: LayoutBuilder(
            builder: (context, box) {
              final s = _screen(box.maxWidth);
              final center = Offset(box.maxWidth / 2, box.maxHeight / 2 - 10);
              final local = Rect.fromCenter(center: center, width: s.width, height: s.height);
              final dragging = _drag != null;
              final preview = dragging ? _nearest(_drag! - center, s) : state.side;
              final remote = _drag ?? _slot(center, state.side, s);
              final ghost = Rect.fromCenter(center: _slot(center, preview, s), width: s.width, height: s.height);
              return Stack(
                children: [
                  Positioned.fill(child: CustomPaint(painter: _DotGrid(c.line))),
                  AnimatedPositioned.fromRect(
                    duration: Motion.fast,
                    rect: ghost,
                    child: AnimatedOpacity(
                      duration: Motion.fast,
                      opacity: dragging ? 0.6 : 0,
                      child: DecoratedBox(
                        decoration: ShapeDecoration(
                          shape: continuous(16, side: BorderSide(color: c.accent, width: 2)),
                        ),
                      ),
                    ),
                  ),
                  AnimatedPositioned.fromRect(
                    duration: const Duration(milliseconds: 400),
                    curve: Motion.spring,
                    rect: _edge(local, preview),
                    child: DecoratedBox(
                      decoration: ShapeDecoration(color: c.accent, shape: continuous(3)),
                    ),
                  ),
                  Positioned.fromRect(
                    rect: local,
                    child: _ScreenBox(name: state.localName, detail: '本机'),
                  ),
                  AnimatedPositioned(
                    duration: dragging ? Duration.zero : const Duration(milliseconds: 500),
                    curve: Motion.spring,
                    left: remote.dx - s.width / 2,
                    top: remote.dy - s.height / 2,
                    width: s.width,
                    height: s.height,
                    child: Semantics(
                      label: '${state.peerName} 在本机${state.side.label}，可以拖动',
                      child: MouseRegion(
                        cursor: dragging ? SystemMouseCursors.grabbing : SystemMouseCursors.grab,
                        child: GestureDetector(
                          // 没连上后台时不能改，拖不动。
                          onPanStart: state.online ? (_) => setState(() => _drag = remote) : null,
                          onPanUpdate: state.online ? (d) => setState(() => _drag = _drag! + d.delta) : null,
                          onPanEnd: state.online
                              ? (_) {
                                  final side = _nearest(_drag! - center, s);
                                  setState(() => _drag = null);
                                  state.moveScreen(side);
                                }
                              : null,
                          child: AnimatedScale(
                            scale: dragging ? 1.04 : 1,
                            duration: Motion.fast,
                            curve: Motion.spring,
                            child: _ScreenBox(
                              name: state.peerName,
                              detail: state.connected ? '已连接' : '未连接',
                              remote: true,
                              lifted: dragging,
                            ),
                          ),
                        ),
                      ),
                    ),
                  ),
                  Positioned(
                    left: 18,
                    right: 18,
                    bottom: 14,
                    child: Text(
                      '${state.peerName}在本机${state.side.label}，光标从相接的那条边穿过去',
                      style: TextStyle(fontSize: 13, color: c.muted),
                    ),
                  ),
                ],
              );
            },
          ),
        ),
      ),
    );
  }
}

class _ScreenBox extends StatelessWidget {
  const _ScreenBox({required this.name, required this.detail, this.remote = false, this.lifted = false});

  final String name;
  final String detail;
  final bool remote;
  final bool lifted;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    return AnimatedContainer(
      duration: Motion.fast,
      decoration: ShapeDecoration(
        color: remote ? c.accentSoft : c.card2,
        shape: continuous(16, side: BorderSide(color: remote ? c.accent : c.line, width: 2)),
        shadows: lifted ? [BoxShadow(color: c.shadow, blurRadius: 32, offset: const Offset(0, 14))] : const [],
      ),
      child: Stack(
        children: [
          Positioned(
            left: 8,
            right: 8,
            top: 8,
            height: 8,
            child: DecoratedBox(
              decoration: ShapeDecoration(
                color: remote ? c.accent.withValues(alpha: 0.3) : c.card3,
                shape: continuous(4),
              ),
            ),
          ),
          Positioned.fill(
            top: 18,
            child: Padding(
              padding: const EdgeInsets.all(6),
              child: FittedBox(
                fit: BoxFit.scaleDown,
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(name, style: const TextStyle(fontSize: 15, fontWeight: FontWeight.w700)),
                    Text(detail, style: TextStyle(fontSize: 12, color: c.muted)),
                  ],
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _DotGrid extends CustomPainter {
  _DotGrid(this.color);

  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()..color = color;
    const step = 22.0;
    for (var y = step / 2; y < size.height; y += step) {
      for (var x = step / 2; x < size.width; x += step) {
        canvas.drawCircle(Offset(x, y), 1.4, paint);
      }
    }
  }

  @override
  bool shouldRepaint(_DotGrid old) => old.color != color;
}
