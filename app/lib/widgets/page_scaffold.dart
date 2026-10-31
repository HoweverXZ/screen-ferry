// 页面骨架：大标题固定在顶部，内容从它下面滚过时显示毛玻璃。

import 'dart:ui';

import 'package:flutter/material.dart';

import '../theme/tokens.dart';

class PageScaffold extends StatelessWidget {
  const PageScaffold({super.key, required this.title, this.leading, this.actions = const [], required this.children});

  final String title;
  final Widget? leading;
  final List<Widget> actions;
  final List<Widget> children;

  static const maxContentWidth = 1064.0;

  @override
  Widget build(BuildContext context) {
    // 在这里读颜色：主题一变，本页就重建并换一个新的标题栏代理，标题栏才会跟着重绘。
    final c = SfColors.of(context);
    return LayoutBuilder(
      builder: (context, box) {
        final narrow = box.maxWidth < 560;
        final gutter = narrow ? 16.0 : 28.0;
        return CustomScrollView(
          slivers: [
            SliverPersistentHeader(
              pinned: true,
              delegate: _Header(
                title: title,
                leading: leading,
                actions: actions,
                gutter: gutter,
                titleSize: narrow ? 26 : 30,
                bg: c.bg,
                fg: c.fg,
              ),
            ),
            SliverPadding(
              padding: EdgeInsets.fromLTRB(gutter, 2, gutter, 48),
              sliver: SliverToBoxAdapter(
                child: Center(
                  child: ConstrainedBox(
                    constraints: const BoxConstraints(maxWidth: maxContentWidth),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        for (var i = 0; i < children.length; i++) ...[
                          if (i > 0) const SizedBox(height: 16),
                          children[i],
                        ],
                      ],
                    ),
                  ),
                ),
              ),
            ),
          ],
        );
      },
    );
  }
}

class _Header extends SliverPersistentHeaderDelegate {
  _Header({
    required this.title,
    required this.leading,
    required this.actions,
    required this.gutter,
    required this.titleSize,
    required this.bg,
    required this.fg,
  });

  final String title;
  final Widget? leading;
  final List<Widget> actions;
  final double gutter;
  final double titleSize;
  final Color bg;
  final Color fg;

  /// 顶部 28 点是 macOS 透明标题栏（红绿灯所在行），标题放在它下面。
  static const height = 92.0;

  @override
  double get minExtent => height;

  @override
  double get maxExtent => height;

  @override
  Widget build(BuildContext context, double shrinkOffset, bool overlapsContent) {
    Widget bar = Container(
      color: overlapsContent ? bg.withValues(alpha: 0.74) : bg,
      padding: EdgeInsets.fromLTRB(gutter, 32, gutter, 14),
      alignment: Alignment.center,
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: PageScaffold.maxContentWidth),
        child: Row(
          children: [
            if (leading != null) ...[leading!, const SizedBox(width: 14)],
            Expanded(
              child: Semantics(
                header: true,
                child: Text(
                  title,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(fontSize: titleSize, fontWeight: FontWeight.w700, height: 1.2, color: fg),
                ),
              ),
            ),
            for (final a in actions) ...[const SizedBox(width: 12), a],
          ],
        ),
      ),
    );
    // 只有内容滚到标题下面时才开毛玻璃：平时不付出模糊的开销。
    if (overlapsContent) {
      bar = ClipRect(
        child: BackdropFilter(filter: ImageFilter.blur(sigmaX: 20, sigmaY: 20), child: bar),
      );
    }
    return bar;
  }

  @override
  bool shouldRebuild(_Header old) =>
      old.title != title ||
      old.leading != leading ||
      old.actions != actions ||
      old.gutter != gutter ||
      old.titleSize != titleSize ||
      old.bg != bg ||
      old.fg != fg;
}
