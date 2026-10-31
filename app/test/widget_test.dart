import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:screenferry_settings/app.dart';
import 'package:screenferry_settings/shell/bottom_nav.dart';
import 'package:screenferry_settings/shell/sidebar.dart';
import 'package:screenferry_settings/state/app_state.dart';
import 'package:screenferry_settings/theme/tokens.dart';
import 'package:screenferry_settings/widgets/glyph.dart';
import 'package:screenferry_settings/widgets/page_scaffold.dart';
import 'package:screenferry_settings/widgets/surface.dart';

/// 花朵一直在转，等不到“全部动画停下”，所以按固定时长推进。
Future<void> settle(WidgetTester tester, {int steps = 6}) async {
  for (var i = 0; i < steps; i++) {
    await tester.pump(const Duration(milliseconds: 150));
  }
}

Future<AppState> launch(WidgetTester tester, Size size) async {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  final state = AppState();
  addTearDown(state.dispose);
  await tester.pumpWidget(ScreenFerryApp(state: state));
  await settle(tester);
  return state;
}

Future<void> visitAll(WidgetTester tester, AppState state) async {
  for (final page in [SfPage.devices, SfPage.layout, SfPage.log, SfPage.settings, SfPage.about, SfPage.overview]) {
    state.go(page);
    await settle(tester);
  }
}

Finder navItem(String label) => find.descendant(of: find.byType(Sidebar), matching: find.text(label));

/// 页面大标题所在标题栏的底色。
Color headerColor(WidgetTester tester, String title) {
  final text = find.descendant(of: find.byType(CustomScrollView), matching: find.text(title)).first;
  return tester.widget<Container>(find.ancestor(of: text, matching: find.byType(Container)).first).color!;
}

void main() {
  for (final (name, size) in [
    ('默认窗口', const Size(1160, 780)),
    ('最小窗口', const Size(880, 600)),
    ('窄屏', const Size(600, 900)),
    ('手机大小', const Size(375, 667)),
  ]) {
    testWidgets('$name下所有页面都能正常显示', (tester) async {
      final state = await launch(tester, size);
      expect(find.text('仪表盘'), findsWidgets);
      await visitAll(tester, state);
      expect(find.text('往返延迟'), findsOneWidget);
    });
  }

  testWidgets('点花朵中心暂停与继续共享', (tester) async {
    final state = await launch(tester, const Size(1160, 780));
    await tester.tap(find.byTooltip('暂停共享'));
    await settle(tester);
    expect(state.sharing, isFalse);
    expect(find.text('已暂停'), findsOneWidget);

    await tester.tap(find.byTooltip('继续共享'));
    await settle(tester);
    expect(state.sharing, isTrue);
  });

  testWidgets('配对局域网里发现的设备', (tester) async {
    final state = await launch(tester, const Size(1160, 780));
    state.go(SfPage.devices);
    await settle(tester);

    await tester.tap(find.text('配对'));
    await settle(tester);
    expect(find.text('与“会议室 PC”配对'), findsOneWidget);

    await tester.tap(find.text('数字一致，配对'));
    await settle(tester, steps: 12);
    expect(state.paired.first.name, '会议室 PC');
    expect(state.found, isEmpty);
    expect(find.text('局域网里没有其他设备'), findsOneWidget);
  });

  testWidgets('移除设备前要在卡片上确认', (tester) async {
    final state = await launch(tester, const Size(1160, 780));
    state.go(SfPage.devices);
    await settle(tester);

    await tester.tap(find.text('移除').first);
    await settle(tester);
    expect(find.text('移除后需重新配对'), findsOneWidget);
    expect(state.paired, hasLength(2), reason: '确认前不能移除');

    // 第一张卡片底部现在是“取消 / 移除”，它的“移除”仍是第一个。
    await tester.tap(find.text('移除').first);
    await settle(tester);
    expect(state.paired.map((d) => d.name), isNot(contains('办公室 PC')));
  });

  testWidgets('日志可以筛选和清空', (tester) async {
    final state = await launch(tester, const Size(1160, 780));
    state.go(SfPage.log);
    await settle(tester);

    final total = state.log.length;
    expect(find.text('共 $total 条'), findsOneWidget);
    await tester.tap(find.text('剪贴板').first);
    await settle(tester);
    final clips = state.log.where((e) => e.kind == LogKind.clip).length;
    expect(find.text('共 $clips 条'), findsOneWidget);

    await tester.tap(find.text('清空'));
    await settle(tester);
    expect(find.text('暂无记录'), findsOneWidget);
  });

  testWidgets('切换深色主题和强调色', (tester) async {
    final state = await launch(tester, const Size(1160, 780));
    state.go(SfPage.settings);
    await settle(tester);

    await tester.tap(find.text('深色'));
    await settle(tester);
    expect(state.appearance.mode, ThemeMode.dark);

    await tester.tap(find.byTooltip('紫色'));
    await settle(tester);
    expect(state.appearance.accent, 2);
  });

  testWidgets('点侧栏菜单切换页面', (tester) async {
    final state = await launch(tester, const Size(1160, 780));
    for (final (label, page) in [
      ('设备', SfPage.devices),
      ('布局', SfPage.layout),
      ('日志', SfPage.log),
      ('设置', SfPage.settings),
      ('仪表盘', SfPage.overview),
    ]) {
      await tester.tap(navItem(label));
      await settle(tester);
      expect(state.page, page);
    }
  });

  testWidgets('切换主题后标题栏跟着换色', (tester) async {
    final state = await launch(tester, const Size(1160, 780));
    state.go(SfPage.settings);
    await settle(tester);
    expect(headerColor(tester, '设置'), SfColors.light.bg);

    await tester.tap(find.text('深色'));
    await settle(tester);
    expect(headerColor(tester, '设置'), SfColors.dark.bg);

    await tester.tap(find.text('浅色'));
    await settle(tester);
    expect(headerColor(tester, '设置'), SfColors.light.bg);
  });

  testWidgets('切换页面时新旧两页不会同时可见', (tester) async {
    final state = await launch(tester, const Size(1160, 780));
    state.go(SfPage.devices);
    var sawOld = false;
    var sawNew = false;
    for (var ms = 0; ms < 420; ms += 20) {
      await tester.pump(const Duration(milliseconds: 20));
      final pages = find.byType(PageScaffold).evaluate().toList();
      final visible = [
        for (final page in pages)
          tester
              .widget<Opacity>(
                find.ancestor(of: find.byElementPredicate((e) => e == page), matching: find.byType(Opacity)).first,
              )
              .opacity,
      ].where((o) => o > 0.01).length;
      expect(visible, lessThanOrEqualTo(1), reason: '第 $ms 毫秒有 $visible 页同时可见');
      if (pages.length == 2) sawOld = true;
      if (pages.length == 1) sawNew = true;
    }
    expect(sawOld && sawNew, isTrue);
  });

  testWidgets('鼠标移开菜单项时底色不会先变暗', (tester) async {
    await launch(tester, const Size(1160, 780));
    // 悬停色本身比侧栏底色略深；渐变过程中不能比悬停色更暗。
    final side = SfColors.light.side;
    final floor = SfColors.light.navHover.computeLuminance() - 0.005;
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: Offset.zero);
    addTearDown(mouse.removePointer);

    final item = navItem('布局');
    final box = find.ancestor(of: item, matching: find.byType(AnimatedContainer)).first;
    Color fill() {
      final deco = tester.widget<DecoratedBox>(find.descendant(of: box, matching: find.byType(DecoratedBox)).first);
      return Color.alphaBlend((deco.decoration as ShapeDecoration).color!, side);
    }

    var sawHover = false;
    for (final target in [tester.getCenter(item), const Offset(700, 400)]) {
      await mouse.moveTo(target);
      for (var i = 0; i < 12; i++) {
        await tester.pump(const Duration(milliseconds: 20));
        if (fill() != side) sawHover = true;
        expect(fill().computeLuminance(), greaterThan(floor));
      }
    }
    // 确认真的出现过悬停色，测试不是空跑。
    expect(sawHover, isTrue);
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));

  for (final (name, size, wide) in [('宽窗口', const Size(1160, 780), true), ('窄屏', const Size(600, 900), false)]) {
    testWidgets('$name下横幅按钮的位置', (tester) async {
      final state = await launch(tester, size);
      for (final (page, label) in [(SfPage.devices, '复制指纹'), (SfPage.about, '检查更新')]) {
        state.go(page);
        await settle(tester);
        final banner = tester.getRect(find.byType(BannerRow));
        final button = tester.getRect(find.ancestor(of: find.text(label), matching: find.byType(PillButton)));
        if (wide) {
          // 按钮贴着横幅右边，和左边的图标一样留出卡片内边距。
          expect(button.right, moreOrLessEquals(banner.right), reason: label);
          expect(button.center.dy, moreOrLessEquals(banner.center.dy, epsilon: 1), reason: label);
        } else {
          expect(button.left, moreOrLessEquals(banner.left), reason: label);
          expect(button.bottom, moreOrLessEquals(banner.bottom), reason: label);
        }
      }
    });
  }

  for (final (name, size) in [('点按钮收起', const Size(1160, 780)), ('窗口太窄自动收起', const Size(600, 900))]) {
    testWidgets('侧栏$name后图标和状态圆点都居中', (tester) async {
      final state = await launch(tester, size);
      if (!state.sidebarCollapsed && size.width >= 900) {
        await tester.tap(find.byTooltip('收起侧栏'));
        await settle(tester);
      }
      final side = tester.getRect(find.byType(Sidebar));
      expect(side.width, collapsedWidth);
      // 内容区左右各留 14，右边还有 1 点分隔线。
      final center = side.left + 14 + (collapsedWidth - 29) / 2;
      final marks = [
        ...find.descendant(of: find.byType(Sidebar), matching: find.byType(GlyphIcon)).evaluate(),
        ...find.descendant(of: find.byType(Sidebar), matching: find.byType(StatusDot)).evaluate(),
      ];
      expect(marks.length, greaterThanOrEqualTo(6));
      for (final e in marks) {
        expect(tester.getCenter(find.byElementPredicate((x) => x == e)).dx, moreOrLessEquals(center), reason: '$e');
      }
    });
  }

  testWidgets('手机大小时用底部标签栏切换页面', (tester) async {
    final state = await launch(tester, const Size(375, 667));
    expect(find.byType(Sidebar), findsNothing);
    for (final (label, page) in [('设备', SfPage.devices), ('日志', SfPage.log), ('仪表盘', SfPage.overview)]) {
      await tester.tap(find.descendant(of: find.byType(BottomNav), matching: find.text(label)));
      await settle(tester);
      expect(state.page, page);
    }

    // 窗口拉宽后换回侧栏，当前页面不变。
    tester.view.physicalSize = const Size(1160, 780);
    await settle(tester);
    expect(find.byType(BottomNav), findsNothing);
    expect(find.byType(Sidebar), findsOneWidget);
    expect(state.page, SfPage.overview);
  });
}
