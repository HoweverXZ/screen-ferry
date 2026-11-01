// 布局：本机居中，每台已配对设备贴在它所在的那一侧；同一侧的几台沿着那条边排开。
// 任何一台都可以拖到别的一侧或同一侧的别的位置，松手后弹性吸附过去并保存。

import 'dart:math';

import 'package:flutter/gestures.dart';
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
        SfGroup(
          children: [
            const SfRow(
              title: '切换方式',
              subtitle: '停留一下再切换，可以避免鼠标甩到边缘时误切 · 即将支持',
              trailing: SfSegmented<SwitchMode>(
                label: '切换方式',
                segments: [(SwitchMode.instant, '立即'), (SwitchMode.dwell, '停留 0.15 秒')],
                value: SwitchMode.instant,
                onChanged: null,
              ),
            ),
            const SfRow(
              title: '拖拽时不切换',
              subtitle: '按住鼠标键时，光标不会切到另一台电脑 · 目前始终开启',
              trailing: SfSwitch(value: true, label: '拖拽时不切换', onChanged: null),
            ),
            SfRow(
              title: '回到本机',
              subtitle: '光标卡在对方屏幕上时，按它立即回来',
              trailing: KeyCombo(['Ctrl', state.localIsMac ? 'Option' : 'Alt', 'Esc']),
            ),
          ],
        ),
      ],
    );
  }
}

// ── 几何 ─────────────────────────────────────

/// 屏幕框的高宽比。
const _aspect = 0.6;

/// 本机与设备框之间的距离，相接的那条边就画在这里。
const _gap = 8.0;

/// 同一侧相邻两台设备之间至少留出的距离。
const _between = 10.0;

/// 画布四周留白。
const _margin = 16.0;

const _maxBoxWidth = 180.0;
const _minBoxWidth = 32.0;

/// 画布高度随宽度变化，窄的时候矮一些，免得上下空出一大片。
double _canvasHeight(double width) => (width * 0.62).clamp(250.0, 372.0);

/// 一种布局下本机和各设备框的位置。
///
/// 左右两侧的框都落在本机的上下边之间，上下两侧的框都落在本机的左右边之间，四侧互不重叠；
/// 一侧有几台，本机的那条边就够放几个框，每台正对着自己的那一段。
class _Geometry {
  _Geometry._(this.local, this.box, this._counts);

  factory _Geometry.of(Size canvas, Arrangement shown) {
    final counts = {for (final s in ScreenSide.values) s: shown.on(s).length};
    final across = max(1, max(counts[ScreenSide.left]!, counts[ScreenSide.right]!));
    final along = max(1, max(counts[ScreenSide.top]!, counts[ScreenSide.bottom]!));
    // 本机宽度 = a × 框宽 + b：只有一台时与设备框一样大，几台时每台再留出间隔。
    final (aH, bH) = along == 1 ? (1.0, 0.0) : (along.toDouble(), along * _between);
    final (aV, bV) = across == 1 ? (1.0, 0.0) : (across.toDouble(), across * _between / _aspect);
    // 横向：本机 + 两侧各一个框；纵向同理（按高宽比换算成宽度）。
    final room = [canvas.width - 2 * _margin - 2 * _gap, (canvas.height - 2 * _margin - 2 * _gap) / _aspect];
    var w = _maxBoxWidth;
    for (final r in room) {
      for (final (a, b) in [(aH, bH), (aV, bV)]) {
        w = min(w, (r - b) / (a + 2));
      }
    }
    w = max(_minBoxWidth, w.floorToDouble());
    final lw = max(aH * w + bH, aV * w + bV).roundToDouble();
    final local = Rect.fromCenter(
      center: Offset(canvas.width / 2, canvas.height / 2),
      width: lw,
      height: (lw * _aspect).roundToDouble(),
    );
    return _Geometry._(local, Size(w, (w * _aspect).roundToDouble()), counts);
  }

  final Rect local;

  /// 设备框的大小。
  final Size box;
  final Map<ScreenSide, int> _counts;

  bool _vertical(ScreenSide side) => side == ScreenSide.left || side == ScreenSide.right;

  /// 本机那条边上属于第 [index] 台的一段：起点和长度。
  (double, double) _segment(ScreenSide side, int index) {
    final k = max(1, _counts[side]!);
    final len = (_vertical(side) ? local.height : local.width) / k;
    return ((_vertical(side) ? local.top : local.left) + len * index, len);
  }

  /// 第 [index] 台设备框的位置。
  Rect slot(ScreenSide side, int index) {
    final (start, len) = _segment(side, index);
    final mid = start + len / 2;
    final center = switch (side) {
      ScreenSide.right => Offset(local.right + _gap + box.width / 2, mid),
      ScreenSide.left => Offset(local.left - _gap - box.width / 2, mid),
      ScreenSide.top => Offset(mid, local.top - _gap - box.height / 2),
      ScreenSide.bottom => Offset(mid, local.bottom + _gap + box.height / 2),
    };
    return Rect.fromCenter(center: center, width: box.width, height: box.height);
  }

  /// 第 [index] 台与本机相接的那一段边，画成一道短线。
  Rect seam(ScreenSide side, int index) {
    const t = 4.0;
    final (start, len) = _segment(side, index);
    final inset = (len * 0.15).clamp(6.0, 16.0);
    final (a, l) = (start + inset, max(0.0, len - inset * 2));
    return switch (side) {
      ScreenSide.right => Rect.fromLTWH(local.right + 2, a, t, l),
      ScreenSide.left => Rect.fromLTWH(local.left - t - 2, a, t, l),
      ScreenSide.top => Rect.fromLTWH(a, local.top - t - 2, l, t),
      ScreenSide.bottom => Rect.fromLTWH(a, local.bottom + 2, l, t),
    };
  }
}

// ── 画布 ─────────────────────────────────────

class _Stage extends StatefulWidget {
  const _Stage();

  @override
  State<_Stage> createState() => _StageState();
}

class _StageState extends State<_Stage> {
  static const _settle = Duration(milliseconds: 500);
  static const _reflow = Duration(milliseconds: 400);

  /// 正在拖动的设备；不在拖动时为 null。
  String? _dragging;

  /// 最近拖过的设备画在最上面，松手后弹回去的途中也不会被别的框挡住。
  String? _top;

  /// 拖动开始时框的中心与指针位置，拖动中框的中心与大小（保持拖起时的大小）。
  Offset _from = Offset.zero;
  Offset _grab = Offset.zero;
  Offset _center = Offset.zero;
  Size _size = Size.zero;
  Size _canvas = Size.zero;

  /// 按框现在的中心，找出它会落到的位置：在每个可能的位置上按那种布局算一遍，取最近的一个。
  /// 每个候选各用自己的几何，结果只取决于指针，不会因为预览改变了框的大小而来回跳。
  PeerPlace _target(Arrangement rest, String fingerprint) {
    final others = rest.without(fingerprint);
    PeerPlace? best;
    var bestDistance = double.infinity;
    for (final side in ScreenSide.values) {
      for (var i = 0; i <= others.on(side).length; i++) {
        final slot = _Geometry.of(_canvas, rest.moved(fingerprint, side, i)).slot(side, i);
        final d = (slot.center - _center).distanceSquared;
        if (d < bestDistance) (best, bestDistance) = ((side: side, index: i), d);
      }
    }
    return best!;
  }

  void _start(String fingerprint, Rect slot, Offset pointer) => setState(() {
    _dragging = fingerprint;
    _top = fingerprint;
    _from = slot.center;
    _center = slot.center;
    _grab = pointer;
    _size = slot.size;
  });

  void _update(Offset pointer) => setState(() => _center = _from + (pointer - _grab));

  void _end(AppState state) {
    final fingerprint = _dragging;
    if (fingerprint == null) return;
    final rest = state.arrangement;
    setState(() => _dragging = null);
    if (rest.placeOf(fingerprint) == null) return;
    final to = _target(rest, fingerprint);
    state.moveDevice(fingerprint, to.side, to.index);
  }

  void _cancel() => setState(() => _dragging = null);

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = SfColors.of(context);
    return DecoratedBox(
      decoration: ShapeDecoration(color: c.card, shape: continuous(26)),
      child: ClipRSuperellipse(
        borderRadius: BorderRadius.circular(26),
        child: CustomPaint(
          painter: _DotGrid(c.line),
          child: LayoutBuilder(
            builder: (context, box) {
              // 窗口大小在变时直接跟上，不播放弹簧，免得框跟着窗口晃。
              final resized = _canvas != Size.zero && _canvas.width != box.maxWidth;
              _canvas = Size(box.maxWidth, _canvasHeight(box.maxWidth));
              Duration motion(Duration d) => resized ? Duration.zero : d;
              final rest = state.arrangement;
              // 拖动中的设备被移除了（例如在别处取消配对），就当拖动结束。
              final dragging = _dragging != null && rest.placeOf(_dragging!) != null ? _dragging : null;
              final target = dragging == null ? null : _target(rest, dragging);
              final shown = target == null ? rest : rest.moved(dragging!, target.side, target.index);
              final g = _Geometry.of(_canvas, shown);
              final cursor = state.cursorPeer?.fingerprint;

              final seams = <Widget>[];
              final boxes = <Widget>[];
              Widget? lifted;
              for (final side in ScreenSide.values) {
                final list = shown.on(side);
                for (var i = 0; i < list.length; i++) {
                  final p = list[i];
                  final isDragged = p.fingerprint == dragging;
                  // 拖动时只突出落点那一段；平时已连接的每段都标出来，未连接的淡一些。
                  final strong = dragging == null ? p.online : isDragged;
                  seams.add(
                    AnimatedPositioned.fromRect(
                      key: ValueKey('seam-${p.fingerprint}'),
                      duration: motion(_reflow),
                      curve: Motion.spring,
                      rect: g.seam(side, i),
                      child: AnimatedContainer(
                        duration: Motion.fast,
                        decoration: ShapeDecoration(color: strong ? c.accent : c.card3, shape: continuous(3)),
                      ),
                    ),
                  );
                  final slot = g.slot(side, i);
                  final rect = isDragged
                      ? Rect.fromCenter(center: _center, width: _size.width, height: _size.height)
                      : slot;
                  final widget = AnimatedPositioned.fromRect(
                    key: ValueKey('screen-${p.fingerprint}'),
                    duration: isDragged ? Duration.zero : motion(dragging == null ? _settle : _reflow),
                    curve: Motion.spring,
                    rect: rect,
                    child: _DeviceTile(
                      peer: p,
                      label: _label(p, side, i, list.length, cursor: p.fingerprint == cursor, movable: state.online),
                      cursor: p.fingerprint == cursor,
                      lifted: isDragged,
                      // 没连上后台时不能改，拖不动。
                      onStart: state.online ? (pointer) => _start(p.fingerprint, slot, pointer) : null,
                      onUpdate: _update,
                      onEnd: () => _end(state),
                      onCancel: _cancel,
                    ),
                  );
                  if (p.fingerprint == _top) {
                    lifted = widget;
                  } else {
                    boxes.add(widget);
                  }
                }
              }

              return Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  SizedBox.fromSize(
                    size: _canvas,
                    child: Stack(
                      children: [
                        if (dragging != null && target != null)
                          AnimatedPositioned.fromRect(
                            key: const ValueKey('ghost'),
                            duration: Motion.fast,
                            curve: Motion.ease,
                            rect: g.slot(target.side, target.index),
                            child: TweenAnimationBuilder<double>(
                              tween: Tween(begin: 0, end: 0.6),
                              duration: Motion.fast,
                              builder: (context, o, child) => Opacity(opacity: o, child: child),
                              child: DecoratedBox(
                                decoration: ShapeDecoration(
                                  shape: continuous(_radius(g.box), side: BorderSide(color: c.accent, width: 2)),
                                ),
                              ),
                            ),
                          ),
                        ...seams,
                        AnimatedPositioned.fromRect(
                          key: const ValueKey('screen-local'),
                          duration: motion(_reflow),
                          curve: Motion.spring,
                          rect: g.local,
                          child: _ScreenBox(name: state.localName, look: _Look.local),
                        ),
                        ...boxes,
                        ?lifted,
                      ],
                    ),
                  ),
                  Padding(
                    padding: const EdgeInsets.fromLTRB(18, 0, 18, 14),
                    child: Row(
                      children: [
                        Expanded(
                          child: Text(
                            _caption(shown, target),
                            style: TextStyle(fontSize: 13, color: c.muted, height: 1.5),
                          ),
                        ),
                        if (shown.isEmpty) ...[
                          const SizedBox(width: 12),
                          TextLink(label: '去配对', chevron: true, onPressed: () => state.go(SfPage.devices)),
                        ],
                      ],
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

  static String _order(ScreenSide side) => side == ScreenSide.left || side == ScreenSide.right ? '从上往下' : '从左往右';

  static String _label(PeerInfo p, ScreenSide side, int i, int k, {required bool cursor, required bool movable}) => [
    '${p.displayName}，在本机${side.label}${k > 1 ? '${_order(side)}第 ${i + 1} 台' : ''}',
    p.online ? '已连接' : '未连接',
    if (cursor) '光标在这台上',
    if (movable) '可以拖动',
  ].join('，');

  static String _caption(Arrangement shown, PeerPlace? target) {
    if (shown.isEmpty) return '还没有配对的设备。在另一台电脑上运行 ScreenFerry 并配对后，它会出现在本机旁边';
    if (target != null) {
      final k = shown.on(target.side).length;
      final where = '松手后放到本机${target.side.label}';
      return k > 1 ? '$where，${_order(target.side)}第 ${target.index + 1} 台' : where;
    }
    if (shown.length == 1) {
      final p = shown.all.single;
      return '${p.displayName} 在本机${shown.placeOf(p.fingerprint)!.side.label}，光标从相接的那条边穿过去';
    }
    final parts = [for (final s in shown.usedSides) '${s.label} ${shown.on(s).length} 台'];
    return '本机${parts.join('、')}。同一侧已连接的几台按先后平分那条边，光标从哪一段穿过去就到哪台';
  }
}

/// 框越小圆角越小，小框也不会变成胶囊。
double _radius(Size box) => min(16.0, box.shortestSide * 0.2).roundToDouble();

/// 一台设备的框：可以拖动。
class _DeviceTile extends StatelessWidget {
  const _DeviceTile({
    required this.peer,
    required this.label,
    required this.cursor,
    required this.lifted,
    required this.onStart,
    required this.onUpdate,
    required this.onEnd,
    required this.onCancel,
  });

  final PeerInfo peer;
  final String label;
  final bool cursor;
  final bool lifted;
  final ValueChanged<Offset>? onStart;
  final ValueChanged<Offset> onUpdate;
  final VoidCallback onEnd;
  final VoidCallback onCancel;

  @override
  Widget build(BuildContext context) {
    final movable = onStart != null;
    return Semantics(
      label: label,
      child: MouseRegion(
        cursor: !movable
            ? SystemMouseCursors.basic
            : lifted
            ? SystemMouseCursors.grabbing
            : SystemMouseCursors.grab,
        child: RawGestureDetector(
          gestures: {
            if (movable)
              _EagerPan: GestureRecognizerFactoryWithHandlers<_EagerPan>(
                _EagerPan.new,
                (r) => r
                  // 按下就开始跟手，框不会落后指针一段。
                  ..dragStartBehavior = DragStartBehavior.down
                  ..onStart = ((d) => onStart!(d.globalPosition))
                  ..onUpdate = ((d) => onUpdate(d.globalPosition))
                  ..onEnd = ((_) => onEnd())
                  ..onCancel = onCancel,
              ),
          },
          child: AnimatedScale(
            scale: lifted ? 1.04 : 1,
            duration: Motion.fast,
            curve: Motion.spring,
            child: _ScreenBox(
              name: peer.displayName,
              look: peer.online ? _Look.online : _Look.offline,
              cursor: cursor,
              lifted: lifted,
            ),
          ),
        ),
      ),
    );
  }
}

/// 按下就认领这次拖动：画布在可以滚动的页面里，竖着拖也不会被页面滚动抢走。
class _EagerPan extends PanGestureRecognizer {
  @override
  void addAllowedPointer(PointerDownEvent event) {
    super.addAllowedPointer(event);
    resolve(GestureDisposition.accepted);
  }
}

enum _Look { local, online, offline }

/// 一块屏幕：顶上一条窄栏，中间是名字和状态。
///
/// 本机是实心的中性色；已连接的设备用强调色；未连接的只有虚线轮廓、文字变淡。
/// 光标在哪台设备上，那台四周有一圈柔和的强调色光晕，状态换成“光标在这里”。
class _ScreenBox extends StatelessWidget {
  const _ScreenBox({required this.name, required this.look, this.cursor = false, this.lifted = false});

  final String name;
  final _Look look;
  final bool cursor;
  final bool lifted;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    return LayoutBuilder(
      builder: (context, box) {
        final size = box.biggest;
        final radius = _radius(size);
        final inset = (size.height * 0.075).clamp(4.0, 8.0);
        final (fill, edge, bar, ink) = switch (look) {
          _Look.local => (c.card2, c.line, c.card3, c.fg),
          _Look.online => (c.accentSoft, c.accent, c.accent.withValues(alpha: 0.3), c.fg),
          _Look.offline => (lifted ? c.card : c.card.withValues(alpha: 0.75), Colors.transparent, c.card3, c.muted),
        };
        final (status, statusInk) = switch (look) {
          _ when cursor => ('光标在这里', c.accent),
          _Look.local => ('本机', c.muted),
          _Look.online => ('已连接', c.muted),
          _Look.offline => ('未连接', c.muted),
        };
        Widget screen = AnimatedContainer(
          duration: Motion.medium,
          curve: Motion.ease,
          decoration: ShapeDecoration(
            color: fill,
            shape: continuous(radius, side: BorderSide(color: edge, width: 2)),
            shadows: [
              if (lifted) BoxShadow(color: c.shadow, blurRadius: 32, offset: const Offset(0, 14)),
              if (cursor) BoxShadow(color: c.accent.withValues(alpha: 0.28), blurRadius: 18, spreadRadius: 1),
            ],
          ),
          child: Stack(
            children: [
              Positioned(
                left: inset,
                right: inset,
                top: inset,
                height: inset,
                child: DecoratedBox(
                  decoration: ShapeDecoration(color: bar, shape: continuous(inset / 2)),
                ),
              ),
              Positioned.fill(
                top: inset * 2 + 2,
                child: Padding(
                  padding: EdgeInsets.all(inset * 0.75),
                  child: FittedBox(
                    fit: BoxFit.scaleDown,
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Text(
                          name,
                          style: TextStyle(fontSize: 15, fontWeight: FontWeight.w700, color: ink),
                        ),
                        const SizedBox(height: 1),
                        Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            if (cursor)
                              Icon(Icons.near_me_rounded, size: 12, color: c.accent)
                            else if (look != _Look.local)
                              StatusDot(on: look == _Look.online),
                            if (cursor || look != _Look.local) const SizedBox(width: 5),
                            Text(
                              status,
                              style: TextStyle(
                                fontSize: 12,
                                color: statusInk,
                                fontWeight: cursor ? FontWeight.w600 : FontWeight.w400,
                              ),
                            ),
                          ],
                        ),
                      ],
                    ),
                  ),
                ),
              ),
            ],
          ),
        );
        if (look == _Look.offline) {
          screen = CustomPaint(
            foregroundPainter: _DashedOutline(color: c.knobOff.withValues(alpha: 0.5), radius: radius),
            child: screen,
          );
        }
        return screen;
      },
    );
  }
}

/// 未连接设备的虚线轮廓，圆角与框一致。
class _DashedOutline extends CustomPainter {
  _DashedOutline({required this.color, required this.radius});

  final Color color;
  final double radius;

  @override
  void paint(Canvas canvas, Size size) {
    final rect = RSuperellipse.fromRectAndRadius((Offset.zero & size).deflate(1), Radius.circular(radius));
    final paint = Paint()
      ..color = color
      ..style = PaintingStyle.stroke
      ..strokeWidth = 2;
    for (final metric in (Path()..addRSuperellipse(rect)).computeMetrics()) {
      for (var d = 0.0; d < metric.length; d += 10) {
        canvas.drawPath(metric.extractPath(d, min(d + 5, metric.length)), paint);
      }
    }
  }

  @override
  bool shouldRepaint(_DashedOutline old) => old.color != color || old.radius != radius;
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
