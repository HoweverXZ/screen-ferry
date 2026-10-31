// 窄窗口（手机大小）时的底部标签栏：图标在上、文字在下，选中项的图标背后有一块胶囊。

import 'package:flutter/material.dart';

import '../state/app_scope.dart';
import '../theme/tokens.dart';
import '../widgets/glyph.dart';
import '../widgets/pressable.dart';
import 'nav_items.dart';

class BottomNav extends StatelessWidget {
  const BottomNav({super.key});

  static const height = 66.0;

  @override
  Widget build(BuildContext context) {
    final state = AppScope.of(context);
    final c = SfColors.of(context);
    final index = navIndexOf(state.page);
    return Container(
      height: height,
      padding: const EdgeInsets.fromLTRB(8, 5, 8, 5),
      decoration: BoxDecoration(
        color: c.side,
        border: Border(top: BorderSide(color: c.line)),
      ),
      child: Row(
        children: [
          for (var i = 0; i < navItems.length; i++)
            Expanded(
              child: _Tab(item: navItems[i], selected: i == index),
            ),
        ],
      ),
    );
  }
}

class _Tab extends StatelessWidget {
  const _Tab({required this.item, required this.selected});

  final NavItem item;
  final bool selected;

  @override
  Widget build(BuildContext context) {
    final c = SfColors.of(context);
    final state = AppScope.read(context);
    final ink = selected ? c.accent : c.fg;
    return Pressable(
      onTap: () => state.go(item.page),
      hoverColor: selected ? c.navHover.withValues(alpha: 0) : c.navHover,
      radius: 14,
      pressScale: 0.94,
      selected: selected,
      semanticLabel: item.label,
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          AnimatedContainer(
            duration: Motion.fast,
            curve: Motion.ease,
            width: 52,
            height: 30,
            alignment: Alignment.center,
            decoration: ShapeDecoration(
              color: c.navPill.withValues(alpha: selected ? 1 : 0),
              shape: const StadiumBorder(),
            ),
            child: GlyphIcon(item.glyph, color: ink),
          ),
          const SizedBox(height: 3),
          AnimatedDefaultTextStyle(
            duration: Motion.fast,
            style: TextStyle(
              fontSize: 12,
              height: 1.2,
              color: ink,
              fontWeight: selected ? FontWeight.w700 : FontWeight.w500,
            ),
            child: Text(item.label, maxLines: 1, softWrap: false, overflow: TextOverflow.fade),
          ),
        ],
      ),
    );
  }
}
