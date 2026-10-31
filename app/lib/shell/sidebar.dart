// 侧栏：顶部收起按钮；选中项是灰色胶囊加左边缘的强调色竖线。
// 切换时胶囊带一点回弹滑过去；竖线前端先到、后端跟上，像被拉长后再收拢。

import 'dart:math' as math;
import 'dart:ui' show lerpDouble;

import 'package:flutter/material.dart';

import '../state/app_scope.dart';
import '../theme/tokens.dart';
import '../widgets/glyph.dart';
import '../widgets/pressable.dart';
import '../widgets/surface.dart';
import 'nav_items.dart';

const _itemHeight = 48.0;
const _itemGap = 4.0;
const _barHeight = 22.0;
const expandedWidth = 228.0;
const collapsedWidth = 80.0;

/// 侧栏内容区左右的留白；右边另有 1 点的分隔线。
const _inset = 14.0;

/// 收起后内容区的宽度。收起按钮、菜单图标、状态圆点都按它居中，展开时位置不变。
const _railWidth = collapsedWidth - _inset * 2 - 1;
const _toggleSize = 44.0;
const _iconSize = 22.0;
const _dotSize = 9.0;

double _topOf(int i) => i * (_itemHeight + _itemGap);

class Sidebar extends StatelessWidget {
  const Sidebar({super.key, required this.forceCollapsed});

  /// 窗口太窄时自动收起，此时不显示收起按钮。
  final bool forceCollapsed;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = SfColors.of(context);
    final collapsed = forceCollapsed || state.sidebarCollapsed;
    final index = navIndexOf(state.page);

    return AnimatedContainer(
      duration: Motion.medium,
      curve: Motion.ease,
      width: collapsed ? collapsedWidth : expandedWidth,
      decoration: BoxDecoration(
        color: c.side,
        border: Border(right: BorderSide(color: c.line)),
      ),
      clipBehavior: Clip.hardEdge,
      // 顶部留出 macOS 红绿灯按钮的位置。
      padding: const EdgeInsets.fromLTRB(_inset, 46, _inset, 16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (!forceCollapsed)
            Padding(
              padding: const EdgeInsets.only(left: (_railWidth - _toggleSize) / 2),
              child: Pressable(
                onTap: () => state.update(() => state.sidebarCollapsed = !state.sidebarCollapsed),
                hoverColor: c.navHover,
                radius: 14,
                tooltip: collapsed ? '展开侧栏' : '收起侧栏',
                semanticLabel: collapsed ? '展开侧栏' : '收起侧栏',
                child: SizedBox.square(
                  dimension: _toggleSize,
                  child: Center(child: GlyphIcon(Glyph.sidebar, color: c.fg)),
                ),
              ),
            ),
          const SizedBox(height: 18),
          SizedBox(
            height: _topOf(navItems.length) - _itemGap,
            child: Stack(
              clipBehavior: Clip.none,
              children: [
                Positioned.fill(child: _Indicator(index: index)),
                Column(
                  children: [
                    for (var i = 0; i < navItems.length; i++) ...[
                      if (i > 0) const SizedBox(height: _itemGap),
                      _NavButton(item: navItems[i], selected: i == index, collapsed: collapsed),
                    ],
                  ],
                ),
              ],
            ),
          ),
          const Spacer(),
          Tooltip(
            message: state.sharing ? '正在共享' : '已暂停共享',
            child: AnimatedContainer(
              duration: Motion.medium,
              curve: Motion.ease,
              height: 44,
              // 收起时圆点正好落在中间；展开时左边留 16。
              padding: EdgeInsets.only(left: collapsed ? (_railWidth - _dotSize) / 2 : 16, right: 12),
              decoration: ShapeDecoration(color: c.card2, shape: continuous(14)),
              child: Row(
                children: [
                  StatusDot(on: state.sharing),
                  Expanded(
                    child: ClipRect(
                      child: AnimatedOpacity(
                        duration: Motion.fast,
                        opacity: collapsed ? 0 : 1,
                        child: Padding(
                          padding: const EdgeInsets.only(left: 10),
                          child: Text(
                            state.sharing ? '共享中 · ${state.latestRtt.toStringAsFixed(2)} ms' : '已暂停共享',
                            maxLines: 1,
                            softWrap: false,
                            overflow: TextOverflow.clip,
                            style: TextStyle(fontSize: 13, color: c.muted, fontFeatures: tabular),
                          ),
                        ),
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}

/// 选中指示：胶囊和左边缘的竖线，由同一个动画驱动。
/// 中途再次切换时从当前画面的位置接着走，不会跳。
class _Indicator extends StatefulWidget {
  const _Indicator({required this.index});

  final int index;

  @override
  State<_Indicator> createState() => _IndicatorState();
}

class _IndicatorState extends State<_Indicator> with SingleTickerProviderStateMixin {
  static const _duration = Duration(milliseconds: 420);

  /// 胶囊：略带回弹。竖线：前端快、后端慢，中间会被拉长。
  static const _pillCurve = Cubic(0.3, 1.25, 0.5, 1);
  static const _leadCurve = Interval(0, 0.55, curve: Curves.easeOutCubic);
  static const _trailCurve = Interval(0.18, 1, curve: Curves.easeInOutCubic);

  late final AnimationController _anim = AnimationController(vsync: this, duration: _duration, value: 1);

  late double _pillFrom = _topOf(widget.index);
  late double _barTopFrom = _barTopAt(widget.index);
  late double _barBottomFrom = _barTopFrom + _barHeight;

  static double _barTopAt(int i) => _topOf(i) + (_itemHeight - _barHeight) / 2;

  /// 从起点（_pillFrom 等）走向第 [index] 项，进度为 [t] 时的画面。
  ({double pill, double barTop, double barBottom}) _frame(int index, double t) {
    final to = _barTopAt(index);
    final down = to >= _barTopFrom;
    final lead = _leadCurve.transform(t);
    final trail = _trailCurve.transform(t);
    final top = lerpDouble(_barTopFrom, to, down ? trail : lead)!;
    final bottom = lerpDouble(_barBottomFrom, to + _barHeight, down ? lead : trail)!;
    return (
      pill: lerpDouble(_pillFrom, _topOf(index), _pillCurve.transform(t))!,
      barTop: math.min(top, bottom - _barHeight),
      barBottom: math.max(bottom, top + _barHeight),
    );
  }

  @override
  void didUpdateWidget(_Indicator old) {
    super.didUpdateWidget(old);
    if (old.index == widget.index) return;
    // 以旧目标算出当前画面，作为新动画的起点。
    final now = _frame(old.index, _anim.value);
    _pillFrom = now.pill;
    _barTopFrom = now.barTop;
    _barBottomFrom = now.barBottom;
    if (MediaQuery.disableAnimationsOf(context)) {
      _anim.value = 1;
    } else {
      _anim.forward(from: 0);
    }
  }

  @override
  void dispose() {
    _anim.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    return AnimatedBuilder(
      animation: _anim,
      builder: (context, _) {
        final f = _frame(widget.index, _anim.value);
        return Stack(
          clipBehavior: Clip.none,
          children: [
            Positioned(
              top: f.pill,
              left: 0,
              right: 0,
              height: _itemHeight,
              child: DecoratedBox(
                decoration: ShapeDecoration(color: c.navPill, shape: continuous(16)),
              ),
            ),
            Positioned(
              left: -_inset,
              top: f.barTop,
              width: 4,
              height: f.barBottom - f.barTop,
              child: DecoratedBox(
                decoration: BoxDecoration(
                  color: c.accent,
                  borderRadius: const BorderRadius.horizontal(right: Radius.circular(4)),
                ),
              ),
            ),
          ],
        );
      },
    );
  }
}

class _NavButton extends StatelessWidget {
  const _NavButton({required this.item, required this.selected, required this.collapsed});

  final NavItem item;
  final bool selected;
  final bool collapsed;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    final state = AppScope.read(context);
    return Pressable(
      onTap: () => state.go(item.page),
      // 选中项已经有胶囊，悬停不再叠一层颜色。
      hoverColor: selected ? c.navHover.withValues(alpha: 0) : c.navHover,
      radius: 16,
      selected: selected,
      semanticLabel: item.label,
      tooltip: collapsed ? item.label : null,
      child: SizedBox(
        height: _itemHeight,
        child: Padding(
          // 图标的左右留白让它在收起后的窄栏里居中。
          padding: const EdgeInsets.symmetric(horizontal: (_railWidth - _iconSize) / 2),
          child: Row(
            children: [
              GlyphIcon(item.glyph, color: selected ? c.accent : c.fg, size: _iconSize),
              Expanded(
                child: ClipRect(
                  child: AnimatedOpacity(
                    duration: Motion.fast,
                    opacity: collapsed ? 0 : 1,
                    child: Padding(
                      padding: const EdgeInsets.only(left: 14),
                      child: Text(
                        item.label,
                        maxLines: 1,
                        softWrap: false,
                        overflow: TextOverflow.clip,
                        style: TextStyle(fontSize: 16, fontWeight: selected ? FontWeight.w700 : FontWeight.w500),
                      ),
                    ),
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
